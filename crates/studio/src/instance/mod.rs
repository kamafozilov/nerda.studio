//! One instance of the app at a time, the newest launched. Two would each save
//! `workspace.json` over the other's changes, so only the instance holding the
//! lock on `instance.lock` in the data folder saves, and a new launch takes over
//! from it over `instance.sock`:
//!
//! 1. The new instance finds the lock held and asks the running one `handoff 1`.
//! 2. The running one writes its pending save, stops saving, lets go of the
//!    lock and answers `handed-off` with its screen: where its window is,
//!    whether the sidebar is open, whether it was in front.
//! 3. The new one takes the lock, loads the workspace, opens its window there
//!    and, once the window has drawn, says `ready`; the old one quits under it,
//!    so the window never goes away.
//!
//! If `ready` never comes, the old instance takes the lock back and saves
//! again, or quits if a newer one holds it by then. A launch that finds another
//! mid-handoff hears `busy` and tries again, so the last launch ends up on
//! screen. The lock is the kernel's: a crashed instance never leaves it held.
//!
//! Live reload (`cargo run -p dev`) is this same takeover: the runner starts
//! each new build, and the build takes over from whichever instance runs.
//!
//! Without Unix sockets (Windows, for now) a second launch can't ask for the
//! window, so it leaves the running instance alone and quits.

mod screen;

use std::{
    fs::{self, File, TryLockError},
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, Read},
    path::PathBuf,
    thread::sleep,
    time::{Duration, Instant},
};

use gpui::{App, Entity, Global, Window};
pub use screen::Screen;

use crate::{shell::Shell, store::Store};

/// How long a launch keeps asking before it gives up on a running instance
/// that holds the lock but doesn't answer.
#[cfg(unix)]
const GIVE_UP: Duration = Duration::from_secs(10);
const RETRY: Duration = Duration::from_millis(50);
/// How long a running instance that answered gets to let go of the lock.
#[cfg(unix)]
const LOCK_WAIT: Duration = Duration::from_secs(2);
/// How long an instance that handed off waits to take the lock back.
const RESUME_WAIT: Duration = Duration::from_millis(500);
/// Set by the live-reload runner on the builds it starts.
const RUNNER: &str = "NERDA_DEV_RUNNER";

/// The right to run, won at launch before the app starts.
pub struct Claim {
    /// `None` without a data folder: nothing is saved, so nothing is guarded.
    files: Option<Files>,
    lock: Option<File>,
    #[cfg(unix)]
    listener: Option<std::os::unix::net::UnixListener>,
    previous: Option<Previous>,
}

/// The instance this one took over from, waiting for `ready`.
pub struct Previous {
    #[cfg(unix)]
    stream: std::os::unix::net::UnixStream,
    screen: Option<Screen>,
}

#[derive(Debug)]
pub enum ClaimError {
    /// Another instance runs and this platform can't take over from it.
    #[cfg_attr(unix, allow(dead_code))]
    Running,
    /// Another instance holds the lock but never handed off.
    #[cfg_attr(not(unix), allow(dead_code))]
    NotAnswering,
}

impl std::fmt::Display for ClaimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ClaimError::Running => "Nerda Studio is already running",
            ClaimError::NotAnswering => {
                "Nerda Studio is already running and not responding; quit it and try again"
            }
        })
    }
}

/// Where the instance files are.
struct Files {
    lock: PathBuf,
    #[cfg_attr(not(unix), allow(dead_code))]
    socket: PathBuf,
}

impl Files {
    fn find() -> Option<Files> {
        let lock = workspace::default_instance_lock_path()?;
        let folder = lock.parent()?.to_path_buf();
        let socket = folder.join("instance.sock");
        // A socket's path fits in 103 bytes; a long home folder's goes to the
        // user's own temp folder, named for the data folder.
        let socket = if socket.as_os_str().len() <= 103 {
            socket
        } else {
            let mut hasher = DefaultHasher::new();
            folder.hash(&mut hasher);
            std::env::temp_dir().join(format!("nerda-studio-{:016x}.sock", hasher.finish()))
        };
        Some(Files { lock, socket })
    }

    fn open_lock(&self) -> io::Result<File> {
        if let Some(folder) = self.lock.parent() {
            fs::create_dir_all(folder)?;
        }
        File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock)
    }
}

/// Takes the lock, from the running instance if there is one. Blocks for up
/// to `GIVE_UP`; call it before the app starts.
pub fn claim() -> Result<Claim, ClaimError> {
    let unguarded = Claim {
        files: None,
        lock: None,
        #[cfg(unix)]
        listener: None,
        previous: None,
    };
    let Some(files) = Files::find() else {
        return Ok(unguarded);
    };
    let lock = match files.open_lock() {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("can't open {}: {error}", files.lock.display());
            return Ok(unguarded);
        }
    };
    match lock.try_lock() {
        Ok(()) => Ok(Claim::owner(files, lock, None)),
        Err(TryLockError::WouldBlock) => take_over(files, lock),
        // A file system without locks: run unguarded rather than not at all.
        Err(TryLockError::Error(error)) => {
            eprintln!("can't lock {}: {error}", files.lock.display());
            Ok(unguarded)
        }
    }
}

/// Asks the running instance to hand off until it does or `GIVE_UP` passes.
#[cfg(unix)]
fn take_over(files: Files, lock: File) -> Result<Claim, ClaimError> {
    let until = Instant::now() + GIVE_UP;
    loop {
        // It quit meanwhile, or another launch took the lock and this one
        // takes over from that one.
        if lock.try_lock().is_ok() {
            return Ok(Claim::owner(files, lock, None));
        }
        if let Ok(previous) = socket::ask(&files.socket)
            && take(&lock, LOCK_WAIT)
        {
            return Ok(Claim::owner(files, lock, Some(previous)));
        }
        if Instant::now() >= until {
            return Err(ClaimError::NotAnswering);
        }
        sleep(RETRY);
    }
}

#[cfg(not(unix))]
fn take_over(_: Files, _: File) -> Result<Claim, ClaimError> {
    Err(ClaimError::Running)
}

/// Tries the lock until `wait` passes.
#[cfg_attr(not(unix), allow(dead_code))]
fn take(lock: &File, wait: Duration) -> bool {
    let until = Instant::now() + wait;
    loop {
        if lock.try_lock().is_ok() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        sleep(RETRY);
    }
}

impl Claim {
    fn owner(files: Files, lock: File, previous: Option<Previous>) -> Claim {
        Claim {
            #[cfg(unix)]
            listener: socket::bind(&files.socket),
            files: Some(files),
            lock: Some(lock),
            previous,
        }
    }

    /// What the instance this one took over from had on screen.
    pub fn screen(&self) -> Option<Screen> {
        self.previous.as_ref().and_then(|previous| previous.screen)
    }

    /// Starts answering later launches, and returns the instance this one took
    /// over from, which waits for [`Previous::ready_after_first_frame`].
    pub fn serve(self, store: Entity<Store>, cx: &mut App) -> Option<Previous> {
        let Claim {
            files,
            lock,
            #[cfg(unix)]
            listener,
            previous,
        } = self;
        if let Some(files) = files {
            cx.set_global(Instance {
                files,
                lock,
                handing_off: false,
                store,
            });
            #[cfg(unix)]
            if let Some(listener) = listener {
                socket::accept(listener, cx);
            }
        }
        previous
    }
}

impl Previous {
    /// Lets the old instance quit once `window` has drawn its first frame.
    pub fn ready_after_first_frame(self, window: &Window) {
        window.on_next_frame(|window, _| {
            window.on_next_frame(move |_, _| {
                #[cfg(unix)]
                socket::write_line(&self.stream, "ready").ok();
                #[cfg(not(unix))]
                drop(self);
            })
        });
    }
}

/// The running instance's side, from `Claim::serve` on.
struct Instance {
    files: Files,
    /// `None` while handing off: the new instance holds it.
    lock: Option<File>,
    handing_off: bool,
    store: Entity<Store>,
}

impl Global for Instance {}

/// Step 2: writes the pending save, stops saving and lets go of the lock.
/// Returns the screen to hand over, or `None` if a handoff is under way.
#[cfg_attr(not(unix), allow(dead_code))]
fn hand_off(cx: &mut App) -> Option<Option<Screen>> {
    let instance = cx.global_mut::<Instance>();
    if instance.handing_off {
        return None;
    }
    instance.handing_off = true;
    let store = instance.store.clone();
    store.update(cx, |store, _| store.hand_off());
    let screen = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<Shell>())
        .and_then(|window| {
            window
                .update(cx, |shell, window, _| shell.screen(window))
                .ok()
        });
    // Only after the last save is on disk, so the new instance loads it.
    cx.global_mut::<Instance>().lock = None;
    Some(screen)
}

/// The new instance never took over: saves again if the lock is still free,
/// else quits, as a newer instance owns the data.
#[cfg_attr(not(unix), allow(dead_code))]
fn resume(cx: &mut App) {
    let instance = cx.global::<Instance>();
    let lock = instance
        .files
        .open_lock()
        .ok()
        .filter(|lock| take(lock, RESUME_WAIT));
    let Some(lock) = lock else {
        cx.quit();
        return;
    };
    #[cfg(unix)]
    let listener = socket::bind(&instance.files.socket);
    let store = instance.store.clone();
    let instance = cx.global_mut::<Instance>();
    instance.lock = Some(lock);
    instance.handing_off = false;
    store.update(cx, |store, cx| store.resume(cx));
    #[cfg(unix)]
    if let Some(listener) = listener {
        socket::accept(listener, cx);
    }
}

/// Under the live-reload runner the app quits when the runner lets go of its
/// stdin, so Ctrl-C there stops both, with the last save written.
pub fn follow_runner(cx: &mut App) {
    if std::env::var_os(RUNNER).is_none() {
        return;
    }
    cx.spawn(async move |cx| {
        cx.background_executor()
            .spawn(async { io::stdin().read_to_end(&mut Vec::new()) })
            .await
            .ok();
        cx.update(|cx| cx.quit());
    })
    .detach();
}

#[cfg(unix)]
mod socket;
