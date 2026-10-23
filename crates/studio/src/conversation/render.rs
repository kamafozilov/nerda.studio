//! Markdown blocks as elements: text in the color and size around it, its
//! styled runs by weight, slant, a line through, or the monospaced font;
//! code on the field's fill, scrolling sideways when a line is long.

use gpui::{
    AnyElement, ElementId, FontStyle, FontWeight, HighlightStyle, IntoElement, ParentElement,
    SharedString, StrikethroughStyle, Styled, StyledText, div, prelude::*, px,
};
use theme::{CONTROL_RADIUS, Colors, HAIRLINE, MONOSPACE_FONT, SPACE_1, SPACE_2, SPACE_3};

use super::markdown::{Block, Inline};

/// The width a list's markers take, as wide as "10." at `TEXT_BODY`.
const MARKER_WIDTH: gpui::Pixels = px(20.);

/// `blocks` one under another; `id` names the code blocks' scroll state.
pub fn blocks(blocks: &[Block], id: u64, colors: Colors) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(SPACE_2)
        .children(
            (blocks.iter().enumerate())
                .map(|(at, block)| self::block(block, nested(id, at), colors)),
        )
        .into_any_element()
}

/// The id of the `at`-th block inside the one `id` names.
fn nested(id: u64, at: usize) -> u64 {
    id.wrapping_mul(64).wrapping_add(at as u64)
}

fn block(block: &Block, id: u64, colors: Colors) -> AnyElement {
    match block {
        Block::Paragraph(inline) => styled(inline).into_any_element(),
        Block::Heading(inline) => div()
            .font_weight(FontWeight::SEMIBOLD)
            .child(styled(inline))
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
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .child(SharedString::from(code.clone())),
            )
            .into_any_element(),
        Block::List { start, items } => div()
            .flex()
            .flex_col()
            .gap(SPACE_1)
            .children(items.iter().enumerate().map(|(at, item)| {
                let marker = match start {
                    Some(start) => format!("{}.", start + at as u64),
                    None => "•".to_owned(),
                };
                div()
                    .flex()
                    .child(div().flex_none().w(MARKER_WIDTH).child(marker))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(blocks(item, nested(id, at), colors)),
                    )
            }))
            .into_any_element(),
        Block::Quote(inner) => div()
            .pl(SPACE_3 - HAIRLINE * 2.)
            .border_l(HAIRLINE * 2.)
            .border_color(colors.border)
            .child(blocks(inner, id, colors))
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
    let highlights = (inline.spans.iter()).map(|(range, style)| {
        let highlight = HighlightStyle {
            font_weight: style.bold.then_some(FontWeight::BOLD),
            font_style: style.italic.then_some(FontStyle::Italic),
            strikethrough: style.strike.then_some(StrikethroughStyle {
                thickness: HAIRLINE,
                color: None,
            }),
            ..HighlightStyle::default()
        };
        (range.clone(), highlight)
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
