//! Runs a coding agent's session as a process and reads what it says into
//! typed events, with no UI: the screen drains the events and shows them.

pub mod protocol;

pub use protocol::{
    Attachment, Command, ConversationId, Decision, Image, ImageFormat, Incoming, Message,
    MessageId, ParseError, RequestId, ToolUseId,
};
