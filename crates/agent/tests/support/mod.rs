// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use agent::protocol::{INITIALIZE, parse};
use agent::{Command, Event, Message, Transcript};

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
