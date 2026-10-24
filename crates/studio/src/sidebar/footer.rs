//! The bottom row: Settings as a row like the pages above the list, then
//! Usage and Pull requests as icon buttons at its trailing edge. Each shows
//! its page's tab, opening it if it isn't open, and stays marked while on
//! screen. While more of the list lies below, the list fades out into the
//! footer, which takes on the sidebar's material.

use std::time::Instant;

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, Styled, Window, canvas, div, fill, prelude::*,
    svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, ICON_SIZE_ROW, MAIN_INSET, ROW_GAP, SPACE_1, SPACE_2, Theme,
};
use workspace::Page;

use super::{Sidebar, list::more_below, nav::page_row};
use crate::{
    assets::Icon,
    tooltip::{TooltipExt, WithTooltip},
};

/// The footer's icon buttons, in order, with their ids.
const BUTTONS: [(&str, Page); 2] = [
    ("usage", Page::Usage),
    ("pull-requests", Page::PullRequests),
];

impl Sidebar {
    pub(super) fn render_footer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let now = Instant::now();
        let material = Theme::of(cx).colors.window_background;
        let settings = self
            .render_page_row(Page::Settings, now, cx)
            .flex_1()
            .min_w_0();
        let buttons = BUTTONS.map(|(id, page)| self.page_button(id, page, cx));
        if self.fills.fading(now) {
            window.request_animation_frame();
        }
        let scroll = self.scroll.clone();
        // The row's bottom sits on the card's bottom edge, `MAIN_INSET` up.
        // The buttons stand `ROW_GAP` apart, so neighboring fills stay two
        // shapes, their trailing edge in line with the sessions' title row.
        div()
            .relative()
            .flex_none()
            .px(SPACE_2)
            .pt(SPACE_1)
            .pb(MAIN_INSET)
            .flex()
            .items_center()
            .gap(ROW_GAP)
            // Read at paint, once the list has laid out this frame.
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let below = more_below(&scroll);
                        if below > 0. {
                            window.paint_quad(fill(bounds, material.opacity(below)));
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(settings)
            .children(buttons)
    }

    /// A page as an icon button with a tooltip naming it. The page on screen
    /// keeps the rows' selected wash, its icon in `text_primary`.
    fn page_button(&self, id: &'static str, page: Page, cx: &mut Context<Self>) -> WithTooltip {
        let colors = Theme::of(cx).colors;
        let shown = self.selected_nav == Some(page_row(page));
        div()
            .id(id)
            .group(id)
            .size(CONTROL_SIZE)
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(CONTROL_RADIUS)
            .cursor_pointer()
            .when(shown, |button| button.bg(colors.row_selected))
            .when(!shown, |button| {
                button.hover(|button| button.bg(colors.hover))
            })
            .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                sidebar.store.update(cx, |store, cx| {
                    store.update(cx, |workspace| workspace.open_page(page))
                });
            }))
            .child(
                // An svg takes no color from its parent, so the hover color is its own.
                svg()
                    .path(Icon::page(page).path())
                    .size(ICON_SIZE_ROW)
                    .text_color(if shown {
                        colors.text_primary
                    } else {
                        colors.text_secondary
                    })
                    .group_hover(id, |icon| icon.text_color(colors.text_primary)),
            )
            .with_tooltip(page.title())
    }
}
