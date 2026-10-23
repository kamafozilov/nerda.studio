//! Stop: the agent's background tasks, its open prompts and its turn end,
//! and a message the stop drops comes back.

mod support;

use agent::{Command, Ending, Event, Message, PermissionState, Session, Status, TurnState};
use serde_json::json;
use support::{Watch, exited, fake, project, status_is, written};

const INIT: &str = r#"{"type":"system","subtype":"init","session_id":"s","model":"claude-haiku-5-5","permissionMode":"default"}"#;

#[test]
fn stop_ends_background_tasks_denies_prompts_then_interrupts_and_gives_back_what_waited() {
    let dir = project(&[
        "initialize.jsonl",
        r#"{"host":"user"}"#,
        r#"{"type":"command_lifecycle","command_uuid":"{{uuid}}","state":"queued"}"#,
        r#"{"type":"command_lifecycle","command_uuid":"{{uuid}}","state":"started"}"#,
        INIT,
        r#"{"type":"system","subtype":"task_started","task_id":"server","tool_use_id":"toolu_1","description":"npm run dev","task_type":"local_bash","is_backgrounded":true}"#,
        r#"{"type":"system","subtype":"task_started","task_id":"build","tool_use_id":"toolu_2","description":"cargo build","task_type":"local_bash","is_backgrounded":false}"#,
        r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"server","task_type":"local_bash","description":"npm run dev"},{"task_id":"watcher","task_type":"monitor","description":"watch logs"}]}"#,
        r#"{"type":"system","subtype":"task_notification","task_id":"watcher","status":"completed","output_file":"","summary":"done"}"#,
        r#"{"type":"control_request","request_id":"prompt-1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"rm -rf build"},"tool_use_id":"toolu_3"}}"#,
        r#"{"host":"user"}"#,
        r#"{"type":"command_lifecycle","command_uuid":"{{uuid}}","state":"queued"}"#,
        r#"{"host":"interrupt"}"#,
        r#"{"type":"control_response","response":{"subtype":"success","request_id":"{{request}}","response":{"still_queued":[]}}}"#,
        r#"{"type":"command_lifecycle","command_uuid":"{{uuid}}","state":"cancelled"}"#,
        r#"{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_tools","session_id":"s","total_cost_usd":0.01,"duration_ms":5,"usage":{}}"#,
    ]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    session.send(Command::Send(Message::new(
        "Start the dev server",
        Vec::new(),
    )));
    watch.until("the prompt", status_is(Status::AwaitingPermission));
    let waiting = Message::new("Then run the tests", Vec::new());
    session.send(Command::Send(waiting.clone()));
    watch.until("the second message waiting", |transcript, _| {
        transcript.turns().len() == 2
    });

    session.send(Command::Interrupt);
    watch.until("stopped", |transcript, seen| {
        transcript.status() == &Status::Ready
            && seen.iter().any(|event| matches!(event, Event::Dropped(_)))
    });

    let requests: Vec<_> = written(&dir)
        .into_iter()
        .filter(|line| {
            line["type"] == "control_request" && line["request_id"] != "initialize"
                || line["type"] == "control_response"
        })
        .map(|line| match line["type"].as_str() {
            Some("control_request") => line["request"].clone(),
            _ => line["response"].clone(),
        })
        .collect();
    assert_eq!(
        requests,
        [
            json!({"subtype": "stop_task", "task_id": "server"}),
            json!({"subtype": "success", "request_id": "prompt-1", "response": {"behavior": "deny", "message": "The user stopped the turn."}}),
            json!({"subtype": "interrupt", "cancel_queued": true}),
        ],
        "only the task still in the background, the prompt, then the turn"
    );

    let dropped: Vec<_> = watch
        .seen
        .iter()
        .filter_map(|event| match event {
            Event::Dropped(message) => Some(message),
            _ => None,
        })
        .collect();
    let [dropped] = &dropped[..] else {
        panic!("{dropped:?}");
    };
    assert_eq!(
        (dropped.text.as_str(), &dropped.attachments),
        (waiting.text.as_str(), &waiting.attachments)
    );
    let turns = watch.transcript.turns();
    assert_eq!(
        turns[1].message.as_ref().map(|message| &message.id),
        Some(&dropped.id)
    );
    assert!(
        turns
            .iter()
            .all(|turn| turn.state == TurnState::Ended(Ending::Interrupted))
    );
    let prompt = turns[0].items.iter().find_map(|item| match item {
        agent::Item::Permission(permission) => Some(permission.state),
        _ => None,
    });
    assert_eq!(prompt, Some(PermissionState::Denied));
    drop(session);
    watch.until("the exit", exited);
}
