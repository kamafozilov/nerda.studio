//! Projects (folders on disk) and the agent sessions inside them, with persistence.

mod age;
pub mod browse;
mod error;
mod git;
mod history;
mod model;
mod paths;
mod query;
mod store;
mod tabs;

use std::path::Path;

pub use age::{age_changes_in, compact_age};
pub use error::{AddProjectError, LoadError};
pub use git::{Head, head};
pub use history::Visits;
pub use model::{
    AddedProject, Page, Project, ProjectId, ProjectScope, Session, SessionId, SessionSort, Tab,
    Timestamp,
};
pub use store::{default_data_dir, default_instance_lock_path, default_state_path, write_state};

/// Title given to every freshly created session.
const NEW_SESSION_TITLE: &str = "New session";

/// All projects and sessions plus the sidebar's sort, scope and selection, and the
/// sessions and pages open as tabs.
#[derive(Clone, Debug, Default)]
pub struct Workspace {
    /// Next id to hand out; shared by projects and sessions and only ever increased.
    next_id: u64,
    projects: Vec<Project>,
    sessions: Vec<Session>,
    sort: SessionSort,
    scope: ProjectScope,
    /// The tab on screen; always one of `tabs`.
    selected: Option<Tab>,
    /// The project whose sessions' tabs show: the selected session's, kept
    /// while a page is on screen.
    project_on_screen: Option<ProjectId>,
    /// Open tabs, in order. Every project's sessions share this list with the
    /// pages, which show beside whichever project is on screen.
    tabs: Vec<Tab>,
}

impl Workspace {
    /// Adds the folder at `path`, or returns the project that already points at the same folder.
    pub fn add_project(
        &mut self,
        path: &Path,
        now: Timestamp,
    ) -> Result<AddedProject, AddProjectError> {
        let path = paths::resolve_project_dir(path)?;
        if let Some(existing) = self.projects.iter().find(|project| project.path == path) {
            return Ok(AddedProject::Existing(existing.id));
        }
        let id = ProjectId(self.allocate_id());
        let title = paths::project_title(&path);
        self.projects.push(Project {
            id,
            title,
            path,
            created_at: now,
        });
        Ok(AddedProject::Added(id))
    }

    /// Returns the project's unused draft session, or creates one; `None` if the project is unknown.
    ///
    /// Returns `Option` so a stale id cannot produce a session that belongs to nothing.
    pub fn new_session(&mut self, project: ProjectId, now: Timestamp) -> Option<SessionId> {
        self.project(project)?;
        if let Some(draft) = self
            .sessions
            .iter()
            .find(|session| session.project == project && session.is_empty())
        {
            return Some(draft.id);
        }
        let id = SessionId(self.allocate_id());
        self.sessions.push(Session {
            id,
            project,
            title: NEW_SESSION_TITLE.to_owned(),
            created_at: now,
            last_activity_at: None,
        });
        Some(id)
    }

    /// Whether any project exists; cheap enough to call on every render.
    pub fn has_projects(&self) -> bool {
        !self.projects.is_empty()
    }

    /// Every project's id, in the order they were added.
    pub fn project_ids(&self) -> Vec<ProjectId> {
        self.projects.iter().map(|project| project.id).collect()
    }

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn sort(&self) -> SessionSort {
        self.sort
    }

    pub fn set_sort(&mut self, sort: SessionSort) {
        self.sort = sort;
    }

    pub fn scope(&self) -> ProjectScope {
        self.scope
    }

    /// Scopes the session list; a scope naming an unknown project becomes `All`.
    pub fn set_scope(&mut self, scope: ProjectScope) {
        self.scope = match scope {
            ProjectScope::Project(id) if self.project(id).is_none() => ProjectScope::All,
            scope => scope,
        };
    }

    /// The tab on screen.
    pub fn selected_tab(&self) -> Option<Tab> {
        self.selected
    }

    /// The session on screen; `None` while a page is.
    pub fn selected_session(&self) -> Option<SessionId> {
        self.selected.and_then(Tab::session)
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
