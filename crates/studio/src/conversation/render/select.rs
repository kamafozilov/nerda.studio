//! Selecting a piece of a reply's text with the pointer: a press starts a
//! selection there (Shift extends the one there is), dragging draws it out
//! across the reply's pieces, a double press takes a word and a triple the
//! whole piece. A right press opens the text's menu. The selection draws
//! as `text_selection` under the text.

use std::{ops::Range, rc::Rc};

use gpui::{
    App, BorderStyle, Bounds, CursorStyle, DispatchPhase, FocusHandle, Hitbox, Hsla, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, TextLayout, Window, point, px,
    quad, size,
};

use super::chip_paint::lines;
use crate::conversation::selection::{Selection, Shared, Target, word_at};

/// Opens the text's menu where the pointer is, for a reply and what the
/// pointer is on.
pub type OpenMenu = Rc<dyn Fn(&(Point<Pixels>, u64, Option<Target>), &mut Window, &mut App)>;

/// What makes a text selectable: the shared selection, the reply and the
/// piece it is, and what its links and chips open.
#[derive(Clone)]
pub struct Select {
    pub state: Shared,
    pub item: u64,
    pub number: usize,
    pub color: Hsla,
    pub focus: FocusHandle,
    pub menu: OpenMenu,
    pub targets: Rc<[(Range<usize>, Target)]>,
}

impl Select {
    /// Paints the part of the text selected, under it.
    pub fn paint(&self, layout: &TextLayout, window: &mut Window) {
        let length = layout.len();
        let Some(range) = self
            .state
            .borrow()
            .selected_in(self.item, self.number, length)
        else {
            return;
        };
        let line_height = layout.line_height();
        for (left, right, top) in lines(layout, &range) {
            let bounds = Bounds::new(
                point(left, top),
                size((right - left).max(px(2.)), line_height),
            );
            window.paint_quad(quad(
                bounds,
                px(0.),
                self.color,
                px(0.),
                self.color,
                BorderStyle::Solid,
            ));
        }
    }

    /// Listens for the pointer over the text, whose layout is `layout`.
    pub fn listen(&self, layout: &TextLayout, hitbox: &Hitbox, window: &mut Window) {
        window.set_cursor_style(CursorStyle::IBeam, hitbox);
        let index = {
            let layout = layout.clone();
            move |position: Point<Pixels>| match layout.index_for_position(position) {
                Ok(at) | Err(at) => at.min(layout.len()),
            }
        };
        let this = self.clone();
        let hitbox_down = hitbox.clone();
        let text = layout.text();
        let down_index = index.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !hitbox_down.is_hovered(window) {
                return;
            }
            let at = down_index(event.position);
            let spot = (this.number, at);
            match event.button {
                MouseButton::Left => {
                    let mut state = this.state.borrow_mut();
                    let selection = state.selection.filter(|s| s.item == this.item);
                    state.selection = Some(match (event.click_count, selection) {
                        (1, Some(selection)) if event.modifiers.shift => Selection {
                            head: spot,
                            ..selection
                        },
                        (1, _) => Selection {
                            item: this.item,
                            anchor: spot,
                            head: spot,
                        },
                        (2, _) => {
                            let word = word_at(&text, at);
                            Selection {
                                item: this.item,
                                anchor: (this.number, word.start),
                                head: (this.number, word.end),
                            }
                        }
                        _ => Selection {
                            item: this.item,
                            anchor: (this.number, 0),
                            head: (this.number, text.len()),
                        },
                    });
                    state.dragging = true;
                    drop(state);
                    window.focus(&this.focus, cx);
                }
                MouseButton::Right => {
                    let target = (this.targets.iter())
                        .find(|(range, _)| range.contains(&at))
                        .map(|(_, target)| target.clone());
                    let mut state = this.state.borrow_mut();
                    // A right press outside the selection leaves it.
                    let inside = state.selected_in(this.item, this.number, text.len());
                    if !inside.is_some_and(|range| range.contains(&at)) {
                        state.selection = None;
                    }
                    drop(state);
                    (this.menu)(&(event.position, this.item, target), window, cx);
                }
                _ => return,
            }
            cx.stop_propagation();
            window.refresh();
        });
        let this = self.clone();
        let bounds = hitbox.bounds;
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble || event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let mut state = this.state.borrow_mut();
            if !state.dragging {
                return;
            }
            let Some(selection) = state.selection.as_mut().filter(|s| s.item == this.item) else {
                return;
            };
            // The piece the pointer is level with takes the selection's end;
            // above or below every piece, the selection keeps its last.
            let y = event.position.y;
            if bounds.top() <= y && y < bounds.bottom() {
                let head = (this.number, index(event.position));
                if selection.head != head {
                    selection.head = head;
                    drop(state);
                    window.refresh();
                }
            }
        });
        let state = self.state.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, _| {
            if phase == DispatchPhase::Bubble {
                state.borrow_mut().dragging = false;
            }
        });
    }
}
