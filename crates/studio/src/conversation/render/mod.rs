//! Markdown blocks as elements, in the color and size around them:
//! paragraphs, headings by size and weight, code blocks ([`code`]), lists
//! with their markers or boxes, quotes behind a rule, and tables
//! ([`table`]). Text that just streamed in fades in at the end of the last
//! block.

mod code;
mod inline;
mod table;

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FontWeight, IntoElement, ParentElement, Pixels, SharedString,
    Styled, Window, div, prelude::*, px, svg,
};
use theme::{
    Colors, HAIRLINE, ICON_SIZE_ROW, SPACE_1, SPACE_2, SPACE_3, TEXT_BODY, TEXT_HEADING,
    TEXT_HEADING_2, TEXT_HEADING_3,
};

pub use inline::line;

use super::{markdown::Block, syntax};
use crate::assets::Icon;

/// The width a list's markers take, as wide as "10." at `TEXT_BODY`; a
/// list numbered past 99 widens it a digit at a time.
const MARKER_WIDTH: Pixels = px(20.);

/// Parts of the end of a text still fading in: each as how far from the
/// end it starts and stops, in bytes, and how far it has come in, 0 to 1.
pub type Tail = [(usize, usize, f32)];

/// Copies a code block's text, given its id and the text.
pub type Copy = Rc<dyn Fn(&(u64, SharedString), &mut Window, &mut App)>;

/// What drawing the blocks takes beyond the blocks themselves.
pub struct Env<'a> {
    pub colors: Colors,
    pub window: &'a Window,
    pub syntax: &'a mut syntax::Cache,
    /// The code block whose Copy just copied it.
    pub copied: Option<u64>,
    pub copy: Copy,
    /// How deep in lists the blocks being drawn are.
    depth: usize,
}

impl<'a> Env<'a> {
    pub fn new(
        colors: Colors,
        window: &'a Window,
        syntax: &'a mut syntax::Cache,
        copied: Option<u64>,
        copy: Copy,
    ) -> Self {
        Self {
            colors,
            window,
            syntax,
            copied,
            copy,
            depth: 0,
        }
    }
}

/// `blocks` one under another; `id` names their state, such as a code
/// block's scroll; `tail` fades in the end of the last one.
pub fn blocks(blocks: &[Block], id: u64, tail: &Tail, env: &mut Env) -> AnyElement {
    let last = blocks.len().saturating_sub(1);
    let children: Vec<AnyElement> = (blocks.iter().enumerate())
        .map(|(at, block)| {
            let tail = if at == last { tail } else { &[] };
            let element = self::block(block, nested(id, at), tail, env);
            // A heading stands apart from what came before it.
            match block {
                Block::Heading(..) if at > 0 => div().pt(SPACE_3).child(element).into_any_element(),
                _ => element,
            }
        })
        .collect();
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(SPACE_2)
        .children(children)
        .into_any_element()
}

/// The id of the `at`-th block inside the one `id` names.
fn nested(id: u64, at: usize) -> u64 {
    id.wrapping_mul(64).wrapping_add(at as u64)
}

fn block(block: &Block, id: u64, tail: &Tail, env: &mut Env) -> AnyElement {
    let colors = env.colors;
    let text_id = || ElementId::NamedInteger("text".into(), id);
    match block {
        Block::Paragraph(text) => inline::text(text, text_id(), tail, colors),
        Block::Heading(level, text) => div()
            .text_size(match level {
                1 => TEXT_HEADING,
                2 => TEXT_HEADING_2,
                3 => TEXT_HEADING_3,
                _ => TEXT_BODY,
            })
            .font_weight(FontWeight::SEMIBOLD)
            .child(inline::text(text, text_id(), tail, colors))
            .into_any_element(),
        Block::Code(code) => code::block(code, id, tail, env),
        Block::Table(table) => table::block(table, id, tail, env),
        Block::List { start, items } => {
            let digits = start.map_or(1, |start| (start + items.len() as u64).to_string().len());
            let marker_width = MARKER_WIDTH + px(7.) * digits.saturating_sub(2) as f32;
            env.depth += 1;
            let rows: Vec<AnyElement> = (items.iter().enumerate())
                .map(|(at, item)| {
                    let tail = if at + 1 == items.len() { tail } else { &[] };
                    let marker = match (item.task, start) {
                        (Some(done), _) => task_box(done, env.window.line_height(), colors),
                        (None, Some(start)) => div()
                            .text_color(colors.text_secondary)
                            .child(format!("{}.", start + at as u64))
                            .into_any_element(),
                        (None, None) => div()
                            .text_color(colors.text_secondary)
                            .child(bullet(env.depth))
                            .into_any_element(),
                    };
                    div()
                        .flex()
                        .child(div().flex_none().w(marker_width).child(marker))
                        .child(div().flex_1().min_w_0().child(blocks(
                            &item.blocks,
                            nested(id, at),
                            tail,
                            env,
                        )))
                        .into_any_element()
                })
                .collect();
            env.depth -= 1;
            div()
                .flex()
                .flex_col()
                .gap(SPACE_1)
                .children(rows)
                .into_any_element()
        }
        Block::Quote(inner) => div()
            .pl(SPACE_3 - HAIRLINE * 2.)
            .border_l(HAIRLINE * 2.)
            .border_color(colors.border)
            .child(blocks(inner, id, tail, env))
            .into_any_element(),
        Block::Rule => div()
            .my(SPACE_2)
            .h(HAIRLINE)
            .bg(colors.border)
            .into_any_element(),
    }
}

/// A bullet list's marker by how deep the list is: a disc, a ring, then a
/// square.
fn bullet(depth: usize) -> &'static str {
    match depth {
        0 | 1 => "•",
        2 => "◦",
        _ => "▪",
    }
}

/// A task list's box: an outline, with a check when the task is done.
fn task_box(done: bool, line_height: Pixels, colors: Colors) -> AnyElement {
    // Centered on the first line's middle, as a bullet is.
    div()
        .h(line_height)
        .flex()
        .items_center()
        .child(
            div()
                .size(ICON_SIZE_ROW)
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border(HAIRLINE)
                .border_color(colors.text_secondary)
                .when(done, |checkbox| {
                    checkbox.child(
                        svg()
                            .path(Icon::Check.path())
                            .size(ICON_SIZE_ROW - px(2.))
                            .text_color(colors.text_primary),
                    )
                }),
        )
        .into_any_element()
}
