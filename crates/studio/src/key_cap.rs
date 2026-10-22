//! Key caps: a shortcut shown beside a button's label or in a hint.

use gpui::{Div, IntoElement, ParentElement, Styled, div, px, svg};
use theme::{Colors, ICON_SIZE, KEY_CAP_SIZE, SPACE_1};

use crate::assets::Icon;

/// What a key cap shows.
#[derive(Clone, Copy)]
pub enum Key {
    Text(&'static str),
    Return,
}

/// One key cap, holding a key or a chord such as ⌘↵: a filled rounded square, no
/// border, so it sits quietly inside a button or beside a hint.
pub fn key_cap(keys: &[Key], colors: Colors) -> Div {
    div()
        .flex_none()
        .h(KEY_CAP_SIZE)
        .min_w(KEY_CAP_SIZE)
        .px(px(2.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(SPACE_1)
        .bg(colors.key_background)
        .text_color(colors.text_secondary)
        .line_height(KEY_CAP_SIZE)
        .children(keys.iter().map(|key| {
            match key {
                // Text takes more room at its sides than the icon, which has its own.
                Key::Text(text) => div().px(px(2.)).child(*text).into_any_element(),
                Key::Return => svg()
                    .path(Icon::Return.path())
                    .size(ICON_SIZE)
                    .text_color(colors.text_secondary)
                    .into_any_element(),
            }
        }))
}
