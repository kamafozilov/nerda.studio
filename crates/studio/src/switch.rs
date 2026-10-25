//! A switch: a pill whose knob stands at its end while on. A click flips it,
//! and so do Space and Enter once Tab has reached it, which rings it in
//! `focus_ring`. The knob slides there on `PANEL_ENTER`, and turns back from
//! where it is if flipped again mid-way.

use gpui::{
    AnimationExt, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    SpringAnimation, StatefulInteractiveElement, Styled, Window, div, transparent_black,
};
use theme::{HAIRLINE, SWITCH_HEIGHT, SWITCH_INSET, SWITCH_KNOB, SWITCH_WIDTH, Theme, motion};

/// The switch `on`, `tab_index` in the page's Tab order, calling `toggle`
/// when flipped.
pub fn switch(
    id: impl Into<ElementId>,
    on: bool,
    tab_index: isize,
    toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let colors = Theme::of(cx).colors;
    let id = id.into();
    let travel = SWITCH_WIDTH - SWITCH_KNOB - SWITCH_INSET * 2.;
    let (track, knob) = if on {
        (colors.switch_on, colors.switch_on_knob)
    } else {
        (colors.switch_off, colors.switch_off_knob)
    };
    let spring = SpringAnimation::new(motion::PANEL_ENTER)
        .to(if on { 1. } else { 0. })
        .with_epsilon(motion::PHASE_EPSILON);
    // The ring stands a hairline outside the track, so it never covers it.
    div()
        .id(id.clone())
        .tab_index(tab_index)
        .flex_none()
        .p(HAIRLINE)
        .rounded_full()
        .border_1()
        .border_color(transparent_black())
        .focus_visible(|ring| ring.border_color(colors.focus_ring))
        .cursor_pointer()
        .on_click(toggle)
        .child(
            // The track's hairline counts in its inset, so the knob stands
            // `SWITCH_INSET` clear of the track's outer edge.
            div()
                .w(SWITCH_WIDTH)
                .h(SWITCH_HEIGHT)
                .p(SWITCH_INSET - HAIRLINE)
                .rounded_full()
                .border_1()
                .border_color(if on {
                    transparent_black()
                } else {
                    colors.popover_border
                })
                .bg(track)
                .child(div().size(SWITCH_KNOB).rounded_full().bg(knob).with_spring(
                    "knob",
                    spring,
                    move |knob, phase: f32| knob.ml(travel * phase.clamp(0., 1.)),
                )),
        )
}
