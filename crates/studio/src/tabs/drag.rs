//! Dragging a tab along the row. The dragged tab follows the pointer, held
//! inside the row; the tabs it passes slide over into the slot it left. On
//! release every tab glides from where it is drawn into its new slot.

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels,
    canvas, prelude::*, px,
};
use workspace::Tab;

use super::TabStrip;

/// How far the pointer travels before a press on a tab becomes a drag.
const DRAG_THRESHOLD: Pixels = px(4.);

pub(super) struct TabDrag {
    pub id: Tab,
    /// Becomes true once the pointer passes the threshold.
    pub active: bool,
    from: usize,
    to: usize,
    origin: Pixels,
    offset: Pixels,
    /// The slots of the tabs on screen when the press began, in order.
    slots: Vec<Bounds<Pixels>>,
}

impl TabStrip {
    pub(super) fn start_drag(&mut self, id: Tab, x: Pixels, cx: &mut Context<Self>) {
        self.skip_click = false;
        let bounds = self.bounds.borrow();
        let slots: Option<Vec<_>> = self
            .live
            .iter()
            .map(|tab| bounds.get(tab).copied())
            .collect();
        drop(bounds);
        let Some(from) = self.live.iter().position(|&tab| tab == id) else {
            return;
        };
        // One tab has nowhere to go; a tab still closing has no slot to hold.
        let Some(slots) = slots.filter(|slots| slots.len() > 1 && self.closing.is_empty()) else {
            return;
        };
        self.drag = Some(TabDrag {
            id,
            active: false,
            from,
            to: from,
            origin: x,
            offset: px(0.),
            slots,
        });
        cx.notify();
    }

    /// How far the tab is drawn from its slot.
    pub(super) fn shift(&self, id: Tab) -> Pixels {
        let Some(drag) = self.drag.as_ref().filter(|drag| drag.active) else {
            return px(0.);
        };
        let Some(index) = self.live.iter().position(|&tab| tab == id) else {
            return px(0.);
        };
        let left = |index: usize| drag.slots[index].left();
        if index == drag.from {
            drag.offset
        } else if drag.from < drag.to && index > drag.from && index <= drag.to {
            left(index - 1) - left(index)
        } else if drag.from > drag.to && index >= drag.to && index < drag.from {
            left(index + 1) - left(index)
        } else {
            px(0.)
        }
    }

    fn move_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        // A release that never reached the window must not wake on the next move.
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            cx.notify();
            return;
        }
        let delta = event.position.x - drag.origin;
        if !drag.active && delta.abs() < DRAG_THRESHOLD {
            return;
        }
        drag.active = true;
        let start = drag.slots[drag.from].left();
        let first = drag.slots[0].left() - start;
        let last = drag.slots[drag.slots.len() - 1].left() - start;
        let offset = delta.clamp(first, last);
        let drawn = start + offset;
        let to = drag
            .slots
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.left() - drawn)
                    .abs()
                    .partial_cmp(&(b.left() - drawn).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map_or(drag.from, |(index, _)| index);
        // Past either end of the row the tab holds still: nothing to redraw.
        if offset == drag.offset && to == drag.to {
            return;
        }
        drag.offset = offset;
        drag.to = to;
        cx.notify();
    }

    fn finish_drag(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        if !drag.active {
            self.drag = None;
            cx.notify();
            return;
        }
        // Where each tab is drawn now, so its glide starts there.
        let drawn: Vec<(Tab, Pixels)> = self
            .live
            .iter()
            .enumerate()
            .map(|(index, &id)| (id, drag.slots[index].left() + self.shift(id)))
            .collect();
        let slots: Vec<Pixels> = drag.slots.iter().map(|slot| slot.left()).collect();
        let (id, from, to) = (drag.id, drag.from, drag.to);
        self.drag = None;
        // Dropped where it started: the tab glides back and nothing is saved.
        if to != from {
            self.store.update(cx, |store, cx| {
                store.update(cx, |workspace| workspace.move_tab(id, to))
            });
        }
        let order = self.store.read(cx).workspace().open_tabs();
        self.drops += 1;
        self.settle = format!("tab-shift-{}", self.drops).into();
        self.settle_from.clear();
        for (index, id) in order.iter().enumerate() {
            if let (Some((_, left)), Some(slot)) =
                (drawn.iter().find(|(tab, _)| tab == id), slots.get(index))
            {
                self.settle_from.insert(*id, (*left - *slot).as_f32());
            }
        }
        self.skip_click = true;
        cx.notify();
    }

    /// While a tab is pressed, follows the pointer anywhere in the window, so a
    /// drag that leaves the row keeps going and its release is never missed.
    pub(super) fn drag_watcher(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.drag.as_ref()?;
        let strip = cx.entity().downgrade();
        Some(
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moves = strip.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture {
                            moves
                                .update(cx, |strip, cx| strip.move_drag(event, cx))
                                .ok();
                        }
                    });
                    let release = strip.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            release.update(cx, |strip, cx| strip.finish_drag(cx)).ok();
                        }
                    });
                },
            )
            .absolute()
            .size_0()
            .into_any_element(),
        )
    }
}
