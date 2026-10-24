//! Markdown blocks as elements: text in the color and size around it, its
//! styled runs by weight, slant, a line through, or the monospaced font;
//! code on the field's fill, scrolling sideways when a line is long. Text
//! that just streamed in can fade in at the end of the last block.

use gpui::{
    AnyElement, ElementId, FontStyle, FontWeight, HighlightStyle, IntoElement, ParentElement,
    SharedString, StrikethroughStyle, Styled, StyledText, div, prelude::*, px,
};
use theme::{CONTROL_RADIUS, Colors, HAIRLINE, MONOSPACE_FONT, SPACE_1, SPACE_2, SPACE_3};

use std::ops::Range;

use super::markdown::{Block, Inline};

/// The width a list's markers take, as wide as "10." at `TEXT_BODY`.
const MARKER_WIDTH: gpui::Pixels = px(20.);

/// Parts of the end of a text still fading in: each as how far from the
/// end it starts and stops, in bytes, and how far it has come in, 0 to 1.
pub type Tail = [(usize, usize, f32)];

/// `blocks` one under another; `id` names the code blocks' scroll state;
/// `tail` fades in the end of the last one.
pub fn blocks(blocks: &[Block], id: u64, tail: &Tail, colors: Colors) -> AnyElement {
    let last = blocks.len().saturating_sub(1);
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(SPACE_2)
        .children((blocks.iter().enumerate()).map(|(at, block)| {
            let tail = if at == last { tail } else { &[] };
            self::block(block, nested(id, at), tail, colors)
        }))
        .into_any_element()
}

/// The id of the `at`-th block inside the one `id` names.
fn nested(id: u64, at: usize) -> u64 {
    id.wrapping_mul(64).wrapping_add(at as u64)
}

fn block(block: &Block, id: u64, tail: &Tail, colors: Colors) -> AnyElement {
    match block {
        Block::Paragraph(inline) => faded(inline, tail).into_any_element(),
        Block::Heading(inline) => div()
            .font_weight(FontWeight::SEMIBOLD)
            .child(faded(inline, tail))
            .into_any_element(),
        Block::Code(code) => div()
            .id(ElementId::NamedInteger("code".into(), id))
            .w_full()
            // A row, so the line keeps its own width and the block scrolls.
            .flex()
            .overflow_x_scroll()
            // Only a sideways scroll is the block's; the conversation keeps
            // the rest.
            .restrict_scroll_to_axis()
            .px(SPACE_3)
            .py(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .bg(colors.field_background)
            .font_family(MONOSPACE_FONT)
            .child(div().flex_none().whitespace_nowrap().child(faded(
                &Inline {
                    text: code.clone(),
                    spans: Vec::new(),
                },
                tail,
            )))
            .into_any_element(),
        Block::List { start, items } => div()
            .flex()
            .flex_col()
            .gap(SPACE_1)
            .children(items.iter().enumerate().map(|(at, item)| {
                let tail = if at + 1 == items.len() { tail } else { &[] };
                let marker = match start {
                    Some(start) => format!("{}.", start + at as u64),
                    None => "•".to_owned(),
                };
                div()
                    .flex()
                    .child(div().flex_none().w(MARKER_WIDTH).child(marker))
                    .child(div().flex_1().min_w_0().child(blocks(
                        item,
                        nested(id, at),
                        tail,
                        colors,
                    )))
            }))
            .into_any_element(),
        Block::Quote(inner) => div()
            .pl(SPACE_3 - HAIRLINE * 2.)
            .border_l(HAIRLINE * 2.)
            .border_color(colors.border)
            .child(blocks(inner, id, tail, colors))
            .into_any_element(),
        Block::Rule => div()
            .my(SPACE_2)
            .h(HAIRLINE)
            .bg(colors.border)
            .into_any_element(),
    }
}

/// The text with its runs styled, in the color and size around it.
pub fn styled(inline: &Inline) -> StyledText {
    faded(inline, &[])
}

/// The text with its runs styled, and the parts of its end in `tail` faded
/// as far as they have yet to come in. The runs are cut where a fade starts
/// or stops, so each part keeps its style.
fn faded(inline: &Inline, tail: &Tail) -> StyledText {
    let length = inline.text.len();
    let boundary = |from_end: usize| {
        let mut at = length.saturating_sub(from_end);
        while !inline.text.is_char_boundary(at) {
            at += 1;
        }
        at
    };
    let fades: Vec<(Range<usize>, f32)> = (tail.iter())
        .map(|&(far, near, amount)| (boundary(far)..boundary(near), amount))
        .filter(|(range, amount)| !range.is_empty() && *amount < 1.)
        .collect();
    let mut cuts: Vec<usize> = vec![0, length];
    let ranges =
        (inline.spans.iter().map(|(range, _)| range)).chain(fades.iter().map(|(range, _)| range));
    for range in ranges {
        cuts.extend([range.start, range.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let highlights = cuts.windows(2).filter_map(|cut| {
        let (start, end) = (cut[0], cut[1]);
        let style = (inline.spans.iter())
            .find(|(range, _)| range.start <= start && end <= range.end)
            .map(|(_, style)| *style);
        let shown: f32 = (fades.iter())
            .filter(|(range, _)| range.start <= start && end <= range.end)
            .map(|(_, amount)| *amount)
            .product();
        if style.is_none() && shown >= 1. {
            return None;
        }
        let style = style.unwrap_or_default();
        let highlight = HighlightStyle {
            font_weight: style.bold.then_some(FontWeight::BOLD),
            font_style: style.italic.then_some(FontStyle::Italic),
            strikethrough: style.strike.then_some(StrikethroughStyle {
                thickness: HAIRLINE,
                color: None,
            }),
            fade_out: (shown < 1.).then_some(1. - shown),
            ..HighlightStyle::default()
        };
        Some((start..end, highlight))
    });
    let monospaced = (inline.spans.iter())
        .filter(|(_, style)| style.code)
        .map(|(range, _)| (range.clone(), SharedString::new_static(MONOSPACE_FONT)));
    StyledText::new(inline.text.clone())
        .with_highlights(highlights.collect::<Vec<_>>())
        .with_font_family_overrides(monospaced.collect::<Vec<_>>())
}

/// A line of inline Markdown, such as a problem's words with a command in
/// backticks.
pub fn line(text: &str) -> StyledText {
    styled(&super::markdown::inline(text))
}
