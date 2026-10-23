//! Starting child processes one at a time.

use std::io;
use std::process::{Child, Command};
use std::sync::{Mutex, PoisonError};

/// On macOS a pipe is made close-on-exec in a second step, so a child started
/// by another thread in between keeps the pipe open. Starting every child of
/// this crate one at a time keeps them off each other's pipes: an agent whose
/// input another child holds open never sees that input close.
static SPAWNING: Mutex<()> = Mutex::new(());

pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    let _spawning = SPAWNING.lock().unwrap_or_else(PoisonError::into_inner);
    command.spawn()
}
