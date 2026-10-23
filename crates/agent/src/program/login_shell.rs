//! The PATH the person's login shell sets up.
//!
//! An app opened from Finder or the Dock never runs the shell's startup
//! files, so its PATH lacks what they add: version managers, a custom npm
//! prefix, `~/.zshrc` exports. Asking the shell itself covers any setup: it
//! runs once as an interactive login shell, prints its environment between
//! two markers, and its PATH is kept for the rest of the app's run.
//!
//! Startup files can misbehave, so:
//! - `-lic` comes first, since version managers load from interactive files;
//!   `-lc` follows when that prints nothing, as for files that hang or start
//!   a terminal multiplexer when interactive.
//! - The output is read on a thread, and the wait ends once the end marker
//!   shows, so a file that blocks after printing, or a process that inherits
//!   the pipe, can't hold it up.
//! - Each attempt has a deadline, after which the shell is killed.

use std::ffi::{OsStr, OsString};
use std::sync::OnceLock;

static CACHE: OnceLock<Option<OsString>> = OnceLock::new();

/// The login shell's PATH, read once and kept, a failure included. `None`
/// off Unix, without a usable shell, or when the shell printed none in time.
pub(crate) fn path() -> Option<&'static OsStr> {
    #[cfg(unix)]
    {
        CACHE.get_or_init(unix::capture).as_deref()
    }
    #[cfg(not(unix))]
    {
        CACHE.get_or_init(|| None).as_deref()
    }
}

#[cfg(unix)]
mod unix {
    use std::ffi::OsString;
    use std::io::Read;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::os::unix::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::{Duration, Instant};

    const BEGIN: &str = "__NERDA_SHELL_ENV_BEGIN__";
    const END: &str = "__NERDA_SHELL_ENV_END__";
    /// Enough for any sane environment; a runaway startup file can't fill memory.
    const MAX_OUTPUT: usize = 2 * 1024 * 1024;
    const ATTEMPT: Duration = Duration::from_secs(5);
    /// Once the shell exits, how long its output has to arrive.
    const FLUSH: Duration = Duration::from_millis(250);

    pub(super) fn capture() -> Option<OsString> {
        snapshot_path(&user_shell()?, ATTEMPT)
    }

    /// `$SHELL`, then the account's shell, then the usual ones; one that isn't
    /// executable, or refuses logins, is skipped.
    fn user_shell() -> Option<PathBuf> {
        let mut candidates = Vec::new();
        candidates.extend(
            std::env::var_os("SHELL")
                .filter(|shell| !shell.is_empty())
                .map(PathBuf::from),
        );
        candidates.extend(account_shell());
        candidates.extend(["/bin/zsh", "/bin/bash", "/bin/sh"].map(PathBuf::from));
        candidates.into_iter().find(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            let refuses = matches!(name.as_deref(), Some("nologin" | "false") | None);
            !refuses && super::super::is_executable(path)
        })
    }

    /// The shell the account database names, for an app started without `$SHELL`.
    fn account_shell() -> Option<PathBuf> {
        // SAFETY: getpwuid's entry is read at once and copied; the cache's
        // initialization runs this on one thread at a time.
        unsafe {
            let entry = libc::getpwuid(libc::getuid());
            if entry.is_null() || (*entry).pw_shell.is_null() {
                return None;
            }
            let shell = std::ffi::CStr::from_ptr((*entry).pw_shell).to_bytes();
            (!shell.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(shell)))
        }
    }

    /// The flags to try, most loaded first. csh and tcsh refuse `-l` with
    /// `-c`; fish reads its config on every start, so `-l` alone loads it all.
    fn attempts(shell: &Path) -> &'static [&'static [&'static str]] {
        let name = shell
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        match name.as_deref().unwrap_or_default() {
            "csh" | "tcsh" => &[&["-c"]],
            "fish" => &[&["-l", "-c"], &["-c"]],
            _ => &[&["-l", "-i", "-c"], &["-l", "-c"]],
        }
    }

    /// The PATH `shell` prints, trying each set of flags until one gives one.
    pub(super) fn snapshot_path(shell: &Path, deadline: Duration) -> Option<OsString> {
        let script = format!("echo {BEGIN}; env; echo {END}");
        attempts(shell)
            .iter()
            .find_map(|flags| parse(&run(shell, flags, &script, deadline)))
    }

    /// The shell's output up to the end marker, its exit, or the deadline,
    /// whichever comes first.
    fn run(shell: &Path, flags: &[&str], script: &str, deadline: Duration) -> Vec<u8> {
        let mut command = std::process::Command::new(shell);
        command
            .args(flags)
            .arg(script)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            // Quiets prompts and themes that draw for a real terminal.
            .env("TERM", "dumb")
            // Its own group, so a deadline ends what its startup files started too.
            .process_group(0);
        let Ok(mut child) = crate::spawning::spawn(&mut command) else {
            return Vec::new();
        };
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        if let Some(mut stdout) = child.stdout.take() {
            let output = Arc::clone(&output);
            let _ = std::thread::Builder::new()
                .name("login shell".to_owned())
                .spawn(move || {
                    let mut chunk = [0u8; 8192];
                    while let Ok(read @ 1..) = stdout.read(&mut chunk) {
                        let mut output = output.lock().unwrap_or_else(PoisonError::into_inner);
                        if output.len() >= MAX_OUTPUT {
                            break;
                        }
                        output.extend_from_slice(&chunk[..read]);
                    }
                });
        }
        let give_up = Instant::now() + deadline;
        let mut exited_at: Option<Instant> = None;
        let mut scanned: usize = 0;
        loop {
            {
                let output = output.lock().unwrap_or_else(PoisonError::into_inner);
                let from = scanned.saturating_sub(END.len());
                if find(&output[from..], END.as_bytes()).is_some() {
                    break;
                }
                scanned = output.len();
            }
            match exited_at {
                Some(at) if at.elapsed() >= FLUSH => break,
                Some(_) => {}
                None => match child.try_wait() {
                    Ok(Some(_)) => exited_at = Some(Instant::now()),
                    Ok(None) if Instant::now() >= give_up => break,
                    Ok(None) => {}
                    Err(_) => break,
                },
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if exited_at.is_none() {
            // SAFETY: signals the group the shell leads, which nothing else is in.
            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
            let _ = child.wait();
        }
        let output = output.lock().unwrap_or_else(PoisonError::into_inner);
        output.clone()
    }

    /// PATH from the environment between the last begin marker and the end
    /// marker after it: a startup file that prints a marker itself prints it
    /// before the real one.
    fn parse(output: &[u8]) -> Option<OsString> {
        let begin = rfind(output, BEGIN.as_bytes())?;
        let after = &output[begin + BEGIN.len()..];
        let end = find(after, END.as_bytes())?;
        after[..end].split(|&byte| byte == b'\n').find_map(|line| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let path = line.strip_prefix(b"PATH=")?;
            (!path.is_empty()).then(|| OsString::from_vec(path.to_vec()))
        })
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .rposition(|window| window == needle)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn fake_shell(dir: &Path, body: &str) -> PathBuf {
            let path = dir.join("fake-shell");
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        }

        /// Skips the flags and runs what follows `-c`.
        const RUN_SCRIPT: &str = r#"
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-c" ]; then shift; exec /bin/sh -c "$1"; fi
  shift
done
exit 1
"#;

        #[test]
        fn the_path_comes_from_between_the_markers() {
            let output = format!(
                "startup noise\n{BEGIN}\nHOME=/home/u\nPATH=/custom/bin:/usr/bin\nX=y\n{END}\ntrailing"
            );
            assert_eq!(parse(output.as_bytes()).unwrap(), "/custom/bin:/usr/bin");
            let echoed = format!("{BEGIN}\nnoise\n{BEGIN}\nPATH=/real/bin\n{END}\n");
            assert_eq!(parse(echoed.as_bytes()).unwrap(), "/real/bin");
        }

        #[test]
        fn the_shell_s_own_path_is_read() {
            let dir = tempfile::tempdir().unwrap();
            let body = format!(
                "#!/bin/sh\nPATH=\"/login/custom/bin:/usr/bin:/bin\"; export PATH\n{RUN_SCRIPT}"
            );
            let shell = fake_shell(dir.path(), &body);
            let path = snapshot_path(&shell, Duration::from_secs(10)).unwrap();
            assert!(
                path.to_string_lossy().starts_with("/login/custom/bin:"),
                "{path:?}"
            );
        }

        #[test]
        fn a_shell_that_hangs_when_interactive_is_asked_again_without() {
            let dir = tempfile::tempdir().unwrap();
            let body = format!(
                "#!/bin/sh\ncase \" $* \" in *\" -i \"*) sleep 60;; esac\nPATH=\"/login/fallback/bin:/usr/bin:/bin\"; export PATH\n{RUN_SCRIPT}"
            );
            let shell = fake_shell(dir.path(), &body);
            let started = Instant::now();
            let path = snapshot_path(&shell, Duration::from_millis(400)).unwrap();
            assert!(
                path.to_string_lossy().starts_with("/login/fallback/bin"),
                "{path:?}"
            );
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                started.elapsed()
            );
        }

        #[test]
        fn a_shell_that_never_answers_gives_nothing_by_the_deadline() {
            let dir = tempfile::tempdir().unwrap();
            let shell = fake_shell(dir.path(), "#!/bin/sh\nsleep 60\n");
            let started = Instant::now();
            assert!(snapshot_path(&shell, Duration::from_millis(300)).is_none());
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                started.elapsed()
            );
        }
    }
}
