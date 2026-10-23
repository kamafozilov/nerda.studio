//! A session's events written to a journal, one JSON line each, and read
//! back into the same transcript.

mod support;

use std::path::PathBuf;

use agent::protocol::parse;
use agent::{
    Answer, Attachment, Command, Ending, Event, Image, Item, Message, PermissionState, Session,
    Status, Transcript, TurnState,
};
use support::{FIXTURES, Line, Player, Watch, agent_lines, exited, fake, project, says, status_is};
use workspace::Access;

fn journal(events: &[Event]) -> String {
    let lines: Vec<String> = events
        .iter()
        .map(|event| serde_json::to_string(event).unwrap())
        .collect();
    lines.join("\n")
}

fn read(journal: &str) -> Vec<Event> {
    journal
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn a_session_s_journal_replays_into_the_transcript_it_showed() {
    let dir = project(&[
        "initialize.jsonl",
        "turn-permission.jsonl",
        "settings.jsonl",
        "turn-image.jsonl",
    ]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    session.send(Command::Send(Message::new(
        "Use the Bash tool: touch probe.txt",
        Vec::new(),
    )));
    watch.until("the prompt", status_is(Status::AwaitingPermission));
    let prompt = watch
        .transcript
        .pending_permissions()
        .next()
        .unwrap()
        .clone();
    session.send(prompt.answer(Answer::AllowOnce));
    watch.until("the reply", status_is(Status::Ready));
    session.send(Command::SetAccess(Access::AcceptEdits));
    session.send(Command::SetModel(workspace::ModelId("haiku".to_owned())));
    session.send(Command::SetEffort(workspace::Effort::Low));
    let png = Image::new(vec![0x89, b'P', b'N', b'G', 1, 2, 3]).unwrap();
    let attachments = vec![
        Attachment::File(PathBuf::from("/Users/me/project/note.txt")),
        Attachment::Image(png),
    ];
    session.send(Command::Send(Message::new(
        "What fruit, and what color? Привет, oʻzbek",
        attachments,
    )));
    watch.until("the second reply", |transcript, _| {
        transcript.turns().len() == 2 && transcript.status() == &Status::Ready
    });
    drop(session);
    watch.until("the exit", exited);

    let written = journal(&watch.seen);
    assert_eq!(
        written.lines().count(),
        watch.seen.len(),
        "one line an event"
    );
    let replayed = read(&written);
    assert_eq!(replayed, watch.seen);
    assert_eq!(Transcript::replay(&replayed), watch.transcript);
}

#[test]
fn every_recorded_line_survives_the_journal() {
    for fixture in FIXTURES {
        for line in agent_lines(fixture) {
            let event = Event::Received(parse(&line).unwrap());
            let written = serde_json::to_string(&event).unwrap();
            let read: Event = serde_json::from_str(&written)
                .unwrap_or_else(|error| panic!("{fixture}: {error}: {written}"));
            assert_eq!(read, event, "{fixture}");
        }
    }
    let unreadable = Event::Unreadable {
        line: "not json".to_owned(),
        error: parse("not json").unwrap_err(),
    };
    let written = serde_json::to_string(&unreadable).unwrap();
    assert_eq!(serde_json::from_str::<Event>(&written).unwrap(), unreadable);
}

#[test]
fn a_journal_cut_short_ends_what_was_open_once_a_new_process_starts() {
    let mut player = Player::new();
    player.play("initialize.jsonl", says("unused"));
    let mut host = says("Use the Bash tool");
    for line in support::fixture("turn-permission.jsonl") {
        if matches!(&line, Line::Host(kind) if kind == "answer") {
            break;
        }
        player.step(line, &mut host);
    }
    let cut = player.transcript.clone();
    assert_eq!(
        cut.status(),
        &Status::AwaitingPermission,
        "the app quit here"
    );
    let mut restarted = cut;
    restarted.apply(&Event::Started);
    let turn = &restarted.turns()[0];
    assert_eq!(turn.state, TurnState::Ended(Ending::Interrupted));
    let prompt = turn.items.iter().find_map(|item| match item {
        Item::Permission(permission) => Some(permission.state),
        _ => None,
    });
    assert_eq!(
        prompt,
        Some(PermissionState::Cancelled),
        "never offered as if its process lived"
    );
    assert_eq!(restarted.status(), &Status::Starting);
}

#[test]
fn an_image_reads_back_only_as_an_image() {
    let not_an_image =
        r#"{"Sent":{"Send":{"id":"m","text":"","attachments":[{"Image":"bm90IGFuIGltYWdl"}]}}}"#;
    assert!(serde_json::from_str::<Event>(not_an_image).is_err());
    let png = r#"{"Sent":{"Send":{"id":"m","text":"","attachments":[{"Image":"iVBORw=="}]}}}"#;
    let Event::Sent(Command::Send(message)) = serde_json::from_str::<Event>(png).unwrap() else {
        panic!("not a message");
    };
    assert!(
        matches!(&message.attachments[..], [Attachment::Image(image)] if image.bytes() == [0x89, b'P', b'N', b'G'])
    );
}
