//! Under the header: a notice if there is one, then the session list in its view
//! (by project, or every session naming its project), or what stands in for it
//! when there is nothing to list. With no projects the list is blank: the main
//! area offers the first step.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, ScrollStrategy, SharedString,
    Styled, div, prelude::*, px, svg, uniform_list,
};
use theme::{
    CONTROL_RADIUS, ICON_SIZE, ICON_SIZE_SMALL, ROW_GAP, ROW_HEIGHT, SPACE_2, SPACE_3, Theme,
};
use workspace::{ProjectScope, SessionId, SessionView, Timestamp, compact_age};

use super::{
    Row, Sidebar,
    project_rows::{indent_guide, row_slot},
};
use crate::{assets::Icon, tooltip::TooltipExt};

/// Widest a project name grows beside a session's title before it truncates.
const PROJECT_LABEL_MAX_WIDTH: gpui::Pixels = px(88.);

impl Sidebar {
    pub(super) fn render_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let notice = self.store.read(cx).notice()?.clone();
        let colors = Theme::of(cx).colors;
        Some(
            div()
                .mx(SPACE_2)
                .mt(SPACE_2)
                .p(SPACE_2)
                .flex()
                .items_start()
                .gap(SPACE_2)
                .rounded(CONTROL_RADIUS)
                .border_1()
                .border_color(colors.border)
                .bg(colors.popover_background)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(colors.text_primary)
                        .child(notice),
                )
                .child(
                    div()
                        .id("dismiss-notice")
                        .group("dismiss-notice")
                        .flex_none()
                        .size(ICON_SIZE)
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(CONTROL_RADIUS)
                        .cursor_pointer()
                        .hover(|button| button.bg(colors.hover))
                        .on_click(cx.listener(|sidebar, _: &ClickEvent, _, cx| {
                            sidebar
                                .store
                                .update(cx, |store, cx| store.dismiss_notice(cx))
                        }))
                        .child(
                            svg()
                                .path(Icon::Close.path())
                                .size(ICON_SIZE_SMALL)
                                .text_color(colors.text_secondary)
                                .group_hover("dismiss-notice", |icon| {
                                    icon.text_color(colors.text_primary)
                                }),
                        )
                        .with_tooltip("Dismiss message"),
                )
                .into_any_element(),
        )
    }

    /// Refreshes `rows` from the workspace, then draws them or an empty state.
    pub(super) fn render_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let workspace = self.store.read(cx).workspace();
        self.rows.clear();
        if !workspace.has_projects() {
            return div().flex_1().into_any_element();
        }
        let view = workspace.view();
        match view {
            SessionView::Projects => self.push_project_rows(workspace),
            SessionView::Sessions => self.rows.extend(
                workspace
                    .visible_sessions(&self.query)
                    .into_iter()
                    .map(|session| Row::Session(session.id)),
            ),
        }
        // Under a header the project goes without saying; in one list it needs
        // naming once more than one project is in view.
        self.label_projects = view == SessionView::Sessions
            && workspace.scope() == ProjectScope::All
            && workspace.projects().len() > 1;
        let searching = self.searching();
        if self.rows.is_empty() {
            let text: SharedString = if searching {
                format!("No sessions match \u{201c}{}\u{201d}", self.query.trim()).into()
            } else {
                "No sessions yet".into()
            };
            return self.empty_state(text, cx).into_any_element();
        }
        if searching && !matches!(self.rows.get(self.highlighted), Some(Row::Session(_))) {
            self.highlighted = self
                .rows
                .iter()
                .position(|row| matches!(row, Row::Session(_)))
                .unwrap_or(0);
        }
        if let Some(session) = self.reveal.take()
            && let Some(index) = self
                .rows
                .iter()
                .position(|&row| row == Row::Session(session))
        {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
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
                        range
                            .map(|index| sidebar.render_row(index, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        match self.rows.get(index) {
            Some(&Row::Project(project, sessions)) => {
                self.render_project_row(project, sessions, cx)
            }
            Some(&Row::Session(session)) => self.render_session_row(index, session, cx),
            Some(&Row::ShowMore(project, hidden)) => {
                self.render_show_row(project, Some(hidden), cx)
            }
            Some(&Row::ShowLess(project)) => self.render_show_row(project, None, cx),
            None => row_slot().into_any_element(),
        }
    }

    fn render_session_row(
        &self,
        index: usize,
        id: SessionId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let Some(session) = workspace.session(id) else {
            return row_slot().into_any_element();
        };
        let nested = workspace.view() == SessionView::Projects;
        let selected = workspace.selected_session() == Some(id);
        let highlighted = self.searching() && index == self.highlighted;
        let title = SharedString::from(session.title.clone());
        let age = compact_age(session.activity(), Timestamp::now());
        let project = self
            .label_projects
            .then(|| workspace.display_name(session.project))
            .flatten()
            .map(SharedString::from);
        // Under a project the row hangs from the project's guide.
        row_slot()
            .when(nested, |row| row.child(indent_guide(colors)))
            .child(
                div()
                    .id(("session", id.0))
                    .flex_1()
                    .min_w_0()
                    .h(ROW_HEIGHT)
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .gap(SPACE_2)
                    .rounded(CONTROL_RADIUS)
                    .when(selected, |row| row.bg(colors.row_selected))
                    .when(highlighted && !selected, |row| row.bg(colors.hover))
                    .when(!selected, |row| row.hover(|row| row.bg(colors.hover)))
                    .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                        sidebar.open_session(id, cx)
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(colors.text_primary)
                            .child(title),
                    )
                    .when_some(project, |row, project| {
                        row.child(
                            div()
                                .flex_none()
                                .max_w(PROJECT_LABEL_MAX_WIDTH)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_color(colors.text_secondary)
                                .child(project),
                        )
                    })
                    .child(
                        div()
                            .flex_none()
                            .text_color(colors.text_secondary)
                            .child(age),
                    ),
            )
            .into_any_element()
    }

    fn empty_state(&self, text: SharedString, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .px(SPACE_3)
            .pt(SPACE_3)
            .text_color(Theme::of(cx).colors.text_secondary)
            .child(text)
    }
}
