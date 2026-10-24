//! Live reload for the app: `cargo run -p dev`.
//!
//! Builds and starts the app, then watches the sources. After a change settles
//! it builds again; only a build that succeeds is started, so a mistake mid-edit
//! leaves the last good one on screen, and only one that changed the app, so a
//! change to tests alone, such as a scene, leaves it running as it is. Each new build takes the window over from
//! the one running (the app's `instance` module): it opens where the old window
//! is, and the old one quits only once the new one has drawn, so the window
//! never goes away and saves made in the old one reach the disk before the new
//! one reads it. A build that never draws leaves the old one running.
//!
//! Another launch, such as an agent's build, takes the window over too; once it
//! quits, the runner opens its own build again. If the app quits on its own, the
//! next change brings it back. Only the first launch comes to the front; one the
//! runner makes on its own opens behind the other windows.
//!
//! It runs only in the owner's checkout: in a linked git worktree, where a
//! session works on its own branch, it would put that branch's half-done
//! builds in the owner's window on every save. A session there uses `try`.
//!
//! `cargo run -p dev -- try` runs a build beside the owner's, in the
//! background; `cargo run -p dev -- install` puts a release build in
//! `/Applications`.

mod app;
#[cfg(target_os = "macos")]
mod install;
mod trial;

use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsStr,
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

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(root) = root.canonicalize() else {
        eprintln!("dev: can't find the workspace at {}", root.display());
        return ExitCode::FAILURE;
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => live_reload(&root),
        Some("try") => trial::run(&root, &args[1..]),
        #[cfg(target_os = "macos")]
        Some("install") => install::run(&root),
        Some(other) => {
            eprintln!(
                "dev: unknown command {other:?}; run it bare, with try, or with install on macOS"
            );
            ExitCode::FAILURE
        }
    }
}

fn live_reload(root: &Path) -> ExitCode {
    // A linked worktree has a `.git` file that points at the main checkout.
    if root.join(".git").is_file() {
        eprintln!(
            "dev: live reload runs only in the main checkout, as its builds take the \
             owner's window over; in a worktree, try a build with `cargo run -p dev -- try`"
        );
        return ExitCode::FAILURE;
    }
    let mut running = Running::default();
    if build(root) {
        running.launch(root);
    }
    let mut seen = sources(root);
    loop {
        sleep(POLL);
        running.follow(root);
        let now = sources(root);
        if now == seen {
            continue;
        }
        seen = settled(root, now);
        println!("dev: sources changed, building");
        let before = built(root);
        if !build(root) {
            println!("dev: the build failed; the running app stays");
            continue;
        }
        // A change only tests see, such as a scene, leaves the app as it was:
        // it keeps running rather than restart for nothing.
        if running.current.is_some() && built(root) == before {
            println!("dev: the app's build didn't change; it keeps running");
            continue;
        }
        running.launch(root);
    }
}

/// The builds the runner started that still run.
#[derive(Default)]
struct Running {
    /// The newest.
    current: Option<App>,
    /// Older ones handing the window over, or back if the new one fails.
    older: Vec<App>,
    /// Another launch has the window; the runner's build comes back once it quits.
    taken_over: bool,
    /// The runner has launched a build before, so the next one stays behind
    /// unless it takes over a window in front.
    launched: bool,
}

impl Running {
    fn launch(&mut self, root: &Path) {
        self.older.extend(self.current.take());
        self.taken_over = false;
        let env: &[(&str, &OsStr)] = if self.launched {
            &[("NERDA_STAY_BEHIND", OsStr::new("1"))]
        } else {
            &[]
        };
        self.launched = true;
        self.current = App::launch(root, env, &[]).or_else(|| self.older.pop());
    }

    /// Notices the builds that quit, and what that leaves running.
    fn follow(&mut self, root: &Path) {
        self.older.retain_mut(|app| app.exited().is_none());
        if let Some(status) = self.current.as_mut().and_then(App::exited) {
            self.current = self.older.pop();
            if self.current.is_some() {
                println!(
                    "dev: the new build quit ({status}) before it took over; the last one stays"
                );
            } else if instance_held() {
                println!(
                    "dev: another launch took over the window; the app comes back when it quits"
                );
                self.taken_over = true;
            } else {
                println!("dev: the app quit ({status}); the next change opens it again");
            }
        }
        if self.taken_over && self.current.is_none() && !instance_held() {
            println!("dev: the launch that took over quit; opening the app again");
            self.launch(root);
        }
    }
}

/// Whether some instance of the app holds the lock only a running one holds.
fn instance_held() -> bool {
    let Some(path) = workspace::default_instance_lock_path() else {
        return false;
    };
    // Held for a moment only: a launch that finds it taken tries again.
    fs::File::open(&path)
        .is_ok_and(|file| matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)))
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

/// When the app's build was last written: cargo leaves it alone when nothing
/// it is made of changed.
fn built(root: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(app::binary(root))
        .and_then(|meta| meta.modified())
        .ok()
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
