mod support;

use agent::protocol::parse;
use agent::{
    Command, Ending, Event, Failure, Incoming, Item, Message, PermissionState, Status, ToolStatus,
    Transcript, TurnState,
};
use support::{Line, Player, says};

fn ready() -> Player {
    let mut player = Player::new();
    player.play("initialize.jsonl", says("unused"));
    player
}

#[test]
fn the_agent_quitting_mid_turn_ends_what_it_had_open() {
    let mut player = ready();
    player.play_until(
        "turn-permission.jsonl",
        says("Use the Bash tool"),
        |line| matches!(line, Line::Host(kind) if kind == "answer"),
    );
    player.transcript.apply(&Event::Exited { code: Some(1) });

    let turn = &player.transcript.turns()[0];
    assert_eq!(
        turn.state,
        TurnState::Ended(Ending::Failed(Failure::Crashed {
            code: Some(1),
            stderr: String::new()
        }))
    );
    let [Item::Tool(call), Item::Permission(permission)] = &turn.items[..] else {
        panic!("items: {:?}", turn.items);
    };
    assert_eq!(call.status, ToolStatus::Failed(String::new()));
    assert_eq!(permission.state, PermissionState::Cancelled);
    assert_eq!(
        player.transcript.status(),
        &Status::Exited { code: Some(1) }
    );
}

#[test]
fn a_message_sent_after_the_agent_quit_shows_it_was_not_sent() {
    let mut player = ready();
    player.transcript.apply(&Event::Exited { code: None });
    player.send(Command::Send(Message::new("still there?", Vec::new())));
    assert_eq!(
        player.transcript.turns()[0].state,
        TurnState::Ended(Ending::Failed(Failure::NotSent))
    );
    assert_eq!(player.transcript.status(), &Status::Exited { code: None });
}

#[test]
fn a_new_process_picks_the_session_up_with_its_turns() {
    let mut player = ready();
    player.play("turn-ok.jsonl", says("reply with the word ok"));
    player.transcript.apply(&Event::Exited { code: Some(0) });
    player.play("initialize.jsonl", says("unused"));
    assert_eq!(player.transcript.status(), &Status::Ready);
    player.play("turn-ok.jsonl", says("again"));
    let turns = player.transcript.turns();
    assert_eq!(turns.len(), 2);
    let costs: Vec<_> = turns
        .iter()
        .map(|turn| match &turn.state {
            TurnState::Ended(Ending::Finished(finished)) => finished.cost_usd,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        costs,
        [0.0035433900000000004, 0.0035433900000000004],
        "each process counts from zero"
    );
}

#[test]
fn a_session_that_could_not_start_has_failed_and_stays_failed() {
    let mut transcript = Transcript::new();
    transcript.apply(&Event::Sent(Command::Send(Message::new("hi", Vec::new()))));
    let message = "Couldn't find claude.".to_owned();
    transcript.apply(&Event::Failed(message.clone()));
    assert_eq!(
        transcript.status(),
        &Status::Failed {
            message: message.clone()
        }
    );
    assert_eq!(
        transcript.turns()[0].state,
        TurnState::Ended(Ending::Failed(Failure::Other(message.clone())))
    );
    transcript.apply(&Event::Exited { code: Some(1) });
    assert_eq!(transcript.status(), &Status::Failed { message });
}

#[test]
fn an_agent_that_refuses_to_initialize_fails_the_session() {
    let mut transcript = Transcript::new();
    transcript.apply(&Event::Started);
    let refused = r#"{"type":"control_response","response":{"subtype":"error","request_id":"initialize","error":"Not logged in"}}"#;
    transcript.apply(&Event::Received(parse(refused).unwrap()));
    assert_eq!(
        transcript.status(),
        &Status::Failed {
            message: "Not logged in".to_owned()
        }
    );
}

#[test]
fn only_the_answer_to_initialize_ends_starting() {
    let mut transcript = Transcript::new();
    transcript.apply(&Event::Started);
    let other =
        r#"{"type":"control_response","response":{"subtype":"success","request_id":"host_1"}}"#;
    transcript.apply(&Event::Received(parse(other).unwrap()));
    transcript.apply(&Event::Sent(Command::Send(Message::new(
        "early",
        Vec::new(),
    ))));
    assert_eq!(transcript.status(), &Status::Starting);
    let answered = r#"{"type":"control_response","response":{"subtype":"success","request_id":"initialize","response":{}}}"#;
    transcript.apply(&Event::Received(parse(answered).unwrap()));
    assert_eq!(
        transcript.status(),
        &Status::Running,
        "the early message waits"
    );
}

#[test]
fn stop_while_idle_and_repeated_stops_change_nothing() {
    let mut player = ready();
    player.send(Command::Interrupt);
    assert_eq!(player.transcript.status(), &Status::Ready);

    player.send(Command::Send(Message::new("count", Vec::new())));
    player.send(Command::Interrupt);
    player.send(Command::Interrupt);
    assert_eq!(player.transcript.status(), &Status::Stopping);
    player.send(Command::Send(Message::new("then this", Vec::new())));
    assert_eq!(
        player.transcript.status(),
        &Status::Running,
        "a message after Stop runs"
    );
}

#[test]
fn lines_that_tell_nothing_about_turns_change_nothing() {
    let mut player = ready();
    player.play("turn-ok.jsonl", says("reply with the word ok"));
    let before = player.transcript.clone();
    for event in [
        Event::Unreadable {
            line: "not json".to_owned(),
            error: parse("not json").unwrap_err(),
        },
        Event::Stderr("warning: something".to_owned()),
        Event::Received(Incoming::Other(serde_json::json!({"type": "keep_alive"}))),
    ] {
        player.transcript.apply(&event);
    }
    for line in [
        r#"{"type":"system","subtype":"status","status":"compacting"}"#,
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
        r#"{"type":"command_lifecycle","command_uuid":"unknown","state":"started"}"#,
        r#"{"type":"control_cancel_request","request_id":"unknown"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"unknown","content":"x"}]}}"#,
    ] {
        player.receive(line);
    }
    assert_eq!(player.transcript.turns(), before.turns());
    assert_eq!(player.transcript.status(), before.status());
}
