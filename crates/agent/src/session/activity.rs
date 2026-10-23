//! What the agent is doing, as far as the session needs to know to act on
//! its own: whether a turn runs or a message waits. Kept from the very
//! events the screen gets, in the same order.

use std::sync::{Mutex, PoisonError};

use async_channel::Sender;

use crate::event::Event;
use crate::protocol::{Command, Incoming, MessageId, Stage};

#[derive(Debug, Default)]
pub(super) struct Activity {
    /// Messages written that the agent hasn't taken up, answered or dropped.
    waiting: Vec<MessageId>,
    /// The agent has begun a turn and not ended it.
    in_turn: bool,
    /// The conversation has a turn on record, so a later process resumes it;
    /// one without would find nothing to resume.
    recorded: bool,
}

impl Activity {
    pub fn new(recorded: bool) -> Self {
        Self {
            recorded,
            ..Self::default()
        }
    }

    /// A turn runs or a message waits for one.
    pub fn busy(&self) -> bool {
        self.in_turn || !self.waiting.is_empty()
    }

    pub fn recorded(&self) -> bool {
        self.recorded
    }

    fn apply(&mut self, event: &Event) {
        match event {
            Event::Sent(Command::Send(message)) => self.waiting.push(message.id.clone()),
            Event::Received(incoming) => self.received(incoming),
            Event::Exited { .. } | Event::Failed(_) => {
                self.waiting.clear();
                self.in_turn = false;
            }
            _ => {}
        }
    }

    fn received(&mut self, incoming: &Incoming) {
        match incoming {
            Incoming::Init(_) => {
                self.in_turn = true;
                self.recorded = true;
            }
            Incoming::Replay(id)
            | Incoming::Lifecycle {
                message: id,
                stage: Stage::Started,
            } => {
                self.waiting.retain(|waiting| waiting != id);
                self.in_turn = true;
            }
            Incoming::Lifecycle {
                message,
                stage: Stage::Cancelled | Stage::Completed,
            } => self.waiting.retain(|waiting| waiting != message),
            Incoming::Result(result) => {
                self.waiting
                    .retain(|waiting| !result.messages.contains(waiting));
                self.in_turn = false;
            }
            _ => {}
        }
    }
}

/// Sends a session's events, keeping its activity from each as it goes.
pub(super) struct Emitter {
    events: Sender<Event>,
    activity: Mutex<Activity>,
}

impl Emitter {
    pub fn new(events: Sender<Event>, activity: Activity) -> Self {
        Self {
            events,
            activity: Mutex::new(activity),
        }
    }

    /// Events nobody drains any more are dropped; the agent is still read, so
    /// it never blocks on a full pipe.
    pub fn emit(&self, event: Event) {
        let mut activity = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
        activity.apply(&event);
        let _ = self.events.try_send(event);
    }

    pub fn activity<T>(&self, read: impl FnOnce(&Activity) -> T) -> T {
        read(&self.activity.lock().unwrap_or_else(PoisonError::into_inner))
    }
}
