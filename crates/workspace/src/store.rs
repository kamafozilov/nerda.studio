use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    LoadError, Project, ProjectId, ProjectScope, Session, SessionId, SessionSort, SessionView,
    Workspace,
};

/// Format version written by this build; files with any other version are refused.
const VERSION: u32 = 1;

/// Distinguishes temp files of concurrent saves within one process.
static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Largest id a file may hold: integers stay exact in every JSON reader, and the counter cannot overflow.
const MAX_ID: u64 = 1 << 53;

/// The folder the app keeps its data in, or `None` if the platform has none.
pub fn default_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("Nerda Studio"))
}

/// Where the app keeps its workspace, or `None` if the platform has no data folder.
pub fn default_state_path() -> Option<PathBuf> {
    default_data_dir().map(|dir| dir.join("workspace.json"))
}

/// Locked by the one running instance of the app, the only one that saves.
pub fn default_instance_lock_path() -> Option<PathBuf> {
    default_data_dir().map(|dir| dir.join("instance.lock"))
}

/// Read first so a newer format is reported as such instead of as corruption.
#[derive(Deserialize)]
struct Header {
    version: u32,
}

#[derive(Serialize)]
struct FileRef<'a> {
    version: u32,
    next_id: u64,
    projects: &'a [Project],
    sessions: &'a [Session],
    view: SessionView,
    sort: SessionSort,
    scope: ProjectScope,
    selected_session: Option<SessionId>,
    tabs: &'a [SessionId],
    collapsed: &'a BTreeSet<ProjectId>,
}

#[derive(Deserialize)]
struct FileV1 {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    sessions: Vec<Session>,
    /// Files written before the view could change have none.
    #[serde(default)]
    view: SessionView,
    #[serde(default)]
    sort: SessionSort,
    #[serde(default)]
    scope: ProjectScope,
    #[serde(default)]
    selected_session: Option<SessionId>,
    /// Files written before sessions opened as tabs have none.
    #[serde(default)]
    tabs: Vec<SessionId>,
    /// Files written before projects could fold have none.
    #[serde(default)]
    collapsed: Vec<ProjectId>,
}

impl Workspace {
    /// Reads the workspace at `path`; a missing file is an empty workspace. Repairs dangling references.
    pub fn load(path: &Path) -> Result<Workspace, LoadError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Workspace::default());
            }
            Err(error) => return Err(LoadError::Io(error)),
        };
        let header: Header = serde_json::from_slice(&bytes).map_err(LoadError::Corrupt)?;
        if header.version != VERSION {
            return Err(LoadError::UnsupportedVersion(header.version));
        }
        let file: FileV1 = serde_json::from_slice(&bytes).map_err(LoadError::Corrupt)?;
        Workspace::from_file(file)
    }

    /// Writes the workspace to `path` atomically, creating missing parent folders.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_state(path, &self.encode()?)
    }

    /// The workspace as the file holds it, for [`write_state`] to write
    /// elsewhere, such as off the UI thread.
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let file = FileRef {
            version: VERSION,
            next_id: self.next_id,
            projects: &self.projects,
            sessions: &self.sessions,
            view: self.view,
            sort: self.sort,
            scope: self.scope,
            selected_session: self.selected_session,
            tabs: &self.tabs,
            collapsed: &self.collapsed,
        };
        serde_json::to_vec_pretty(&file).map_err(io::Error::other)
    }

    /// Rebuilds a workspace from disk, dropping duplicates and anything pointing at missing items.
    fn from_file(file: FileV1) -> Result<Workspace, LoadError> {
        let mut project_ids = HashSet::new();
        let projects: Vec<Project> = file
            .projects
            .into_iter()
            .filter(|project| project_ids.insert(project.id))
            .collect();
        let mut session_ids = HashSet::new();
        let sessions: Vec<Session> = file
            .sessions
            .into_iter()
            .filter(|session| {
                project_ids.contains(&session.project) && session_ids.insert(session.id)
            })
            .collect();
        let highest = project_ids
            .iter()
            .map(|id| id.0)
            .chain(session_ids.iter().map(|id| id.0))
            .max();
        let next_id = file
            .next_id
            .max(highest.map_or(0, |id| id.saturating_add(1)));
        if next_id > MAX_ID {
            return Err(LoadError::Corrupt(serde::de::Error::custom(
                "ids exceed the id range",
            )));
        }
        let mut workspace = Workspace {
            next_id,
            projects,
            sessions,
            view: file.view,
            sort: file.sort,
            ..Workspace::default()
        };
        workspace.set_scope(file.scope);
        for tab in file.tabs {
            if workspace.session(tab).is_some() && !workspace.tabs.contains(&tab) {
                workspace.tabs.push(tab);
            }
        }
        // An older file's selection becomes its one tab.
        if let Some(selected) = file.selected_session {
            workspace.open_session(selected);
        }
        for project in file.collapsed {
            workspace.set_collapsed(project, true);
        }
        Ok(workspace)
    }
}
/// Writes `bytes` from [`Workspace::encode`] to `path` atomically, creating
/// missing parent folders: a crash mid-write leaves the old file whole.
pub fn write_state(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new(""));
    fs::create_dir_all(parent)?;
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the state path has no file name",
        )
    })?;
    let serial = SAVE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(name);
    temp_name.push(format!(".{}.{serial}.tmp", std::process::id()));
    let temp = parent.join(temp_name);
    let written = write_synced(&temp, bytes).and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

/// Writes `bytes` to a new file and flushes it to disk before it can replace the real one.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
