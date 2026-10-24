//! A build tried out beside the owner's work (`cargo run -p dev -- try`) runs in
//! the background: no Dock icon, no ⌘Tab entry, no menu bar, never activated,
//! and its window opens under the desktop picture, where no screen shows it, so
//! the owner's screens and keyboard stay theirs. It prints the window's number,
//! which `screencapture -l` takes to capture the window where it is.
//!
//! macOS gives a window on no screen no frames, so the app asks for them
//! itself, as often as a display would: what animates, animates there, and
//! its cost can be measured, without bringing the window in front. What
//! only the front showed is told to it instead: `NERDA_TRY_ACTIVE` draws
//! the window as the active one, and `NERDA_TRY_DESKTOP` (`rrggbb`) the
//! desktop its glass shows. Nothing that would open outside the window, a
//! system picker or the Finder, opens from it.

use std::time::Duration;

use gpui::{ActivationPolicy, App, Hsla, Rgba, Window, WindowHandle, rgb};

use crate::native_window::{self, Place};

/// Set by `cargo run -p dev -- try` unless it runs with `--front`.
const BACKGROUND: &str = "NERDA_BACKGROUND";

/// Whether this launch asked to stay in the background.
pub fn requested() -> bool {
    std::env::var_os(BACKGROUND).is_some()
}

/// Draws the window as the active one, cursor and all.
const ACTIVE: &str = "NERDA_TRY_ACTIVE";
/// The desktop the glass shows, as `rrggbb`.
const DESKTOP: &str = "NERDA_TRY_DESKTOP";

/// Whether `window` draws as the active window: when it is, or when a
/// background build was told to.
pub fn active(window: &Window) -> bool {
    window.is_window_active() || (requested() && std::env::var_os(ACTIVE).is_some())
}

/// The desktop a background build's glass shows, when it was given one.
pub fn desktop() -> Option<Hsla> {
    if !requested() {
        return None;
    }
    let hex = std::env::var(DESKTOP).ok()?;
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).ok()?;
    Some(rgb(value).into())
}

/// `color` over the desktop a background build was given, as the window
/// server would composite it over the real one; `color` itself otherwise.
pub fn over_desktop(color: Hsla) -> Hsla {
    let Some(desktop) = desktop() else {
        return color;
    };
    let (top, under) = (Rgba::from(color), Rgba::from(desktop));
    let blend = |a: f32, b: f32| a * top.a + b * (1. - top.a);
    Rgba {
        r: blend(top.r, under.r),
        g: blend(top.g, under.g),
        b: blend(top.b, under.b),
        a: 1.,
    }
    .into()
}

/// Whether `what`, a system window or another app, may open: never from a
/// background build, where it would come up over the owner's work and take
/// their keyboard. It says so instead.
pub fn allows_system_ui(what: &str) -> bool {
    if requested() {
        println!("background: {what} doesn't open from a background build");
        return false;
    }
    true
}

/// Whether this launch may come on screen: refused when nobody at the Mac
/// asked for it. A launch in front, or one taking the running app's window
/// over, would take the owner's window, keyboard and mouse; it may come from
/// a person at a terminal, from the live-reload runner or `dev try`
/// (`NERDA_DEV_RUNNER`), from macOS opening the app (parent `launchd`), or
/// in the background. A program launching it any other way, such as an
/// agent's `cargo run -p studio`, is told to use `dev try`.
pub fn may_launch() -> bool {
    #[cfg(unix)]
    {
        use std::io::IsTerminal;
        let asked = requested()
            || std::env::var_os("NERDA_DEV_RUNNER").is_some()
            || std::io::stdin().is_terminal()
            || std::os::unix::process::parent_id() == 1;
        if !asked {
            eprintln!(
                "studio: launched by a program, not a person, it would take the owner's window, keyboard and mouse; run `cargo run -p dev -- try` to try a build in the background"
            );
        }
        asked
    }
    #[cfg(not(unix))]
    true
}

/// Keeps the app out of the Dock, ⌘Tab and the menu bar.
pub fn start(cx: &mut App) {
    cx.set_activation_policy(ActivationPolicy::Accessory);
}

/// Shows `window`, which opened hidden, under the desktop picture, and prints
/// its number.
pub fn show_hidden(window: &Window) {
    native_window::show(window, Place::UnderDesktop);
    if let Some(number) = native_window::number(window) {
        println!("window {number}");
    }
}

/// How often a background window draws what changed: a 60 Hz display's pace.
const FRAME: Duration = Duration::from_micros(16_667);

/// Draws `window` whenever something in it changed, a frame at a time, for
/// as long as it is open. A frame with nothing changed draws nothing.
pub fn drive_frames<V: 'static>(window: WindowHandle<V>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(FRAME).await;
            let Ok(view) = window.update(cx, |_, window, _| native_window::view(window)) else {
                break;
            };
            // Outside the window's update: drawing runs inside this call.
            native_window::display(view);
        }
    })
    .detach();
}
