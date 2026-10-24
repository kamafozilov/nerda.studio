//! Keeps the theme in step with the system: light or dark, glass or solid.

use gpui::{App, Window, WindowAppearance};
use theme::Theme;

use crate::accessibility;

/// The theme the system's settings call for now.
pub fn current(appearance: WindowAppearance) -> Theme {
    // A scene's appearance is its own, not the test window's.
    #[cfg(all(target_os = "macos", test))]
    let appearance = {
        let _ = appearance;
        crate::scene::appearance()
    };
    Theme::new(appearance, accessibility::reduce_transparency())
}

/// Re-reads the system's settings and, if they call for another theme, makes it
/// the app's and gives the window the matching material. Returns whether the
/// theme changed, so the caller knows to redraw.
pub fn refresh(window: &mut Window, cx: &mut App) -> bool {
    let theme = current(window.appearance());
    if cx.try_global::<Theme>() == Some(&theme) {
        return false;
    }
    window.set_background_appearance(theme.window_background_appearance());
    cx.set_global(theme);
    true
}
