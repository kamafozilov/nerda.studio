//! The sidebar under the top row: the header (search, the pages, filter and
//! sort, add project, new session) and the session list.
//!
//! Behavior: every session in one list, naming its project; the filter narrows it
//! to one project, and a new session from the header goes there. Search filters
//! live by session title; a new session reuses its project's empty session; adding
//! a folder that is already a project opens that project instead.

mod card;
mod footer;
mod glide;
mod header;
mod list;
mod menu;
mod pages;
mod shift;

pub(crate) use header::icon_button;

use std::path::Path;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, MouseExitEvent, MouseMoveEvent,
    ParentElement, Render, ScrollStrategy, Styled, Subscription, Task, UniformListScrollHandle,
    Window, actions, div, prelude::*,
};
use workspace::{AddProjectError, AddedProject, ProjectId, ProjectScope, SessionId, Timestamp};

use self::{card::Row, glide::Track, shift::Shifts};
use crate::{
    store::Store,
    text_input::{Lines, TextInput, TextInputEvent},
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
    /// The workspace, the search or the clock moved on since `rows` was read.
    stale: bool,
    /// The search and the filter `rows` was read under: a change to either
    /// replaces the list, while any other change moves the cards.
    rows_read_for: (String, ProjectScope),
    shifts: Shifts,
    /// The sessions on screen, in order, as the list draws them; read once
    /// per change, not on every frame of a glide.
    rows: Vec<Row>,
    /// The row of the search result Enter opens; meaningful only while searching.
    highlighted: usize,
    /// A session to bring into view on the next render, after a change put it there.
    reveal: Option<SessionId>,
    /// The rows under the pointer.
    pointer: Hovered,
    /// The rows the list drew last, to tell whether the hover surface's row is
    /// still in view.
    drawn_rows: std::ops::Range<usize>,
    /// The pointer left the window over the sidebar. GPUI then finds it over
    /// the row it left from, at its last place inside, so rows entered before
    /// the pointer moves again only wait here, and the next move inside the
    /// sidebar shows them.
    pointer_outside: bool,
    waiting: Hovered,
    /// The selection as the list last drew it: choosing another session glides
    /// the surface to it, while the same session moving in the list snaps.
    drawn_selection: Option<SessionId>,
    /// The selected session's row, while it is in the list.
    selected_row: Option<usize>,
    /// The page on screen, as a row of the pages.
    selected_page: Option<usize>,
    selection_surface: Track,
    hover_surface: Track,
    page_selection_surface: Track,
    page_hover_surface: Track,
    menu_open: bool,
    /// Where focus was before `focus_search` took it, given back by Escape on an
    /// empty search and when the sidebar closes.
    focus_before_search: Option<FocusHandle>,
    /// Where focus goes back to when nothing else held it: the window's own.
    home_focus: FocusHandle,
    /// Whether the sidebar is open: closed, nothing in it glides or keeps key
    /// focus.
    open: bool,
    /// Holds key focus while the menu is open, so Escape reaches it.
    menu_focus: FocusHandle,
    /// Where focus was before the menu opened, to give it back.
    focus_before_menu: Option<FocusHandle>,
    scroll: UniformListScrollHandle,
    /// The projects whose heads were read, to read again when they change.
    headed_projects: Vec<ProjectId>,
    heads_task: Task<()>,
    /// Redraws the rows when the first of their ages next changes ("5m" to
    /// "6m"), and reads the heads again then.
    age_timer: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl Sidebar {
    pub fn new(
        store: Entity<Store>,
        home_focus: FocusHandle,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search sessions", Lines::Single, cx));
        let search_focus = search.focus_handle(cx);
        let subscriptions = vec![
            cx.subscribe(&search, |sidebar, search, TextInputEvent::Changed, cx| {
                sidebar.query = search.read(cx).text().to_owned();
                sidebar.highlighted = 0;
                sidebar.stale = true;
                sidebar.scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            }),
            // Whatever gives focus back to the search while the sidebar is
            // closed (a palette or a menu closing), it doesn't stay there.
            cx.on_focus(&search_focus, window, |sidebar, window, cx| {
                if !sidebar.open {
                    sidebar.give_focus_back(window, cx);
                }
            }),
            cx.observe(&store, |sidebar, store, cx| {
                sidebar.stale = true;
                let projects = store.read(cx).workspace().project_ids();
                if projects != sidebar.headed_projects {
                    sidebar.refresh_heads(cx);
                }
                cx.notify();
            }),
        ];
        let mut sidebar = Self {
            store,
            search,
            query: String::new(),
            stale: true,
            rows_read_for: (String::new(), ProjectScope::All),
            shifts: Shifts::default(),
            rows: Vec::new(),
            highlighted: 0,
            reveal: None,
            pointer: Hovered::default(),
            drawn_rows: 0..0,
            pointer_outside: false,
            waiting: Hovered::default(),
            drawn_selection: None,
            selected_row: None,
            selected_page: None,
            selection_surface: Track::default(),
            hover_surface: Track::default(),
            page_selection_surface: Track::default(),
            page_hover_surface: Track::default(),
            menu_open: false,
            focus_before_search: None,
            home_focus,
            open,
            menu_focus: cx.focus_handle(),
            focus_before_menu: None,
            scroll: UniformListScrollHandle::new(),
            headed_projects: Vec::new(),
            heads_task: Task::ready(()),
            age_timer: Task::ready(()),
            _subscriptions: subscriptions,
        };
        sidebar.read_heads_now(cx);
        sidebar
    }

    fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Puts the cursor in the search field, its text selected so typing replaces it.
    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            self.close_menu(window, cx);
        }
        let search = self.search.focus_handle(cx);
        if !search.contains_focused(window, cx) {
            self.focus_before_search = window.focused(cx);
        }
        window.focus(&search, cx);
        window.dispatch_action(Box::new(crate::text_input::keys::SelectAll), cx);
    }

    /// Gives focus back to where it was before the search took it, else to the
    /// window.
    fn give_focus_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self
            .focus_before_search
            .take()
            .unwrap_or_else(|| self.home_focus.clone());
        window.focus(&focus, cx);
    }

    /// The sidebar opens or closes. Closing, nothing hidden keeps key focus, so
    /// typing never goes to a field out of sight.
    pub fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.open = open;
        if open {
            return;
        }
        if self.menu_open {
            self.close_menu(window, cx);
        }
        if self.search.focus_handle(cx).contains_focused(window, cx) {
            self.give_focus_back(window, cx);
        }
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
        let from = self.highlighted;
        self.highlighted = (self.highlighted as isize + step).rem_euclid(count) as usize;
        // To or from a row out of view the surface lands, as the list jumps.
        if self.drawn_rows.contains(&from) && self.drawn_rows.contains(&self.highlighted) {
            self.aim_hover();
        } else {
            self.hover_surface.place(self.hover_target());
        }
        self.scroll
            .scroll_to_item(self.highlighted, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn open_highlighted(
        &mut self,
        _: &OpenHighlighted,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.searching() {
            return;
        }
        if let Some(session) = self.rows.get(self.highlighted).map(|row| row.id) {
            self.open_session(session, cx);
            // The search is done: focus goes back to where it was.
            self.give_focus_back(window, cx);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            self.close_menu(window, cx);
        } else if self.query.is_empty() {
            // An empty search gives focus back to where Cmd-K took it from.
            self.give_focus_back(window, cx);
        } else {
            self.clear_search(cx);
        }
    }
}

impl Sidebar {
    fn pointer_left(&mut self, cx: &mut Context<Self>) {
        self.pointer_outside = true;
        self.waiting = Hovered::default();
        self.pointer = Hovered::default();
        self.aim_hover();
        self.aim_page_hover();
        cx.notify();
    }

    fn pointer_moved(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.pointer_outside) {
            return;
        }
        self.pointer = std::mem::take(&mut self.waiting);
        if let Some(row) = self.pointer.row
            && self.searching()
        {
            self.highlighted = row;
        }
        self.aim_hover();
        self.aim_page_hover();
        cx.notify();
    }
}

/// The rows under the pointer: one in the session list, one in the pages.
#[derive(Clone, Copy, Debug, Default)]
struct Hovered {
    row: Option<usize>,
    page: Option<usize>,
}

/// Notes that the pointer entered or left row `index`, in `slot`; returns
/// whether that changed it.
fn note_hover(slot: &mut Option<usize>, index: usize, hovered: bool) -> bool {
    if hovered {
        *slot = Some(index);
    } else if *slot == Some(index) {
        *slot = None;
    } else {
        return false;
    }
    true
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_rows(cx);
        let notice = self.render_notice(cx);
        let list = self.render_list(window, cx);
        div()
            .on_mouse_exit(
                cx.listener(|sidebar, _: &MouseExitEvent, _, cx| sidebar.pointer_left(cx)),
            )
            .on_mouse_move(
                cx.listener(|sidebar, _: &MouseMoveEvent, _, cx| sidebar.pointer_moved(cx)),
            )
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_header(window, cx))
            .children(notice)
            .child(list)
            .child(self.render_footer(cx))
    }
}
