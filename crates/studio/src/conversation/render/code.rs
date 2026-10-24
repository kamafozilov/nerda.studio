//! A code block: on the field's fill, a header with its language and Copy,
//! over its text in the monospaced font, colored by its grammar, scrolling
//! sideways when a line is longer than the column.

use gpui::{
    AnyElement, ElementId, HighlightStyle, Hsla, IntoElement, ParentElement, SharedString, Styled,
    div, prelude::*, svg,
};
use theme::{
    CONTROL_RADIUS, Colors, FIELD_TARGET_SIZE, ICON_SIZE_ROW, MENU_RADIUS, MONOSPACE_FONT, SPACE_1,
    SPACE_2, SPACE_3, TEXT_CAPTION, TEXT_SMALL,
};

use super::{Env, Tail, inline::with_fades};
use crate::{
    assets::Icon,
    conversation::{markdown::Code, syntax::Role},
    tooltip::TooltipExt,
};

pub fn block(code: &Code, id: u64, tail: &Tail, env: &mut Env) -> AnyElement {
    let colors = env.colors;
    let runs = env.syntax.runs(id, code.language.as_deref(), &code.text);
    let styles = runs.map_or_else(Vec::new, |runs| colored(&runs, colors));
    let text = with_fades(&code.text, styles, tail);
    let copied = env.copied == Some(id);
    let copy = env.copy.clone();
    let source = SharedString::from(code.text.clone());
    let button = div()
        .id(ElementId::NamedInteger("copy-code".into(), id))
        .group("copy-code")
        .size(FIELD_TARGET_SIZE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(CONTROL_RADIUS)
        .hover(|button| button.bg(colors.hover))
        .on_click(move |_, window, cx| copy(&(id, source.clone()), window, cx))
        .child(
            svg()
                .path(if copied { Icon::Check } else { Icon::Copy }.path())
                .size(ICON_SIZE_ROW)
                .text_color(colors.text_secondary)
                .group_hover("copy-code", |icon| icon.text_color(colors.text_primary)),
        )
        .with_tooltip(if copied { "Copied" } else { "Copy" });
    let language = code.language.clone().unwrap_or_default();
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(MENU_RADIUS)
        .bg(colors.field_background)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pl(SPACE_3)
                .pr(SPACE_1)
                .pt(SPACE_1)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(MONOSPACE_FONT)
                        .text_size(TEXT_CAPTION)
                        .text_color(colors.text_secondary)
                        .child(language),
                )
                .child(button),
        )
        .child(
            div()
                .id(ElementId::NamedInteger("code".into(), id))
                .w_full()
                // A row, so the line keeps its own width and the block scrolls.
                .flex()
                .overflow_x_scroll()
                // Only a sideways scroll is the block's; the conversation keeps
                // the rest.
                .restrict_scroll_to_axis()
                .px(SPACE_3)
                .pb(SPACE_2 + SPACE_1)
                .font_family(MONOSPACE_FONT)
                .text_size(TEXT_SMALL)
                .child(div().flex_none().whitespace_nowrap().child(text)),
        )
        .into_any_element()
}

/// The code's tokens in their roles' colors, comments also in italics.
fn colored(
    runs: &[(std::ops::Range<usize>, Role)],
    colors: Colors,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    (runs.iter())
        .map(|(range, role)| (range.clone(), style(*role, colors)))
        .collect()
}

fn style(role: Role, colors: Colors) -> HighlightStyle {
    let syntax = colors.syntax;
    let color: Hsla = match role {
        Role::Keyword => syntax.keyword,
        Role::String => syntax.string,
        Role::Constant => syntax.constant,
        Role::Function => syntax.function,
        Role::Type => syntax.type_name,
        Role::Comment => colors.text_secondary,
        Role::Added => colors.diff_added,
        Role::Removed => colors.danger,
    };
    HighlightStyle {
        color: Some(color),
        font_style: (role == Role::Comment).then_some(gpui::FontStyle::Italic),
        ..HighlightStyle::default()
    }
}
