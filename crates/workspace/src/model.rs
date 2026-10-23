use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Identifies a project; never reused within one workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectId(pub u64);

/// Identifies a session; never reused within one workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u64);

/// Milliseconds since the UNIX epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// The current wall-clock time; a clock set before 1970 reads as the epoch.
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        Self(millis)
    }
}

/// A folder on disk that sessions run in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    /// Canonical absolute path, used for duplicate detection.
    pub path: PathBuf,
    pub created_at: Timestamp,
}

/// What a session with a blank title is shown as.
const UNTITLED: &str = "Untitled session";

/// An agent session inside a project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub project: ProjectId,
    pub title: String,
    pub created_at: Timestamp,
    /// When the last message was sent; `None` until the first one.
    pub last_activity_at: Option<Timestamp>,
}

impl Session {
    /// True while no message has been sent, so the session is still a reusable draft.
    pub fn is_empty(&self) -> bool {
        self.last_activity_at.is_none()
    }

    /// The time the session was last touched, falling back to its creation.
    pub fn activity(&self) -> Timestamp {
        self.last_activity_at.unwrap_or(self.created_at)
    }

    /// The title as one line to show: runs of whitespace, line breaks and tabs
    /// included, read as one space and trimmed; a blank title reads
    /// "Untitled session".
    pub fn display_title(&self) -> String {
        let title = self.title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() {
            UNTITLED.to_owned()
        } else {
            title
        }
    }
}

/// How the session list is ordered; newest first in both cases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionSort {
    #[default]
    LastActivity,
    Created,
}

/// Which projects' sessions the session list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectScope {
    #[default]
    All,
    Project(ProjectId),
}

/// Outcome of adding a folder: a new project or the one already pointing at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddedProject {
    Added(ProjectId),
    Existing(ProjectId),
}

/// A part of the app beside the sessions, not tied to a project. There is one
/// of each: opening it again shows its tab instead of adding another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Notes,
    Design,
    Routines,
}

impl Page {
    /// Every page, in the order the sidebar lists them.
    pub const ALL: [Page; 3] = [Page::Notes, Page::Design, Page::Routines];

    pub fn title(self) -> &'static str {
        match self {
            Page::Notes => "Notes",
            Page::Design => "Design",
            Page::Routines => "Routines",
        }
    }

    /// The name the saved workspace knows the page by; never changes once shipped.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Page::Notes => "notes",
            Page::Design => "design",
            Page::Routines => "routines",
        }
    }

    /// The page saved under `key`; `None` for one this build doesn't know.
    pub(crate) fn from_key(key: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|page| page.key() == key)
    }
}

/// What a tab shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Session(SessionId),
    Page(Page),
}

impl Tab {
    /// A number no other tab shares, to name its elements by: a session's id,
    /// which a loaded workspace keeps at most 2^53, else one past every id.
    pub fn key(self) -> u64 {
        match self {
            Tab::Session(id) => id.0,
            Tab::Page(page) => u64::MAX - page as u64,
        }
    }

    pub fn session(self) -> Option<SessionId> {
        match self {
            Tab::Session(id) => Some(id),
            Tab::Page(_) => None,
        }
    }
}
