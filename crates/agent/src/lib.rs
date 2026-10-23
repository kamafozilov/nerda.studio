//! Runs a coding agent's session as a process and reads what it says into
//! typed events, with no UI: the screen drains the events and shows them.

mod event;
mod failure;
mod program;
pub mod protocol;
mod session;
mod spawning;
mod transcript;

pub use event::Event;
pub use failure::Failure;
pub use program::prewarm;
pub use protocol::{
    Ask, Attachment, Choice, Command, ConversationId, Decision, Image, ImageFormat, Incoming,
    Message, MessageId, ParseError, Question, RequestId, TaskId, ToolUseId,
};
pub use session::{Conversation, Events, Launch, Refusal, Session, StartError};
pub use transcript::{
    Answer, Ending, Finished, Item, Permission, PermissionState, Status, Text, ToolCall,
    ToolStatus, Transcript, Turn, TurnState,
};
