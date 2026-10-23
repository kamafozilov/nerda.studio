//! Which turn each of the agent's lines belongs to, in the cases where the
//! agent's own turns and the app's messages cross.

mod support;

use agent::{Command, Ending, Failure, Item, Message, Text, TurnState};
use support::{Player, says};

fn ready() -> Player {
    let mut player = Player::new();
    player.play("initialize.jsonl", says("unused"));
    player
}

fn lifecycle(message: &Message, state: &str) -> String {
    format!(
        r#"{{"type":"command_lifecycle","command_uuid":"{}","state":"{state}"}}"#,
        message.id.0
    )
}

fn reply(text: &str) -> String {
    format!(
        r#"{{"type":"assistant","message":{{"id":"m-{text}","content":[{{"type":"text","text":"{text}"}}]}},"parent_tool_use_id":null}}"#
    )
}

const INIT: &str = r#"{"type":"system","subtype":"init","session_id":"s","model":"claude-haiku-5-5","permissionMode":"default"}"#;
const NAMING_NONE: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":0,"result":"","session_id":"s","total_cost_usd":0,"duration_ms":0,"usage":{}}"#;

fn naming(message: &Message, text: &str) -> String {
    format!(
        r#"{{"type":"result","subtype":"success","is_error":false,"result":"{text}","session_id":"s","total_cost_usd":0.01,"duration_ms":5,"usage":{{}},"user_message_uuids":["{}"]}}"#,
        message.id.0
    )
}

fn text(text: &str) -> Item {
    Item::Text(Text {
        text: text.to_owned(),
        streaming: false,
    })
}

#[test]
fn results_settling_old_background_tasks_do_not_end_a_waiting_message() {
    let mut player = ready();
    let message = Message::new("Reply PONG", Vec::new());
    player.send(Command::Send(message.clone()));
    for line in [
        r#"{"type":"system","subtype":"task_notification","task_id":"a-1","status":"stopped"}"#
            .to_owned(),
        lifecycle(&message, "queued"),
        INIT.to_owned(),
        NAMING_NONE.to_owned(),
        INIT.to_owned(),
        NAMING_NONE.to_owned(),
    ] {
        player.receive(&line);
    }
    assert_eq!(player.transcript.turns().len(), 1);
    assert_eq!(
        player.transcript.turns()[0].state,
        TurnState::Queued,
        "marked done the moment it was sent"
    );

    for line in [
        lifecycle(&message, "started"),
        INIT.to_owned(),
        reply("PONG"),
        NAMING_NONE.to_owned(),
    ] {
        player.receive(&line);
    }
    let turn = &player.transcript.turns()[0];
    assert!(
        matches!(turn.state, TurnState::Ended(Ending::Finished(_))),
        "{:?}",
        turn.state
    );
    assert_eq!(turn.items, [text("PONG")]);
}

#[test]
fn a_turn_the_agent_begins_while_a_message_waits_keeps_its_output_to_itself() {
    let mut player = ready();
    let message = Message::new("Reply PONG", Vec::new());
    player.send(Command::Send(message.clone()));
    for line in [
        lifecycle(&message, "queued"),
        INIT.to_owned(),
        reply("The background build finished."),
        NAMING_NONE.to_owned(),
        lifecycle(&message, "started"),
        INIT.to_owned(),
        reply("PONG"),
        naming(&message, "PONG"),
    ] {
        player.receive(&line);
    }
    let turns = player.transcript.turns();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].message, None, "the agent's own turn comes first");
    assert_eq!(turns[0].items, [text("The background build finished.")]);
    assert_eq!(turns[1].message.as_ref(), Some(&message));
    assert_eq!(turns[1].items, [text("PONG")]);
    for turn in turns {
        assert!(
            matches!(turn.state, TurnState::Ended(Ending::Finished(_))),
            "{:?}",
            turn.state
        );
    }
}

#[test]
fn a_message_the_agent_takes_but_never_runs_fails_saying_so() {
    let mut player = ready();
    let message = Message::new("again", Vec::new());
    player.send(Command::Send(message.clone()));
    player.receive(&lifecycle(&message, "queued"));
    player.receive(&lifecycle(&message, "completed"));
    let turn = &player.transcript.turns()[0];
    assert_eq!(
        turn.state,
        TurnState::Ended(Ending::Failed(Failure::NeverStarted))
    );
    assert_eq!(
        Failure::NeverStarted.suggestion(),
        Some("Send the message again.")
    );
}
