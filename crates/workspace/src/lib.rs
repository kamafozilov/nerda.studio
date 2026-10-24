//! Projects (folders on disk, or the computer itself) and the agent sessions
//! inside them, with persistence.

mod age;
mod agents;
mod archive;
mod backdrop;
mod branches;
pub mod browse;
mod error;
mod git;
mod history;
mod icon;
mod model;
mod paths;
mod query;
mod store;
mod tabs;

use std::path::Path;

pub use age::{age_changes_in, compact_age};
pub use agents::{
    AGENTS, Access, AccessMode, Agent, AgentChoice, AgentId, Effort, EffortLevel, Model, ModelId,
    agent,
};
pub use backdrop::{Backdrop, BackdropStrength, BackdropStyle, BackdropTint};
pub use branches::{
    Branch, Branches, RemoteBranch, Switch, Worktree, branch_name, branches, switch,
};
pub use error::{AddProjectError, LoadError};
pub use git::{Head, head};
pub use history::Visits;
pub use icon::project_icon;
pub use model::{
    AddedProject, ConversationId, NO_PROJECT, Page, Project, ProjectId, ProjectKind, ProjectScope,
    Session, SessionId, SessionSort, Tab, Timestamp,
};
pub use paths::home_relative;
pub use query::matches_search;
pub use store::{default_data_dir, default_instance_lock_path, default_state_path, write_state};

/// Title given to every freshly created session.
const NEW_SESSION_TITLE: &str = "New session";

/// Longest title a first message gives its session, in characters: the
/// sidebar and the tabs show far less, and the file needn't hold a whole
/// pasted page.
const TITLE_MAX_CHARS: usize = 200;

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
    /// The choice last made in any session, which new sessions start from.
    last_choice: AgentChoice,
    /// What stands behind a new session's composer.
    backdrop: Backdrop,
    /// The sidebar's width in points as last dragged; `None` keeps the
    /// app's default.
    sidebar_width: Option<f32>,
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
            kind: ProjectKind::Folder,
        });
        Ok(AddedProject::Added(id))
    }

    /// The computer as a project, created the first time it is asked for;
    /// `None` only when the platform names no home folder to run it in.
    pub fn computer_project(&mut self, now: Timestamp) -> Option<ProjectId> {
        if let Some(computer) = self.computer() {
            return Some(computer.id);
        }
        let path = paths::resolve_project_dir(&dirs::home_dir()?).ok()?;
        let id = ProjectId(self.allocate_id());
        self.projects.push(Project {
            id,
            title: NO_PROJECT.to_owned(),
            path,
            created_at: now,
            kind: ProjectKind::Computer,
        });
        Some(id)
    }

    /// The computer as a project, once a session has been started on it.
    pub fn computer(&self) -> Option<&Project> {
        (self.projects.iter()).find(|project| project.kind == ProjectKind::Computer)
    }

    /// Returns the project's unused draft session, or creates one; `None` if the project is unknown.
    /// Either way it starts from the last choice made.
    ///
    /// Returns `Option` so a stale id cannot produce a session that belongs to nothing.
    pub fn new_session(&mut self, project: ProjectId, now: Timestamp) -> Option<SessionId> {
        self.project(project)?;
        if let Some(draft) = self
            .sessions
            .iter_mut()
            .find(|session| session.project == project && session.is_empty())
        {
            draft.choice = self.last_choice.clone();
            return Some(draft.id);
        }
        let id = SessionId(self.allocate_id());
        self.sessions.push(Session {
            id,
            project,
            title: NEW_SESSION_TITLE.to_owned(),
            created_at: now,
            last_activity_at: None,
            choice: self.last_choice.clone(),
            conversation: None,
            archived_at: None,
        });
        Some(id)
    }

    /// The choice last made in any session, which new sessions start from.
    pub fn backdrop(&self) -> &Backdrop {
        &self.backdrop
    }

    pub fn set_backdrop(&mut self, backdrop: Backdrop) {
        self.backdrop = backdrop;
    }

    /// The sidebar's width as last dragged, unchecked: the app keeps it
    /// within its bounds.
    pub fn sidebar_width(&self) -> Option<f32> {
        self.sidebar_width
    }

    pub fn set_sidebar_width(&mut self, width: Option<f32>) {
        self.sidebar_width = width;
    }

    pub fn last_choice(&self) -> &AgentChoice {
        &self.last_choice
    }

    /// Sets what runs `session`, and what new sessions start from; does
    /// nothing for an unknown session.
    pub fn set_choice(&mut self, session: SessionId, choice: AgentChoice) {
        if let Some(session) = self.sessions.iter_mut().find(|item| item.id == session) {
            session.choice = choice.clone();
            self.last_choice = choice;
        }
    }

    /// Records a message sent in `session` at `now`, so the session has seen
    /// work: it joins the list and the projects' order. The first message
    /// titles a session still called what it was created as, or blank, by its
    /// first line that isn't blank. An archived session comes back to the
    /// list. Does nothing for an unknown session.
    pub fn record_message(&mut self, session: SessionId, text: &str, now: Timestamp) {
        let Some(session) = self.sessions.iter_mut().find(|item| item.id == session) else {
            return;
        };
        let untitled = session.title.trim().is_empty() || session.title == NEW_SESSION_TITLE;
        let first_line = text.lines().map(str::trim).find(|line| !line.is_empty());
        if let (true, true, Some(line)) = (session.is_empty(), untitled, first_line) {
            session.title = line.chars().take(TITLE_MAX_CHARS).collect();
        }
        session.last_activity_at = Some(now);
        session.archived_at = None;
    }

    /// Records the agent's conversation `session` runs, which a later
    /// process resumes; does nothing for an unknown session.
    pub fn set_conversation(&mut self, session: SessionId, conversation: ConversationId) {
        if let Some(session) = self.sessions.iter_mut().find(|item| item.id == session) {
            session.conversation = Some(conversation);
        }
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
