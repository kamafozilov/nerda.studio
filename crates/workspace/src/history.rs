//! Back and Forward through the sessions shown, like a browser's history. A
//! closed tab drops out rather than being reopened by Back, and a new visit
//! discards Forward. Kept in memory only.
//!

use crate::SessionId;

/// Most visits Back remembers.
const LIMIT: usize = 50;

#[derive(Clone, Debug, Default)]
pub struct Visits {
    back: Vec<SessionId>,
    /// Nearest first.
    forward: Vec<SessionId>,
    current: Option<SessionId>,
}

impl Visits {
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Notes that `id` is on screen; showing a new session discards Forward.
    pub fn record(&mut self, id: SessionId) {
        if self.current == Some(id) {
            return;
        }
        if let Some(current) = self.current {
            self.push_back(current);
        }
        self.current = Some(id);
        self.forward.clear();
    }

    /// Steps back (`back`) or forward and returns the session to show.
    pub fn step(&mut self, back: bool) -> Option<SessionId> {
        let target = if back {
            let target = self.back.pop()?;
            if let Some(current) = self.current {
                self.forward.insert(0, current);
            }
            target
        } else {
            if self.forward.is_empty() {
                return None;
            }
            let target = self.forward.remove(0);
            if let Some(current) = self.current {
                self.push_back(current);
            }
            target
        };
        self.current = Some(target);
        Some(target)
    }

    /// Forgets visits to sessions no longer open as tabs, and the repeats of the
    /// current one that leaves next to it.
    pub fn prune(&mut self, open: &[SessionId], current: Option<SessionId>) {
        self.back.retain(|id| open.contains(id));
        self.forward.retain(|id| open.contains(id));
        if self.current.is_none_or(|id| !open.contains(&id)) {
            self.current = current;
        }
        self.back.dedup();
        self.forward.dedup();
        while self.back.last().is_some_and(|&id| Some(id) == self.current) {
            self.back.pop();
        }
        while self
            .forward
            .first()
            .is_some_and(|&id| Some(id) == self.current)
        {
            self.forward.remove(0);
        }
    }

    fn push_back(&mut self, id: SessionId) {
        if self.back.last() != Some(&id) {
            self.back.push(id);
        }
        if self.back.len() > LIMIT {
            self.back.remove(0);
        }
    }
}
