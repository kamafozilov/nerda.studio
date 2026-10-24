//! A tool call's row: its icon, verb and target on one line, quieter detail
//! after them, a change's counts of lines, and, opened, what it did; the
//! line over a group of calls; and the rail that holds a group's rows.

use std::{f32::consts::FRAC_PI_2, path::Path};

use agent::{ToolCall, ToolStatus};
use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, FontWeight, Hsla, IntoElement, ParentElement,
    SharedString, StrikethroughStyle, Styled, StyledText, Window, div, prelude::*, px, radians,
    svg,
};
use theme::{
    CONTROL_RADIUS, Colors, HAIRLINE, ICON_SIZE_ROW, ICON_SIZE_SMALL, MONOSPACE_FONT, SPACE_1,
    SPACE_2, SPACE_3, TEXT_CAPTION, TEXT_SMALL,
};

use super::{
    animation,
    diff::{self, DiffLine},
    tools::{self, Body, Target, TodoState, Tool},
};
use crate::{assets::Icon, tooltip::TooltipExt};

/// Lines of a command's or a tool's output an opened row shows; the rest is
/// counted.
const OUTPUT_LINES: usize = 16;

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The call's row, opened or not; `on_click` opens and closes it when it
/// has something to show.
pub fn tool(
    call: &ToolCall,
    folder: Option<&Path>,
    open: bool,
    id: ElementId,
    on_click: OnClick,
    still: bool,
    colors: Colors,
) -> AnyElement {
    let tool = tools::describe(call, folder);
    let body = tools::body(call);
    let running = call.status == ToolStatus::Running;
    let failed = tools::failed(call);
    let openable = body != Body::Nothing && !matches!(body, Body::Todos(_));
    let (icon, icon_color) = match failed {
        true => (Icon::Alert, colors.danger),
        false => (tool.icon, colors.text_secondary),
    };
    let group = SharedString::from(format!("tool-{id:?}"));
    let line = div()
        .id(id)
        .group(group.clone())
        .w_full()
        .flex()
        .items_center()
        .gap(SPACE_2)
        .when(openable, |line| line.cursor_pointer().on_click(on_click))
        .child(icon_slot(icon, icon_color).when(running, |slot| {
            slot.opacity(animation::breathe(0.45, still))
        }))
        .child(words(&tool, running, still, colors))
        .when_some(tool.stats, |line, stats| {
            line.child(stats_label(stats, colors))
        })
        .when(failed, |line| line.child(failed_label(None, colors)))
        .when(openable, |line| {
            line.child(disclosure(open, &group, colors))
        });
    let body = match body {
        Body::Todos(todos) if !todos.is_empty() => Some(todo_list(&todos, colors)),
        _ if !open => None,
        Body::Command { command, output } => Some(command_block(&command, &output, failed, colors)),
        Body::Diff(lines) => Some(diff_block(&lines, colors)),
        Body::Output(output) => Some(output_block(&output, failed, colors)),
        Body::Todos(_) | Body::Nothing => None,
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(SPACE_1)
        .child(line)
        .when_some(body, |row, body| {
            row.child(div().pl(ICON_SIZE_ROW + SPACE_2).child(body))
        })
        .into_any_element()
}

/// The verb, the target and the detail, truncating the detail first.
fn words(tool: &Tool, running: bool, still: bool, colors: Colors) -> Div {
    let verb = match running {
        true => animation::shimmer(
            tool.live_verb,
            colors.text_secondary,
            colors.text_primary,
            still,
        ),
        false => animation::shimmer(
            tool.verb,
            colors.text_secondary,
            colors.text_secondary,
            true,
        ),
    };
    let target = tool.target.as_ref().map(|target| {
        let element = div().min_w_0().truncate();
        match target {
            Target::File { name, path } => element
                .id(SharedString::from(format!("file-{path}")))
                .text_color(colors.text_primary)
                .child(name.clone())
                .with_tooltip(path.clone())
                .into_any_element(),
            Target::Code(code) => element
                .font_family(MONOSPACE_FONT)
                .text_size(TEXT_CAPTION)
                .text_color(colors.text_primary)
                .child(code.clone())
                .into_any_element(),
            Target::Words(words) => element
                .text_color(colors.text_primary)
                .child(words.clone())
                .into_any_element(),
        }
    });
    div()
        .min_w_0()
        .flex()
        .items_baseline()
        .gap(SPACE_1)
        .child(div().flex_none().child(verb))
        .when_some(target, |words, target| {
            words.child(div().min_w_0().flex().child(target))
        })
        .when_some(tool.detail.clone(), |words, detail| {
            words.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(colors.text_tertiary)
                    .child(detail),
            )
        })
}

/// "failed", or "2 failed" over a group, in `danger`.
fn failed_label(count: Option<usize>, colors: Colors) -> Div {
    let words = match count {
        Some(count) => format!("{count} failed"),
        None => "failed".to_owned(),
    };
    div().flex_none().text_color(colors.danger).child(words)
}

/// "+12 −3", the counts in their colors.
fn stats_label((added, removed): (usize, usize), colors: Colors) -> Div {
    div()
        .flex_none()
        .flex()
        .gap(SPACE_1)
        .text_size(TEXT_SMALL)
        .font_family(MONOSPACE_FONT)
        .when(added > 0, |stats| {
            stats.child(
                div()
                    .text_color(colors.diff_added)
                    .child(format!("+{added}")),
            )
        })
        .when(removed > 0, |stats| {
            stats.child(
                div()
                    .text_color(colors.danger)
                    .child(format!("\u{2212}{removed}")),
            )
        })
}

/// The icon at the line's start, in a slot of its own width.
pub fn icon_slot(icon: Icon, color: Hsla) -> Div {
    div()
        .flex_none()
        .size(ICON_SIZE_ROW)
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(icon.path())
                .size(ICON_SIZE_ROW)
                .text_color(color),
        )
}

/// The chevron that shows, under the pointer, that the row opens; turned
/// down while it is open.
fn disclosure(open: bool, group: &SharedString, colors: Colors) -> Div {
    div()
        .flex_none()
        .opacity(0.)
        .group_hover(group.clone(), |chevron| chevron.opacity(1.))
        .child(chevron(open, colors.text_tertiary))
}

/// A chevron pointing down when `open`, else right.
pub fn chevron(open: bool, color: Hsla) -> impl IntoElement {
    svg()
        .path(Icon::ChevronDown.path())
        .flex_none()
        .size(ICON_SIZE_SMALL)
        .text_color(color)
        .when(!open, |icon| {
            icon.with_transformation(gpui::Transformation::rotate(radians(-FRAC_PI_2)))
        })
}

/// The line over a group: a chevron where its rows' icons stand, what the
/// group did, glowing while it goes on, and what it changed in lines.
#[allow(clippy::too_many_arguments)]
pub fn group(
    summary: String,
    stats: (usize, usize),
    failed: usize,
    open: bool,
    live: bool,
    id: ElementId,
    on_click: OnClick,
    still: bool,
    colors: Colors,
) -> AnyElement {
    let peak = if live {
        colors.text_primary
    } else {
        colors.text_secondary
    };
    div()
        .id(id)
        .w_full()
        .flex()
        .items_center()
        .gap(SPACE_2)
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .flex_none()
                .size(ICON_SIZE_ROW)
                .flex()
                .items_center()
                .justify_center()
                .child(chevron(open, colors.text_secondary)),
        )
        .child(div().min_w_0().truncate().child(animation::shimmer(
            summary,
            colors.text_secondary,
            peak,
            still || !live,
        )))
        .when(stats != (0, 0), |line| {
            line.child(stats_label(stats, colors))
        })
        .when(failed > 0, |line| {
            line.child(failed_label(Some(failed), colors))
        })
        .into_any_element()
}

/// The rail down a group's rows, from its line's chevron, `content` beside
/// it, `space` under the row above.
pub fn railed(content: impl IntoElement, space: gpui::Pixels, colors: Colors) -> Div {
    div()
        .relative()
        .w_full()
        .pt(space)
        .pl(ICON_SIZE_ROW + SPACE_2)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(ICON_SIZE_ROW / 2. - HAIRLINE / 2.)
                .w(HAIRLINE)
                .bg(colors.border),
        )
        .child(content)
}

/// A block of code-like text on the field's fill.
fn block(colors: Colors) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .px(SPACE_3)
        .py(SPACE_2)
        .gap(SPACE_1)
        .rounded(CONTROL_RADIUS)
        .bg(colors.field_background)
        .font_family(MONOSPACE_FONT)
        .text_size(TEXT_SMALL)
}

/// Up to `OUTPUT_LINES` of `text`, then how many more there are.
fn clipped(text: &str, color: Hsla, colors: Colors) -> Div {
    let lines: Vec<&str> = text.lines().collect();
    let shown = lines.len().min(OUTPUT_LINES);
    div()
        .flex()
        .flex_col()
        .text_color(color)
        .child(SharedString::from(lines[..shown].join("\n")))
        .when(lines.len() > shown, |output| {
            let more = tools::count(lines.len() - shown, "more line");
            output.child(
                div()
                    .text_color(colors.text_tertiary)
                    .child(format!("\u{2026} {more}")),
            )
        })
}

fn command_block(command: &str, output: &str, failed: bool, colors: Colors) -> AnyElement {
    let color = if failed {
        colors.danger
    } else {
        colors.text_secondary
    };
    block(colors)
        .child(
            div()
                .text_color(colors.text_primary)
                .child(SharedString::from(format!("$ {}", command.trim()))),
        )
        .when(!output.trim().is_empty(), |block| {
            block.child(clipped(output, color, colors))
        })
        .into_any_element()
}

fn output_block(output: &str, failed: bool, colors: Colors) -> AnyElement {
    let color = if failed {
        colors.danger
    } else {
        colors.text_secondary
    };
    block(colors)
        .child(clipped(output, color, colors))
        .into_any_element()
}

fn diff_block(lines: &[DiffLine], colors: Colors) -> AnyElement {
    let shown = lines.len().min(diff::MAX_LINES);
    let rows = lines[..shown].iter().map(|line| {
        let (mark, text, mark_color, background) = match line {
            DiffLine::Added(text) => ("+", text, colors.diff_added, colors.diff_added_background),
            DiffLine::Removed(text) => (
                "\u{2212}",
                text,
                colors.danger,
                colors.diff_removed_background,
            ),
            DiffLine::Kept(text) => (" ", text, colors.text_tertiary, gpui::transparent_black()),
        };
        div()
            .w_full()
            .flex()
            .px(SPACE_2)
            .bg(background)
            .child(
                div()
                    .flex_none()
                    .w(SPACE_3)
                    .text_color(mark_color)
                    .child(mark),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(text.clone())),
            )
    });
    div()
        .w_full()
        .flex()
        .flex_col()
        .py(SPACE_1)
        .rounded(CONTROL_RADIUS)
        .overflow_hidden()
        .bg(colors.field_background)
        .font_family(MONOSPACE_FONT)
        .text_size(TEXT_SMALL)
        .children(rows)
        .when(lines.len() > shown, |block| {
            let more = tools::count(lines.len() - shown, "more line");
            block.child(
                div()
                    .px(SPACE_2)
                    .text_color(colors.text_tertiary)
                    .child(format!("\u{2026} {more}")),
            )
        })
        .into_any_element()
}

/// The plan's steps: done ones struck through, the one in hand in full
/// color, the rest waiting.
fn todo_list(todos: &[tools::Todo], colors: Colors) -> AnyElement {
    let rows = todos.iter().map(|todo| {
        let (mark, color): (AnyElement, Hsla) = match todo.state {
            TodoState::Done => (
                svg()
                    .path(Icon::Check.path())
                    .size(ICON_SIZE_SMALL)
                    .text_color(colors.text_tertiary)
                    .into_any_element(),
                colors.text_tertiary,
            ),
            TodoState::Active => (
                svg()
                    .path(Icon::Forward.path())
                    .size(ICON_SIZE_SMALL)
                    .text_color(colors.text_primary)
                    .into_any_element(),
                colors.text_primary,
            ),
            TodoState::Pending => (
                div()
                    .size(px(8.))
                    .rounded_full()
                    .border(HAIRLINE)
                    .border_color(colors.text_tertiary)
                    .into_any_element(),
                colors.text_secondary,
            ),
        };
        let text = SharedString::from(todo.text.clone());
        let text = match todo.state {
            TodoState::Done => StyledText::new(text.clone()).with_highlights([(
                0..text.len(),
                gpui::HighlightStyle {
                    strikethrough: Some(StrikethroughStyle {
                        thickness: HAIRLINE,
                        color: None,
                    }),
                    ..Default::default()
                },
            )]),
            _ => StyledText::new(text),
        };
        div()
            .flex()
            .items_center()
            .gap(SPACE_2)
            .text_color(color)
            .when(todo.state == TodoState::Active, |row| {
                row.font_weight(FontWeight::MEDIUM)
            })
            .child(
                div()
                    .flex_none()
                    .size(ICON_SIZE_SMALL)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(mark),
            )
            .child(div().min_w_0().child(text))
    });
    div()
        .flex()
        .flex_col()
        .gap(SPACE_1)
        .text_size(TEXT_SMALL)
        .children(rows)
        .into_any_element()
}
