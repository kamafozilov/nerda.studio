//! One run of the installed `claude` on haiku, which costs a fraction of a
//! cent. Run it on purpose: `cargo test -p agent --test real -- --ignored`.

mod support;

use std::time::Duration;

use agent::{
    Command, Conversation, ConversationId, Ending, Item, Launch, Message, Session, Status,
    TurnState,
};
use support::{TempDir, Watch};
use workspace::{AgentChoice, ModelId};

#[test]
#[ignore = "runs the installed claude, which costs money"]
fn claude_answers_a_message_end_to_end() {
    let dir = TempDir::new();
    let launch = Launch {
        folder: dir.path().to_owned(),
        choice: AgentChoice {
            model: ModelId("haiku".to_owned()),
            ..AgentChoice::default()
        },
        conversation: Conversation::New(ConversationId::new()),
        program: None,
    };
    let (session, events) = Session::start(&launch).unwrap();
    let mut watch = Watch::new(events);
    let minute = Duration::from_secs(60);
    watch.until_within(minute, "ready", |transcript, _| {
        transcript.status() == &Status::Ready
    });

    let message = Message::new("reply with the word ok", Vec::new());
    session.send(Command::Send(message));
    watch.until_within(2 * minute, "the reply", |transcript, _| {
        let turn = transcript.turns().first();
        turn.is_some_and(|turn| matches!(turn.state, TurnState::Ended(_)))
    });
    let turn = &watch.transcript.turns()[0];
    let TurnState::Ended(Ending::Finished(finished)) = &turn.state else {
        panic!("the turn ended {:?}", turn.state);
    };
    let reply: String = turn
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(reply.to_lowercase().contains("ok"), "replied {reply:?}");
    eprintln!("replied {reply:?}; {finished:?}");

    drop(session);
    watch.until("the exit", |transcript, _| {
        matches!(transcript.status(), Status::Exited { .. })
    });
    assert_eq!(watch.transcript.status(), &Status::Exited { code: Some(0) });
}
