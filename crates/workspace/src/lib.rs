//! Projects (folders on disk) and the agent sessions inside them, with persistence.

mod age;
pub mod browse;
mod error;
mod history;
mod model;
mod paths;
mod query;
mod store;
mod tabs;

use std::{collections::BTreeSet, path::Path};

pub use age::compact_age;
pub use error::{AddProjectError, LoadError};
pub use history::Visits;
pub use model::{
    AddedProject, Project, ProjectId, ProjectScope, Session, SessionId, SessionSort, SessionView,
    Timestamp,
};
pub use query::SessionGroup;
pub use store::{default_data_dir, default_instance_lock_path, default_state_path, write_state};

/// Title given to every freshly created session.
const NEW_SESSION_TITLE: &str = "New session";

/// All projects and sessions plus the sidebar's view, sort, scope and selection,
/// and the sessions open as tabs.
#[derive(Clone, Debug, Default)]
pub struct Workspace {
    /// Next id to hand out; shared by projects and sessions and only ever increased.
    next_id: u64,
    projects: Vec<Project>,
    sessions: Vec<Session>,
    view: SessionView,
    sort: SessionSort,
    scope: ProjectScope,
    /// The session on screen; always one of `tabs`.
    selected_session: Option<SessionId>,
    /// Sessions open as tabs, in order. Every project's tabs share this list, and
    /// the selected session's project is the one whose tabs show.
    tabs: Vec<SessionId>,
    /// Projects folded to their header in the session list.
    collapsed: BTreeSet<ProjectId>,
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

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn view(&self) -> SessionView {
        self.view
    }

    pub fn set_view(&mut self, view: SessionView) {
        self.view = view;
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

    pub fn selected_session(&self) -> Option<SessionId> {
        self.selected_session
    }

    /// Whether the project is folded to its header in the session list.
    pub fn is_collapsed(&self, project: ProjectId) -> bool {
        self.collapsed.contains(&project)
    }

    /// Folds or unfolds a project in the session list; an unknown project is ignored.
    pub fn set_collapsed(&mut self, project: ProjectId, collapsed: bool) {
        if !collapsed {
            self.collapsed.remove(&project);
        } else if self.project(project).is_some() {
            self.collapsed.insert(project);
        }
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
