//! One build of the app the runner started.

use std::{
    path::Path,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
};

pub struct App {
    child: Child,
    /// Held, never written: the app quits once the runner lets go of it.
    _stdin: Option<ChildStdin>,
}

impl App {
    /// Starts the latest build; it takes over the window from any instance
    /// running, the runner's or not.
    pub fn launch(root: &Path) -> Option<App> {
        let binary = root
            .join("target/debug/studio")
            .with_extension(std::env::consts::EXE_EXTENSION);
        let mut command = Command::new(&binary);
        command.env("NERDA_DEV_RUNNER", "1").stdin(Stdio::piped());
        // Its own process group, so Ctrl-C stops the runner alone and the app
        // quits on the closed stdin, its saves written, instead of dying mid-save.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        match command.spawn() {
            Ok(mut child) => Some(App {
                _stdin: child.stdin.take(),
                child,
            }),
            Err(error) => {
                eprintln!("dev: can't start {}: {error}", binary.display());
                None
            }
        }
    }

    /// The app's exit, once it has quit.
    pub fn exited(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }
}
