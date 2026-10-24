//! A table in a frame: a `border` hairline with `MENU_RADIUS` corners, its
//! head on the field's fill in semibold, a hairline between rows, no lines
//! between columns. Each column is as wide as its widest cell, up to
//! `CELL_MAX_WIDTH`, and shares out the room left over; past that a cell
//! wraps, down to `CELL_MIN_WIDTH` or its longest word, its head never.
//! When the columns can't go that narrow in the column of text, the table
//! scrolls sideways inside its frame.

use std::collections::HashSet;

use gpui::{
    AnyElement, Div, ElementId, Font, FontWeight, IntoElement, ParentElement, Pixels, SharedString,
    Styled, TextRun, Window, div, prelude::*, px,
};
use theme::{HAIRLINE, MENU_RADIUS, MONOSPACE_FONT, SPACE_2, SPACE_3, TEXT_BODY};

use super::{Env, Tail, chips, inline, nested};
use crate::conversation::markdown::{Align, Inline, Table};

/// The widest a column grows to fit its text before its cells wrap.
const CELL_MAX_WIDTH: Pixels = px(320.);
/// The narrowest a column wraps its cells to, when its text is wider.
const CELL_MIN_WIDTH: Pixels = px(120.);

pub fn block(table: &Table, id: u64, tail: &Tail, env: &mut Env) -> AnyElement {
    let colors = env.colors;
    let count = table.head.len().max(1);
    let padding = SPACE_3 * 2.;
    let columns: Vec<(Pixels, Pixels)> = (0..count)
        .map(|column| {
            let cells = (table.rows.iter())
                .filter_map(|row| row.get(column).map(|cell| (cell, false)))
                .chain(table.head.get(column).map(|cell| (cell, true)));
            let (mut widest, mut word, mut head_width) = (px(0.), px(0.), px(0.));
            for (cell, head) in cells {
                let (cell, _) = chips::decorate(cell, None, &HashSet::new());
                let (line, longest) = measure(env.window, &cell, head);
                widest = widest.max(line);
                word = word.max(longest);
                if head {
                    head_width = line;
                }
            }
            // A column wraps its cells only down to `CELL_MIN_WIDTH`, and
            // never its head; a table that can't fit then scrolls.
            let width = widest.min(CELL_MAX_WIDTH);
            let least = word.max(CELL_MIN_WIDTH).max(head_width).min(width);
            // A pixel's slack, so a word as wide as its cell doesn't wrap.
            (width + padding + px(1.), least + padding + px(1.))
        })
        .collect();
    let narrowest: Pixels = columns.iter().map(|(_, least)| *least).sum();
    let last = table.rows.len();
    let row = |cells: &[Inline], at: usize, env: &mut Env| {
        let tail = if at == last { tail } else { &[] };
        div()
            .flex()
            .w_full()
            .min_w(narrowest)
            .when(at != last, |row| {
                row.border_b(HAIRLINE).border_color(colors.border)
            })
            .children(columns.iter().enumerate().map(|(column, &(width, least))| {
                let tail = if column + 1 == cells.len() { tail } else { &[] };
                let id = ElementId::NamedInteger("cell".into(), nested(nested(id, at), column));
                let align = table.align.get(column).copied().unwrap_or_default();
                aligned(div(), align)
                    .flex_basis(width)
                    .flex_grow(f32::from(width))
                    .flex_shrink(1.)
                    .min_w(least)
                    .px(SPACE_3)
                    .py(SPACE_2)
                    .children(cells.get(column).map(|cell| {
                        // Copied, a row's cells stand a tab apart.
                        if let Some(selecting) = env.selecting.as_mut() {
                            selecting.before = if column == 0 { "\n" } else { "\t" };
                        }
                        div().min_w_0().child(inline::text(cell, id, tail, env))
                    }))
            }))
    };
    let head = row(&table.head, usize::MAX, env)
        .bg(colors.field_background)
        .font_weight(FontWeight::SEMIBOLD);
    let rows: Vec<Div> = (table.rows.iter().enumerate())
        .map(|(at, cells)| row(cells, at + 1, env))
        .collect();
    div()
        .w_full()
        .rounded(MENU_RADIUS)
        .border(HAIRLINE)
        .border_color(colors.border)
        .overflow_hidden()
        .child(
            div()
                .id(ElementId::NamedInteger("table".into(), id))
                .w_full()
                .overflow_x_scroll()
                .restrict_scroll_to_axis()
                .text_size(TEXT_BODY)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w(narrowest)
                        .child(head)
                        .children(rows),
                ),
        )
        .into_any_element()
}

/// A cell's content where its column aligns it. The text stays as wide as
/// it is and the cell places it, rather than the text aligning its own
/// lines, so a selection and chips drawn from the text's layout stay
/// under it.
fn aligned(cell: Div, align: Align) -> Div {
    let cell = cell.flex().items_start();
    match align {
        Align::Start => cell,
        Align::Center => cell.justify_center(),
        Align::End => cell.justify_end(),
    }
}

/// How wide `cell` is on one line at the table's size, its code in the
/// monospaced font, and its longest word, which a cell can't wrap.
fn measure(window: &Window, cell: &Inline, head: bool) -> (Pixels, Pixels) {
    let mut font = window.text_style().font();
    if head {
        font.weight = FontWeight::SEMIBOLD;
    }
    let mono = Font {
        family: SharedString::new_static(MONOSPACE_FONT),
        ..font.clone()
    };
    let font_at = |at: usize| {
        let code = (cell.spans.iter()).any(|(range, style)| style.code && range.contains(&at));
        if code { mono.clone() } else { font.clone() }
    };
    let width = |start: usize, part: &str| {
        let mut runs: Vec<TextRun> = Vec::new();
        for (at, c) in part.char_indices() {
            let font = font_at(start + at);
            match runs.last_mut() {
                Some(run) if run.font == font => run.len += c.len_utf8(),
                _ => runs.push(TextRun {
                    len: c.len_utf8(),
                    font,
                    color: gpui::black(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }),
            }
        }
        let text = SharedString::from(part.to_owned());
        window
            .text_system()
            .shape_line(text, TEXT_BODY, &runs, None)
            .width
    };
    let text = &cell.text;
    let pieces = |split: fn(char) -> bool| {
        let mut start = 0;
        let mut widest = px(0.);
        for (at, c) in text.char_indices().chain([(text.len(), '\n')]) {
            if split(c) {
                if at > start {
                    widest = widest.max(width(start, &text[start..at]));
                }
                start = at + c.len_utf8();
            }
        }
        widest
    };
    let line = pieces(|c| c == '\n');
    let word = pieces(|c| c.is_whitespace() && c != '\u{A0}' && c != '\u{202F}');
    (line.ceil(), word.ceil())
}
