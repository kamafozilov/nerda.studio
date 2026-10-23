//! Runs a coding agent's session as a process and reads what it says into
//! typed events, with no UI: the screen drains the events and shows them.

mod event;
pub mod protocol;
mod session;
mod transcript;

pub use event::Event;
pub use protocol::{
    Attachment, Command, ConversationId, Decision, Image, ImageFormat, Incoming, Message,
    MessageId, ParseError, RequestId, ToolUseId,
};
pub use session::{Conversation, Events, Launch, Session, StartError};
pub use transcript::{
    Ending, Finished, Item, Permission, PermissionState, Status, Text, ToolCall, ToolStatus,
    Transcript, Turn, TurnState,
};
