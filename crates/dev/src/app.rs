//! One running build of the app and the lines it trades with the runner; the
//! app's side is `crates/studio/src/dev_reload`.

use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{Receiver, RecvTimeoutError, channel},
    thread,
    time::{Duration, Instant},
};

/// Marks the app's answers on its stdout; every other line is its own output.
const ANSWER: &str = "nerda-dev: ";
/// How long the app gets to quit before it is killed.
const QUIT_GRACE: Duration = Duration::from_secs(3);

pub struct App {
    child: Child,
    stdin: Option<ChildStdin>,
    answers: Receiver<String>,
}

impl App {
    pub fn launch(root: &Path, state: &Path) -> Option<App> {
        let binary = root
            .join("target/debug/studio")
            .with_extension(std::env::consts::EXE_EXTENSION);
        let mut command = Command::new(&binary);
        command
            .env("NERDA_DEV_STATE", state)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        // Its own process group, so Ctrl-C stops the runner alone and the app
        // quits on the closed stdin, its saves written, instead of dying mid-save.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                eprintln!("dev: can't start {}: {error}", binary.display());
                return None;
            }
        };
        let stdin = child.stdin.take();
        let stdout = child.stdout.take()?;
        let (send, answers) = channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                match line.strip_prefix(ANSWER) {
                    Some(word) => {
                        send.send(word.to_owned()).ok();
                    }
                    None => println!("{line}"),
                }
            }
        });
        Some(App {
            child,
            stdin,
            answers,
        })
    }

    /// The app's exit, once it has quit on its own.
    pub fn exited(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Sends one request; false if the app no longer listens.
    pub fn ask(&mut self, request: &str) -> bool {
        self.stdin.as_mut().is_some_and(|stdin| {
            writeln!(stdin, "{request}")
                .and_then(|()| stdin.flush())
                .is_ok()
        })
    }

    /// Waits for `word` from the app; false if it quits or `timeout` passes first.
    pub fn wait_for(&self, word: &str, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            match self.answers.recv_timeout(left) {
                Ok(answer) if answer == word => return true,
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return false,
            }
        }
    }

    /// Lets go of the app's stdin, which asks it to quit, and kills it if it
    /// hasn't within `QUIT_GRACE`.
    pub fn quit(mut self) {
        drop(self.stdin.take());
        let asked = Instant::now();
        while asked.elapsed() < QUIT_GRACE {
            if self.exited().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        eprintln!("dev: the app didn't quit in time; stopping it");
        self.kill();
    }

    pub fn kill(mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}
