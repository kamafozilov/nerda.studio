//! Archiving a session takes it out of the list and closes its tab; it keeps
//! its title, its conversation and its agent, and the archive lists it until
//! it is restored, or until a message sent in it brings it back.

use crate::{SessionId, Tab, Timestamp, Workspace};

impl Workspace {
    /// Archives a session that has seen work and closes its tab, as closing
    /// it would; returns whether it was archived. A draft, which isn't
    /// listed, an archived session and an unknown one stay as they are.
    pub fn archive_session(&mut self, id: SessionId, now: Timestamp) -> bool {
        let Some(session) = self.sessions.iter_mut().find(|session| session.id == id) else {
            return false;
        };
        if session.is_empty() || session.is_archived() {
            return false;
        }
        session.archived_at = Some(now);
        self.close_tabs(&[Tab::Session(id)], None, now);
        true
    }

    /// Brings an archived session back to the list where its activity puts it.
    pub fn unarchive_session(&mut self, id: SessionId) {
        if let Some(session) = self.sessions.iter_mut().find(|session| session.id == id) {
            session.archived_at = None;
        }
    }
}
