use crate::event::Event;
use crate::protocol::{Command, INITIALIZE, Incoming};

/// Where a session's agent stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// The process is up but hasn't answered yet.
    Starting,
    /// Idle until the next message.
    Ready,
    /// Working on a message, or holding some in its queue.
    Running,
    /// Waiting for the person to allow or deny a tool.
    AwaitingPermission,
    /// Asked to stop; what it had open hasn't ended yet.
    Stopping,
    /// The process ended; `None` when a signal ended it.
    Exited { code: Option<i32> },
    /// It couldn't start, or refused the session.
    Failed { message: String },
}

/// What the turns hold once an event is applied, as far as status cares.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Work {
    /// A turn is queued or running.
    pub busy: bool,
    /// A permission prompt waits for an answer.
    pub asking: bool,
}

impl Status {
    /// Every change of status: the status after `event`, with `work` what the
    /// turns hold once it is applied.
    pub(crate) fn next(&self, event: &Event, work: Work) -> Status {
        let working = if work.asking {
            Status::AwaitingPermission
        } else if work.busy {
            Status::Running
        } else {
            Status::Ready
        };
        match (self, event) {
            (_, Event::Started) => Status::Starting,
            (_, Event::Failed(message)) => Status::Failed {
                message: message.clone(),
            },
            (Status::Failed { .. }, _) => self.clone(),
            (_, Event::Exited { code }) => Status::Exited { code: *code },
            (Status::Exited { .. }, _) => self.clone(),
            (Status::Starting, Event::Received(Incoming::Response { id, result }))
                if id.0 == INITIALIZE =>
            {
                match result {
                    Ok(_) => working,
                    Err(error) => Status::Failed {
                        message: error.clone(),
                    },
                }
            }
            (Status::Starting, _) => Status::Starting,
            (_, Event::Sent(Command::Interrupt)) if work.busy => Status::Stopping,
            (Status::Stopping, Event::Sent(Command::Send(_))) => working,
            (Status::Stopping, _) if work.busy => Status::Stopping,
            _ => working,
        }
    }
}
