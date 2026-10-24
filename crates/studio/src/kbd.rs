//! Key caps: a shortcut shown beside a button's label, in a tooltip, a menu
//! item or a hint, written the platform's way (⌘W on macOS, Ctrl+W elsewhere).

use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, px, svg,
};
use theme::{
    ICON_SIZE, ICON_SIZE_SMALL, KEY_CAP_SIZE, KEY_CAP_SIZE_SMALL, SPACE_1, TEXT_BODY, TEXT_SMALL,
    Theme,
};

use crate::assets::Icon;

const MAC: bool = cfg!(target_os = "macos");

/// The key a shortcut ends on.
#[derive(Clone, Copy)]
pub enum Key {
    Text(&'static str),
    Return,
    Escape,
}

/// A key and the modifiers held with it. `cmd` is Cmd on macOS and Ctrl
/// elsewhere, as the window's bindings are.
#[derive(Clone, Copy)]
pub struct Shortcut {
    key: Key,
    cmd: bool,
    alt: bool,
    shift: bool,
}

impl Shortcut {
    /// A key with no modifiers, written as `text`.
    pub const fn key(text: &'static str) -> Self {
        Self::of(Key::Text(text))
    }

    pub const fn of(key: Key) -> Self {
        Self {
            key,
            cmd: false,
            alt: false,
            shift: false,
        }
    }

    pub const fn cmd(mut self) -> Self {
        self.cmd = true;
        self
    }

    pub const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// The modifiers in the platform's order: ⌥⇧⌘ on macOS, run together;
    /// elsewhere Ctrl+Alt+Shift+, each joined to the key with a plus.
    fn modifiers(&self) -> String {
        let held = if MAC {
            [(self.alt, "⌥"), (self.shift, "⇧"), (self.cmd, "⌘")]
        } else {
            [
                (self.cmd, "Ctrl+"),
                (self.alt, "Alt+"),
                (self.shift, "Shift+"),
            ]
        };
        held.into_iter()
            .filter(|(on, _)| *on)
            .map(|(_, text)| text)
            .collect()
    }
}

/// One key cap holding a whole shortcut, such as ⌘W or ⌘↵.
#[derive(IntoElement)]
pub struct Kbd {
    shortcut: Shortcut,
    small: bool,
    color: Option<Hsla>,
}

impl Kbd {
    /// A cap `KEY_CAP_SIZE` high on `key_background`, with no hairline, so it
    /// sits quietly inside a button or beside a hint.
    pub fn new(shortcut: Shortcut) -> Self {
        Self {
            shortcut,
            small: false,
            color: None,
        }
    }

    /// `KEY_CAP_SIZE_SMALL` high in `TEXT_SMALL`, for tooltips and menu items.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// The key's color, `text_secondary` unless set.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Kbd {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let color = self.color.unwrap_or(colors.text_secondary);
        let (height, text_size, icon_size) = if self.small {
            (KEY_CAP_SIZE_SMALL, TEXT_SMALL, ICON_SIZE_SMALL)
        } else {
            (KEY_CAP_SIZE, TEXT_BODY, ICON_SIZE)
        };
        let mut text = self.shortcut.modifiers();
        let icon = match self.shortcut.key {
            Key::Text(key) => {
                text.push_str(key);
                None
            }
            Key::Escape => {
                text.push_str(if MAC { "esc" } else { "Esc" });
                None
            }
            Key::Return => Some(Icon::Return),
        };
        let parts: [Option<AnyElement>; 2] = [
            // Text takes more room at its sides than the icon, which has its own.
            (!text.is_empty()).then(|| {
                div()
                    .px(px(2.))
                    .child(SharedString::from(text))
                    .into_any_element()
            }),
            icon.map(|icon| {
                svg()
                    .path(icon.path())
                    .size(icon_size)
                    .text_color(color)
                    .into_any_element()
            }),
        ];
        div()
            .flex_none()
            .h(height)
            .min_w(height)
            .px(px(2.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(SPACE_1)
            .whitespace_nowrap()
            .text_size(text_size)
            .line_height(height)
            .text_color(color)
            .bg(colors.key_background)
            .children(parts.into_iter().flatten())
    }
}
