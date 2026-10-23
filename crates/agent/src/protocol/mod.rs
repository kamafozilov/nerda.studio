//! The wire between the app and the agent: one JSON object per line, the
//! app's commands on the agent's stdin and its events on stdout. Lines are
//! parsed here once, at the boundary; everything past it trusts the types.

mod incoming;
mod outgoing;

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;
use uuid::Uuid;

pub use incoming::{
    AgentStatus, Assistant, Block, BlockKind, Delta, Incoming, Init, Outcome, ParseError,
    PermissionRequest, RateLimit, Stage, Stream, ToolResult, TurnResult, Usage, parse,
};
pub use outgoing::{
    Attachment, Command, Decision, INITIALIZE, Image, ImageFormat, Message, encode, encode_decline,
    encode_initialize,
};

/// The id the app gives a message it sends; the agent names the message by it
/// when it takes the message up, queues it or answers it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub String);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// Names a control request, so its answer can find it. The app picks the ids
/// of its own requests; the agent picks the ids of its.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub String);

/// One tool call, as the agent names it in the call, its permission prompt and
/// its result.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct ToolUseId(pub String);

/// The agent's own id for a conversation: a new session starts with one the
/// app chose, and a later process resumes the conversation by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConversationId(Uuid);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ConversationId {
    type Err = uuid::Error;

    /// Reads a saved id; the agent takes only a UUID.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text).map(Self)
    }
}
