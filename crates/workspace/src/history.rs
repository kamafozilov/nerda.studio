//! Back and Forward through the tabs shown, like a browser's history. A closed
//! tab drops out rather than being reopened by Back, and a new visit discards
//! Forward. Kept in memory only.

use crate::Tab;

/// Most visits Back remembers.
const LIMIT: usize = 50;

#[derive(Clone, Debug, Default)]
pub struct Visits {
    back: Vec<Tab>,
    /// Nearest first.
    forward: Vec<Tab>,
    current: Option<Tab>,
}

impl Visits {
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Notes that `tab` is on screen; showing a new tab discards Forward.
    pub fn record(&mut self, tab: Tab) {
        if self.current == Some(tab) {
            return;
        }
        if let Some(current) = self.current {
            self.push_back(current);
        }
        self.current = Some(tab);
        self.forward.clear();
    }

    /// Steps back (`back`) or forward and returns the tab to show.
    pub fn step(&mut self, back: bool) -> Option<Tab> {
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

    /// Forgets visits to tabs no longer open, and the repeats of the current
    /// one that leaves next to it.
    pub fn prune(&mut self, open: &[Tab], current: Option<Tab>) {
        self.back.retain(|tab| open.contains(tab));
        self.forward.retain(|tab| open.contains(tab));
        if self.current.is_none_or(|tab| !open.contains(&tab)) {
            self.current = current;
        }
        self.back.dedup();
        self.forward.dedup();
        while self
            .back
            .last()
            .is_some_and(|&tab| Some(tab) == self.current)
        {
            self.back.pop();
        }
        while self
            .forward
            .first()
            .is_some_and(|&tab| Some(tab) == self.current)
        {
            self.forward.remove(0);
        }
    }

    fn push_back(&mut self, tab: Tab) {
        if self.back.last() != Some(&tab) {
            self.back.push(tab);
        }
        if self.back.len() > LIMIT {
            self.back.remove(0);
        }
    }
}
