//! A session's agent: one process at a time, fed commands in order by a
//! thread of its own, and ended when the session is dropped, together with
//! every process it started.

mod activity;
mod process;
mod refresh_lock;
mod tree;

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, mpsc};

use async_channel::Receiver;
use workspace::{Access, AgentChoice, AgentId};

use crate::event::Event;
use crate::protocol::{self, Command, ConversationId, MessageId, RequestId};
use activity::{Activity, Emitter};
use process::{Process, Spec};

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

/// A running agent. Dropping it ends the agent and what it started.
pub struct Session {
    requests: mpsc::Sender<Request>,
    /// The process running now; a new one replaces it when Full access is
    /// switched on or off.
    pid: Arc<AtomicU32>,
}

enum Request {
    Command(Command),
    End,
}

/// Why the session didn't carry out a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Switching to or from Full access starts the agent again, which would
    /// end the turn that runs or drop the messages that wait.
    TurnRunning,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::TurnRunning => f.write_str(
                "Full access can't be switched while the agent works. Switch once it is done, or stop it first.",
            ),
        }
    }
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
        let choice = launch.choice.clone();
        let agent = workspace::agent(&choice.agent)
            .ok_or_else(|| StartError::UnknownAgent(choice.agent.clone()))?;
        let spec = Spec {
            agent,
            folder: launch.folder.clone(),
            program: launch.program.clone(),
        };
        let (events, receiver) = async_channel::unbounded();
        let recorded = matches!(launch.conversation, Conversation::Resume(_));
        let emitter = Arc::new(Emitter::new(events, Activity::new(recorded)));
        let process = Process::start(&spec, &choice, &launch.conversation, &emitter)?;
        let pid = Arc::new(AtomicU32::new(process.pid));
        let (requests, inbox) = mpsc::channel();
        let conversation = match launch.conversation {
            Conversation::New(id) | Conversation::Resume(id) => id,
        };
        let supervisor = Supervisor {
            spec,
            choice,
            conversation,
            process: Some(process),
            emitter,
            pid: Arc::clone(&pid),
            requests: 0,
        };
        process::spawn("agent session", move || supervisor.run(inbox));
        Ok((Session { requests, pid }, receiver))
    }

    /// Queues `command` for the agent; its `Event::Sent` comes as it is
    /// written. One sent after the agent quit shows in the transcript as
    /// not sent.
    ///
    /// Switching to or from Full access starts the agent again on the same
    /// conversation, since a running agent can't change into or out of it;
    /// while a turn runs or a message waits it is refused with
    /// `Event::Refused` instead.
    pub fn send(&self, command: Command) {
        let _ = self.requests.send(Request::Command(command));
    }

    /// The process id of the agent running now.
    pub fn pid(&self) -> u32 {
        self.pid.load(Ordering::Relaxed)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::End);
    }
}

/// Carries out a session's commands in order, on the process running now.
struct Supervisor {
    spec: Spec,
    /// What runs now, as changed since the launch.
    choice: AgentChoice,
    conversation: ConversationId,
    /// `None` once a new process couldn't start.
    process: Option<Process>,
    emitter: Arc<Emitter>,
    pid: Arc<AtomicU32>,
    /// The app's control requests so far, which name each one.
    requests: u64,
}

impl Supervisor {
    fn run(mut self, inbox: mpsc::Receiver<Request>) {
        for request in inbox {
            match request {
                Request::Command(command) => self.command(command),
                Request::End => break,
            }
        }
        if let Some(process) = &self.process {
            process.end();
        }
    }

    fn command(&mut self, command: Command) {
        if let Command::SetAccess(access) = command
            && (access == Access::FullAccess) != (self.choice.access == Access::FullAccess)
        {
            return self.restart_in(access, command);
        }
        self.write(command);
    }

    /// Writes `command`, each message under a new id: the agent drops a
    /// message whose id its conversation has seen, so one sent again would
    /// never run. Once the agent stops reading, each command is still
    /// reported sent, so none goes missing without a word.
    fn write(&mut self, mut command: Command) {
        if let Command::Send(message) = &mut command {
            message.id = MessageId::new();
        }
        self.requests += 1;
        let request = RequestId(format!("host_{}", self.requests));
        let Some(line) = protocol::encode(&command, self.spec.agent, &request) else {
            return;
        };
        match &command {
            Command::SetModel(model) => self.choice.model = model.clone(),
            Command::SetEffort(effort) => self.choice.effort = *effort,
            Command::SetAccess(access) => self.choice.access = *access,
            _ => {}
        }
        self.emitter.emit(Event::Sent(command));
        if let Some(process) = &self.process {
            process.write(&line);
        }
    }

    /// Ends the agent and starts it again in `access`, resuming the
    /// conversation when it has a turn on record and starting it anew under
    /// the same id when it hasn't.
    fn restart_in(&mut self, access: Access, command: Command) {
        if self.emitter.activity(Activity::busy) {
            let reason = Refusal::TurnRunning;
            return self.emitter.emit(Event::Refused { command, reason });
        }
        self.emitter.emit(Event::Sent(command));
        if let Some(process) = self.process.take() {
            process.end_and_wait();
        }
        self.choice.access = access;
        let id = self.conversation;
        let conversation = match self.emitter.activity(Activity::recorded) {
            true => Conversation::Resume(id),
            false => Conversation::New(id),
        };
        match Process::start(&self.spec, &self.choice, &conversation, &self.emitter) {
            Ok(process) => {
                self.pid.store(process.pid, Ordering::Relaxed);
                self.process = Some(process);
            }
            Err(error) => self.emitter.emit(Event::Failed(error.to_string())),
        }
    }
}
