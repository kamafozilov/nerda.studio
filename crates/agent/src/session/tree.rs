//! The agent's process, the group it leads and everything it started, and
//! ending them all.
//!
//! The agent starts in a process group of its own, which the tools it runs
//! join. Its background shells, though, each start a group of their own, so
//! a signal to the agent's group misses them, and they outlive the agent
//! once it is gone: a dev server started from Bash would keep running. They
//! are found by walking the process table from the agent while it still
//! runs; once it has quit, its orphans can no longer be traced.

use std::time::Duration;

pub(crate) struct Tree {
    /// The agent, which leads its group.
    root: u32,
    /// What the agent had started when last looked at.
    #[cfg(unix)]
    found: Vec<i32>,
}

impl Tree {
    pub(crate) fn new(root: u32) -> Self {
        Self {
            root,
            #[cfg(unix)]
            found: Vec::new(),
        }
    }

    /// Notes every process the agent has started so far; call it while the
    /// agent runs.
    pub(crate) fn snapshot(&mut self) {
        #[cfg(unix)]
        {
            self.found = unix::descendants(self.root);
        }
    }

    /// Asks the agent's group to quit, as a terminal's SIGTERM would.
    pub(crate) fn terminate(&self) {
        #[cfg(unix)]
        unix::signal(-(self.root as i32), libc::SIGTERM);
    }

    /// Ends the agent and its group at once.
    pub(crate) fn kill(&self) {
        #[cfg(unix)]
        unix::signal(-(self.root as i32), libc::SIGKILL);
        #[cfg(windows)]
        windows::kill_tree(self.root);
    }

    /// Once the agent has quit: ends what is left of its group and what the
    /// last snapshot found, with SIGTERM, then SIGKILL for whatever still
    /// runs after `grace`.
    pub(crate) fn end_leftovers(&self, grace: Duration) {
        #[cfg(unix)]
        unix::end(self.root, &self.found, grace);
        #[cfg(not(unix))]
        let _ = grace;
    }
}

#[cfg(unix)]
mod unix {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// Every live descendant of `root`, from one look at the process table.
    pub(super) fn descendants(root: u32) -> Vec<i32> {
        let mut ps = Command::new("ps");
        ps.args(["-A", "-o", "pid=,ppid="])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let Ok(child) = crate::spawning::spawn(&mut ps) else {
            return Vec::new();
        };
        let Ok(output) = child.wait_with_output() else {
            return Vec::new();
        };
        let table: Vec<(i32, i32)> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let mut columns = line.split_whitespace().map(str::parse::<i32>);
                Some((columns.next()?.ok()?, columns.next()?.ok()?))
            })
            .collect();
        let mut found = Vec::new();
        let mut frontier = vec![root as i32];
        while let Some(parent) = frontier.pop() {
            for &(pid, ppid) in &table {
                if ppid == parent && pid > 1 && !found.contains(&pid) {
                    found.push(pid);
                    frontier.push(pid);
                }
            }
        }
        found
    }

    pub(super) fn signal(target: i32, signal: i32) {
        // SAFETY: kill only sends a signal; a target that is gone fails with ESRCH.
        unsafe { libc::kill(target, signal) };
    }

    fn alive(target: i32) -> bool {
        // SAFETY: signal 0 only checks that the target exists.
        unsafe { libc::kill(target, 0) == 0 }
    }

    /// The group a process leads, or the process alone.
    fn target(pid: i32) -> i32 {
        // SAFETY: getpgid only reads; a pid that is gone gives -1.
        if unsafe { libc::getpgid(pid) } == pid {
            -pid
        } else {
            pid
        }
    }

    pub(super) fn end(root: u32, found: &[i32], grace: Duration) {
        let mut targets: Vec<i32> = found.iter().map(|&pid| target(pid)).collect();
        targets.push(-(root as i32));
        targets.retain(|&target| alive(target));
        for &target in &targets {
            signal(target, libc::SIGTERM);
        }
        let give_up = Instant::now() + grace;
        while targets.iter().any(|&target| alive(target)) && Instant::now() < give_up {
            std::thread::sleep(Duration::from_millis(20));
        }
        for &target in &targets {
            if alive(target) {
                signal(target, libc::SIGKILL);
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Ends `root` and every process it started, while their parents still
    /// trace back to it.
    pub(super) fn kill_tree(root: u32) {
        let mut taskkill = Command::new("taskkill");
        taskkill
            .args(["/PID", &root.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        if let Ok(mut child) = crate::spawning::spawn(&mut taskkill) {
            let _ = child.wait();
        }
    }
}
