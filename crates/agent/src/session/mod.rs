//! A session's agent as a process: started in the project folder, fed
//! commands on stdin, read on threads, and ended when the session is dropped,
//! together with every process it started.

mod refresh_lock;
mod tree;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, OnceLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use workspace::{Agent, AgentChoice, AgentId};

use crate::event::Event;
use crate::protocol::{self, Command, ConversationId, Incoming, MessageId, RequestId};
use crate::{program, spawning};
use tree::Tree;

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

/// How long an agent whose input closed has to quit before it is sent
/// SIGTERM, and then how long it has before it is killed. Closing its input
/// is enough for an idle one.
const GRACE: Duration = Duration::from_secs(2);

/// How long what the agent started has, once it has quit, to end on SIGTERM
/// before it is killed.
const LEFTOVERS: Duration = Duration::from_secs(1);

/// How often the session looks whether its agent still runs.
const WATCH: Duration = Duration::from_millis(50);

/// How long the exit waits, once the agent has quit, for the last of what it
/// wrote. A process the agent started can hold its pipes open past its end.
const TAIL: Duration = Duration::from_millis(500);

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
    /// Run in place of the agent's own command. A bare name is looked up as
    /// the agent's is: on PATH, the login shell's PATH, then where installers
    /// put it.
    pub program: Option<PathBuf>,
}

/// A session's events, oldest first; drained by one task.
pub type Events = Receiver<Event>;

/// A running agent. Dropping it ends the agent and what it started: see
/// `watch`.
pub struct Session {
    commands: mpsc::Sender<Outgoing>,
    /// Set once the session is dropped.
    closing: Arc<OnceLock<()>>,
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
    NoFolder {
        program: String,
        folder: PathBuf,
    },
    /// Nowhere to be found; `install` is the command that installs it.
    NotFound {
        program: String,
        install: &'static str,
    },
    Io {
        program: String,
        error: io::Error,
    },
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
            StartError::NotFound { program, install } => write!(
                f,
                "Couldn't find {program}. Install it with `{install}` in a terminal, then try again."
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
        let not_found = || StartError::NotFound {
            program: name.clone(),
            install: agent.install,
        };
        let path = program::find(&program).ok_or_else(not_found)?;
        let mut process = std::process::Command::new(&path);
        if let Some(search) = program::child_path(&path) {
            process.env("PATH", search);
        }
        process
            .args(arguments(agent, launch))
            .current_dir(&launch.folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            process.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            process.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
        }
        let mut child = spawning::spawn(&mut process).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => not_found(),
            _ => StartError::Io {
                program: name.clone(),
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
        let closing = Arc::new(OnceLock::new());
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
            let watching = Watching {
                closing: Arc::clone(&closing),
                commands: commands.clone(),
                readers,
                events,
                config_dir: refresh_lock::config_dir(),
            };
            move || watch(child, watching)
        });
        Ok((
            Session {
                commands,
                closing,
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
        let _ = self.closing.set(());
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
/// sent, so none goes missing without a word. Each message goes under a new
/// id: the agent drops a message whose id its conversation has seen, so one
/// sent again would never run.
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
            Outgoing::Command(mut command) => {
                if let Command::Send(message) = &mut command {
                    message.id = MessageId::new();
                }
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

struct Watching {
    closing: Arc<OnceLock<()>>,
    commands: mpsc::Sender<Outgoing>,
    readers: [JoinHandle<()>; 2],
    events: Sender<Event>,
    /// Where the agent keeps its sign-in.
    config_dir: PathBuf,
}

/// How far ending the agent has gone.
#[derive(Clone, Copy)]
enum Shutdown {
    Running,
    /// Its input is closed; SIGTERM follows at the instant.
    Closed(Instant),
    /// SIGTERM was sent; SIGKILL follows at the instant.
    Terminated(Instant),
    Killed,
}

/// Waits for the agent to quit, then ends what is left of what it started
/// and reports the exit after the last of what it wrote.
///
/// Once the session is dropped, it notes what the agent has started, closes
/// the agent's input, sends SIGTERM to the agent's group if the agent hasn't
/// quit within `GRACE`, and SIGKILL after another. Before closing the input
/// and before each signal it waits, a bounded time, while the agent is
/// refreshing its sign-in, so ending it never signs the person out.
fn watch(mut child: Child, watching: Watching) {
    let mut tree = Tree::new(child.id());
    let mut shutdown = Shutdown::Running;
    let settle = || refresh_lock::settle(&watching.config_dir, refresh_lock::SETTLE);
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        let now = Instant::now();
        shutdown = match shutdown {
            Shutdown::Running if watching.closing.get().is_some() => {
                tree.snapshot();
                settle();
                let _ = watching.commands.send(Outgoing::Close);
                Shutdown::Closed(Instant::now() + GRACE)
            }
            Shutdown::Closed(at) if now >= at => {
                tree.snapshot();
                settle();
                tree.terminate();
                Shutdown::Terminated(Instant::now() + GRACE)
            }
            Shutdown::Terminated(at) if now >= at => {
                settle();
                tree.kill();
                let _ = child.kill();
                Shutdown::Killed
            }
            shutdown => shutdown,
        };
        thread::sleep(WATCH);
    };
    tree.end_leftovers(LEFTOVERS);
    let deadline = Instant::now() + TAIL;
    while watching.readers.iter().any(|reader| !reader.is_finished()) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    send(&watching.events, Event::Exited { code });
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
