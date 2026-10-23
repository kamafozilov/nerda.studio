// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
