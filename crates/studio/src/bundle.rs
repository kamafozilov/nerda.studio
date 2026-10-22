//! A build started outside an app bundle, as `cargo run`, the live-reload
//! runner and `target/debug/studio` start it, puts itself in one beside it and
//! runs from there, so macOS knows it as Nerda Studio Dev: its name in the
//! menu bar, its Dock icon, ⌘Tab. The process stays the same (same id, stdin,
//! stdout and environment), so whoever started it still holds it.

use std::{env, os::unix::process::CommandExt, process::Command};

use app_bundle::DEV;

/// Returns only if the app already runs from a bundle or can't get into one.
pub fn run_from_bundle() {
    let Ok(executable) = env::current_exe() else {
        return;
    };
    if app_bundle::is_bundled(&executable) {
        return;
    }
    let app = app_bundle::beside(&executable, &DEV);
    let error = match app_bundle::assemble(&executable, &app, &DEV, env!("CARGO_PKG_VERSION")) {
        // `exec` returns only if it failed.
        Ok(inside) => Command::new(inside).args(env::args_os().skip(1)).exec(),
        Err(error) => error,
    };
    eprintln!(
        "can't run from {}: {error}; running outside a bundle",
        app.display()
    );
}
