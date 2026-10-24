//! A menu: a list of choices on `popover_background` that hangs from a control
//! or opens at a point, over a window-wide backdrop whose click closes it.
//!
//! Arrows move the highlight, which the pointer moves too; Enter or a click
//! picks, and Escape closes. Closing gives focus back, unless focus left the
//! menu first, which closes it too. The owner keeps a [`Menu`] and draws it
//! where the menu hangs from; a pick runs in the owner, then the menu closes.
//! An item may open a submenu beside the menu (`submenu`).
//!
//! A menu made [`Menu::with_search`] leads with a search field, which holds
//! the focus: typing filters, as the owner's entries say, and Enter picks the
//! highlighted item, else the first.

mod entry;
mod submenu;

pub(crate) use entry::{Entry, Item};

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, Context, Div, Entity, FocusHandle, Focusable, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Pixels, Point, ScrollHandle, Styled, Subscription,
    Window, actions, anchored, deferred, div, point, prelude::*, px,
};
use theme::{HAIRLINE, MENU_RADIUS, MENU_WIDTH, SPACE_1, SPACE_2, Theme};

use self::{entry::Pick, submenu::Moves};
use crate::text_input::TextInput;

actions!(
    menu,
    [
        /// Closes the open menu.
        Dismiss,
        /// Moves the highlight down, wrapping at the end.
        SelectNext,
        /// Moves the highlight up, wrapping at the start.
        SelectPrevious,
        /// Picks the highlighted item, or opens its submenu.
        Confirm,
        /// Opens the highlighted item's submenu.
        OpenSubmenu,
        /// Closes the submenu, back to the item that opened it.
        CloseSubmenu,
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
        gpui::KeyBinding::new("right", OpenSubmenu, Some(CONTEXT)),
        gpui::KeyBinding::new("left", CloseSubmenu, Some(CONTEXT)),
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
    /// The item whose submenu is open beside the menu.
    sub: Option<usize>,
    /// The submenu's entry under the pointer or the keyboard's highlight;
    /// while there is one, the arrows and Enter act in the submenu.
    sub_highlighted: Option<usize>,
    scroll: ScrollHandle,
    _focus_out: Option<Subscription>,
}

/// The menu itself, or the submenu open beside it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Root,
    Sub,
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

impl<T: 'static, K: 'static> Menu<T, K> {
    pub fn new(access: fn(&mut T) -> &mut Menu<T, K>, cx: &mut App) -> Self {
        Self {
            access,
            open: None,
            focus: cx.focus_handle(),
            search: None,
            give_back: None,
            highlighted: None,
            sub: None,
            sub_highlighted: None,
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
        self.close_submenu();
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
        self.close_submenu();
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

    /// The menu's panel: opaque, with a hairline and a large shadow.
    fn panel(cx: &App) -> Div {
        let colors = Theme::of(cx).colors;
        div()
            .occlude()
            .max_w(MENU_MAX_WIDTH)
            .flex()
            .flex_col()
            .rounded(MENU_RADIUS)
            .border_1()
            .border_color(colors.popover_border)
            .bg(colors.popover_background)
            .shadow(colors.popover_shadows())
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
        let moves = Rc::new(Moves::of(&entries, self.sub));
        let rows: Vec<AnyElement> = (entries.into_iter().enumerate())
            .map(|(at, entry)| self.render_entry(Level::Root, at, entry, window, cx))
            .collect();
        let panel = Self::panel(cx)
            .id(id)
            .key_context(CONTEXT)
            .when(self.search.is_none(), |panel| {
                panel.track_focus(&self.focus)
            })
            .on_action(cx.listener(move |owner, _: &Dismiss, window, cx| close(owner, window, cx)))
            .on_action(cx.listener({
                let moves = moves.clone();
                move |owner, _: &SelectNext, _, cx| access(owner).step(&moves, true, cx)
            }))
            .on_action(cx.listener({
                let moves = moves.clone();
                move |owner, _: &SelectPrevious, _, cx| access(owner).step(&moves, false, cx)
            }))
            .on_action(cx.listener({
                let moves = moves.clone();
                move |owner, _: &OpenSubmenu, _, cx| {
                    let menu = access(owner);
                    if let Some(at) = menu.highlighted.filter(|_| menu.sub_highlighted.is_none()) {
                        menu.open_submenu_with_keys(at, &moves, cx);
                    }
                }
            }))
            .on_action(cx.listener(move |owner, _: &CloseSubmenu, _, cx| {
                let menu = access(owner);
                if menu.sub_highlighted.is_some() {
                    menu.highlighted = menu.sub;
                    menu.close_submenu();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(move |owner, _: &Confirm, window, cx| {
                let menu = access(owner);
                let pick: Option<Pick<T>> = match menu.sub_highlighted {
                    Some(at) => moves.sub_pick(at),
                    None => {
                        let highlighted = match (menu.highlighted, &menu.search) {
                            (None, Some(_)) => moves.items.first().copied(),
                            (highlighted, _) => highlighted,
                        };
                        let Some(at) = highlighted else { return };
                        if moves.has_submenu(at) {
                            menu.open_submenu_with_keys(at, &moves, cx);
                            return;
                        }
                        moves.pick(at)
                    }
                };
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
            .min_w(if self.search.is_some() {
                SEARCH_MENU_WIDTH
            } else {
                MENU_WIDTH
            });
        let list = div()
            .id((id, 2u64))
            .max_h(MENU_MAX_HEIGHT)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(SPACE_1 - HAIRLINE)
            .flex()
            .flex_col()
            .children(rows);
        let colors = Theme::of(cx).colors;
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
}
