//! One run of the agent's program: started in the project folder, read on
//! threads, and ended together with every process it started.

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use workspace::{Agent, AgentChoice};

use super::activity::Emitter;
use super::tree::Tree;
use super::{Conversation, StartError, refresh_lock};
use crate::event::Event;
use crate::protocol::{self, Incoming};
use crate::{program, spawning};

/// What makes the agent read and write one JSON object per line, stream its
/// reply, echo the messages it takes up, write a summary of its thinking
/// rather than nothing, and ask the app before a tool runs.
const STREAM_JSON: &[&str] = &[
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    "--replay-user-messages",
    "--thinking-display",
    "summarized",
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

/// How often the watch looks whether the agent still runs.
const WATCH: Duration = Duration::from_millis(50);

/// How long the exit waits, once the agent has quit, for the last of what it
/// wrote. A process the agent started can hold its pipes open past its end.
const TAIL: Duration = Duration::from_millis(500);

/// What a session runs, whichever process runs it.
#[derive(Clone)]
pub(super) struct Spec {
    pub agent: &'static Agent,
    pub folder: PathBuf,
    pub program: Option<PathBuf>,
}

pub(super) struct Process {
    /// The agent's input, until it is closed.
    input: Arc<Mutex<Option<ChildStdin>>>,
    pub pid: u32,
    /// Set to end the process.
    closing: Arc<OnceLock<()>>,
    watch: JoinHandle<()>,
}

impl Process {
    /// Starts the agent on `choice` and `conversation`; `Event::Started` is
    /// the first event it sends, then it writes `initialize`.
    pub fn start(
        spec: &Spec,
        choice: &AgentChoice,
        conversation: &Conversation,
        emitter: &Arc<Emitter>,
    ) -> Result<Self, StartError> {
        let program: OsString = match &spec.program {
            Some(program) => program.into(),
            None => spec.agent.id.into(),
        };
        let name = Path::new(&program).file_name().unwrap_or(&program);
        let name = name.to_string_lossy().into_owned();
        if !spec.folder.is_dir() {
            let folder = spec.folder.clone();
            return Err(StartError::NoFolder {
                program: name,
                folder,
            });
        }
        let not_found = || StartError::NotFound {
            program: name.clone(),
            install: spec.agent.install,
        };
        let path = program::find(&program).ok_or_else(not_found)?;
        let mut command = std::process::Command::new(&path);
        if let Some(search) = program::child_path(&path) {
            command.env("PATH", search);
        }
        command
            .args(arguments(spec.agent, choice, conversation))
            .current_dir(&spec.folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
        }
        let mut child = spawning::spawn(&mut command).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => not_found(),
            _ => StartError::Io {
                program: name.clone(),
                error,
            },
        })?;

        emitter.emit(Event::Started);
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
            unreachable!("all three are piped");
        };
        let input = Arc::new(Mutex::new(Some(stdin)));
        let closing = Arc::new(OnceLock::new());
        let readers = [
            spawn("agent stdout", {
                let (emitter, input) = (Arc::clone(emitter), Arc::clone(&input));
                move || read_lines(stdout, |line| emitter.emit(received(line, &input)))
            }),
            spawn("agent stderr", {
                let emitter = Arc::clone(emitter);
                move || read_lines(stderr, |line| emitter.emit(Event::Stderr(line)))
            }),
        ];
        let pid = child.id();
        let watching = Watching {
            input: Arc::clone(&input),
            closing: Arc::clone(&closing),
            readers,
            emitter: Arc::clone(emitter),
            config_dir: refresh_lock::config_dir(),
        };
        let watch = spawn("agent watch", move || watch(child, watching));
        let process = Process {
            input,
            pid,
            closing,
            watch,
        };
        process.write(&protocol::encode_initialize());
        Ok(process)
    }

    /// Writes one line; false once the agent no longer reads.
    pub fn write(&self, line: &str) -> bool {
        write_line(&self.input, line)
    }

    /// Ends the agent and what it started; see `watch`.
    pub fn end(&self) {
        let _ = self.closing.set(());
    }

    /// Ends the agent and waits until it and what it started are gone, and
    /// its exit is reported.
    pub fn end_and_wait(self) {
        self.end();
        let _ = self.watch.join();
    }
}

fn arguments(agent: &Agent, choice: &AgentChoice, conversation: &Conversation) -> Vec<String> {
    let mut arguments: Vec<String> = STREAM_JSON.iter().map(|&a| a.to_owned()).collect();
    arguments.extend(agent.arguments(choice).into_iter().map(str::to_owned));
    let (flag, id) = match conversation {
        Conversation::New(id) => ("--session-id", id),
        Conversation::Resume(id) => ("--resume", id),
    };
    arguments.extend([flag.to_owned(), id.to_string()]);
    arguments
}

fn write_line(input: &Mutex<Option<ChildStdin>>, line: &str) -> bool {
    let mut input = input.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(stdin) = input.as_mut() else {
        return false;
    };
    let mut line = line.to_owned();
    line.push('\n');
    stdin
        .write_all(line.as_bytes())
        .and_then(|()| stdin.flush())
        .is_ok()
}

/// One line of the agent's stdout as an event, declining a request this app
/// doesn't handle so the agent doesn't wait on it.
fn received(line: String, input: &Mutex<Option<ChildStdin>>) -> Event {
    match protocol::parse(&line) {
        Ok(incoming) => {
            if let Incoming::Request { id, .. } = &incoming {
                write_line(input, &protocol::encode_decline(id));
            }
            Event::Received(incoming)
        }
        Err(error) => Event::Unreadable { line, error },
    }
}

struct Watching {
    input: Arc<Mutex<Option<ChildStdin>>>,
    closing: Arc<OnceLock<()>>,
    readers: [JoinHandle<()>; 2],
    emitter: Arc<Emitter>,
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
/// Once the process is ended, it notes what the agent has started, closes
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
                watching
                    .input
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take();
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
    watching.emitter.emit(Event::Exited { code });
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

pub(super) fn spawn<T: Send + 'static>(
    name: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> JoinHandle<T> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(work)
        .expect("the system lets a thread start")
}
