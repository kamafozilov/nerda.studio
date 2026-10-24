mod accessibility;
mod add_project;
mod agents;
mod app_menu;
mod assets;
mod background;
#[cfg(target_os = "macos")]
mod bundle;
mod composer;
mod conversation;
mod empty_page;
mod instance;
mod kbd;
mod menu;
mod native_window;
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
    App, AppContext, Bounds, DisplayId, Entity, Global, KeyBinding, TitlebarOptions, WindowBounds,
    WindowHandle, WindowOptions, px, size,
};
use gpui_platform::application;
use instance::Screen;
use native_window::Place;
use shell::{
    ActivateTab, CloseAllTabs, CloseOtherTabs, CloseTab, GoBack, GoForward, NewSession, NextTab,
    OpenSettings, PreviousTab, SearchSessions, Shell, ToggleSidebar,
};
use store::Store;
use theme::Theme;

fn main() {
    #[cfg(target_os = "macos")]
    bundle::run_from_bundle();
    // Before anything reads the workspace: only one instance saves it.
    let claim = match instance::claim() {
        Ok(claim) => claim,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let app = application().with_assets(Assets);
    app.on_reopen(reopen);
    app.run(move |cx: &mut App| {
        if background::requested() {
            background::start(cx);
        }
        bind_keys(cx);
        text_input::bind_keys(cx);
        composer::bind_keys(cx);
        sidebar::bind_keys(cx);
        add_project::bind_keys(cx);
        menu::bind_keys(cx);
        app_menu::init(cx);
        let theme = system_theme::current(cx.window_appearance());
        cx.set_global(theme);
        // The app keeps its projects while no window is open, so the window a
        // click on the Dock icon brings back shows them as they were.
        let store = cx.new(Store::load);
        cx.set_global(Projects(store.clone()));
        agents::init(store.clone(), cx);
        // Taking over from a running instance, the window opens where its was;
        // otherwise where it was when the app last quit.
        let left = claim.screen();
        let screen = left.or(claim.remembered());
        let previous = claim.serve(store.clone(), cx);
        instance::follow_runner(cx);
        let (bounds, display) = screen
            .and_then(|screen| screen.place(cx))
            .unwrap_or_else(|| (centered(cx), None));
        let sidebar_open = screen.is_none_or(|screen| screen.sidebar_open);
        let arrival = arrival(left);
        let window = open_window(bounds, display, store, sidebar_open, arrival, cx);
        if let Some(previous) = previous {
            window
                .update(cx, |_, window, _| previous.ready_after_first_frame(window))
                .ok();
        }
        // Elsewhere nothing brings a closed window back, so the app goes with it.
        if !cfg!(target_os = "macos") {
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
        }
        if arrival == Arrival::Front {
            cx.activate(true);
        }
    });
}

/// How the window comes on screen.
#[derive(Clone, Copy, PartialEq)]
enum Arrival {
    /// In front, with the focus.
    Front,
    /// Without the focus, at a place among the other windows.
    Quiet(Place),
    /// Under the desktop picture, for `cargo run -p dev -- try`.
    Background,
}

/// A launch takes the front only when someone asked for the app: a takeover
/// while the editor is in front leaves it there, the new window taking the old
/// one's place among the others, and the live-reload runner bringing the app
/// back after it quit opens it behind them.
fn arrival(left: Option<Screen>) -> Arrival {
    if background::requested() {
        return Arrival::Background;
    }
    match left {
        Some(left) if left.active => Arrival::Front,
        Some(left) => Arrival::Quiet(left.window.map_or(Place::Behind, Place::Above)),
        None if instance::stays_behind() => Arrival::Quiet(Place::Behind),
        None => Arrival::Front,
    }
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
    open_window(centered(cx), None, store.clone(), true, Arrival::Front, cx);
    cx.activate(true);
}

/// Where a new window opens: centered on the main screen.
fn centered(cx: &App) -> WindowBounds {
    WindowBounds::Windowed(Bounds::centered(None, size(px(1200.), px(800.)), cx))
}

/// Opens the main window on `store`.
fn open_window(
    bounds: WindowBounds,
    display_id: Option<DisplayId>,
    store: Entity<Store>,
    sidebar_open: bool,
    arrival: Arrival,
    cx: &mut App,
) -> WindowHandle<Shell> {
    let theme = *Theme::of(cx);
    let front = arrival == Arrival::Front;
    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                display_id,
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
                // Anywhere but the front it opens hidden, then goes to its place.
                focus: front,
                show: front,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Shell::new(store, sidebar_open, window, cx)),
        )
        .expect("the main window opens");
    window
        .update(cx, |_, window, _| match arrival {
            Arrival::Front => {}
            Arrival::Quiet(place) => native_window::show(window, place),
            Arrival::Background => background::show_hidden(window),
        })
        .ok();
    window
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
        KeyBinding::new(key("cmd-k", "ctrl-k"), SearchSessions, None),
        KeyBinding::new(key("cmd-n", "ctrl-n"), NewSession, None),
        KeyBinding::new(key("cmd-t", "ctrl-t"), NewSession, None),
        KeyBinding::new(key("cmd-,", "ctrl-,"), OpenSettings, None),
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
