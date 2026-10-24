//! A session's conversation in the main area, over the docked composer: the
//! person's messages, what came back, and how each turn ended, newest at the
//! bottom, in the composer's column. Tool calls in a row, with the thinking
//! between them, gather under one line that stays open while the agent adds
//! to it and closes once it moves on. While output streams the list keeps
//! to its bottom, unless the person scrolled up: there it stays until they
//! scroll back down, or press the button over the composer that takes them
//! there. A line under the newest row shows what the agent is doing, and
//! for how long, so it stays where the eye is.

mod animation;
mod diff;
mod items;
mod markdown;
mod render;
mod rows;
mod summary;
mod tool_row;
mod tools;
mod working;

use std::{
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

use agent::{Item, TurnState};
use gpui::{
    AnyElement, Context, Div, ElementId, Entity, FollowMode, Hsla, IntoElement, ListAlignment,
    ListState, ParentElement, Pixels, Render, Styled, Subscription, Window, canvas, div, fill,
    linear_color_stop, linear_gradient, list, prelude::*, px, svg,
};
use theme::{
    COMPOSER_MAX_WIDTH, CONTROL_SIZE, ICON_SIZE, SCROLL_EDGE_FADE, SPACE_1, SPACE_2, SPACE_3, Theme,
};

pub use render::line as markdown_line;
use rows::{Disclosure, Nest, Row, Stamped};

use crate::{agents::SessionAgent, assets::Icon, tooltip::TooltipExt};

/// A text's Markdown, with the length of the text it was parsed from.
type Parsed = (usize, Rc<Vec<markdown::Block>>);

/// How far past the view the list lays rows out, so they don't pop in.
const OVERDRAW: Pixels = px(800.);

pub struct Conversation {
    agent: Entity<SessionAgent>,
    list: ListState,
    rows: Vec<Stamped>,
    disclosure: Disclosure,
    /// When each row came while the agent worked, for its entrance.
    appeared: HashMap<Row, Instant>,
    /// The parts of each streaming text still fading in, by turn and item:
    /// the length the text had before each came, and when it came.
    arriving: HashMap<(usize, usize), Vec<(usize, Instant)>>,
    /// The length of each streaming text when last seen.
    seen: HashMap<(usize, usize), usize>,
    /// Each text's Markdown, by turn and item, with its length when parsed.
    parsed: HashMap<(usize, usize), Parsed>,
    /// The project's folder, which paths in tool calls read relative to.
    folder: Option<PathBuf>,
    /// The redraws for the clocks and the motion run.
    ticking: bool,
    _agent: Subscription,
}

impl Conversation {
    pub fn new(agent: Entity<SessionAgent>, cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Top, OVERDRAW);
        list.set_follow_mode(FollowMode::Tail);
        let subscription = cx.observe(&agent, |conversation, _, cx| conversation.sync(cx));
        let folder = agent.read(cx).folder(cx);
        let mut conversation = Self {
            agent,
            list,
            rows: Vec::new(),
            disclosure: Disclosure::default(),
            appeared: HashMap::new(),
            arriving: HashMap::new(),
            seen: HashMap::new(),
            parsed: HashMap::new(),
            folder,
            ticking: false,
            _agent: subscription,
        };
        conversation.sync(cx);
        conversation
    }

    /// Takes the transcript's rows in: rows whose look changed are measured
    /// again where they are, and from where rows came or went the list lays
    /// out anew. Rows that come while the agent works make an entrance.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let agent = self.agent.read(cx);
        let rows = rows::rows(agent.transcript(), &self.disclosure);
        let change = rows::change(&self.rows, &rows);
        for at in change.changed {
            self.list.remeasure_items(at..at + 1);
        }
        if let Some((replaced, count)) = change.replaced.clone() {
            self.list.splice(replaced, count);
        }
        if agent.is_busy() && !cx.reduce_motion() {
            let now = Instant::now();
            if let Some((_, count)) = change.replaced {
                let new = &rows[rows.len() - count..];
                let old: std::collections::HashSet<Row> =
                    self.rows.iter().map(|(row, _)| *row).collect();
                for (row, _) in new.iter().filter(|(row, _)| !old.contains(row)) {
                    self.appeared.entry(*row).or_insert(now);
                }
            }
        }
        let animate = agent.is_busy() && !cx.reduce_motion();
        let now = Instant::now();
        let mut seen = HashMap::new();
        for (at, turn) in agent.transcript().turns().iter().enumerate() {
            for (item, part) in turn.items.iter().enumerate() {
                let Item::Text(text) = part else { continue };
                if !text.streaming {
                    continue;
                }
                let before = self.seen.get(&(at, item)).copied().unwrap_or(0);
                if animate && text.text.len() > before {
                    self.arriving
                        .entry((at, item))
                        .or_default()
                        .push((before, now));
                }
                seen.insert((at, item), text.text.len());
            }
        }
        self.seen = seen;
        self.rows = rows;
        self.tick(cx);
        cx.notify();
    }

    /// How long until the next redraw: a frame while something moves, a
    /// second while only the clocks run, none while nothing changes.
    fn interval(&mut self, cx: &mut Context<Self>) -> Option<Duration> {
        self.appeared
            .retain(|_, since| animation::entering(since.elapsed()));
        for parts in self.arriving.values_mut() {
            parts.retain(|(_, since)| animation::arriving(since.elapsed()));
        }
        self.arriving.retain(|_, parts| !parts.is_empty());
        let busy = self.agent.read(cx).is_busy();
        let moving =
            (busy && !cx.reduce_motion()) || !self.appeared.is_empty() || !self.arriving.is_empty();
        if moving {
            Some(animation::FRAME)
        } else {
            busy.then_some(Duration::from_secs(1))
        }
    }

    /// Starts the redraws when they are wanted and aren't running; they end
    /// themselves once nothing needs them.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.ticking || self.interval(cx).is_none() {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |conversation, cx| {
            loop {
                let next = conversation.update(cx, |conversation, cx| {
                    cx.notify();
                    let next = conversation.interval(cx);
                    conversation.ticking = next.is_some();
                    next
                });
                let Ok(Some(wait)) = next else { break };
                cx.background_executor().timer(wait).await;
            }
        })
        .detach();
    }

    fn toggle_item(&mut self, turn: usize, item: usize, cx: &mut Context<Self>) {
        if !self.disclosure.items.remove(&(turn, item)) {
            self.disclosure.items.insert((turn, item));
        }
        self.sync(cx);
    }

    fn toggle_group(&mut self, turn: usize, first: usize, cx: &mut Context<Self>) {
        let agent = self.agent.read(cx);
        let Some(each) = agent.transcript().turns().get(turn) else {
            return;
        };
        let Some(range) = rows::groups(each)
            .into_iter()
            .find(|range| range.start == first)
        else {
            return;
        };
        let open = rows::open(&self.disclosure, turn, each, &range);
        self.disclosure.groups.insert((turn, first), !open);
        self.sync(cx);
    }

    /// The Markdown of item `item` of turn `turn`, parsed again only once
    /// its text grew.
    fn markdown(&mut self, turn: usize, item: usize, text: &str) -> Rc<Vec<markdown::Block>> {
        match self.parsed.get(&(turn, item)) {
            Some((length, blocks)) if *length == text.len() => blocks.clone(),
            _ => {
                let blocks = Rc::new(markdown::parse(text));
                self.parsed
                    .insert((turn, item), (text.len(), blocks.clone()));
                blocks
            }
        }
    }

    /// The parts of the end of item `item` of turn `turn`, `length` long now,
    /// still fading in, measured from its end.
    fn tail(&self, turn: usize, item: usize, length: usize) -> Vec<(usize, usize, f32)> {
        let Some(parts) = self.arriving.get(&(turn, item)) else {
            return Vec::new();
        };
        let ends = parts
            .iter()
            .skip(1)
            .map(|(start, _)| *start)
            .chain([length]);
        (parts.iter().zip(ends))
            .map(|(&(start, since), end)| {
                let amount = animation::arrived(since.elapsed());
                (
                    length.saturating_sub(start),
                    length.saturating_sub(end),
                    amount,
                )
            })
            .collect()
    }

    fn render_row(&mut self, at: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let still = cx.reduce_motion();
        let Some(&(row, _)) = self.rows.get(at) else {
            return div().into_any_element();
        };
        let agent = self.agent.clone();
        let agent = agent.read(cx);
        let turns = agent.transcript().turns();
        let folder = self.folder.clone();
        let folder = folder.as_deref();
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
            Row::Group { turn, first, last } => {
                let Some(each) = turns.get(turn) else {
                    return nothing();
                };
                let range = first..last + 1;
                let Some(parts) = each.items.get(range.clone()) else {
                    return nothing();
                };
                let calls: Vec<&agent::ToolCall> = (parts.iter())
                    .filter_map(|part| match part {
                        Item::Tool(call) => Some(call),
                        _ => None,
                    })
                    .collect();
                let thoughts = parts.len() - calls.len();
                let stats = (calls.iter())
                    .filter_map(|call| tools::describe(call, folder).stats)
                    .fold((0, 0), |(a, r), (added, removed)| (a + added, r + removed));
                let open = rows::open(&self.disclosure, turn, each, &range);
                let live = rows::live(each, &range);
                let id = ElementId::NamedInteger("group".into(), key(turn, first));
                let toggle = cx.listener(move |conversation, _, _, cx| {
                    conversation.toggle_group(turn, first, cx)
                });
                let summary = summary::summary(&calls, thoughts, folder);
                let failed = summary::failures(&calls);
                let header = tool_row::group(
                    summary,
                    stats,
                    failed,
                    open,
                    live,
                    id,
                    Box::new(toggle),
                    still,
                    colors,
                );
                (header, SPACE_2)
            }
            Row::Item { turn, item, nest } => {
                let Some(part) = turns.get(turn).and_then(|each| each.items.get(item)) else {
                    return nothing();
                };
                let key = key(turn, item);
                let open = self.disclosure.items.contains(&(turn, item));
                let toggle = cx.listener(move |conversation, _, _, cx| {
                    conversation.toggle_item(turn, item, cx)
                });
                let (content, space) = match part {
                    Item::Text(text) => {
                        let blocks = self.markdown(turn, item, &text.text);
                        let tail = self.tail(turn, item, text.text.len());
                        (render::blocks(&blocks, key, &tail, colors), SPACE_3)
                    }
                    Item::Thinking(text) => {
                        let since = agent.thinking_since(turn, item);
                        let took = agent.thought_for(turn, item);
                        let id = ElementId::NamedInteger("thinking".into(), key);
                        let row =
                            items::thinking(text, since, took, open, id, toggle, still, colors);
                        (row, SPACE_2)
                    }
                    Item::Tool(call) => {
                        let id = ElementId::NamedInteger("tool".into(), key);
                        let row =
                            tool_row::tool(call, folder, open, id, Box::new(toggle), still, colors);
                        (row, SPACE_2)
                    }
                    Item::Permission(permission) => {
                        (items::permission(permission, colors), SPACE_2)
                    }
                };
                match nest {
                    Nest::Alone => (content, space),
                    Nest::Inside | Nest::Last => {
                        let railed = tool_row::railed(content, SPACE_1, colors);
                        (railed.into_any_element(), px(0.))
                    }
                }
            }
            Row::Working => {
                let Some(since) = agent.busy_since() else {
                    return nothing();
                };
                let doing = working::doing(agent.transcript(), folder);
                let elapsed = items::elapsed(since.elapsed());
                (
                    working::indicator(doing, elapsed, still, colors).into_any_element(),
                    SPACE_3,
                )
            }
            Row::End { turn } => match turns.get(turn).map(|turn| &turn.state) {
                Some(TurnState::Ended(ending)) => (items::end(ending, colors), SPACE_3),
                _ => return nothing(),
            },
        };
        let entered =
            (self.appeared.get(&row)).map_or(1., |since| animation::entered(since.elapsed()));
        in_column(content, space)
            .when(entered < 1., |row| {
                row.opacity(entered)
                    .relative()
                    .top(px(animation::ENTER_RISE * (1. - entered)))
            })
            .into_any_element()
    }
}

/// One number for item `item` of turn `turn`, to name its elements by.
fn key(turn: usize, item: usize) -> u64 {
    (turn as u64) << 24 | item as u64
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

impl Conversation {
    /// The rows fade into the composer while more lie below, thinning out
    /// over the last `SCROLL_EDGE_FADE` of them. Read at paint, once the
    /// list has laid out this frame.
    fn edge_fade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let card = Theme::of(cx).colors.panel_background;
        let list = self.list.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let below =
                    list.max_offset_for_scrollbar().y + list.scroll_px_offset_for_scrollbar().y;
                let below = (below / SCROLL_EDGE_FADE).clamp(0., 1.);
                if below > 0. {
                    let to = card.opacity(below);
                    let from = Hsla { a: 0., ..to };
                    window.paint_quad(fill(
                        bounds,
                        linear_gradient(
                            180.,
                            linear_color_stop(from, 0.),
                            linear_color_stop(to, 1.),
                        ),
                    ));
                }
            },
        )
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(SCROLL_EDGE_FADE)
    }

    /// The button that brings a list scrolled up back to its end, centered
    /// over the composer: a circle on a menu's surface, lifted as a menu is,
    /// its arrow turning `text_primary` under the pointer.
    fn scroll_to_end_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let list = self.list.clone();
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(SPACE_2)
            .flex()
            .justify_center()
            .child(
                div()
                    .id("scroll-to-end")
                    .group("scroll-to-end")
                    .size(CONTROL_SIZE)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_1()
                    .border_color(colors.popover_border)
                    .bg(colors.popover_background)
                    .shadow(colors.popover_shadows())
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        list.scroll_to_end();
                        list.set_follow_mode(FollowMode::Tail);
                        cx.notify();
                    }))
                    .child(
                        svg()
                            .path(Icon::ScrollDown.path())
                            .size(ICON_SIZE)
                            .text_color(colors.text_secondary)
                            .group_hover("scroll-to-end", |icon| {
                                icon.text_color(colors.text_primary)
                            }),
                    )
                    .with_tooltip("Scroll to the end"),
            )
    }
}

impl Render for Conversation {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The list redraws its view on every scroll, so this follows it.
        let away = !self.list.is_following_tail() && self.list.is_scrolled_to_end() == Some(false);
        let rows = list(
            self.list.clone(),
            cx.processor(|conversation, at, window, cx| conversation.render_row(at, window, cx)),
        )
        .flex_1()
        .min_h_0()
        .w_full()
        .pt(SPACE_3)
        // Room under the last row, inside the scroll, so at the end it stands
        // clear of the composer and of the edge fade, while rows scrolled up
        // still pass under the fade into the composer.
        .pb(SCROLL_EDGE_FADE + SPACE_2);
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(rows)
            .child(self.edge_fade(cx))
            .when(away, |view| view.child(self.scroll_to_end_button(cx)))
    }
}
