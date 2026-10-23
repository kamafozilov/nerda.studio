//! The header: the pages, then the search field on its own row, then the
//! list's title, filter and sort, add project, new session.

use gpui::{
    ClickEvent, Context, Div, Focusable, FontWeight, IntoElement, ParentElement, SharedString,
    Stateful, Styled, Window, div, prelude::*, svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, Colors, FIELD_TARGET_SIZE, ICON_SIZE, ICON_SIZE_SMALL, SPACE_1,
    SPACE_2, Theme,
};
use workspace::ProjectScope;

use super::Sidebar;
use crate::{
    assets::Icon,
    tooltip::{TooltipExt, WithTooltip},
};

/// Opacity of a control that can't be used yet; disabled controls are exempt
/// from contrast minimums.
const DISABLED_OPACITY: f32 = 0.4;

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
                .size(ICON_SIZE)
                .text_color(colors.text_secondary)
                .when(enabled, |icon| {
                    icon.group_hover(id, |icon| icon.text_color(colors.text_primary))
                }),
        );
    build(button).with_tooltip(label)
}

impl Sidebar {
    pub(super) fn render_header(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let workspace = self.store.read(cx).workspace();
        let has_projects = workspace.has_projects();
        // Filtered to one project, the button starts the session there, and says so.
        let new_session_label: SharedString = match workspace.scope() {
            _ if !has_projects => "New session: add a project first".into(),
            ProjectScope::Project(project) => match workspace.display_name(project) {
                Some(name) => format!("New session in {name}").into(),
                None => "New session".into(),
            },
            ProjectScope::All if cfg!(target_os = "macos") => "New session (⌘N)".into(),
            ProjectScope::All => "New session (Ctrl+N)".into(),
        };
        // The pages first; then search spans the sidebar on its own row; under it,
        // the list's title at the leading edge and the actions on it at the
        // trailing edge.
        div()
            .mx(SPACE_2)
            .flex()
            .flex_col()
            .gap(SPACE_2)
            .child(self.render_pages(cx))
            .child(self.render_search_field(window, cx))
            .child(
                div()
                    .h(CONTROL_SIZE)
                    .flex()
                    .items_center()
                    .gap(SPACE_1)
                    // The title and the filter show only once there is something to
                    // list or filter.
                    .when(has_projects, |row| {
                        row.child(self.render_title(cx))
                            .child(div().flex_1())
                            .child(self.render_menu_button(window, cx))
                    })
                    .when(!has_projects, |row| row.justify_end())
                    .child(self.render_action_buttons(has_projects, new_session_label, cx)),
            )
    }

    /// Names the list; its text lines up with the sessions' titles below.
    fn render_title(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_none()
            .pl(SPACE_2)
            .font_weight(FontWeight::MEDIUM)
            .text_color(Theme::of(cx).colors.text_primary)
            .child("Sessions")
    }

    fn render_action_buttons(
        &self,
        has_projects: bool,
        new_session_label: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        div()
            .flex()
            .gap(SPACE_1)
            .child(icon_button(
                "add-project",
                Icon::FolderAdd,
                "Add project",
                true,
                colors,
                |button| {
                    button.on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(crate::shell::AddProject), cx)
                    })
                },
            ))
            .child(icon_button(
                "new-session",
                Icon::NewSession,
                new_session_label,
                has_projects,
                colors,
                |button| {
                    button.when(has_projects, |button| {
                        button.on_click(cx.listener(|sidebar, _: &ClickEvent, _, cx| {
                            sidebar.new_session_in_view(cx)
                        }))
                    })
                },
            ))
    }

    fn render_search_field(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let input_focus = self.search.focus_handle(cx);
        let focused = input_focus.contains_focused(window, cx);
        div()
            .id("search-field")
            .key_context("SidebarSearch")
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_highlighted))
            .on_action(cx.listener(Self::dismiss))
            .w_full()
            .h(CONTROL_SIZE)
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .border_1()
            .border_color(if focused {
                colors.focus_ring
            } else {
                colors.border
            })
            .bg(colors.field_background)
            .cursor_text()
            // The icon and padding are part of the field: a click there focuses it.
            .on_click(
                cx.listener(move |_, _: &ClickEvent, window, cx| window.focus(&input_focus, cx)),
            )
            .child(
                svg()
                    .path(Icon::Search.path())
                    .size(ICON_SIZE)
                    .flex_none()
                    .text_color(colors.text_secondary),
            )
            .child(div().flex_1().min_w_0().child(self.search.clone()))
            .when(!self.query.is_empty(), |field| {
                field.child(
                    // The target is WCAG's 24 pt minimum around a 16 pt visible button;
                    // the negative margin keeps the button where the field's padding puts it.
                    div()
                        .id("clear-search")
                        .group("clear-search")
                        .flex_none()
                        .size(FIELD_TARGET_SIZE)
                        .mx(-(FIELD_TARGET_SIZE - ICON_SIZE) / 2.)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        // Clearing keeps the field focused, so typing goes on.
                        .on_click(cx.listener(|sidebar, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            sidebar.clear_search(cx);
                            window.focus(&sidebar.search.focus_handle(cx), cx);
                        }))
                        .child(
                            div()
                                .size(ICON_SIZE)
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(CONTROL_RADIUS)
                                .group_hover("clear-search", |button| button.bg(colors.hover))
                                .child(
                                    svg()
                                        .path(Icon::Close.path())
                                        .size(ICON_SIZE_SMALL)
                                        .text_color(colors.text_secondary)
                                        .group_hover("clear-search", |icon| {
                                            icon.text_color(colors.text_primary)
                                        }),
                                ),
                        )
                        .with_tooltip("Clear search"),
                )
            })
    }
}
