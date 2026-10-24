//! The agent's Markdown as blocks the conversation draws: paragraphs and
//! headings of styled runs, code blocks with their language, lists (task
//! lists among them), quotes and tables. Links, and web addresses written
//! out bare, are runs with their target. A line break stays one, as in the
//! terminal the agent writes for. Text still streaming parses as it stands:
//! a code fence not closed yet runs to the end, a table shows the rows that
//! came.

mod links;
pub mod mention;
#[cfg(test)]
mod tests;

use std::ops::Range;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Inline),
    /// Its level, 1 to 6, and its text.
    Heading(u8, Inline),
    Code(Code),
    /// Each item's blocks; numbered from `start` when ordered.
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Quote(Vec<Block>),
    Table(Table),
    Rule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    /// The fence's first word, such as `rust`, as written.
    pub language: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListItem {
    /// A task list's box: whether it is checked.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    /// How each column aligns; as many as the header has cells.
    pub align: Vec<Align>,
    pub head: Vec<Inline>,
    /// Each row's cells, as many as the header's: one a row left out is
    /// empty.
    pub rows: Vec<Vec<Inline>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// Text with the ranges styled apart from the rest, and the ranges that
/// link somewhere.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
    pub links: Vec<(Range<usize>, String)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
}

/// `text` as blocks.
pub fn parse(text: &str) -> Vec<Block> {
    let mut builder = Builder::default();
    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
    for event in Parser::new_ext(text, options) {
        builder.event(event);
    }
    builder.finish()
}

/// `text` as one line of inline Markdown, such as a problem's words with a
/// command in backticks.
pub fn inline(text: &str) -> Inline {
    let mut blocks = parse(text).into_iter();
    match (blocks.next(), blocks.next()) {
        (Some(Block::Paragraph(inline)), None) => inline,
        _ => Inline {
            text: text.to_owned(),
            ..Inline::default()
        },
    }
}

enum Open {
    Root(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Item(ListItem),
    Quote(Vec<Block>),
    Table {
        table: Table,
        row: Vec<Inline>,
    },
}

/// What the inline being written is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Paragraph,
    Heading(u8),
    Cell,
}

#[derive(Default)]
struct Builder {
    open: Vec<Open>,
    inline: Option<(Inline, Kind)>,
    code: Option<Code>,
    /// Where each link open now starts in the inline, and its target.
    linking: Vec<(usize, String)>,
    bold: usize,
    italic: usize,
    strike: usize,
}

impl Builder {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => match &mut self.code {
                Some(code) => code.text.push_str(&text),
                None => self.push(&text, false),
            },
            Event::Code(text) => self.push(&text, true),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.push(&text, true),
            Event::Html(text) | Event::InlineHtml(text) => self.push(&text, false),
            Event::SoftBreak | Event::HardBreak => self.push("\n", false),
            Event::Rule => {
                self.close_inline();
                self.add(Block::Rule);
            }
            Event::TaskListMarker(done) => {
                if let Some(Open::Item(item)) = self.open.last_mut() {
                    item.task = Some(done);
                }
            }
            Event::FootnoteReference(label) => self.push(&format!("[{label}]"), false),
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.open_inline(Kind::Paragraph),
            Tag::Heading { level, .. } => self.open_inline(Kind::Heading(level_number(level))),
            Tag::CodeBlock(kind) => {
                self.close_inline();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split(|c: char| c.is_whitespace() || c == ',' || c == '{')
                        .next()
                        .filter(|word| !word.is_empty())
                        .map(str::to_owned),
                    CodeBlockKind::Indented => None,
                };
                self.code = Some(Code {
                    language,
                    text: String::new(),
                });
            }
            Tag::List(start) => {
                self.close_inline();
                self.open.push(Open::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.close_inline();
                self.open.push(Open::Item(ListItem::default()));
            }
            Tag::BlockQuote(_) => {
                self.close_inline();
                self.open.push(Open::Quote(Vec::new()));
            }
            Tag::Table(align) => {
                self.close_inline();
                let align = (align.into_iter())
                    .map(|align| match align {
                        Alignment::Center => Align::Center,
                        Alignment::Right => Align::End,
                        Alignment::None | Alignment::Left => Align::Start,
                    })
                    .collect();
                self.open.push(Open::Table {
                    table: Table {
                        align,
                        ..Table::default()
                    },
                    row: Vec::new(),
                });
            }
            Tag::TableCell => self.open_inline(Kind::Cell),
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                let at = self
                    .inline
                    .as_ref()
                    .map_or(0, |(inline, _)| inline.text.len());
                self.linking.push((at, dest_url.into_string()));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.close_inline(),
            TagEnd::CodeBlock => {
                if let Some(mut code) = self.code.take() {
                    if code.text.ends_with('\n') {
                        code.text.pop();
                    }
                    self.add(Block::Code(code));
                }
            }
            TagEnd::List(_) => {
                self.close_inline();
                if let Some(Open::List { start, items }) = self.open.pop() {
                    self.add(Block::List { start, items });
                }
            }
            TagEnd::Item => {
                self.close_inline();
                self.close_item();
            }
            TagEnd::BlockQuote(_) => {
                self.close_inline();
                if let Some(Open::Quote(blocks)) = self.open.pop() {
                    self.add(Block::Quote(blocks));
                }
            }
            TagEnd::TableCell => {
                let cell = self.take_inline().map(|(inline, _)| inline);
                if let Some(Open::Table { row, .. }) = self.open.last_mut() {
                    row.push(cell.unwrap_or_default());
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(Open::Table { table, row }) = self.open.last_mut() {
                    let row = std::mem::take(row);
                    match tag {
                        TagEnd::TableHead => table.head = row,
                        _ => table.rows.push(row),
                    }
                }
            }
            TagEnd::Table => self.close_table(),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link | TagEnd::Image => {
                if let Some((start, target)) = self.linking.pop()
                    && let Some((inline, _)) = &mut self.inline
                    && start < inline.text.len()
                {
                    inline.links.push((start..inline.text.len(), target));
                }
            }
            _ => {}
        }
    }

    /// Text in the current style; a tight list item's text has no
    /// paragraph of its own, so it opens one.
    fn push(&mut self, text: &str, code: bool) {
        if self.inline.is_none() {
            self.open_inline(Kind::Paragraph);
        }
        let style = Style {
            bold: self.bold > 0,
            italic: self.italic > 0,
            code,
            strike: self.strike > 0,
        };
        let Some((inline, _)) = &mut self.inline else {
            return;
        };
        let start = inline.text.len();
        // Uzbek's oʻ and gʻ (U+02BB, and U+02BC) as the quotes that look
        // the same: text wraps inside words of those, not of these.
        if text.contains(['\u{2BB}', '\u{2BC}']) {
            inline.text.push_str(
                &text
                    .replace('\u{2BB}', "\u{2018}")
                    .replace('\u{2BC}', "\u{2019}"),
            );
        } else {
            inline.text.push_str(text);
        }
        if style != Style::default() {
            inline.spans.push((start..inline.text.len(), style));
        }
    }

    fn open_inline(&mut self, kind: Kind) {
        self.close_inline();
        self.inline = Some((Inline::default(), kind));
    }

    /// The inline being written, its bare web addresses made links.
    fn take_inline(&mut self) -> Option<(Inline, Kind)> {
        let (mut inline, kind) = self.inline.take()?;
        links::find_bare(&mut inline);
        Some((inline, kind))
    }

    fn close_inline(&mut self) {
        let Some((inline, kind)) = self.take_inline() else {
            return;
        };
        if inline.text.trim().is_empty() {
            return;
        }
        self.add(match kind {
            Kind::Heading(level) => Block::Heading(level, inline),
            Kind::Paragraph | Kind::Cell => Block::Paragraph(inline),
        });
    }

    fn close_item(&mut self) {
        if let Some(Open::Item(item)) = self.open.pop()
            && let Some(Open::List { items, .. }) = self.open.last_mut()
        {
            items.push(item);
        }
    }

    fn close_table(&mut self) {
        if let Some(Open::Table { mut table, row }) = self.open.pop() {
            if !row.is_empty() {
                match table.head.is_empty() {
                    true => table.head = row,
                    false => table.rows.push(row),
                }
            }
            self.add(Block::Table(table));
        }
    }

    fn add(&mut self, block: Block) {
        match self.open.last_mut() {
            Some(Open::Root(blocks) | Open::Quote(blocks)) => blocks.push(block),
            Some(Open::Item(item)) => item.blocks.push(block),
            // A block straight inside a list or a table, which Markdown
            // doesn't make.
            Some(Open::List { items, .. }) => items.push(ListItem {
                task: None,
                blocks: vec![block],
            }),
            Some(Open::Table { .. }) => {}
            None => self.open.push(Open::Root(vec![block])),
        }
    }

    /// Closes whatever the text left open, as streaming text does.
    fn finish(mut self) -> Vec<Block> {
        if let Some((cell, Kind::Cell)) = self.take_inline()
            && let Some(Open::Table { row, .. }) = self.open.last_mut()
        {
            row.push(cell);
        }
        self.close_inline();
        if let Some(code) = self.code.take() {
            self.add(Block::Code(code));
        }
        while self.open.len() > 1 {
            match self.open.last() {
                Some(Open::List { .. }) => {
                    if let Some(Open::List { start, items }) = self.open.pop() {
                        self.add(Block::List { start, items });
                    }
                }
                Some(Open::Item(_)) => self.close_item(),
                Some(Open::Table { .. }) => self.close_table(),
                Some(Open::Quote(_) | Open::Root(_)) => {
                    if let Some(Open::Quote(blocks) | Open::Root(blocks)) = self.open.pop() {
                        self.add(Block::Quote(blocks));
                    }
                }
                None => break,
            }
        }
        match self.open.pop() {
            Some(Open::Root(blocks)) => blocks,
            _ => Vec::new(),
        }
    }
}

fn level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
