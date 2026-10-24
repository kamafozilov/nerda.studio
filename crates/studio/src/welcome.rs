//! The main area before the first project: what a project is, and the button
//! that adds one. The sidebar stays blank until then.

use gpui::{
    App, ClickEvent, FontWeight, IntoElement, ParentElement, Styled, Window, div, prelude::*,
};
use theme::{CONTROL_RADIUS, CONTROL_SIZE, SPACE_1, SPACE_3, Theme};

pub fn no_projects(
    on_add: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let colors = Theme::of(cx).colors;
    div()
        .flex_1()
        .min_h_0()
        .p(SPACE_3)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(SPACE_3)
        .text_center()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(SPACE_1)
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.text_primary)
                        .child("No projects yet"),
                )
                .child(
                    div()
                        .text_color(colors.text_secondary)
                        .child("A project is a folder your sessions work in."),
                ),
        )
        .child(
            div()
                .id("first-project")
                .h(CONTROL_SIZE)
                .px(SPACE_3)
                .flex()
                .items_center()
                .rounded(CONTROL_RADIUS)
                .border_1()
                .border_color(colors.border)
                .text_color(colors.text_primary)
                .cursor_pointer()
                .hover(|button| button.bg(colors.hover))
                .on_click(on_add)
                .child("Add project"),
        )
}
