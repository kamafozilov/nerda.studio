//! Sessions open as tabs. Each project keeps its own row of tabs; the selected
//! session's project is the one on screen. Opening a session gives it a tab
//! unless it has one; closing the selected tab selects its right neighbor, else
//! its left, and closing a project's last tab opens the project's draft, so a
//! project on screen always shows a session.
//!

use std::collections::HashSet;

use crate::{ProjectId, SessionId, Timestamp, Workspace};

impl Workspace {
    /// Every open tab, in order, across all projects.
    pub fn tabs(&self) -> &[SessionId] {
        &self.tabs
    }

    /// The tabs on screen: the selected session's project's, in order.
    pub fn open_tabs(&self) -> Vec<SessionId> {
        self.selected_session
            .and_then(|id| self.session(id))
            .map(|session| self.tabs_of(session.project))
            .unwrap_or_default()
    }

    /// Shows a session: opens it as the last tab unless it has one, and selects it.
    /// An unknown id changes nothing.
    pub fn open_session(&mut self, id: SessionId) {
        if self.session(id).is_none() {
            return;
        }
        if !self.tabs.contains(&id) {
            self.tabs.push(id);
        }
        self.selected_session = Some(id);
    }

    /// With projects but no session on screen, opens the context project's
    /// latest tab, else its draft. Returns whether anything changed.
    pub fn ensure_open_session(&mut self, now: Timestamp) -> bool {
        if self.selected_session.is_some() {
            return false;
        }
        let Some(project) = self.context_project() else {
            return false;
        };
        let session = self
            .tabs_of(project)
            .last()
            .copied()
            .or_else(|| self.new_session(project, now));
        session.inspect(|&id| self.open_session(id)).is_some()
    }

    pub fn close_tab(&mut self, id: SessionId, now: Timestamp) {
        self.close_tabs(&[id], None, now);
    }

    /// Closes every tab on screen but `id`, and selects it.
    pub fn close_other_tabs(&mut self, id: SessionId, now: Timestamp) {
        let others: Vec<SessionId> = self
            .open_tabs()
            .into_iter()
            .filter(|&tab| tab != id)
            .collect();
        self.close_tabs(&others, Some(id), now);
    }

    /// Closes the tabs on screen after `id`; `id` is selected if the selection closes.
    pub fn close_tabs_after(&mut self, id: SessionId, now: Timestamp) {
        let tabs = self.open_tabs();
        if let Some(index) = tabs.iter().position(|&tab| tab == id) {
            self.close_tabs(&tabs[index + 1..], Some(id), now);
        }
    }

    /// Closes the tabs on screen before `id`; `id` is selected if the selection closes.
    pub fn close_tabs_before(&mut self, id: SessionId, now: Timestamp) {
        let tabs = self.open_tabs();
        if let Some(index) = tabs.iter().position(|&tab| tab == id) {
            self.close_tabs(&tabs[..index], Some(id), now);
        }
    }

    /// Closes every tab on screen; the project's draft opens in their place.
    pub fn close_all_tabs(&mut self, now: Timestamp) {
        let tabs = self.open_tabs();
        self.close_tabs(&tabs, None, now);
    }

    /// Moves a tab on screen to `to`, an index among the tabs on screen; other
    /// projects' tabs keep their places.
    pub fn move_tab(&mut self, id: SessionId, to: usize) {
        let mut tabs = self.open_tabs();
        let Some(from) = tabs.iter().position(|&tab| tab == id) else {
            return;
        };
        tabs.remove(from);
        tabs.insert(to.min(tabs.len()), id);
        let shown: HashSet<SessionId> = tabs.iter().copied().collect();
        let mut reordered = tabs.into_iter();
        for slot in self.tabs.iter_mut().filter(|tab| shown.contains(tab)) {
            if let Some(tab) = reordered.next() {
                *slot = tab;
            }
        }
    }

    /// The tab `step` places from the selected one, wrapping at either end.
    pub fn adjacent_tab(&self, step: isize) -> Option<SessionId> {
        let tabs = self.open_tabs();
        let current = tabs
            .iter()
            .position(|&tab| Some(tab) == self.selected_session)?;
        let count = tabs.len() as isize;
        Some(tabs[(current as isize + step).rem_euclid(count) as usize])
    }

    /// The tab at `index` on screen; `None` past the last one.
    pub fn tab_at(&self, index: usize) -> Option<SessionId> {
        self.open_tabs().get(index).copied()
    }

    fn tabs_of(&self, project: ProjectId) -> Vec<SessionId> {
        self.tabs
            .iter()
            .copied()
            .filter(|&id| {
                self.session(id)
                    .is_some_and(|session| session.project == project)
            })
            .collect()
    }

    /// Closes `closing`. If that closes the selection, `preferred` is selected,
    /// else the right neighbor on screen, else the left, else the project's draft.
    fn close_tabs(&mut self, closing: &[SessionId], preferred: Option<SessionId>, now: Timestamp) {
        let closing: HashSet<SessionId> = closing
            .iter()
            .copied()
            .filter(|id| self.tabs.contains(id))
            .collect();
        if closing.is_empty() {
            return;
        }
        let before = self.open_tabs();
        let selected = self.selected_session.filter(|id| closing.contains(id));
        self.tabs.retain(|id| !closing.contains(id));
        let Some(selected) = selected else {
            return;
        };
        let index = before.iter().position(|&tab| tab == selected).unwrap_or(0);
        let next = preferred
            .filter(|id| before.contains(id) && !closing.contains(id))
            .or_else(|| {
                before[index + 1..]
                    .iter()
                    .chain(before[..index].iter().rev())
                    .find(|id| !closing.contains(id))
                    .copied()
            });
        self.selected_session = None;
        let project = self.session(selected).map(|session| session.project);
        if let Some(next) =
            next.or_else(|| project.and_then(|project| self.new_session(project, now)))
        {
            self.open_session(next);
        }
    }
}
