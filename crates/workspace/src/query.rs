use std::{cmp::Reverse, path::Component};

use crate::{Project, ProjectId, ProjectScope, Session, SessionSort, Timestamp, Workspace};

impl Workspace {
    /// Projects, most recently active first; ties by title ignoring case, then by id.
    pub fn projects(&self) -> Vec<&Project> {
        let mut projects: Vec<(Timestamp, String, &Project)> = self
            .projects
            .iter()
            .map(|project| {
                (
                    self.project_activity(project),
                    project.title.to_lowercase(),
                    project,
                )
            })
            .collect();
        projects.sort_by(|(a_time, a_title, a), (b_time, b_title, b)| {
            b_time
                .cmp(a_time)
                .then_with(|| a_title.cmp(b_title))
                .then_with(|| a.id.cmp(&b.id))
        });
        projects
            .into_iter()
            .map(|(_, _, project)| project)
            .collect()
    }

    /// Sessions in the current scope that have seen work and whose title, as
    /// shown, contains the query, in sort order. A draft stays out until its
    /// first message: an open tab alone is not work. Matching ignores case, runs of whitespace and the
    /// spellings people type for each other: е for ё, and a plain apostrophe
    /// for the Uzbek ʻ and ʼ.
    pub fn visible_sessions(&self, query: &str) -> Vec<&Session> {
        let needle = fold(&query.split_whitespace().collect::<Vec<_>>().join(" "));
        let mut sessions: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|session| !session.is_empty())
            .filter(|session| match self.scope {
                ProjectScope::All => true,
                ProjectScope::Project(id) => session.project == id,
            })
            .filter(|session| needle.is_empty() || fold(&session.display_title()).contains(&needle))
            .collect();
        sessions.sort_by_key(|session| Reverse((self.sort_key(session), session.id)));
        sessions
    }

    /// The project a new session should go to: the one on screen, the scoped one, else the most recently active.
    pub fn context_project(&self) -> Option<ProjectId> {
        if let Some(project) = self.project_on_screen {
            return Some(project);
        }
        if let ProjectScope::Project(id) = self.scope {
            return Some(id);
        }
        self.projects().first().map(|project| project.id)
    }

    /// The project's most recently active session, ties by newest id; `None` for an unknown or empty project.
    pub fn latest_session(&self, project: ProjectId) -> Option<&Session> {
        self.sessions
            .iter()
            .filter(|session| session.project == project)
            .max_by_key(|session| (session.activity(), session.id))
    }

    /// The project's name as the UI shows it: its title, or, when another project
    /// has the same title, the shortest end of its path that no such project
    /// shares ("site/web" beside "app/web"). `None` for an unknown project.
    pub fn display_name(&self, id: ProjectId) -> Option<String> {
        let project = self.project(id)?;
        let twins: Vec<Vec<Component>> = self
            .projects
            .iter()
            .filter(|other| other.id != id && other.title == project.title)
            .map(|other| other.path.components().collect())
            .collect();
        if twins.is_empty() {
            return Some(project.title.clone());
        }
        let own: Vec<Component> = project.path.components().collect();
        let unique_from = (1..=own.len())
            .find(|&count| {
                let tail = &own[own.len() - count..];
                twins.iter().all(|twin| !twin.ends_with(tail))
            })
            .unwrap_or(own.len());
        let tail: std::path::PathBuf = own[own.len() - unique_from..].iter().collect();
        Some(tail.display().to_string())
    }

    /// The latest activity among the project's sessions, or its creation time
    /// when none has any; opening a draft is not activity.
    fn project_activity(&self, project: &Project) -> Timestamp {
        self.sessions
            .iter()
            .filter(|session| session.project == project.id)
            .filter_map(|session| session.last_activity_at)
            .max()
            .unwrap_or(project.created_at)
    }

    /// The time the sort orders a session by.
    fn sort_key(&self, session: &Session) -> Timestamp {
        match self.sort {
            SessionSort::LastActivity => session.activity(),
            SessionSort::Created => session.created_at,
        }
    }
}

/// Text as search compares it: lowercase, ё as е, and every apostrophe-like
/// mark (ʻ ʼ ‘ ’ `) as a plain apostrophe.
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'ё' => 'е',
            'ʻ' | 'ʼ' | '‘' | '’' | '`' => '\'',
            c => c,
        })
        .collect()
}
