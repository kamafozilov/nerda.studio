//! `cargo run -p dev -- try`: builds the app and runs it beside the owner's
//! instead of in its place, for an agent to prove a change on.
//!
//! The build gets a data folder of its own, so it neither takes the owner's
//! window over nor reads or saves the owner's workspace, and it runs in the
//! background (the app's `background` module): no Dock icon, no focus, its
//! window behind every other. It prints `window <number>` for
//! `screencapture -l`. It quits when this command ends, by Ctrl-C, a kill or
//! the time limit, so a forgotten build never stays open.
//!
//! - `--front` runs it as a regular app in front, for what only an active
//!   window or an on-screen region shows; it takes the focus.
//! - `--data <folder>` uses that data folder and keeps it, to start from a
//!   prepared, corrupt or newer `workspace.json`; otherwise a fresh folder
//!   under `target/try/` is removed afterwards.
//! - `--for <seconds>` sets the time limit, 900 by default.
//! - What follows `--` goes to the app, such as
//!   `-NSRequiresAquaSystemAppearance YES` for the light appearance.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    thread::sleep,
    time::{Duration, Instant},
};

use crate::{app::App, build};

const LIMIT: Duration = Duration::from_secs(900);
const POLL: Duration = Duration::from_millis(100);
/// How long the app gets to save and quit once the limit has passed.
const QUIT_WAIT: Duration = Duration::from_secs(5);
const SWEEP_AGE: Duration = Duration::from_secs(60);

struct Options {
    front: bool,
    data: Option<PathBuf>,
    limit: Duration,
    app_args: Vec<String>,
}

pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let options = match parse(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("dev try: {message}");
            eprintln!(
                "usage: cargo run -p dev -- try [--front] [--data <folder>] [--for <seconds>] [-- <app args>]"
            );
            return ExitCode::FAILURE;
        }
    };
    let trials = root.join("target/try");
    sweep(&trials);
    if !build(root) {
        eprintln!("dev try: the build failed");
        return ExitCode::FAILURE;
    }
    let (data, scratch) = match options.data {
        Some(data) => (data, false),
        None => (trials.join(std::process::id().to_string()), true),
    };
    if let Err(error) = fs::create_dir_all(&data) {
        eprintln!("dev try: can't make {}: {error}", data.display());
        return ExitCode::FAILURE;
    }
    let mut env = vec![("NERDA_DATA_DIR", data.as_os_str())];
    if !options.front {
        env.push(("NERDA_BACKGROUND", OsStr::new("1")));
    }
    let Some(mut app) = App::launch(root, &env, &options.app_args) else {
        return ExitCode::FAILURE;
    };
    println!(
        "dev try: running with data in {}, for {}s at most",
        data.display(),
        options.limit.as_secs()
    );
    let status = wait(&mut app, options.limit);
    if scratch {
        fs::remove_dir_all(&data).ok();
    }
    match status {
        Some(status) if status.success() => ExitCode::SUCCESS,
        Some(status) => {
            eprintln!("dev try: the app quit ({status})");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("dev try: the app didn't quit in time");
            ExitCode::FAILURE
        }
    }
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        front: false,
        data: None,
        limit: LIMIT,
        app_args: Vec::new(),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" => {
                options.app_args = args.cloned().collect();
                break;
            }
            "--front" => options.front = true,
            "--data" => {
                let folder = args.next().ok_or("--data needs a folder")?;
                options.data = Some(PathBuf::from(folder));
            }
            "--for" => {
                let seconds = args.next().ok_or("--for needs a number of seconds")?;
                let seconds = seconds
                    .parse()
                    .map_err(|_| format!("--for takes whole seconds, not {seconds:?}"))?;
                options.limit = Duration::from_secs(seconds);
            }
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    Ok(options)
}

/// Waits for the app to quit, and past `limit` makes it.
fn wait(app: &mut App, limit: Duration) -> Option<std::process::ExitStatus> {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if let Some(status) = app.exited() {
            return Some(status);
        }
        sleep(POLL);
    }
    println!("dev try: time is up; the app quits");
    app.let_go();
    let until = Instant::now() + QUIT_WAIT;
    while Instant::now() < until {
        if let Some(status) = app.exited() {
            return Some(status);
        }
        sleep(POLL);
    }
    None
}

/// Removes the scratch folders of trials that ended without cleaning up,
/// killed with this command: no instance holds their lock any more. A folder
/// younger than `SWEEP_AGE` may belong to a trial whose app is still starting.
fn sweep(trials: &Path) {
    let Ok(entries) = fs::read_dir(trials) else {
        return;
    };
    for entry in entries.flatten() {
        let folder = entry.path();
        let young = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age < SWEEP_AGE));
        let held = fs::File::open(folder.join("instance.lock"))
            .is_ok_and(|file| matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)));
        if !young && !held {
            fs::remove_dir_all(&folder).ok();
        }
    }
}
