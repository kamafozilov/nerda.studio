use serde::{Deserialize, Serialize};

use crate::protocol::{Command, Incoming, Message, ParseError};
use crate::session::Refusal;

/// Something that happened in a session, in the order it happened: what a
/// transcript is reduced from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A process began for the session, new or resuming its conversation.
    Started,
    /// The app sent this; it comes before anything the agent says about it.
    Sent(Command),
    /// The session didn't carry out `command`, for `reason`.
    Refused { command: Command, reason: Refusal },
    /// The agent dropped this message, waiting behind the turn that ran,
    /// when it was stopped; it never ran, and can be sent again.
    Dropped(Message),
    /// The agent said this.
    Received(Incoming),
    /// The agent wrote a line that isn't one of its events; it is skipped.
    Unreadable { line: String, error: ParseError },
    /// A line the agent wrote on stderr, such as why it refused to start.
    Stderr(String),
    /// The process ended; `None` when a signal ended it.
    Exited { code: Option<i32> },
    /// The session couldn't start, in words to show.
    Failed(String),
}
