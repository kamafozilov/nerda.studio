//! The reply as it streams in, and the finished blocks taking its place.

use std::mem::discriminant;

use serde_json::Value;

use super::{Item, Text, ToolCall, ToolStatus, Transcript};
use crate::protocol::{Assistant, Block, BlockKind, Delta, Stream, ToolResult};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Streaming {
    message: String,
    pub(super) open: Vec<OpenBlock>,
}

/// A block of the streaming message, by its index, and the item showing it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct OpenBlock {
    index: u32,
    pub(super) turn: usize,
    item: usize,
}

impl Transcript {
    pub(super) fn stream(&mut self, stream: &Stream) {
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
    pub(super) fn assistant(&mut self, assistant: &Assistant) {
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

    pub(super) fn tool_result(&mut self, result: &ToolResult) {
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

    fn open_block(&self, index: u32) -> Option<OpenBlock> {
        let streaming = self.streaming.as_ref()?;
        streaming
            .open
            .iter()
            .find(|block| block.index == index)
            .copied()
    }
}
