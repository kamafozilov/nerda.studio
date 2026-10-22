//! The window's top row, on the glass across the whole window: the traffic
//! lights, the sidebar toggle, Back and Forward at the open sidebar's edge, then
//! the tabs over the card. Its empty space stands in for the macOS titlebar.

use gpui::{
    AnimationExt, ClickEvent, Context, IntoElement, MouseButton, ParentElement, StyleRefinement,
    Styled, div, prelude::*,
};
use theme::{CONTROL_RADIUS, CONTROL_SIZE, HEADER_HEIGHT, MAIN_INSET, SIDEBAR_WIDTH, Theme};

use super::{GoBack, GoForward, Shell};
use crate::{
    assets::Icon, sidebar::icon_button, sidebar_glyph::sidebar_glyph, tooltip::TooltipExt,
    traffic_lights,
};

const MAC: bool = cfg!(target_os = "macos");

/// Width of the navigation while the sidebar is closed: the traffic lights, the
/// toggle, and Back and Forward right after it.
fn closed_width() -> gpui::Pixels {
    traffic_lights::content_start() + CONTROL_SIZE * 3.
}

impl Shell {
    pub(super) fn top_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .flex_none()
            .h(HEADER_HEIGHT)
            .flex()
            .items_center()
            // First, so every control in the row draws over it.
            .when(MAC, |row| row.child(self.titlebar(cx)))
            .child(self.navigation(cx))
            // Cached: a drag or a tab's motion redraws the tabs alone.
            .child(
                self.tabs
                    .clone()
                    .cached(StyleRefinement::default().flex_1().min_w_0().h_full()),
            )
            .child(div().flex_none().w(MAIN_INSET))
    }

    /// The top row's empty space, standing in for the macOS titlebar. The app
    /// owns it (`app_owns_titlebar_drag`), so AppKit acts on no click there:
    /// dragging it moves the window and double-clicking it does what System
    /// Settings says (zoom or minimize), while a control in the row, drawn over
    /// it and occluding it, gets only its own clicks.
    fn titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("titlebar")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, _| shell.titlebar_pressed = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|shell, _, _, _| shell.titlebar_pressed = false),
            )
            .on_mouse_down_out(cx.listener(|shell, _, _, _| shell.titlebar_pressed = false))
            .on_mouse_move(cx.listener(|shell, _, window, _| {
                if shell.titlebar_pressed {
                    shell.titlebar_pressed = false;
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
    }

    /// The toggle holds still beside the traffic lights while Back and Forward
    /// ride the sidebar's edge, coming to rest beside the toggle once it closes.
    fn navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let (back, forward) = if MAC {
            ("Back (⌘[)", "Forward (⌘])")
        } else {
            ("Back (Ctrl+[)", "Forward (Ctrl+])")
        };
        let can_go_back = self.visits.can_go_back();
        let can_go_forward = self.visits.can_go_forward();
        div()
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .pl(traffic_lights::content_start())
            .child(self.sidebar_toggle(cx))
            .child(div().flex_1())
            .child(
                icon_button("back", Icon::Back, back, can_go_back, colors, |button| {
                    button.occlude().when(can_go_back, |button| {
                        button.on_click(|_: &ClickEvent, window, cx| {
                            window.dispatch_action(Box::new(GoBack), cx)
                        })
                    })
                })
                .below(),
            )
            .child(
                icon_button(
                    "forward",
                    Icon::Forward,
                    forward,
                    can_go_forward,
                    colors,
                    |button| {
                        button.occlude().when(can_go_forward, |button| {
                            button.on_click(|_: &ClickEvent, window, cx| {
                                window.dispatch_action(Box::new(GoForward), cx)
                            })
                        })
                    },
                )
                .below(),
            )
            .with_spring(
                "navigation",
                self.sidebar_spring(),
                move |navigation, phase: f32| {
                    let phase = phase.clamp(0., 1.);
                    let closed = closed_width();
                    // Open, the arrows sit `MAIN_INSET` inside the sidebar's edge.
                    navigation
                        .w(closed + (SIDEBAR_WIDTH - closed) * phase)
                        .pr(MAIN_INSET * phase)
                },
            )
    }

    fn sidebar_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let label = match (self.sidebar_open, MAC) {
            (true, true) => "Hide sidebar (⌘B)",
            (true, false) => "Hide sidebar (Ctrl+B)",
            (false, true) => "Show sidebar (⌘B)",
            (false, false) => "Show sidebar (Ctrl+B)",
        };
        div()
            .id("sidebar-toggle")
            .occlude()
            .group("sidebar-toggle")
            .flex_none()
            .size(CONTROL_SIZE)
            .flex()
            .items_center()
            .justify_center()
            .rounded(CONTROL_RADIUS)
            .hover(|button| button.bg(colors.hover))
            .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| shell.toggle_sidebar(cx)))
            .child(div().with_spring(
                "sidebar-glyph",
                self.sidebar_spring(),
                move |glyph, phase| {
                    glyph.child(sidebar_glyph(
                        phase,
                        colors.text_secondary,
                        "sidebar-toggle",
                        colors.text_primary,
                    ))
                },
            ))
            .with_tooltip(label)
            .below()
    }
}
