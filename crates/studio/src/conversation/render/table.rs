//! A table: its head in semibold over its rows, a hairline under each row,
//! no lines between columns. Each column is as wide as its widest cell, up
//! to `CELL_MAX_WIDTH`, and shares out the room left over; past that a
//! cell wraps. When the narrowest the columns can go is wider than the
//! column of text, the table scrolls sideways.

use gpui::{
    AnyElement, Div, ElementId, FontWeight, IntoElement, ParentElement, Pixels, SharedString,
    Styled, TextRun, Window, div, prelude::*, px,
};
use theme::{HAIRLINE, SPACE_2, SPACE_3, TEXT_SMALL};

use super::{Env, Tail, inline};
use crate::conversation::markdown::{Align, Inline, Table};

/// The widest a column grows to fit its text before its cells wrap.
const CELL_MAX_WIDTH: Pixels = px(320.);

pub fn block(table: &Table, id: u64, tail: &Tail, env: &mut Env) -> AnyElement {
    let colors = env.colors;
    let count = table.head.len().max(1);
    let columns: Vec<(Pixels, Pixels)> = (0..count)
        .map(|column| {
            let cells = (table.rows.iter())
                .filter_map(|row| row.get(column).map(|cell| (cell, false)))
                .chain(table.head.get(column).map(|cell| (cell, true)));
            let (mut widest, mut word) = (px(0.), px(0.));
            for (cell, bold) in cells {
                let (line, longest) = measure(env.window, &cell.text, bold);
                widest = widest.max(line);
                word = word.max(longest);
            }
            let width = widest.min(CELL_MAX_WIDTH) + padding(column, count);
            (width, (word + padding(column, count)).min(width))
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
            .border_b(HAIRLINE)
            .border_color(colors.border)
            .children(columns.iter().enumerate().map(|(column, &(width, least))| {
                let cell = cells.get(column);
                let tail = if column + 1 == cells.len() { tail } else { &[] };
                let id = ElementId::NamedInteger(
                    "cell".into(),
                    super::nested(super::nested(id, at), column),
                );
                let align = table.align.get(column).copied().unwrap_or_default();
                aligned(div(), align)
                    .flex_basis(width)
                    .flex_grow(f32::from(width))
                    .flex_shrink(1.)
                    .min_w(least)
                    .pl(if column == 0 { px(0.) } else { SPACE_3 })
                    .pr(if column + 1 == count { px(0.) } else { SPACE_3 })
                    .py(SPACE_2)
                    .children(cell.map(|cell| inline::text(cell, id, tail, env.colors)))
            }))
    };
    let head = row(&table.head, usize::MAX, env).font_weight(FontWeight::SEMIBOLD);
    let rows: Vec<Div> = (table.rows.iter().enumerate())
        .map(|(at, cells)| row(cells, at + 1, env))
        .collect();
    div()
        .id(ElementId::NamedInteger("table".into(), id))
        .w_full()
        .overflow_x_scroll()
        .restrict_scroll_to_axis()
        .text_size(TEXT_SMALL)
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(narrowest)
                .child(head)
                .children(rows),
        )
        .into_any_element()
}

/// A cell's room on each side: none on the table's outer edges, so its
/// text stands in line with the paragraphs around it.
fn padding(column: usize, count: usize) -> Pixels {
    let inner = |inner: bool| if inner { SPACE_3 } else { px(0.) };
    inner(column > 0) + inner(column + 1 < count)
}

fn aligned(cell: Div, align: Align) -> Div {
    match align {
        Align::Start => cell,
        Align::Center => cell.text_center(),
        Align::End => cell.text_right(),
    }
}

/// How wide `text` is on one line at the table's size, and its longest
/// word, which a cell can't wrap.
fn measure(window: &Window, text: &str, bold: bool) -> (Pixels, Pixels) {
    let mut font = window.text_style().font();
    if bold {
        font.weight = FontWeight::SEMIBOLD;
    }
    let width = |text: &str| {
        let run = TextRun {
            len: text.len(),
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window.text_system().shape_line(
            SharedString::from(text.to_owned()),
            TEXT_SMALL,
            &[run],
            None,
        );
        line.width
    };
    let line = text.lines().map(width).fold(px(0.), Pixels::max);
    let word = (text.split_whitespace())
        .map(width)
        .fold(px(0.), Pixels::max);
    (line.ceil(), word.ceil())
}
