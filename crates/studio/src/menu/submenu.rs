//! Moving through a menu and the submenu an item opens beside it. The pointer
//! opens a submenu by resting on its item and closes it on another item; the
//! right arrow and Enter open it with its first item highlighted, the left
//! arrow goes back to the item.

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Pixels, Styled, Window, anchored, deferred,
    div, prelude::*, px,
};
use theme::{HAIRLINE, SPACE_1, SPACE_2};

use super::{
    Level, Menu,
    entry::{Entry, Pick},
};

/// Narrowest a submenu draws: its items are short choices.
const SUBMENU_WIDTH: Pixels = px(176.);

/// Where the highlight can go in one frame's entries, and what each pick does.
pub(super) struct Moves<T> {
    /// The menu's items the highlight can land on, in order.
    pub items: Vec<usize>,
    /// What each item does: picks, or opens its submenu, whose first item the
    /// keys highlight.
    actions: Vec<(usize, Action<T>)>,
    /// The open submenu's items the highlight can land on, with their picks.
    sub_picks: Vec<(usize, Pick<T>)>,
}

enum Action<T> {
    Pick(Pick<T>),
    Open(Option<usize>),
}

/// The entries the highlight can land on, with what each does.
fn enabled<T>(entries: &[Entry<T>]) -> impl Iterator<Item = (usize, &super::Item<T>)> {
    (entries.iter().enumerate()).filter_map(|(at, entry)| match entry {
        Entry::Item(item) if item.enabled => Some((at, item)),
        _ => None,
    })
}

impl<T> Moves<T> {
    pub fn of(entries: &[Entry<T>], sub: Option<usize>) -> Self {
        let mut moves = Self {
            items: Vec::new(),
            actions: Vec::new(),
            sub_picks: Vec::new(),
        };
        for (at, item) in enabled(entries) {
            moves.items.push(at);
            let action = match &item.submenu {
                None => Action::Pick(item.pick.clone()),
                Some((_, inner)) => {
                    if sub == Some(at) {
                        moves.sub_picks = (enabled(inner))
                            .map(|(at, item)| (at, item.pick.clone()))
                            .collect();
                    }
                    Action::Open(enabled(inner).map(|(at, _)| at).next())
                }
            };
            moves.actions.push((at, action));
        }
        moves
    }

    fn action(&self, at: usize) -> Option<&Action<T>> {
        (self.actions.iter()).find_map(|(each, action)| (*each == at).then_some(action))
    }

    pub fn has_submenu(&self, at: usize) -> bool {
        matches!(self.action(at), Some(Action::Open(_)))
    }

    pub fn pick(&self, at: usize) -> Option<Pick<T>> {
        match self.action(at)? {
            Action::Pick(pick) => Some(pick.clone()),
            Action::Open(_) => None,
        }
    }

    pub fn sub_pick(&self, at: usize) -> Option<Pick<T>> {
        (self.sub_picks.iter()).find_map(|(each, pick)| (*each == at).then(|| pick.clone()))
    }
}

/// One step along `items` from `from`, wrapping; from nothing, down starts at
/// the first and up at the last.
fn step_along(items: &[usize], from: Option<usize>, forward: bool) -> Option<usize> {
    let count = items.len();
    if count == 0 {
        return None;
    }
    let at = from.and_then(|entry| items.iter().position(|&item| item == entry));
    let next = match (at, forward) {
        (None, true) => 0,
        (None, false) => count - 1,
        (Some(at), true) => (at + 1) % count,
        (Some(at), false) => (at + count - 1) % count,
    };
    Some(items[next])
}

impl<T: 'static, K: 'static> Menu<T, K> {
    /// Moves the highlight one item, in the submenu while it holds the
    /// highlight, else in the menu, which closes the submenu.
    pub(super) fn step(&mut self, moves: &Moves<T>, forward: bool, cx: &mut Context<T>) {
        if self.sub_highlighted.is_some() {
            let items: Vec<usize> = moves.sub_picks.iter().map(|(at, _)| *at).collect();
            self.sub_highlighted = step_along(&items, self.sub_highlighted, forward);
        } else if let Some(next) = step_along(&moves.items, self.highlighted, forward) {
            self.close_submenu();
            self.highlighted = Some(next);
            self.scroll.scroll_to_item(next);
        }
        cx.notify();
    }

    /// The pointer moved over entry `at`: it takes the highlight, and in the
    /// menu opens the item's submenu or closes another's.
    pub(super) fn point_at(
        &mut self,
        level: Level,
        at: usize,
        enabled: bool,
        has_submenu: bool,
        cx: &mut Context<T>,
    ) {
        let target = enabled.then_some(at);
        match level {
            Level::Sub if self.sub_highlighted != target => self.sub_highlighted = target,
            Level::Root if self.highlighted != target || self.sub_highlighted.is_some() => {
                self.highlighted = target;
                let sub = target.filter(|_| has_submenu);
                if self.sub != sub {
                    self.sub = sub;
                }
                self.sub_highlighted = None;
            }
            _ => return,
        }
        cx.notify();
    }

    /// Opens item `at`'s submenu beside it, nothing in it highlighted.
    pub(super) fn open_submenu(&mut self, at: usize, cx: &mut Context<T>) {
        self.highlighted = Some(at);
        self.sub = Some(at);
        self.sub_highlighted = None;
        cx.notify();
    }

    /// Opens item `at`'s submenu with its first item highlighted, so the
    /// arrows and Enter go on in it.
    pub(super) fn open_submenu_with_keys(
        &mut self,
        at: usize,
        moves: &Moves<T>,
        cx: &mut Context<T>,
    ) {
        let Some(Action::Open(first)) = moves.action(at) else {
            return;
        };
        self.open_submenu(at, cx);
        self.sub_highlighted = *first;
    }

    pub(super) fn close_submenu(&mut self) {
        self.sub = None;
        self.sub_highlighted = None;
    }

    /// The submenu beside its item, its first item level with the item, and
    /// flipped to the menu's other side where the window ends.
    pub(super) fn render_submenu(
        &self,
        entries: Vec<Entry<T>>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> AnyElement {
        let access = self.access;
        let rows: Vec<AnyElement> = (entries.into_iter().enumerate())
            .map(|(at, entry)| self.render_entry(Level::Sub, at, entry, window, cx))
            .collect();
        let panel = Self::panel(cx)
            .id("submenu")
            .min_w(SUBMENU_WIDTH)
            .on_hover(cx.listener(move |owner, hovered: &bool, _, cx| {
                if !*hovered {
                    access(owner).sub_highlighted = None;
                    cx.notify();
                }
            }))
            .child(div().p(SPACE_1 - HAIRLINE).flex().flex_col().children(rows));
        div()
            .absolute()
            .top(-SPACE_1)
            .left_full()
            .child(
                deferred(anchored().snap_to_window_with_margin(SPACE_2).child(panel)).priority(3),
            )
            .into_any_element()
    }
}
