mod accessibility;
mod add_project;
mod assets;
mod dev_reload;
mod shell;
mod sidebar;
mod sidebar_glyph;
mod store;
mod system_theme;
mod tabs;
mod text_input;
mod tooltip;
mod traffic_lights;
mod welcome;

use assets::Assets;
use gpui::{
    App, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use gpui_platform::application;
use shell::{
    ActivateTab, CloseAllTabs, CloseOtherTabs, CloseTab, GoBack, GoForward, NewSession, NextTab,
    PreviousTab, Shell, ToggleSidebar,
};

fn main() {
    application().with_assets(Assets).run(|cx: &mut App| {
        bind_keys(cx);
        text_input::bind_keys(cx);
        sidebar::bind_keys(cx);
        add_project::bind_keys(cx);
        tabs::bind_keys(cx);
        let theme = system_theme::current(cx.window_appearance());
        cx.set_global(theme);
        // Under live reload, a new build opens where the last one was.
        let dev = dev_reload::Link::join();
        let left = dev.as_ref().and_then(|(_, left)| *left);
        let bounds = left.map_or_else(
            || WindowBounds::Windowed(Bounds::centered(None, size(px(1200.), px(800.)), cx)),
            |left| left.bounds,
        );
        cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                window_min_size: Some(size(px(720.), px(480.))),
                // Windows keeps its system titlebar until the app draws its own
                // caption buttons; macOS draws the traffic lights over our glass.
                titlebar: Some(TitlebarOptions {
                    title: Some("Nerda Studio".into()),
                    appears_transparent: cfg!(target_os = "macos"),
                    traffic_light_position: Some(traffic_lights::position()),
                }),
                // The shell's titlebar drags and zooms the window itself, so AppKit
                // takes no click in the top row: a double-click on a control there
                // stays the control's. No-op off macOS.
                app_owns_titlebar_drag: true,
                window_background: theme.window_background_appearance(),
                app_id: Some("studio.nerda".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Shell::new(dev.map(|(link, _)| link), window, cx)),
        )
        .expect("the main window opens");
        // A reload while the editor is in front leaves it there.
        if left.is_none_or(|left| left.active) {
            cx.activate(true);
        }
    });
}

/// The window's shortcuts: Cmd on macOS, Ctrl elsewhere.
fn bind_keys(cx: &mut App) {
    let key = |mac: &'static str, other: &'static str| {
        if cfg!(target_os = "macos") {
            mac
        } else {
            other
        }
    };
    cx.bind_keys([
        KeyBinding::new(key("cmd-b", "ctrl-b"), ToggleSidebar, None),
        KeyBinding::new(key("cmd-n", "ctrl-n"), NewSession, None),
        KeyBinding::new(key("cmd-t", "ctrl-t"), NewSession, None),
        KeyBinding::new(key("cmd-w", "ctrl-w"), CloseTab, None),
        KeyBinding::new(key("cmd-alt-t", "ctrl-alt-t"), CloseOtherTabs, None),
        KeyBinding::new(key("cmd-shift-w", "ctrl-shift-w"), CloseAllTabs, None),
        KeyBinding::new(key("cmd-shift-]", "ctrl-shift-]"), NextTab, None),
        KeyBinding::new(key("cmd-shift-[", "ctrl-shift-["), PreviousTab, None),
        KeyBinding::new(key("cmd-}", "ctrl-}"), NextTab, None),
        KeyBinding::new(key("cmd-{", "ctrl-{"), PreviousTab, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
        KeyBinding::new(key("cmd-[", "ctrl-["), GoBack, None),
        KeyBinding::new(key("cmd-]", "ctrl-]"), GoForward, None),
    ]);
    // The first eight tabs by number, and the last by 9.
    let digits = [
        key("cmd-1", "ctrl-1"),
        key("cmd-2", "ctrl-2"),
        key("cmd-3", "ctrl-3"),
        key("cmd-4", "ctrl-4"),
        key("cmd-5", "ctrl-5"),
        key("cmd-6", "ctrl-6"),
        key("cmd-7", "ctrl-7"),
        key("cmd-8", "ctrl-8"),
    ];
    cx.bind_keys(
        digits
            .into_iter()
            .enumerate()
            .map(|(index, keys)| KeyBinding::new(keys, ActivateTab(index), None))
            .chain([KeyBinding::new(
                key("cmd-9", "ctrl-9"),
                ActivateTab(ActivateTab::LAST),
                None,
            )]),
    );
}
