//! The newest message at the top of the view. A message the person sends
//! goes to the top, where the first one stands, and its turn grows under
//! it. The list keeps room under the newest turn, as much as the view lacks
//! to show it at the top; while the turn is shorter than the view the list
//! holds still there, and once it grows past the view's end the list
//! follows the end again. The person's own scroll lets it go.

use std::{cell::Cell, rc::Rc};

use gpui::{Context, FollowMode, IntoElement, ListOffset, ListState, Pixels, Styled, canvas, px};
use theme::{SCROLL_EDGE_FADE, SPACE_2, SPACE_3};

use super::{
    Conversation, OVERDRAW,
    rows::{Row, Stamped},
};

/// The list's room above its first row.
pub const LIST_TOP: Pixels = SPACE_3;
/// The list's room under its last row, inside the scroll, so at the end it
/// stands clear of the composer and of the edge fade.
pub fn list_bottom() -> Pixels {
    SCROLL_EDGE_FADE + SPACE_2
}

/// What the list's room and its pin share with the frame that measures
/// them.
#[derive(Default)]
pub struct Pin {
    /// The room under the newest turn.
    pub room: Cell<Pixels>,
    /// Whether the newest message holds at the top of the view.
    pub held: Cell<bool>,
    /// The person scrolled since the last frame.
    pub scrolled: Cell<bool>,
}

pub type Shared = Rc<Pin>;

/// The space above the message in row `at`, the first's none.
pub fn message_space(at: usize) -> Pixels {
    if at == 0 { px(0.) } else { SPACE_3 * 2. }
}

/// Where the list stands with the newest message at its top: the bubble
/// where the first message's stands.
pub fn target(rows: &[Stamped]) -> Option<ListOffset> {
    let at = rows
        .iter()
        .rposition(|(row, _)| matches!(row, Row::Message { .. }))?;
    Some(ListOffset {
        item_ix: at,
        offset_in_item: message_space(at),
    })
}

impl Conversation {
    /// Whether the newest message holds at the top of the view.
    #[cfg(all(target_os = "macos", test))]
    pub fn holds_newest(&self) -> bool {
        self.pin.held.get()
    }

    /// Whether the list keeps to its end as the turn grows.
    #[cfg(all(target_os = "macos", test))]
    pub fn follows_end(&self) -> bool {
        self.list.is_following_tail()
    }

    /// Takes the list to the newest message and holds it there.
    pub(super) fn pin_newest(&mut self) {
        let Some(target) = target(&self.rows) else {
            return;
        };
        self.pin.held.set(true);
        self.list.set_follow_mode(FollowMode::Normal);
        self.list.scroll_to(target);
    }

    /// Measures the room under the newest turn once the list has laid out
    /// this frame, and keeps the newest message at the top while its turn
    /// fits under it: drawn after the list, it asks for another frame when
    /// the room or the scroll changed.
    pub(super) fn pin_watch(&self, _: &mut Context<Self>) -> impl IntoElement {
        let list = self.list.clone();
        let pin = self.pin.clone();
        let target = target(&self.rows);
        let room_at = self.rows.len().saturating_sub(1);
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let Some(target) = target else { return };
                if pin.scrolled.take() && pin.held.get() {
                    // Scrolled from the top, it stays where the person took
                    // it, and follows the end again once back there.
                    pin.held.set(false);
                    let top = list.logical_scroll_top();
                    list.set_follow_mode(FollowMode::Tail);
                    list.scroll_to(top);
                }
                if settle(&list, &pin, target, room_at) {
                    window.refresh();
                }
            },
        )
        .absolute()
        .size_0()
    }
}

/// Measures the room and moves the list to or from the pin; whether the
/// list needs to lay out again.
fn settle(list: &ListState, pin: &Pin, target: ListOffset, room_at: usize) -> bool {
    // The newest turn, from its message's top to the room: unknown while
    // the message is above the view, where the room keeps what it was.
    let Some(message) = list.bounds_for_item(target.item_ix) else {
        return false;
    };
    let view = list.viewport_bounds().size.height - LIST_TOP - list_bottom();
    // The list lays out no further than its overdraw past the view: a room
    // it hasn't reached is further down than the view is tall.
    let turn = match list.bounds_for_item(room_at) {
        Some(room) => room.top() - message.top() - target.offset_in_item,
        None => view + OVERDRAW,
    };
    let wanted = (view - turn).max(px(0.)).ceil();
    let mut again = false;
    if wanted != pin.room.get() {
        pin.room.set(wanted);
        list.remeasure_items(room_at..room_at + 1);
        again = true;
    }
    if !pin.held.get() && list.is_following_tail() && wanted > px(0.) {
        // At the end with room under the newest turn, the end is the pin:
        // holding it there keeps the room's growing and shrinking still.
        pin.held.set(true);
        list.set_follow_mode(FollowMode::Normal);
        again = true;
    }
    if pin.held.get() {
        if turn > view + px(1.) {
            pin.held.set(false);
            list.set_follow_mode(FollowMode::Tail);
            return true;
        }
        if again {
            list.scroll_to(target);
        }
    }
    again
}
