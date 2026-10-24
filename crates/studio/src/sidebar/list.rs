//! Under the header: a notice if there is one, then every session in one list,
//! one line each (`card`), or what stands in for it when there is nothing to list.
//! With no projects the list is blank: the main area offers the first step.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, ScrollStrategy, SharedString,
    Styled, Task, Window, div, prelude::*, uniform_list,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use theme::{CONTROL_RADIUS, HAIRLINE, NOTICE_MAX_HEIGHT, ROW_HEIGHT, SPACE_1, SPACE_2, Theme};

use workspace::{Page, ProjectId, ProjectScope, Tab, Timestamp, age_changes_in};

use super::{
    Sidebar,
    card::ProjectFace,
    header::close_button,
    nav::{NEW_SESSION, page_row},
};

/// The longest the ages go without being checked: one minute.
const AGE_CHECK_MS: u64 = 60_000;

impl Sidebar {
    pub(super) fn render_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let notice = self.store.read(cx).notice()?.clone();
        let colors = Theme::of(cx).colors;
        Some(
            // The hairline comes out of the padding, so the text starts in the
            // sessions' title column.
            div()
                .mx(SPACE_2)
                .mt(SPACE_2)
                .p(SPACE_2 - HAIRLINE)
                .flex()
                .items_start()
                .gap(SPACE_2)
                .rounded(CONTROL_RADIUS)
                .border_1()
                .border_color(colors.border)
                .bg(colors.popover_background)
                .child(
                    div()
                        .id("notice-text")
                        .flex_1()
                        .min_w_0()
                        .max_h(NOTICE_MAX_HEIGHT)
                        .overflow_y_scroll()
                        .text_color(colors.text_primary)
                        .child(notice),
                )
                .child(close_button(
                    "dismiss-notice",
                    "Dismiss message",
                    colors,
                    cx.listener(|sidebar, _: &ClickEvent, _, cx| {
                        sidebar
                            .store
                            .update(cx, |store, cx| store.dismiss_notice(cx))
                    }),
                ))
                .into_any_element(),
        )
    }

    /// Reads the rows again if the workspace, the search or the clock moved
    /// on, and notes what is selected: the session on screen, or New session
    /// or the page in the rows above.
    pub(super) fn refresh_rows(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.stale) {
            return;
        }
        let store = self.store.read(cx);
        let workspace = store.workspace();
        self.rows.clear();
        let now = Timestamp::now();
        let mut faces: HashMap<ProjectId, ProjectFace> = HashMap::new();
        for session in workspace.visible_sessions(&self.query) {
            let Some(project) = workspace.project(session.project) else {
                continue;
            };
            let face = faces
                .entry(project.id)
                .or_insert_with(|| ProjectFace::new(project));
            self.rows.push(face.row(session, now));
        }
        if self.highlighted >= self.rows.len() {
            self.highlighted = 0;
        }
        let shown_page = workspace.selected_tab().and_then(Tab::page);
        self.footer_page = shown_page.filter(|page| !Page::LISTED.contains(page));
        let selected = workspace.selected_session();
        // A draft on screen is New session's: it isn't listed until its first
        // message.
        let draft = selected
            .and_then(|id| workspace.session(id))
            .is_some_and(|session| session.is_empty());
        self.selected_nav = match shown_page {
            Some(page) => page_row(page),
            None if draft => Some(NEW_SESSION),
            None => None,
        };
        // A search or a filter replaces the list: its rows show their fills
        // at once rather than fading from the old list's.
        let read_for = (self.query.clone(), workspace.scope());
        if read_for != self.rows_read_for {
            self.fills.forget_sessions();
        }
        self.rows_read_for = read_for;
        // Chosen from elsewhere (a tab, a shortcut), the row scrolls into view.
        if selected != self.drawn_selection {
            self.reveal = self.reveal.or(selected);
        }
        self.drawn_selection = selected;
        // The row under a still pointer may be another session now.
        self.pointer.row = self.pointer.row.filter(|&row| row < self.rows.len());
        self.schedule_age_redraw(now, cx);
    }

    /// Wakes when the first of the rows' ages next reads differently, to draw
    /// them again, and at least once a minute: the timer's clock stops while
    /// the Mac sleeps. Nothing wakes while no row shows an age.
    fn schedule_age_redraw(&mut self, now: Timestamp, cx: &mut Context<Self>) {
        let next = self
            .rows
            .iter()
            .map(|row| age_changes_in(row.activity, now).min(AGE_CHECK_MS))
            .min();
        self.age_timer = match next {
            Some(wait) => cx.spawn(async move |sidebar, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(wait))
                    .await;
                sidebar
                    .update(cx, |sidebar, cx| {
                        sidebar.stale = true;
                        sidebar.refresh_heads(cx);
                        cx.notify();
                    })
                    .ok();
            }),
            None => Task::ready(()),
        };
    }

    /// The row the hover marks: while searching, the result Enter opens,
    /// which the pointer moves; otherwise the row under the pointer.
    pub(super) fn hover_target(&self) -> Option<usize> {
        if self.searching() && !self.rows.is_empty() {
            Some(self.highlighted)
        } else {
            self.pointer.row
        }
    }

    /// The pointer entered or left session row `index`.
    pub(super) fn hover_row(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        if self.pointer_outside {
            super::note_hover(&mut self.waiting.row, index, hovered);
            return;
        }
        if !super::note_hover(&mut self.pointer.row, index, hovered) {
            return;
        }
        if hovered && self.searching() {
            self.highlighted = index;
        }
        cx.notify();
    }

    /// Draws the rows, or what stands in for them when there is nothing to list.
    pub(super) fn render_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.store.read(cx).workspace().has_projects() {
            return div().flex_1().into_any_element();
        }
        if self.rows.is_empty() {
            let workspace = self.store.read(cx).workspace();
            // Filtered to one project, the list says so: other projects may
            // have sessions.
            let filtered = match workspace.scope() {
                ProjectScope::Project(project) => workspace.display_name(project),
                ProjectScope::All => None,
            };
            let text: SharedString = match (self.searching(), filtered) {
                (true, _) => {
                    format!("No sessions match \u{201c}{}\u{201d}", self.query.trim()).into()
                }
                (false, Some(project)) => format!("No sessions in {project} yet").into(),
                (false, None) => "No sessions yet".into(),
            };
            return self.empty_state(text, cx).into_any_element();
        }
        if let Some(session) = self.reveal.take()
            && let Some(index) = self.rows.iter().position(|row| row.id == session)
        {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
        // The rows' fills draw again each frame until they settle.
        if self.fills.fading(Instant::now()) {
            window.request_animation_frame();
        }
        div()
            .flex_1()
            .min_h_0()
            .pt(SPACE_1)
            .flex()
            .flex_col()
            .child(
                uniform_list(
                    "sessions",
                    self.rows.len(),
                    cx.processor(|sidebar, range: std::ops::Range<usize>, window, cx| {
                        let rows: Vec<_> = range
                            .map(|index| sidebar.render_session_row(index, cx))
                            .collect();
                        if sidebar.fills.fading(Instant::now()) {
                            window.request_animation_frame();
                        }
                        rows
                    }),
                )
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }

    /// One line where the first row's title would start.
    fn empty_state(&self, text: SharedString, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_1().min_h_0().mx(SPACE_2).pt(SPACE_1).child(
            div()
                .h(ROW_HEIGHT)
                .px(SPACE_2)
                .flex()
                .items_center()
                .text_color(Theme::of(cx).colors.text_secondary)
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(text),
                ),
        )
    }
}
