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
}

/// How the session list is ordered; newest first in both cases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionSort {
    #[default]
    LastActivity,
    Created,
}

/// How the session list is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionView {
    /// Each project a header over its own sessions.
    #[default]
    Projects,
    /// One list of every session, each naming its project.
    Sessions,
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
