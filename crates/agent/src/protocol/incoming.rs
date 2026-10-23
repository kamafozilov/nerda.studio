//! What the agent writes on stdout, parsed into typed events. A line of a
//! kind, or a shape, this crate doesn't model comes through whole as
//! `Incoming::Other`, so a newer agent's events never fail a session.

use std::fmt;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{MessageId, RequestId, ToolUseId};

/// One line from the agent.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// A turn began, on this conversation, model and permission mode.
    Init(Init),
    /// The agent's activity or permission mode changed.
    Status(AgentStatus),
    /// A piece of the reply, as it streams.
    Stream(Stream),
    /// Finished blocks of the reply.
    Assistant(Assistant),
    /// The agent took up the app's message: the message, echoed back.
    Replay(MessageId),
    /// Tools' output, as the model gets it.
    ToolResults(Vec<ToolResult>),
    /// A turn ended.
    Result(TurnResult),
    /// The agent asks whether a tool may run.
    Permission(PermissionRequest),
    /// Any other request the agent makes of the app.
    Request {
        id: RequestId,
        subtype: String,
    },
    /// The answer to a control request: a success's payload, or an error's text.
    Response {
        id: RequestId,
        result: Result<Value, String>,
    },
    /// The agent no longer needs an answer to its request.
    Cancel(RequestId),
    RateLimit(RateLimit),
    /// One of the app's messages moved through the agent's queue.
    Lifecycle {
        message: MessageId,
        stage: Stage,
    },
    /// A line this crate doesn't model, such as a hook's progress, a
    /// subagent's own messages, or a kind a newer agent added.
    Other(Value),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Init {
    pub session_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default, rename = "permissionMode")]
    pub permission_mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AgentStatus {
    /// Such as `requesting` while a model call is out; `None` when idle.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "permissionMode")]
    pub permission_mode: Option<String>,
}

/// The reply as it streams: one message at a time, its blocks by index.
#[derive(Clone, Debug, PartialEq)]
pub enum Stream {
    MessageStart { id: String },
    BlockStart { index: u32, kind: BlockKind },
    Delta { index: u32, delta: Delta },
    BlockStop { index: u32 },
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockKind {
    Text,
    Thinking,
    ToolUse { id: ToolUseId, name: String },
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Delta {
    Text(String),
    Thinking(String),
    /// A tool's input or a signature, which the finished block carries whole.
    Other,
}

/// Finished blocks of the reply message `message_id`, in order. A message's
/// blocks may come in several of these.
#[derive(Clone, Debug, PartialEq)]
pub struct Assistant {
    pub message_id: String,
    pub blocks: Vec<Block>,
    /// The turn was interrupted: these blocks are as far as they got.
    pub aborted: bool,
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResult {
    pub tool_use_id: ToolUseId,
    /// The text the tool gave back, its text parts joined by line breaks.
    pub output: String,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurnResult {
    pub session_id: String,
    /// The app's messages this turn answered.
    pub messages: Vec<MessageId>,
    /// What the process has cost so far, this turn included.
    pub total_cost_usd: f64,
    pub duration_ms: u64,
    /// This turn's tokens.
    pub usage: Usage,
    pub outcome: Outcome,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Interrupted,
    /// The agent's own words for what went wrong.
    Failed(String),
}

/// A tool call waiting for the person's yes or no.
#[derive(Clone, Debug, PartialEq)]
pub struct PermissionRequest {
    pub id: RequestId,
    pub tool_name: String,
    pub display_name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Why the agent asks; may hold terminal escape codes.
    pub decision_reason: Option<String>,
    pub input: Value,
    pub tool_use_id: Option<ToolUseId>,
    pub blocked_path: Option<String>,
    /// Approving must take a deliberate choice: the prompt opens on Deny.
    pub default_to_no: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RateLimit {
    /// Such as `allowed`, or a limit reached.
    #[serde(default)]
    pub status: String,
    /// Seconds since the UNIX epoch.
    #[serde(default, rename = "resetsAt")]
    pub resets_at: Option<i64>,
    /// Which window, such as `five_hour`.
    #[serde(default, rename = "rateLimitType")]
    pub kind: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Queued,
    Started,
    Completed,
    Cancelled,
    #[serde(other)]
    Other,
}

/// A line that isn't one of the agent's events: not JSON, without a type, or
/// of a known kind in a shape that doesn't fit it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

/// Reads one line of the agent's stdout.
pub fn parse(line: &str) -> Result<Incoming, ParseError> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| ParseError(format!("not JSON: {error}")))?;
    let Some(kind) = value.get("type").and_then(Value::as_str) else {
        return Err(ParseError("no \"type\" field".to_owned()));
    };
    let parsed = match kind {
        "system" => system(&value),
        "stream_event" => stream(&value),
        "assistant" => assistant(&value),
        "user" => user(&value),
        "result" => turn_result(&value),
        "control_request" => control_request(&value),
        "control_response" => control_response(&value),
        "control_cancel_request" => {
            read::<CancelLine>(&value).map(|line| Some(Incoming::Cancel(line.request_id)))
        }
        "rate_limit_event" => read::<RateLimitLine>(&value)
            .map(|line| Some(Incoming::RateLimit(line.rate_limit_info))),
        "command_lifecycle" => read::<LifecycleLine>(&value).map(|line| {
            Some(Incoming::Lifecycle {
                message: line.command_uuid,
                stage: line.state,
            })
        }),
        _ => Ok(None),
    };
    match parsed {
        Ok(Some(incoming)) => Ok(incoming),
        Ok(None) => Ok(Incoming::Other(value)),
        Err(error) => Err(ParseError(format!(
            "a {kind} line that doesn't fit: {error}"
        ))),
    }
}

type Parsed = Result<Option<Incoming>, serde_json::Error>;

fn read<T: DeserializeOwned>(value: &Value) -> Result<T, serde_json::Error> {
    T::deserialize(value)
}

/// Subagents' messages carry the tool call that runs them; they aren't the
/// session's own reply.
fn on_main_thread(value: &Value) -> bool {
    value.get("parent_tool_use_id").is_none_or(Value::is_null)
}

fn system(value: &Value) -> Parsed {
    Ok(match value.get("subtype").and_then(Value::as_str) {
        Some("init") => Some(Incoming::Init(read(value)?)),
        Some("status") => Some(Incoming::Status(read(value)?)),
        _ => None,
    })
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

fn stream(value: &Value) -> Parsed {
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

fn block_type(block: &Value) -> &str {
    block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct AssistantLine {
    message: AssistantMessage,
    #[serde(default)]
    aborted: bool,
}

#[derive(Deserialize)]
struct AssistantMessage {
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

fn assistant(value: &Value) -> Parsed {
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
    Ok(Some(Incoming::Assistant(Assistant {
        message_id: line.message.id,
        blocks,
        aborted: line.aborted,
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

fn user(value: &Value) -> Parsed {
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

#[derive(Deserialize)]
struct ResultLine {
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    terminal_reason: Option<String>,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    total_cost_usd: f64,
    #[serde(default)]
    duration_ms: u64,
    #[serde(default)]
    usage: Usage,
    #[serde(default)]
    user_message_uuid: Option<MessageId>,
    #[serde(default)]
    user_message_uuids: Vec<MessageId>,
}

fn turn_result(value: &Value) -> Parsed {
    let line: ResultLine = read(value)?;
    let interrupted = line
        .terminal_reason
        .as_deref()
        .is_some_and(|reason| reason.starts_with("aborted"));
    let outcome = if interrupted {
        Outcome::Interrupted
    } else if line.is_error {
        let message = [line.result.unwrap_or_default(), line.errors.join("\n")]
            .into_iter()
            .find(|text| !text.trim().is_empty())
            .unwrap_or(line.subtype);
        Outcome::Failed(message)
    } else {
        Outcome::Completed
    };
    let mut messages = line.user_message_uuids;
    if messages.is_empty() {
        messages.extend(line.user_message_uuid);
    }
    Ok(Some(Incoming::Result(TurnResult {
        session_id: line.session_id,
        messages,
        total_cost_usd: line.total_cost_usd,
        duration_ms: line.duration_ms,
        usage: line.usage,
        outcome,
    })))
}

#[derive(Deserialize)]
struct RequestLine {
    request_id: RequestId,
    request: Value,
}

#[derive(Deserialize)]
struct CanUseTool {
    tool_name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    decision_reason: Option<String>,
    #[serde(default)]
    input: Value,
    #[serde(default)]
    tool_use_id: Option<ToolUseId>,
    #[serde(default)]
    blocked_path: Option<String>,
    #[serde(default)]
    default_to_no: bool,
}

fn control_request(value: &Value) -> Parsed {
    let line: RequestLine = read(value)?;
    let subtype = line.request.get("subtype").and_then(Value::as_str);
    let subtype = subtype.unwrap_or_default().to_owned();
    if subtype != "can_use_tool" {
        return Ok(Some(Incoming::Request {
            id: line.request_id,
            subtype,
        }));
    }
    let ask: CanUseTool = read(&line.request)?;
    Ok(Some(Incoming::Permission(PermissionRequest {
        id: line.request_id,
        tool_name: ask.tool_name,
        display_name: ask.display_name,
        title: ask.title,
        description: ask.description,
        decision_reason: ask.decision_reason,
        input: ask.input,
        tool_use_id: ask.tool_use_id,
        blocked_path: ask.blocked_path,
        default_to_no: ask.default_to_no,
    })))
}

#[derive(Deserialize)]
struct ResponseLine {
    response: ResponseBody,
}

#[derive(Deserialize)]
struct ResponseBody {
    subtype: String,
    request_id: RequestId,
    #[serde(default)]
    response: Value,
    #[serde(default)]
    error: Option<String>,
}

fn control_response(value: &Value) -> Parsed {
    let body = read::<ResponseLine>(value)?.response;
    let result = match body.subtype.as_str() {
        "success" => Ok(body.response),
        "error" => Err(body.error.unwrap_or_default()),
        _ => return Ok(None),
    };
    Ok(Some(Incoming::Response {
        id: body.request_id,
        result,
    }))
}

#[derive(Deserialize)]
struct CancelLine {
    request_id: RequestId,
}

#[derive(Deserialize)]
struct RateLimitLine {
    rate_limit_info: RateLimit,
}

#[derive(Deserialize)]
struct LifecycleLine {
    command_uuid: MessageId,
    state: Stage,
}
