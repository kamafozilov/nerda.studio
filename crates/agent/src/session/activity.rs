//! What the agent is doing, as far as the session needs to know to act on
//! its own: whether a turn runs, which messages wait, which prompts are open
//! and which tasks run in the background. Kept from the very events the
//! screen gets, in the same order.

use std::sync::{Mutex, PoisonError};

use async_channel::Sender;

use crate::event::Event;
use crate::protocol::{Command, Incoming, Message, RequestId, Stage, TaskEvent, TaskId};

#[derive(Debug, Default)]
pub(super) struct Activity {
    /// Messages written that the agent hasn't taken up, answered or dropped.
    waiting: Vec<Message>,
    /// The agent has begun a turn and not ended it.
    in_turn: bool,
    /// The conversation has a turn on record, so a later process resumes it;
    /// one without would find nothing to resume.
    recorded: bool,
    /// Prompts the agent waits on an answer to.
    prompts: Vec<RequestId>,
    /// The agent's tasks running in the background.
    tasks: Vec<TaskId>,
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

    pub fn prompts(&self) -> Vec<RequestId> {
        self.prompts.clone()
    }

    pub fn tasks(&self) -> Vec<TaskId> {
        self.tasks.clone()
    }

    /// Takes `event` in, giving back the waiting message it drops, if any.
    fn apply(&mut self, event: &Event) -> Option<Message> {
        match event {
            Event::Sent(Command::Send(message)) => self.waiting.push(message.clone()),
            Event::Sent(Command::Answer { request, .. }) => {
                self.prompts.retain(|open| open != request)
            }
            Event::Received(incoming) => return self.received(incoming),
            Event::Exited { .. } | Event::Failed(_) => {
                self.waiting.clear();
                self.in_turn = false;
                self.prompts.clear();
                self.tasks.clear();
            }
            _ => {}
        }
        None
    }

    fn received(&mut self, incoming: &Incoming) -> Option<Message> {
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
                self.waiting.retain(|waiting| waiting.id != *id);
                self.in_turn = true;
            }
            Incoming::Lifecycle {
                message,
                stage: Stage::Cancelled,
            } => {
                let at = self
                    .waiting
                    .iter()
                    .position(|waiting| waiting.id == *message)?;
                return Some(self.waiting.remove(at));
            }
            Incoming::Lifecycle {
                message,
                stage: Stage::Completed,
            } => self.waiting.retain(|waiting| waiting.id != *message),
            Incoming::Result(result) => {
                self.waiting
                    .retain(|waiting| !result.messages.contains(&waiting.id));
                self.in_turn = false;
                self.prompts.clear();
            }
            Incoming::Permission(request) => self.prompts.push(request.id.clone()),
            Incoming::Cancel(id) => self.prompts.retain(|open| open != id),
            Incoming::Task(task) => self.task(task),
            _ => {}
        }
        None
    }

    fn task(&mut self, task: &TaskEvent) {
        match task {
            TaskEvent::Started {
                id,
                background: true,
            }
            | TaskEvent::Backgrounded(id) => {
                if !self.tasks.contains(id) {
                    self.tasks.push(id.clone());
                }
            }
            TaskEvent::Started {
                background: false, ..
            } => {}
            TaskEvent::Ended(id) => self.tasks.retain(|running| running != id),
            TaskEvent::Background(running) => self.tasks = running.clone(),
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

    /// Sends `event`, then `Event::Dropped` for a waiting message it says the
    /// agent dropped. Events nobody drains any more are dropped; the agent is
    /// still read, so it never blocks on a full pipe.
    pub fn emit(&self, event: Event) {
        let mut activity = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
        let dropped = activity.apply(&event);
        let _ = self.events.try_send(event);
        if let Some(message) = dropped {
            let _ = self.events.try_send(Event::Dropped(message));
        }
    }

    pub fn activity<T>(&self, read: impl FnOnce(&Activity) -> T) -> T {
        read(&self.activity.lock().unwrap_or_else(PoisonError::into_inner))
    }
}
