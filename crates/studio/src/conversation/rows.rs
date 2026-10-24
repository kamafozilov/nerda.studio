//! The conversation as the rows its list draws: each turn's message, the
//! line saying how long it worked, what came back in it, item by item, and
//! how it ended. A finished turn folds its work under that line, its last
//! reply left in view. Each row carries a stamp
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
    /// How long turn `turn` has worked, under its message, once the agent
    /// took it up; the work folds under it once the turn finished.
    Header { turn: usize },
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
    /// Room under the newest turn, so its message can stand at the top of
    /// the view however little came back yet.
    Room,
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

/// What the person opened and closed: items by turn and item, groups by
/// turn and first item, open or not, and the turns whose folded work they
/// opened.
#[derive(Default)]
pub struct Disclosure {
    pub items: HashSet<(usize, usize)>,
    pub groups: HashMap<(usize, usize), bool>,
    pub turns: HashSet<usize>,
}

/// A finished turn's work, folded under its header: every item but `kept`,
/// its last reply, which stays in view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fold {
    pub kept: Option<usize>,
}

/// Whether `item` is text the agent wrote.
fn reply(item: &Item) -> bool {
    matches!(item, Item::Text(text) if !text.text.trim().is_empty())
}

/// How `turn` folds: once it finished, when it has more than its last reply
/// to fold. A stopped or failed turn keeps its work in view, where what
/// went wrong is.
pub fn fold(turn: &Turn) -> Option<Fold> {
    if !matches!(turn.state, TurnState::Ended(Ending::Finished(_))) {
        return None;
    }
    let kept = turn.items.iter().rposition(reply);
    let folded = turn.items.len() - usize::from(kept.is_some());
    (folded > 0).then_some(Fold { kept })
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
        let started = match &turn.state {
            TurnState::Queued => false,
            TurnState::Running | TurnState::Ended(Ending::Finished(_)) => true,
            // One that ended before it did anything, as one that never ran,
            // says only how it ended.
            TurnState::Ended(_) => !turn.items.is_empty(),
        };
        let fold = fold(turn);
        let opened = disclosure.turns.contains(&at);
        let header = (discriminant(&turn.state), fold, opened);
        if started {
            rows.push((Row::Header { turn: at }, stamp(header)));
        }
        if let Some(fold) = fold.filter(|_| !opened) {
            if let Some(item) = fold.kept {
                let row = Row::Item {
                    turn: at,
                    item,
                    nest: Nest::Alone,
                };
                rows.push((
                    row,
                    look(turn, item, disclosure.items.contains(&(at, item))),
                ));
            }
        } else {
            items(turn, at, disclosure, &mut rows);
        }
        if let TurnState::Ended(ending) = &turn.state {
            let look = match ending {
                // Its line is the header's; under it, Copy, when it wrote
                // something.
                Ending::Finished(_) if !turn.items.iter().any(reply) => None,
                Ending::Finished(_) => Some(stamp(0)),
                Ending::Interrupted => Some(stamp(1)),
                Ending::Failed(failure) => Some(stamp((2, failure.message()))),
            };
            if let Some(look) = look {
                rows.push((Row::End { turn: at }, look));
            }
        }
    }
    let busy = (transcript.turns().iter()).any(|turn| !matches!(turn.state, TurnState::Ended(_)));
    if busy {
        rows.push((Row::Working, 0));
    }
    rows
}

/// The rows of each item of `turn`, turn `at`, a group's under its line.
fn items(turn: &Turn, at: usize, disclosure: &Disclosure, rows: &mut Vec<Stamped>) {
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

    fn turn(items: Vec<Item>, state: TurnState) -> Turn {
        Turn {
            message: None,
            state,
            items,
        }
    }

    fn text(text: &str) -> Item {
        Item::Text(agent::Text {
            text: text.to_owned(),
            streaming: false,
        })
    }

    fn tool() -> Item {
        Item::Tool(agent::ToolCall {
            id: serde_json::from_value(serde_json::json!("t")).unwrap(),
            name: "Bash".to_owned(),
            input: serde_json::json!({"command": "ls"}),
            status: ToolStatus::Done(String::new()),
        })
    }

    fn finished() -> TurnState {
        TurnState::Ended(Ending::Finished(agent::Finished {
            cost_usd: 0.,
            usage: Default::default(),
            duration_ms: 1000,
        }))
    }

    #[test]
    fn a_finished_turn_folds_all_but_its_last_reply() {
        let items = vec![tool(), text("Looking."), tool(), text("Done."), tool()];
        let done = turn(items.clone(), finished());
        assert_eq!(fold(&done), Some(Fold { kept: Some(3) }));
        let quiet = turn(vec![tool(), text("  ")], finished());
        assert_eq!(fold(&quiet), Some(Fold { kept: None }), "no reply to keep");
        let answer = turn(vec![text("Yes.")], finished());
        assert_eq!(fold(&answer), None, "nothing else to fold");
        let running = turn(items.clone(), TurnState::Running);
        assert_eq!(fold(&running), None, "not until it finished");
        let stopped = turn(items, TurnState::Ended(Ending::Interrupted));
        assert_eq!(fold(&stopped), None, "its work stays in view");
    }

    #[test]
    fn a_folded_turn_shows_its_header_and_reply_until_opened() {
        let mut transcript = Transcript::new();
        let message = sent(&mut transcript, "Fix it");
        let result: agent::Incoming = serde_json::from_value(serde_json::json!({
            "Result": {
                "session_id": "s",
                "messages": [message.id],
                "total_cost_usd": 0.0,
                "duration_ms": 1000,
                "usage": {},
                "outcome": "Completed",
            }
        }))
        .unwrap();
        let assistant: agent::Incoming = serde_json::from_value(serde_json::json!({
            "Assistant": {
                "message_id": "m",
                "blocks": [
                    {"ToolUse": {"id": "t", "name": "Bash", "input": {"command": "ls"}}},
                    {"Text": "Done."},
                ],
                "aborted": false,
                "error": null,
            }
        }))
        .unwrap();
        let started: agent::Incoming = serde_json::from_value(serde_json::json!({
            "Replay": message.id,
        }))
        .unwrap();
        for incoming in [started, assistant, result] {
            transcript.apply(&Event::Received(incoming));
        }
        let kinds = |disclosure: &Disclosure| -> Vec<Row> {
            rows(&transcript, disclosure)
                .into_iter()
                .map(|(row, _)| row)
                .collect()
        };
        let item = |item| Row::Item {
            turn: 0,
            item,
            nest: Nest::Alone,
        };
        let header = Row::Header { turn: 0 };
        let end = Row::End { turn: 0 };
        let message = Row::Message { turn: 0 };
        let mut disclosure = Disclosure::default();
        assert_eq!(kinds(&disclosure), [message, header, item(1), end]);
        disclosure.turns.insert(0);
        assert_eq!(kinds(&disclosure), [message, header, item(0), item(1), end]);
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
