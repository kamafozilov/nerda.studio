//! Markdown blocks as elements, in the color and size around them:
//! paragraphs, headings by size and weight, code blocks ([`code`]), lists
//! with their markers or boxes, quotes behind a rule, and tables
//! ([`table`]). Text that just streamed in fades in at the end of the last
//! block.

mod chip_paint;
mod chips;
mod code;
mod inline;
mod select;
mod table;

use std::{collections::HashSet, path::PathBuf, rc::Rc};

use gpui::{
    AnyElement, App, ElementId, FocusHandle, FontWeight, IntoElement, ParentElement, Pixels,
    SharedString, Styled, Window, div, prelude::*, px, relative, svg,
};
use theme::{
    CONTROL_RADIUS, Colors, FIELD_TARGET_SIZE, HAIRLINE, HEADING_LINE_HEIGHT, ICON_SIZE_ROW,
    READING_LINE_HEIGHT, SPACE_1, SPACE_2, SPACE_3, TEXT_READING, TEXT_READING_HEADINGS,
};

pub use inline::line;
pub use select::OpenMenu;

use self::select::Select;
use super::{
    markdown::{Block, Inline},
    selection, syntax,
};
use crate::{
    assets::Icon,
    tooltip::{TooltipExt, WithTooltip},
};

/// The width a list's markers take, as wide as "10." at `TEXT_READING`; a
/// list numbered past 99 widens it a digit at a time.
const MARKER_WIDTH: Pixels = px(22.);

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
    /// The project's folder, which paths the text names read relative to.
    pub folder: Option<PathBuf>,
    /// The names of files named at more than one path in the text.
    pub crowded: HashSet<String>,
    /// What makes the reply's text selectable, with the number its next
    /// piece takes and what goes before it when copied.
    pub selecting: Option<Selecting>,
    /// How deep in lists the blocks being drawn are.
    depth: usize,
}

/// The selection a reply's pieces share, and how they number.
pub struct Selecting {
    pub state: selection::Shared,
    pub item: u64,
    pub focus: FocusHandle,
    pub menu: OpenMenu,
    pub next: usize,
    pub before: &'static str,
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
            folder: None,
            crowded: HashSet::new(),
            selecting: None,
            depth: 0,
        }
    }

    /// Makes the text a piece of the reply's selection, `piece` as copied;
    /// what lets the pointer select it, if the reply is selectable.
    fn piece(
        &mut self,
        piece: selection::Piece,
        targets: Rc<[(std::ops::Range<usize>, selection::Target)]>,
    ) -> Option<Select> {
        let selecting = self.selecting.as_mut()?;
        let number = selecting.next;
        selecting.next += 1;
        let piece = selection::Piece {
            before: std::mem::replace(&mut selecting.before, "\n"),
            ..piece
        };
        selecting
            .state
            .borrow_mut()
            .add(selecting.item, number, piece);
        Some(Select {
            state: selecting.state.clone(),
            item: selecting.item,
            number,
            color: self.colors.text_selection,
            focus: selecting.focus.clone(),
            menu: selecting.menu.clone(),
            targets,
        })
    }

    /// For the text whose blocks are `blocks`, in `folder`.
    pub fn text(mut self, folder: Option<PathBuf>, blocks: &[Block]) -> Self {
        self.folder = folder;
        self.crowded = chips::crowded(blocks);
        self
    }
}

/// `blocks` one under another, `SPACE_3` apart, `SPACE_2` inside a list; `id` names their state, such as a code
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
    // Inside a list an item's blocks stay close, so its items read apart.
    let gap = if env.depth > 0 { SPACE_2 } else { SPACE_3 };
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(gap)
        .children(children)
        .into_any_element()
}

/// `text` as written, not read as Markdown, its lines kept: the person's
/// message. `id` names its state.
pub fn plain(text: &str, id: u64, env: &mut Env) -> AnyElement {
    let inline = Inline {
        text: text.to_owned(),
        ..Inline::default()
    };
    inline::text(
        &inline,
        ElementId::NamedInteger("text".into(), id),
        &[],
        env,
    )
}

/// The id of the `at`-th block inside the one `id` names.
fn nested(id: u64, at: usize) -> u64 {
    id.wrapping_mul(64).wrapping_add(at as u64)
}

fn block(block: &Block, id: u64, tail: &Tail, env: &mut Env) -> AnyElement {
    let colors = env.colors;
    let text_id = || ElementId::NamedInteger("text".into(), id);
    match block {
        Block::Paragraph(text) => inline::text(text, text_id(), tail, env),
        Block::Heading(level, text) => div()
            .text_size(
                (TEXT_READING_HEADINGS.get(usize::from(*level).wrapping_sub(1)))
                    .copied()
                    .unwrap_or(TEXT_READING),
            )
            .line_height(relative(HEADING_LINE_HEIGHT))
            .font_weight(FontWeight::SEMIBOLD)
            .child(inline::text(text, text_id(), tail, env))
            .into_any_element(),
        Block::Code(code) => code::block(code, id, tail, env),
        Block::Table(table) => table::block(table, id, tail, env),
        Block::List { start, items } => {
            let digits = start.map_or(1, |start| (start + items.len() as u64).to_string().len());
            let marker_width = MARKER_WIDTH + px(8.) * digits.saturating_sub(2) as f32;
            env.depth += 1;
            let rows: Vec<AnyElement> = (items.iter().enumerate())
                .map(|(at, item)| {
                    let tail = if at + 1 == items.len() { tail } else { &[] };
                    let marker = match (item.task, start) {
                        (Some(done), _) => {
                            task_box(done, TEXT_READING * READING_LINE_HEIGHT, colors)
                        }
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

/// A Copy button: its icon turns a check, its tooltip "Copied", while
/// `copied`; `label` names it otherwise.
pub fn copy_button(
    id: ElementId,
    copied: bool,
    label: &'static str,
    colors: Colors,
    copy: impl Fn(&mut Window, &mut App) + 'static,
) -> WithTooltip {
    div()
        .id(id)
        .group("copy")
        .size(FIELD_TARGET_SIZE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(CONTROL_RADIUS)
        .hover(|button| button.bg(colors.hover))
        .on_click(move |_, window, cx| copy(window, cx))
        .child(
            svg()
                .path(if copied { Icon::Check } else { Icon::Copy }.path())
                .size(ICON_SIZE_ROW)
                .text_color(colors.text_secondary)
                .group_hover("copy", |icon| icon.text_color(colors.text_primary)),
        )
        .with_tooltip(if copied { "Copied" } else { label })
}
