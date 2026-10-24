//! The list's edge over the composer: the rows fading into it while more
//! lie below, and the button that takes the list back to its end.

use gpui::{
    Context, FollowMode, Hsla, IntoElement, ParentElement, Styled, canvas, div, fill,
    linear_color_stop, linear_gradient, prelude::*, svg,
};
use theme::{CONTROL_SIZE, ICON_SIZE, SCROLL_EDGE_FADE, SPACE_2, Theme};

use super::Conversation;
use crate::{assets::Icon, tooltip::TooltipExt};

impl Conversation {
    /// The rows fade into the composer while more lie below, thinning out
    /// over the last `SCROLL_EDGE_FADE` of them. Read at paint, once the
    /// list has laid out this frame.
    pub(super) fn edge_fade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let card = Theme::of(cx).colors.panel_background;
        let list = self.list.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let below =
                    list.max_offset_for_scrollbar().y + list.scroll_px_offset_for_scrollbar().y;
                let below = (below / SCROLL_EDGE_FADE).clamp(0., 1.);
                if below > 0. {
                    let to = card.opacity(below);
                    let from = Hsla { a: 0., ..to };
                    window.paint_quad(fill(
                        bounds,
                        linear_gradient(
                            180.,
                            linear_color_stop(from, 0.),
                            linear_color_stop(to, 1.),
                        ),
                    ));
                }
            },
        )
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(SCROLL_EDGE_FADE)
    }

    /// The button that brings a list scrolled up back to its end, centered
    /// over the composer: a circle on a menu's surface, lifted as a menu is,
    /// its arrow turning `text_primary` under the pointer.
    pub(super) fn scroll_to_end_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let list = self.list.clone();
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(SPACE_2)
            .flex()
            .justify_center()
            .child(
                div()
                    .id("scroll-to-end")
                    .group("scroll-to-end")
                    .size(CONTROL_SIZE)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_1()
                    .border_color(colors.popover_border)
                    .bg(colors.popover_background)
                    .shadow(colors.popover_shadows())
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        list.scroll_to_end();
                        list.set_follow_mode(FollowMode::Tail);
                        cx.notify();
                    }))
                    .child(
                        svg()
                            .path(Icon::ScrollDown.path())
                            .size(ICON_SIZE)
                            .text_color(colors.text_secondary)
                            .group_hover("scroll-to-end", |icon| {
                                icon.text_color(colors.text_primary)
                            }),
                    )
                    .with_tooltip("Scroll to the end"),
            )
    }
}
