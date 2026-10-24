//! Inline code and the files the text names, as chips: the text is
//! rebuilt with room around each (and for a file's icon), and each chip
//! remembers what it is. A file's chip shows its name, its folder when
//! another file of the same name is named too, and the line it points at.

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::{Path, PathBuf},
};

use crate::conversation::markdown::{
    Block, Inline, Style,
    mention::{Mention, mention},
};

/// Room on each side of a chip's text: two narrow spaces that don't
/// break, so a chip wraps to the next line whole.
pub const PAD: &str = "\u{202F}\u{202F}";
/// Room for a file's icon, in spaces that don't break either.
const ICON_ROOM: &str = "\u{A0}\u{A0}\u{A0}\u{A0}\u{202F}";
/// Between a file's name and its folder or place.
const DOT: &str = "\u{A0}·\u{A0}";

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Code,
    /// A file or folder, with where it is on disk.
    Mention {
        mention: Mention,
        path: PathBuf,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    /// The whole chip, its room included.
    pub range: Range<usize>,
    pub kind: Kind,
    /// Where a file's icon goes, and its place (` · L12`), if it has one.
    pub icon: Option<usize>,
    pub place: Option<Range<usize>>,
    pub label: Range<usize>,
    /// What the chip stands for, as copied: its code, or the path as
    /// written.
    pub source: String,
}

/// `inline` with its chips: what to draw and where each chip is.
pub fn decorate(
    inline: &Inline,
    folder: Option<&Path>,
    crowded: &HashSet<String>,
) -> (Inline, Vec<Chip>) {
    // What each chip replaces: the link's text for a link to a path, or
    // the code.
    let mut sources: Vec<(Range<usize>, Option<Mention>, Style, String)> = Vec::new();
    for (range, target) in &inline.links {
        if let Some(mention) = mention(target) {
            sources.push((
                range.clone(),
                Some(mention),
                Style::default(),
                target.clone(),
            ));
        }
    }
    for (range, style) in inline.spans.iter().filter(|(_, style)| style.code) {
        if sources
            .iter()
            .any(|(taken, ..)| taken.start < range.end && range.start < taken.end)
        {
            continue;
        }
        let code = &inline.text[range.clone()];
        sources.push((range.clone(), mention(code), *style, code.to_owned()));
    }
    sources.sort_by_key(|(range, ..)| range.start);
    if sources.is_empty() {
        return (inline.clone(), Vec::new());
    }

    let mut text = String::with_capacity(inline.text.len() + sources.len() * 16);
    let mut chips = Vec::new();
    let mut spans = Vec::new();
    // Where each original byte went: shifts at the end of each source.
    let mut shifts: Vec<(usize, isize)> = Vec::new();
    let taken: Vec<Range<usize>> = sources.iter().map(|(range, ..)| range.clone()).collect();
    let mut from = 0;
    for (range, mention, style, source) in sources {
        text.push_str(&inline.text[from..range.start]);
        let start = text.len();
        text.push_str(PAD);
        let (kind, icon, label, place) = match mention {
            Some(mention) => {
                let icon = text.len();
                text.push_str(ICON_ROOM);
                let label_start = text.len();
                text.push_str(mention.name());
                if crowded.contains(mention.name())
                    && let Some(parent) = mention.parent()
                {
                    text.push_str(DOT);
                    text.push_str(parent);
                }
                let label = label_start..text.len();
                let place = mention.place().map(|place| {
                    let start = text.len();
                    text.push_str(DOT);
                    text.push_str(&place);
                    start..text.len()
                });
                let path = resolve(&mention.path, folder);
                (Kind::Mention { mention, path }, Some(icon), label, place)
            }
            None => {
                let label_start = text.len();
                text.push_str(&inline.text[range.clone()]);
                let label = label_start..text.len();
                spans.push((label.clone(), style));
                (Kind::Code, None, label, None)
            }
        };
        text.push_str(PAD);
        chips.push(Chip {
            range: start..text.len(),
            kind,
            icon,
            place,
            label,
            source,
        });
        shifts.push((range.end, text.len() as isize - range.end as isize));
        from = range.end;
    }
    text.push_str(&inline.text[from..]);

    let moved = |at: usize| {
        let shift = (shifts.iter())
            .take_while(|(end, _)| *end <= at)
            .last()
            .map_or(0, |(_, shift)| *shift);
        at.saturating_add_signed(shift)
    };
    let replaced = |range: &Range<usize>| {
        (taken.iter()).any(|source| source.start < range.end && range.start < source.end)
    };
    for (range, style) in &inline.spans {
        if !replaced(range) {
            spans.push((moved(range.start)..moved(range.end), *style));
        }
    }
    spans.sort_by_key(|(range, _)| range.start);
    let links = (inline.links.iter())
        .filter(|(range, _)| !replaced(range))
        .map(|(range, target)| (moved(range.start)..moved(range.end), target.clone()))
        .collect();
    (Inline { text, spans, links }, chips)
}

/// Where `path` is on disk: from the home folder for `~/`, as written when
/// absolute, else inside the project's folder.
fn resolve(path: &str, folder: Option<&Path>) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    let path = Path::new(path);
    match folder {
        Some(folder) if path.is_relative() => folder.join(path),
        _ => path.to_owned(),
    }
}

/// The names of files the blocks name more than once, at different paths,
/// whose chips need their folders to tell them apart.
pub fn crowded(blocks: &[Block]) -> HashSet<String> {
    let mut paths: HashMap<String, HashSet<String>> = HashMap::new();
    let mut add = |inline: &Inline| {
        let named = (inline.links.iter().map(|(_, target)| target.as_str())).chain(
            (inline.spans.iter())
                .filter(|(_, style)| style.code)
                .filter_map(|(range, _)| inline.text.get(range.clone())),
        );
        for mention in named.filter_map(mention) {
            let path = mention.path.trim_end_matches(['/', '\\']).to_owned();
            paths
                .entry(mention.name().to_owned())
                .or_default()
                .insert(path);
        }
    };
    walk(blocks, &mut add);
    (paths.into_iter())
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(name, _)| name)
        .collect()
}

fn walk(blocks: &[Block], add: &mut impl FnMut(&Inline)) {
    for block in blocks {
        match block {
            Block::Paragraph(inline) | Block::Heading(_, inline) => add(inline),
            Block::List { items, .. } => {
                for item in items {
                    walk(&item.blocks, add);
                }
            }
            Block::Quote(blocks) => walk(blocks, add),
            Block::Table(table) => {
                for cell in table.head.iter().chain(table.rows.iter().flatten()) {
                    add(cell);
                }
            }
            Block::Code(_) | Block::Rule => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::markdown::{Block, parse};

    fn first(text: &str) -> Inline {
        match parse(text).into_iter().next() {
            Some(Block::Paragraph(inline)) => inline,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn code_and_files_become_chips_with_room() {
        let inline = first(
            "Run `cargo test` on **`src/app.rs:12`** then [see](https://x.io) [app](/r/src/app.rs).",
        );
        let (shown, chips) = decorate(&inline, Some(Path::new("/repo")), &HashSet::new());
        let labels: Vec<&str> = chips
            .iter()
            .map(|chip| &shown.text[chip.label.clone()])
            .collect();
        assert_eq!(labels, ["cargo test", "app.rs", "app.rs"]);
        assert_eq!(
            &shown.text[chips[0].range.clone()],
            "\u{202F}\u{202F}cargo test\u{202F}\u{202F}"
        );
        assert_eq!(
            chips[1].place.clone().map(|place| &shown.text[place]),
            Some("\u{A0}·\u{A0}L12")
        );
        let Kind::Mention { path, .. } = &chips[1].kind else {
            panic!();
        };
        assert_eq!(path, Path::new("/repo/src/app.rs"));
        let Kind::Mention { path, .. } = &chips[2].kind else {
            panic!();
        };
        assert_eq!(path, Path::new("/r/src/app.rs"));
        // The code keeps its font inside its room; the web link moved with it.
        assert!(
            shown
                .spans
                .iter()
                .any(|(range, style)| style.code && *range == chips[0].label)
        );
        let [(range, target)] = &shown.links[..] else {
            panic!("{:?}", shown.links);
        };
        assert_eq!(
            (&shown.text[range.clone()], target.as_str()),
            ("see", "https://x.io")
        );
        assert!(shown.text.starts_with("Run ") && shown.text.ends_with('.'));
    }

    #[test]
    fn two_files_of_one_name_show_their_folders() {
        let blocks = parse("`a/mod.rs` and `b/mod.rs`, `a/mod.rs` again, `c/lib.rs`");
        let crowded = crowded(&blocks);
        assert_eq!(crowded, HashSet::from(["mod.rs".to_owned()]));
        let Some(Block::Paragraph(inline)) = blocks.first() else {
            panic!();
        };
        let (shown, chips) = decorate(inline, None, &crowded);
        let labels: Vec<&str> = chips
            .iter()
            .map(|chip| &shown.text[chip.label.clone()])
            .collect();
        assert_eq!(
            labels,
            [
                "mod.rs\u{A0}·\u{A0}a",
                "mod.rs\u{A0}·\u{A0}b",
                "mod.rs\u{A0}·\u{A0}a",
                "lib.rs"
            ]
        );
    }

    #[test]
    fn text_without_chips_stays_as_it_is() {
        let inline = first("Plain **bold** and https://x.io");
        assert_eq!(
            decorate(&inline, None, &HashSet::new()),
            (inline, Vec::new())
        );
    }
}
