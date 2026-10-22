//! Springs, described by a response (how quickly the value gets there, in seconds)
//! and a damping ratio (1 settles without overshoot).
//!
//! Springs rather than timed curves because a toggle pressed again mid-flight
//! turns back from where it is, keeping its speed.

use std::{f32::consts::TAU, time::Duration};

use gpui::SpringConfig;

/// A unit-mass spring with the given response and damping ratio.
const fn spring(response: f32, damping_ratio: f32) -> SpringConfig {
    let natural_frequency = TAU / response;
    SpringConfig::new(
        natural_frequency * natural_frequency,
        2. * damping_ratio * natural_frequency,
        1.,
    )
}

/// A panel sliding in, such as the sidebar opening. Critically damped: a click
/// carries no momentum to overshoot with.
pub const PANEL_ENTER: SpringConfig = spring(0.3, 1.);
/// A panel sliding out: quicker than its entrance, since nothing new arrives.
pub const PANEL_EXIT: SpringConfig = spring(0.25, 1.);

/// A tab's slot opening in the top row, or a dragged tab settling into its slot.
pub const TAB_ENTER: SpringConfig = spring(0.25, 1.);
/// A closed tab's slot collapsing before it leaves the row.
pub const TAB_EXIT: SpringConfig = spring(0.2, 1.);

/// Settling tolerance for a 0-to-1 phase, a quarter of a pixel on the sidebar.
pub const PHASE_EPSILON: f32 = 0.001;
/// Settling tolerance for a spring in pixels, such as a tab gliding into its
/// slot: a quarter of a pixel, so it stops asking for frames once nothing
/// visible moves.
pub const PIXEL_EPSILON: f32 = 0.25;

/// How long the pointer rests on a control before its tooltip shows.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
/// Once a tooltip hides, the next one within this time shows at once, so a
/// sweep along a row of icons reads each label without waiting, as on macOS.
pub const TOOLTIP_WARM: Duration = Duration::from_millis(300);
