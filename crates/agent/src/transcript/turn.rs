//! What a transcript holds: turns, and the items that came back in them.

use serde_json::Value;

use crate::failure::Failure;
use crate::protocol::{Command, Decision, Message, MessageId, PermissionRequest, ToolUseId, Usage};

#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    /// What the person sent; `None` for a turn the agent began on its own,
    /// as when a background task of its finishes.
    pub message: Option<Message>,
    pub state: TurnState,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TurnState {
    /// Sent; the agent hasn't taken it up.
    Queued,
    Running,
    Ended(Ending),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ending {
    Finished(Finished),
    /// Stopped, before or while it ran.
    Interrupted,
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finished {
    /// What this turn cost.
    pub cost_usd: f64,
    pub usage: Usage,
    pub duration_ms: u64,
}

/// What came back, in the order it came.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Text(Text),
    Thinking(Text),
    Tool(ToolCall),
    Permission(Permission),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub text: String,
    /// More of it may come.
    pub streaming: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: ToolUseId,
    pub name: String,
    /// `Null` until the call is written out in full.
    pub input: Value,
    pub status: ToolStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    Running,
    /// With the tool's output.
    Done(String),
    /// With the tool's output; empty when the turn ended before the tool did.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Permission {
    pub request: PermissionRequest,
    pub state: PermissionState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionState {
    Pending,
    Allowed,
    Denied,
    /// Withdrawn by the agent, or left unanswered when the turn ended.
    Cancelled,
}

impl Permission {
    /// The answer that lets the tool run as asked.
    pub fn allow(&self) -> Command {
        self.answer(Decision::Allow {
            input: self.request.input.clone(),
        })
    }

    /// The answer that refuses the tool, telling the agent why.
    pub fn deny(&self, message: impl Into<String>) -> Command {
        self.answer(Decision::Deny {
            message: message.into(),
        })
    }

    fn answer(&self, decision: Decision) -> Command {
        Command::Answer {
            request: self.request.id.clone(),
            decision,
        }
    }
}

impl Turn {
    pub(super) fn is_ended(&self) -> bool {
        matches!(self.state, TurnState::Ended(_))
    }

    pub(super) fn answers(&self, id: &MessageId) -> bool {
        self.message
            .as_ref()
            .is_some_and(|message| message.id == *id)
    }

    /// Whether the agent wrote any text in reply.
    pub(super) fn replied(&self) -> bool {
        let text = |item: &Item| matches!(item, Item::Text(text) if !text.text.trim().is_empty());
        self.items.iter().any(text)
    }
}

impl Text {
    pub(super) fn streaming() -> Self {
        Self {
            text: String::new(),
            streaming: true,
        }
    }

    pub(super) fn done(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            streaming: false,
        }
    }
}
