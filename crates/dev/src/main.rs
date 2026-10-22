//! Live reload for the app: `cargo run -p dev`.
//!
//! Builds and starts the app, then watches the sources. After a change settles
//! it builds again; only a build that succeeds replaces the running app, so a
//! mistake mid-edit leaves the last good one on screen. The new build opens
//! where the old window is and the old one quits only once the new one has
//! drawn, so the window never goes away; saves made in the old one reach the
//! disk before the new one reads it. If the app quits on its own, the next
//! change brings it back.

mod app;

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    thread::sleep,
    time::{Duration, Instant},
};

use app::App;

/// How often the sources are checked.
const POLL: Duration = Duration::from_millis(250);
/// A change counts once the sources hold still this long: an editor saving many
/// files, or a formatter after it, gives one build.
const SETTLE: Duration = Duration::from_millis(300);
/// How long the running app gets to write its pending save.
const HANDOFF_WAIT: Duration = Duration::from_secs(2);
/// How long a new build gets to draw its first frame.
const READY_WAIT: Duration = Duration::from_secs(20);

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(root) = root.canonicalize() else {
        eprintln!("dev: can't find the workspace at {}", root.display());
        return ExitCode::FAILURE;
    };
    let state = root.join("target/dev/screen.txt");
    if let Some(folder) = state.parent()
        && let Err(error) = fs::create_dir_all(folder)
    {
        eprintln!("dev: can't create {}: {error}", folder.display());
        return ExitCode::FAILURE;
    }
    let mut app = build(&root).then(|| App::launch(&root, &state)).flatten();
    let mut seen = sources(&root);
    loop {
        sleep(POLL);
        if let Some(status) = app.as_mut().and_then(App::exited) {
            println!("dev: the app quit ({status}); the next change opens it again");
            app = None;
        }
        let now = sources(&root);
        if now == seen {
            continue;
        }
        seen = settled(&root, now);
        println!("dev: sources changed, building");
        if !build(&root) {
            println!("dev: the build failed; the running app stays");
            continue;
        }
        app = replace(app, &root, &state);
    }
}

/// Puts the new build on screen in place of `old`, which keeps running if the
/// new one never draws.
fn replace(old: Option<App>, root: &Path, state: &Path) -> Option<App> {
    let Some(mut old) = old else {
        return App::launch(root, state);
    };
    // An app that can't hand off its file quits first, so two never save at once.
    if !(old.ask("handoff") && old.wait_for("handed-off", HANDOFF_WAIT)) {
        old.quit();
        return App::launch(root, state);
    }
    match App::launch(root, state) {
        Some(new) if new.wait_for("ready", READY_WAIT) => {
            old.quit();
            Some(new)
        }
        new => {
            println!("dev: the new build didn't open; the running app stays");
            if let Some(new) = new {
                new.kill();
            }
            old.ask("resume");
            Some(old)
        }
    }
}

/// Waits until the sources stop changing and returns how they settled.
fn settled(root: &Path, mut last: u64) -> u64 {
    let mut since = Instant::now();
    while since.elapsed() < SETTLE {
        sleep(POLL.min(SETTLE));
        let now = sources(root);
        if now != last {
            last = now;
            since = Instant::now();
        }
    }
    last
}

/// One number for every source file's path, size and modification time.
fn sources(root: &Path) -> u64 {
    let mut files = Vec::new();
    collect(&root.join("crates"), &root.join("crates/dev"), &mut files);
    for file in ["Cargo.toml", "Cargo.lock"] {
        files.push(root.join(file));
    }
    files.sort();
    let mut hasher = DefaultHasher::new();
    for file in files {
        if let Ok(meta) = fs::metadata(&file) {
            file.hash(&mut hasher);
            meta.len().hash(&mut hasher);
            meta.modified().ok().hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Every file under `folder`, but not under `skip` (this runner) or hidden.
fn collect(folder: &Path, skip: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        if hidden || path == skip {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => collect(&path, skip, files),
            Ok(_) => files.push(path),
            Err(_) => {}
        }
    }
}

fn build(root: &Path) -> bool {
    // `cargo run` names the cargo that started this runner.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo)
        .args(["build", "-p", "studio"])
        .current_dir(root)
        .status()
        .is_ok_and(|status| status.success())
}
