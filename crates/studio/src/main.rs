mod accessibility;
mod assets;
mod shell;
mod sidebar;
mod sidebar_glyph;
mod store;
mod system_theme;
mod text_input;
mod tooltip;
mod traffic_lights;
mod welcome;

use assets::Assets;
use gpui::{
    App, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use gpui_platform::application;
use shell::{NewSession, Shell, ToggleSidebar};

fn main() {
    application().with_assets(Assets).run(|cx: &mut App| {
        let (toggle_sidebar, new_session) = if cfg!(target_os = "macos") {
            ("cmd-b", "cmd-n")
        } else {
            ("ctrl-b", "ctrl-n")
        };
        cx.bind_keys([
            KeyBinding::new(toggle_sidebar, ToggleSidebar, None),
            KeyBinding::new(new_session, NewSession, None),
        ]);
        text_input::bind_keys(cx);
        sidebar::bind_keys(cx);
        let theme = system_theme::current(cx.window_appearance());
        cx.set_global(theme);
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
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
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        )
        .expect("the main window opens");
        cx.activate(true);
    });
}
