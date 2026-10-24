//! The conversation as the rows its list draws: each turn's message, what
//! came back in it, item by item, and how it ended. Each row carries a stamp
//! of what its look depends on, so the list measures again only the rows
//! that changed, and lays out anew only from where rows came or went.

use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    mem::discriminant,
    ops::Range,
};

use agent::{Ending, Item, ToolStatus, Transcript, Turn, TurnState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    /// The person's message in turn `turn`.
    Message { turn: usize },
    /// The `item`-th thing that came back in turn `turn`, and whether it
    /// stands in a group.
    Item {
        turn: usize,
        item: usize,
        nest: Nest,
    },
    /// The line over a group of tool calls and thinking, items `first` to
    /// `last` of turn `turn`.
    Group {
        turn: usize,
        first: usize,
        last: usize,
    },
    /// How turn `turn` ended.
    End { turn: usize },
    /// What the agent is doing, under everything, while a turn runs or a
    /// message waits.
    Working,
}

/// Where an item stands among its group's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Nest {
    Alone,
    Inside,
    Last,
}

/// A row and a stamp that changes whenever its look does.
pub type Stamped = (Row, u64);

/// What the person opened and closed: items by turn and item, and groups by
/// turn and first item, open or not.
#[derive(Default)]
pub struct Disclosure {
    pub items: HashSet<(usize, usize)>,
    pub groups: HashMap<(usize, usize), bool>,
}

/// The runs of tool calls and thinking in `turn` that show as a group: two
/// items or more, one of them a tool call.
pub fn groups(turn: &Turn) -> Vec<Range<usize>> {
    let work = |item: &Item| matches!(item, Item::Tool(_) | Item::Thinking(_));
    let mut groups = Vec::new();
    let mut at = 0;
    while at < turn.items.len() {
        if !work(&turn.items[at]) {
            at += 1;
            continue;
        }
        let start = at;
        while at < turn.items.len() && work(&turn.items[at]) {
            at += 1;
        }
        let run = &turn.items[start..at];
        if run.len() > 1 && run.iter().any(|item| matches!(item, Item::Tool(_))) {
            groups.push(start..at);
        }
    }
    groups
}

/// Whether the group `range` of `turn` is still being added to: the turn
/// runs and nothing came after it.
pub fn live(turn: &Turn, range: &Range<usize>) -> bool {
    turn.state == TurnState::Running && range.end == turn.items.len()
}

/// Whether the group `range` of turn `at` shows its items: as the person
/// left it, else while it is live.
pub fn open(disclosure: &Disclosure, at: usize, turn: &Turn, range: &Range<usize>) -> bool {
    (disclosure.groups.get(&(at, range.start)).copied()).unwrap_or_else(|| live(turn, range))
}

/// The rows of `transcript`, opened and closed as `disclosure` says.
pub fn rows(transcript: &Transcript, disclosure: &Disclosure) -> Vec<Stamped> {
    let mut rows = Vec::new();
    for (at, turn) in transcript.turns().iter().enumerate() {
        if let Some(message) = &turn.message {
            let queued = turn.state == TurnState::Queued;
            let row = Row::Message { turn: at };
            rows.push((row, stamp((message.text.len(), queued))));
        }
        let groups = groups(turn);
        let mut item = 0;
        while item < turn.items.len() {
            let group = groups.iter().find(|range| range.start == item);
            let Some(range) = group else {
                let row = Row::Item {
                    turn: at,
                    item,
                    nest: Nest::Alone,
                };
                rows.push((
                    row,
                    look(turn, item, disclosure.items.contains(&(at, item))),
                ));
                item += 1;
                continue;
            };
            let open = open(disclosure, at, turn, range);
            let statuses: Vec<u64> = (turn.items[range.clone()].iter())
                .map(|part| match part {
                    Item::Tool(call) => stamp((status(&call.status), call.input.is_null())),
                    _ => 0,
                })
                .collect();
            let header = Row::Group {
                turn: at,
                first: range.start,
                last: range.end - 1,
            };
            rows.push((header, stamp((statuses, open, live(turn, range)))));
            if open {
                for inner in range.clone() {
                    let nest = if inner + 1 == range.end {
                        Nest::Last
                    } else {
                        Nest::Inside
                    };
                    let row = Row::Item {
                        turn: at,
                        item: inner,
                        nest,
                    };
                    rows.push((
                        row,
                        look(turn, inner, disclosure.items.contains(&(at, inner))),
                    ));
                }
            }
            item = range.end;
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
    let busy = (transcript.turns().iter()).any(|turn| !matches!(turn.state, TurnState::Ended(_)));
    if busy {
        rows.push((Row::Working, 0));
    }
    rows
}

/// A stamp of how item `item` of `turn` looks, `open` or not.
fn look(turn: &Turn, item: usize, open: bool) -> u64 {
    match &turn.items[item] {
        Item::Text(text) => stamp((0, text.text.len(), text.streaming)),
        Item::Thinking(text) => stamp((1, text.text.len(), text.streaming, open)),
        Item::Tool(call) => stamp((
            2,
            &call.name,
            call.input.is_null(),
            status(&call.status),
            open,
        )),
        Item::Permission(permission) => stamp((3, discriminant(&permission.state))),
    }
}

fn status(status: &ToolStatus) -> u64 {
    match status {
        ToolStatus::Running => 0,
        ToolStatus::Done(output) => stamp((1, output.len())),
        ToolStatus::Failed(output) => stamp((2, output.len())),
    }
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
        let none = Disclosure::default();
        let queued = rows(&transcript, &none);
        assert_eq!(queued.len(), 2);
        assert_eq!(queued[0].0, Row::Message { turn: 0 });

        transcript.apply(&Event::Exited { code: Some(1) });
        assert_eq!(
            queued[1].0,
            Row::Working,
            "it works while the message waits"
        );
        let ended = rows(&transcript, &none);
        let kinds: Vec<Row> = ended.iter().map(|(row, _)| *row).collect();
        assert_eq!(kinds, [Row::Message { turn: 0 }, Row::End { turn: 0 }]);
        assert_ne!(queued[0].1, ended[0].1, "no longer queued");
    }

    #[test]
    fn a_row_whose_look_changed_is_measured_again_in_place() {
        let row = |turn| Row::Item {
            turn,
            item: 0,
            nest: Nest::Alone,
        };
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
            (
                Row::Item {
                    turn: 1,
                    item: 0,
                    nest: Nest::Alone,
                },
                1,
            ),
            (message(2), 1),
        ];
        assert_eq!(change(&old, &inserted).replaced, Some((1..2, 2)));
        assert_eq!(change(&old, &old), Change::default());
    }
}
