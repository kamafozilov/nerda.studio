use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    AgentChoice, Backdrop, LoadError, NO_PROJECT, Page, Preferences, Project, ProjectId,
    ProjectKind, ProjectScope, Session, SessionId, SessionSort, Tab, Workspace, paths,
};

/// Format version written by this build; files with any other version are refused.
const VERSION: u32 = 1;

/// Distinguishes temp files of concurrent saves within one process.
static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Largest id a file may hold: integers stay exact in every JSON reader, and the counter cannot overflow.
const MAX_ID: u64 = 1 << 53;

/// Names another data folder: a build tried out beside the owner's keeps its
/// own data, lock and window instead of taking over the owner's.
const DATA_DIR: &str = "NERDA_DATA_DIR";

/// The folder the app keeps its data in, or `None` if the platform has none.
pub fn default_data_dir() -> Option<PathBuf> {
    match std::env::var_os(DATA_DIR) {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => dirs::data_dir().map(|dir| dir.join("Nerda Studio")),
    }
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
    sort: SessionSort,
    scope: ProjectScope,
    selected_session: Option<SessionId>,
    selected_page: Option<&'static str>,
    project_on_screen: Option<ProjectId>,
    /// The sessions' tabs in order, without the pages, so a build from before
    /// pages still reads them.
    tabs: Vec<SessionId>,
    pages: Vec<PageTabRef>,
    last_choice: &'a AgentChoice,
    backdrop: &'a Backdrop,
    #[serde(skip_serializing_if = "Option::is_none")]
    sidebar_width: Option<f32>,
    preferences: &'a Preferences,
}

/// A page's tab and its index among all tabs.
#[derive(Serialize)]
struct PageTabRef {
    page: &'static str,
    at: usize,
}

/// Keys this build no longer reads, such as `view` and `collapsed` from when the
/// list could group by project, are ignored, so those files still load.
#[derive(Deserialize)]
struct FileV1 {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    sessions: Vec<Session>,
    #[serde(default)]
    sort: SessionSort,
    #[serde(default)]
    scope: ProjectScope,
    #[serde(default)]
    selected_session: Option<SessionId>,
    /// Pages are named, so one a newer build added is skipped, not corrupt.
    #[serde(default)]
    selected_page: Option<String>,
    #[serde(default)]
    project_on_screen: Option<ProjectId>,
    /// Files written before sessions opened as tabs have none.
    #[serde(default)]
    tabs: Vec<SessionId>,
    #[serde(default)]
    pages: Vec<PageTab>,
    /// Files written before sessions had a choice have none.
    #[serde(default)]
    last_choice: AgentChoice,
    /// Files written before there was a backdrop have none.
    #[serde(default)]
    backdrop: Backdrop,
    /// Files written before the sidebar could be resized have none.
    #[serde(default)]
    sidebar_width: Option<f32>,
    /// Files written before the Settings page have none.
    #[serde(default)]
    preferences: Preferences,
}

#[derive(Deserialize)]
struct PageTab {
    page: String,
    at: usize,
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
            sort: self.sort,
            scope: self.scope,
            selected_session: self.selected_session(),
            selected_page: match self.selected {
                Some(Tab::Page(page)) => Some(page.key()),
                _ => None,
            },
            project_on_screen: self.project_on_screen,
            tabs: self.tabs.iter().filter_map(|tab| tab.session()).collect(),
            pages: (self.tabs.iter().enumerate())
                .filter_map(|(at, tab)| match tab {
                    Tab::Page(page) => Some(PageTabRef {
                        page: page.key(),
                        at,
                    }),
                    Tab::Session(_) => None,
                })
                .collect(),
            last_choice: &self.last_choice,
            backdrop: &self.backdrop,
            sidebar_width: self.sidebar_width,
            preferences: &self.preferences,
        };
        serde_json::to_vec_pretty(&file).map_err(io::Error::other)
    }

    /// Rebuilds a workspace from disk, dropping duplicates and anything pointing at missing items.
    fn from_file(file: FileV1) -> Result<Workspace, LoadError> {
        let mut project_ids = HashSet::new();
        let mut projects: Vec<Project> = file
            .projects
            .into_iter()
            .filter(|project| project_ids.insert(project.id))
            .collect();
        let folded = fold_home_into_computer(&mut projects);
        let moved = |id: ProjectId| -> ProjectId {
            (folded.iter())
                .find(|&&(from, _)| from == id)
                .map_or(id, |&(_, to)| to)
        };
        let mut session_ids = HashSet::new();
        let sessions: Vec<Session> = file
            .sessions
            .into_iter()
            .map(|mut session| {
                session.project = moved(session.project);
                session
            })
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
            sort: file.sort,
            last_choice: file.last_choice,
            backdrop: file.backdrop,
            sidebar_width: file.sidebar_width,
            preferences: file.preferences,
            ..Workspace::default()
        };
        workspace.set_scope(match file.scope {
            ProjectScope::Project(id) => ProjectScope::Project(moved(id)),
            ProjectScope::All => ProjectScope::All,
        });
        for id in file.tabs {
            let tab = Tab::Session(id);
            if workspace.session(id).is_some() && !workspace.tabs.contains(&tab) {
                workspace.tabs.push(tab);
            }
        }
        // Inserted by index, lowest first, each page lands where it was.
        let mut pages = file.pages;
        pages.sort_by_key(|page| page.at);
        for PageTab { page, at } in pages {
            let Some(tab) = Page::from_key(&page).map(Tab::Page) else {
                continue;
            };
            if !workspace.tabs.contains(&tab) {
                let at = at.min(workspace.tabs.len());
                workspace.tabs.insert(at, tab);
            }
        }
        workspace.project_on_screen = (file.project_on_screen.map(moved))
            .filter(|&project| workspace.project(project).is_some());
        // An older file's selection becomes its one tab.
        if let Some(selected) = file.selected_session {
            workspace.open_session(selected);
        } else if let Some(page) = file.selected_page.as_deref().and_then(Page::from_key) {
            workspace.open_page(page);
        }
        Ok(workspace)
    }
}
/// Makes the home folder's entries one computer. An older build, which knows
/// no kinds, writes the computer back as a folder at the home path, and one
/// could add the home folder as a folder; either would show beside the
/// computer as a second No project. The computer kept is the first one, else
/// the first entry at home; every other is dropped, and the pairs returned
/// name each dropped id with the kept one, for what pointed at it to follow.
fn fold_home_into_computer(projects: &mut Vec<Project>) -> Vec<(ProjectId, ProjectId)> {
    let home = paths::home_project_dir();
    let at_home = |project: &Project| {
        project.kind == ProjectKind::Computer || Some(&project.path) == home.as_ref()
    };
    let kept = (projects.iter())
        .position(|project| project.kind == ProjectKind::Computer)
        .or_else(|| projects.iter().position(at_home));
    let Some(kept) = kept else {
        return Vec::new();
    };
    let computer = projects[kept].id;
    projects[kept].kind = ProjectKind::Computer;
    projects[kept].title = NO_PROJECT.to_owned();
    let mut folded = Vec::new();
    projects.retain(|project| {
        if project.id == computer || !at_home(project) {
            return true;
        }
        folded.push((project.id, computer));
        false
    });
    folded
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
