// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use agent::protocol::{INITIALIZE, parse};
use agent::{
    Command, Conversation, ConversationId, Event, Events, Launch, Message, Status, Transcript,
};
use serde_json::Value;
use workspace::{AgentChoice, ModelId};

/// Every recorded fixture: the wire of real `claude` runs, trimmed and with
/// private paths replaced. Agent lines are what it wrote on stdout; a
/// `{"host": …}` line stands where the app wrote to its stdin.
pub const FIXTURES: &[&str] = &[
    "initialize.jsonl",
    "turn-ok.jsonl",
    "turn-permission.jsonl",
    "settings.jsonl",
    "turn-image.jsonl",
    "turn-interrupt.jsonl",
    "turn-after-interrupt.jsonl",
    "turn-model-not-found.jsonl",
    "turn-signed-out.jsonl",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// The app spoke here: `user`, `answer`, `initialize`, `interrupt`, …
    Host(String),
    /// A line the agent wrote, `{{uuid}}` and `{{request}}` standing for the
    /// ids of the app's message and control request it answers.
    Agent(String),
}

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn fixture(name: &str) -> Vec<Line> {
    let text = std::fs::read_to_string(fixture_path(name)).unwrap();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            match value.get("host").and_then(|host| host.as_str()) {
                Some(host) => Line::Host(host.to_owned()),
                None => Line::Agent(line.to_owned()),
            }
        })
        .collect()
}

pub fn agent_lines(name: &str) -> Vec<String> {
    fixture(name)
        .into_iter()
        .filter_map(|line| match line {
            Line::Agent(line) => Some(line),
            Line::Host(_) => None,
        })
        .collect()
}

/// Plays fixtures against a transcript as a session would deliver them. A
/// host line takes the app's next command: a message sent ahead, which the
/// agent reads in order, or else the command `host` gives. An agent line is
/// received with the ids of the message and request it answers filled in.
pub struct Player {
    pub transcript: Transcript,
    /// Messages sent that no host line has taken up yet.
    inbox: VecDeque<String>,
    uuid: String,
    request: String,
    requests: u32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            transcript: Transcript::new(),
            inbox: VecDeque::new(),
            uuid: String::new(),
            request: String::new(),
            requests: 0,
        }
    }

    pub fn send(&mut self, command: Command) {
        match &command {
            Command::Send(message) => self.inbox.push_back(message.id.0.clone()),
            Command::Answer { .. } => {}
            _ => {
                self.requests += 1;
                self.request = format!("host_{}", self.requests);
            }
        }
        self.transcript.apply(&Event::Sent(command));
    }

    pub fn receive(&mut self, line: &str) {
        let line = line
            .replace("{{uuid}}", &self.uuid)
            .replace("{{request}}", &self.request);
        let incoming = parse(&line).unwrap_or_else(|error| panic!("{error}: {line}"));
        self.transcript.apply(&Event::Received(incoming));
    }

    pub fn step(&mut self, line: Line, host: &mut impl FnMut(&str, &Transcript) -> Command) {
        match line {
            Line::Host(kind) if kind == "initialize" => {
                self.request = INITIALIZE.to_owned();
                self.transcript.apply(&Event::Started);
            }
            Line::Host(kind) => {
                if kind != "user" || self.inbox.is_empty() {
                    let command = host(&kind, &self.transcript);
                    self.send(command);
                }
                if kind == "user" {
                    self.uuid = self.inbox.pop_front().expect("a message for the agent");
                }
            }
            Line::Agent(line) => self.receive(&line),
        }
    }

    pub fn play(&mut self, fixture: &str, mut host: impl FnMut(&str, &Transcript) -> Command) {
        for line in self::fixture(fixture) {
            self.step(line, &mut host);
        }
    }

    /// Plays `fixture` up to, not including, the first line `stop` picks.
    pub fn play_until(
        &mut self,
        fixture: &str,
        mut host: impl FnMut(&str, &Transcript) -> Command,
        stop: impl Fn(&Line) -> bool,
    ) -> Vec<Line> {
        let mut lines = self::fixture(fixture).into_iter();
        for line in lines.by_ref() {
            if stop(&line) {
                return std::iter::once(line).chain(lines).collect();
            }
            self.step(line, &mut host);
        }
        Vec::new()
    }
}

/// A host that answers each user line with a new message saying `text`.
pub fn says(text: &str) -> impl FnMut(&str, &Transcript) -> Command + '_ {
    move |kind, _| match kind {
        "user" => Command::Send(Message::new(text, Vec::new())),
        other => panic!("the host was asked for {other}"),
    }
}

/// A project folder whose fake agent plays `steps`: fixtures by file name,
/// and any other step as a line of the script.
pub fn project(steps: &[&str]) -> TempDir {
    let dir = TempDir::new();
    let mut script = String::new();
    for step in steps {
        if step.ends_with(".jsonl") {
            script.push_str(&std::fs::read_to_string(fixture_path(step)).unwrap());
        } else {
            script.push_str(step);
            script.push('\n');
        }
    }
    std::fs::write(dir.path().join("fake-agent.jsonl"), script).unwrap();
    dir
}

/// The fixture's lines as script steps, for splicing.
pub fn script_lines(fixture: &str) -> Vec<String> {
    let text = std::fs::read_to_string(fixture_path(fixture)).unwrap();
    text.lines().map(str::to_owned).collect()
}

/// Runs the fake agent in `dir` on haiku, asking first, in a new conversation.
pub fn fake(dir: &TempDir) -> Launch {
    Launch {
        folder: dir.path().to_owned(),
        choice: AgentChoice {
            model: ModelId("haiku".to_owned()),
            ..AgentChoice::default()
        },
        conversation: Conversation::New(ConversationId::new()),
        program: Some(PathBuf::from(env!("CARGO_BIN_EXE_fake-agent"))),
    }
}

/// Each line the fake agent read from the app.
pub fn written(dir: &TempDir) -> Vec<Value> {
    let log = std::fs::read_to_string(dir.path().join("fake-agent.stdin.jsonl")).unwrap();
    log.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The arguments the fake agent was started with.
pub fn arguments(dir: &TempDir) -> Vec<String> {
    let text = std::fs::read_to_string(dir.path().join("fake-agent.args")).unwrap();
    text.lines().map(str::to_owned).collect()
}

pub fn status_is(status: Status) -> impl Fn(&Transcript, &[Event]) -> bool {
    move |transcript, _| transcript.status() == &status
}

pub fn exited(transcript: &Transcript, _: &[Event]) -> bool {
    matches!(transcript.status(), Status::Exited { .. })
}

/// Drains a session's events into a transcript, as the screen would.
pub struct Watch {
    events: Events,
    pub transcript: Transcript,
    pub seen: Vec<Event>,
}

impl Watch {
    pub fn new(events: Events) -> Self {
        Self {
            events,
            transcript: Transcript::new(),
            seen: Vec::new(),
        }
    }

    /// Applies the events that have come so far.
    pub fn drain(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.transcript.apply(&event);
            self.seen.push(event);
        }
    }

    /// Applies events until `done` holds; fails after ten seconds.
    pub fn until(&mut self, what: &str, done: impl Fn(&Transcript, &[Event]) -> bool) {
        self.until_within(Duration::from_secs(10), what, done);
    }

    pub fn until_within(
        &mut self,
        limit: Duration,
        what: &str,
        done: impl Fn(&Transcript, &[Event]) -> bool,
    ) {
        let deadline = Instant::now() + limit;
        while !done(&self.transcript, &self.seen) {
            match self.events.try_recv() {
                Ok(event) => {
                    self.transcript.apply(&event);
                    self.seen.push(event);
                }
                Err(async_channel::TryRecvError::Empty) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!(
                    "{error:?} waiting for {what}, at {:?} after {:#?}",
                    self.transcript.status(),
                    self.seen
                ),
            }
        }
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A fresh folder under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!(
            "nerda-agent-test-{}-{}-{nanos}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
