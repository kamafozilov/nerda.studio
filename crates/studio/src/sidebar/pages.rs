//! The pages at the top of the sidebar: Notes, Design and Routines, one row
//! each. A click shows the page's tab, opening it if it isn't open; there is
//! never a second one.

use gpui::{ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, svg};
use theme::{CONTROL_RADIUS, ICON_SIZE, ROW_GAP, ROW_HEIGHT, SPACE_2, Theme};
use workspace::{Page, Tab};

use super::Sidebar;
use crate::assets::Icon;

fn icon(page: Page) -> Icon {
    match page {
        Page::Notes => Icon::Notes,
        Page::Design => Icon::Design,
        Page::Routines => Icon::Routines,
    }
}

impl Sidebar {
    pub(super) fn render_pages(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let selected = self.store.read(cx).workspace().selected_tab();
        let rows = Page::ALL.map(|page| {
            let is_selected = selected == Some(Tab::Page(page));
            // Each slot is `ROW_GAP` taller than its surface, so a selected row and
            // a hovered neighbor stay two shapes.
            div().h(ROW_HEIGHT + ROW_GAP).flex().items_center().child(
                div()
                    .id(("page", page as u64))
                    .flex_1()
                    .min_w_0()
                    .h(ROW_HEIGHT)
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .gap(SPACE_2)
                    .rounded(CONTROL_RADIUS)
                    .when(is_selected, |row| row.bg(colors.row_selected))
                    .when(!is_selected, |row| row.hover(|row| row.bg(colors.hover)))
                    .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                        sidebar.store.update(cx, |store, cx| {
                            store.update(cx, |workspace| workspace.open_page(page))
                        });
                    }))
                    .child(
                        svg()
                            .path(icon(page).path())
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
        // given back, so the rows sit flush with the header's edges.
        div().my(-(ROW_GAP / 2.)).flex().flex_col().children(rows)
    }
}
