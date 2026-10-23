//! The filter and sort menu: how sessions are ordered and which project they come
//! from.

use gpui::{
    Anchor, AnyElement, ClickEvent, Context, ElementId, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, Window, anchored, deferred, div, point, prelude::*, px, svg,
};
use theme::{
    CONTROL_RADIUS, HAIRLINE, ICON_SIZE, MENU_RADIUS, MENU_WIDTH, ROW_HEIGHT, SPACE_1, SPACE_2,
    Theme,
};
use workspace::{ProjectScope, SessionSort};

use super::{Sidebar, header::icon_button};
use crate::assets::Icon;

/// Tallest the menu grows before its project list scrolls.
const MENU_MAX_HEIGHT: gpui::Pixels = px(360.);

impl Sidebar {
    pub(super) fn render_menu_button(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        // A project filter stays marked on the button, so a list narrowed to one
        // project never passes for all of them.
        let filtered = self.store.read(cx).workspace().scope() != ProjectScope::All;
        div()
            .relative()
            .flex_none()
            .child(icon_button(
                "menu",
                Icon::Sort,
                "Sort and filter sessions",
                true,
                colors,
                |button| {
                    button
                        .when(self.menu_open || filtered, |button| button.bg(colors.hover))
                        .on_click(cx.listener(|sidebar, _: &ClickEvent, window, cx| {
                            sidebar.open_menu(window, cx)
                        }))
                },
            ))
            .when(self.menu_open, |anchor| {
                anchor
                    .child(self.render_backdrop(window, cx))
                    // A point at the button's bottom-right corner for the menu to hang from.
                    .child(
                        div().absolute().top_full().right_0().child(
                            deferred(
                                anchored()
                                    .anchor(Anchor::TopRight)
                                    .offset(point(px(0.), SPACE_1))
                                    .snap_to_window_with_margin(SPACE_2)
                                    .child(self.render_menu(cx)),
                            )
                            .priority(2),
                        ),
                    )
            })
    }

    /// Covers the window under the menu: a click anywhere else closes the menu and
    /// goes no further, so it can't also press what lies beneath (the menu button
    /// included, which would reopen it).
    fn render_backdrop(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("menu-backdrop")
                    .w(viewport.width)
                    .h(viewport.height)
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|sidebar, _, window, cx| sidebar.close_menu(window, cx)),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|sidebar, _, window, cx| sidebar.close_menu(window, cx)),
                    ),
            ),
        )
        .priority(1)
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let sort = workspace.sort();
        let scope = workspace.scope();
        let projects: Vec<_> = workspace
            .projects()
            .into_iter()
            .map(|project| {
                let name = workspace.display_name(project.id).unwrap_or_default();
                (project.id, SharedString::from(name))
            })
            .collect();
        div()
            .id("menu")
            .key_context("SidebarMenu")
            .track_focus(&self.menu_focus)
            .on_action(cx.listener(Self::dismiss))
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
            .child(self.section_label("Sort by", cx))
            .child(self.menu_item(
                "sort-activity",
                "Last activity",
                sort == SessionSort::LastActivity,
                cx,
                |workspace| workspace.set_sort(SessionSort::LastActivity),
            ))
            .child(self.menu_item(
                "sort-created",
                "Created",
                sort == SessionSort::Created,
                cx,
                |workspace| workspace.set_sort(SessionSort::Created),
            ))
            .child(div().my(SPACE_1).h(HAIRLINE).bg(colors.border))
            .child(self.section_label("Project", cx))
            .child(self.menu_item(
                "scope-all",
                "All projects",
                scope == ProjectScope::All,
                cx,
                |workspace| workspace.set_scope(ProjectScope::All),
            ))
            .children(projects.into_iter().map(|(project, title)| {
                self.menu_item(
                    ("scope", project.0),
                    title,
                    scope == ProjectScope::Project(project),
                    cx,
                    move |workspace| workspace.set_scope(ProjectScope::Project(project)),
                )
            }))
    }

    fn section_label(&self, text: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(ROW_HEIGHT)
            .px(SPACE_2)
            .flex()
            .items_center()
            .text_color(Theme::of(cx).colors.text_secondary)
            .child(text)
    }

    /// A choice in the menu: picking it applies it and closes the menu.
    fn menu_item(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        checked: bool,
        cx: &mut Context<Self>,
        choose: impl Fn(&mut workspace::Workspace) + 'static,
    ) -> AnyElement {
        let colors = Theme::of(cx).colors;
        div()
            .id(id)
            .h(ROW_HEIGHT)
            .flex_none()
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .hover(|item| item.bg(colors.hover))
            .on_click(cx.listener(move |sidebar, _: &ClickEvent, window, cx| {
                sidebar.store.update(cx, |store, cx| {
                    store.update(cx, |workspace| choose(workspace))
                });
                sidebar.close_menu(window, cx);
            }))
            // The check keeps its slot when absent, so labels stay aligned.
            .child(div().size(ICON_SIZE).flex_none().when(checked, |slot| {
                slot.child(
                    svg()
                        .path(Icon::Check.path())
                        .size(ICON_SIZE)
                        .text_color(colors.text_primary),
                )
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_color(colors.text_primary)
                    .child(label.into()),
            )
            .into_any_element()
    }
}
