//! The sidebar toggle's glyph: a rounded frame holding a panel that narrows as
//! the sidebar closes.
//!
//! GPUI's SVGs are static, so it is drawn from quads in a 24-unit space,
//! scaled to the icon size.

use gpui::{Div, Hsla, Styled, div, prelude::*, px};
use theme::ICON_SIZE;

/// Width of the inner panel, in the glyph's 24 units, when the sidebar is open
/// and when it is closed.
const PANEL_OPEN: f32 = 5.5;
const PANEL_CLOSED: f32 = 1.75;

/// `open` is the sidebar's phase, 1 open and 0 closed. The glyph takes
/// `hover_color` while the element named `hover_group` is hovered.
pub fn sidebar_glyph(open: f32, color: Hsla, hover_group: &'static str, hover_color: Hsla) -> Div {
    let unit = ICON_SIZE.as_f32() / 24.;
    // `<rect x=3 y=4 w=18 h=16 rx=4 stroke-width=1.75>`: a centered stroke grows
    // the box by half the stroke on each side.
    let stroke = 1.75;
    let frame = div()
        .absolute()
        .left(px((3. - stroke / 2.) * unit))
        .top(px((4. - stroke / 2.) * unit))
        .w(px((18. + stroke) * unit))
        .h(px((16. + stroke) * unit))
        .rounded(px((4. + stroke / 2.) * unit))
        .border(px(stroke * unit))
        .border_color(color)
        .group_hover(hover_group, |frame| frame.border_color(hover_color));
    // `<rect x=6.5 y=7.5 w=5.5 h=9 rx=.875>`, narrowing to w=1.75.
    let width = PANEL_CLOSED + (PANEL_OPEN - PANEL_CLOSED) * open.clamp(0., 1.);
    let panel = div()
        .absolute()
        .left(px(6.5 * unit))
        .top(px(7.5 * unit))
        .w(px(width * unit))
        .h(px(9. * unit))
        .rounded(px(0.875 * unit))
        .bg(color)
        .group_hover(hover_group, |panel| panel.bg(hover_color));
    div()
        .relative()
        .flex_none()
        .size(ICON_SIZE)
        .child(frame)
        .child(panel)
}
