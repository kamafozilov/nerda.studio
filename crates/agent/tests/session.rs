mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use agent::{
    Answer, Attachment, Command, Conversation, ConversationId, Ending, Event, Failure, Image,
    Incoming, Item, Message, MessageId, RequestId, Session, StartError, Status, Text, ToolStatus,
    Transcript, TurnState,
};
use serde_json::json;
use support::{TempDir, Watch, arguments, exited, fake, project, script_lines, status_is, written};
use workspace::{Access, AgentChoice, AgentId, Effort, ModelId};

fn texts(transcript: &Transcript, turn: usize) -> Vec<String> {
    transcript.turns()[turn]
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_agent_asks_before_a_tool_runs_and_runs_it_once_allowed() {
    let dir = project(&["initialize.jsonl", "turn-permission.jsonl"]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    watch.until("ready", status_is(Status::Ready));
    assert_eq!(watch.seen[0], Event::Started);

    let message = Message::new("Use the Bash tool: touch probe.txt", Vec::new());
    session.send(Command::Send(message.clone()));
    watch.until("the prompt", status_is(Status::AwaitingPermission));
    let allow = watch
        .transcript
        .pending_permissions()
        .next()
        .unwrap()
        .answer(Answer::AllowOnce);
    session.send(allow);
    watch.until("the reply", status_is(Status::Ready));
    let turn = &watch.transcript.turns()[0];
    assert!(
        matches!(&turn.items[0], Item::Tool(call) if call.status == ToolStatus::Done("(Bash completed with no output)".to_owned()))
    );
    assert_eq!(texts(&watch.transcript, 0), ["ok"]);

    drop(session);
    watch.until("the exit", exited);
    assert_eq!(watch.transcript.status(), &Status::Exited { code: Some(0) });
    let input = json!({"command": "touch probe.txt", "description": "Create empty probe.txt file in working directory"});
    let [id] = &sent_ids(&watch.seen)[..] else {
        panic!("one message");
    };
    assert_eq!(
        written(&dir),
        [
            json!({"type": "control_request", "request_id": "initialize", "request": {"subtype": "initialize"}}),
            json!({"type": "user", "uuid": id.0, "parent_tool_use_id": null, "message": {
                "role": "user", "content": [{"type": "text", "text": "Use the Bash tool: touch probe.txt"}]}}),
            json!({"type": "control_response", "response": {"subtype": "success",
                "request_id": "a6c22aa7-22c2-43d3-9358-c0477a9b497f",
                "response": {"behavior": "allow", "updatedInput": input}}}),
        ]
    );
}

/// The ids messages went to the agent under, as their `Sent` events say.
fn sent_ids(seen: &[Event]) -> Vec<MessageId> {
    seen.iter()
        .filter_map(|event| match event {
            Event::Sent(Command::Send(message)) => Some(message.id.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_message_sent_again_goes_under_a_new_id_and_runs_again() {
    let dir = project(&[
        "initialize.jsonl",
        "turn-after-interrupt.jsonl",
        "turn-after-interrupt.jsonl",
    ]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    let message = Message::new("What is 17*23?", Vec::new());
    session.send(Command::Send(message.clone()));
    watch.until("the first answer", status_is(Status::Ready));
    session.send(Command::Send(message.clone()));
    watch.until("the second answer", |transcript, _| {
        transcript.turns().len() == 2 && transcript.status() == &Status::Ready
    });
    assert_eq!(
        [texts(&watch.transcript, 0), texts(&watch.transcript, 1)],
        [["391"], ["391"]]
    );
    let ids = sent_ids(&watch.seen);
    let uuids: Vec<_> = written(&dir)
        .into_iter()
        .filter(|line| line["type"] == "user")
        .map(|line| MessageId(line["uuid"].as_str().unwrap().to_owned()))
        .collect();
    assert_eq!(uuids, ids);
    assert_ne!(ids[0], ids[1]);
    assert!(
        !ids.contains(&message.id),
        "written under the id it was made with"
    );
    drop(session);
}

#[test]
fn changes_mid_session_and_attachments_reach_the_agent() {
    let dir = project(&["initialize.jsonl", "settings.jsonl", "turn-image.jsonl"]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    for command in [
        Command::SetAccess(Access::AcceptEdits),
        Command::SetModel(ModelId("haiku".to_owned())),
        Command::SetEffort(Effort::Low),
    ] {
        session.send(command);
    }
    watch.until("three answers", |_, seen| {
        let answers = seen.iter().filter(|event| {
            matches!(event, Event::Received(Incoming::Response { id, .. }) if id.0.starts_with("host_"))
        });
        answers.count() == 3
    });
    let png = Image::new(vec![0x89, b'P', b'N', b'G']).unwrap();
    let attachments = vec![
        Attachment::File(PathBuf::from("/Users/me/project/note.txt")),
        Attachment::Image(png),
    ];
    let message = Message::new("What fruit, and what color?", attachments);
    session.send(Command::Send(message));
    watch.until("the reply", |transcript, _| {
        transcript.turns().len() == 1 && transcript.status() == &Status::Ready
    });
    assert_eq!(texts(&watch.transcript, 0), ["banana red"]);
    drop(session);
    watch.until("the exit", exited);

    let written = written(&dir);
    let requests: Vec<_> = written[1..4]
        .iter()
        .map(|line| line["request"].clone())
        .collect();
    assert_eq!(
        requests,
        [
            json!({"subtype": "set_permission_mode", "mode": "acceptEdits"}),
            json!({"subtype": "set_model", "model": "haiku"}),
            json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": "low"}}),
        ]
    );
    assert_eq!(
        written[4]["message"]["content"],
        json!([
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBORw=="}},
            {"type": "text", "text": "What fruit, and what color?\n\nAttached files:\n/Users/me/project/note.txt"},
        ])
    );
}

#[test]
fn stop_ends_the_turn_and_the_agent_answers_the_next_message() {
    let dir = project(&[
        "initialize.jsonl",
        "turn-interrupt.jsonl",
        "turn-after-interrupt.jsonl",
    ]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    session.send(Command::Send(Message::new("Count to 300", Vec::new())));
    watch.until("text streaming", |transcript, _| {
        let turn = transcript.turns().first();
        let items = turn.map(|turn| &turn.items[..]).unwrap_or_default();
        matches!(
            items,
            [Item::Text(Text {
                streaming: true,
                ..
            })]
        )
    });
    session.send(Command::Interrupt);
    watch.until("stopping", status_is(Status::Stopping));
    watch.until("stopped", status_is(Status::Ready));
    assert_eq!(
        watch.transcript.turns()[0].state,
        TurnState::Ended(Ending::Interrupted)
    );

    session.send(Command::Send(Message::new("What is 17*23?", Vec::new())));
    watch.until("the answer", |transcript, _| {
        transcript.turns().len() == 2 && transcript.status() == &Status::Ready
    });
    assert_eq!(texts(&watch.transcript, 1), ["391"]);
    drop(session);
    watch.until("the exit", exited);
    let interrupt = written(&dir)
        .into_iter()
        .find(|line| line["request"]["subtype"] == "interrupt");
    assert_eq!(interrupt.unwrap()["request"]["cancel_queued"], true);
}

#[test]
fn the_agent_quitting_mid_turn_ends_the_turn_after_its_last_words() {
    let mut steps: Vec<String> = vec!["initialize.jsonl".to_owned()];
    let turn = script_lines("turn-interrupt.jsonl");
    let first_delta = turn
        .iter()
        .position(|line| line.contains("text_delta"))
        .unwrap();
    steps.extend(turn[..=first_delta].iter().cloned());
    steps.push(r#"{"agent": "stderr", "text": "Error: the model went away"}"#.to_owned());
    steps.push(r#"{"agent": "exit", "code": 3}"#.to_owned());
    let dir = project(&steps.iter().map(String::as_str).collect::<Vec<_>>());

    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    session.send(Command::Send(Message::new("Count to 300", Vec::new())));
    watch.until("the exit", exited);
    assert_eq!(watch.transcript.status(), &Status::Exited { code: Some(3) });
    let turn = &watch.transcript.turns()[0];
    let crashed = Failure::Crashed {
        code: Some(3),
        stderr: "Error: the model went away".to_owned(),
    };
    assert_eq!(
        turn.state,
        TurnState::Ended(Ending::Failed(crashed.clone()))
    );
    assert_eq!(
        crashed.message(),
        "The agent quit before it finished (exit code 3)."
    );
    assert_eq!(
        turn.items,
        [Item::Text(Text {
            text: "1\n2\n3\n4".to_owned(),
            streaming: false
        })]
    );
    // Stdout and stderr are separate pipes, so only the exit's place is fixed.
    let said = Event::Stderr("Error: the model went away".to_owned());
    assert!(watch.seen.contains(&said));
    assert_eq!(watch.seen.last(), Some(&Event::Exited { code: Some(3) }));
    drop(session);
}

#[test]
fn the_exit_comes_even_while_a_process_the_agent_started_holds_its_stderr() {
    let dir = project(&[
        "initialize.jsonl",
        r#"{"agent": "orphan"}"#,
        r#"{"agent": "exit", "code": 4}"#,
    ]);
    let started = Instant::now();
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    watch.until("the exit", exited);
    assert_eq!(watch.transcript.status(), &Status::Exited { code: Some(4) });
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    drop(session);
}

#[test]
fn a_line_that_is_not_an_event_is_skipped_and_the_session_goes_on() {
    let mut steps = vec!["initialize.jsonl".to_owned()];
    let mut turn = script_lines("turn-after-interrupt.jsonl");
    turn.insert(
        1,
        r#"{"agent": "raw", "text": "this is not json"}"#.to_owned(),
    );
    turn.insert(
        2,
        r#"{"agent": "raw", "text": "{\"type\": \"keep_alive\"}"}"#.to_owned(),
    );
    steps.extend(turn);
    let dir = project(&steps.iter().map(String::as_str).collect::<Vec<_>>());

    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    session.send(Command::Send(Message::new("What is 17*23?", Vec::new())));
    watch.until("the answer", |transcript, _| {
        transcript.turns().len() == 1 && transcript.status() == &Status::Ready
    });
    assert_eq!(texts(&watch.transcript, 0), ["391"]);
    let unreadable: Vec<_> = watch
        .seen
        .iter()
        .filter_map(|event| match event {
            Event::Unreadable { line, .. } => Some(line.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(unreadable, ["this is not json"]);
    assert!(watch.seen.contains(&Event::Received(Incoming::Other(
        json!({"type": "keep_alive"})
    ))));
    drop(session);
}

#[test]
fn a_request_the_app_does_not_handle_is_declined() {
    let dir = project(&[
        "initialize.jsonl",
        r#"{"type": "control_request", "request_id": "r9", "request": {"subtype": "elicitation", "message": "Pick one"}}"#,
        r#"{"host": "answer"}"#,
        r#"{"agent": "exit", "code": 0}"#,
    ]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    watch.until("the exit", exited);
    assert!(watch.seen.contains(&Event::Received(Incoming::Request {
        id: RequestId("r9".to_owned()),
        subtype: "elicitation".to_owned(),
    })));
    let decline = written(&dir).pop().unwrap();
    assert_eq!(decline["response"]["subtype"], "error");
    assert_eq!(decline["response"]["request_id"], "r9");
    drop(session);
}

#[test]
fn a_message_after_the_agent_quit_is_not_lost_without_a_word() {
    let dir = project(&["initialize.jsonl", r#"{"agent": "exit", "code": 0}"#]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    watch.until("the exit", exited);

    for text in ["still there?", "hello?"] {
        session.send(Command::Send(Message::new(text, Vec::new())));
    }
    watch.until("both messages", |transcript, _| {
        transcript.turns().len() == 2
    });
    let unsent = TurnState::Ended(Ending::Failed(Failure::NotSent));
    for turn in watch.transcript.turns() {
        assert_eq!(turn.state, unsent);
    }
}

#[test]
fn a_session_that_cannot_start_says_why_naming_the_program() {
    let dir = TempDir::new();
    let mut launch = fake(&dir);
    launch.program = Some(PathBuf::from("/nowhere/claude"));
    let error = Session::start(&launch).err().unwrap();
    assert!(matches!(error, StartError::NotFound { .. }));
    assert_eq!(
        error.to_string(),
        "Couldn't find claude. Install it with `curl -fsSL https://claude.ai/install.sh | bash` in a terminal, then try again."
    );

    launch.program = Some(PathBuf::from("no-such-agent-on-path"));
    assert!(matches!(
        Session::start(&launch),
        Err(StartError::NotFound { .. })
    ));

    let mut missing = fake(&dir);
    missing.folder = dir.path().join("gone");
    assert!(matches!(
        Session::start(&missing),
        Err(StartError::NoFolder { .. })
    ));

    let mut unknown = fake(&dir);
    unknown.choice.agent = AgentId("someday".to_owned());
    assert!(matches!(
        Session::start(&unknown),
        Err(StartError::UnknownAgent(_))
    ));

    let mut transcript = Transcript::new();
    transcript.apply(&Event::Failed(error.to_string()));
    assert_eq!(
        transcript.status(),
        &Status::Failed {
            message: error.to_string()
        }
    );
}

#[test]
fn the_agent_runs_with_its_own_folder_and_the_app_s_path() {
    let dir = project(&["initialize.jsonl"]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    Watch::new(events).until("ready", status_is(Status::Ready));
    let path = std::fs::read_to_string(dir.path().join("fake-agent.path")).unwrap();
    let folders: Vec<PathBuf> = std::env::split_paths(&path).collect();
    let program = PathBuf::from(env!("CARGO_BIN_EXE_fake-agent"));
    assert_eq!(folders.first().map(PathBuf::as_path), program.parent());
    let ours = std::env::var_os("PATH").unwrap();
    for folder in std::env::split_paths(&ours).filter(|folder| !folder.as_os_str().is_empty()) {
        assert!(folders.contains(&folder), "{folder:?} missing from {path}");
    }
    drop(session);
}

#[test]
fn the_agent_starts_on_the_session_s_choice_and_conversation() {
    let dir = project(&["initialize.jsonl"]);
    let id = ConversationId::new();
    let mut launch = fake(&dir);
    launch.choice = AgentChoice {
        model: ModelId("sonnet".to_owned()),
        effort: Effort::ExtraHigh,
        access: Access::AcceptEdits,
        ..AgentChoice::default()
    };
    launch.conversation = Conversation::New(id);
    let stream_json = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--replay-user-messages",
        "--thinking-display",
        "summarized",
        "--permission-prompt-tool",
        "stdio",
    ];
    for (conversation, flag) in [
        (Conversation::New(id), "--session-id"),
        (Conversation::Resume(id), "--resume"),
    ] {
        launch.conversation = conversation;
        let (session, events) = Session::start(&launch).unwrap();
        Watch::new(events).until("ready", status_is(Status::Ready));
        let mut expected: Vec<String> = stream_json.iter().map(|a| a.to_string()).collect();
        let choice = [
            "--model",
            "sonnet",
            "--effort",
            "xhigh",
            "--permission-mode",
            "acceptEdits",
        ];
        expected.extend(choice.iter().map(|a| a.to_string()));
        expected.extend([flag.to_owned(), id.to_string()]);
        assert_eq!(arguments(&dir), expected);
        drop(session);
    }
}
