//! A build tried out beside the owner's work (`cargo run -p dev -- try`) runs in
//! the background: no Dock icon, no ⌘Tab entry, no menu bar, never activated,
//! and its window opens under the desktop picture, where no screen shows it, so
//! the owner's screens and keyboard stay theirs. It prints the window's number,
//! which `screencapture -l` takes to capture the window where it is.

use gpui::{ActivationPolicy, App, Window};

use crate::native_window::{self, Place};

/// Set by `cargo run -p dev -- try` unless it runs with `--front`.
const BACKGROUND: &str = "NERDA_BACKGROUND";

/// Whether this launch asked to stay in the background.
pub fn requested() -> bool {
    std::env::var_os(BACKGROUND).is_some()
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
