//! The composer's toolbar: Attach, the session's choices as pills, and Send.
//! None of them acts until agents come; the pills show the values a session
//! will start with.

use gpui::{App, IntoElement, ParentElement, Styled, div, prelude::*, svg};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, Colors, DISABLED_OPACITY, ICON_SIZE, ICON_SIZE_SMALL, SPACE_1,
    SPACE_2, Theme,
};

use super::Send;
use crate::{
    assets::Icon,
    sidebar::icon_button,
    tooltip::{TooltipExt, WithTooltip},
};

/// A choice a session makes before it starts: its pill's element id and the
/// value it starts at.
struct Choice {
    id: &'static str,
    value: &'static str,
}

/// The agent that runs the session, how hard it thinks, and what it may do
/// without asking.
const CHOICES: [Choice; 3] = [
    Choice {
        id: "composer-agent",
        value: "Claude Code",
    },
    Choice {
        id: "composer-effort",
        value: "High",
    },
    Choice {
        id: "composer-access",
        value: "Ask first",
    },
];

pub(super) fn toolbar(has_message: bool, cx: &App) -> impl IntoElement {
    let colors = Theme::of(cx).colors;
    div()
        .h(CONTROL_SIZE)
        .flex()
        .items_center()
        .gap(SPACE_1)
        .cursor_default()
        .child(icon_button(
            "composer-attach",
            Icon::NewTab,
            "Attachments come with agents",
            false,
            colors,
            |button| button,
        ))
        .children(CHOICES.iter().map(|choice| pill(choice, colors)))
        .child(div().flex_1())
        .child(send_button(has_message, colors))
}

/// A choice's current value and a chevron, filled only under the pointer, as
/// an icon button is. Its tooltip says why it opens nothing yet.
fn pill(choice: &Choice, colors: Colors) -> WithTooltip {
    div()
        .id(choice.id)
        .flex_none()
        .h(CONTROL_SIZE)
        .px(SPACE_2)
        .flex()
        .items_center()
        .gap(SPACE_1)
        .rounded(CONTROL_RADIUS)
        .hover(|pill| pill.bg(colors.hover))
        .text_color(colors.text_secondary)
        .whitespace_nowrap()
        .child(choice.value)
        .child(
            svg()
                .path(Icon::ChevronDown.path())
                .flex_none()
                .size(ICON_SIZE_SMALL)
                .text_color(colors.text_secondary),
        )
        .with_tooltip("Choosing comes with agents")
}

/// A round button in `text_primary` with an arrow cut in the card's color.
/// Faded until there is a message, since whitespace alone isn't one.
fn send_button(has_message: bool, colors: Colors) -> WithTooltip {
    let label = match (has_message, cfg!(target_os = "macos")) {
        (false, _) => "Type a message to send",
        (true, true) => "Send (↩)",
        (true, false) => "Send (Enter)",
    };
    div()
        .id("composer-send")
        .flex_none()
        .size(CONTROL_SIZE)
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(colors.text_primary)
        .when(!has_message, |button| button.opacity(DISABLED_OPACITY))
        .when(has_message, |button| {
            button.on_click(|_, window, cx| window.dispatch_action(Box::new(Send), cx))
        })
        .child(
            svg()
                .path(Icon::Send.path())
                .size(ICON_SIZE)
                .text_color(colors.panel_background),
        )
        .with_tooltip(label)
}
