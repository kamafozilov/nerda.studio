mod accessibility;
mod add_project;
mod app_menu;
mod assets;
mod dev_reload;
mod key_cap;
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
    App, AppContext, Bounds, Entity, Global, KeyBinding, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};
use gpui_platform::application;
use shell::{
    ActivateTab, CloseAllTabs, CloseOtherTabs, CloseTab, GoBack, GoForward, NewSession, NextTab,
    PreviousTab, Shell, ToggleSidebar,
};
use store::Store;
use theme::Theme;

fn main() {
    let app = application().with_assets(Assets);
    app.on_reopen(reopen);
    app.run(|cx: &mut App| {
        bind_keys(cx);
        text_input::bind_keys(cx);
        sidebar::bind_keys(cx);
        add_project::bind_keys(cx);
        tabs::bind_keys(cx);
        app_menu::init(cx);
        let theme = system_theme::current(cx.window_appearance());
        cx.set_global(theme);
        // The app keeps its projects while no window is open, so the window a
        // click on the Dock icon brings back shows them as they were.
        let store = cx.new(Store::load);
        cx.set_global(Projects(store.clone()));
        // Under live reload, a new build opens where the last one was.
        let dev = dev_reload::Link::join();
        let left = dev.as_ref().and_then(|(_, left)| *left);
        let bounds = left.map_or_else(|| centered(cx), |left| left.bounds);
        open_window(bounds, store, dev.map(|(link, _)| link), cx);
        // Elsewhere nothing brings a closed window back, so the app goes with it.
        if !cfg!(target_os = "macos") {
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
        }
        // A reload while the editor is in front leaves it there.
        if left.is_none_or(|left| left.active) {
            cx.activate(true);
        }
    });
}

/// The projects the windows show, kept while none is open.
struct Projects(Entity<Store>);

impl Global for Projects {}

/// A click on the Dock icon while no window is open opens one again.
fn reopen(cx: &mut App) {
    if !cx.windows().is_empty() {
        return;
    }
    let Some(Projects(store)) = cx.try_global::<Projects>() else {
        return;
    };
    open_window(centered(cx), store.clone(), None, cx);
    cx.activate(true);
}

/// Where a new window opens: centered on the main screen.
fn centered(cx: &App) -> WindowBounds {
    WindowBounds::Windowed(Bounds::centered(None, size(px(1200.), px(800.)), cx))
}

/// Opens the main window on `store`.
fn open_window(
    bounds: WindowBounds,
    store: Entity<Store>,
    dev: Option<dev_reload::Link>,
    cx: &mut App,
) {
    let theme = *Theme::of(cx);
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
        |window, cx| cx.new(|cx| Shell::new(store, dev, window, cx)),
    )
    .expect("the main window opens");
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
