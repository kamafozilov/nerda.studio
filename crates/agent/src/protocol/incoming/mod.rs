//! What the agent writes on stdout, parsed into typed events. A line of a
//! kind, or a shape, this crate doesn't model comes through whole as
//! `Incoming::Other`, so a newer agent's events never fail a session.

mod control;
mod reply;
mod task;
mod time;
mod turn;

use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{MessageId, RequestId};

pub use control::PermissionRequest;
pub use reply::ApiError;
pub use reply::{Assistant, Block, BlockKind, Delta, Stream, ToolResult};
pub use task::TaskEvent;
pub(crate) use time::epoch_seconds;
pub use turn::{Outcome, RateLimit, TurnError, TurnResult, Usage};

/// One line from the agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    Permission(Box<PermissionRequest>),
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
    /// The agent's tasks changed.
    Task(TaskEvent),
    /// One of the app's messages moved through the agent's queue.
    Lifecycle {
        message: MessageId,
        stage: Stage,
    },
    /// A line this crate doesn't model, such as a hook's progress, a
    /// subagent's own messages, or a kind a newer agent added.
    Other(Value),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Init {
    pub session_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default, rename = "permissionMode")]
    pub permission_mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AgentStatus {
    /// Such as `requesting` while a model call is out; `None` when idle.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "permissionMode")]
    pub permission_mode: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
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
        "stream_event" => reply::stream(&value),
        "assistant" => reply::assistant(&value),
        "user" => reply::user(&value),
        "result" => turn::turn_result(&value),
        "control_request" => control::control_request(&value),
        "control_response" => control::control_response(&value),
        "control_cancel_request" => {
            read::<CancelLine>(&value).map(|line| Some(Incoming::Cancel(line.request_id)))
        }
        "rate_limit_event" => turn::rate_limit(&value),
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
    let subtype = value
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(task) = task::task(subtype, value) {
        return task;
    }
    Ok(match subtype {
        "init" => Some(Incoming::Init(read(value)?)),
        "status" => Some(Incoming::Status(read(value)?)),
        _ => None,
    })
}

/// A line the agent writes for its own telemetry, not for the person:
/// `[ede_diagnostic] …` among a result's errors, `[claude-code:…] …` on
/// stderr.
pub(crate) fn is_diagnostic(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with("[ede_diagnostic]") || text.starts_with("[claude-code:")
}

fn block_type(block: &Value) -> &str {
    block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct CancelLine {
    request_id: RequestId,
}

#[derive(Deserialize)]
struct LifecycleLine {
    command_uuid: MessageId,
    state: Stage,
}
