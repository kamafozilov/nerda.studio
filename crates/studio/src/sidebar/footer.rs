//! The bottom row: usage, pull requests and settings, at the trailing edge.
//! Each shows its page's tab, opening it if it isn't open; the button of the
//! page on screen keeps its `hover` surface.

use gpui::{ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*};
use theme::{MAIN_INSET, SPACE_1, SPACE_2, Theme};
use workspace::Page;

use super::{Sidebar, header::icon_button};
use crate::assets::Icon;

/// The footer's pages, in order, with their buttons' ids.
const PAGES: [(&str, Page); 3] = [
    ("usage", Page::Usage),
    ("pull-requests", Page::PullRequests),
    ("settings", Page::Settings),
];

impl Sidebar {
    pub(super) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let buttons = PAGES.map(|(id, page)| {
            let shown = self.footer_page == Some(page);
            let label = match page {
                Page::Settings if cfg!(target_os = "macos") => "Settings (⌘,)",
                Page::Settings => "Settings (Ctrl+,)",
                page => page.title(),
            };
            icon_button(id, Icon::page(page), label, true, colors, |button| {
                button
                    .when(shown, |button| button.bg(colors.hover))
                    .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                        sidebar.store.update(cx, |store, cx| {
                            store.update(cx, |workspace| workspace.open_page(page))
                        });
                    }))
            })
        });
        // The buttons' bottom sits on the card's bottom edge, `MAIN_INSET` up.
        div()
            .flex_none()
            .border_t_1()
            .border_color(colors.border)
            .px(SPACE_2)
            .pt(SPACE_2)
            .pb(MAIN_INSET)
            .flex()
            .items_center()
            .justify_end()
            .gap(SPACE_1)
            .children(buttons)
    }
}
