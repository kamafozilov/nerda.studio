//! What a menu lists, and how one row of it draws: a section's label, a
//! separator, or an item, which picks or opens a submenu beside the menu.

use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, ElementId, Hsla, IntoElement, MouseMoveEvent, ParentElement,
    SharedString, Styled, Window, div, prelude::*, svg,
};
use theme::{
    CONTROL_RADIUS, DISABLED_OPACITY, HAIRLINE, ICON_SIZE_ROW, ROW_HEIGHT, SPACE_1, SPACE_2,
    SPACE_3, Theme,
};

use super::{Level, Menu};
use crate::{
    assets::Icon,
    kbd::{Kbd, Shortcut},
};

/// What a pick does in the menu's owner.
pub(super) type Pick<T> = Rc<dyn Fn(&mut T, &mut Window, &mut Context<T>)>;

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
    shortcut: Option<Shortcut>,
    /// A short note at the row's end in secondary text: an age, a place.
    detail: Option<SharedString>,
    /// The label in secondary text: a choice that stands back, such as a
    /// remote branch under the local ones.
    quiet: bool,
    pub(super) enabled: bool,
    pub(super) pick: Pick<T>,
    /// Items that open beside this one, with what is chosen among them now.
    pub(super) submenu: Option<(SharedString, Vec<Entry<T>>)>,
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
            detail: None,
            quiet: false,
            enabled: true,
            pick: Rc::new(pick),
            submenu: None,
        }
    }

    /// An item that opens `entries` beside the menu, under the pointer, on a
    /// click, Enter or the right arrow; it shows `value`, what is chosen there.
    pub fn submenu(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        entries: Vec<Entry<T>>,
    ) -> Self {
        Self {
            submenu: Some((value.into(), entries)),
            ..Self::new(id, label, |_, _, _| {})
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

    pub fn shortcut(mut self, keys: impl Into<Option<Shortcut>>) -> Self {
        self.shortcut = keys.into();
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl<T: 'static, K: 'static> Menu<T, K> {
    /// Row `at` of the menu at `level`; an item whose submenu is open draws
    /// the submenu beside it.
    pub(super) fn render_entry(
        &self,
        level: Level,
        at: usize,
        entry: Entry<T>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> AnyElement {
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
            // Hairlines run edge to edge, so the groups read as bands.
            Entry::Separator => {
                return div()
                    .flex_none()
                    .my(SPACE_1)
                    .mx(-(SPACE_1 - HAIRLINE))
                    .h(HAIRLINE)
                    .bg(colors.border)
                    .into_any_element();
            }
            Entry::Item(item) => item,
        };
        let access = self.access;
        let open_here = level == Level::Root && self.sub == Some(at);
        let highlighted = match level {
            Level::Root => self.highlighted == Some(at) || open_here,
            Level::Sub => self.sub_highlighted == Some(at),
        };
        // Drawn because of state, the highlight redraws the row, so its
        // supporting text keeps its contrast on the surface.
        let supporting = if highlighted {
            colors.text_secondary_on_surface
        } else {
            colors.text_secondary
        };
        let enabled = item.enabled;
        let has_submenu = item.submenu.is_some();
        let pick = item.pick;
        let (value, submenu) = item.submenu.unzip();
        let check = |checked: bool| {
            div().size(ICON_SIZE_ROW).flex_none().when(checked, |slot| {
                slot.child(
                    svg()
                        .path(Icon::Check.path())
                        .size(ICON_SIZE_ROW)
                        .text_color(colors.text_primary),
                )
            })
        };
        div()
            .id(item.id)
            .relative()
            .min_h(ROW_HEIGHT)
            .flex_none()
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .when(highlighted, |row| row.bg(colors.hover))
            .on_mouse_move(cx.listener(move |owner, _: &MouseMoveEvent, _, cx| {
                access(owner).point_at(level, at, enabled, has_submenu, cx)
            }))
            .when(enabled, |row| {
                row.cursor_pointer().on_click(cx.listener(
                    move |owner, _: &ClickEvent, window, cx| {
                        if has_submenu {
                            access(owner).open_submenu(at, cx);
                        } else {
                            pick(owner, window, cx);
                            access(owner).close(window, cx);
                        }
                    },
                ))
            })
            .when(!enabled, |row| row.opacity(DISABLED_OPACITY))
            .when_some(
                item.checked.filter(|_| !item.check_trails),
                |row, checked| row.child(check(checked)),
            )
            .children(item.icon.map(|(icon, color)| {
                svg()
                    .path(icon.path())
                    .size(ICON_SIZE_ROW)
                    .flex_none()
                    .text_color(color)
            }))
            .child(
                div()
                    .flex_auto()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_color(if item.quiet {
                        supporting
                    } else {
                        colors.text_primary
                    })
                    .child(item.label),
            )
            .children(
                item.detail
                    .filter(|detail| !detail.is_empty())
                    .map(|detail| {
                        div()
                            .flex_none()
                            .max_w(SPACE_3 * 10.)
                            .pl(SPACE_1)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(supporting)
                            .child(detail)
                    }),
            )
            .when(item.check_trails && item.checked == Some(true), |row| {
                row.child(check(true).ml(SPACE_1))
            })
            .children(item.shortcut.map(|keys| {
                div()
                    .pl(SPACE_1)
                    .child(Kbd::new(keys).small().color(supporting))
            }))
            .children(value.map(|value| {
                div()
                    .flex_none()
                    .max_w(SPACE_3 * 10.)
                    .pl(SPACE_1)
                    .flex()
                    .items_center()
                    .gap(SPACE_2)
                    .text_color(supporting)
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(value),
                    )
                    .child(
                        svg()
                            .path(Icon::Forward.path())
                            .size(ICON_SIZE_ROW)
                            .flex_none()
                            .text_color(colors.text_tertiary),
                    )
            }))
            .children(
                submenu
                    .filter(|_| open_here)
                    .map(|entries| self.render_submenu(entries, window, cx)),
            )
            .into_any_element()
    }
}
