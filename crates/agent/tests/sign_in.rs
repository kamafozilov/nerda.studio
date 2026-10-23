//! Its own test binary: the agent's sign-in folder comes from the
//! environment, which only this test sets.

mod support;

use std::time::{Duration, Instant};

use agent::{Session, Status};
use support::{TempDir, Watch, fake, project};

#[test]
fn ending_an_agent_waits_while_it_refreshes_its_sign_in() {
    let config = TempDir::new();
    let lock = config.path().join(".oauth_refresh.lock");
    std::fs::create_dir(&lock).unwrap();
    // SAFETY: the only test in this binary, and no thread of it reads the
    // environment yet.
    unsafe { std::env::set_var("CLAUDE_CONFIG_DIR", config.path()) };

    let dir = project(&["initialize.jsonl"]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    watch.until("ready", |transcript, _| {
        transcript.status() == &Status::Ready
    });
    drop(session);
    let held = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < held {
        watch.drain();
        assert_eq!(
            watch.transcript.status(),
            &Status::Ready,
            "ended mid-refresh"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    std::fs::remove_dir(&lock).unwrap();
    let released = Instant::now();
    watch.until("the exit", |transcript, _| {
        matches!(transcript.status(), Status::Exited { .. })
    });
    assert_eq!(watch.transcript.status(), &Status::Exited { code: Some(0) });
    assert!(
        released.elapsed() < Duration::from_secs(1),
        "{:?}",
        released.elapsed()
    );
}
