//! Text selected in the agent's reply. Each text a reply draws (a
//! paragraph, a heading, a cell, a code block) is a piece, numbered in the
//! order it draws; a selection runs from one spot to another, a spot being
//! a piece and a byte in it, inside one reply. Copying gives the pieces'
//! text, a chip as what it stands for: its code, or its path.

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ops::Range,
    path::PathBuf,
    rc::Rc,
};

/// A piece, by its number in its reply, and a byte in its text.
pub type Spot = (usize, usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The reply, by the key of its turn and item.
    pub item: u64,
    /// Where the selection began, and where it reaches now.
    pub anchor: Spot,
    pub head: Spot,
}

impl Selection {
    /// Its first spot and its last.
    pub fn ordered(&self) -> (Spot, Spot) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

/// What a click on a part of the text opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Web(String),
    File(PathBuf),
}

/// One text as drawn: what it shows, what goes before it when copied
/// after another (a line break, or a tab between cells of a row), and
/// what each chip in it stands for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Piece {
    pub text: String,
    pub before: &'static str,
    pub sources: Vec<(Range<usize>, String)>,
}

#[derive(Default)]
pub struct Selecting {
    pub selection: Option<Selection>,
    /// The pointer is down and drawing out the selection.
    pub dragging: bool,
    pieces: HashMap<u64, BTreeMap<usize, Piece>>,
}

pub type Shared = Rc<RefCell<Selecting>>;

impl Selecting {
    /// Reply `item` draws again: its pieces come anew.
    pub fn redraw(&mut self, item: u64) {
        self.pieces.remove(&item);
    }

    pub fn add(&mut self, item: u64, number: usize, piece: Piece) {
        self.pieces.entry(item).or_default().insert(number, piece);
    }

    /// The part of piece `number` of `item`, `length` long, selected.
    pub fn selected_in(&self, item: u64, number: usize, length: usize) -> Option<Range<usize>> {
        let selection = self.selection.filter(|s| s.item == item && !s.is_empty())?;
        let ((first, from), (last, to)) = selection.ordered();
        if number < first || last < number {
            return None;
        }
        let start = if number == first { from.min(length) } else { 0 };
        let end = if number == last {
            to.min(length)
        } else {
            length
        };
        (start < end).then_some(start..end)
    }

    /// The whole of reply `item`.
    pub fn select_all(&mut self, item: u64) {
        let Some(pieces) = self.pieces.get(&item) else {
            return;
        };
        let (Some((&first, _)), Some((&last, piece))) =
            (pieces.first_key_value(), pieces.last_key_value())
        else {
            return;
        };
        self.selection = Some(Selection {
            item,
            anchor: (first, 0),
            head: (last, piece.text.len()),
        });
    }

    /// The selected text, as copied.
    pub fn copied(&self) -> Option<String> {
        let selection = self.selection.filter(|s| !s.is_empty())?;
        let pieces = self.pieces.get(&selection.item)?;
        let ((first, _), (last, _)) = selection.ordered();
        let mut copied = String::new();
        for (&number, piece) in pieces.range(first..=last) {
            let Some(range) = self.selected_in(selection.item, number, piece.text.len()) else {
                continue;
            };
            if number != first {
                copied.push_str(piece.before);
            }
            copied.push_str(&text_of(piece, range));
        }
        (!copied.is_empty()).then_some(copied)
    }

    /// Whether a reply has something selected.
    pub fn has_selection(&self) -> bool {
        self.copied().is_some()
    }
}

/// `range` of a piece as copied: a chip the range touches gives what it
/// stands for, whole; Uzbek's ‘ after o or g is its ʻ again.
fn text_of(piece: &Piece, range: Range<usize>) -> String {
    let mut copied = String::new();
    let mut at = range.start;
    while at < range.end {
        if let Some((chip, source)) =
            (piece.sources.iter()).find(|(chip, _)| chip.start <= at && at < chip.end)
        {
            copied.push_str(source);
            at = chip.end;
            continue;
        }
        let next = (piece.sources.iter())
            .map(|(chip, _)| chip.start)
            .filter(|start| *start > at)
            .min()
            .unwrap_or(range.end)
            .min(range.end);
        copied.push_str(piece.text.get(at..next).unwrap_or_default());
        at = next;
    }
    let mut uzbek = String::with_capacity(copied.len());
    let mut before = None;
    for c in copied.chars() {
        let okina = c == '\u{2018}' && matches!(before, Some('o' | 'O' | 'g' | 'G'));
        uzbek.push(if okina { '\u{2BB}' } else { c });
        before = Some(c);
    }
    uzbek
}

/// The word around byte `at` of `text`: letters, digits and `_`, or the run
/// of other characters there that aren't spaces.
pub fn word_at(text: &str, at: usize) -> Range<usize> {
    let wordy = |c: char| c.is_alphanumeric() || c == '_' || c == '\u{2018}' || c == '\u{2019}';
    let at = at.min(text.len());
    let Some(here) = text[at..].chars().next() else {
        return at..at;
    };
    if here.is_whitespace() {
        return at..at + here.len_utf8();
    }
    let same = |c: char| !c.is_whitespace() && wordy(c) == wordy(here);
    let start = (text[..at].char_indices().rev())
        .take_while(|(_, c)| same(*c))
        .last()
        .map_or(at, |(i, _)| i);
    let end = (text[at..].char_indices())
        .find(|(_, c)| !same(*c))
        .map_or(text.len(), |(i, _)| at + i);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pieces: &[(&str, &'static str)]) -> Selecting {
        let mut selecting = Selecting::default();
        for (number, (text, before)) in pieces.iter().enumerate() {
            let piece = Piece {
                text: (*text).to_owned(),
                before,
                sources: Vec::new(),
            };
            selecting.add(1, number, piece);
        }
        selecting
    }

    #[test]
    fn a_selection_across_pieces_copies_their_parts() {
        let mut selecting = with(&[
            ("First line", ""),
            ("A", "\n"),
            ("B", "\t"),
            ("Last one", "\n"),
        ]);
        selecting.selection = Some(Selection {
            item: 1,
            anchor: (3, 4),
            head: (0, 6),
        });
        assert_eq!(selecting.copied().as_deref(), Some("line\nA\tB\nLast"));
        assert_eq!(selecting.selected_in(1, 0, 10), Some(6..10));
        assert_eq!(selecting.selected_in(1, 2, 1), Some(0..1));
        assert_eq!(selecting.selected_in(2, 2, 1), None);
        selecting.select_all(1);
        assert_eq!(
            selecting.copied().as_deref(),
            Some("First line\nA\tB\nLast one")
        );
    }

    #[test]
    fn a_chip_copies_what_it_stands_for() {
        let mut selecting = Selecting::default();
        let text = "See \u{202F}main.rs\u{A0}·\u{A0}L4\u{202F} now o\u{2018}zbek";
        let chip = 4..text.find(" now").unwrap();
        let piece = Piece {
            text: text.to_owned(),
            before: "",
            sources: vec![(chip.clone(), "src/main.rs:4".to_owned())],
        };
        selecting.add(1, 0, piece);
        selecting.selection = Some(Selection {
            item: 1,
            anchor: (0, 0),
            head: (0, chip.start + 3),
        });
        assert_eq!(selecting.copied().as_deref(), Some("See src/main.rs:4"));
        selecting.select_all(1);
        assert_eq!(
            selecting.copied().as_deref(),
            Some("See src/main.rs:4 now o\u{2BB}zbek")
        );
    }

    #[test]
    fn a_double_click_takes_the_word() {
        let text = "fn main_loop() { o\u{2018}zbek }";
        let word = |at| &text[word_at(text, at)];
        assert_eq!(word(4), "main_loop");
        assert_eq!(word(0), "fn");
        assert_eq!(word(text.find("o\u{2018}").unwrap() + 1), "o\u{2018}zbek");
        assert_eq!(word(text.len()), "");
    }
}
