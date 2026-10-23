//! The conversation as the rows its list draws: each turn's message, what
//! came back in it, item by item, and how it ended. Each row carries a stamp
//! of what its look depends on, so the list measures again only the rows
//! that changed, and lays out anew only from where rows came or went.

use std::{
    collections::HashSet,
    hash::{DefaultHasher, Hash, Hasher},
    mem::discriminant,
    ops::Range,
};

use agent::{Ending, Item, Transcript, TurnState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    /// The person's message in turn `turn`.
    Message { turn: usize },
    /// The `item`-th thing that came back in turn `turn`.
    Item { turn: usize, item: usize },
    /// How turn `turn` ended.
    End { turn: usize },
}

/// A row and a stamp that changes whenever its look does.
pub type Stamped = (Row, u64);

/// The rows of `transcript`, with the thinking blocks at `expanded` open.
pub fn rows(transcript: &Transcript, expanded: &HashSet<(usize, usize)>) -> Vec<Stamped> {
    let mut rows = Vec::new();
    for (at, turn) in transcript.turns().iter().enumerate() {
        if let Some(message) = &turn.message {
            let queued = turn.state == TurnState::Queued;
            let row = Row::Message { turn: at };
            rows.push((row, stamp((message.text.len(), queued))));
        }
        for (item, part) in turn.items.iter().enumerate() {
            let look = match part {
                Item::Text(text) => stamp((0, text.text.len(), text.streaming)),
                Item::Thinking(text) => {
                    let open = expanded.contains(&(at, item));
                    stamp((1, text.text.len(), text.streaming, open))
                }
                Item::Tool(call) => stamp((2, &call.name, discriminant(&call.status))),
                Item::Permission(permission) => stamp((3, discriminant(&permission.state))),
            };
            rows.push((Row::Item { turn: at, item }, look));
        }
        if let TurnState::Ended(ending) = &turn.state {
            let look = match ending {
                Ending::Finished(finished) => stamp((0, finished.duration_ms)),
                Ending::Interrupted => stamp(1),
                Ending::Failed(failure) => stamp((2, failure.message())),
            };
            rows.push((Row::End { turn: at }, look));
        }
    }
    rows
}

fn stamp(look: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    look.hash(&mut hasher);
    hasher.finish()
}

/// What changed between two lists of rows.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// Rows still in place whose look changed.
    pub changed: Vec<usize>,
    /// From where the rows differ: the old ones there, and how many new
    /// ones take their place.
    pub replaced: Option<(Range<usize>, usize)>,
}

pub fn change(old: &[Stamped], new: &[Stamped]) -> Change {
    let kept = (old.iter().zip(new))
        .take_while(|((a, _), (b, _))| a == b)
        .count();
    let changed = (0..kept).filter(|&at| old[at].1 != new[at].1).collect();
    let replaced =
        (kept < old.len() || kept < new.len()).then(|| (kept..old.len(), new.len() - kept));
    Change { changed, replaced }
}

#[cfg(test)]
mod tests {
    use agent::{Command, Event, Message};

    use super::*;

    fn sent(transcript: &mut Transcript, text: &str) -> Message {
        let message = Message::new(text, Vec::new());
        transcript.apply(&Event::Sent(Command::Send(message.clone())));
        message
    }

    #[test]
    fn a_queued_message_then_its_end_are_rows() {
        let mut transcript = Transcript::new();
        sent(&mut transcript, "Hello");
        let none = HashSet::new();
        let queued = rows(&transcript, &none);
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, Row::Message { turn: 0 });

        transcript.apply(&Event::Exited { code: Some(1) });
        let ended = rows(&transcript, &none);
        let kinds: Vec<Row> = ended.iter().map(|(row, _)| *row).collect();
        assert_eq!(kinds, [Row::Message { turn: 0 }, Row::End { turn: 0 }]);
        assert_ne!(queued[0].1, ended[0].1, "no longer queued");
    }

    #[test]
    fn a_row_whose_look_changed_is_measured_again_in_place() {
        let row = |turn| Row::Item { turn, item: 0 };
        let old = [(row(0), 1), (row(1), 2)];
        let new = [(row(0), 1), (row(1), 3)];
        assert_eq!(
            change(&old, &new),
            Change {
                changed: vec![1],
                replaced: None
            }
        );
    }

    #[test]
    fn rows_that_came_or_went_are_laid_out_from_the_first_difference() {
        let message = |turn| Row::Message { turn };
        let old = [(message(0), 1), (message(1), 1)];
        let appended = [(message(0), 1), (message(1), 1), (message(2), 1)];
        assert_eq!(change(&old, &appended).replaced, Some((2..2, 1)));
        // A turn of the agent's own, put in before a waiting message, moves
        // the rows after it.
        let inserted = [
            (message(0), 1),
            (Row::Item { turn: 1, item: 0 }, 1),
            (message(2), 1),
        ];
        assert_eq!(change(&old, &inserted).replaced, Some((1..2, 2)));
        assert_eq!(change(&old, &old), Change::default());
    }
}
