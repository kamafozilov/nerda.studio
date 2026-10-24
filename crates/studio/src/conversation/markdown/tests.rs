use super::*;

fn paragraph(text: &str) -> Block {
    Block::Paragraph(Inline {
        text: text.to_owned(),
        ..Inline::default()
    })
}

fn item(blocks: Vec<Block>) -> ListItem {
    ListItem { task: None, blocks }
}

fn code(language: Option<&str>, text: &str) -> Block {
    Block::Code(Code {
        language: language.map(str::to_owned),
        text: text.to_owned(),
    })
}

fn texts(cells: &[Inline]) -> Vec<&str> {
    cells.iter().map(|cell| cell.text.as_str()).collect()
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
    let links: Vec<(&str, &str)> = (inline.links.iter())
        .map(|(range, to)| (&inline.text[range.clone()], to.as_str()))
        .collect();
    assert_eq!(links, [("here", "https://x.y")]);
}

#[test]
fn headings_code_lists_and_quotes_are_blocks() {
    let text = "# Title\n\nText\n\n```rust\nfn main() {}\n```\n\n1. one\n2. two\n   - inner\n\n> said\n\n---";
    let blocks = parse(text);
    let heading = Block::Heading(
        1,
        Inline {
            text: "Title".to_owned(),
            ..Inline::default()
        },
    );
    let inner = Block::List {
        start: None,
        items: vec![item(vec![paragraph("inner")])],
    };
    assert_eq!(
        blocks,
        [
            heading,
            paragraph("Text"),
            code(Some("rust"), "fn main() {}"),
            Block::List {
                start: Some(1),
                items: vec![
                    item(vec![paragraph("one")]),
                    item(vec![paragraph("two"), inner])
                ],
            },
            Block::Quote(vec![paragraph("said")]),
            Block::Rule,
        ]
    );
}

#[test]
fn a_fence_keeps_its_language_alone() {
    let blocks = parse("```ts title=\"a.ts\"\nlet a\n```\n\n```\nplain\n```\n\n    indented");
    assert_eq!(
        blocks,
        [
            code(Some("ts"), "let a"),
            code(None, "plain"),
            code(None, "indented")
        ]
    );
}

#[test]
fn a_fence_still_streaming_runs_to_the_end() {
    let blocks = parse("Here:\n\n```py\nlet a = 1;\nlet b");
    assert_eq!(
        blocks,
        [paragraph("Here:"), code(Some("py"), "let a = 1;\nlet b")]
    );
}

#[test]
fn a_table_has_its_head_rows_and_alignment() {
    let blocks =
        parse("| App | Who | N |\n|:---|:---:|---:|\n| **Desk** | admins | 2 |\n| Shell | | 10 |");
    let [Block::Table(table)] = &blocks[..] else {
        panic!("{blocks:?}");
    };
    assert_eq!(table.align, [Align::Start, Align::Center, Align::End]);
    assert_eq!(texts(&table.head), ["App", "Who", "N"]);
    assert_eq!(table.rows.len(), 2);
    assert_eq!(texts(&table.rows[0]), ["Desk", "admins", "2"]);
    assert!(table.rows[0][0].spans[0].1.bold);
    assert_eq!(texts(&table.rows[1]), ["Shell", "", "10"]);
}

#[test]
fn a_table_still_streaming_shows_the_rows_that_came() {
    let blocks = parse("Apps:\n\n| A | B |\n|---|---|\n| one | two |\n| thr");
    let [_, Block::Table(table)] = &blocks[..] else {
        panic!("{blocks:?}");
    };
    assert_eq!(texts(&table.head), ["A", "B"]);
    assert_eq!(texts(&table.rows[0]), ["one", "two"]);
    assert_eq!(texts(&table.rows[1]), ["thr", ""]);
    // Without its delimiter row yet, a table is still a paragraph.
    assert_eq!(parse("| A | B |"), [paragraph("| A | B |")]);
}

#[test]
fn a_task_list_keeps_its_boxes() {
    let blocks = parse("- [x] done\n- [ ] to do\n- plain");
    let [Block::List { items, .. }] = &blocks[..] else {
        panic!("{blocks:?}");
    };
    let tasks: Vec<Option<bool>> = items.iter().map(|item| item.task).collect();
    assert_eq!(tasks, [Some(true), Some(false), None]);
    assert_eq!(items[1].blocks, [paragraph("to do")]);
}

#[test]
fn bare_addresses_are_links() {
    let blocks = parse(
        "See https://example.com/a_(b), or (https://x.io/y). Not `https://in.code` nor xhttps://no.",
    );
    let [Block::Paragraph(inline)] = &blocks[..] else {
        panic!("{blocks:?}");
    };
    let links: Vec<&str> = (inline.links.iter())
        .map(|(range, to)| {
            assert_eq!(&inline.text[range.clone()], to);
            to.as_str()
        })
        .collect();
    assert_eq!(links, ["https://example.com/a_(b)", "https://x.io/y"]);
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
    assert!(line.links.is_empty());
    assert_eq!(inline("# not a heading here").text, "# not a heading here");
}
