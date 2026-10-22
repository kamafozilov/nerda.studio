//! One slot in the row: a tab, or a closed tab collapsing. The slot's width
//! springs open and shut while the tab inside keeps its settled width, so its
//! title is cut by the slot's edge instead of re-truncating every frame.

use gpui::{
    AnimationExt, AnyElement, ClickEvent, Context, ElementId, MouseButton, MouseDownEvent,
    ParentElement, SpringAnimation, Stateful, Styled, canvas, deferred, div, prelude::*, px,
    relative, svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, Colors, FIELD_TARGET_SIZE, ICON_SIZE, ICON_SIZE_SMALL, SPACE_1,
    SPACE_2, TAB_CLOSE_MIN_WIDTH, TAB_GAP, TAB_MAX_WIDTH, TAB_SELECTED_MIN_WIDTH,
    motion::{PHASE_EPSILON, PIXEL_EPSILON, TAB_ENTER, TAB_EXIT},
};
use workspace::SessionId;

use super::{Label, TabStrip};
use crate::{assets::Icon, tooltip::TooltipExt};

/// Narrowest share of its settled width a growing tab is drawn at; below it the
/// slot shows next to nothing anyway.
const MIN_OPEN: f32 = 0.05;

impl TabStrip {
    pub(super) fn render_slot(
        &mut self,
        id: SessionId,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let closing = self.closing.get(&id).map(|closing| closing.label.clone());
        let is_selected = closing.is_none() && self.selected == Some(id);
        let content = match closing {
            Some(label) => tab_face(id, &label, false, colors).into_any_element(),
            None => {
                let Some(label) = self.labels.get(&id).cloned() else {
                    return div().into_any_element();
                };
                let width = self.bounds.borrow().get(&id).map(|slot| slot.size.width);
                let can_close =
                    is_selected || width.is_none_or(|width| width >= TAB_CLOSE_MIN_WIDTH);
                self.live_tab(id, &label, is_selected, can_close, colors, cx)
                    .into_any_element()
            }
        };
        let open = if self.closing.contains_key(&id) {
            SpringAnimation::new(TAB_EXIT).to(0.)
        } else if self.opening.contains(&id) {
            SpringAnimation::new(TAB_ENTER).to(1.).from(0.)
        } else {
            SpringAnimation::new(TAB_ENTER).to(1.)
        }
        .with_epsilon(PHASE_EPSILON);
        let bounds = self.bounds.clone();
        let measure = canvas(
            move |slot, _, _| {
                bounds.borrow_mut().insert(id, slot);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let slot = div()
            .relative()
            .h_full()
            .min_w_0()
            .flex()
            .items_center()
            .overflow_hidden()
            .child(measure)
            .with_spring(("tab-slot", id.0), open, move |slot, open: f32| {
                let open = open.clamp(0., 1.);
                slot.flex_basis(TAB_MAX_WIDTH * open)
                    // A collapsing slot gives back its gap too.
                    .mr(-(TAB_GAP * (1. - open)))
                    .when(is_selected, |slot| {
                        slot.min_w(TAB_SELECTED_MIN_WIDTH.min(TAB_MAX_WIDTH * open))
                    })
                    .child(
                        div()
                            .flex_none()
                            .w(relative(1. / open.max(MIN_OPEN)))
                            .max_w(TAB_MAX_WIDTH)
                            .child(content),
                    )
            });
        let shift = self.shift(id);
        // The dragged tab rides the pointer itself, over its neighbors.
        if self
            .drag
            .as_ref()
            .is_some_and(|drag| drag.active && drag.id == id)
        {
            return deferred(slot.map_element(move |slot| slot.left(shift)))
                .priority(1)
                .into_any_element();
        }
        let from = self.settle_from.get(&id).copied().unwrap_or(0.);
        slot.with_spring(
            ElementId::NamedInteger(self.settle.clone(), id.0),
            SpringAnimation::new(TAB_ENTER)
                .to(shift.as_f32())
                .from(from)
                .with_epsilon(PIXEL_EPSILON),
            |slot, shift: f32| slot.map_element(move |slot| slot.left(px(shift))),
        )
        .into_any_element()
    }

    fn live_tab(
        &self,
        id: SessionId,
        label: &Label,
        selected: bool,
        can_close: bool,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> Stateful<gpui::Div> {
        tab_face(id, label, selected, colors)
            .occlude()
            .when(!selected, |tab| tab.hover(|tab| tab.bg(colors.hover)))
            .when(can_close, |tab| {
                tab.pr(FIELD_TARGET_SIZE + SPACE_1)
                    .child(close_button(id, label, colors, cx))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |strip, event: &MouseDownEvent, _, cx| {
                    strip.start_drag(id, event.position.x, cx)
                }),
            )
            .on_click(cx.listener(move |strip, _: &ClickEvent, _, cx| {
                if !std::mem::take(&mut strip.skip_click) {
                    strip.open(id, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(move |strip, _, _, cx| strip.close(id, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |strip, event: &MouseDownEvent, window, cx| {
                    strip.open_menu(id, event.position, window, cx)
                }),
            )
    }
}

/// The tab's plate and title; the selected one sits on the selection's fill.
fn tab_face(id: SessionId, label: &Label, selected: bool, colors: Colors) -> Stateful<gpui::Div> {
    div()
        .id(("tab", id.0))
        .group(label.group.clone())
        .relative()
        .h(CONTROL_SIZE)
        .px(SPACE_2)
        .flex()
        .items_center()
        .rounded(CONTROL_RADIUS)
        .when(selected, |tab| tab.bg(colors.row_selected))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_color(colors.text_primary)
                .child(label.title.clone()),
        )
}

/// Shows under the pointer only, so a row of tabs reads as titles. Hidden, it
/// still takes its clicks, but only a pointer over the tab, which shows it, can
/// reach it.
fn close_button(
    id: SessionId,
    label: &Label,
    colors: Colors,
    cx: &mut Context<TabStrip>,
) -> impl IntoElement {
    let button = label.close.clone();
    let target = div()
        .id(button.clone())
        .group(button.clone())
        .size(FIELD_TARGET_SIZE)
        .flex()
        .items_center()
        .justify_center()
        .opacity(0.)
        .group_hover(label.group.clone(), |button| button.opacity(1.))
        // The press closes, never starts a drag.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |strip, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            strip.close(id, cx);
        }))
        .child(
            div()
                .size(ICON_SIZE)
                .flex()
                .items_center()
                .justify_center()
                .rounded(CONTROL_RADIUS)
                .group_hover(button.clone(), |wash| wash.bg(colors.hover))
                .child(
                    svg()
                        .path(Icon::Close.path())
                        .size(ICON_SIZE_SMALL)
                        .text_color(colors.text_secondary)
                        .group_hover(button, |icon| icon.text_color(colors.text_primary)),
                ),
        );
    // The tooltip positions its control relatively, so this holder places it.
    div()
        .absolute()
        .right(SPACE_1)
        .top((CONTROL_SIZE - FIELD_TARGET_SIZE) / 2.)
        .child(
            target
                .with_tooltip(if cfg!(target_os = "macos") {
                    "Close tab (⌘W)"
                } else {
                    "Close tab (Ctrl+W)"
                })
                .below(),
        )
}
