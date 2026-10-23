mod support;

use agent::protocol::{
    AgentStatus, Assistant, Block, BlockKind, Delta, Init, Outcome, PermissionRequest, RateLimit,
    Stage, Stream, ToolResult, TurnError, TurnResult, Usage,
};
use agent::{Incoming, MessageId, RequestId, ToolUseId, protocol::parse};
use serde_json::json;
use support::{FIXTURES, agent_lines};

fn parsed(fixture: &str) -> Vec<Incoming> {
    agent_lines(fixture)
        .iter()
        .map(|line| parse(line).unwrap())
        .collect()
}

fn find<T>(fixture: &str, pick: impl FnMut(Incoming) -> Option<T>) -> Vec<T> {
    parsed(fixture).into_iter().filter_map(pick).collect()
}

fn uuid() -> MessageId {
    MessageId("{{uuid}}".to_owned())
}

#[test]
fn every_recorded_line_parses() {
    for fixture in FIXTURES {
        for line in agent_lines(fixture) {
            if let Err(error) = parse(&line) {
                panic!("{fixture}: {error}: {line}");
            }
        }
    }
}

#[test]
fn a_turn_names_its_conversation_model_and_mode() {
    let inits = find("turn-permission.jsonl", |event| match event {
        Incoming::Init(init) => Some(init),
        _ => None,
    });
    assert_eq!(
        inits,
        [Init {
            session_id: "75c98f01-f805-4e67-9364-889d9c590313".to_owned(),
            model: "claude-haiku-5-5".to_owned(),
            permission_mode: "default".to_owned(),
        }]
    );
}

#[test]
fn a_streamed_reply_reads_as_message_blocks_and_deltas() {
    let stream = find("turn-ok.jsonl", |event| match event {
        Incoming::Stream(stream) => Some(stream),
        _ => None,
    });
    assert_eq!(
        stream,
        [
            Stream::MessageStart {
                id: "msg_011CgKV8FNZj274cGFd3GXRm".to_owned()
            },
            Stream::BlockStart {
                index: 0,
                kind: BlockKind::Text
            },
            Stream::Delta {
                index: 0,
                delta: Delta::Text("ok".to_owned())
            },
            Stream::BlockStop { index: 0 },
            Stream::Other,
            Stream::Other,
        ]
    );
}

#[test]
fn a_tool_call_starts_by_its_id_and_name() {
    let starts = find("turn-permission.jsonl", |event| match event {
        Incoming::Stream(Stream::BlockStart { kind, .. }) => Some(kind),
        _ => None,
    });
    assert_eq!(
        starts,
        [
            BlockKind::ToolUse {
                id: ToolUseId("toolu_01FtUazAVz9ksL4K2nMikyVK".to_owned()),
                name: "Bash".to_owned()
            },
            BlockKind::Thinking,
            BlockKind::Text,
        ]
    );
}

#[test]
fn finished_blocks_carry_text_thinking_and_tool_input() {
    let blocks = find("turn-image.jsonl", |event| match event {
        Incoming::Assistant(assistant) => Some(assistant),
        _ => None,
    });
    let message = "msg_011CgKJReQbSJdry31sPMmTi".to_owned();
    assert_eq!(
        blocks,
        [
            Assistant {
                message_id: message.clone(),
                blocks: vec![Block::Thinking(String::new())],
                aborted: false,
                error: None,
            },
            Assistant {
                message_id: message,
                blocks: vec![Block::ToolUse {
                    id: ToolUseId("toolu_01EnVVzZ9Hh7LYR9sC5x2z2C".to_owned()),
                    name: "Read".to_owned(),
                    input: json!({"file_path": "/Users/me/project/note.txt"}),
                }],
                aborted: false,
                error: None,
            },
            Assistant {
                message_id: "msg_011CgKJRoWHoTuu7T42okot3".to_owned(),
                blocks: vec![Block::Text("banana red".to_owned())],
                aborted: false,
                error: None,
            },
        ]
    );
}

#[test]
fn an_interrupted_reply_is_marked_aborted_and_its_turn_interrupted() {
    let events = parsed("turn-interrupt.jsonl");
    let aborted = events.iter().find_map(|event| match event {
        Incoming::Assistant(assistant) => Some(assistant),
        _ => None,
    });
    let aborted = aborted.unwrap();
    assert!(aborted.aborted);
    assert!(matches!(&aborted.blocks[..], [Block::Text(text)] if text.starts_with("1\n2\n3\n")));
    let outcomes: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Incoming::Result(result) => Some(&result.outcome),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes, [&Outcome::Interrupted]);
}

#[test]
fn the_message_taken_up_is_echoed_by_its_id() {
    let replays = find("turn-permission.jsonl", |event| match event {
        Incoming::Replay(id) => Some(id),
        _ => None,
    });
    assert_eq!(replays, [uuid()]);
}

#[test]
fn tool_output_comes_back_by_the_call_s_id() {
    let results = find("turn-image.jsonl", |event| match event {
        Incoming::ToolResults(results) => Some(results),
        _ => None,
    });
    assert_eq!(
        results,
        [vec![ToolResult {
            tool_use_id: ToolUseId("toolu_01EnVVzZ9Hh7LYR9sC5x2z2C".to_owned()),
            output: "1\tThe fruit is banana.\n2\t".to_owned(),
            is_error: false,
        }]]
    );
}

#[test]
fn a_turn_s_result_carries_cost_usage_and_the_message_it_answered() {
    let results = find("turn-permission.jsonl", |event| match event {
        Incoming::Result(result) => Some(result),
        _ => None,
    });
    assert_eq!(
        results,
        [TurnResult {
            session_id: "75c98f01-f805-4e67-9364-889d9c590313".to_owned(),
            messages: vec![uuid()],
            total_cost_usd: 0.00490588,
            duration_ms: 3714,
            usage: Usage {
                input_tokens: 4,
                output_tokens: 150,
                cache_read_input_tokens: 44108,
                cache_creation_input_tokens: 21947,
            },
            outcome: Outcome::Completed,
        }]
    );
}

#[test]
fn a_permission_prompt_names_the_tool_its_input_and_its_call() {
    let asks = find("turn-permission.jsonl", |event| match event {
        Incoming::Permission(ask) => Some(ask),
        _ => None,
    });
    assert_eq!(
        asks,
        [PermissionRequest {
            id: RequestId("a6c22aa7-22c2-43d3-9358-c0477a9b497f".to_owned()),
            tool_name: "Bash".to_owned(),
            display_name: Some("Bash".to_owned()),
            title: None,
            description: Some("Create empty probe.txt file in working directory".to_owned()),
            decision_reason: None,
            input: json!({
                "command": "touch probe.txt",
                "description": "Create empty probe.txt file in working directory",
            }),
            tool_use_id: Some(ToolUseId("toolu_01FtUazAVz9ksL4K2nMikyVK".to_owned())),
            blocked_path: Some("/Users/me/project/probe.txt".to_owned()),
            default_to_no: false,
        }]
    );
}

#[test]
fn queue_moves_status_and_rate_limits_read_as_typed_events() {
    let stages = find("turn-interrupt.jsonl", |event| match event {
        Incoming::Lifecycle { message, stage } => Some((message, stage)),
        _ => None,
    });
    let expected = [Stage::Queued, Stage::Started, Stage::Cancelled].map(|stage| (uuid(), stage));
    assert_eq!(stages, expected);

    let statuses = find("settings.jsonl", |event| match event {
        Incoming::Status(status) => Some(status),
        _ => None,
    });
    assert_eq!(
        statuses,
        [AgentStatus {
            status: None,
            permission_mode: Some("acceptEdits".to_owned())
        }]
    );

    let limits = find("turn-permission.jsonl", |event| match event {
        Incoming::RateLimit(limit) => Some(limit),
        _ => None,
    });
    assert_eq!(
        limits,
        [RateLimit {
            status: "allowed".to_owned(),
            resets_at: Some(1792778400),
            kind: Some("five_hour".to_owned()),
            overage_status: Some("rejected".to_owned()),
            using_overage: false,
            overage_in_use: false,
        }]
    );
}

#[test]
fn answers_to_control_requests_carry_their_id() {
    let answers = find("settings.jsonl", |event| match event {
        Incoming::Response { id, result } => Some((id.0, result)),
        _ => None,
    });
    let request = || "{{request}}".to_owned();
    assert_eq!(
        answers,
        [
            (request(), Ok(json!({"mode": "acceptEdits"}))),
            (request(), Ok(json!({"model": "claude-haiku-5-5"}))),
            (request(), Ok(serde_json::Value::Null)),
        ]
    );
    let refused = r#"{"type":"control_response","response":{"subtype":"error","request_id":"host_3","error":"Model 'no-such-model' not found"}}"#;
    assert_eq!(
        parse(refused).unwrap(),
        Incoming::Response {
            id: RequestId("host_3".to_owned()),
            result: Err("Model 'no-such-model' not found".to_owned()),
        }
    );
}

#[test]
fn a_withdrawn_or_unknown_request_reads_by_its_id() {
    let cancel = r#"{"type":"control_cancel_request","request_id":"a6c22aa7"}"#;
    assert_eq!(
        parse(cancel).unwrap(),
        Incoming::Cancel(RequestId("a6c22aa7".to_owned()))
    );
    let other = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"elicitation","message":"?"}}"#;
    assert_eq!(
        parse(other).unwrap(),
        Incoming::Request {
            id: RequestId("r1".to_owned()),
            subtype: "elicitation".to_owned()
        }
    );
}

#[test]
fn a_failed_turn_keeps_the_agent_s_words() {
    let failed = r#"{"type":"result","subtype":"error_max_turns","is_error":true,"errors":["Reached the maximum number of turns (3)"],"terminal_reason":"max_turns","session_id":"s","total_cost_usd":0.1,"duration_ms":5,"usage":{}}"#;
    let Incoming::Result(result) = parse(failed).unwrap() else {
        panic!("not a result");
    };
    assert_eq!(
        result.outcome,
        Outcome::Failed(TurnError {
            message: "Reached the maximum number of turns (3)".to_owned(),
            subtype: "error_max_turns".to_owned(),
            terminal_reason: Some("max_turns".to_owned()),
            api_status: None,
        })
    );
}

#[test]
fn lines_this_crate_does_not_model_come_through_whole() {
    let hooks = agent_lines("initialize.jsonl");
    let hook: serde_json::Value = serde_json::from_str(&hooks[0]).unwrap();
    assert_eq!(parse(&hooks[0]).unwrap(), Incoming::Other(hook));

    let interrupted_notice = agent_lines("turn-interrupt.jsonl")
        .into_iter()
        .find(|line| line.contains("[Request interrupted by user]"))
        .unwrap();
    assert!(matches!(
        parse(&interrupted_notice).unwrap(),
        Incoming::Other(_)
    ));

    for line in [
        r#"{"type":"keep_alive"}"#,
        r#"{"type":"system","subtype":"compact_boundary"}"#,
        r#"{"type":"assistant","parent_tool_use_id":"toolu_task","message":{"id":"m","content":[{"type":"text","text":"from a subagent"}]}}"#,
    ] {
        let parsed = parse(line).unwrap();
        assert!(
            matches!(parsed, Incoming::Other(_)),
            "{line} read as {parsed:?}"
        );
    }
    let unknown_stage = r#"{"type":"command_lifecycle","command_uuid":"u","state":"paused"}"#;
    assert_eq!(
        parse(unknown_stage).unwrap(),
        Incoming::Lifecycle {
            message: MessageId("u".to_owned()),
            stage: Stage::Other
        }
    );
}

#[test]
fn a_line_that_is_not_an_event_is_an_error() {
    for line in [
        "",
        "not json",
        "[1, 2]",
        r#"{"no": "type"}"#,
        r#"{"type": "result", "total_cost_usd": "a lot"}"#,
        r#"{"type": "control_request", "request": {"subtype": "can_use_tool"}}"#,
    ] {
        assert!(parse(line).is_err(), "{line:?} parsed");
    }
}
