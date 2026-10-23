//! A tab's menu, opened by a right click on it: close it, the others, those to
//! either side, or all of them.

use gpui::{
    AnyElement, ClickEvent, Context, FocusHandle, IntoElement, MouseButton, ParentElement, Pixels,
    Point, Styled, Window, anchored, deferred, div, point, prelude::*, px,
};
use theme::{CONTROL_RADIUS, MENU_WIDTH, ROW_HEIGHT, SPACE_1, SPACE_2, SPACE_3, Theme};
use workspace::{Tab, Timestamp, Workspace};

use super::{DismissMenu, TabStrip};

/// Opacity of a choice that can't apply here, as for disabled controls.
const DISABLED_OPACITY: f32 = 0.4;

const MAC: bool = cfg!(target_os = "macos");

pub(super) struct TabMenu {
    pub tab: Tab,
    position: Point<Pixels>,
    /// Where focus was before the menu opened, to give it back.
    focus_before: Option<FocusHandle>,
}

/// A choice in the menu and what it does to the tab it opened on.
struct Choice {
    id: &'static str,
    label: &'static str,
    shortcut: Option<&'static str>,
    enabled: bool,
    run: fn(&mut Workspace, Tab, Timestamp),
}

impl TabStrip {
    pub(super) fn open_menu(
        &mut self,
        tab: Tab,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus_before = window.focused(cx);
        self.menu = Some(TabMenu {
            tab,
            position,
            focus_before,
        });
        window.focus(&self.menu_focus, cx);
        cx.notify();
    }

    pub(super) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.menu.take()
            && let Some(focus) = menu.focus_before
        {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    pub(super) fn render_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let tabs = workspace.open_tabs();
        let index = tabs.iter().position(|&tab| tab == menu.tab)?;
        // A lone draft can't close; with company, every tab can.
        let closable = workspace.can_close_tab(menu.tab);
        let choices = [
            Choice {
                id: "close",
                label: "Close Tab",
                shortcut: Some(if MAC { "⌘W" } else { "Ctrl+W" }),
                enabled: closable,
                run: Workspace::close_tab,
            },
            Choice {
                id: "close-others",
                label: "Close Other Tabs",
                shortcut: Some(if MAC { "⌥⌘T" } else { "Ctrl+Alt+T" }),
                enabled: tabs.len() > 1,
                run: Workspace::close_other_tabs,
            },
            Choice {
                id: "close-right",
                label: "Close Tabs to the Right",
                shortcut: None,
                enabled: index + 1 < tabs.len(),
                run: Workspace::close_tabs_after,
            },
            Choice {
                id: "close-left",
                label: "Close Tabs to the Left",
                shortcut: None,
                enabled: index > 0,
                run: Workspace::close_tabs_before,
            },
            Choice {
                id: "close-all",
                label: "Close All Tabs",
                shortcut: Some(if MAC { "⇧⌘W" } else { "Ctrl+Shift+W" }),
                enabled: closable,
                run: |workspace, _, now| workspace.close_all_tabs(now),
            },
        ];
        let tab = menu.tab;
        let items = choices.map(|choice| {
            let Choice {
                id,
                label,
                shortcut,
                enabled,
                run,
            } = choice;
            div()
                .id(id)
                .h(ROW_HEIGHT)
                .flex_none()
                .px(SPACE_2)
                .flex()
                .items_center()
                .gap(SPACE_3)
                .rounded(CONTROL_RADIUS)
                .when(enabled, |item| {
                    item.hover(|item| item.bg(colors.hover))
                        .on_click(cx.listener(move |strip, _: &ClickEvent, window, cx| {
                            strip.store.update(cx, |store, cx| {
                                store.update(cx, |workspace| run(workspace, tab, Timestamp::now()))
                            });
                            strip.close_menu(window, cx);
                        }))
                })
                .when(!enabled, |item| item.opacity(DISABLED_OPACITY))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_color(colors.text_primary)
                        .child(label),
                )
                .children(shortcut.map(|keys| {
                    div()
                        .flex_none()
                        .text_color(colors.text_secondary)
                        .child(keys)
                }))
        });
        let viewport = window.viewport_size();
        Some(
            div()
                .child(
                    // Under the menu, a click anywhere else closes it and goes no further.
                    deferred(
                        anchored().position(point(px(0.), px(0.))).child(
                            div()
                                .id("tab-menu-backdrop")
                                .w(viewport.width)
                                .h(viewport.height)
                                .occlude()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|strip, _, window, cx| {
                                        strip.close_menu(window, cx)
                                    }),
                                )
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(|strip, _, window, cx| {
                                        strip.close_menu(window, cx)
                                    }),
                                ),
                        ),
                    )
                    .priority(1),
                )
                .child(
                    deferred(
                        anchored()
                            .position(menu.position)
                            .offset(point(px(0.), SPACE_1))
                            .snap_to_window_with_margin(SPACE_2)
                            .child(
                                div()
                                    .id("tab-menu")
                                    .key_context("TabMenu")
                                    .track_focus(&self.menu_focus)
                                    .on_action(cx.listener(|strip, _: &DismissMenu, window, cx| {
                                        strip.close_menu(window, cx)
                                    }))
                                    .occlude()
                                    .w(MENU_WIDTH)
                                    .p(SPACE_1)
                                    .flex()
                                    .flex_col()
                                    .rounded(CONTROL_RADIUS)
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.popover_background)
                                    .shadow_lg()
                                    .children(items),
                            ),
                    )
                    .priority(2),
                )
                .into_any_element(),
        )
    }
}
