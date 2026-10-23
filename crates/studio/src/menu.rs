//! A menu: a list of choices on `popover_background` that hangs from a control
//! or opens at a point, over a window-wide backdrop whose click closes it.
//! Escape closes it too, and closing gives focus back.
//!
//! The owner keeps a [`Menu`] and draws it where the menu hangs from; a pick
//! runs in the owner, then the menu closes.

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, ClickEvent, Context, ElementId, FocusHandle, IntoElement, MouseButton,
    ParentElement, Pixels, Point, SharedString, Styled, Window, actions, anchored, deferred, div,
    point, prelude::*, px, svg,
};
use theme::{
    CONTROL_RADIUS, DISABLED_OPACITY, HAIRLINE, ICON_SIZE, MENU_RADIUS, MENU_WIDTH, ROW_HEIGHT,
    SPACE_1, SPACE_2, SPACE_3, Theme,
};

use crate::assets::Icon;

actions!(
    menu,
    [
        /// Closes the open menu.
        Dismiss,
    ]
);

const CONTEXT: &str = "Menu";

/// Tallest a menu grows before it scrolls.
const MENU_MAX_HEIGHT: Pixels = px(360.);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([gpui::KeyBinding::new("escape", Dismiss, Some(CONTEXT))]);
}

/// A menu's state in its owner `T`: whether it is open, and on what (`K`).
pub(crate) struct Menu<T, K> {
    /// Finds this menu in its owner, for the menu's own listeners.
    access: fn(&mut T) -> &mut Menu<T, K>,
    open: Option<K>,
    /// Holds key focus while the menu is open, so Escape reaches it.
    focus: FocusHandle,
    /// Where focus goes when the menu closes.
    give_back: Option<FocusHandle>,
}

/// Where a menu opens.
#[derive(Clone, Copy)]
pub(crate) enum Placement {
    /// From where the owner draws it, `SPACE_1` clear: below for a top anchor.
    Hang(Anchor),
    /// Below a point in the window, as under a right click.
    At(Point<Pixels>),
}

/// What a pick does in the menu's owner.
type Pick<T> = Rc<dyn Fn(&mut T, &mut Window, &mut Context<T>)>;

pub(crate) enum Entry<T> {
    /// A label over the items after it.
    Section(&'static str),
    Separator,
    Item(Item<T>),
}

pub(crate) struct Item<T> {
    id: ElementId,
    label: SharedString,
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
            checked: None,
            shortcut: None,
            enabled: true,
            pick: Rc::new(pick),
        }
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
        }
    }

    /// What the menu is open on, while it is.
    pub fn opened(&self) -> Option<&K> {
        self.open.as_ref()
    }

    /// Opens the menu on `key` and takes key focus, to give it back on close.
    pub fn open(&mut self, key: K, window: &mut Window, cx: &mut Context<T>) {
        if self.open.is_none() {
            self.give_back = window.focused(cx);
        }
        self.open = Some(key);
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
        let colors = Theme::of(cx).colors;
        let close = move |owner: &mut T, window: &mut Window, cx: &mut Context<T>| {
            access(owner).close(window, cx)
        };
        let rows = entries.into_iter().map(|entry| match entry {
            Entry::Section(text) => div()
                .h(ROW_HEIGHT)
                .flex_none()
                .px(SPACE_2)
                .flex()
                .items_center()
                .text_color(colors.text_secondary)
                .child(text)
                .into_any_element(),
            Entry::Separator => div()
                .flex_none()
                .my(SPACE_1)
                .h(HAIRLINE)
                .bg(colors.border)
                .into_any_element(),
            Entry::Item(item) => {
                let pick = item.pick.clone();
                div()
                    .id(item.id)
                    .h(ROW_HEIGHT)
                    .flex_none()
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .rounded(CONTROL_RADIUS)
                    .when(item.enabled, |row| {
                        row.hover(|row| row.bg(colors.hover)).on_click(cx.listener(
                            move |owner, _: &ClickEvent, window, cx| {
                                pick(owner, window, cx);
                                close(owner, window, cx);
                            },
                        ))
                    })
                    .when(!item.enabled, |row| row.opacity(DISABLED_OPACITY))
                    .when_some(item.checked, |row, checked| {
                        row.child(div().size(ICON_SIZE).flex_none().mr(SPACE_2).when(
                            checked,
                            |slot| {
                                slot.child(
                                    svg()
                                        .path(Icon::Check.path())
                                        .size(ICON_SIZE)
                                        .text_color(colors.text_primary),
                                )
                            },
                        ))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(colors.text_primary)
                            .child(item.label),
                    )
                    .children(item.shortcut.map(|keys| {
                        div()
                            .flex_none()
                            .pl(SPACE_3)
                            .text_color(colors.text_secondary)
                            .child(keys)
                    }))
                    .into_any_element()
            }
        });
        let panel = div()
            .id(id)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(move |owner, _: &Dismiss, window, cx| close(owner, window, cx)))
            .occlude()
            .w(MENU_WIDTH)
            .max_h(MENU_MAX_HEIGHT)
            .overflow_y_scroll()
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
        let viewport = window.viewport_size();
        Some(
            div()
                .child(
                    // Under the menu, a click anywhere else closes it and goes no
                    // further, so it can't also press what lies beneath (the
                    // control that opened it included, which would reopen it).
                    deferred(
                        anchored().position(point(px(0.), px(0.))).child(
                            div()
                                .id((id, 1u64))
                                .w(viewport.width)
                                .h(viewport.height)
                                .occlude()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |owner, _, window, cx| {
                                        close(owner, window, cx)
                                    }),
                                )
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |owner, _, window, cx| {
                                        close(owner, window, cx)
                                    }),
                                ),
                        ),
                    )
                    .priority(1),
                )
                .child(deferred(hung.snap_to_window_with_margin(SPACE_2).child(panel)).priority(2))
                .into_any_element(),
        )
    }
}
