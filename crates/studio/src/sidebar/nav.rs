//! The rows at the top of the sidebar: New session, then the pages, Notes,
//! Design and Routines; the footer's pages are rows of the same kind. New
//! session opens its project's draft and stays marked while a draft is on
//! screen; a page shows its tab, opening it if it isn't open, and stays marked
//! while on screen. Each row's fill fades on its own.

use std::time::Instant;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString,
    Styled, Window, div, prelude::*, svg,
};
use theme::{
    CONTROL_RADIUS, DISABLED_OPACITY, ICON_SIZE_ROW, ROW_GAP, ROW_HEIGHT, SPACE_1, SPACE_2, Theme,
};
use workspace::{Page, ProjectScope};

use super::{Sidebar, fade::Key};
use crate::{
    assets::Icon,
    kbd::{Kbd, Shortcut},
    tooltip::TooltipExt,
};

/// New session's place among the rows; the pages follow it, the listed ones
/// first, then the footer's.
pub(super) const NEW_SESSION: usize = 0;

/// Settings' row, in the footer after the pages.
pub(super) const SETTINGS: usize = NEW_SESSION + 1 + Page::ALL.len();

/// The row the page takes.
pub(super) fn page_row(page: Page) -> usize {
    let at = Page::ALL.iter().position(|&each| each == page);
    NEW_SESSION + 1 + at.expect("every page is in Page::ALL")
}

impl Sidebar {
    fn hover_nav(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        if self.pointer_outside {
            super::note_hover(&mut self.waiting.nav, index, hovered);
            return;
        }
        if super::note_hover(&mut self.pointer.nav, index, hovered) {
            cx.notify();
        }
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

    pub(super) fn render_nav(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let now = Instant::now();
        let has_projects = self.store.read(cx).workspace().has_projects();
        let mut rows = vec![self.render_new_session(has_projects, now, cx)];
        for page in Page::LISTED {
            rows.push(self.render_page_row(page, now, cx).into_any_element());
        }
        if self.fills.fading(now) {
            window.request_animation_frame();
        }
        div()
            .px(SPACE_2)
            .py(SPACE_1)
            .flex()
            .flex_col()
            .gap(ROW_GAP)
            .children(rows)
            .into_any_element()
    }

    /// A page's row: a click shows its tab, opening it if it isn't open.
    pub(super) fn render_page_row(
        &mut self,
        page: Page,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        self.nav_row(
            page_row(page),
            Icon::page(page),
            page.title().into(),
            true,
            now,
            cx,
        )
        .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
            sidebar.store.update(cx, |store, cx| {
                store.update(cx, |workspace| workspace.open_page(page))
            });
        }))
    }

    fn render_new_session(
        &mut self,
        enabled: bool,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovered = self.pointer.nav == Some(NEW_SESSION);
        let row = self
            .nav_row(
                NEW_SESSION,
                Icon::NewSession,
                "New session".into(),
                enabled,
                now,
                cx,
            )
            // The shortcut shows under the pointer, so at rest the row reads
            // as the others do.
            .when(enabled && hovered, |row| {
                row.child(div().mr(-SPACE_1).child(Kbd::new(Shortcut::key("N").cmd())))
            });
        if enabled {
            row.on_click(
                cx.listener(|sidebar, _: &ClickEvent, _, cx| sidebar.new_session_in_view(cx)),
            )
            .into_any_element()
        } else {
            // The row tracks the pointer itself, so the tooltip goes on a
            // wrapper: one element takes one hover listener.
            div()
                .id("new-session-disabled")
                .opacity(DISABLED_OPACITY)
                .child(row)
                .with_tooltip("New session: add a project first")
                // Above, it would cover the top row's controls.
                .below()
                .into_any_element()
        }
    }

    /// One row: its icon and title in the row's color, on its fill. The row
    /// on screen and the one under the pointer take the wash and
    /// `text_primary`; the others rest in `text_secondary`.
    pub(super) fn nav_row(
        &mut self,
        index: usize,
        icon: Icon,
        title: SharedString,
        enabled: bool,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = Theme::of(cx).colors;
        let selected = self.selected_nav == Some(index);
        let hovered = enabled && self.pointer.nav == Some(index);
        let target = if selected {
            colors.row_selected
        } else if hovered {
            colors.hover
        } else {
            Hsla::transparent_black()
        };
        let fill = self.fills.at(Key::Nav(index), target, now);
        let color = if selected || hovered {
            colors.text_primary
        } else {
            colors.text_secondary
        };
        div()
            .id(("nav", index))
            .h(ROW_HEIGHT)
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .bg(fill)
            .when(enabled, |row| row.cursor_pointer())
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
            .on_hover(cx.listener(move |sidebar, hovered: &bool, _, cx| {
                sidebar.hover_nav(index, *hovered, cx)
            }))
            .child(
                svg()
                    .path(icon.path())
                    .size(ICON_SIZE_ROW)
                    .flex_none()
                    .text_color(color),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(title),
            )
    }
}
