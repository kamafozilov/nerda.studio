//! Why a turn failed, as one typed answer with plain words for each.
//!
//! The agent tells it in several places: the error code of an assistant
//! message it writes itself, a rate limit that rejected the turn, the
//! result's text and HTTP status, or its stderr when it quits.

use std::collections::VecDeque;

use crate::protocol::{ApiError, RateLimit, TurnError, is_diagnostic};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The plan's usage limit is reached, until `resets_at`, in seconds since
    /// the UNIX epoch, when the agent said.
    UsageLimit { resets_at: Option<u64> },
    /// The agent isn't signed in, or its sign-in or API key was refused;
    /// `message` is the agent's own words.
    SignedOut { message: String },
    /// The model service is overloaded.
    Overloaded,
    /// The model service failed the request.
    Provider {
        status: Option<u16>,
        message: String,
    },
    /// The agent quit before the turn ended; `None` when a signal ended it.
    /// `stderr` is the last of what it wrote there, credentials removed.
    Crashed { code: Option<i32>, stderr: String },
    /// The message was sent after the agent had quit.
    NotSent,
    /// The agent took the message up but never ran a turn for it.
    NeverStarted,
    /// Anything else, in the agent's words.
    Other(String),
}

impl Failure {
    /// What went wrong, as a sentence to show.
    pub fn message(&self) -> String {
        match self {
            Failure::UsageLimit { .. } => "You've reached your usage limit.".to_owned(),
            Failure::SignedOut { .. } => "The agent isn't signed in.".to_owned(),
            Failure::Overloaded => "The model is overloaded right now.".to_owned(),
            Failure::Provider {
                status: Some(status),
                ..
            } => {
                format!("The model's service failed the request (HTTP {status}).")
            }
            Failure::Provider { status: None, .. } => {
                "The model's service failed the request.".to_owned()
            }
            Failure::Crashed {
                code: Some(code), ..
            } => {
                format!("The agent quit before it finished (exit code {code}).")
            }
            Failure::Crashed { code: None, .. } => {
                "The agent was stopped before it finished.".to_owned()
            }
            Failure::NotSent => "The agent wasn't running, so the message wasn't sent.".to_owned(),
            Failure::NeverStarted => {
                "The agent took the message but never started on it.".to_owned()
            }
            Failure::Other(message) => message.clone(),
        }
    }

    /// What the person can do about it, when there is something.
    pub fn suggestion(&self) -> Option<&'static str> {
        Some(match self {
            Failure::UsageLimit { .. } => "Try again once it resets.",
            Failure::SignedOut { .. } => "Run `claude auth login` in a terminal, then try again.",
            Failure::Overloaded | Failure::Provider { .. } => "Try again in a moment.",
            Failure::NotSent | Failure::NeverStarted => "Send the message again.",
            Failure::Crashed { .. } | Failure::Other(_) => return None,
        })
    }
}

/// What a turn ran into before its result, from the agent's other lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Signals {
    /// The last failed model request, and the words the agent wrote for it.
    pub api_error: Option<(ApiError, String)>,
    /// The last rate limit that rejected the session's turns.
    pub limit: Option<RateLimit>,
}

impl Signals {
    pub fn is_empty(&self) -> bool {
        self.api_error.is_none() && self.limit.is_none()
    }
}

/// The failure a turn ended in, from its result's error, when the result
/// said there was one, and what the turn ran into before.
pub(crate) fn classify(signals: &Signals, error: Option<&TurnError>) -> Failure {
    let (code, said) = match &signals.api_error {
        Some((api_error, said)) => (Some(api_error.code.as_str()), said.as_str()),
        None => (None, ""),
    };
    let message = error
        .map(|error| error.message.as_str())
        .filter(|text| !text.trim().is_empty());
    let message = message.unwrap_or(said).trim().to_owned();
    let status = error.and_then(|error| error.api_status);
    let status = status.or(signals
        .api_error
        .as_ref()
        .and_then(|(error, _)| error.status));
    let lower = message.to_lowercase();
    let limited = || Failure::UsageLimit {
        resets_at: signals
            .limit
            .as_ref()
            .and_then(|limit| limit.resets_at)
            .or_else(|| reset_in(&message)),
    };
    match (code, status) {
        (Some("authentication_failed"), _) | (_, Some(401)) => {
            return Failure::SignedOut { message };
        }
        (Some("rate_limit"), _) => return limited(),
        (Some("overloaded"), _) | (_, Some(529)) => return Failure::Overloaded,
        _ => {}
    }
    if SIGNED_OUT.iter().any(|words| lower.contains(words)) {
        return Failure::SignedOut { message };
    }
    if signals.limit.is_some() || LIMIT_REACHED.iter().any(|words| lower.contains(words)) {
        return limited();
    }
    if lower.contains("overloaded") {
        return Failure::Overloaded;
    }
    match (code, status) {
        (Some("server_error"), status) => Failure::Provider { status, message },
        (_, Some(status)) if status >= 500 => Failure::Provider {
            status: Some(status),
            message,
        },
        _ if message.is_empty() => Failure::Other(fallback(code, error).to_owned()),
        _ => Failure::Other(message),
    }
}

const SIGNED_OUT: &[&str] = &[
    "not logged in",
    "not signed in",
    "please run /login",
    "invalid api key",
    "authentication_error",
    "authentication failed",
    "oauth token has expired",
    "token has expired",
    "sign in again",
    "log in again",
];

const LIMIT_REACHED: &[&str] = &[
    "usage limit",
    "limit reached",
    "hit your limit",
    "you've hit your",
    "you have hit your",
    "rate limit",
    "quota",
];

/// The reset time an older agent puts after the limit's words, as in
/// `Claude AI usage limit reached|1760000000`.
fn reset_in(message: &str) -> Option<u64> {
    let (_, time) = message.rsplit_once('|')?;
    let time = serde_json::Value::String(time.trim().to_owned());
    crate::protocol::epoch_seconds(&time)
}

/// Words for a failure the agent gave none for, so a failed turn never ends
/// blank.
fn fallback(code: Option<&str>, error: Option<&TurnError>) -> &'static str {
    let reason = error.and_then(|error| error.terminal_reason.as_deref());
    let subtype = error.map(|error| error.subtype.as_str());
    match (code, reason, subtype) {
        (_, Some("prompt_too_long" | "blocking_limit"), _) => {
            "The conversation is too long for the model. Run /compact, then try again."
        }
        (_, Some("image_error"), _) => "The model couldn't read an attached image.",
        (_, Some("budget_exhausted"), _) | (_, _, Some("error_max_budget_usd")) => {
            "The turn reached its cost limit."
        }
        (_, _, Some("error_max_turns")) => "The turn reached its limit of steps.",
        (Some("billing_error"), ..) => "The account has a billing problem.",
        (Some("model_not_found"), ..) => "The chosen model isn't available.",
        (Some("max_output_tokens"), ..) => "The reply reached the longest the model can write.",
        (Some("invalid_request"), ..) => "The model refused the request as invalid.",
        _ => "The turn ended with an error.",
    }
}

/// The last of what the agent wrote on stderr: about 8 KB of whole lines, its
/// internal diagnostics left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StderrTail {
    lines: VecDeque<String>,
    bytes: usize,
}

impl StderrTail {
    const KEEP: usize = 8 * 1024;

    pub fn push(&mut self, line: &str) {
        let line = line.trim_end();
        if line.trim().is_empty() || is_diagnostic(line) {
            return;
        }
        let mut line = line.to_owned();
        if line.len() > Self::KEEP {
            let mut start = line.len() - Self::KEEP;
            while !line.is_char_boundary(start) {
                start += 1;
            }
            line.drain(..start);
        }
        self.bytes += line.len() + 1;
        self.lines.push_back(line);
        while self.bytes > Self::KEEP {
            let Some(oldest) = self.lines.pop_front() else {
                break;
            };
            self.bytes -= oldest.len() + 1;
        }
    }

    /// The tail as one text to show, credentials removed.
    pub fn text(&self) -> String {
        let joined: Vec<&str> = self.lines.iter().map(String::as_str).collect();
        redact(&joined.join("\n"))
    }
}

/// `text` with what looks like a credential replaced by `[REDACTED]`: what
/// follows `Bearer ` or `api_key=`, and tokens that start `sk-` or `ghp_`.
pub(crate) fn redact(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let markers = ["bearer ", "basic ", "sk-", "ghp_", "xox", "api_key="];
    let mut redacted = String::new();
    let mut offset = 0;
    while let Some((start, marker)) = markers
        .iter()
        .filter_map(|marker| {
            lower[offset..]
                .find(marker)
                .map(|at| (offset + at, *marker))
        })
        .min_by_key(|(at, _)| *at)
    {
        let credential = if marker.ends_with([' ', '=']) {
            start + marker.len()
        } else {
            start
        };
        let quoted = text[credential..]
            .trim_start_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
        let credential = text.len() - quoted.len();
        let end = text[credential..]
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | '&' | '<' | '>')
            })
            .map_or(text.len(), |at| credential + at);
        redacted.push_str(&text[offset..credential]);
        redacted.push_str("[REDACTED]");
        offset = end.max(start + marker.len());
    }
    redacted.push_str(&text[offset..]);
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_removed_and_the_rest_kept() {
        let raw = "request failed: Bearer secret-one Basic secret-two sk-private ghp-private ghp_private xoxp-private api_key=private&code=401 café";
        assert_eq!(
            redact(raw),
            "request failed: Bearer [REDACTED] Basic [REDACTED] [REDACTED] ghp-private [REDACTED] [REDACTED] api_key=[REDACTED]&code=401 café"
        );
        assert_eq!(
            redact("Authorization: bEaReR token"),
            "Authorization: bEaReR [REDACTED]"
        );
        assert_eq!(
            redact("Bearer   hidden api_key=\"secret\""),
            "Bearer   [REDACTED] api_key=\"[REDACTED]\""
        );
    }

    #[test]
    fn the_stderr_tail_keeps_the_last_lines_without_diagnostics() {
        let mut tail = StderrTail::default();
        for n in 0..2000 {
            tail.push(&format!("line {n}"));
        }
        tail.push("[claude-code:unrecognized_model] {\"model\":\"x\"}");
        tail.push("Error: token sk-ant-secret refused");
        let text = tail.text();
        assert!(text.len() <= StderrTail::KEEP, "{}", text.len());
        assert!(
            text.ends_with("line 1999\nError: token [REDACTED] refused"),
            "{text}"
        );
        assert!(!text.contains("line 0\n") && !text.contains("claude-code:"));
        let mut long = StderrTail::default();
        long.push(&"é".repeat(10_000));
        assert!(long.text().len() <= StderrTail::KEEP);
    }
}
