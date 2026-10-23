//! A session's conversation as the screen shows it: turns of the person's
//! message and what came back, reduced from the session's events.

mod reply;
mod status;
mod turn;

use reply::Streaming;
pub use status::Status;
use status::Work;
pub use turn::{
    Ending, Finished, Item, Permission, PermissionState, Text, ToolCall, ToolStatus, Turn,
    TurnState,
};

use crate::event::Event;
use crate::protocol::{
    Command, Decision, Incoming, MessageId, Outcome, RequestId, Stage, ToolUseId, TurnResult,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Transcript {
    turns: Vec<Turn>,
    status: Status,
    /// The reply message streaming now, and its blocks not yet finished.
    streaming: Option<Streaming>,
    /// What the process had cost when its last turn ended.
    spent: f64,
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
        }
    }

    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    pub fn status(&self) -> &Status {
        &self.status
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

    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Started => {
                self.spent = 0.0;
                self.streaming = None;
            }
            Event::Sent(command) => self.sent(command),
            Event::Received(incoming) => self.received(incoming),
            Event::Exited { code } => self.end_open(&Ending::Errored(quit_message(*code))),
            Event::Failed(message) => self.end_open(&Ending::Errored(message.clone())),
            Event::Unreadable { .. } | Event::Stderr(_) => {}
        }
        self.status = self.status.next(event, self.work());
    }

    fn sent(&mut self, command: &Command) {
        match command {
            Command::Send(message) => {
                let gone = matches!(self.status, Status::Exited { .. } | Status::Failed { .. });
                let state = if gone {
                    TurnState::Ended(Ending::Errored(NOT_RUNNING.to_owned()))
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
            Incoming::Stream(stream) => self.stream(stream),
            Incoming::Assistant(assistant) => self.assistant(assistant),
            Incoming::ToolResults(results) => results.iter().for_each(|r| self.tool_result(r)),
            Incoming::Permission(request) => {
                let turn = match &request.tool_use_id {
                    Some(id) => self.turn_of_tool(id),
                    None => None,
                };
                let turn = turn.unwrap_or_else(|| self.active_turn());
                self.turns[turn].items.push(Item::Permission(Permission {
                    request: request.clone(),
                    state: PermissionState::Pending,
                }));
            }
            Incoming::Cancel(id) => {
                if let Some(permission) = self.pending_mut(id) {
                    permission.state = PermissionState::Cancelled;
                }
            }
            Incoming::Result(result) => self.finish(result),
            _ => {}
        }
    }

    fn finish(&mut self, result: &TurnResult) {
        let cost_usd = (result.total_cost_usd - self.spent).max(0.0);
        self.spent = result.total_cost_usd;
        let ending = match &result.outcome {
            Outcome::Completed => Ending::Finished(Finished {
                cost_usd,
                usage: result.usage,
                duration_ms: result.duration_ms,
            }),
            Outcome::Interrupted => Ending::Interrupted,
            Outcome::Failed(message) => Ending::Errored(message.clone()),
        };
        let mut answered: Vec<usize> = result
            .messages
            .iter()
            .filter_map(|id| self.open_turn_of(id))
            .collect();
        if answered.is_empty() {
            // Naming none of the app's open messages, it ends the turn the
            // agent began on its own; naming none at all, the one that runs.
            answered.extend(self.turns.iter().position(|turn| {
                !turn.is_ended() && (result.messages.is_empty() || turn.message.is_none())
            }));
        }
        for turn in answered {
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

    /// The turn the agent works on: the first not ended, which the agent has
    /// taken up if it still showed as queued; a new turn of its own if none.
    fn active_turn(&mut self) -> usize {
        if let Some(turn) = self.turns.iter().position(|turn| !turn.is_ended()) {
            if self.turns[turn].state == TurnState::Queued {
                self.turns[turn].state = TurnState::Running;
            }
            return turn;
        }
        self.turns.push(Turn {
            message: None,
            state: TurnState::Running,
            items: Vec::new(),
        });
        self.turns.len() - 1
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

const NOT_RUNNING: &str = "The agent wasn't running, so the message wasn't sent.";

fn quit_message(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("The agent quit before it finished (exit code {code})."),
        None => "The agent was stopped before it finished.".to_owned(),
    }
}
