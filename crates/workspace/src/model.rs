use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::AgentChoice;

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

/// Where sessions run: a folder on disk that sessions run in, or the
/// computer itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    /// Canonical absolute path, used for duplicate detection; the home folder
    /// for the computer.
    pub path: PathBuf,
    pub created_at: Timestamp,
    /// Files from before the computer could be a project hold none, and a
    /// folder is written without one, so its entry reads the same as before.
    #[serde(default, skip_serializing_if = "ProjectKind::is_folder")]
    pub kind: ProjectKind,
}

/// What the computer is called as a project.
pub const NO_PROJECT: &str = "No project";

/// What a project is. The computer, "No project" on screen, is at most one
/// project: sessions in it work on the computer itself, from the home folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    #[default]
    Folder,
    Computer,
}

impl ProjectKind {
    pub fn is_folder(&self) -> bool {
        *self == ProjectKind::Folder
    }
}

/// The agent's own id for a session's conversation, by which a later process
/// resumes it. Kept as written, so one this build can't read survives loading
/// and saving.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(pub String);

impl ConversationId {
    /// A new id; the agent takes a UUID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
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
    /// What runs the session; files from before there was a choice hold none.
    #[serde(default)]
    pub choice: AgentChoice,
    /// The agent's conversation, once a process has started on it; `None`
    /// before, and in files from before sessions ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation: Option<ConversationId>,
    /// When the session was archived; `None` while it is listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<Timestamp>,
}

impl Session {
    /// True while no message has been sent, so the session is still a reusable draft.
    pub fn is_empty(&self) -> bool {
        self.last_activity_at.is_none()
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
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
    Usage,
    PullRequests,
    Settings,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::Notes,
        Page::Design,
        Page::Routines,
        Page::Usage,
        Page::PullRequests,
        Page::Settings,
    ];

    /// The pages the sidebar lists under its search field, in order; the rest
    /// open from its footer.
    pub const LISTED: [Page; 3] = [Page::Notes, Page::Design, Page::Routines];

    pub fn title(self) -> &'static str {
        match self {
            Page::Notes => "Notes",
            Page::Design => "Design",
            Page::Routines => "Routines",
            Page::Usage => "Usage",
            Page::PullRequests => "Pull requests",
            Page::Settings => "Settings",
        }
    }

    /// The name the saved workspace knows the page by; never changes once shipped.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Page::Notes => "notes",
            Page::Design => "design",
            Page::Routines => "routines",
            Page::Usage => "usage",
            Page::PullRequests => "pull-requests",
            Page::Settings => "settings",
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

    pub fn page(self) -> Option<Page> {
        match self {
            Tab::Page(page) => Some(page),
            Tab::Session(_) => None,
        }
    }
}
