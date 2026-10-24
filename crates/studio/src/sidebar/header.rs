//! The header: New session and the pages, then the sessions' title with
//! search and the list's menu; while searching, the field takes the title's
//! place.

use gpui::{
    App, ClickEvent, Context, Div, Focusable, FontWeight, IntoElement, ParentElement, Pixels,
    SharedString, Stateful, Styled, Window, div, prelude::*, svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, Colors, DISABLED_OPACITY, FIELD_TARGET_SIZE, ICON_SIZE,
    ICON_SIZE_ROW, ICON_SIZE_SMALL, SPACE_2, SPACE_3, TEXT_SMALL, Theme,
};

use super::Sidebar;
use crate::{
    assets::Icon,
    kbd::Shortcut,
    tooltip::{TooltipExt, WithTooltip},
};

/// A square icon-only control with a tooltip naming it. Chain listeners on
/// `build`; the tooltip wraps the result.
pub(crate) fn icon_button(
    id: impl Into<SharedString>,
    icon: Icon,
    label: impl Into<SharedString>,
    enabled: bool,
    colors: Colors,
    build: impl FnOnce(Stateful<Div>) -> Stateful<Div>,
) -> WithTooltip {
    icon_button_sized(id, icon, ICON_SIZE, label, enabled, colors, build)
}

/// An icon button in the sidebar's section titles, its icon `ICON_SIZE_ROW`
/// like the rows'.
pub(super) fn row_button(
    id: impl Into<SharedString>,
    icon: Icon,
    label: impl Into<SharedString>,
    colors: Colors,
    build: impl FnOnce(Stateful<Div>) -> Stateful<Div>,
) -> WithTooltip {
    icon_button_sized(id, icon, ICON_SIZE_ROW, label, true, colors, build)
}

fn icon_button_sized(
    id: impl Into<SharedString>,
    icon: Icon,
    icon_size: Pixels,
    label: impl Into<SharedString>,
    enabled: bool,
    colors: Colors,
    build: impl FnOnce(Stateful<Div>) -> Stateful<Div>,
) -> WithTooltip {
    let id: SharedString = id.into();
    let button = div()
        .id(id.clone())
        .group(id.clone())
        .size(CONTROL_SIZE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(CONTROL_RADIUS)
        .when(enabled, |button| {
            button.hover(|button| button.bg(colors.hover))
        })
        .when(!enabled, |button| button.opacity(DISABLED_OPACITY))
        .child(
            // An svg takes no color from its parent, so the hover color is its own.
            svg()
                .path(icon.path())
                .size(icon_size)
                .text_color(colors.text_secondary)
                .when(enabled, |icon| {
                    icon.group_hover(id, |icon| icon.text_color(colors.text_primary))
                }),
        );
    build(button).with_tooltip(label)
}

/// A close button inside a field or a message: 16 pt visible, taking clicks over
/// WCAG's 24 pt minimum, the negative margin keeping it where the padding puts
/// it.
pub(super) fn close_button(
    id: &'static str,
    label: &'static str,
    colors: Colors,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> WithTooltip {
    div()
        .id(id)
        .group(id)
        .flex_none()
        .size(FIELD_TARGET_SIZE)
        .m(-(FIELD_TARGET_SIZE - ICON_SIZE) / 2.)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(on_click)
        .child(
            div()
                .size(ICON_SIZE)
                .flex()
                .items_center()
                .justify_center()
                .rounded(CONTROL_RADIUS)
                .group_hover(id, |button| button.bg(colors.hover))
                .child(
                    svg()
                        .path(Icon::Close.path())
                        .size(ICON_SIZE_SMALL)
                        .text_color(colors.text_secondary)
                        .group_hover(id, |icon| icon.text_color(colors.text_primary)),
                ),
        )
        .with_tooltip(label)
}

impl Sidebar {
    /// New session and the pages, then the sessions' title with search and
    /// the list's menu, then the search field while it is open.
    pub(super) fn render_header(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        // While searching, the field takes the title's place and Search turns
        // into Close search.
        let lead = if self.search_open {
            self.render_search_field(cx).into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .px(SPACE_2)
                .text_size(TEXT_SMALL)
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors.text_tertiary)
                .child("Sessions")
                .into_any_element()
        };
        let toggle = if self.search_open {
            row_button(
                "close-search",
                Icon::Close,
                "Close search",
                colors,
                |button| {
                    button.on_click(cx.listener(|sidebar, _: &ClickEvent, window, cx| {
                        sidebar.close_search(window, cx)
                    }))
                },
            )
        } else {
            row_button(
                "search",
                Icon::Search,
                "Search sessions",
                colors,
                |button| {
                    button.on_click(cx.listener(|sidebar, _: &ClickEvent, window, cx| {
                        sidebar.focus_search(window, cx)
                    }))
                },
            )
            .shortcut(Shortcut::key("K").cmd())
        };
        let title = div()
            .mx(SPACE_2)
            .mt(SPACE_3)
            .h(CONTROL_SIZE)
            .flex()
            .items_center()
            .child(lead)
            .child(toggle)
            .child(self.render_menu_button(window, cx));
        div()
            .flex()
            .flex_col()
            .child(self.render_nav(window, cx))
            .child(title)
    }

    /// The search field in the title's place: no box of its own, only the
    /// magnifier and the text in the title's size and weight.
    fn render_search_field(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let input_focus = self.search.focus_handle(cx);
        div()
            .id("search-field")
            .key_context("SidebarSearch")
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_highlighted))
            .on_action(cx.listener(Self::dismiss))
            .flex_1()
            .min_w_0()
            .h(CONTROL_SIZE)
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .text_size(TEXT_SMALL)
            .font_weight(FontWeight::MEDIUM)
            .cursor_text()
            // The magnifier and padding are part of the field: a click there
            // focuses it.
            .on_click(
                cx.listener(move |_, _: &ClickEvent, window, cx| window.focus(&input_focus, cx)),
            )
            .child(
                svg()
                    .path(Icon::Search.path())
                    .size(ICON_SIZE_ROW)
                    .flex_none()
                    .text_color(colors.text_tertiary),
            )
            .child(div().flex_1().min_w_0().child(self.search.clone()))
    }
}
