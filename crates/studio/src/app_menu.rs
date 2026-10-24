//! The commands every Mac app answers to (Quit, Hide, Minimize, Zoom, Full
//! Screen) and the menu bar, which lists them with the app's own commands, each
//! beside its shortcut. Off macOS there is no menu bar and the window commands
//! keep their system shortcuts; Quit is Ctrl-Q.

use gpui::{App, Div, InteractiveElement, KeyBinding, Menu, MenuItem, OsAction, actions};

use crate::{
    shell::{
        AddProject, CloseAllTabs, CloseOtherTabs, CloseTab, GoBack, GoForward, NewSession, NextTab,
        OpenSettings, PreviousTab, RequestQuit, SearchSessions, ToggleSidebar,
    },
    text_input::keys::{Copy, Cut, Paste, SelectAll},
};

actions!(
    studio,
    [
        /// Quits at once: the menu's Quit, and Cmd-Q while no window is open.
        /// Cmd-Q in the window asks for a second press; see `RequestQuit`.
        Quit,
        /// Hides the app; a click on its Dock icon shows it again.
        Hide,
        /// Hides every other app.
        HideOthers,
        /// Shows the apps Hide Others hid.
        ShowAll,
        Minimize,
        Zoom,
        ToggleFullScreen,
    ]
);

const MAC: bool = cfg!(target_os = "macos");

/// Binds the app's commands and, on macOS, puts up the menu bar. Call after
/// every other `bind_keys`: a menu item shows the shortcut bound when it is built.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    // Inside the window Cmd-Q asks first. Quit's binding is shut out of the
    // window: one without a context would outrank the shell's from a text field
    // deeper down. It stays for the menu to show, and acts with no window open.
    let quit_key = if MAC { "cmd-q" } else { "ctrl-q" };
    let outside_window = format!("!{}", crate::shell::CONTEXT);
    cx.bind_keys([
        KeyBinding::new(quit_key, Quit, Some(&outside_window)),
        KeyBinding::new(quit_key, RequestQuit, Some(crate::shell::CONTEXT)),
    ]);
    if MAC {
        cx.bind_keys([
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("cmd-alt-h", HideOthers, None),
            KeyBinding::new("cmd-m", Minimize, None),
            KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        ]);
        cx.set_menus(menus());
    }
}

/// The window's own commands, for the window's root element: they act on the
/// window the action reached.
pub fn on_window_actions(root: Div) -> Div {
    root.on_action(|_: &Minimize, window, _| window.minimize_window())
        .on_action(|_: &Zoom, window, _| window.zoom_window())
        .on_action(|_: &ToggleFullScreen, window, _| window.toggle_fullscreen())
}

/// The menu bar. An item whose action nothing on screen handles is dimmed, so
/// Edit's items enable only in a text field.
fn menus() -> Vec<Menu> {
    vec![
        Menu::new("Nerda Studio").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", gpui::SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide Nerda Studio", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit Nerda Studio", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Session", NewSession),
            MenuItem::action("Add Project…", AddProject),
            MenuItem::separator(),
            MenuItem::action("Close Tab", CloseTab),
            MenuItem::action("Close Other Tabs", CloseOtherTabs),
            MenuItem::action("Close All Tabs", CloseAllTabs),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::action("Search Sessions", SearchSessions),
            MenuItem::separator(),
            MenuItem::action("Back", GoBack),
            MenuItem::action("Forward", GoForward),
            MenuItem::separator(),
            MenuItem::action("Toggle Full Screen", ToggleFullScreen),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
            MenuItem::separator(),
            MenuItem::action("Show Next Tab", NextTab),
            MenuItem::action("Show Previous Tab", PreviousTab),
        ]),
    ]
}
