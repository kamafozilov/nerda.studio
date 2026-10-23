//! What the app writes on the agent's stdin, each command one line.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use workspace::{Access, Agent, Effort, ModelId};

use super::{Attachment, MessageId, RequestId, TaskId};

/// The id of the `initialize` request a session sends first: its answer
/// means the agent is up and reading its input.
pub const INITIALIZE: &str = "initialize";

/// Something the app tells a running agent.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// A message; one sent while a turn runs waits in the agent's queue.
    Send(Message),
    /// Stops the turn that runs and drops the messages queued behind it; the
    /// agent stays up for the next message. A session sends `StopTask` for
    /// each task running in the background and denies each open prompt
    /// first, so nothing of the turn goes on.
    Interrupt,
    /// Stops one of the agent's tasks.
    StopTask(TaskId),
    /// Takes effect from the agent's next model call.
    SetModel(ModelId),
    /// Takes effect from the next turn.
    SetEffort(Effort),
    SetAccess(Access),
    /// Answers the agent's permission prompt `request`.
    Answer {
        request: RequestId,
        decision: Decision,
    },
}

/// An answer to a prompt as the agent takes it; `Permission::answer` makes
/// one from the person's.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// Run the tool on `input`: the input it asked with, or one the person
    /// changed or answered; keep `permissions` for the rest of the session.
    Allow {
        input: Value,
        permissions: Vec<Value>,
    },
    /// Don't run it; `message` tells the agent why; `interrupt` also stops
    /// the turn.
    Deny { message: String, interrupt: bool },
}

/// A message the person sends.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// The id it went to the agent under: a session gives each message a new
    /// one as it writes it, which its `Event::Sent` carries.
    pub id: MessageId,
    pub text: String,
    pub attachments: Vec<Attachment>,
}

impl Message {
    pub fn new(text: impl Into<String>, attachments: Vec<Attachment>) -> Self {
        Self {
            id: MessageId::new(),
            text: text.into(),
            attachments,
        }
    }
}

/// The line that sends `command` to `agent`, under the id `request` when it
/// is a control request of the app's. `None` for an effort or access the
/// agent doesn't offer, which stays as it was, as at launch.
pub fn encode(command: &Command, agent: &Agent, request: &RequestId) -> Option<String> {
    let line = match command {
        Command::Send(message) => json!({
            "type": "user",
            "uuid": message.id.0,
            "parent_tool_use_id": null,
            "message": {"role": "user", "content": content(message)},
        }),
        Command::Interrupt => control(
            &request.0,
            json!({"subtype": "interrupt", "cancel_queued": true}),
        ),
        Command::StopTask(task) => control(
            &request.0,
            json!({"subtype": "stop_task", "task_id": task.0}),
        ),
        Command::SetModel(model) => control(
            &request.0,
            json!({"subtype": "set_model", "model": model.0}),
        ),
        Command::SetEffort(effort) => {
            let level = agent.efforts.iter().find(|level| level.effort == *effort)?;
            control(
                &request.0,
                json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": level.cli}}),
            )
        }
        Command::SetAccess(access) => {
            let mode = agent.access_mode(*access)?;
            control(
                &request.0,
                json!({"subtype": "set_permission_mode", "mode": mode.cli}),
            )
        }
        Command::Answer { request, decision } => {
            let mut answer = match decision {
                Decision::Allow { input, .. } => {
                    json!({"behavior": "allow", "updatedInput": input})
                }
                Decision::Deny { message, .. } => json!({"behavior": "deny", "message": message}),
            };
            match decision {
                Decision::Allow { permissions, .. } if !permissions.is_empty() => {
                    answer["updatedPermissions"] = Value::from(permissions.clone());
                }
                Decision::Deny {
                    interrupt: true, ..
                } => answer["interrupt"] = Value::from(true),
                _ => {}
            }
            json!({
                "type": "control_response",
                "response": {"subtype": "success", "request_id": request.0, "response": answer},
            })
        }
    };
    Some(line.to_string())
}

/// The first line a session sends, which the agent answers once it is up.
pub fn encode_initialize() -> String {
    control(INITIALIZE, json!({"subtype": "initialize"})).to_string()
}

/// Turns down a request of the agent's that this app doesn't handle, so the
/// agent doesn't wait on it.
pub fn encode_decline(request: &RequestId) -> String {
    json!({
        "type": "control_response",
        "response": {
            "subtype": "error",
            "request_id": request.0,
            "error": "This app doesn't handle this request.",
        },
    })
    .to_string()
}

fn control(request_id: &str, request: Value) -> Value {
    json!({"type": "control_request", "request_id": request_id, "request": request})
}

/// The images, then the text with any files' paths after it: the agent
/// expands a `/command` only from the last text block, and only when it
/// leads it. A message of whitespace alone sends no text block, which the
/// model would refuse.
fn content(message: &Message) -> Vec<Value> {
    let mut blocks: Vec<Value> = message
        .attachments
        .iter()
        .filter_map(|attachment| match attachment {
            Attachment::Image(image) => Some(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": image.format().media_type(),
                    "data": STANDARD.encode(image.bytes()),
                },
            })),
            Attachment::File(_) => None,
        })
        .collect();
    let mut text = message.text.clone();
    let files: Vec<String> = message
        .attachments
        .iter()
        .filter_map(|attachment| match attachment {
            Attachment::File(path) => Some(path.display().to_string()),
            Attachment::Image(_) => None,
        })
        .collect();
    if !files.is_empty() {
        if !text.trim().is_empty() {
            text.push_str("\n\n");
        }
        text.push_str("Attached files:\n");
        text.push_str(&files.join("\n"));
    }
    if !text.trim().is_empty() {
        blocks.push(json!({"type": "text", "text": text}));
    }
    blocks
}
