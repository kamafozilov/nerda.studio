//! A group of tool calls in one line: what it did by kind, the kinds that
//! change things first, "Edited motion.rs, read 3 files and ran 2
//! commands", three kinds at most and the rest counted.

use std::{collections::BTreeMap, path::Path};

use agent::ToolCall;
use serde_json::Value;

use super::tools::{self, Kind, Target, count};

/// Kinds a group's line names; the calls of the rest are counted.
const NAMED_KINDS: usize = 3;

/// The line over the group of `calls`, with `thoughts` thinking blocks.
pub fn summary(calls: &[&ToolCall], thoughts: usize, folder: Option<&Path>) -> String {
    let mut kinds: BTreeMap<Kind, Vec<&ToolCall>> = BTreeMap::new();
    for call in calls {
        kinds
            .entry(tools::describe(call, folder).kind)
            .or_default()
            .push(call);
    }
    let mut parts: Vec<String> = (kinds.iter().take(NAMED_KINDS))
        .map(|(kind, calls)| part(*kind, calls, folder))
        .collect();
    let rest: usize = kinds.values().skip(NAMED_KINDS).map(Vec::len).sum();
    if rest > 0 {
        parts.push(format!("{rest} more"));
    }
    if parts.is_empty() && thoughts > 0 {
        parts.push("thought".to_owned());
    }
    capitalized(&sentence(&parts))
}

/// How many of `calls` failed.
pub fn failures(calls: &[&ToolCall]) -> usize {
    calls.iter().filter(|call| tools::failed(call)).count()
}

fn part(kind: Kind, calls: &[&ToolCall], folder: Option<&Path>) -> String {
    let files = || {
        let mut paths: Vec<&str> = calls
            .iter()
            .filter_map(|call| call.input.get("file_path").and_then(Value::as_str))
            .collect();
        paths.sort_unstable();
        paths.dedup();
        paths
    };
    let one_file = |verb: &str| {
        let paths = files();
        match paths.as_slice() {
            [_] => match tools::describe(calls[0], folder).target {
                Some(Target::File { name, .. }) => format!("{verb} {name}"),
                _ => format!("{verb} 1 file"),
            },
            _ => format!("{verb} {}", count(paths.len().max(1), "file")),
        }
    };
    match kind {
        Kind::Edit => one_file("edited"),
        Kind::Read => one_file("read"),
        Kind::Run => format!("ran {}", count(calls.len(), "command")),
        Kind::Search => format!("searched {}", times(calls.len())),
        Kind::Web => format!("looked on the web {}", times(calls.len())),
        Kind::Agent => format!("ran {}", count(calls.len(), "agent")),
        Kind::Plan => "updated the plan".to_owned(),
        Kind::Other => format!("used {}", count(calls.len(), "tool")),
    }
}

fn times(count: usize) -> String {
    match count {
        1 => "once".to_owned(),
        2 => "twice".to_owned(),
        _ => format!("{count} times"),
    }
}

/// "a", "a and b", "a, b and c".
fn sentence(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use agent::ToolStatus;
    use serde_json::json;

    use super::*;

    fn call(name: &str, input: Value, status: ToolStatus) -> ToolCall {
        ToolCall {
            id: serde_json::from_value(json!("t")).unwrap(),
            name: name.to_owned(),
            input,
            status,
        }
    }

    #[test]
    fn kinds_that_change_things_come_first() {
        let done = || ToolStatus::Done(String::new());
        let calls = [
            call("Read", json!({"file_path": "/p/a.rs"}), done()),
            call("Read", json!({"file_path": "/p/b.rs"}), done()),
            call("Bash", json!({"command": "ls"}), done()),
            call("Edit", json!({"file_path": "/p/src/motion.rs"}), done()),
            call(
                "Grep",
                json!({"pattern": "x"}),
                ToolStatus::Failed(String::new()),
            ),
        ];
        let calls: Vec<&ToolCall> = calls.iter().collect();
        assert_eq!(failures(&calls), 1);
        assert_eq!(
            summary(&calls, 0, None),
            "Edited motion.rs, ran 1 command, read 2 files and 1 more"
        );
    }

    #[test]
    fn thoughts_alone_read_as_thought() {
        assert_eq!(summary(&[], 2, None), "Thought");
    }
}
