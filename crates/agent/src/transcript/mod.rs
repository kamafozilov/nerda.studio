//! A session's conversation as the screen shows it: turns of the person's
//! message and what came back, reduced from the session's events.

mod prompt;
mod reply;
mod status;
mod turn;

pub use prompt::{Answer, Permission, PermissionState};
use reply::Streaming;
pub use status::Status;
use status::Work;
pub use turn::{Ending, Finished, Item, Text, ToolCall, ToolStatus, Turn, TurnState};

use crate::event::Event;
use crate::failure::{self, Failure, Signals, StderrTail};
use crate::models::{self, Offered};
use crate::protocol::{
    Command, Decision, INITIALIZE, Incoming, MessageId, Outcome, RequestId, Stage, ToolUseId,
    TurnResult,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Transcript {
    turns: Vec<Turn>,
    status: Status,
    /// The reply message streaming now, and its blocks not yet finished.
    streaming: Option<Streaming>,
    /// What the conversation had cost by the agent's count when its last
    /// turn ended.
    spent: f64,
    /// What the running turn ran into that may explain how it ends.
    signals: Signals,
    /// The last of what the process wrote on stderr.
    stderr: StderrTail,
    /// The models the agent offered when it last answered `initialize`.
    offered: Vec<Offered>,
}

impl Default for Transcript {
    fn default() -> Self {
        Self::new()
    }
}

impl Transcript {
    /// Empty, its session starting.
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            status: Status::Starting,
            streaming: None,
            spent: 0.0,
            signals: Signals::default(),
            stderr: StderrTail::default(),
            offered: Vec::new(),
        }
    }

    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub(crate) fn offered(&self) -> &[Offered] {
        &self.offered
    }

    /// The prompts waiting for the person, oldest first.
    pub fn pending_permissions(&self) -> impl Iterator<Item = &Permission> {
        self.turns
            .iter()
            .flat_map(|turn| &turn.items)
            .filter_map(|item| match item {
                Item::Permission(permission) if permission.state == PermissionState::Pending => {
                    Some(permission)
                }
                _ => None,
            })
    }

    /// What `events`, a session's events as a journal kept them, add up to.
    pub fn replay<'a>(events: impl IntoIterator<Item = &'a Event>) -> Self {
        let mut transcript = Self::new();
        for event in events {
            transcript.apply(event);
        }
        transcript
    }

    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Started => {
                // What a process left open ends with it, though a journal
                // the app kept may hold no exit for it, as after a crash.
                self.end_open(&Ending::Interrupted);
                self.streaming = None;
                self.signals = Signals::default();
                self.stderr = StderrTail::default();
            }
            Event::Sent(command) => self.sent(command),
            Event::Received(incoming) => self.received(incoming),
            Event::Exited { code } => {
                let stderr = self.stderr.text();
                self.end_open(&Ending::Failed(Failure::Crashed {
                    code: *code,
                    stderr,
                }));
            }
            Event::Failed(message) => {
                self.end_open(&Ending::Failed(Failure::Other(message.clone())))
            }
            Event::Stderr(line) => self.stderr.push(line),
            Event::Unreadable { .. } | Event::Refused { .. } | Event::Dropped(_) => {}
        }
        self.status = self.status.next(event, self.work());
    }

    fn sent(&mut self, command: &Command) {
        match command {
            Command::Send(message) => {
                let gone = matches!(self.status, Status::Exited { .. } | Status::Failed { .. });
                let state = if gone {
                    TurnState::Ended(Ending::Failed(Failure::NotSent))
                } else {
                    TurnState::Queued
                };
                self.turns.push(Turn {
                    message: Some(message.clone()),
                    state,
                    items: Vec::new(),
                });
            }
            Command::Answer { request, decision } => {
                if let Some(permission) = self.pending_mut(request) {
                    permission.state = match decision {
                        Decision::Allow { .. } => PermissionState::Allowed,
                        Decision::Deny { .. } => PermissionState::Denied,
                    };
                }
            }
            _ => {}
        }
    }

    fn received(&mut self, incoming: &Incoming) {
        match incoming {
            Incoming::Replay(id)
            | Incoming::Lifecycle {
                message: id,
                stage: Stage::Started,
            } => {
                if let Some(turn) = self.open_turn_of(id)
                    && self.turns[turn].state == TurnState::Queued
                {
                    self.turns[turn].state = TurnState::Running;
                }
            }
            Incoming::Lifecycle {
                message,
                stage: Stage::Cancelled,
            } => {
                if let Some(turn) = self.open_turn_of(message) {
                    self.end(turn, &Ending::Interrupted);
                }
            }
            // The agent took a message it had already run, as it does with an
            // id its conversation has seen, and dropped it.
            Incoming::Lifecycle {
                message,
                stage: Stage::Completed,
            } => {
                if let Some(turn) = self.open_turn_of(message)
                    && self.turns[turn].state == TurnState::Queued
                {
                    self.end(turn, &Ending::Failed(Failure::NeverStarted));
                }
            }
            Incoming::Stream(stream) => self.stream(stream),
            Incoming::Assistant(assistant) => match &assistant.error {
                // The agent's own words about a failed request: they show as
                // the turn's failure, not as a reply.
                Some(error) => {
                    let said = assistant.blocks.iter().filter_map(|block| match block {
                        crate::protocol::Block::Text(text) => Some(text.as_str()),
                        _ => None,
                    });
                    self.signals.api_error =
                        Some((error.clone(), said.collect::<Vec<_>>().join("\n")));
                }
                None => self.assistant(assistant),
            },
            Incoming::RateLimit(limit) => {
                self.signals.limit = limit.blocks().then(|| limit.clone())
            }
            Incoming::ToolResults(results) => results.iter().for_each(|r| self.tool_result(r)),
            Incoming::Permission(request) => {
                let turn = match &request.tool_use_id {
                    Some(id) => self.turn_of_tool(id),
                    None => None,
                };
                let turn = turn.unwrap_or_else(|| self.active_turn());
                self.turns[turn].items.push(Item::Permission(Permission {
                    request: (**request).clone(),
                    state: PermissionState::Pending,
                }));
            }
            Incoming::Cancel(id) => {
                if let Some(permission) = self.pending_mut(id) {
                    permission.state = PermissionState::Cancelled;
                }
            }
            Incoming::Result(result) => self.finish(result),
            Incoming::Response {
                id,
                result: Ok(answer),
            } if id.0 == INITIALIZE => {
                self.offered = models::offered(answer);
            }
            _ => {}
        }
    }

    fn finish(&mut self, result: &TurnResult) {
        // A process resuming the conversation goes on from the total it saved;
        // one that counts from zero shows it by a total below the last.
        let total = result.total_cost_usd;
        let cost_usd = if total >= self.spent {
            total - self.spent
        } else {
            total
        };
        self.spent = total;
        let signals = std::mem::take(&mut self.signals);
        let finished = Ending::Finished(Finished {
            cost_usd,
            usage: result.usage,
            duration_ms: result.duration_ms,
        });
        let ending = |turn: &Turn| match &result.outcome {
            // A turn that ran into a failed request or a rejecting limit and
            // has no reply to show failed, whatever its result says.
            Outcome::Completed if !signals.is_empty() && !turn.replied() => {
                Ending::Failed(failure::classify(&signals, None))
            }
            Outcome::Completed => finished.clone(),
            Outcome::Interrupted => Ending::Interrupted,
            Outcome::Failed(error) => Ending::Failed(failure::classify(&signals, Some(error))),
        };
        let mut answered: Vec<usize> = result
            .messages
            .iter()
            .filter_map(|id| self.open_turn_of(id))
            .collect();
        if answered.is_empty() {
            // Naming none of the app's open messages, it ends the turn the
            // agent began on its own; naming none at all, the one that runs.
            // Never a message the agent hasn't taken up: the agent settles
            // its old background tasks with such results before it starts on
            // a message, as when it resumes a conversation.
            let running = |turn: &Turn| turn.state == TurnState::Running;
            let own = self
                .turns
                .iter()
                .position(|turn| running(turn) && turn.message.is_none());
            let any = || self.turns.iter().position(running);
            answered.extend(own.or_else(|| result.messages.is_empty().then(any).flatten()));
        }
        for turn in answered {
            let ending = ending(&self.turns[turn]);
            self.end(turn, &ending);
        }
    }

    /// Ends a turn: nothing in it streams, runs or waits any longer.
    fn end(&mut self, index: usize, ending: &Ending) {
        let turn = &mut self.turns[index];
        turn.state = TurnState::Ended(ending.clone());
        for item in &mut turn.items {
            match item {
                Item::Text(text) | Item::Thinking(text) => text.streaming = false,
                Item::Tool(call) if call.status == ToolStatus::Running => {
                    call.status = ToolStatus::Failed(String::new());
                }
                Item::Permission(permission) if permission.state == PermissionState::Pending => {
                    permission.state = PermissionState::Cancelled;
                }
                _ => {}
            }
        }
        if let Some(streaming) = &mut self.streaming {
            streaming.open.retain(|block| block.turn != index);
        }
    }

    fn end_open(&mut self, ending: &Ending) {
        for turn in 0..self.turns.len() {
            if !self.turns[turn].is_ended() {
                self.end(turn, ending);
            }
        }
    }

    /// The turn the agent works on: the first that runs; when none does, a
    /// turn of its own, as when a background task of its finishes, placed
    /// before the messages still waiting, which it hasn't taken up. The agent
    /// echoes a message as it takes it up, so output alone never starts one.
    fn active_turn(&mut self) -> usize {
        if let Some(turn) = self
            .turns
            .iter()
            .position(|turn| turn.state == TurnState::Running)
        {
            return turn;
        }
        let at = self
            .turns
            .iter()
            .position(|turn| turn.state == TurnState::Queued);
        let at = at.unwrap_or(self.turns.len());
        self.turns.insert(
            at,
            Turn {
                message: None,
                state: TurnState::Running,
                items: Vec::new(),
            },
        );
        if let Some(streaming) = &mut self.streaming {
            streaming.shift_turns_from(at);
        }
        at
    }

    fn open_turn_of(&self, id: &MessageId) -> Option<usize> {
        let open = |turn: &Turn| !turn.is_ended() && turn.answers(id);
        self.turns.iter().position(open)
    }

    fn turn_of_tool(&self, id: &ToolUseId) -> Option<usize> {
        self.turns.iter().rposition(|turn| {
            turn.items
                .iter()
                .any(|item| matches!(item, Item::Tool(call) if call.id == *id))
        })
    }

    fn pending_mut(&mut self, id: &RequestId) -> Option<&mut Permission> {
        let mut items = self.turns.iter_mut().flat_map(|turn| &mut turn.items);
        items.find_map(|item| match item {
            Item::Permission(permission)
                if permission.request.id == *id && permission.state == PermissionState::Pending =>
            {
                Some(permission)
            }
            _ => None,
        })
    }

    fn work(&self) -> Work {
        Work {
            busy: self.turns.iter().any(|turn| !turn.is_ended()),
            asking: self.pending_permissions().next().is_some(),
        }
    }
}
