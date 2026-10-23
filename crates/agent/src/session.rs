//! A session's agent as a process: started in the project folder, fed
//! commands on stdin, read on threads, and ended when the session is dropped.

use std::ffi::OsString;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use workspace::{Agent, AgentChoice, AgentId};

use crate::event::Event;
use crate::protocol::{self, Command, ConversationId, Incoming, RequestId};

/// What makes the agent read and write one JSON object per line, stream its
/// reply, echo the messages it takes up, and ask the app before a tool runs.
const STREAM_JSON: &[&str] = &[
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    "--replay-user-messages",
    "--permission-prompt-tool",
    "stdio",
];

/// How long a dropped session's agent has to quit on its own before it is
/// killed. Closing its input is enough for an idle one.
const GRACE: Duration = Duration::from_secs(2);

/// How often the session looks whether its agent still runs.
const WATCH: Duration = Duration::from_millis(50);

/// How long the exit waits, once the agent has quit, for the last of what it
/// wrote. A process the agent started can hold its pipes open past its end.
const TAIL: Duration = Duration::from_millis(500);

/// On macOS a pipe is made close-on-exec in a second step, so a child started
/// by another thread in between keeps the pipe open. Starting one agent at a
/// time keeps agents off each other's pipes.
static SPAWNING: Mutex<()> = Mutex::new(());

/// Which conversation a process runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversation {
    /// A new one under this id, which a later process resumes it by.
    New(ConversationId),
    /// One that ran before.
    Resume(ConversationId),
}

#[derive(Clone, Debug)]
pub struct Launch {
    /// The project folder, where the agent works.
    pub folder: PathBuf,
    pub choice: AgentChoice,
    pub conversation: Conversation,
    /// Run in place of the agent's own command, which is looked up on PATH.
    pub program: Option<PathBuf>,
}

/// A session's events, oldest first; drained by one task.
pub type Events = Receiver<Event>;

/// A running agent. Dropping it closes the agent's input and kills the
/// agent if it hasn't quit within a moment.
pub struct Session {
    commands: mpsc::Sender<Outgoing>,
    /// When the agent has to be gone by; set once the session is dropped.
    kill_at: Arc<OnceLock<Instant>>,
    pid: u32,
}

enum Outgoing {
    Command(Command),
    Decline(RequestId),
    Close,
}

#[derive(Debug)]
pub enum StartError {
    UnknownAgent(AgentId),
    NoFolder { program: String, folder: PathBuf },
    NotFound { program: String },
    Io { program: String, error: io::Error },
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::UnknownAgent(id) => {
                write!(f, "This build doesn't know the agent {}.", id.0)
            }
            StartError::NoFolder { program, folder } => write!(
                f,
                "Couldn't start {program}: the folder {} isn't there.",
                folder.display()
            ),
            StartError::NotFound { program } => write!(
                f,
                "Couldn't start {program}: it isn't installed, or isn't on PATH."
            ),
            StartError::Io { program, error } => write!(f, "Couldn't start {program}: {error}."),
        }
    }
}

impl std::error::Error for StartError {}

impl Session {
    /// Starts the agent and returns it with its events, `Event::Started` first.
    pub fn start(launch: &Launch) -> Result<(Session, Events), StartError> {
        let choice = &launch.choice;
        let agent = workspace::agent(&choice.agent)
            .ok_or_else(|| StartError::UnknownAgent(choice.agent.clone()))?;
        let program: OsString = match &launch.program {
            Some(program) => program.into(),
            None => agent.id.into(),
        };
        let name = Path::new(&program)
            .file_name()
            .unwrap_or(&program)
            .to_string_lossy()
            .into_owned();
        if !launch.folder.is_dir() {
            let folder = launch.folder.clone();
            return Err(StartError::NoFolder {
                program: name,
                folder,
            });
        }
        let mut process = std::process::Command::new(&program);
        process
            .args(arguments(agent, launch))
            .current_dir(&launch.folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            process.creation_flags(CREATE_NO_WINDOW);
        }
        let spawning = SPAWNING.lock().unwrap_or_else(PoisonError::into_inner);
        let spawned = process.spawn();
        drop(spawning);
        let mut child = spawned.map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => StartError::NotFound { program: name },
            _ => StartError::Io {
                program: name,
                error,
            },
        })?;

        let (events, receiver) = async_channel::unbounded();
        let _ = events.try_send(Event::Started);
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
            unreachable!("all three are piped");
        };
        let pid = child.id();
        let kill_at = Arc::new(OnceLock::new());
        let (commands, inbox) = mpsc::channel();

        spawn("agent stdin", {
            let events = events.clone();
            move || write_commands(stdin, inbox, agent, events)
        });
        let readers = [
            spawn("agent stdout", {
                let (events, commands) = (events.clone(), commands.clone());
                move || read_lines(stdout, |line| send(&events, received(line, &commands)))
            }),
            spawn("agent stderr", {
                let events = events.clone();
                move || read_lines(stderr, |line| send(&events, Event::Stderr(line)))
            }),
        ];
        spawn("agent watch", {
            let kill_at = Arc::clone(&kill_at);
            move || watch(child, &kill_at, readers, &events)
        });
        Ok((
            Session {
                commands,
                kill_at,
                pid,
            },
            receiver,
        ))
    }

    /// Queues `command` for the agent; its `Event::Sent` comes as it is
    /// written. One sent after the agent quit shows in the transcript as
    /// not sent.
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(Outgoing::Command(command));
    }

    /// The agent's process id.
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.commands.send(Outgoing::Close);
        let _ = self.kill_at.set(Instant::now() + GRACE);
    }
}

fn arguments(agent: &Agent, launch: &Launch) -> Vec<String> {
    let mut arguments: Vec<String> = STREAM_JSON.iter().map(|&a| a.to_owned()).collect();
    arguments.extend(
        agent
            .arguments(&launch.choice)
            .into_iter()
            .map(str::to_owned),
    );
    let (flag, id) = match launch.conversation {
        Conversation::New(id) => ("--session-id", id),
        Conversation::Resume(id) => ("--resume", id),
    };
    arguments.extend([flag.to_owned(), id.to_string()]);
    arguments
}

/// Writes `initialize`, then each command as it comes, until the session
/// closes. Once the agent stops reading, each command is still reported
/// sent, so none goes missing without a word.
fn write_commands(
    mut stdin: ChildStdin,
    inbox: mpsc::Receiver<Outgoing>,
    agent: &'static Agent,
    events: Sender<Event>,
) {
    let mut write = |line: &str| {
        let mut line = line.to_owned();
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.flush())
    };
    let mut reading = write(&protocol::encode_initialize()).is_ok();
    let mut requests = 0;
    for outgoing in inbox {
        let line = match outgoing {
            Outgoing::Command(command) => {
                requests += 1;
                let request = RequestId(format!("host_{requests}"));
                let Some(line) = protocol::encode(&command, agent, &request) else {
                    continue;
                };
                send(&events, Event::Sent(command));
                line
            }
            Outgoing::Decline(request) => protocol::encode_decline(&request),
            Outgoing::Close => return,
        };
        reading = reading && write(&line).is_ok();
    }
}

/// One line of the agent's stdout as an event, declining a request this app
/// doesn't handle so the agent doesn't wait on it.
fn received(line: String, commands: &mpsc::Sender<Outgoing>) -> Event {
    match protocol::parse(&line) {
        Ok(incoming) => {
            if let Incoming::Request { id, .. } = &incoming {
                let _ = commands.send(Outgoing::Decline(id.clone()));
            }
            Event::Received(incoming)
        }
        Err(error) => Event::Unreadable { line, error },
    }
}

/// Waits for the agent to quit, killing it once `kill_at` passes, then
/// reports its exit after the last of what it wrote.
fn watch(
    mut child: Child,
    kill_at: &OnceLock<Instant>,
    readers: [JoinHandle<()>; 2],
    events: &Sender<Event>,
) {
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        if kill_at
            .get()
            .is_some_and(|&deadline| Instant::now() >= deadline)
        {
            let _ = child.kill();
        }
        thread::sleep(WATCH);
    };
    let deadline = Instant::now() + TAIL;
    while readers.iter().any(|reader| !reader.is_finished()) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    send(events, Event::Exited { code });
}

/// Each non-blank line, bytes that aren't UTF-8 replaced, until the end or a
/// read error.
fn read_lines(source: impl Read, mut each: impl FnMut(String)) {
    let mut source = BufReader::new(source);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match source.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&bytes);
        let line = line.trim_end_matches(['\n', '\r']);
        if !line.trim().is_empty() {
            each(line.to_owned());
        }
    }
}

/// Events nobody drains any more are dropped; the agent is still read, so
/// it never blocks on a full pipe.
fn send(events: &Sender<Event>, event: Event) {
    let _ = events.try_send(event);
}

fn spawn<T: Send + 'static>(
    name: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> JoinHandle<T> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(work)
        .expect("the system lets a thread start")
}
