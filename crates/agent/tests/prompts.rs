//! The person's answers to the agent's prompts: tools, questions and plans.

mod support;

use agent::protocol::encode;
use agent::{
    Answer, Ask, Choice, Command, Decision, Item, Permission, PermissionState, Question, RequestId,
    Transcript, TurnState,
};
use serde_json::{Value, json};
use support::{Line, Player, says};
use workspace::AgentId;

fn ready() -> Player {
    let mut player = Player::new();
    player.play("initialize.jsonl", says("unused"));
    player
}

/// The `nth` prompt the agent waits on in `fixture`, the ones before it
/// allowed.
fn prompt(fixture: &str, nth: usize) -> Permission {
    let mut player = ready();
    let mut host = |kind: &str, transcript: &Transcript| match kind {
        "user" => Command::Send(agent::Message::new("go", Vec::new())),
        "answer" => {
            let pending = transcript.pending_permissions().next().unwrap();
            pending.answer(Answer::AllowOnce)
        }
        other => panic!("asked for {other}"),
    };
    let mut answered = 0;
    for line in support::fixture(fixture) {
        if matches!(&line, Line::Host(kind) if kind == "answer") {
            if answered == nth {
                break;
            }
            answered += 1;
        }
        player.step(line, &mut host);
    }
    player
        .transcript
        .pending_permissions()
        .next()
        .unwrap()
        .clone()
}

fn decision(command: Command) -> Decision {
    match command {
        Command::Answer { decision, .. } => decision,
        other => panic!("{other:?}"),
    }
}

/// The answer as the agent reads it.
fn wire(command: &Command) -> Value {
    let claude = workspace::agent(&AgentId("claude".to_owned())).unwrap();
    let line = encode(command, claude, &RequestId("unused".to_owned())).unwrap();
    serde_json::from_str::<Value>(&line).unwrap()["response"]["response"].clone()
}

#[test]
fn allowing_for_the_session_keeps_the_suggested_rules_for_the_session_alone() {
    let bash = prompt("turn-permission.jsonl", 0);
    assert_eq!(bash.request.ask, Ask::Tool);
    let Decision::Allow { permissions, .. } = decision(bash.answer(Answer::AllowForSession)) else {
        panic!("not allowed");
    };
    assert_eq!(
        permissions,
        [
            json!({"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "touch probe.txt"}], "behavior": "allow", "destination": "session"}),
            json!({"type": "addDirectories", "directories": ["/Users/me/project"], "destination": "session"}),
        ],
        "never the project's settings file, never a change of mode"
    );
    let Decision::Allow { permissions, .. } = decision(bash.answer(Answer::AllowOnce)) else {
        panic!("not allowed");
    };
    assert!(permissions.is_empty());
    assert!(
        wire(&bash.answer(Answer::AllowOnce))
            .get("updatedPermissions")
            .is_none()
    );

    let write = prompt("turn-plan.jsonl", 2);
    assert_eq!(write.request.tool_name, "Write");
    let Decision::Allow { permissions, .. } = decision(write.answer(Answer::AllowForSession))
    else {
        panic!("not allowed");
    };
    assert_eq!(
        permissions,
        [
            json!({"type": "addRules", "rules": [{"toolName": "Write"}], "behavior": "allow", "destination": "session"})
        ],
        "with only a change of mode suggested, a rule for the tool"
    );

    let mut suppressed = bash.clone();
    suppressed.request.session_rule = false;
    assert_eq!(
        suppressed.answer(Answer::AllowForSession),
        suppressed.answer(Answer::AllowOnce)
    );
}

#[test]
fn denying_tells_the_agent_what_the_person_said_and_cancel_stops_the_turn() {
    let bash = prompt("turn-permission.jsonl", 0);
    assert_eq!(
        wire(&bash.answer(Answer::Deny {
            feedback: Some("use mkdir instead".to_owned())
        })),
        json!({"behavior": "deny", "message": "The user denied this tool call, and said: use mkdir instead"})
    );
    assert_eq!(
        wire(&bash.answer(Answer::Deny {
            feedback: Some("  ".to_owned())
        })),
        json!({"behavior": "deny", "message": "The user denied this tool call."})
    );
    assert_eq!(
        wire(&bash.answer(Answer::Cancel)),
        json!({"behavior": "deny", "message": "The user stopped the turn.", "interrupt": true})
    );
}

#[test]
fn the_agent_s_questions_read_as_questions_and_go_back_answered_by_their_text() {
    let asked = prompt("turn-plan.jsonl", 0);
    let questions = vec![Question {
        question: "Which color do you prefer?".to_owned(),
        header: "Color".to_owned(),
        options: vec![
            Choice {
                label: "Red".to_owned(),
                description: "Choose red".to_owned(),
            },
            Choice {
                label: "Blue".to_owned(),
                description: "Choose blue".to_owned(),
            },
        ],
        multi_select: false,
    }];
    assert_eq!(asked.request.ask, Ask::Questions(questions));
    let answered = asked.answer_questions(&[vec!["Red".to_owned()]]);
    let mut input = asked.request.input.clone();
    input["answers"] = json!({"Which color do you prefer?": "Red"});
    assert_eq!(
        wire(&answered),
        json!({"behavior": "allow", "updatedInput": input}),
        "as the recorded run answered, which the agent took"
    );

    let mut several = asked.clone();
    let Ask::Questions(questions) = &mut several.request.ask else {
        unreachable!()
    };
    questions[0].multi_select = true;
    let answered = several.answer_questions(&[vec!["Red".to_owned(), "Blue".to_owned()]]);
    assert_eq!(
        wire(&answered)["updatedInput"]["answers"],
        json!({"Which color do you prefer?": ["Red", "Blue"]})
    );
}

#[test]
fn a_proposed_plan_reads_as_a_plan_to_approve_or_keep_planning() {
    let plan = prompt("turn-plan.jsonl", 1);
    assert_eq!(
        plan.request.ask,
        Ask::Plan("# Plan\n\n1. Write `red` to `color.txt` in the working directory.\n".to_owned())
    );
    assert_eq!(
        wire(&plan.answer(Answer::AllowOnce)),
        json!({"behavior": "allow", "updatedInput": plan.request.input})
    );
    assert_eq!(
        wire(&plan.answer(Answer::Deny {
            feedback: Some("write blue too".to_owned())
        }))["message"],
        "The user wants to keep planning, not start on this plan yet, and said: write blue too"
    );
}

#[test]
fn a_planning_turn_runs_through_its_questions_and_plan_once_answered() {
    let mut player = ready();
    player.play(
        "turn-plan.jsonl",
        |kind, transcript: &Transcript| match kind {
            "user" => Command::Send(agent::Message::new("Ask, then plan", Vec::new())),
            "answer" => {
                let pending = transcript.pending_permissions().next().unwrap();
                match &pending.request.ask {
                    Ask::Questions(_) => pending.answer_questions(&[vec!["Red".to_owned()]]),
                    Ask::Plan(_) | Ask::Tool => pending.answer(Answer::AllowOnce),
                }
            }
            other => panic!("asked for {other}"),
        },
    );
    let turn = &player.transcript.turns()[0];
    assert!(
        matches!(turn.state, TurnState::Ended(_)),
        "{:?}",
        turn.state
    );
    let prompts: Vec<_> = turn
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Permission(permission) => {
                Some((permission.request.tool_name.as_str(), permission.state))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        prompts,
        [
            ("AskUserQuestion", PermissionState::Allowed),
            ("ExitPlanMode", PermissionState::Allowed),
            ("Write", PermissionState::Allowed),
        ]
    );
}

#[test]
fn input_that_does_not_read_as_questions_or_a_plan_asks_as_a_tool() {
    for (tool, input) in [
        ("AskUserQuestion", json!({"questions": []})),
        (
            "AskUserQuestion",
            json!({"questions": [{"options": ["a"]}]}),
        ),
        ("ExitPlanMode", json!({})),
    ] {
        let line = json!({"type": "control_request", "request_id": "r1", "request": {
            "subtype": "can_use_tool", "tool_name": tool, "input": input}});
        let agent::Incoming::Permission(request) =
            agent::protocol::parse(&line.to_string()).unwrap()
        else {
            panic!("not a prompt");
        };
        assert_eq!(request.ask, Ask::Tool, "{tool} {input}");
    }
    let strings = json!({"type": "control_request", "request_id": "r2", "request": {
        "subtype": "can_use_tool", "tool_name": "AskUserQuestion",
        "input": {"questions": [{"prompt": "Ship it?", "title": "Ship", "options": ["Yes", "No"]}]}}});
    let agent::Incoming::Permission(request) =
        agent::protocol::parse(&strings.to_string()).unwrap()
    else {
        panic!("not a prompt");
    };
    let Ask::Questions(questions) = &request.ask else {
        panic!("{:?}", request.ask);
    };
    let labels: Vec<_> = questions[0]
        .options
        .iter()
        .map(|choice| choice.label.as_str())
        .collect();
    assert_eq!(
        (questions[0].question.as_str(), labels),
        ("Ship it?", vec!["Yes", "No"])
    );
}
