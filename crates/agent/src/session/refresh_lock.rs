//! Waiting out the agent's sign-in refresh before ending it.
//!
//! A claude.ai refresh token works once: each refresh replaces the pair, and
//! the agent saves the new pair only if the store still holds the token it
//! sent. An agent killed mid-refresh saves nothing, and on SIGTERM or its
//! input closing it waits only 2 s for a refresh in flight, so the person
//! would be signed out at the next refresh. The agent holds a lock while it
//! refreshes: `<config>/.oauth_refresh.lock`, a folder whose time it touches
//! every 5 s, and older versions `<real config path>.lock`. A session waits,
//! a bounded time, while either is fresh before it closes the agent's input
//! or signals it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const LOCK: &str = ".oauth_refresh.lock";
/// A lock untouched this long lost its holder, which holds nothing up.
const LIVE: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(150);
/// The longest wait: the agent's refresh request times out at 30 s, then it
/// fetches the profile and saves.
pub(crate) const SETTLE: Duration = Duration::from_secs(40);

/// The agent's sign-in folder, as the agent picks it from the environment:
/// `CLAUDE_SECURESTORAGE_CONFIG_DIR` (empty meaning the default), then
/// `CLAUDE_CONFIG_DIR`, then `~/.claude`.
pub(crate) fn config_dir() -> PathBuf {
    config_dir_from(
        std::env::var_os("CLAUDE_SECURESTORAGE_CONFIG_DIR"),
        std::env::var_os("CLAUDE_CONFIG_DIR"),
    )
}

fn config_dir_from(secure_storage: Option<OsString>, config: Option<OsString>) -> PathBuf {
    let default = || {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".claude")
    };
    match secure_storage {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        Some(_) => default(),
        None => config
            .filter(|dir| !dir.is_empty())
            .map_or_else(default, PathBuf::from),
    }
}

/// The current lock, then the one older versions take.
fn lock_paths(config_dir: &Path) -> [PathBuf; 2] {
    let real = std::fs::canonicalize(config_dir).unwrap_or_else(|_| config_dir.to_owned());
    let mut legacy = real.into_os_string();
    legacy.push(".lock");
    [config_dir.join(LOCK), PathBuf::from(legacy)]
}

fn age(path: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(
        SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default(),
    )
}

/// Whether an agent is refreshing the sign-in kept in `config_dir`.
pub(crate) fn refreshing(config_dir: &Path) -> bool {
    lock_paths(config_dir)
        .iter()
        .any(|path| age(path).is_some_and(|age| age < LIVE))
}

/// Waits, at most `longest`, until no agent is refreshing; false when one
/// still was at the end.
pub(crate) fn settle(config_dir: &Path, longest: Duration) -> bool {
    let give_up = Instant::now() + longest;
    while refreshing(config_dir) {
        if Instant::now() >= give_up {
            return false;
        }
        std::thread::sleep(POLL);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn age_by(path: &Path, by: Duration) {
        let folder = std::fs::File::open(path).unwrap();
        folder.set_modified(SystemTime::now() - by).unwrap();
    }

    #[test]
    fn the_folder_is_picked_as_the_agent_picks_it() {
        let home = dirs::home_dir().unwrap().join(".claude");
        assert_eq!(config_dir_from(None, None), home);
        assert_eq!(config_dir_from(None, Some("".into())), home);
        assert_eq!(
            config_dir_from(None, Some("/c".into())),
            PathBuf::from("/c")
        );
        assert_eq!(
            config_dir_from(Some("/s".into()), Some("/c".into())),
            PathBuf::from("/s")
        );
        assert_eq!(config_dir_from(Some("".into()), Some("/c".into())), home);
    }

    #[test]
    fn a_fresh_lock_is_waited_out_and_an_abandoned_one_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("claude");
        std::fs::create_dir_all(&config).unwrap();
        assert!(!refreshing(&config));

        let [current, legacy] = lock_paths(&config);
        std::fs::create_dir(&current).unwrap();
        assert!(refreshing(&config));
        let started = Instant::now();
        assert!(!settle(&config, Duration::from_millis(300)));
        assert!(started.elapsed() >= Duration::from_millis(300));

        age_by(&current, Duration::from_secs(30));
        assert!(!refreshing(&config));
        assert!(settle(&config, Duration::from_millis(300)));

        std::fs::create_dir(&legacy).unwrap();
        assert!(refreshing(&config), "older versions lock beside the folder");
    }

    #[test]
    fn the_wait_ends_once_the_lock_is_released() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("claude");
        std::fs::create_dir_all(&config).unwrap();
        let [current, _] = lock_paths(&config);
        std::fs::create_dir(&current).unwrap();
        let release = current.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            std::fs::remove_dir(release).unwrap();
        });
        let started = Instant::now();
        assert!(settle(&config, Duration::from_secs(5)));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
}
