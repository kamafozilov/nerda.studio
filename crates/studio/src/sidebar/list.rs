//! Under the header: a notice if there is one, then every session in one list,
//! as cards (`card`), or what stands in for it when there is nothing to list.
//! With no projects the list is blank: the main area offers the first step.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, ScrollStrategy, SharedString,
    Styled, Task, Window, div, prelude::*, uniform_list,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use theme::{
    CONTROL_RADIUS, HAIRLINE, NOTICE_MAX_HEIGHT, ROW_GAP, ROW_HEIGHT, SESSION_ROW_HEIGHT, SPACE_2,
    Theme,
};

use workspace::{Page, ProjectId, ProjectScope, SessionId, Tab, Timestamp, age_changes_in};

use super::{Sidebar, card::ProjectFace, glide::Glide, header::close_button};

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

    /// Reads the rows again if the workspace, the search or the clock moved on,
    /// and aims the surfaces: a newly chosen session glides the selection to it,
    /// while rows that moved under a choice that stayed put the surface there.
    pub(super) fn refresh_rows(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.stale) {
            return;
        }
        let store = self.store.read(cx);
        let workspace = store.workspace();
        let previous: Vec<SessionId> = self.rows.iter().map(|row| row.id).collect();
        self.rows.clear();
        let now = Timestamp::now();
        let mut faces: HashMap<ProjectId, ProjectFace> = HashMap::new();
        for session in workspace.visible_sessions(&self.query) {
            let Some(project) = workspace.project(session.project) else {
                continue;
            };
            let face = faces.entry(project.id).or_insert_with(|| {
                let name = workspace.display_name(project.id).unwrap_or_default();
                ProjectFace::new(project, name, store.head(project.id))
            });
            self.rows.push(face.row(session, now));
        }
        if self.highlighted >= self.rows.len() {
            self.highlighted = 0;
        }
        let page = match workspace.selected_tab() {
            Some(Tab::Page(page)) => Page::ALL.iter().position(|&each| each == page),
            _ => None,
        };
        self.page_selection_surface.aim(page);
        self.selected_page = page;
        self.aim_page_hover();
        let selected = workspace.selected_session();
        let index = selected.and_then(|id| self.rows.iter().position(|row| row.id == id));
        let moved = !self
            .rows
            .iter()
            .map(|row| row.id)
            .eq(previous.iter().copied());
        let read_for = (self.query.clone(), workspace.scope());
        let replaced = moved && (read_for != self.rows_read_for || previous.is_empty());
        if moved {
            if replaced || !self.open || crate::accessibility::reduce_motion() {
                self.shifts.stop();
            } else {
                let after: Vec<SessionId> = self.rows.iter().map(|row| row.id).collect();
                let slot = (SESSION_ROW_HEIGHT + ROW_GAP).as_f32();
                self.shifts.start(&previous, &after, slot);
            }
        }
        self.rows_read_for = read_for;
        // A new choice glides to its row, unless the list was replaced under it
        // (a slide from a place in the old list would mean nothing), one of the
        // rows is out of view (the list jumps to it), or the sidebar is closed.
        // Chosen from elsewhere (a tab, a shortcut), the row scrolls into view.
        if selected != self.drawn_selection {
            let in_view =
                |row: Option<usize>| row.is_some_and(|row| self.drawn_rows.contains(&row));
            let glides = !replaced
                && self.open
                && in_view(Some(self.selection_surface.row()))
                && in_view(index);
            if glides {
                self.selection_surface.aim(index);
            } else {
                self.selection_surface.place(index);
            }
            self.reveal = self.reveal.or(selected);
        } else if moved {
            self.selection_surface.place(index);
        }
        self.drawn_selection = selected;
        self.selected_row = index;
        if moved {
            // The row under a still pointer may be another session now; the
            // surface stays where the pointer is.
            self.pointer.row = self.pointer.row.filter(|&row| row < self.rows.len());
            self.hover_surface.place(self.hover_target());
        } else {
            // A new search over the same rows starts the highlight on the first.
            self.aim_hover();
        }
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

    /// The row the hover surface marks: while searching, the result Enter opens,
    /// which the pointer moves; otherwise the row under the pointer. Never the
    /// selected row: its own surface marks it, and the two washes together
    /// would take its text under 4.5:1.
    pub(super) fn hover_target(&self) -> Option<usize> {
        let target = if self.searching() && !self.rows.is_empty() {
            Some(self.highlighted)
        } else {
            self.pointer.row
        };
        target.filter(|&row| Some(row) != self.selected_row)
    }

    pub(super) fn aim_hover(&mut self) {
        let target = self.hover_target();
        self.hover_surface.aim(target);
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
        // From a row scrolled out of view, as after a long scroll, the surface
        // lands rather than sliding in across the list.
        if self.drawn_rows.contains(&self.hover_surface.row()) {
            self.aim_hover();
        } else {
            self.hover_surface.place(self.hover_target());
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
        // Cards gliding to new places are drawn anew each frame until they land.
        if self.shifts.active() {
            window.request_animation_frame();
        }
        let colors = Theme::of(cx).colors;
        let glide = Glide {
            name: "session-surface",
            slot: SESSION_ROW_HEIGHT + ROW_GAP,
            inset: SPACE_2,
            selected: self.selection_surface.take(),
            // The selection rides its card while the card is in view; one that
            // glides out of view goes with it, its surface already in place.
            selected_offset: self
                .drawn_selection
                .filter(|_| {
                    self.selected_row
                        .is_some_and(|row| self.drawn_rows.contains(&row))
                })
                .map_or(0., |id| self.shifts.offset(id, Instant::now())),
            hovered: self.hover_surface.take(),
            selected_color: colors.row_selected,
            hovered_color: colors.hover,
        };
        // The gap under the header sits outside the scrolling area, so rows scroll
        // out of sight below it instead of up against the header. The first row's
        // slot adds half a `ROW_GAP` above its surface, so the gap gives that back.
        div()
            .flex_1()
            .min_h_0()
            .pt(SPACE_2 - ROW_GAP / 2.)
            .flex()
            .flex_col()
            .child(
                uniform_list(
                    "sessions",
                    self.rows.len(),
                    cx.processor(|sidebar, range: std::ops::Range<usize>, _, cx| {
                        sidebar.drawn_rows = range.clone();
                        range
                            .map(|index| sidebar.render_session_row(index, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .with_decoration(glide)
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }

    /// One line where the first card's title would start: in the title column,
    /// `SPACE_2` under the header.
    fn empty_state(&self, text: SharedString, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_1().min_h_0().mx(SPACE_2).pt(SPACE_2).child(
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
