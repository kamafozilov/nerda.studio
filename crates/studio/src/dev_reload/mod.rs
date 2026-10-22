//! The app's side of live reload (`cargo run -p dev`). The runner starts the app
//! with `NERDA_DEV_STATE` naming a file, and talks to it over stdin and stdout.
//! Without `NERDA_DEV_STATE` none of this runs.
//!
//! A new build takes over without the window ever going away:
//!
//! 1. The runner sends `handoff`: the running app writes its pending save,
//!    stops saving, and answers `handed-off`.
//! 2. The runner starts the new build, which opens where the old window is,
//!    from the screen file, and answers `ready` once its first frame is drawn.
//! 3. The runner closes the old app's stdin, and the old app quits under the
//!    new window. If the new build never gets ready, `resume` lets the old app
//!    save again.

mod screen;

use std::{io::Write, path::PathBuf};

use gpui::{App, Entity, Window, WindowBounds};
pub use screen::Screen;

use crate::store::Store;

const STATE: &str = "NERDA_DEV_STATE";

/// The runner, while the app runs under it.
pub struct Link {
    /// The screen file the runner named.
    path: PathBuf,
    screen: Screen,
}

impl Link {
    /// Joins the runner, if it started the app, with the screen the build
    /// before this one left.
    pub fn join() -> Option<(Link, Option<Screen>)> {
        let path = PathBuf::from(std::env::var_os(STATE)?);
        let left = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| screen::parse(&text));
        let screen = left.unwrap_or(Screen {
            bounds: WindowBounds::default(),
            sidebar_open: true,
            active: true,
        });
        Some((Link { path, screen }, left))
    }

    /// Follows the runner's requests until it lets go of stdin, then quits.
    pub fn listen(&self, store: Entity<Store>, cx: &mut App) {
        cx.spawn(async move |cx| {
            loop {
                // Blocks a background thread until the runner says something.
                let line = cx
                    .background_executor()
                    .spawn(async {
                        let mut line = String::new();
                        match std::io::stdin().read_line(&mut line) {
                            Ok(0) | Err(_) => None,
                            Ok(_) => Some(line),
                        }
                    })
                    .await;
                let Some(line) = line else {
                    cx.update(|cx| cx.quit());
                    return;
                };
                match line.trim() {
                    "handoff" => {
                        store.update(cx, |store, _| store.hand_off());
                        answer("handed-off");
                    }
                    "resume" => store.update(cx, |store, cx| store.resume(cx)),
                    _ => {}
                }
            }
        })
        .detach();
    }

    /// Tells the runner the window is on screen, once its first frame is drawn.
    pub fn announce_ready(window: &Window) {
        window.on_next_frame(|window, _| window.on_next_frame(|_, _| answer("ready")));
    }

    pub fn sidebar_open(&self) -> bool {
        self.screen.sidebar_open
    }

    pub fn set_bounds(&mut self, bounds: WindowBounds) {
        self.update(Screen {
            bounds,
            ..self.screen
        });
    }

    pub fn set_sidebar_open(&mut self, sidebar_open: bool) {
        self.update(Screen {
            sidebar_open,
            ..self.screen
        });
    }

    pub fn set_active(&mut self, active: bool) {
        self.update(Screen {
            active,
            ..self.screen
        });
    }

    fn update(&mut self, screen: Screen) {
        if screen == self.screen {
            return;
        }
        self.screen = screen;
        // A lost write costs only the next build's placement.
        std::fs::write(&self.path, screen::format(&screen)).ok();
    }
}

/// One line to the runner, prefixed so it can tell answers from the app's output.
fn answer(word: &str) {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "nerda-dev: {word}").ok();
    stdout.flush().ok();
}
