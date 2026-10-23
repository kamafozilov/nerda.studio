//! The agent's Markdown as blocks the conversation draws: paragraphs and
//! headings of styled runs, code blocks, lists and quotes. Links show as
//! their text. A line break stays one, as in the terminal the agent writes
//! for. Text still streaming parses as it stands: a code fence not closed
//! yet runs to the end.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Inline),
    /// Set apart by weight, not size.
    Heading(Inline),
    Code(String),
    /// Each item's blocks; numbered from `start` when ordered.
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Quote(Vec<Block>),
    Rule,
}

/// Text with the ranges styled apart from the rest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
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
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
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
            spans: Vec::new(),
        },
    }
}

enum Open {
    Root(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Item(Vec<Block>),
    Quote(Vec<Block>),
}

#[derive(Default)]
struct Builder {
    open: Vec<Open>,
    /// The paragraph or heading being written, and whether it is a heading.
    inline: Option<(Inline, bool)>,
    code: Option<String>,
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
                Some(code) => code.push_str(&text),
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
            Event::TaskListMarker(done) => self.push(if done { "☑ " } else { "☐ " }, false),
            Event::FootnoteReference(label) => self.push(&format!("[{label}]"), false),
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.open_inline(false),
            Tag::Heading { .. } => self.open_inline(true),
            Tag::CodeBlock(_) => {
                self.close_inline();
                self.code = Some(String::new());
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
                self.open.push(Open::Item(Vec::new()));
            }
            Tag::BlockQuote(_) => {
                self.close_inline();
                self.open.push(Open::Quote(Vec::new()));
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.close_inline(),
            TagEnd::CodeBlock => {
                if let Some(mut code) = self.code.take() {
                    if code.ends_with('\n') {
                        code.pop();
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
                if let Some(Open::Item(blocks)) = self.open.pop()
                    && let Some(Open::List { items, .. }) = self.open.last_mut()
                {
                    items.push(blocks);
                }
            }
            TagEnd::BlockQuote(_) => {
                self.close_inline();
                if let Some(Open::Quote(blocks)) = self.open.pop() {
                    self.add(Block::Quote(blocks));
                }
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            _ => {}
        }
    }

    /// Text in the current style; a tight list item's text has no
    /// paragraph of its own, so it opens one.
    fn push(&mut self, text: &str, code: bool) {
        if self.inline.is_none() {
            self.open_inline(false);
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
        inline.text.push_str(text);
        if style != Style::default() {
            inline.spans.push((start..inline.text.len(), style));
        }
    }

    fn open_inline(&mut self, heading: bool) {
        self.close_inline();
        self.inline = Some((Inline::default(), heading));
    }

    fn close_inline(&mut self) {
        if let Some((inline, heading)) = self.inline.take()
            && !inline.text.trim().is_empty()
        {
            self.add(match heading {
                true => Block::Heading(inline),
                false => Block::Paragraph(inline),
            });
        }
    }

    fn add(&mut self, block: Block) {
        match self.open.last_mut() {
            Some(Open::Root(blocks) | Open::Item(blocks) | Open::Quote(blocks)) => {
                blocks.push(block)
            }
            // A block straight inside a list, which Markdown doesn't make.
            Some(Open::List { items, .. }) => items.push(vec![block]),
            None => self.open.push(Open::Root(vec![block])),
        }
    }

    /// Closes whatever the text left open, as streaming text does.
    fn finish(mut self) -> Vec<Block> {
        self.close_inline();
        if let Some(code) = self.code.take() {
            self.add(Block::Code(code));
        }
        while self.open.len() > 1 {
            let block = match self.open.pop() {
                Some(Open::List { start, items }) => Block::List { start, items },
                Some(Open::Item(blocks)) => {
                    if let Some(Open::List { items, .. }) = self.open.last_mut() {
                        items.push(blocks);
                    }
                    continue;
                }
                Some(Open::Quote(blocks)) => Block::Quote(blocks),
                Some(Open::Root(blocks)) => Block::Quote(blocks),
                None => break,
            };
            self.add(block);
        }
        match self.open.pop() {
            Some(Open::Root(blocks)) => blocks,
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(text: &str) -> Block {
        Block::Paragraph(Inline {
            text: text.to_owned(),
            spans: Vec::new(),
        })
    }

    #[test]
    fn styles_are_runs_of_the_text() {
        let blocks = parse("Say **bold** and *it* with `code`, ~~gone~~ [here](https://x.y).");
        let [Block::Paragraph(inline)] = &blocks[..] else {
            panic!("{blocks:?}");
        };
        assert_eq!(inline.text, "Say bold and it with code, gone here.");
        let styled: Vec<(&str, Style)> = (inline.spans.iter())
            .map(|(range, style)| (&inline.text[range.clone()], *style))
            .collect();
        let bold = Style {
            bold: true,
            ..Style::default()
        };
        let italic = Style {
            italic: true,
            ..Style::default()
        };
        let code = Style {
            code: true,
            ..Style::default()
        };
        let strike = Style {
            strike: true,
            ..Style::default()
        };
        assert_eq!(
            styled,
            [
                ("bold", bold),
                ("it", italic),
                ("code", code),
                ("gone", strike)
            ]
        );
    }

    #[test]
    fn headings_code_lists_and_quotes_are_blocks() {
        let text = "# Title\n\nText\n\n```rust\nfn main() {}\n```\n\n1. one\n2. two\n   - inner\n\n> said\n\n---";
        let blocks = parse(text);
        let heading = Block::Heading(Inline {
            text: "Title".to_owned(),
            spans: Vec::new(),
        });
        let inner = Block::List {
            start: None,
            items: vec![vec![paragraph("inner")]],
        };
        assert_eq!(
            blocks,
            [
                heading,
                paragraph("Text"),
                Block::Code("fn main() {}".to_owned()),
                Block::List {
                    start: Some(1),
                    items: vec![vec![paragraph("one")], vec![paragraph("two"), inner]],
                },
                Block::Quote(vec![paragraph("said")]),
                Block::Rule,
            ]
        );
    }

    #[test]
    fn a_fence_still_streaming_runs_to_the_end() {
        let blocks = parse("Here:\n\n```\nlet a = 1;\nlet b");
        assert_eq!(
            blocks,
            [
                paragraph("Here:"),
                Block::Code("let a = 1;\nlet b".to_owned())
            ]
        );
    }

    #[test]
    fn line_breaks_stay() {
        assert_eq!(parse("one\ntwo  \nthree"), [paragraph("one\ntwo\nthree")]);
        assert_eq!(
            parse("Кириллица и oʻzbek"),
            [paragraph("Кириллица и oʻzbek")]
        );
        assert!(parse("  \n\n ").is_empty());
    }

    #[test]
    fn a_problem_line_keeps_its_command_apart() {
        let line = inline("Install it with `curl -fsSL https://claude.ai/install.sh | bash` now.");
        assert_eq!(
            line.text,
            "Install it with curl -fsSL https://claude.ai/install.sh | bash now."
        );
        assert_eq!(line.spans.len(), 1);
        assert_eq!(inline("# not a heading here").text, "# not a heading here");
    }
}
