//! The line under the conversation while the agent works: a square of dots
//! rising in a wave and what the agent is doing in words, glowing. How long
//! it has worked is on the turn's header.

use std::path::Path;

use agent::{Item, Status, ToolStatus, Transcript, TurnState};
use gpui::{Div, ParentElement, Styled, div, px};
use theme::{Colors, ICON_SIZE_ROW, SPACE_2};

use super::{
    animation,
    tools::{self, Target},
};

/// Dots on a side of the working indicator's square.
const SIDE: usize = 3;
/// A dot's side; the dots stand half of it apart.
const DOT: f32 = 3.;

/// What the agent is doing, in a few words: "Thinking", "Reading
/// motion.rs", "Waiting for your answer".
pub fn doing(transcript: &Transcript, folder: Option<&Path>) -> String {
    match transcript.status() {
        Status::Starting => return "Starting".to_owned(),
        Status::Stopping => return "Stopping".to_owned(),
        Status::AwaitingPermission => return "Waiting for your answer".to_owned(),
        _ => {}
    }
    let turn = (transcript.turns().iter()).find(|turn| turn.state == TurnState::Running);
    let Some(turn) = turn else {
        return "Working".to_owned();
    };
    let running = turn.items.iter().rev().find_map(|item| match item {
        Item::Tool(call) if call.status == ToolStatus::Running => Some(call),
        _ => None,
    });
    if let Some(call) = running {
        let tool = tools::describe(call, folder);
        return match tool.target {
            Some(Target::File { name, .. }) => format!("{} {name}", tool.live_verb),
            Some(Target::Code(text) | Target::Words(text)) => format!("{} {text}", tool.live_verb),
            None => tool.live_verb.to_owned(),
        };
    }
    match turn.items.last() {
        Some(Item::Thinking(text)) if text.streaming => "Thinking".to_owned(),
        Some(Item::Text(text)) if text.streaming => "Writing".to_owned(),
        _ => "Working".to_owned(),
    }
}

/// The indicator: the dots and `doing`.
pub fn indicator(doing: String, still: bool, colors: Colors) -> Div {
    let gap = px(DOT / 2.);
    let square = div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(gap)
        .children((0..SIDE).map(|row| {
            let tint = colors.activity[row];
            div().flex().gap(gap).children((0..SIDE).map(move |column| {
                div()
                    .size(px(DOT))
                    .rounded_full()
                    .bg(tint)
                    .opacity(animation::wave(row, column, SIDE, still))
            }))
        }));
    div()
        .flex()
        .items_center()
        .gap(SPACE_2)
        .min_w_0()
        // In the rows' icon slot, so the words stand in line with theirs.
        .child(
            div()
                .flex_none()
                .size(ICON_SIZE_ROW)
                .flex()
                .items_center()
                .justify_center()
                .child(square),
        )
        .child(div().min_w_0().truncate().child(animation::shimmer(
            doing,
            colors.text_secondary,
            colors.text_primary,
            still,
        )))
}
