//! A session's conversation as the screen shows it: turns of the person's
//! message and what came back, reduced from the session's events.

mod status;

use std::mem::discriminant;

use serde_json::Value;

pub use status::Status;
use status::Work;

use crate::event::Event;
use crate::protocol::{
    Assistant, Block, BlockKind, Command, Decision, Delta, Incoming, Message, MessageId, Outcome,
    PermissionRequest, RequestId, Stage, Stream, ToolResult, ToolUseId, TurnResult, Usage,
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

#[derive(Clone, Debug, PartialEq)]
struct Streaming {
    message: String,
    open: Vec<OpenBlock>,
}

/// A block of the streaming message, by its index, and the item showing it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OpenBlock {
    index: u32,
    turn: usize,
    item: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    /// What the person sent; `None` for a turn the agent began on its own,
    /// as when a background task of its finishes.
    pub message: Option<Message>,
    pub state: TurnState,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TurnState {
    /// Sent; the agent hasn't taken it up.
    Queued,
    Running,
    Ended(Ending),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ending {
    Finished(Finished),
    /// Stopped, before or while it ran.
    Interrupted,
    /// In words to show.
    Errored(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finished {
    /// What this turn cost.
    pub cost_usd: f64,
    pub usage: Usage,
    pub duration_ms: u64,
}

/// What came back, in the order it came.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Text(Text),
    Thinking(Text),
    Tool(ToolCall),
    Permission(Permission),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub text: String,
    /// More of it may come.
    pub streaming: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: ToolUseId,
    pub name: String,
    /// `Null` until the call is written out in full.
    pub input: Value,
    pub status: ToolStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    Running,
    /// With the tool's output.
    Done(String),
    /// With the tool's output; empty when the turn ended before the tool did.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Permission {
    pub request: PermissionRequest,
    pub state: PermissionState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionState {
    Pending,
    Allowed,
    Denied,
    /// Withdrawn by the agent, or left unanswered when the turn ended.
    Cancelled,
}

impl Permission {
    /// The answer that lets the tool run as asked.
    pub fn allow(&self) -> Command {
        self.answer(Decision::Allow {
            input: self.request.input.clone(),
        })
    }

    /// The answer that refuses the tool, telling the agent why.
    pub fn deny(&self, message: impl Into<String>) -> Command {
        self.answer(Decision::Deny {
            message: message.into(),
        })
    }

    fn answer(&self, decision: Decision) -> Command {
        Command::Answer {
            request: self.request.id.clone(),
            decision,
        }
    }
}

impl Turn {
    fn is_ended(&self) -> bool {
        matches!(self.state, TurnState::Ended(_))
    }

    fn answers(&self, id: &MessageId) -> bool {
        self.message
            .as_ref()
            .is_some_and(|message| message.id == *id)
    }
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

    fn stream(&mut self, stream: &Stream) {
        match stream {
            Stream::MessageStart { id } => {
                self.streaming = Some(Streaming {
                    message: id.clone(),
                    open: Vec::new(),
                });
            }
            Stream::BlockStart { index, kind } => {
                let item = match kind {
                    BlockKind::Text => Item::Text(Text::streaming()),
                    BlockKind::Thinking => Item::Thinking(Text::streaming()),
                    BlockKind::ToolUse { id, name } => Item::Tool(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: Value::Null,
                        status: ToolStatus::Running,
                    }),
                    BlockKind::Other => return,
                };
                // Without its message's start, the finished block alone shows it.
                if self.streaming.is_none() {
                    return;
                }
                let turn = self.active_turn();
                self.turns[turn].items.push(item);
                let item = self.turns[turn].items.len() - 1;
                if let Some(streaming) = &mut self.streaming {
                    let index = *index;
                    streaming.open.push(OpenBlock { index, turn, item });
                }
            }
            Stream::Delta { index, delta } => {
                let (Delta::Text(more) | Delta::Thinking(more)) = delta else {
                    return;
                };
                let Some(block) = self.open_block(*index) else {
                    return;
                };
                if let Item::Text(text) | Item::Thinking(text) =
                    &mut self.turns[block.turn].items[block.item]
                {
                    text.text.push_str(more);
                }
            }
            Stream::BlockStop { index } => {
                let Some(streaming) = &mut self.streaming else {
                    return;
                };
                let Some(at) = streaming.open.iter().position(|b| b.index == *index) else {
                    return;
                };
                let block = streaming.open.remove(at);
                if let Item::Text(text) | Item::Thinking(text) =
                    &mut self.turns[block.turn].items[block.item]
                {
                    text.streaming = false;
                }
            }
            Stream::Other => {}
        }
    }

    /// A finished block takes the place of the one that streamed it, or, when
    /// none did, comes after what the turn has.
    fn assistant(&mut self, assistant: &Assistant) {
        let turn = self.active_turn();
        for block in &assistant.blocks {
            let finished = match block {
                Block::Text(text) => Item::Text(Text::done(text)),
                Block::Thinking(text) => Item::Thinking(Text::done(text)),
                Block::ToolUse { id, name, input } => Item::Tool(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    status: ToolStatus::Running,
                }),
                Block::Other(_) => continue,
            };
            let turns = &self.turns;
            let streamed = self
                .streaming
                .as_mut()
                .filter(|streaming| streaming.message == assistant.message_id)
                .and_then(|streaming| {
                    let at = streaming.open.iter().position(|block| {
                        let item = &turns[block.turn].items[block.item];
                        discriminant(item) == discriminant(&finished)
                    })?;
                    Some(streaming.open.remove(at))
                });
            match streamed {
                Some(block) => match (&mut self.turns[block.turn].items[block.item], finished) {
                    (Item::Tool(call), Item::Tool(written)) => call.input = written.input,
                    (item, finished) => *item = finished,
                },
                None => self.turns[turn].items.push(finished),
            }
        }
    }

    fn tool_result(&mut self, result: &ToolResult) {
        let calls = self.turns.iter_mut().rev().flat_map(|turn| &mut turn.items);
        for item in calls {
            if let Item::Tool(call) = item
                && call.id == result.tool_use_id
            {
                call.status = if result.is_error {
                    ToolStatus::Failed(result.output.clone())
                } else {
                    ToolStatus::Done(result.output.clone())
                };
                return;
            }
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

    fn open_block(&self, index: u32) -> Option<OpenBlock> {
        let streaming = self.streaming.as_ref()?;
        streaming
            .open
            .iter()
            .find(|block| block.index == index)
            .copied()
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

impl Text {
    fn streaming() -> Self {
        Self {
            text: String::new(),
            streaming: true,
        }
    }

    fn done(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            streaming: false,
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
