//! A Settings section's parts: each a heading over its rows, grouped under one
//! rounded shape on `field_background` with a hairline round it and between
//! them. A row is a label over a line on what it does, its control at the end.

use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
};
use theme::{HAIRLINE, MENU_RADIUS, SPACE_2, SPACE_3, TEXT_SUBTITLE, Theme};

/// One setting: `label`, `detail` under it, `control` at the end. Long text
/// wraps; the control keeps its size.
pub(super) struct Row {
    pub label: SharedString,
    pub detail: SharedString,
    pub control: AnyElement,
}

pub(super) fn part(heading: &'static str, rows: Vec<Row>, cx: &App) -> impl IntoElement + use<> {
    let colors = Theme::of(cx).colors;
    let last = rows.len().saturating_sub(1);
    let group = div()
        .flex()
        .flex_col()
        .rounded(MENU_RADIUS)
        .border_1()
        .border_color(colors.border)
        .bg(colors.field_background)
        .children(rows.into_iter().enumerate().map(|(at, row)| {
            div()
                .p(SPACE_2 * 2. - HAIRLINE)
                .flex()
                .items_center()
                .justify_between()
                .gap(SPACE_3 * 2.)
                .when(at < last, |line| {
                    line.border_b(HAIRLINE).border_color(colors.border)
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        // The line sits close under its label.
                        .gap(HAIRLINE * 2.)
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(colors.text_primary)
                                .child(row.label),
                        )
                        .child(div().text_color(colors.text_secondary).child(row.detail)),
                )
                .child(div().flex_none().child(row.control))
        }));
    div()
        .flex()
        .flex_col()
        .gap(SPACE_3)
        .child(
            div()
                .text_size(TEXT_SUBTITLE)
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors.text_primary)
                .child(heading),
        )
        .child(group)
}
