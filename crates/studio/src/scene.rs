//! Scenes: the app's real window, opened in a test with no window on any
//! screen. GPUI's test platform runs it with the Mac's own text system and
//! Metal, so a scene draws every pixel the app would, clicks, types and resizes
//! it, and writes what it drew to `target/scenes/<scene>/<shot>.png`. Nothing
//! shows, nothing takes the keyboard, no permission is asked, and scenes run in
//! parallel with each other and with the app.
//!
//! What only the window server shows, the glass's blur of the desktop, the
//! traffic lights, the menu bar, full screen, stays with `dev try`.

use std::{
    cell::Cell,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use gpui::{
    App, AppContext, HeadlessAppContext, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, WindowAppearance, WindowHandle, point, px, size,
};
use image::RgbaImage;

use crate::{assets::Assets, shell::Shell, system_theme};

/// The system settings a scene runs under, in place of the Mac's.
#[derive(Clone, Copy, Debug, Default)]
pub struct Settings {
    pub dark: bool,
    pub reduce_motion: bool,
    pub reduce_transparency: bool,
}

thread_local! {
    /// Each scene runs on its own test thread, so each keeps its own.
    static SETTINGS: Cell<Settings> = Cell::new(Settings::default());
}

/// The settings of the scene on this thread.
pub fn settings() -> Settings {
    SETTINGS.get()
}

/// The appearance the scene on this thread asks for.
pub fn appearance() -> WindowAppearance {
    if settings().dark {
        WindowAppearance::Dark
    } else {
        WindowAppearance::Light
    }
}

pub struct Scene {
    cx: HeadlessAppContext,
    window: WindowHandle<Shell>,
    folder: PathBuf,
}

impl Scene {
    /// Opens the app at 1200 by 800 points with no saved data, light, with the
    /// sidebar open.
    pub fn open(name: &str) -> Self {
        Self::open_with(name, Settings::default(), |_| {})
    }

    /// Opens the app under `settings`, with the data folder `prepare` filled
    /// first: a saved, corrupt or newer `workspace.json`, say.
    pub fn open_with(name: &str, settings: Settings, prepare: impl FnOnce(&Path)) -> Self {
        SETTINGS.set(settings);
        let folder = scenes_folder().join(name);
        let data = folder.join("data");
        std::fs::remove_dir_all(&folder).ok();
        std::fs::create_dir_all(&data).expect("the scene's folder is made");
        prepare(&data);
        let text_system = Arc::new(gpui_apple::AppleTextSystem::new());
        let mut cx = HeadlessAppContext::with_platform(text_system, Arc::new(Assets), || {
            gpui_platform::current_headless_renderer()
        });
        let store = cx.update(|cx| crate::start(Some(data.join("workspace.json")), cx));
        let window = cx
            .open_window(size(px(1200.), px(800.)), |window, cx| {
                cx.new(|cx| Shell::new(store, true, window, cx))
            })
            .expect("the scene's window opens");
        let mut scene = Self { cx, window, folder };
        // In front, as the window a person works in: hover and clicks follow.
        scene
            .cx
            .update_window(window.into(), |_, window, _| window.activate_window())
            .ok();
        scene.settle();
        scene
    }

    /// Runs what is due and draws the frame.
    pub fn settle(&mut self) {
        self.cx.run_until_parked();
        self.cx
            .update_window(self.window.into(), |_, window, _| window.refresh())
            .ok();
        self.cx.run_until_parked();
    }

    /// Lets `ms` pass, on the timers and on the clock motion reads, and draws.
    pub fn wait(&mut self, ms: u64) {
        let duration = Duration::from_millis(ms);
        std::thread::sleep(duration);
        self.cx.advance_clock(duration);
        self.settle();
    }

    /// Presses keys, such as `"cmd-b"` or `"cmd-shift-] enter"`.
    pub fn keys(&mut self, keys: &'static str) {
        for key in keys.split(' ') {
            let keystroke = Keystroke::parse(key).expect("a keystroke such as cmd-b");
            self.cx
                .update_window(self.window.into(), |_, window, cx| {
                    window.dispatch_keystroke(keystroke, cx);
                })
                .ok();
            self.settle();
        }
    }

    /// Moves the pointer to `x`, `y` in points from the window's top left.
    pub fn hover(&mut self, x: f32, y: f32) {
        self.input(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));
    }

    /// Clicks at `x`, `y` in points from the window's top left.
    pub fn click(&mut self, x: f32, y: f32) {
        self.hover(x, y);
        let position = point(px(x), px(y));
        self.input(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
        self.input(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
    }

    fn input(&mut self, event: PlatformInput) {
        self.cx
            .update_window(self.window.into(), |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .ok();
        self.settle();
    }

    /// Resizes the window to `width` by `height` points.
    pub fn resize(&mut self, width: f32, height: f32) {
        self.cx
            .update_window(self.window.into(), |_, window, _| {
                window.resize(size(px(width), px(height)));
            })
            .ok();
        self.settle();
    }

    /// Changes the system settings mid-scene, as System Settings would.
    pub fn switch(&mut self, settings: Settings) {
        SETTINGS.set(settings);
        self.cx
            .update_window(self.window.into(), |_, window, cx| {
                cx.set_reduce_motion(settings.reduce_motion);
                if system_theme::refresh(window, cx) {
                    window.refresh();
                }
            })
            .ok();
        self.settle();
    }

    /// The scene's data folder.
    pub fn data(&self) -> PathBuf {
        self.folder.join("data")
    }

    /// Reads the shell.
    pub fn read<R>(&mut self, read: impl FnOnce(&Shell, &App) -> R) -> R {
        self.cx
            .read_window(&self.window, |shell, cx| read(shell.read(cx), cx))
            .expect("the scene's window is open")
    }

    /// Draws the frame and writes it to `target/scenes/<scene>/<label>.png`.
    pub fn shot(&mut self, label: &str) -> Shot {
        self.settle();
        let (image, width) = self
            .cx
            .update_window(self.window.into(), |_, window, _| {
                (window.render_to_image(), window.bounds().size.width)
            })
            .expect("the scene's window is open");
        let image = image.expect("the scene draws");
        let path = self.folder.join(format!("{label}.png"));
        image.save(&path).expect("the shot is written");
        Shot {
            scale: image.width() as f32 / f32::from(width),
            image,
        }
    }
}

/// One drawn frame.
pub struct Shot {
    pub image: RgbaImage,
    /// Pixels per point.
    pub scale: f32,
}

impl Shot {
    /// The color at `x`, `y` in points, as `[r, g, b, a]`.
    pub fn at(&self, x: f32, y: f32) -> [u8; 4] {
        let (x, y) = ((x * self.scale) as u32, (y * self.scale) as u32);
        self.image.get_pixel(x, y).0
    }
}

/// `target/scenes`, in the target folder the test was built in.
fn scenes_folder() -> PathBuf {
    let exe = std::env::current_exe().expect("the test knows where it runs");
    // `<target>/<profile>/deps/studio-<hash>`
    exe.ancestors()
        .nth(3)
        .expect("the test runs from a target folder")
        .join("scenes")
}
