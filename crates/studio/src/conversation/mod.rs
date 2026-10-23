//! A session's conversation in the main area, over the docked composer: the
//! person's messages, what came back, and how each turn ended, newest at the
//! bottom, in the composer's column. While output streams the list keeps to
//! its bottom, unless the person scrolled up: there it stays until they
//! scroll back down. A line over the composer says the agent is working, and
//! for how long; its room is kept, so the composer never moves.

mod items;
mod markdown;
mod render;
mod rows;

use std::{collections::HashSet, time::Duration};

use agent::{Item, TurnState};
use gpui::{
    AnyElement, Context, Div, ElementId, Entity, FollowMode, IntoElement, ListAlignment, ListState,
    ParentElement, Pixels, Render, Styled, Subscription, Task, Window, div, list, px,
};
use theme::{COMPOSER_MAX_WIDTH, CONTROL_SIZE, SPACE_2, SPACE_3, TEXT_SMALL, Theme};

pub use render::line as markdown_line;
use rows::{Row, Stamped};

use crate::agents::SessionAgent;

/// How far past the view the list lays rows out, so they don't pop in.
const OVERDRAW: Pixels = px(800.);

pub struct Conversation {
    agent: Entity<SessionAgent>,
    list: ListState,
    rows: Vec<Stamped>,
    /// The thinking blocks opened, by turn and item.
    open: HashSet<(usize, usize)>,
    /// Redraws every second while the agent works, for the clocks.
    clock: Option<Task<()>>,
    _agent: Subscription,
}

impl Conversation {
    pub fn new(agent: Entity<SessionAgent>, cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Top, OVERDRAW);
        list.set_follow_mode(FollowMode::Tail);
        let subscription = cx.observe(&agent, |conversation, _, cx| conversation.sync(cx));
        let mut conversation = Self {
            agent,
            list,
            rows: Vec::new(),
            open: HashSet::new(),
            clock: None,
            _agent: subscription,
        };
        conversation.sync(cx);
        conversation
    }

    /// Takes the transcript's rows in: rows whose look changed are measured
    /// again where they are, and from where rows came or went the list lays
    /// out anew.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let agent = self.agent.read(cx);
        let rows = rows::rows(agent.transcript(), &self.open);
        let change = rows::change(&self.rows, &rows);
        for at in change.changed {
            self.list.remeasure_items(at..at + 1);
        }
        if let Some((replaced, count)) = change.replaced {
            self.list.splice(replaced, count);
        }
        self.rows = rows;
        let busy = agent.is_busy();
        if busy != self.clock.is_some() {
            self.clock = busy.then(|| {
                cx.spawn(async move |conversation, cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        if conversation.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                })
            });
        }
        cx.notify();
    }

    fn toggle(&mut self, turn: usize, item: usize, cx: &mut Context<Self>) {
        if !self.open.remove(&(turn, item)) {
            self.open.insert((turn, item));
        }
        self.sync(cx);
    }

    fn render_row(&mut self, at: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let Some(&(row, _)) = self.rows.get(at) else {
            return div().into_any_element();
        };
        let agent = self.agent.read(cx);
        let turns = agent.transcript().turns();
        // The rows follow the transcript on every change; a row it no
        // longer has draws as nothing rather than failing.
        let nothing = || div().into_any_element();
        let (content, space) = match row {
            Row::Message { turn } => {
                let Some(turn) = turns.get(turn) else {
                    return nothing();
                };
                let Some(message) = &turn.message else {
                    return nothing();
                };
                let queued = turn.state == TurnState::Queued;
                let space = if at == 0 { px(0.) } else { SPACE_3 * 2. };
                (items::message(message, queued, colors), space)
            }
            Row::Item { turn, item } => {
                let Some(part) = turns.get(turn).and_then(|each| each.items.get(item)) else {
                    return nothing();
                };
                let key = (turn as u64) << 24 | item as u64;
                match part {
                    Item::Text(text) => {
                        let blocks = markdown::parse(&text.text);
                        (render::blocks(&blocks, key, colors), SPACE_3)
                    }
                    Item::Thinking(text) => {
                        let since = agent.thinking_since(turn, item);
                        let open = self.open.contains(&(turn, item));
                        let id = ElementId::NamedInteger("thinking".into(), key);
                        let toggle = cx.listener(move |conversation, _, _, cx| {
                            conversation.toggle(turn, item, cx)
                        });
                        (
                            items::thinking(text, since, open, id, toggle, colors),
                            SPACE_3,
                        )
                    }
                    Item::Tool(call) => (items::tool(call, colors), SPACE_2),
                    Item::Permission(permission) => {
                        (items::permission(permission, colors), SPACE_2)
                    }
                }
            }
            Row::End { turn } => match turns.get(turn).map(|turn| &turn.state) {
                Some(TurnState::Ended(ending)) => (items::end(ending, colors), SPACE_2),
                _ => return nothing(),
            },
        };
        in_column(content, space).into_any_element()
    }
}

/// `content` in the composer's column: at most `COMPOSER_MAX_WIDTH`,
/// centered, `SPACE_3` clear of the card's sides when the card is narrower,
/// `space` under what is above.
fn in_column(content: impl IntoElement, space: Pixels) -> Div {
    div().w_full().px(SPACE_3).flex().justify_center().child(
        div()
            .w_full()
            .max_w(COMPOSER_MAX_WIDTH)
            .pt(space)
            .child(content),
    )
}

impl Render for Conversation {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let working = (self.agent.read(cx).busy_since())
            .map(|since| format!("Working\u{2026} {}", items::elapsed(since.elapsed())));
        let rows = list(
            self.list.clone(),
            cx.processor(|conversation, at, window, cx| conversation.render_row(at, window, cx)),
        )
        .flex_1()
        .w_full()
        .pt(SPACE_3);
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(rows)
            .child(
                in_column(
                    div()
                        .h(CONTROL_SIZE)
                        .px(SPACE_3)
                        .flex()
                        .items_center()
                        .text_size(TEXT_SMALL)
                        .text_color(colors.text_secondary)
                        .children(working),
                    px(0.),
                )
                .flex_none(),
            )
    }
}
