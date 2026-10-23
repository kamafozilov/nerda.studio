//! Each kind of row: the person's message, thinking, a tool call or a
//! prompt, and how a turn ended. Tool calls and prompts are one line each
//! for now.

use std::{
    f32::consts::FRAC_PI_2,
    time::{Duration, Instant},
};

use agent::{
    Ask, Attachment, Ending, ImageFormat, Message, Permission, PermissionState, Text, ToolCall,
    ToolStatus,
};
use gpui::{
    AnyElement, ClickEvent, ElementId, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, radians, relative, svg,
};
use theme::{
    COMPOSER_RADIUS, Colors, ICON_SIZE_SMALL, MESSAGE_MAX_WIDTH, SPACE_1, SPACE_2, SPACE_3,
    TEXT_SMALL,
};

use super::render;
use crate::{
    assets::Icon,
    composer::{chip, chip_label},
    tooltip::TooltipExt,
};

/// The person's message, at the column's trailing edge on the field's fill:
/// its attachments as chips over its text, and "Queued" under it while it
/// waits for the turn before.
pub fn message(message: &Message, queued: bool, colors: Colors) -> AnyElement {
    let chips = (message.attachments.iter().enumerate()).map(|(at, attachment)| {
        let id = ElementId::NamedInteger("sent-attachment".into(), at as u64);
        match attachment {
            // Sent by its bytes, an image keeps no name.
            Attachment::Image(image) => {
                let name = match image.format() {
                    ImageFormat::Png => "PNG image",
                    ImageFormat::Jpeg => "JPEG image",
                    ImageFormat::Gif => "GIF image",
                    ImageFormat::Webp => "WebP image",
                };
                chip(colors)
                    .pr(SPACE_2)
                    .child(chip_label(id, Icon::Image, name.into(), colors))
                    .into_any_element()
            }
            Attachment::File(path) => {
                let icon = if path.is_dir() {
                    Icon::Folder
                } else {
                    Icon::File
                };
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                chip(colors)
                    .pr(SPACE_2)
                    .child(
                        chip_label(id, icon, name.into(), colors)
                            .with_tooltip(workspace::home_relative(path)),
                    )
                    .into_any_element()
            }
        }
    });
    let chips: Vec<AnyElement> = chips.collect();
    let text = message.text.trim_end();
    let bubble = div()
        .max_w(relative(MESSAGE_MAX_WIDTH))
        .px(SPACE_3)
        .py(SPACE_2)
        .flex()
        .flex_col()
        .gap(SPACE_2)
        .rounded(COMPOSER_RADIUS)
        .bg(colors.field_background)
        .when(!chips.is_empty(), |bubble| {
            bubble.child(div().flex().flex_wrap().gap(SPACE_1).children(chips))
        })
        .when(!text.is_empty(), |bubble| {
            bubble.child(SharedString::from(text.to_owned()))
        });
    div()
        .w_full()
        .flex()
        .flex_col()
        .items_end()
        .child(bubble)
        .when(queued, |row| {
            row.child(supporting(colors).mt(SPACE_1).child("Queued"))
        })
        .into_any_element()
}

/// "Thinking", with how long while it streams, over the summary once open.
pub fn thinking(
    text: &Text,
    since: Option<Instant>,
    open: bool,
    id: ElementId,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
    colors: Colors,
) -> AnyElement {
    let label = match (text.streaming, since) {
        (true, Some(since)) => format!("Thinking\u{2026} {}", elapsed(since.elapsed())),
        (true, None) => "Thinking\u{2026}".to_owned(),
        (false, _) => "Thinking".to_owned(),
    };
    let chevron = svg()
        .path(Icon::ChevronDown.path())
        .flex_none()
        .size(ICON_SIZE_SMALL)
        .text_color(colors.text_secondary)
        .when(!open, |icon| {
            icon.with_transformation(gpui::Transformation::rotate(radians(-FRAC_PI_2)))
        });
    div()
        .flex()
        .flex_col()
        .gap(SPACE_1)
        .text_color(colors.text_secondary)
        .child(
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(SPACE_1)
                .cursor_pointer()
                .on_click(on_click)
                .child(chevron)
                .child(label),
        )
        .when(open && !text.text.trim().is_empty(), |row| {
            row.child(
                div()
                    .pl(ICON_SIZE_SMALL + SPACE_1)
                    .child(SharedString::from(text.text.trim().to_owned())),
            )
        })
        .into_any_element()
}

/// The tool's name and where its call stands.
pub fn tool(call: &ToolCall, colors: Colors) -> AnyElement {
    let state = match call.status {
        ToolStatus::Running => "running",
        ToolStatus::Done(_) => "done",
        ToolStatus::Failed(_) => "failed",
    };
    supporting(colors)
        .child(format!("{} \u{b7} {state}", call.name))
        .into_any_element()
}

/// A prompt: while it waits, that the agent waits for an answer.
pub fn permission(permission: &Permission, colors: Colors) -> AnyElement {
    let tool = &permission.request.tool_name;
    let line = match (&permission.request.ask, permission.state) {
        (Ask::Questions(_), PermissionState::Pending) => {
            "The agent asked a question and is waiting for an answer.".to_owned()
        }
        (Ask::Plan(_), PermissionState::Pending) => {
            "The agent proposed a plan and is waiting for an answer.".to_owned()
        }
        (Ask::Tool, PermissionState::Pending) => {
            format!("The agent is waiting for an answer to run {tool}.")
        }
        (_, PermissionState::Allowed) => format!("{tool} \u{b7} allowed"),
        (_, PermissionState::Denied) => format!("{tool} \u{b7} denied"),
        (_, PermissionState::Cancelled) => format!("{tool} \u{b7} not answered"),
    };
    supporting(colors).child(line).into_any_element()
}

/// How the turn ended: how long it worked, that it was stopped, or what went
/// wrong and what to do, as the composer's problem line says it.
pub fn end(ending: &Ending, colors: Colors) -> AnyElement {
    match ending {
        Ending::Finished(finished) => supporting(colors)
            .child(format!(
                "Worked for {}",
                elapsed(Duration::from_millis(finished.duration_ms))
            ))
            .into_any_element(),
        Ending::Interrupted => supporting(colors).child("Stopped").into_any_element(),
        Ending::Failed(failure) => div()
            .flex()
            .flex_col()
            .text_size(TEXT_SMALL)
            .text_color(colors.text_primary)
            .child(render::line(&failure.message()))
            .children(failure.suggestion().map(render::line))
            .into_any_element(),
    }
}

/// A supporting line: `TEXT_SMALL` in `text_secondary`.
fn supporting(colors: Colors) -> gpui::Div {
    div()
        .text_size(TEXT_SMALL)
        .text_color(colors.text_secondary)
}

/// "12s", "1m 12s", "2h 5m": whole seconds, at least one.
pub fn elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs_f64().round().max(1.) as u64;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_in_whole_units() {
        let seconds = |s: f64| elapsed(Duration::from_secs_f64(s));
        assert_eq!(seconds(0.2), "1s");
        assert_eq!(seconds(12.4), "12s");
        assert_eq!(seconds(72.0), "1m 12s");
        assert_eq!(seconds(7500.0), "2h 5m");
    }
}
