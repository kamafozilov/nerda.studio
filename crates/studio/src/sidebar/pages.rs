//! The pages under the search field: Notes, Design and Routines, one row
//! each. A click shows the page's tab, opening it if it isn't open; there is
//! never a second one. Their surfaces glide like the sessions'.

use gpui::{ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px, svg};
use theme::{ICON_SIZE, ROW_GAP, ROW_HEIGHT, SPACE_2, Theme};
use workspace::Page;

use super::{Sidebar, glide::Glide};
use crate::assets::Icon;

impl Sidebar {
    fn hover_page(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        if self.pointer_outside {
            super::note_hover(&mut self.waiting.page, index, hovered);
            return;
        }
        if super::note_hover(&mut self.pointer.page, index, hovered) {
            self.aim_page_hover();
            cx.notify();
        }
    }

    /// Aims the pages' hover surface at the page under the pointer, unless it
    /// is the one on screen, which its own surface marks.
    pub(super) fn aim_page_hover(&mut self) {
        let target = self
            .pointer
            .page
            .filter(|&row| Some(row) != self.selected_page);
        self.page_hover_surface.aim(target);
    }

    pub(super) fn render_pages(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let glide = Glide {
            name: "page-surface",
            slot: ROW_HEIGHT + ROW_GAP,
            inset: px(0.),
            selected: self.page_selection_surface.take(),
            selected_offset: 0.,
            hovered: self.page_hover_surface.take(),
            selected_color: colors.row_selected,
            hovered_color: colors.hover,
        };
        let rows = Page::LISTED.into_iter().enumerate().map(|(index, page)| {
            // Each slot is `ROW_GAP` taller than its surface, so a selected row and
            // a hovered neighbor stay two shapes.
            div()
                .id(("page-slot", index))
                .h(ROW_HEIGHT + ROW_GAP)
                .flex()
                .items_center()
                .on_hover(cx.listener(move |sidebar, hovered: &bool, _, cx| {
                    sidebar.hover_page(index, *hovered, cx)
                }))
                .child(
                    div()
                        .id(("page", page as u64))
                        .flex_1()
                        .min_w_0()
                        .h(ROW_HEIGHT)
                        .px(SPACE_2)
                        .flex()
                        .items_center()
                        .gap(SPACE_2)
                        .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                            sidebar.store.update(cx, |store, cx| {
                                store.update(cx, |workspace| workspace.open_page(page))
                            });
                        }))
                        .child(
                            svg()
                                .path(Icon::page(page).path())
                                .size(ICON_SIZE)
                                .flex_none()
                                .text_color(colors.text_secondary),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_color(colors.text_primary)
                                .child(page.title()),
                        ),
                )
        });
        // The slots' half gaps above the first surface and under the last are
        // given back, so the rows sit flush with the header's edges. The
        // surfaces go first, under the rows.
        div()
            .relative()
            .my(-(ROW_GAP / 2.))
            .flex()
            .flex_col()
            .child(glide.render())
            .children(rows)
    }
}
