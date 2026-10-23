//! Ending a session: the agent and everything it started go with it.

mod support;

use std::time::{Duration, Instant};

use agent::{Session, Status};
use support::{Watch, exited, fake, project, status_is};

#[test]
fn an_idle_agent_quits_when_its_session_is_dropped() {
    let dir = project(&["initialize.jsonl"]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    watch.until("ready", status_is(Status::Ready));
    let dropped = Instant::now();
    drop(session);
    watch.until("the exit", exited);
    assert_eq!(watch.transcript.status(), &Status::Exited { code: Some(0) });
    assert!(
        dropped.elapsed() < Duration::from_secs(1),
        "{:?}",
        dropped.elapsed()
    );
}

#[test]
fn an_agent_that_ignores_its_input_closing_is_killed() {
    let dir = project(&["initialize.jsonl", r#"{"agent": "hang"}"#]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let pid = session.pid();
    let mut watch = Watch::new(events);
    watch.until("ready", status_is(Status::Ready));
    let dropped = Instant::now();
    drop(session);
    watch.until("the exit", exited);
    let waited = dropped.elapsed();
    assert_eq!(watch.transcript.status(), &Status::Exited { code: None });
    assert!(
        (Duration::from_secs(2)..Duration::from_secs(4)).contains(&waited),
        "{waited:?}"
    );
    assert!(!runs(pid), "process {pid} still runs");
}

#[cfg(unix)]
fn pid(dir: &support::TempDir, name: &str) -> u32 {
    let file = dir.path().join(format!("{name}.pid"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(pid) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| text.parse().ok())
        {
            return pid;
        }
        assert!(Instant::now() < deadline, "{name} never started");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Kills what a failed test leaves behind.
#[cfg(unix)]
struct Reap(Vec<u32>);

#[cfg(unix)]
impl Drop for Reap {
    fn drop(&mut self) {
        for pid in &self.0 {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status();
        }
    }
}

#[cfg(unix)]
#[test]
fn dropping_a_session_ends_what_the_agent_started_in_its_group_and_outside_it() {
    let dir = project(&[
        "initialize.jsonl",
        r#"{"agent": "spawn", "name": "tool"}"#,
        r#"{"agent": "spawn", "name": "server", "group": "own"}"#,
    ]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    watch.until("ready", status_is(Status::Ready));
    let started = [pid(&dir, "tool"), pid(&dir, "server")];
    let _reap = Reap(started.to_vec());
    assert!(started.iter().all(|&pid| runs(pid)));
    drop(session);
    watch.until("the exit", exited);
    assert_eq!(watch.transcript.status(), &Status::Exited { code: Some(0) });
    for pid in started {
        assert!(!runs(pid), "process {pid} outlived the session");
    }
}

#[cfg(unix)]
#[test]
fn an_agent_that_quits_on_its_own_takes_its_group_with_it() {
    let dir = project(&[
        "initialize.jsonl",
        r#"{"agent": "spawn", "name": "tool"}"#,
        r#"{"agent": "exit", "code": 0}"#,
    ]);
    let (session, events) = Session::start(&fake(&dir)).unwrap();
    let mut watch = Watch::new(events);
    let tool = pid(&dir, "tool");
    let _reap = Reap(vec![tool]);
    watch.until("the exit", exited);
    assert!(!runs(tool), "process {tool} outlived the agent");
    drop(session);
}

fn runs(pid: u32) -> bool {
    #[cfg(unix)]
    let runs = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success();
    #[cfg(windows)]
    let runs = {
        let filter = format!("PID eq {pid}");
        let list = std::process::Command::new("tasklist")
            .args(["/FI", &filter, "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&list.stdout).contains(&pid.to_string())
    };
    runs
}
