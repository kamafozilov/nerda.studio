//! The agent's tasks: background shells, subagents and watchers it runs
//! beside or within a turn.

use serde::Deserialize;
use serde_json::Value;

use super::{Incoming, Parsed, read};
use crate::protocol::TaskId;

/// A change in the agent's tasks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskEvent {
    /// It started, `background` when it runs beside the turn rather than
    /// holding it up.
    Started { id: TaskId, background: bool },
    /// It moved to the background.
    Backgrounded(TaskId),
    /// It completed, failed or was stopped.
    Ended(TaskId),
    /// These, and no others, run in the background now.
    Background(Vec<TaskId>),
}

#[derive(Deserialize)]
struct Started {
    task_id: TaskId,
    #[serde(default)]
    is_backgrounded: bool,
}

#[derive(Deserialize)]
struct Updated {
    task_id: TaskId,
    #[serde(default)]
    patch: Value,
}

#[derive(Deserialize)]
struct Ended {
    task_id: TaskId,
}

#[derive(Deserialize)]
struct Background {
    tasks: Vec<Ended>,
}

/// A task line by its `system` subtype; `None` for one that isn't.
pub(super) fn task(subtype: &str, value: &Value) -> Option<Parsed> {
    let event = |event| Ok(Some(Incoming::Task(event)));
    Some(match subtype {
        "task_started" => read::<Started>(value).and_then(|line| {
            event(TaskEvent::Started {
                id: line.task_id,
                background: line.is_backgrounded,
            })
        }),
        "task_updated" => read::<Updated>(value).and_then(|line| {
            match line.patch.get("is_backgrounded").and_then(Value::as_bool) {
                Some(true) => event(TaskEvent::Backgrounded(line.task_id)),
                _ => Ok(None),
            }
        }),
        "task_notification" => {
            read::<Ended>(value).and_then(|line| event(TaskEvent::Ended(line.task_id)))
        }
        "background_tasks_changed" => read::<Background>(value).and_then(|line| {
            event(TaskEvent::Background(
                line.tasks.into_iter().map(|task| task.task_id).collect(),
            ))
        }),
        _ => return None,
    })
}
