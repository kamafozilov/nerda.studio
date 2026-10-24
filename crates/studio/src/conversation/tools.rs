//! What a tool call says on screen: its icon, a verb in the past tense once
//! it is done and in the present while it runs, what it acted on, and what
//! opening it shows. Read from the call's name and input as the agent wrote
//! them; a tool this doesn't know shows by its name.

use std::path::Path;

use agent::{ToolCall, ToolStatus};
use serde_json::Value;

use super::diff::{self, DiffLine};
use crate::assets::Icon;

/// A tool call as its row shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub icon: Icon,
    /// "Read", "Ran": the call is done.
    pub verb: &'static str,
    /// "Reading", "Running": the call runs.
    pub live_verb: &'static str,
    pub target: Option<Target>,
    /// Quieter words after the target, such as the folder a file is in.
    pub detail: Option<String>,
    /// Lines added and removed, for a change to a file.
    pub stats: Option<(usize, usize)>,
    pub kind: Kind,
}

/// What a call acted on.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A file, shown by its name; its whole path goes in the tooltip.
    File {
        name: String,
        path: String,
    },
    /// A command or a pattern, in the monospaced font.
    Code(String),
    Words(String),
}

/// What a group of calls counts it as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Edit,
    Run,
    Read,
    Search,
    Web,
    Agent,
    Plan,
    Other,
}

/// What opening a call shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Nothing,
    /// The command in full over what it printed.
    Command {
        command: String,
        output: String,
    },
    Diff(Vec<DiffLine>),
    Output(String),
    Todos(Vec<Todo>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub text: String,
    pub state: TodoState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoState {
    Pending,
    Active,
    Done,
}

pub fn describe(call: &ToolCall, folder: Option<&Path>) -> Tool {
    let input = &call.input;
    let text = |key: &str| input.get(key).and_then(Value::as_str).map(str::to_owned);
    let file = || {
        text("file_path")
            .or_else(|| text("notebook_path"))
            .map(|path| file(&path, folder))
    };
    let tool = |icon, verb, live_verb, kind| Tool {
        icon,
        verb,
        live_verb,
        target: None,
        detail: None,
        stats: None,
        kind,
    };
    let output = output(&call.status);
    match call.name.as_str() {
        "Bash" => Tool {
            target: text("command").map(|command| Target::Code(first_line(&command))),
            ..tool(Icon::Terminal, "Ran", "Running", Kind::Run)
        },
        "Read" => {
            let lines = match (number(input, "offset"), number(input, "limit")) {
                (Some(offset), Some(limit)) => {
                    Some(format!("lines {offset}\u{2013}{}", offset + limit - 1))
                }
                (Some(offset), None) => Some(format!("from line {offset}")),
                _ => None,
            };
            let (target, folder) = file().unzip();
            Tool {
                target,
                detail: lines.or(folder.flatten()),
                ..tool(Icon::FileView, "Read", "Reading", Kind::Read)
            }
        }
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => {
            let (target, folder) = file().unzip();
            let (icon, verb, live_verb) = match call.name.as_str() {
                "Write" => (Icon::FileAdd, "Wrote", "Writing"),
                _ => (Icon::NewSession, "Edited", "Editing"),
            };
            let lines = diff::of(&call.name, input);
            Tool {
                target,
                detail: folder.flatten(),
                stats: (!lines.is_empty()).then(|| diff::stats(&lines)),
                ..tool(icon, verb, live_verb, Kind::Edit)
            }
        }
        "Grep" => Tool {
            target: text("pattern").map(Target::Code),
            detail: text("path")
                .or_else(|| text("glob"))
                .map(|path| format!("in {}", short(&path, folder))),
            ..tool(Icon::Search, "Searched for", "Searching for", Kind::Search)
        },
        "Glob" => Tool {
            target: text("pattern").map(Target::Code),
            detail: output.map(|found| {
                count(
                    found.lines().filter(|l| !l.trim().is_empty()).count(),
                    "file",
                )
            }),
            ..tool(Icon::FileSearch, "Found", "Finding", Kind::Search)
        },
        "WebFetch" => Tool {
            target: text("url").map(|url| Target::Words(site(&url))),
            ..tool(Icon::Globe, "Fetched", "Fetching", Kind::Web)
        },
        "WebSearch" => Tool {
            target: text("query").map(Target::Words),
            ..tool(
                Icon::Globe,
                "Searched the web for",
                "Searching the web for",
                Kind::Web,
            )
        },
        "Task" | "Agent" => Tool {
            target: text("description").map(Target::Words),
            detail: text("subagent_type"),
            ..tool(
                Icon::Agent,
                "Ran an agent:",
                "Running an agent:",
                Kind::Agent,
            )
        },
        "TodoWrite" => {
            let todos = todos(input);
            let done = todos
                .iter()
                .filter(|todo| todo.state == TodoState::Done)
                .count();
            Tool {
                detail: (!todos.is_empty()).then(|| format!("{done} of {} done", todos.len())),
                ..tool(
                    Icon::Checklist,
                    "Updated the plan",
                    "Updating the plan",
                    Kind::Plan,
                )
            }
        }
        "Skill" => Tool {
            target: text("skill").or_else(|| text("command")).map(Target::Words),
            ..tool(
                Icon::Skill,
                "Used the skill",
                "Using the skill",
                Kind::Other,
            )
        },
        "AskUserQuestion" => Tool {
            target: question(input).map(Target::Words),
            ..tool(Icon::Question, "Asked", "Asking", Kind::Other)
        },
        "ExitPlanMode" => tool(
            Icon::Checklist,
            "Proposed a plan",
            "Proposing a plan",
            Kind::Plan,
        ),
        name => match mcp(name) {
            Some((server, tool_name)) => Tool {
                target: Some(Target::Words(tool_name.replace('_', " "))),
                detail: Some(server.to_owned()),
                ..tool(Icon::Plug, "Called", "Calling", Kind::Other)
            },
            None => Tool {
                target: Some(Target::Words(name.to_owned())),
                ..tool(Icon::Tool, "Used", "Using", Kind::Other)
            },
        },
    }
}

/// What opening `call` shows.
pub fn body(call: &ToolCall) -> Body {
    let input = &call.input;
    let output = output(&call.status)
        .unwrap_or_default()
        .trim_end()
        .to_owned();
    match call.name.as_str() {
        "Bash" => Body::Command {
            command: (input.get("command").and_then(Value::as_str))
                .unwrap_or_default()
                .to_owned(),
            output,
        },
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" if !failed(call) => {
            match diff::of(&call.name, input) {
                lines if lines.is_empty() => Body::Nothing,
                lines => Body::Diff(lines),
            }
        }
        "TodoWrite" => Body::Todos(todos(input)),
        _ if output.is_empty() => Body::Nothing,
        _ => Body::Output(output),
    }
}

pub fn failed(call: &ToolCall) -> bool {
    matches!(call.status, ToolStatus::Failed(_))
}

fn output(status: &ToolStatus) -> Option<&str> {
    match status {
        ToolStatus::Running => None,
        ToolStatus::Done(output) | ToolStatus::Failed(output) => Some(output),
    }
}

/// A file as a target, and the folder it is in, relative to the project.
fn file(path: &str, folder: Option<&Path>) -> (Target, Option<String>) {
    let shown = short(path, folder);
    let (parent, name) = match shown.rsplit_once('/') {
        Some((parent, name)) if !name.is_empty() => (Some(parent.to_owned()), name.to_owned()),
        _ => (None, shown.clone()),
    };
    let path = workspace::home_relative(Path::new(path));
    (
        Target::File { name, path },
        parent.filter(|parent| !parent.is_empty()),
    )
}

/// `path` inside the project as relative to it, anything else from home.
fn short(path: &str, folder: Option<&Path>) -> String {
    let inside = folder.and_then(|folder| Path::new(path).strip_prefix(folder).ok());
    match inside {
        Some(relative) if relative.as_os_str().is_empty() => ".".to_owned(),
        Some(relative) => relative.display().to_string(),
        None => workspace::home_relative(Path::new(path)),
    }
}

fn first_line(text: &str) -> String {
    let mut lines = text.trim().lines();
    let first = lines.next().unwrap_or_default().trim().to_owned();
    match lines.next() {
        Some(_) => format!("{first} \u{2026}"),
        None => first,
    }
}

fn number(input: &Value, key: &str) -> Option<u64> {
    input.get(key).and_then(Value::as_u64)
}

pub fn count(count: usize, noun: &str) -> String {
    match count {
        1 => format!("1 {noun}"),
        _ => format!("{count} {noun}s"),
    }
}

/// A URL by its host and path, without the scheme.
fn site(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.trim_end_matches('/').to_owned()
}

fn mcp(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    let server = server.rsplit(['_', ':']).next().unwrap_or(server);
    Some((server, tool))
}

fn question(input: &Value) -> Option<String> {
    let first = input.get("questions")?.as_array()?.first()?;
    let text = first.get("question").or_else(|| first.get("header"))?;
    text.as_str().map(str::to_owned)
}

fn todos(input: &Value) -> Vec<Todo> {
    let Some(todos) = input.get("todos").and_then(Value::as_array) else {
        return Vec::new();
    };
    let todo = |todo: &Value| {
        let text = todo.get("content").and_then(Value::as_str)?.to_owned();
        let state = match todo.get("status").and_then(Value::as_str) {
            Some("completed") => TodoState::Done,
            Some("in_progress") => TodoState::Active,
            _ => TodoState::Pending,
        };
        Some(Todo { text, state })
    };
    todos.iter().filter_map(todo).collect()
}

#[cfg(test)]
mod tests {
    use agent::ToolCall;
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
    fn a_read_names_its_file_and_folder_inside_the_project() {
        let read = call(
            "Read",
            json!({"file_path": "/p/crates/theme/src/motion.rs"}),
            ToolStatus::Running,
        );
        let tool = describe(&read, Some(Path::new("/p")));
        assert_eq!(tool.live_verb, "Reading");
        let Some(Target::File { name, .. }) = tool.target else {
            panic!()
        };
        assert_eq!(name, "motion.rs");
        assert_eq!(tool.detail.as_deref(), Some("crates/theme/src"));
        let ranged = call(
            "Read",
            json!({"file_path": "/p/a.rs", "offset": 40, "limit": 80}),
            ToolStatus::Running,
        );
        assert_eq!(
            describe(&ranged, None).detail.as_deref(),
            Some("lines 40\u{2013}119")
        );
    }

    #[test]
    fn a_command_shows_its_first_line() {
        let bash = call(
            "Bash",
            json!({"command": "cargo test\ncargo clippy"}),
            ToolStatus::Done(String::new()),
        );
        assert_eq!(
            describe(&bash, None).target,
            Some(Target::Code("cargo test \u{2026}".to_owned()))
        );
    }

    #[test]
    fn an_edit_counts_its_lines() {
        let edit =
            json!({"file_path": "/p/a.rs", "old_string": "a\nb\nc", "new_string": "a\nB\nc\nd"});
        let tool = describe(&call("Edit", edit, ToolStatus::Done(String::new())), None);
        assert_eq!(tool.stats, Some((2, 1)));
    }

    #[test]
    fn an_mcp_tool_names_its_server() {
        let tool = describe(
            &call(
                "mcp__plugin_paper-desktop_paper__get_screenshot",
                json!({}),
                ToolStatus::Running,
            ),
            None,
        );
        assert_eq!(
            tool.target,
            Some(Target::Words("get screenshot".to_owned()))
        );
        assert_eq!(tool.detail.as_deref(), Some("paper"));
    }

    #[test]
    fn a_plan_counts_what_is_done() {
        let todos = json!({"todos": [
            {"content": "A", "status": "completed"},
            {"content": "B", "status": "in_progress"},
            {"content": "C", "status": "pending"}
        ]});
        let todo = call("TodoWrite", todos, ToolStatus::Done(String::new()));
        assert_eq!(describe(&todo, None).detail.as_deref(), Some("1 of 3 done"));
        let Body::Todos(todos) = body(&todo) else {
            panic!()
        };
        assert_eq!(todos[1].state, TodoState::Active);
    }
}
