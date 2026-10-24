//! A menu: a list of choices on `popover_background` that hangs from a control
//! or opens at a point, over a window-wide backdrop whose click closes it.
//!
//! Arrows move the highlight, which the pointer moves too; Enter or a click
//! picks, and Escape closes. Closing gives focus back, unless focus left the
//! menu first, which closes it too. The owner keeps a [`Menu`] and draws it
//! where the menu hangs from; a pick runs in the owner, then the menu closes.
//!
//! A menu made [`Menu::with_search`] leads with a search field, which holds
//! the focus: typing filters, as the owner's entries say, and Enter picks the
//! highlighted item, else the first.

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, ClickEvent, Context, ElementId, Entity, FocusHandle, Focusable, Hsla,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point,
    ScrollHandle, SharedString, Styled, Subscription, Window, actions, anchored, deferred, div,
    point, prelude::*, px, svg,
};
use theme::{
    CONTROL_RADIUS, DISABLED_OPACITY, HAIRLINE, ICON_SIZE, MENU_RADIUS, MENU_WIDTH, ROW_HEIGHT,
    SPACE_1, SPACE_2, SPACE_3, Theme,
};

use crate::{assets::Icon, text_input::TextInput};

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

/// A searching menu's field, over its hairline.
const SEARCH_HEIGHT: Pixels = px(36.);

/// A searching menu is wider than a plain one: its field needs room to type.
const SEARCH_MENU_WIDTH: Pixels = px(256.);

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
    /// Holds key focus while the menu is open, so its keys reach it: the
    /// search field's, when the menu has one.
    focus: FocusHandle,
    search: Option<Entity<TextInput>>,
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
    /// `None` leaves out the check's slot; `Some(false)` keeps it empty, so
    /// labels stay aligned.
    checked: Option<bool>,
    /// The check after the label rather than before it.
    check_trails: bool,
    /// A mark before the label, in its own color, such as an agent's logo.
    icon: Option<(Icon, Hsla)>,
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
            checked: None,
            check_trails: false,
            icon: None,
            shortcut: None,
            enabled: true,
            pick: Rc::new(pick),
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Checked at the row's end, where a list of places marks the current
    /// one; unchecked, the row keeps no slot.
    pub fn checked_at_end(mut self, checked: bool) -> Self {
        self.checked = checked.then_some(true);
        self.check_trails = true;
        self
    }

    pub fn icon(mut self, icon: Icon, color: Hsla) -> Self {
        self.icon = Some((icon, color));
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
            search: None,
            give_back: None,
            highlighted: None,
            scroll: ScrollHandle::new(),
            _focus_out: None,
        }
    }

    /// A menu that leads with `search`, which takes the focus while it is
    /// open; each opening starts it empty.
    pub fn with_search(
        access: fn(&mut T) -> &mut Menu<T, K>,
        search: Entity<TextInput>,
        cx: &mut App,
    ) -> Self {
        Self {
            focus: search.focus_handle(cx),
            search: Some(search),
            ..Self::new(access, cx)
        }
    }

    /// What the search field holds; empty for a menu without one.
    pub fn query(&self, cx: &App) -> String {
        (self.search.as_ref()).map_or_else(String::new, |search| search.read(cx).text().to_owned())
    }

    /// The entries changed under a new query: the highlight starts over.
    pub fn query_changed(&mut self, cx: &mut Context<T>) {
        self.highlighted = None;
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
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
        if let Some(search) = self.search.clone().filter(|_| self.open.is_none()) {
            search.update(cx, |search, cx| search.set_text("", cx));
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
            .when(self.search.is_none(), |panel| {
                panel.track_focus(&self.focus)
            })
            .on_action(cx.listener(move |owner, _: &Dismiss, window, cx| close(owner, window, cx)))
            .on_action(cx.listener({
                let items = items.clone();
                move |owner, _: &SelectNext, _, cx| access(owner).step(&items, true, cx)
            }))
            .on_action(cx.listener(move |owner, _: &SelectPrevious, _, cx| {
                access(owner).step(&items, false, cx)
            }))
            .on_action(cx.listener(move |owner, _: &Confirm, window, cx| {
                let menu = access(owner);
                let highlighted = match (menu.highlighted, &menu.search) {
                    (None, Some(_)) => picks.first().map(|(at, _)| *at),
                    (highlighted, _) => highlighted,
                };
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
            .min_w(if self.search.is_some() {
                SEARCH_MENU_WIDTH
            } else {
                MENU_WIDTH
            })
            .max_w(MENU_MAX_WIDTH)
            .flex()
            .flex_col()
            .rounded(MENU_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.popover_background)
            .shadow_lg();
        let list = div()
            .id((id, 2u64))
            .max_h(MENU_MAX_HEIGHT)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(SPACE_1 - HAIRLINE)
            .flex()
            .flex_col()
            .children(rows);
        let panel = match &self.search {
            None => panel.child(list),
            // The field's text starts in the items' icon column: `SPACE_1` to
            // the row, `SPACE_2` inside it.
            Some(search) => panel
                .child(
                    div()
                        .h(SEARCH_HEIGHT)
                        .flex_none()
                        .px(SPACE_1 + SPACE_2 - HAIRLINE)
                        .flex()
                        .items_center()
                        .child(div().flex_1().min_w_0().child(search.clone())),
                )
                .child(div().flex_none().h(HAIRLINE).bg(colors.border))
                .child(list),
        };
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
            // A searching menu's hairlines run edge to edge, as the one under
            // its field does, so its groups read as bands.
            Entry::Separator => {
                return div()
                    .flex_none()
                    .my(SPACE_1)
                    .when(self.search.is_some(), |line| line.mx(-(SPACE_1 - HAIRLINE)))
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
            .when_some(
                item.checked.filter(|_| !item.check_trails),
                |row, checked| {
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
                },
            )
            .children(item.icon.map(|(icon, color)| {
                svg()
                    .path(icon.path())
                    .size(ICON_SIZE)
                    .flex_none()
                    .mr(SPACE_2)
                    .text_color(color)
            }))
            .child(
                div()
                    .flex_auto()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_color(colors.text_primary)
                    .child(item.label),
            )
            .when(item.check_trails && item.checked == Some(true), |row| {
                row.child(
                    svg()
                        .path(Icon::Check.path())
                        .flex_none()
                        .ml(SPACE_3)
                        .size(ICON_SIZE)
                        .text_color(colors.text_primary),
                )
            })
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
