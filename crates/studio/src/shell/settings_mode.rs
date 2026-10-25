//! Settings takes the whole window: its menu in the sidebar's place, its
//! section in the card, and the tabs and Back and Forward out of the top row.
//! `Cmd-,`, the sidebar's Settings and the menu bar's open it; Back in its
//! menu, Escape, `Cmd-,` again and `Cmd-W` leave it, and so does anything
//! that shows a tab, such as a new session or a tab's shortcut. The tabs come
//! back as they were.

use gpui::{Context, Focusable, Window};

use super::Shell;

impl Shell {
    pub(super) fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.leave_settings(window, cx);
        } else {
            self.settings_open = true;
            (self.tabs).update(cx, |tabs, cx| tabs.dismiss_menu(window, cx));
            window.focus(&self.settings.focus_handle(cx), cx);
            cx.notify();
        }
    }

    /// Leaves Settings, if open, for the tab on screen; returns whether it was open.
    pub(super) fn leave_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.settings_open {
            return false;
        }
        self.settings_open = false;
        self.focus_shown(window, cx);
        cx.notify();
        true
    }

    #[cfg(all(target_os = "macos", test))]
    pub fn settings_open(&self) -> bool {
        self.settings_open
    }

    #[cfg(all(target_os = "macos", test))]
    pub fn settings(&self) -> &gpui::Entity<crate::settings::SettingsPage> {
        &self.settings
    }

    #[cfg(all(target_os = "macos", test))]
    pub fn preferences(&self, cx: &gpui::App) -> workspace::Preferences {
        self.store.read(cx).workspace().preferences().clone()
    }

    #[cfg(all(target_os = "macos", test))]
    pub fn quit_hint_shown(&self) -> bool {
        self.quit_hint.is_some()
    }

    #[cfg(all(target_os = "macos", test))]
    pub fn open_tabs(&self, cx: &gpui::App) -> Vec<workspace::Tab> {
        self.store.read(cx).workspace().open_tabs()
    }
}
