//! Sessions and pages open as tabs. Each project keeps its own row of session
//! tabs, and the pages show in every row: the row on screen is the pages and
//! the sessions of the project on screen, the selected session's. Opening a
//! session or a page gives it a tab unless it has one; closing the selected tab
//! selects its right neighbor, else its left, and closing the last tab on
//! screen opens the project's draft, so a project on screen always shows
//! something. That draft alone on screen can't close: closing it would only
//! open it again.

use std::collections::HashSet;

use crate::{Page, ProjectId, SessionId, Tab, Timestamp, Workspace};

impl Workspace {
    /// Every open tab, in order, across all projects.
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// The tabs on screen, in order: the pages and the project on screen's sessions.
    pub fn open_tabs(&self) -> Vec<Tab> {
        self.tabs
            .iter()
            .copied()
            .filter(|&tab| match tab {
                Tab::Page(_) => true,
                Tab::Session(id) => self.project_on_screen.is_some_and(|project| {
                    self.session(id)
                        .is_some_and(|session| session.project == project)
                }),
            })
            .collect()
    }

    /// Shows a tab: opens it last unless it is open, and selects it. A session
    /// brings its project on screen; an unknown one changes nothing.
    pub fn open_tab(&mut self, tab: Tab) {
        if let Tab::Session(id) = tab {
            let Some(project) = self.session(id).map(|session| session.project) else {
                return;
            };
            self.project_on_screen = Some(project);
        }
        if !self.tabs.contains(&tab) {
            self.tabs.push(tab);
        }
        self.selected = Some(tab);
    }

    pub fn open_session(&mut self, id: SessionId) {
        self.open_tab(Tab::Session(id));
    }

    pub fn open_page(&mut self, page: Page) {
        self.open_tab(Tab::Page(page));
    }

    /// With projects but nothing on screen, opens the context project's latest
    /// tab, else its draft. Returns whether anything changed.
    pub fn ensure_open_session(&mut self, now: Timestamp) -> bool {
        if self.selected.is_some() {
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

    /// Whether closing the tab on screen `tab` changes anything: false off
    /// screen, and for an unused draft alone on screen.
    pub fn can_close_tab(&self, tab: Tab) -> bool {
        let tabs = self.open_tabs();
        tabs.contains(&tab) && (tabs.len() > 1 || !self.is_draft(tab))
    }

    pub fn close_tab(&mut self, tab: Tab, now: Timestamp) {
        if self.can_close_tab(tab) {
            self.close_tabs(&[tab], None, now);
        }
    }

    /// Closes every tab on screen but `tab`, and selects it.
    pub fn close_other_tabs(&mut self, tab: Tab, now: Timestamp) {
        let others: Vec<Tab> = self
            .open_tabs()
            .into_iter()
            .filter(|&other| other != tab)
            .collect();
        self.close_tabs(&others, Some(tab), now);
    }

    /// Closes the tabs on screen after `tab`; `tab` is selected if the selection closes.
    pub fn close_tabs_after(&mut self, tab: Tab, now: Timestamp) {
        let tabs = self.open_tabs();
        if let Some(index) = tabs.iter().position(|&open| open == tab) {
            self.close_tabs(&tabs[index + 1..], Some(tab), now);
        }
    }

    /// Closes the tabs on screen before `tab`; `tab` is selected if the selection closes.
    pub fn close_tabs_before(&mut self, tab: Tab, now: Timestamp) {
        let tabs = self.open_tabs();
        if let Some(index) = tabs.iter().position(|&open| open == tab) {
            self.close_tabs(&tabs[..index], Some(tab), now);
        }
    }

    /// Closes every tab on screen; the project's draft opens in their place.
    pub fn close_all_tabs(&mut self, now: Timestamp) {
        let tabs = self.open_tabs();
        self.close_tabs(&tabs, None, now);
    }

    /// Moves a tab on screen to `to`, an index among the tabs on screen; other
    /// projects' tabs keep their places.
    pub fn move_tab(&mut self, tab: Tab, to: usize) {
        let mut tabs = self.open_tabs();
        let Some(from) = tabs.iter().position(|&open| open == tab) else {
            return;
        };
        tabs.remove(from);
        tabs.insert(to.min(tabs.len()), tab);
        let shown: HashSet<Tab> = tabs.iter().copied().collect();
        let mut reordered = tabs.into_iter();
        for slot in self.tabs.iter_mut().filter(|open| shown.contains(open)) {
            if let Some(tab) = reordered.next() {
                *slot = tab;
            }
        }
    }

    /// The tab `step` places from the selected one, wrapping at either end.
    pub fn adjacent_tab(&self, step: isize) -> Option<Tab> {
        let tabs = self.open_tabs();
        let current = tabs.iter().position(|&tab| Some(tab) == self.selected)?;
        let count = tabs.len() as isize;
        Some(tabs[(current as isize + step).rem_euclid(count) as usize])
    }

    /// The tab at `index` on screen; `None` past the last one.
    pub fn tab_at(&self, index: usize) -> Option<Tab> {
        self.open_tabs().get(index).copied()
    }

    /// The project's sessions open as tabs, in order.
    fn tabs_of(&self, project: ProjectId) -> Vec<SessionId> {
        self.tabs
            .iter()
            .filter_map(|tab| tab.session())
            .filter(|&id| {
                self.session(id)
                    .is_some_and(|session| session.project == project)
            })
            .collect()
    }

    /// An unused session: a reusable draft.
    fn is_draft(&self, tab: Tab) -> bool {
        tab.session()
            .and_then(|id| self.session(id))
            .is_some_and(|session| session.is_empty())
    }

    /// Closes `closing`. If that closes the selection, `preferred` is selected,
    /// else the right neighbor on screen, else the left, else the draft of the
    /// project on screen; with no project, nothing.
    pub(crate) fn close_tabs(&mut self, closing: &[Tab], preferred: Option<Tab>, now: Timestamp) {
        let closing: HashSet<Tab> = closing
            .iter()
            .copied()
            .filter(|tab| self.tabs.contains(tab))
            .collect();
        if closing.is_empty() {
            return;
        }
        let before = self.open_tabs();
        let selected = self.selected.filter(|tab| closing.contains(tab));
        self.tabs.retain(|tab| !closing.contains(tab));
        let Some(selected) = selected else {
            return;
        };
        let index = before.iter().position(|&tab| tab == selected).unwrap_or(0);
        let next = preferred
            .filter(|tab| before.contains(tab) && !closing.contains(tab))
            .or_else(|| {
                before[index + 1..]
                    .iter()
                    .chain(before[..index].iter().rev())
                    .find(|tab| !closing.contains(tab))
                    .copied()
            });
        self.selected = None;
        let draft = || {
            self.project_on_screen
                .and_then(|project| self.new_session(project, now))
                .map(Tab::Session)
        };
        if let Some(next) = next.or_else(draft) {
            self.open_tab(next);
        }
    }
}
