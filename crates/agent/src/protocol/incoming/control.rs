//! The agent's requests of the app, and the answers to the app's.

use serde::Deserialize;
use serde_json::Value;

use super::{Incoming, Parsed, read};
use crate::protocol::{Ask, RequestId, ToolUseId};

/// A tool call waiting for the person: to run, or, for the agent's questions
/// and plans, to be answered.
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
    /// What it asks, by the tool asking.
    pub ask: Ask,
    /// The permissions the agent suggests keeping, as it writes them.
    pub suggestions: Vec<Value>,
    /// The agent lets the person allow it for the rest of the session.
    pub session_rule: bool,
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
    #[serde(default)]
    permission_suggestions: Vec<Value>,
    #[serde(default)]
    suppress_always_allow_rule: bool,
}

pub(super) fn control_request(value: &Value) -> Parsed {
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
    let asks = Ask::of(&ask.tool_name, &ask.input);
    Ok(Some(Incoming::Permission(Box::new(PermissionRequest {
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
        ask: asks,
        suggestions: ask.permission_suggestions,
        session_rule: !ask.suppress_always_allow_rule,
    }))))
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

pub(super) fn control_response(value: &Value) -> Parsed {
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
