//! A build tried out beside the owner's work (`cargo run -p dev -- try`) runs in
//! the background: no Dock icon, no ⌘Tab entry, no menu bar, never activated,
//! and its window opens behind every other window, so the owner's editor keeps
//! the screen and the keyboard. It prints the window's number, which
//! `screencapture -l` takes to capture the window while it stays behind.

use gpui::{ActivationPolicy, App, Window};

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

/// Shows `window`, which opened hidden, behind every other window, and prints
/// its number.
pub fn show_behind(window: &Window) {
    #[cfg(target_os = "macos")]
    match macos::order_back(window) {
        Some(number) => println!("window {number}"),
        None => eprintln!("can't reach the native window to show it behind the others"),
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

#[cfg(target_os = "macos")]
mod macos {
    use gpui::Window;
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// Orders the window in at the back and returns its window-server number.
    pub fn order_back(window: &Window) -> Option<isize> {
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
        else {
            return None;
        };
        // SAFETY: GPUI hands out its live content view, and this runs on the
        // main thread, inside the window's own update.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let native = view.window()?;
        native.orderBack(None);
        Some(native.windowNumber())
    }
}
