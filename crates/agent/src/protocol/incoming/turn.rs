//! How a turn ended, and the plan's rate limits.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Incoming, Parsed, is_diagnostic, read, time};
use crate::protocol::MessageId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Completed,
    Interrupted,
    Failed(TurnError),
}

/// What the agent said about a turn that failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnError {
    /// Its own words: the result's text, else its errors; the agent's
    /// internal diagnostics left out. Empty when it gave none.
    pub message: String,
    /// Such as `success`, `error_during_execution` or `error_max_turns`.
    pub subtype: String,
    /// Such as `api_error` or `prompt_too_long`.
    pub terminal_reason: Option<String>,
    /// The model service's HTTP status, when a request to it failed.
    pub api_status: Option<u16>,
}

/// Why a turn ended that means it failed, even when the result doesn't say
/// it is an error.
const FAILED: &[&str] = &[
    "api_error",
    "blocking_limit",
    "budget_exhausted",
    "image_error",
    "malformed_tool_use_exhausted",
    "model_error",
    "prompt_too_long",
    "rapid_refill_breaker",
    "turn_setup_failed",
];

/// Where the plan's usage stands, sent as it changes.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RateLimit {
    /// `allowed`, `allowed_warning` or `rejected`.
    #[serde(default)]
    pub status: String,
    /// Seconds since the UNIX epoch.
    #[serde(default, rename = "resetsAt", deserialize_with = "time::deserialize")]
    pub resets_at: Option<u64>,
    /// Which window, such as `five_hour`, or `overage`.
    #[serde(default, rename = "rateLimitType")]
    pub kind: Option<String>,
    /// Whether extra usage may be spent past a rejected window.
    #[serde(default, rename = "overageStatus")]
    pub overage_status: Option<String>,
    #[serde(default, rename = "isUsingOverage")]
    pub using_overage: bool,
    #[serde(default, rename = "overageInUse")]
    pub overage_in_use: bool,
}

impl RateLimit {
    /// The limit stops the session's turns: rejected, with no extra usage to
    /// go on with.
    pub fn blocks(&self) -> bool {
        self.status == "rejected"
            && self.kind.as_deref() != Some("overage")
            && !matches!(
                self.overage_status.as_deref(),
                Some("allowed" | "allowed_warning")
            )
            && !self.using_overage
            && !self.overage_in_use
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
    /// Strings, or from some versions objects.
    #[serde(default)]
    errors: Vec<Value>,
    #[serde(default)]
    terminal_reason: Option<String>,
    #[serde(default)]
    api_error_status: Option<u16>,
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
    let failed = line.is_error
        || line.api_error_status.is_some()
        || line
            .terminal_reason
            .as_deref()
            .is_some_and(|reason| FAILED.contains(&reason));
    let outcome = if interrupted {
        Outcome::Interrupted
    } else if failed {
        let errors: Vec<String> = line
            .errors
            .iter()
            .map(|error| match error {
                Value::String(text) => text.clone(),
                other => other
                    .get("message")
                    .and_then(Value::as_str)
                    .map_or_else(|| other.to_string(), str::to_owned),
            })
            .filter(|text| !text.trim().is_empty() && !is_diagnostic(text))
            .collect();
        let message = [line.result.unwrap_or_default(), errors.join("\n")]
            .into_iter()
            .find(|text| !text.trim().is_empty() && !is_diagnostic(text))
            .unwrap_or_default();
        Outcome::Failed(TurnError {
            message,
            subtype: line.subtype,
            terminal_reason: line.terminal_reason,
            api_status: line.api_error_status,
        })
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

/// A subagent's limit never stops the session's own turns, so only the
/// session's own count.
pub(super) fn rate_limit(value: &Value) -> Parsed {
    if !super::on_main_thread(value) {
        return Ok(None);
    }
    read::<RateLimitLine>(value).map(|line| Some(Incoming::RateLimit(line.rate_limit_info)))
}
