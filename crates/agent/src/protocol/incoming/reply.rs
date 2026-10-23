//! The agent's reply: streamed pieces, finished blocks and tools' output.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Incoming, Parsed, block_type, on_main_thread, read};
use crate::protocol::{MessageId, ToolUseId};

/// The reply as it streams: one message at a time, its blocks by index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stream {
    MessageStart { id: String },
    BlockStart { index: u32, kind: BlockKind },
    Delta { index: u32, delta: Delta },
    BlockStop { index: u32 },
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlockKind {
    Text,
    Thinking,
    ToolUse { id: ToolUseId, name: String },
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Delta {
    Text(String),
    Thinking(String),
    /// A tool's input or a signature, which the finished block carries whole.
    Other,
}

/// Finished blocks of the reply message `message_id`, in order. A message's
/// blocks may come in several of these.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assistant {
    pub message_id: String,
    pub blocks: Vec<Block>,
    /// The turn was interrupted: these blocks are as far as they got.
    pub aborted: bool,
    /// A request to the model failed: the message is the agent's own, telling
    /// the person what went wrong, not a reply.
    pub error: Option<ApiError>,
}

/// A failed request to the model, as an assistant message reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    /// Such as `authentication_failed`, `rate_limit`, `overloaded`,
    /// `model_not_found` or `server_error`.
    pub code: String,
    /// The model service's HTTP status, when it answered.
    pub status: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Block {
    Text(String),
    Thinking(String),
    ToolUse {
        id: ToolUseId,
        name: String,
        input: Value,
    },
    Other(Value),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: ToolUseId,
    /// The text the tool gave back, its text parts joined by line breaks.
    pub output: String,
    pub is_error: bool,
}

#[derive(Deserialize)]
struct StreamLine {
    event: StreamWire,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamWire {
    MessageStart {
        message: IdOnly,
    },
    ContentBlockStart {
        index: u32,
        content_block: Value,
    },
    ContentBlockDelta {
        index: u32,
        delta: Value,
    },
    ContentBlockStop {
        index: u32,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct IdOnly {
    id: String,
}

#[derive(Deserialize)]
struct ToolUseStart {
    id: ToolUseId,
    name: String,
}

pub(super) fn stream(value: &Value) -> Parsed {
    if !on_main_thread(value) {
        return Ok(None);
    }
    let line: StreamLine = read(value)?;
    let event = match line.event {
        StreamWire::MessageStart { message } => Stream::MessageStart { id: message.id },
        StreamWire::ContentBlockStart {
            index,
            content_block,
        } => {
            let kind = match block_type(&content_block) {
                "text" => BlockKind::Text,
                "thinking" => BlockKind::Thinking,
                "tool_use" => {
                    let start: ToolUseStart = read(&content_block)?;
                    BlockKind::ToolUse {
                        id: start.id,
                        name: start.name,
                    }
                }
                _ => BlockKind::Other,
            };
            Stream::BlockStart { index, kind }
        }
        StreamWire::ContentBlockDelta { index, delta } => {
            let text = |key| delta.get(key).and_then(Value::as_str).unwrap_or_default();
            let delta = match block_type(&delta) {
                "text_delta" => Delta::Text(text("text").to_owned()),
                "thinking_delta" => Delta::Thinking(text("thinking").to_owned()),
                _ => Delta::Other,
            };
            Stream::Delta { index, delta }
        }
        StreamWire::ContentBlockStop { index } => Stream::BlockStop { index },
        StreamWire::Other => Stream::Other,
    };
    Ok(Some(Incoming::Stream(event)))
}

#[derive(Deserialize)]
struct AssistantLine {
    message: AssistantMessage,
    #[serde(default)]
    aborted: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    api_error_status: Option<u16>,
}

#[derive(Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    id: String,
    #[serde(default)]
    content: Vec<Value>,
}

#[derive(Deserialize)]
struct ToolUseBlock {
    id: ToolUseId,
    name: String,
    #[serde(default)]
    input: Value,
}

pub(super) fn assistant(value: &Value) -> Parsed {
    if !on_main_thread(value) {
        return Ok(None);
    }
    let line: AssistantLine = read(value)?;
    let text = |block: &Value, key| {
        let text = block.get(key).and_then(Value::as_str);
        text.unwrap_or_default().to_owned()
    };
    let blocks = line
        .message
        .content
        .into_iter()
        .map(|block| {
            Ok(match block_type(&block) {
                "text" => Block::Text(text(&block, "text")),
                "thinking" => Block::Thinking(text(&block, "thinking")),
                "tool_use" => {
                    let tool: ToolUseBlock = read(&block)?;
                    Block::ToolUse {
                        id: tool.id,
                        name: tool.name,
                        input: tool.input,
                    }
                }
                _ => Block::Other(block),
            })
        })
        .collect::<Result<_, serde_json::Error>>()?;
    let error = line.error.map(|code| ApiError {
        code,
        status: line.api_error_status,
    });
    Ok(Some(Incoming::Assistant(Assistant {
        message_id: line.message.id,
        blocks,
        aborted: line.aborted,
        error,
    })))
}

#[derive(Deserialize)]
struct UserLine {
    #[serde(default, rename = "isReplay")]
    is_replay: bool,
    #[serde(default)]
    uuid: Option<MessageId>,
    message: UserContent,
}

#[derive(Deserialize)]
struct UserContent {
    #[serde(default)]
    content: Value,
}

#[derive(Deserialize)]
struct ToolResultBlock {
    tool_use_id: ToolUseId,
    #[serde(default)]
    content: Value,
    #[serde(default)]
    is_error: bool,
}

pub(super) fn user(value: &Value) -> Parsed {
    if !on_main_thread(value) {
        return Ok(None);
    }
    let line: UserLine = read(value)?;
    if line.is_replay {
        return Ok(line.uuid.map(Incoming::Replay));
    }
    let Some(blocks) = line.message.content.as_array() else {
        return Ok(None);
    };
    let results = blocks
        .iter()
        .filter(|block| block_type(block) == "tool_result")
        .map(|block| {
            let result: ToolResultBlock = read(block)?;
            Ok(ToolResult {
                tool_use_id: result.tool_use_id,
                output: plain_text(&result.content),
                is_error: result.is_error,
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    Ok((!results.is_empty()).then_some(Incoming::ToolResults(results)))
}

/// A tool result's content: a string, or blocks whose text parts count.
fn plain_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block_type(block) == "text")
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
