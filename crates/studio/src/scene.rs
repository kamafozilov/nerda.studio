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
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    sync::{Arc, Once},
    time::Duration,
};

use gpui::{
    App, AppContext, Entity, HeadlessAppContext, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, ScrollDelta,
    ScrollEvent, WindowAppearance, WindowHandle, point, px, size,
};
use image::RgbaImage;
use workspace::{AddedProject, Timestamp, Workspace};

use crate::{agents, assets::Assets, shell::Shell, store::Store, system_theme};

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
    /// The program the scene's agents run in place of claude.
    static AGENT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// The program the agents of the scene on this thread run, if it set one.
pub fn agent_program() -> Option<PathBuf> {
    AGENT.with_borrow(Clone::clone)
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
    store: Entity<Store>,
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
        Self::open_at(name, settings, (1200., 800.), prepare)
    }

    /// Opens the app as `open_with` does, its window `size` points wide and
    /// tall: the way to see a window at another size, since GPUI's headless
    /// window takes a `resize` without laying out again.
    pub fn open_at(
        name: &str,
        settings: Settings,
        (width, height): (f32, f32),
        prepare: impl FnOnce(&Path),
    ) -> Self {
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
            .open_window(size(px(width), px(height)), |window, cx| {
                cx.new(|cx| Shell::new(store.clone(), true, window, cx))
            })
            .expect("the scene's window opens");
        let mut scene = Self {
            cx,
            window,
            store,
            folder,
        };
        // In front, as the window a person works in: hover and clicks follow.
        scene
            .cx
            .update_window(window.into(), |_, window, _| window.activate_window())
            .ok();
        scene.settle();
        scene
    }

    /// Opens the app on a session in a project whose agent is the agent
    /// crate's `fake-agent`, answering each message with the next of
    /// `replies`, as Markdown.
    pub fn conversation(name: &str, settings: Settings, replies: &[&str]) -> Self {
        Self::conversation_at(name, settings, (1200., 800.), replies)
    }

    /// A conversation as `conversation` opens it, in a window `size` points
    /// wide and tall.
    pub fn conversation_at(
        name: &str,
        settings: Settings,
        size: (f32, f32),
        replies: &[&str],
    ) -> Self {
        AGENT.set(Some(fake_agent()));
        Self::open_at(name, settings, size, |data| {
            let project = data.with_file_name("project");
            std::fs::create_dir_all(&project).expect("the project's folder is made");
            std::fs::write(project.join("fake-agent.jsonl"), script(replies))
                .expect("the agent's script is written");
            let now = Timestamp::now();
            let mut workspace = Workspace::default();
            let (AddedProject::Added(id) | AddedProject::Existing(id)) = workspace
                .add_project(&project, now)
                .expect("the project is added");
            let session = workspace.new_session(id, now).expect("a session opens");
            workspace.open_session(session);
            workspace
                .save(&data.join("workspace.json"))
                .expect("the workspace is saved");
        })
    }

    /// Sends `text` from the session on screen, as its composer does, and
    /// lets the agent answer.
    pub fn send(&mut self, text: &str) {
        let store = self.store.clone();
        self.cx.update(|cx| {
            let session =
                (store.read(cx).workspace().selected_session()).expect("a session is on screen");
            agents::of(session, cx)
                .update(cx, |agent, cx| agent.send(text.to_owned(), &[], cx))
                .expect("the agent starts");
        });
        self.settle();
    }

    /// Lets time pass until the agent of the session on screen has
    /// answered everything sent, up to five seconds, and the rows that came
    /// have made their entrance.
    pub fn wait_for_agent(&mut self) {
        let store = self.store.clone();
        for _ in 0..100 {
            self.wait(50);
            let busy = self.cx.update(|cx| {
                let session = store.read(cx).workspace().selected_session();
                session.is_some_and(|session| agents::of(session, cx).read(cx).is_busy())
            });
            if !busy {
                self.wait(400);
                return;
            }
        }
        panic!("the agent didn't answer in five seconds");
    }

    /// Scrolls by `dy` points at `x`, `y`, as a trackpad does: up is
    /// positive.
    pub fn scroll(&mut self, x: f32, y: f32, dy: f32) {
        self.input(PlatformInput::Scroll(ScrollEvent {
            position: point(px(x), px(y)),
            delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
            ..ScrollEvent::default()
        }));
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

    /// Presses and releases a key, such as `"space"`: what a focused control
    /// takes as a keyboard-activated click, which `keys` (down only) is not.
    pub fn press(&mut self, key: &'static str) {
        let keystroke = Keystroke::parse(key).expect("a keystroke such as space");
        self.input(PlatformInput::KeyDown(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        }));
        self.input(PlatformInput::KeyUp(KeyUpEvent { keystroke }));
        self.settle();
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

    /// Double-clicks at `x`, `y`, as two clicks the second of which counts two.
    pub fn double_click(&mut self, x: f32, y: f32) {
        self.click(x, y);
        let position = point(px(x), px(y));
        self.input(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 2,
            first_mouse: false,
        }));
        self.input(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 2,
        }));
    }

    /// Presses at `from`, moves to `to` in ten steps with the button down,
    /// and lets go there; `to` may lie outside the window.
    pub fn drag(&mut self, from: (f32, f32), to: (f32, f32)) {
        self.hover(from.0, from.1);
        self.input(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(from.0), px(from.1)),
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
        for step in 1..=10 {
            let t = step as f32 / 10.;
            self.input(PlatformInput::MouseMove(MouseMoveEvent {
                position: point(
                    px(from.0 + (to.0 - from.0) * t),
                    px(from.1 + (to.1 - from.1) * t),
                ),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::default(),
            }));
        }
        self.input(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(to.0), px(to.1)),
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

/// The agent crate's `fake-agent`, built once for the scenes, beside the
/// test in its target folder.
fn fake_agent() -> PathBuf {
    static BUILD: Once = Once::new();
    let profile = scenes_folder().with_file_name("debug");
    BUILD.call_once(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let built = std::process::Command::new(cargo)
            .args(["build", "--quiet", "-p", "agent", "--bin", "fake-agent"])
            .env(
                "CARGO_TARGET_DIR",
                profile.parent().expect("a target folder"),
            )
            .status()
            .is_ok_and(|status| status.success());
        assert!(built, "cargo build -p agent --bin fake-agent");
    });
    profile.join("fake-agent")
}

/// The fake agent's script: it starts as claude does, then answers each
/// message with the next reply.
fn script(replies: &[&str]) -> String {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures");
    let read = |name: &str| std::fs::read_to_string(fixtures.join(name)).expect("a fixture");
    let turn = read("turn-ok.jsonl");
    let mut script = read("initialize.jsonl");
    for reply in replies {
        let text = serde_json::to_string(reply).expect("a reply");
        script.push_str(&turn.replace(r#""text":"ok""#, &format!(r#""text":{text}"#)));
    }
    script
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
