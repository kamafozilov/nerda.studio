//! Full access can't be switched on or off in a running agent, so the
//! session starts the agent again on the same conversation.

mod support;

use agent::{Command, Conversation, ConversationId, Event, Message, Refusal, Session, Status};
use support::{TempDir, Watch, arguments, exited, fake, project, status_is};
use workspace::Access;

/// The value after `flag` in the arguments the agent last started with.
fn after(dir: &TempDir, flag: &str) -> Option<String> {
    let arguments = arguments(dir);
    let at = arguments.iter().position(|argument| argument == flag)?;
    arguments.get(at + 1).cloned()
}

fn starts(seen: &[Event]) -> usize {
    seen.iter()
        .filter(|event| **event == Event::Started)
        .count()
}

#[test]
fn full_access_starts_the_agent_again_resuming_its_conversation() {
    let dir = project(&["initialize.jsonl", "turn-ok.jsonl"]);
    let id = ConversationId::new();
    let mut launch = fake(&dir);
    launch.conversation = Conversation::New(id);
    let (session, events) = Session::start(&launch).unwrap();
    let mut watch = Watch::new(events);
    session.send(Command::Send(Message::new(
        "reply with the word ok",
        Vec::new(),
    )));
    watch.until("the reply", |transcript, _| {
        transcript.turns().len() == 1 && transcript.status() == &Status::Ready
    });
    let first = session.pid();
    assert_eq!(after(&dir, "--permission-mode").as_deref(), Some("manual"));

    session.send(Command::SetAccess(Access::FullAccess));
    watch.until("the new process", |transcript, seen| {
        starts(seen) == 2 && transcript.status() == &Status::Ready
    });
    assert!(
        watch.seen.contains(&Event::Exited { code: Some(0) }),
        "the old one quit first"
    );
    assert_ne!(session.pid(), first);
    assert_eq!(
        after(&dir, "--permission-mode").as_deref(),
        Some("bypassPermissions")
    );
    assert_eq!(after(&dir, "--resume"), Some(id.to_string()));
    assert_eq!(after(&dir, "--session-id"), None);

    session.send(Command::Send(Message::new(
        "reply with the word ok",
        Vec::new(),
    )));
    watch.until("the reply in full access", |transcript, _| {
        transcript.turns().len() == 2 && transcript.status() == &Status::Ready
    });
    drop(session);
    watch.until("the exit", exited);
}

#[test]
fn a_conversation_without_a_turn_starts_anew_under_its_id() {
    let dir = project(&["initialize.jsonl"]);
    let id = ConversationId::new();
    let mut launch = fake(&dir);
    launch.choice.access = Access::FullAccess;
    launch.conversation = Conversation::New(id);
    let (session, events) = Session::start(&launch).unwrap();
    let mut watch = Watch::new(events);
    watch.until("ready", status_is(Status::Ready));

    session.send(Command::SetAccess(Access::AcceptEdits));
    watch.until("the new process", |transcript, seen| {
        starts(seen) == 2 && transcript.status() == &Status::Ready
    });
    assert_eq!(
        after(&dir, "--permission-mode").as_deref(),
        Some("acceptEdits")
    );
    assert_eq!(after(&dir, "--session-id"), Some(id.to_string()));
    assert_eq!(after(&dir, "--resume"), None);
    drop(session);
}

#[test]
fn full_access_is_refused_while_the_agent_works() {
    let dir = project(&["initialize.jsonl", "turn-permission.jsonl"]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    session.send(Command::Send(Message::new("Use the Bash tool", Vec::new())));
    watch.until("the prompt", status_is(Status::AwaitingPermission));
    let pid = session.pid();

    session.send(Command::SetAccess(Access::FullAccess));
    let refused = Event::Refused {
        command: Command::SetAccess(Access::FullAccess),
        reason: Refusal::TurnRunning,
    };
    watch.until("the refusal", |_, seen| seen.contains(&refused));
    assert_eq!(watch.transcript.status(), &Status::AwaitingPermission);
    assert_eq!(session.pid(), pid);
    assert_eq!(starts(&watch.seen), 1);
    assert!(Refusal::TurnRunning.to_string().contains("Full access"));
    drop(session);
}

#[test]
fn only_full_access_starts_the_agent_without_permission_checks() {
    let dir = project(&["initialize.jsonl"]);
    for access in [
        Access::AskFirst,
        Access::AcceptEdits,
        Access::Plan,
        Access::Auto,
        Access::FullAccess,
    ] {
        let mut launch = fake(&dir);
        launch.choice.access = access;
        let (session, events) = Session::start(&launch).unwrap();
        Watch::new(events).until("ready", status_is(Status::Ready));
        let unchecked = after(&dir, "--permission-mode").as_deref() == Some("bypassPermissions");
        assert_eq!(unchecked, access == Access::FullAccess, "{access:?}");
        assert!(
            !arguments(&dir)
                .iter()
                .any(|argument| argument.contains("dangerously")),
            "{access:?}"
        );
        drop(session);
    }
}
