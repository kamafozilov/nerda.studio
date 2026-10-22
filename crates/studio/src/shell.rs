//! The main window's layout: a sidebar on the window's glass and the main area
//! raised over it, sharing one top row.

use gpui::{
    AnimationExt, ClickEvent, Context, FocusHandle, FontWeight, IntoElement, MouseButton,
    ParentElement, Render, SpringAnimation, Styled, Window, actions, div, prelude::*,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, HEADER_HEIGHT, SIDEBAR_WIDTH, SPACE_1, TEXT_BODY, Theme, motion,
};

use crate::{accessibility, sidebar_glyph::sidebar_glyph, system_theme, traffic_lights};

actions!(studio, [ToggleSidebar]);

pub struct Shell {
    focus_handle: FocusHandle,
    sidebar_open: bool,
    /// The left button went down on the titlebar; the next move drags the window.
    titlebar_pressed: bool,
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // A change made in System Settings shows up when the app comes back to
        // front, so the theme is re-read then as well as on an appearance switch.
        let refresh = |_: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            if system_theme::refresh(window, cx) {
                cx.notify();
            }
        };
        cx.observe_window_appearance(window, refresh).detach();
        cx.observe_window_activation(window, refresh).detach();
        // Key bindings dispatch from the focused element, so the shell holds focus
        // until something inside it takes it.
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            focus_handle,
            sidebar_open: true,
            titlebar_pressed: false,
        }
    }

    /// Every way in slides the sidebar, the click and `Cmd-B` alike, so the
    /// same action always looks the same.
    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        cx.set_reduce_motion(accessibility::reduce_motion());
        self.sidebar_open = !self.sidebar_open;
        cx.notify();
    }

    /// The sidebar's phase, 1 open and 0 closed. The sidebar and the toggle's
    /// glyph each hold a spring built from this, so they move as one.
    fn sidebar_spring(&self) -> SpringAnimation<f32> {
        let (target, config) = if self.sidebar_open {
            (1., motion::PANEL_ENTER)
        } else {
            (0., motion::PANEL_EXIT)
        };
        SpringAnimation::new(config)
            .to(target)
            .with_epsilon(motion::PHASE_EPSILON)
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
            .right_0()
            .h(HEADER_HEIGHT)
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

    fn sidebar_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        div()
            .id("sidebar-toggle")
            .occlude()
            .group("sidebar-toggle")
            .absolute()
            .top((HEADER_HEIGHT - CONTROL_SIZE) / 2.)
            .left(traffic_lights::content_start())
            .size(CONTROL_SIZE)
            .flex()
            .items_center()
            .justify_center()
            .rounded(CONTROL_RADIUS)
            .hover(|button| button.bg(colors.hover))
            .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| shell.toggle_sidebar(cx)))
            .with_spring(
                "sidebar-glyph",
                self.sidebar_spring(),
                move |button, phase| {
                    button.child(sidebar_glyph(
                        phase,
                        colors.text_secondary,
                        "sidebar-toggle",
                        colors.text_primary,
                    ))
                },
            )
    }

    /// The sidebar's contents at full width, pinned to the window's left edge:
    /// the sidebar's edge sweeps over them as it narrows. Sliding them instead
    /// would carry the title under the traffic lights and the toggle, which
    /// stay put.
    fn sidebar_content(&self) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(HEADER_HEIGHT)
                    .flex_none()
                    .flex()
                    .items_center()
                    .pl(traffic_lights::content_start() + CONTROL_SIZE + SPACE_1)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Nerda Studio"),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = Theme::of(cx).colors.border;
        div()
            .relative()
            .flex_none()
            .h_full()
            .overflow_hidden()
            .child(self.sidebar_content())
            .with_spring(
                "sidebar",
                self.sidebar_spring(),
                move |sidebar, phase: f32| {
                    let phase = phase.clamp(0., 1.);
                    sidebar
                        .w(SIDEBAR_WIDTH * phase)
                        .opacity(phase)
                        .when(phase > 0., |sidebar| {
                            sidebar.border_r_1().border_color(border)
                        })
                },
            )
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|shell, _: &ToggleSidebar, _, cx| shell.toggle_sidebar(cx)))
            .relative()
            .size_full()
            .flex()
            .bg(colors.window_background)
            .text_color(colors.text_primary)
            .text_size(TEXT_BODY)
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .bg(colors.panel_background),
            )
            // The titlebar spans the top row on macOS, where it replaces AppKit's.
            .when(cfg!(target_os = "macos"), |root| {
                root.child(self.titlebar(cx))
            })
            // Last, so it draws over the rest: it stays put while the sidebar moves.
            .child(self.sidebar_toggle(cx))
    }
}
