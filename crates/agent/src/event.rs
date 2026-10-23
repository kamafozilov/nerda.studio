use crate::protocol::{Command, Incoming, ParseError};

/// Something that happened in a session, in the order it happened: what a
/// transcript is reduced from.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A process began for the session, new or resuming its conversation.
    Started,
    /// The app sent this; it comes before anything the agent says about it.
    Sent(Command),
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
