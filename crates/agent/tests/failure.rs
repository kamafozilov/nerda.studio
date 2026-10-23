mod support;

use agent::protocol::parse;
use agent::{Command, Ending, Failure, Incoming, Message, Status, Transcript, TurnState};
use support::{Player, says};

fn ready() -> Player {
    let mut player = Player::new();
    player.play("initialize.jsonl", says("unused"));
    player
}

/// A turn of `message` that the agent takes up and then receives `lines`,
/// `{{uuid}}` standing for the message's id; how it ended.
fn ended(lines: &[&str]) -> Ending {
    let mut player = ready();
    let message = Message::new("go on", Vec::new());
    player.send(Command::Send(message.clone()));
    let started = r#"{"type":"command_lifecycle","command_uuid":"{{uuid}}","state":"started"}"#;
    for line in std::iter::once(&started).chain(lines) {
        player.receive(&line.replace("{{uuid}}", &message.id.0));
    }
    let transcript: &Transcript = &player.transcript;
    assert_eq!(transcript.status(), &Status::Ready, "the turn ended");
    match &transcript.turns()[0].state {
        TurnState::Ended(ending) => ending.clone(),
        other => panic!("still {other:?}"),
    }
}

fn failed(lines: &[&str]) -> Failure {
    match ended(lines) {
        Ending::Failed(failure) => failure,
        other => panic!("ended {other:?}"),
    }
}

fn result(fields: &str) -> String {
    format!(
        r#"{{"type":"result","session_id":"s","total_cost_usd":0,"duration_ms":10,"usage":{{}},"user_message_uuids":["{{{{uuid}}}}"],{fields}}}"#
    )
}

/// The agent's own message about a failed request, as it writes one.
fn api_error(code: &str, status: u16, text: &str) -> String {
    format!(
        r#"{{"type":"assistant","message":{{"id":"m1","model":"<synthetic>","role":"assistant","content":[{{"type":"text","text":"{text}"}}]}},"parent_tool_use_id":null,"error":"{code}","is_api_error_message":true,"api_error_status":{status}}}"#
    )
}

#[test]
fn a_refused_key_reads_as_signed_out_with_the_agent_s_words() {
    let mut player = ready();
    player.play("turn-signed-out.jsonl", says("reply with the word ok"));
    let turn = &player.transcript.turns()[0];
    let TurnState::Ended(Ending::Failed(failure)) = &turn.state else {
        panic!("ended {:?}", turn.state);
    };
    assert_eq!(
        failure,
        &Failure::SignedOut {
            message: "Invalid API key · Fix external API key".to_owned()
        }
    );
    assert_eq!(failure.message(), "The agent isn't signed in.");
    assert!(failure.suggestion().unwrap().contains("claude auth login"));
    assert!(
        turn.items.is_empty(),
        "the agent's error isn't a reply: {:?}",
        turn.items
    );
    assert_eq!(player.transcript.status(), &Status::Ready);
}

#[test]
fn a_model_that_does_not_exist_fails_in_the_agent_s_words() {
    let mut player = ready();
    player.play("turn-model-not-found.jsonl", says("reply with the word ok"));
    let turn = &player.transcript.turns()[0];
    let words = "There's an issue with the selected model (claude-nonexistent-9-9). It may not exist or you may not have access to it. Run --model to pick a different model.";
    assert_eq!(
        turn.state,
        TurnState::Ended(Ending::Failed(Failure::Other(words.to_owned())))
    );
    assert!(turn.items.is_empty());
}

#[test]
fn a_rejecting_rate_limit_fails_the_turn_with_its_reset_time() {
    let limit = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792600200000,"rateLimitType":"five_hour","overageStatus":"rejected"}}"#;
    let error = api_error(
        "rate_limit",
        429,
        "You've hit your session limit · resets 7pm",
    );
    let end = result(
        r#""subtype":"success","is_error":true,"api_error_status":429,"result":"You've hit your session limit · resets 7pm""#,
    );
    let failure = failed(&[limit, &error, &end]);
    assert_eq!(
        failure,
        Failure::UsageLimit {
            resets_at: Some(1_792_600_200)
        }
    );
    assert_eq!(failure.message(), "You've reached your usage limit.");

    let older = result(
        r#""subtype":"success","is_error":true,"result":"Claude AI usage limit reached|1792600200""#,
    );
    assert_eq!(
        failed(&[&older]),
        Failure::UsageLimit {
            resets_at: Some(1_792_600_200)
        }
    );
}

#[test]
fn a_limit_covered_by_extra_usage_or_a_subagent_s_limit_stops_nothing() {
    let reply = r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"done"}]},"parent_tool_use_id":null}"#;
    let end = result(r#""subtype":"success","is_error":false,"result":"done""#);
    for limit in [
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","rateLimitType":"five_hour","overageStatus":"allowed"}}"#,
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","isUsingOverage":true}}"#,
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","rateLimitType":"overage"}}"#,
    ] {
        assert!(
            matches!(ended(&[limit, reply, &end]), Ending::Finished(_)),
            "{limit}"
        );
    }
    let subagent = r#"{"type":"rate_limit_event","parent_tool_use_id":"toolu_1","rate_limit_info":{"status":"rejected"}}"#;
    assert!(matches!(parse(subagent).unwrap(), Incoming::Other(_)));
}

#[test]
fn errors_written_as_objects_still_end_the_turn() {
    let end = result(
        r#""subtype":"error_during_execution","is_error":true,"errors":[{"message":"The tool runner crashed"},"[ede_diagnostic] result_type=user stop_reason=null"]"#,
    );
    assert_eq!(
        failed(&[&end]),
        Failure::Other("The tool runner crashed".to_owned())
    );
}

#[test]
fn a_failure_with_only_diagnostics_still_says_something() {
    let end = result(
        r#""subtype":"error_during_execution","is_error":true,"errors":["[ede_diagnostic] turn aborted stop_reason=null"]"#,
    );
    assert_eq!(
        failed(&[&end]),
        Failure::Other("The turn ended with an error.".to_owned())
    );
    let too_long =
        result(r#""subtype":"success","is_error":false,"terminal_reason":"prompt_too_long""#);
    let Failure::Other(words) = failed(&[&too_long]) else {
        panic!("not in words");
    };
    assert!(words.contains("/compact"), "{words}");
}

#[test]
fn an_overloaded_or_failing_model_service_reads_as_such() {
    let error = api_error("overloaded", 529, "API Error: 529 Overloaded");
    let end = result(
        r#""subtype":"success","is_error":true,"api_error_status":529,"result":"API Error: 529 Overloaded""#,
    );
    assert_eq!(failed(&[&error, &end]), Failure::Overloaded);
    let bare =
        result(r#""subtype":"success","is_error":true,"result":"API Error: 529 Overloaded""#);
    assert_eq!(failed(&[&bare]), Failure::Overloaded);

    let server = result(
        r#""subtype":"success","is_error":true,"api_error_status":500,"result":"API Error: 500 Internal server error""#,
    );
    let failure = failed(&[&server]);
    assert_eq!(
        failure,
        Failure::Provider {
            status: Some(500),
            message: "API Error: 500 Internal server error".to_owned()
        }
    );
    assert_eq!(
        failure.message(),
        "The model's service failed the request (HTTP 500)."
    );
}

#[test]
fn a_turn_whose_request_failed_and_that_has_no_reply_failed_whatever_its_result_says() {
    let error = api_error("max_output_tokens", 200, "");
    let end = result(r#""subtype":"success","is_error":false,"result":"""#);
    assert_eq!(
        failed(&[&error, &end]),
        Failure::Other("The reply reached the longest the model can write.".to_owned())
    );
}
