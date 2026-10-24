//! What the main window needs from AppKit that GPUI has no call for: its
//! number with the window server, and where it goes among other windows when
//! it opens hidden. Off macOS each is a no-op.

use gpui::Window;

/// The window's number with the window server: `screencapture -l` takes it,
/// and a window taking this one's place opens right above it.
pub fn number(window: &Window) -> Option<isize> {
    #[cfg(target_os = "macos")]
    return macos::ns_window(window).map(|native| native.windowNumber());
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        None
    }
}

/// Where a window that opened hidden goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// Right above the window with this number, so it takes that window's
    /// place among the others without coming forward.
    Above(isize),
    /// Behind every other app's window.
    Behind,
    /// Under the desktop picture, on no screen anyone sees, yet drawn and
    /// captured by `screencapture -l` as any other window.
    UnderDesktop,
}

/// Shows `window`, which opened hidden, at `place`, without taking the focus.
pub fn show(window: &Window, place: Place) {
    #[cfg(target_os = "macos")]
    macos::show(window, place);
    #[cfg(not(target_os = "macos"))]
    let _ = (window, place);
}

#[cfg(target_os = "macos")]
mod macos {
    use gpui::Window;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSView, NSWindow, NSWindowOrderingMode};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::Place;

    /// `kCGDesktopWindowLevelKey`: the desktop picture sits one level below
    /// the level this key names.
    const DESKTOP_LEVEL_KEY: i32 = 2;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowLevelForKey(key: i32) -> i32;
    }

    pub fn ns_window(window: &Window) -> Option<Retained<NSWindow>> {
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
        else {
            return None;
        };
        // SAFETY: GPUI hands out its live content view, and this runs on the
        // main thread, inside the window's own update.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        view.window()
    }

    pub fn show(window: &Window, place: Place) {
        let Some(native) = ns_window(window) else {
            eprintln!("can't reach the native window to place it");
            return;
        };
        match place {
            Place::Above(other) => {
                native.orderWindow_relativeTo(NSWindowOrderingMode::Above, other)
            }
            Place::Behind => native.orderBack(None),
            Place::UnderDesktop => {
                // SAFETY: a plain function of the window server, any thread.
                let desktop = unsafe { CGWindowLevelForKey(DESKTOP_LEVEL_KEY) };
                native.setLevel(desktop as isize - 2);
                native.orderBack(None);
            }
        }
    }
}
