//! A menu: a list of choices on `popover_background` that hangs from a control
//! or opens at a point, over a window-wide backdrop whose click closes it.
//!
//! Arrows move the highlight, which the pointer moves too; Enter or a click
//! picks, and Escape closes. Closing gives focus back, unless focus left the
//! menu first, which closes it too. The owner keeps a [`Menu`] and draws it
//! where the menu hangs from; a pick runs in the owner, then the menu closes.

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, ClickEvent, Context, ElementId, FocusHandle, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point, ScrollHandle, SharedString,
    Styled, Subscription, Window, actions, anchored, deferred, div, point, prelude::*, px, svg,
};
use theme::{
    CONTROL_RADIUS, DISABLED_OPACITY, HAIRLINE, ICON_SIZE, MENU_RADIUS, MENU_WIDTH, ROW_HEIGHT,
    SPACE_1, SPACE_2, SPACE_3, TEXT_SMALL, Theme,
};

use crate::assets::Icon;

actions!(
    menu,
    [
        /// Closes the open menu.
        Dismiss,
        /// Moves the highlight down, wrapping at the end.
        SelectNext,
        /// Moves the highlight up, wrapping at the start.
        SelectPrevious,
        /// Picks the highlighted item.
        Confirm,
    ]
);

const CONTEXT: &str = "Menu";

/// Tallest a menu grows before it scrolls.
const MENU_MAX_HEIGHT: Pixels = px(360.);

/// Widest a menu grows to fit its lines before they truncate.
const MENU_MAX_WIDTH: Pixels = px(320.);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("escape", Dismiss, Some(CONTEXT)),
        gpui::KeyBinding::new("down", SelectNext, Some(CONTEXT)),
        gpui::KeyBinding::new("up", SelectPrevious, Some(CONTEXT)),
        gpui::KeyBinding::new("enter", Confirm, Some(CONTEXT)),
    ]);
}

/// A menu's state in its owner `T`: whether it is open, and on what (`K`).
pub(crate) struct Menu<T, K> {
    /// Finds this menu in its owner, for the menu's own listeners.
    access: fn(&mut T) -> &mut Menu<T, K>,
    open: Option<K>,
    /// Holds key focus while the menu is open, so its keys reach it.
    focus: FocusHandle,
    /// Where focus goes when the menu closes.
    give_back: Option<FocusHandle>,
    /// The entry under the pointer or the keyboard's highlight.
    highlighted: Option<usize>,
    scroll: ScrollHandle,
    _focus_out: Option<Subscription>,
}

/// Where a menu opens.
#[derive(Clone, Copy)]
pub(crate) enum Placement {
    /// From where the owner draws it, `SPACE_1` clear: below for a top
    /// anchor, above for a bottom one.
    Hang(Anchor),
    /// Below a point in the window, as under a right click.
    At(Point<Pixels>),
}

/// What a pick does in the menu's owner.
type Pick<T> = Rc<dyn Fn(&mut T, &mut Window, &mut Context<T>)>;

pub(crate) enum Entry<T> {
    /// A label over the items after it.
    Section(SharedString),
    Separator,
    Item(Item<T>),
}

pub(crate) struct Item<T> {
    id: ElementId,
    label: SharedString,
    /// A line under the label, saying what the choice does.
    detail: Option<SharedString>,
    /// `None` leaves out the check's slot; `Some(false)` keeps it empty, so
    /// labels stay aligned.
    checked: Option<bool>,
    shortcut: Option<SharedString>,
    enabled: bool,
    pick: Pick<T>,
}

impl<T> Item<T> {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        pick: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            checked: None,
            shortcut: None,
            enabled: true,
            pick: Rc::new(pick),
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl<T: 'static, K: 'static> Menu<T, K> {
    pub fn new(access: fn(&mut T) -> &mut Menu<T, K>, cx: &mut App) -> Self {
        Self {
            access,
            open: None,
            focus: cx.focus_handle(),
            give_back: None,
            highlighted: None,
            scroll: ScrollHandle::new(),
            _focus_out: None,
        }
    }

    /// What the menu is open on, while it is.
    pub fn opened(&self) -> Option<&K> {
        self.open.as_ref()
    }

    /// Opens the menu on `key`, to give focus back to where it is now.
    pub fn open(&mut self, key: K, window: &mut Window, cx: &mut Context<T>) {
        let give_back = window.focused(cx);
        self.open_from(key, give_back, window, cx);
    }

    /// Opens the menu on `key` and takes key focus, to give it to
    /// `give_back` on close. Opened on another key, it keeps where focus goes.
    pub fn open_from(
        &mut self,
        key: K,
        give_back: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        if self.open.is_none() {
            self.give_back = give_back;
        }
        self.open = Some(key);
        self.highlighted = None;
        self.scroll.set_offset(point(px(0.), px(0.)));
        let access = self.access;
        // Focus taken elsewhere, as by a shortcut or a hidden tab, closes the
        // menu and stays where it went.
        self._focus_out = Some(
            cx.on_focus_out(&self.focus, window, move |owner, _, _, cx| {
                let menu = access(owner);
                if menu.open.take().is_some() {
                    menu.give_back = None;
                    cx.notify();
                }
            }),
        );
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Closes the menu, if open, and gives focus back.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<T>) {
        if self.open.take().is_some() {
            if let Some(focus) = self.give_back.take() {
                window.focus(&focus, cx);
            }
            cx.notify();
        }
    }

    /// Moves the highlight one item along `items`, wrapping; from nothing,
    /// down starts at the first and up at the last.
    fn step(&mut self, items: &[usize], forward: bool, cx: &mut Context<T>) {
        let count = items.len();
        if count == 0 {
            return;
        }
        let at = self
            .highlighted
            .and_then(|entry| items.iter().position(|&item| item == entry));
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(at), true) => (at + 1) % count,
            (Some(at), false) => (at + count - 1) % count,
        };
        self.highlighted = Some(items[next]);
        self.scroll.scroll_to_item(items[next]);
        cx.notify();
    }

    /// The backdrop and the menu, while open. The owner draws it where a
    /// [`Placement::Hang`] menu hangs from.
    pub fn render(
        &self,
        id: &'static str,
        entries: Vec<Entry<T>>,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Option<AnyElement> {
        self.open.as_ref()?;
        let access = self.access;
        let close = move |owner: &mut T, window: &mut Window, cx: &mut Context<T>| {
            access(owner).close(window, cx)
        };
        // The entries a highlight can land on, with what picking each does.
        let picks: Rc<Vec<(usize, Pick<T>)>> = Rc::new(
            (entries.iter().enumerate())
                .filter_map(|(at, entry)| match entry {
                    Entry::Item(item) if item.enabled => Some((at, item.pick.clone())),
                    _ => None,
                })
                .collect(),
        );
        let items: Rc<Vec<usize>> = Rc::new(picks.iter().map(|(at, _)| *at).collect());
        let rows: Vec<AnyElement> = (entries.into_iter().enumerate())
            .map(|(at, entry)| self.render_entry(at, entry, cx))
            .collect();
        let colors = Theme::of(cx).colors;
        let panel = div()
            .id(id)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(move |owner, _: &Dismiss, window, cx| close(owner, window, cx)))
            .on_action(cx.listener({
                let items = items.clone();
                move |owner, _: &SelectNext, _, cx| access(owner).step(&items, true, cx)
            }))
            .on_action(cx.listener(move |owner, _: &SelectPrevious, _, cx| {
                access(owner).step(&items, false, cx)
            }))
            .on_action(cx.listener(move |owner, _: &Confirm, window, cx| {
                let highlighted = access(owner).highlighted;
                let pick = picks
                    .iter()
                    .find(|(at, _)| Some(*at) == highlighted)
                    .map(|(_, pick)| pick.clone());
                if let Some(pick) = pick {
                    pick(owner, window, cx);
                    close(owner, window, cx);
                }
            }))
            .on_hover(cx.listener(move |owner, hovered: &bool, _, cx| {
                if !*hovered {
                    access(owner).highlighted = None;
                    cx.notify();
                }
            }))
            .occlude()
            .min_w(MENU_WIDTH)
            .max_w(MENU_MAX_WIDTH)
            .max_h(MENU_MAX_HEIGHT)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(SPACE_1 - HAIRLINE)
            .flex()
            .flex_col()
            .rounded(MENU_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.popover_background)
            .shadow_lg()
            .children(rows);
        let hung = match placement {
            Placement::Hang(anchor) => {
                let below = matches!(anchor, Anchor::TopLeft | Anchor::TopRight);
                anchored()
                    .anchor(anchor)
                    .offset(point(px(0.), if below { SPACE_1 } else { -SPACE_1 }))
            }
            Placement::At(position) => anchored().position(position).offset(point(px(0.), SPACE_1)),
        };
        // Under the menu, a click anywhere else closes it and goes no further,
        // so it can't also press what lies beneath (the control that opened it
        // included, which would reopen it). The second press of a double-click
        // that opened the menu leaves it open.
        let backdrop_close = move |owner: &mut T,
                                   event: &MouseDownEvent,
                                   window: &mut Window,
                                   cx: &mut Context<T>| {
            if event.click_count < 2 {
                close(owner, window, cx)
            }
        };
        let viewport = window.viewport_size();
        let backdrop = div()
            .id((id, 1u64))
            .w(viewport.width)
            .h(viewport.height)
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(backdrop_close))
            .on_mouse_down(MouseButton::Right, cx.listener(backdrop_close));
        Some(
            div()
                .child(
                    deferred(anchored().position(point(px(0.), px(0.))).child(backdrop))
                        .priority(1),
                )
                .child(deferred(hung.snap_to_window_with_margin(SPACE_2).child(panel)).priority(2))
                .into_any_element(),
        )
    }

    fn render_entry(&self, at: usize, entry: Entry<T>, cx: &mut Context<T>) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let item = match entry {
            Entry::Section(text) => {
                return div()
                    .h(ROW_HEIGHT)
                    .flex_none()
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .text_color(colors.text_secondary)
                    .child(text)
                    .into_any_element();
            }
            Entry::Separator => {
                return div()
                    .flex_none()
                    .my(SPACE_1)
                    .h(HAIRLINE)
                    .bg(colors.border)
                    .into_any_element();
            }
            Entry::Item(item) => item,
        };
        let access = self.access;
        let highlighted = self.highlighted == Some(at);
        // Drawn because of state, the highlight redraws the row, so its
        // supporting text keeps its contrast on the surface.
        let supporting = if highlighted {
            colors.text_secondary_on_surface
        } else {
            colors.text_secondary
        };
        let enabled = item.enabled;
        let pick = item.pick;
        div()
            .id(item.id)
            .min_h(ROW_HEIGHT)
            .flex_none()
            .px(SPACE_2)
            .when(item.detail.is_some(), |row| row.py(SPACE_1))
            .flex()
            .items_center()
            .rounded(CONTROL_RADIUS)
            .when(highlighted, |row| row.bg(colors.hover))
            .on_mouse_move(cx.listener(move |owner, _: &MouseMoveEvent, _, cx| {
                let menu = access(owner);
                let target = enabled.then_some(at);
                if menu.highlighted != target {
                    menu.highlighted = target;
                    cx.notify();
                }
            }))
            .when(enabled, |row| {
                row.on_click(cx.listener(move |owner, _: &ClickEvent, window, cx| {
                    pick(owner, window, cx);
                    access(owner).close(window, cx);
                }))
            })
            .when(!enabled, |row| row.opacity(DISABLED_OPACITY))
            .when_some(item.checked, |row, checked| {
                row.child(
                    div()
                        .size(ICON_SIZE)
                        .flex_none()
                        .mr(SPACE_2)
                        .when(checked, |slot| {
                            slot.child(
                                svg()
                                    .path(Icon::Check.path())
                                    .size(ICON_SIZE)
                                    .text_color(colors.text_primary),
                            )
                        }),
                )
            })
            .child(
                div()
                    .flex_auto()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(colors.text_primary)
                            .child(item.label),
                    )
                    .children(item.detail.map(|detail| {
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(TEXT_SMALL)
                            .text_color(supporting)
                            .child(detail)
                    })),
            )
            .children(item.shortcut.map(|keys| {
                div()
                    .flex_none()
                    .pl(SPACE_3)
                    .text_color(supporting)
                    .child(keys)
            }))
            .into_any_element()
    }
}
