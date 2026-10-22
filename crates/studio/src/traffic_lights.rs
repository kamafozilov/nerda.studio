//! Where the macOS traffic lights sit, so the window's top row can make room.
//!
//! GPUI puts the buttons' top edge `position().y` below the window's top and
//! sizes the titlebar to the button height plus that gap twice, so the buttons
//! center on the top row when the gap is half of what the buttons leave.

use gpui::{Pixels, Point, point, px};
use theme::{HEADER_HEIGHT, SPACE_3};

/// Height of AppKit's close, minimize and zoom buttons.
const BUTTON_HEIGHT: Pixels = px(14.);
const LEFT: Pixels = px(14.);
/// Right edge of the zoom button from the window's left edge, measured on
/// macOS 27: three 14 px buttons, 9 px apart.
const RIGHT: Pixels = px(74.);

pub fn position() -> Point<Pixels> {
    point(LEFT, (HEADER_HEIGHT - BUTTON_HEIGHT) / 2.)
}

/// Where the top row's own content may start, clear of the buttons.
pub fn content_start() -> Pixels {
    RIGHT + SPACE_3
}
