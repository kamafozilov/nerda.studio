mod support;

use agent::protocol::Usage;
use agent::{
    Command, Ending, Finished, Item, Message, PermissionState, Status, Text, ToolCall, ToolStatus,
    ToolUseId, Transcript, TurnState,
};
use serde_json::json;
use support::{Line, Player, says};
use workspace::{Access, Effort, ModelId};

fn text(text: &str) -> Item {
    Item::Text(Text {
        text: text.to_owned(),
        streaming: false,
    })
}

fn thinking() -> Item {
    Item::Thinking(Text {
        text: String::new(),
        streaming: false,
    })
}

fn tool(id: &str, name: &str, input: serde_json::Value, status: ToolStatus) -> Item {
    Item::Tool(ToolCall {
        id: ToolUseId(id.to_owned()),
        name: name.to_owned(),
        input,
        status,
    })
}

fn cost(ending: &TurnState) -> f64 {
    match ending {
        TurnState::Ended(Ending::Finished(finished)) => finished.cost_usd,
        other => panic!("not finished: {other:?}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

fn ready() -> Player {
    let mut player = Player::new();
    assert_eq!(player.transcript.status(), &Status::Starting);
    player.play("initialize.jsonl", says("unused"));
    player
}

fn bash_input() -> serde_json::Value {
    json!({
        "command": "touch probe.txt",
        "description": "Create empty probe.txt file in working directory",
    })
}

#[test]
fn the_session_is_ready_once_the_agent_answers() {
    let player = ready();
    assert_eq!(player.transcript.status(), &Status::Ready);
    assert!(player.transcript.turns().is_empty());
}

#[test]
fn a_streamed_reply_settles_into_the_one_text_it_finished_as() {
    let mut player = ready();
    let rest = player.play_until(
        "turn-ok.jsonl",
        says("reply with the word ok"),
        |line| matches!(line, Line::Agent(line) if line.contains(r#""type":"assistant""#)),
    );
    let turn = &player.transcript.turns()[0];
    assert_eq!(turn.state, TurnState::Running);
    assert_eq!(
        turn.items,
        [Item::Text(Text {
            text: "ok".to_owned(),
            streaming: true
        })]
    );

    let mut host = says("unused");
    rest.into_iter()
        .for_each(|line| player.step(line, &mut host));
    let turns = player.transcript.turns();
    assert_eq!(turns.len(), 1);
    assert_eq!(
        turns[0].message.as_ref().unwrap().text,
        "reply with the word ok"
    );
    assert_eq!(turns[0].items, [text("ok")]);
    assert_eq!(
        turns[0].state,
        TurnState::Ended(Ending::Finished(Finished {
            cost_usd: 0.0035433900000000004,
            usage: Usage {
                input_tokens: 2,
                output_tokens: 4,
                cache_read_input_tokens: 12799,
                cache_creation_input_tokens: 17066,
            },
            duration_ms: 2137,
        }))
    );
    assert_eq!(player.transcript.status(), &Status::Ready);
}

#[test]
fn a_message_shows_queued_until_the_agent_takes_it_up_and_never_twice() {
    let mut player = ready();
    let message = Message::new("Use the Bash tool", Vec::new());
    player.send(Command::Send(message.clone()));
    assert_eq!(player.transcript.turns()[0].state, TurnState::Queued);
    assert_eq!(player.transcript.status(), &Status::Running);

    let mut states = Vec::new();
    let mut host = says("unused");
    for line in support::fixture("turn-permission.jsonl")
        .into_iter()
        .take(6)
    {
        player.step(line, &mut host);
        states.push(player.transcript.turns()[0].state.clone());
    }
    use TurnState::{Queued, Running};
    assert_eq!(
        states,
        [Queued, Queued, Running, Running, Running, Running],
        "after the host line, queued, started, the turn's start, its status, the echo"
    );
    assert_eq!(player.transcript.turns().len(), 1);
    assert_eq!(player.transcript.turns()[0].message, Some(message));
}

#[test]
fn a_tool_waits_for_the_person_s_answer_and_then_runs() {
    let mut player = ready();
    let mut asked = 0;
    player.play(
        "turn-permission.jsonl",
        |kind, transcript: &Transcript| match kind {
            "user" => Command::Send(Message::new("Use the Bash tool", Vec::new())),
            "answer" => {
                asked += 1;
                assert_eq!(transcript.status(), &Status::AwaitingPermission);
                let pending: Vec<_> = transcript.pending_permissions().collect();
                assert_eq!(pending.len(), 1);
                assert_eq!(pending[0].request.tool_name, "Bash");
                pending[0].allow()
            }
            other => panic!("asked for {other}"),
        },
    );
    assert_eq!(asked, 1);
    let turn = &player.transcript.turns()[0];
    let [call, Item::Permission(permission), think, reply] = &turn.items[..] else {
        panic!("items: {:?}", turn.items);
    };
    assert_eq!(
        call,
        &tool(
            "toolu_01FtUazAVz9ksL4K2nMikyVK",
            "Bash",
            bash_input(),
            ToolStatus::Done("(Bash completed with no output)".to_owned())
        )
    );
    assert_eq!(permission.state, PermissionState::Allowed);
    assert_eq!(permission.request.input, bash_input());
    assert_eq!([think, reply], [&thinking(), &text("ok")]);
    assert!(close(cost(&turn.state), 0.00490588));
    assert_eq!(player.transcript.status(), &Status::Ready);
}

fn at_the_prompt() -> Player {
    let mut player = ready();
    player.play_until(
        "turn-permission.jsonl",
        says("Use the Bash tool"),
        |line| matches!(line, Line::Host(kind) if kind == "answer"),
    );
    assert_eq!(player.transcript.status(), &Status::AwaitingPermission);
    player
}

#[test]
fn a_denied_or_withdrawn_prompt_stops_waiting() {
    let mut denied = at_the_prompt();
    let permission = denied
        .transcript
        .pending_permissions()
        .next()
        .unwrap()
        .clone();
    denied.send(permission.deny("Not now"));
    assert_eq!(denied.transcript.pending_permissions().count(), 0);
    let states: Vec<_> = denied.transcript.turns()[0]
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Permission(permission) => Some(permission.state),
            _ => None,
        })
        .collect();
    assert_eq!(states, [PermissionState::Denied]);
    assert_eq!(denied.transcript.status(), &Status::Running);

    let mut withdrawn = at_the_prompt();
    withdrawn.receive(
        r#"{"type":"control_cancel_request","request_id":"a6c22aa7-22c2-43d3-9358-c0477a9b497f"}"#,
    );
    assert_eq!(withdrawn.transcript.pending_permissions().count(), 0);
    assert_eq!(withdrawn.transcript.status(), &Status::Running);
}

#[test]
fn turns_on_one_process_each_show_what_they_cost() {
    let mut player = ready();
    let mut host = |kind: &str, transcript: &Transcript| match kind {
        "user" => Command::Send(Message::new(
            format!("message {}", transcript.turns().len()),
            Vec::new(),
        )),
        "answer" => transcript.pending_permissions().next().unwrap().allow(),
        "set_permission_mode" => Command::SetAccess(Access::AcceptEdits),
        "set_model" => Command::SetModel(ModelId("haiku".to_owned())),
        "apply_flag_settings" => Command::SetEffort(Effort::Low),
        other => panic!("asked for {other}"),
    };
    for fixture in [
        "turn-permission.jsonl",
        "settings.jsonl",
        "turn-image.jsonl",
    ] {
        let lines = support::fixture(fixture);
        lines
            .into_iter()
            .for_each(|line| player.step(line, &mut host));
    }
    let turns = player.transcript.turns();
    assert_eq!(turns.len(), 2);
    assert_eq!(
        turns[1].items,
        [
            thinking(),
            tool(
                "toolu_01EnVVzZ9Hh7LYR9sC5x2z2C",
                "Read",
                json!({"file_path": "/Users/me/project/note.txt"}),
                ToolStatus::Done("1\tThe fruit is banana.\n2\t".to_owned())
            ),
            text("banana red"),
        ]
    );
    assert!(close(cost(&turns[0].state), 0.00490588));
    assert!(close(cost(&turns[1].state), 0.00783103 - 0.00490588));
    assert_eq!(player.transcript.status(), &Status::Ready);
}

#[test]
fn stop_interrupts_the_turn_and_the_agent_takes_the_next_message() {
    let mut player = ready();
    let streamed: String = (1..=16)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let mut host = |kind: &str, transcript: &Transcript| match kind {
        "user" => Command::Send(Message::new("Count from 1 to 300", Vec::new())),
        "interrupt" => {
            assert_eq!(transcript.status(), &Status::Running);
            assert_eq!(
                transcript.turns()[0].items,
                [Item::Text(Text {
                    text: streamed.clone(),
                    streaming: true
                })]
            );
            Command::Interrupt
        }
        other => panic!("asked for {other}"),
    };
    let rest = player.play_until(
        "turn-interrupt.jsonl",
        &mut host,
        |line| matches!(line, Line::Agent(line) if line.contains(r#""type":"control_response""#)),
    );
    assert_eq!(player.transcript.status(), &Status::Stopping);
    rest.into_iter()
        .for_each(|line| player.step(line, &mut host));

    let aborted: String = (1..=104)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let first = &player.transcript.turns()[0];
    assert_eq!(first.state, TurnState::Ended(Ending::Interrupted));
    assert_eq!(first.items, [text(&aborted)]);
    assert_eq!(player.transcript.status(), &Status::Ready);

    player.play("turn-after-interrupt.jsonl", says("What is 17*23?"));
    let second = &player.transcript.turns()[1];
    assert_eq!(second.items, [text("391")]);
    assert!(close(
        cost(&second.state),
        0.008334129999999999 - 0.00783103
    ));
    assert_eq!(player.transcript.status(), &Status::Ready);
}

#[test]
fn a_message_sent_during_a_turn_waits_and_stop_drops_it() {
    let mut player = ready();
    let first = Message::new("What is 17*23?", Vec::new());
    let second = Message::new("And 18*23?", Vec::new());
    player.send(Command::Send(first.clone()));
    player.send(Command::Send(second.clone()));
    player.play("turn-after-interrupt.jsonl", says("unused"));

    let turns = player.transcript.turns();
    assert_eq!(turns[0].message, Some(first));
    assert!(matches!(
        turns[0].state,
        TurnState::Ended(Ending::Finished(_))
    ));
    assert_eq!(turns[1].message.as_ref(), Some(&second));
    assert_eq!(turns[1].state, TurnState::Queued);
    assert_eq!(player.transcript.status(), &Status::Running);

    player.send(Command::Interrupt);
    assert_eq!(player.transcript.status(), &Status::Stopping);
    player.receive(&format!(
        r#"{{"type":"command_lifecycle","command_uuid":"{}","state":"cancelled"}}"#,
        second.id.0
    ));
    let turns = player.transcript.turns();
    assert_eq!(turns[1].state, TurnState::Ended(Ending::Interrupted));
    assert_eq!(player.transcript.status(), &Status::Ready);
}

#[test]
fn without_streaming_the_finished_blocks_show_alone() {
    let mut player = ready();
    player.send(Command::Send(Message::new(
        "reply with the word ok",
        Vec::new(),
    )));
    for line in support::agent_lines("turn-ok.jsonl") {
        if !line.contains(r#""type":"stream_event""#) {
            player.receive(&line);
        }
    }
    assert_eq!(player.transcript.turns()[0].items, [text("ok")]);
}

#[test]
fn a_block_whose_finished_copy_never_comes_settles_when_it_stops() {
    let mut player = ready();
    player.send(Command::Send(Message::new(
        "reply with the word ok",
        Vec::new(),
    )));
    for line in support::agent_lines("turn-ok.jsonl") {
        if line.contains(r#""type":"result""#) {
            break;
        }
        if !line.contains(r#""type":"assistant""#) {
            player.receive(&line);
        }
    }
    let turn = &player.transcript.turns()[0];
    assert_eq!(turn.state, TurnState::Running);
    assert_eq!(turn.items, [text("ok")]);
}

#[test]
fn blocks_streamed_without_their_message_s_start_show_once() {
    let mut player = ready();
    player.send(Command::Send(Message::new(
        "reply with the word ok",
        Vec::new(),
    )));
    for line in support::agent_lines("turn-ok.jsonl") {
        if !line.contains(r#""type":"message_start""#) {
            player.receive(&line);
        }
    }
    assert_eq!(player.transcript.turns()[0].items, [text("ok")]);
}

#[test]
fn a_turn_the_agent_begins_on_its_own_has_no_message() {
    let mut player = ready();
    player.play("turn-ok.jsonl", says("reply with the word ok"));
    for line in support::agent_lines("turn-ok.jsonl") {
        player.receive(&line);
    }
    let turns = player.transcript.turns();
    assert_eq!(turns.len(), 2);
    assert!(turns[0].message.is_some());
    assert_eq!(turns[1].message, None);
    assert_eq!(turns[1].items, [text("ok")]);
    assert!(matches!(
        turns[1].state,
        TurnState::Ended(Ending::Finished(_))
    ));
}

#[test]
fn thinking_streams_its_summary_and_settles_into_the_finished_block() {
    let mut player = ready();
    let rest = player.play_until(
        "turn-thinking.jsonl",
        says("Is 391 a prime number? Think it through carefully first, then answer in one word."),
        |line| matches!(line, Line::Agent(line) if line.contains(r#""type":"assistant""#)),
    );
    let thought = "Checking divisibility by primes up to 19: 391 isn't divisible by 2,3,5,7,11, or 13, but 17×23 = 391, so it factors evenly.\n\n";
    assert_eq!(
        player.transcript.turns()[0].items,
        [Item::Thinking(Text {
            text: thought.to_owned(),
            streaming: true
        })]
    );
    let mut host = says("unused");
    rest.into_iter()
        .for_each(|line| player.step(line, &mut host));
    let turn = &player.transcript.turns()[0];
    let summary = Item::Thinking(Text {
        text: thought.to_owned(),
        streaming: false,
    });
    assert_eq!(turn.items, [summary, text("No")]);
    assert!(matches!(turn.state, TurnState::Ended(Ending::Finished(_))));
}
