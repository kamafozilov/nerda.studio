//! A text tooltip naming an icon-only control, centered above it, or below it
//! in the window's top row, where there is no room above.
//!
//! GPUI's own `.tooltip()` places the label at the pointer and flips it over
//! the pointer near the window's bottom, onto neighboring controls. This one
//! anchors to the control instead.

use std::time::Instant;

use gpui::{
    App, Context, Div, ElementId, Global, IntoElement, MouseButton, ParentElement, RenderOnce,
    SharedString, Stateful, Styled, Task, WeakEntity, Window, deferred, div, prelude::*,
};
use theme::{
    CONTROL_RADIUS, SPACE_1, SPACE_2, TEXT_BODY, TOOLTIP_HEIGHT, Theme,
    motion::{TOOLTIP_DELAY, TOOLTIP_WARM},
};

pub trait TooltipExt {
    /// Shows `text` centered above the control once the pointer rests on it.
    /// Call it last: it ends the builder chain.
    fn with_tooltip(self, text: impl Into<SharedString>) -> WithTooltip;
}

impl TooltipExt for Stateful<Div> {
    fn with_tooltip(mut self, text: impl Into<SharedString>) -> WithTooltip {
        // The control's id keys the tooltip's state; a `Stateful` always has one.
        let id = self
            .interactivity()
            .element_id
            .clone()
            .unwrap_or_else(|| ElementId::Name(SharedString::default()));
        WithTooltip {
            id,
            text: text.into(),
            target: self,
            below: false,
        }
    }
}

#[derive(IntoElement)]
pub struct WithTooltip {
    id: ElementId,
    text: SharedString,
    target: Stateful<Div>,
    below: bool,
}

impl WithTooltip {
    /// Shows the tooltip under the control, for controls in the top row.
    pub fn below(mut self) -> Self {
        self.below = true;
        self
    }
}

#[derive(Default)]
struct TooltipState {
    visible: bool,
    pending: Option<Task<()>>,
}

/// Whether a tooltip showed a moment ago, which makes the next one show at once.
/// Hover events reach the control being entered before the one being left, so
/// a tooltip still showing counts too.
#[derive(Default)]
enum Warmth {
    #[default]
    Cold,
    Showing(WeakEntity<TooltipState>),
    HiddenAt(Instant),
}

impl Global for Warmth {}

impl TooltipState {
    fn hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            self.hide(Warmth::HiddenAt(Instant::now()), cx);
            return;
        }
        if self.is_warm(cx) {
            self.show(cx);
            return;
        }
        self.pending = Some(cx.spawn(async move |state, cx| {
            cx.background_executor().timer(TOOLTIP_DELAY).await;
            state.update(cx, |state, cx| state.show(cx)).ok();
        }));
    }

    fn is_warm(&self, cx: &Context<Self>) -> bool {
        match cx.try_global::<Warmth>() {
            Some(Warmth::Showing(other)) => {
                other.entity_id() != cx.entity_id()
                    && other.upgrade().is_some_and(|other| other.read(cx).visible)
            }
            Some(Warmth::HiddenAt(hidden)) => hidden.elapsed() < TOOLTIP_WARM,
            Some(Warmth::Cold) | None => false,
        }
    }

    fn show(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.visible = true;
        cx.set_global(Warmth::Showing(cx.weak_entity()));
        cx.notify();
    }

    /// Hides the tooltip; `after` replaces the warmth only if this one set it,
    /// so a neighbor that already took over keeps it.
    fn hide(&mut self, after: Warmth, cx: &mut Context<Self>) {
        self.pending = None;
        if !std::mem::take(&mut self.visible) {
            return;
        }
        if let Some(Warmth::Showing(owner)) = cx.try_global::<Warmth>()
            && owner.entity_id() == cx.entity_id()
        {
            cx.set_global(after);
        }
        cx.notify();
    }
}

impl RenderOnce for WithTooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, _| TooltipState::default());
        let visible = state.read(cx).visible;
        let colors = Theme::of(cx).colors;
        let on_hover = state.clone();
        self.target
            .relative()
            .on_hover(move |hovered, _, cx| {
                on_hover.update(cx, |state, cx| state.hover(*hovered, cx))
            })
            // A press acts on the control; its label has done its job.
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                state.update(cx, |state, cx| state.hide(Warmth::Cold, cx))
            })
            .when(visible, |target| {
                // Deferred, so it paints over later siblings and escapes the
                // sidebar's clip. The zero-inset row is the control's width; the
                // label overflows it evenly, which centers it.
                target.child(deferred(
                    div()
                        .absolute()
                        .when(self.below, |row| row.top_full().mt(SPACE_1))
                        .when(!self.below, |row| row.bottom_full().mb(SPACE_1))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .flex_none()
                                .h(TOOLTIP_HEIGHT)
                                .flex()
                                .items_center()
                                .whitespace_nowrap()
                                .px(SPACE_2)
                                .rounded(CONTROL_RADIUS)
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.popover_background)
                                .text_size(TEXT_BODY)
                                .text_color(colors.text_primary)
                                .child(self.text),
                        ),
                ))
            })
    }
}
