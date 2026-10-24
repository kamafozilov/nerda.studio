//! The small controls of the backdrop's panel: a titled section, two lines
//! of text, a choice chip, a text button and a switch.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Pixels, SharedString, Stateful,
    Styled, div, prelude::*, px,
};
use theme::{CONTROL_RADIUS, DISABLED_OPACITY, HAIRLINE, SPACE_2, SPACE_3, TEXT_SMALL, Theme};

/// The switch: its track and its knob.
const SWITCH_SIZE: (Pixels, Pixels) = (px(32.), px(18.));
const KNOB: Pixels = px(14.);

/// A titled part of the panel.
pub(super) fn section(
    title: &'static str,
    content: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    section_with(title, None, content, cx)
}

/// A titled part of the panel, with a control at the title's trailing end.
pub(super) fn section_with(
    title: &'static str,
    trailing: Option<AnyElement>,
    content: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(SPACE_2)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(TEXT_SMALL)
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::of(cx).colors.text_tertiary)
                .child(title)
                .children(trailing),
        )
        .child(content)
}

/// A title over a line of supporting text.
pub(super) fn two_lines(title: &'static str, detail: &'static str, cx: &App) -> impl IntoElement {
    let colors = Theme::of(cx).colors;
    div()
        .flex()
        .flex_col()
        .child(div().text_color(colors.text_primary).child(title))
        .child(
            div()
                .text_size(TEXT_SMALL)
                .text_color(colors.text_secondary)
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(detail),
        )
}

/// A chip that picks one of a few: `row_selected` with a ring while
/// chosen, faded while it can't be.
pub(super) fn choice_chip(
    id: SharedString,
    selected: bool,
    enabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let colors = Theme::of(cx).colors;
    div()
        .id(id)
        .h(px(28.))
        .px(SPACE_3 - HAIRLINE)
        .flex()
        .items_center()
        .gap(SPACE_2)
        .rounded_full()
        .border_1()
        .border_color(if selected {
            colors.focus_ring
        } else {
            colors.border
        })
        .when(selected, |chip| {
            chip.bg(colors.row_selected).text_color(colors.text_primary)
        })
        .when(!selected, |chip| chip.text_color(colors.text_secondary))
        .when(enabled && !selected, |chip| {
            chip.cursor_pointer()
                .hover(|chip| chip.bg(colors.hover).text_color(colors.text_primary))
        })
        .when(!enabled, |chip| chip.opacity(DISABLED_OPACITY))
}

/// A small button with a word on it.
pub(super) fn text_button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let colors = Theme::of(cx).colors;
    div()
        .id(id)
        .flex_none()
        .h(px(24.))
        .px(SPACE_2)
        .flex()
        .items_center()
        .rounded(CONTROL_RADIUS)
        .bg(colors.key_background)
        .text_size(TEXT_SMALL)
        .text_color(colors.text_primary)
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(colors.hover))
        })
        .when(!enabled, |button| button.opacity(DISABLED_OPACITY))
        .child(label)
}

/// A switch: the knob at the end and the track in the primary text's color
/// while on, the knob at the start on a key cap's fill while off.
pub(super) fn switch(on: bool, cx: &App) -> impl IntoElement {
    let colors = Theme::of(cx).colors;
    let inset = (SWITCH_SIZE.1 - KNOB) / 2.;
    div()
        .flex_none()
        .w(SWITCH_SIZE.0)
        .h(SWITCH_SIZE.1)
        .px(inset)
        .flex()
        .items_center()
        .when(on, |track| track.justify_end().bg(colors.text_primary))
        .when(!on, |track| track.bg(colors.key_background))
        .rounded_full()
        .child(div().size(KNOB).rounded_full().bg(if on {
            colors.popover_background
        } else {
            colors.text_secondary
        }))
}
