//! How a turn ended, and the plan's rate limits.

use serde::Deserialize;
use serde_json::Value;

use super::{Incoming, Parsed, read};
use crate::protocol::MessageId;

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

pub(super) fn turn_result(value: &Value) -> Parsed {
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
struct RateLimitLine {
    rate_limit_info: RateLimit,
}

pub(super) fn rate_limit(value: &Value) -> Parsed {
    read::<RateLimitLine>(value).map(|line| Some(Incoming::RateLimit(line.rate_limit_info)))
}
