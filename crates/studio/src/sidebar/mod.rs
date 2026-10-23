//! The sidebar under the top row: the header (the pages, search, filter and
//! sort, add project, new session) and the session list.
//!
//! Behavior: every session in one list, naming its project; the filter narrows it
//! to one project, and a new session from the header goes there. Search filters
//! live by session title; a new session reuses its project's empty session; adding
//! a folder that is already a project opens that project instead.

mod footer;
mod header;
mod list;
mod menu;
mod pages;

pub(crate) use header::icon_button;

use std::path::Path;

use gpui::{
    App, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, ScrollStrategy, Styled,
    Subscription, Task, UniformListScrollHandle, Window, actions, div, prelude::*,
};
use workspace::{AddProjectError, AddedProject, ProjectId, ProjectScope, SessionId, Timestamp};

use crate::{
    store::Store,
    text_input::{TextInput, TextInputEvent},
};

actions!(
    sidebar,
    [
        /// Moves the search highlight down, wrapping at the end.
        SelectNext,
        /// Moves the search highlight up, wrapping at the start.
        SelectPrevious,
        /// Opens the highlighted search result and clears the search.
        OpenHighlighted,
        /// Closes the menu, else clears the search; does nothing on an empty one.
        Dismiss,
    ]
);

/// Key bindings for the search field and the menu.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("down", SelectNext, Some("SidebarSearch")),
        gpui::KeyBinding::new("up", SelectPrevious, Some("SidebarSearch")),
        gpui::KeyBinding::new("enter", OpenHighlighted, Some("SidebarSearch")),
        gpui::KeyBinding::new("escape", Dismiss, Some("SidebarSearch")),
        gpui::KeyBinding::new("escape", Dismiss, Some("SidebarMenu")),
    ]);
}

pub struct Sidebar {
    store: Entity<Store>,
    search: Entity<TextInput>,
    /// The search text, trimmed by the workspace when matching.
    query: String,
    /// The sessions on screen, in order: the list renders rows from this.
    rows: Vec<SessionId>,
    /// The row of the search result Enter opens; meaningful only while searching.
    highlighted: usize,
    /// A session to bring into view on the next render, after a change put it there.
    reveal: Option<SessionId>,
    /// Session rows name their project: more than one project is in view.
    label_projects: bool,
    menu_open: bool,
    /// Holds key focus while the menu is open, so Escape reaches it.
    menu_focus: FocusHandle,
    /// Where focus was before the menu opened, to give it back.
    focus_before_menu: Option<FocusHandle>,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
    /// Redraws now and then so "5m" ages stay true.
    _age_ticker: Task<()>,
}

impl Sidebar {
    pub fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search sessions", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |sidebar, search, TextInputEvent::Changed, cx| {
                sidebar.query = search.read(cx).text().to_owned();
                sidebar.highlighted = 0;
                sidebar.scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
        ];
        let age_ticker = cx.spawn(async move |sidebar, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(30))
                    .await;
                if sidebar.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            store,
            search,
            query: String::new(),
            rows: Vec::new(),
            highlighted: 0,
            reveal: None,
            label_projects: false,
            menu_open: false,
            menu_focus: cx.focus_handle(),
            focus_before_menu: None,
            scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
            _age_ticker: age_ticker,
        }
    }

    fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }

    fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_text("", cx));
    }

    /// Opens a session in its tab and leaves search so it shows in place.
    fn open_session(&mut self, session: SessionId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| workspace.open_session(session));
        });
        self.clear_search(cx);
        self.reveal = Some(session);
        cx.notify();
    }

    /// Makes `session` the selection, widening the filter if it would hide it.
    fn show_session_in(&mut self, session: SessionId, project: ProjectId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                if matches!(workspace.scope(), ProjectScope::Project(scoped) if scoped != project) {
                    workspace.set_scope(ProjectScope::All);
                }
                workspace.open_session(session);
            });
        });
        self.clear_search(cx);
        self.reveal = Some(session);
        cx.notify();
    }

    /// Starts a session in the current project, or reopens its empty one.
    pub fn new_session(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.store.read(cx).workspace().context_project() else {
            return;
        };
        self.new_session_in(project, cx);
    }

    /// The header's New session: in the project the list is filtered to, else
    /// where the shortcut would start one.
    fn new_session_in_view(&mut self, cx: &mut Context<Self>) {
        let workspace = self.store.read(cx).workspace();
        let project = match workspace.scope() {
            ProjectScope::Project(project) => Some(project),
            ProjectScope::All => workspace.context_project(),
        };
        if let Some(project) = project {
            self.new_session_in(project, cx);
        }
    }

    fn new_session_in(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let session = self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                workspace.new_session(project, Timestamp::now())
            })
        });
        if let Some(session) = session {
            self.show_session_in(session, project, cx);
        }
    }

    /// Adds the folder at `path`. A new project opens a new session in it; a folder
    /// already added opens that project's latest session, or a new one if it has none.
    pub fn add_project_at(
        &mut self,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> Result<(), AddProjectError> {
        let added = self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                workspace.add_project(path, Timestamp::now())
            })
        })?;
        match added {
            AddedProject::Added(project) => self.new_session_in(project, cx),
            AddedProject::Existing(project) => {
                let latest = self
                    .store
                    .read(cx)
                    .workspace()
                    .latest_session(project)
                    .map(|session| session.id);
                match latest {
                    Some(session) => self.show_session_in(session, project, cx),
                    None => self.new_session_in(project, cx),
                }
            }
        }
        Ok(())
    }

    fn open_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_before_menu = window.focused(cx);
        self.menu_open = true;
        window.focus(&self.menu_focus, cx);
        cx.notify();
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = false;
        if let Some(focus) = self.focus_before_menu.take() {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(1, cx);
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(-1, cx);
    }

    /// Steps the search highlight over the rows, wrapping at either end.
    fn move_highlight(&mut self, step: isize, cx: &mut Context<Self>) {
        if !self.searching() || self.rows.is_empty() {
            return;
        }
        let count = self.rows.len() as isize;
        self.highlighted = (self.highlighted as isize + step).rem_euclid(count) as usize;
        self.scroll
            .scroll_to_item(self.highlighted, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn open_highlighted(&mut self, _: &OpenHighlighted, _: &mut Window, cx: &mut Context<Self>) {
        if !self.searching() {
            return;
        }
        if let Some(&session) = self.rows.get(self.highlighted) {
            self.open_session(session, cx);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            self.close_menu(window, cx);
        } else if self.query.is_empty() {
            cx.propagate();
        } else {
            self.clear_search(cx);
        }
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notice = self.render_notice(cx);
        let list = self.render_list(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_header(window, cx))
            .children(notice)
            .child(list)
            .child(self.render_footer(cx))
    }
}
