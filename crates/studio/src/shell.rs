//! The main window's layout: a sidebar on the window's glass and the main area
//! raised over it, sharing one top row.

use gpui::{
    AnimationExt, ClickEvent, Context, DismissEvent, Entity, FocusHandle, FontWeight, IntoElement,
    MouseButton, ParentElement, Pixels, Render, SharedString, SpringAnimation, Styled,
    Subscription, Window, actions, div, prelude::*, relative,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, HEADER_HEIGHT, SIDEBAR_WIDTH, SPACE_1, SPACE_2, SPACE_3,
    TEXT_BODY, Theme, motion,
};

use crate::{
    accessibility, add_project::AddProjectPalette, sidebar::Sidebar, sidebar_glyph::sidebar_glyph,
    store::Store, system_theme, traffic_lights, welcome,
};

actions!(
    studio,
    [
        ToggleSidebar,
        /// Starts a session in the current project, as the sidebar's button does.
        NewSession,
        /// Opens the add-project palette, as the sidebar's button does.
        AddProject
    ]
);

pub struct Shell {
    focus_handle: FocusHandle,
    store: Entity<Store>,
    sidebar: Entity<Sidebar>,
    sidebar_open: bool,
    /// The left button went down on the titlebar; the next move drags the window.
    titlebar_pressed: bool,
    add_project: Option<OpenPalette>,
}

/// The add-project palette while it is open.
struct OpenPalette {
    palette: Entity<AddProjectPalette>,
    /// Where focus was before it opened, to give it back.
    focus_before: Option<FocusHandle>,
    _subscription: Subscription,
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
        let store = cx.new(|_| Store::load());
        // The main area's top row names the selected session.
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        let sidebar = cx.new(|cx| Sidebar::new(store.clone(), cx));
        Self {
            focus_handle,
            store,
            sidebar,
            sidebar_open: true,
            titlebar_pressed: false,
            add_project: None,
        }
    }

    /// Opens the add-project palette; does nothing while it is open.
    fn open_add_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.add_project.is_some() {
            return;
        }
        let focus_before = window.focused(cx);
        let workspace = self.store.read(cx).workspace();
        let base = workspace
            .context_project()
            .and_then(|project| workspace.project(project))
            .map(|project| project.path.clone());
        let sidebar = self.sidebar.clone();
        let palette = cx.new(|cx| AddProjectPalette::new(sidebar, base, window, cx));
        let subscription = cx.subscribe_in(
            &palette,
            window,
            |shell, _, _: &DismissEvent, window, cx| shell.close_add_project(window, cx),
        );
        self.add_project = Some(OpenPalette {
            palette,
            focus_before,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Closes the palette and gives focus back to where it was.
    fn close_add_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.add_project.take() else {
            return;
        };
        let focus = open
            .focus_before
            .unwrap_or_else(|| self.focus_handle.clone());
        window.focus(&focus, cx);
        cx.notify();
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
            .child(div().flex_1().min_h_0().child(self.sidebar.clone()))
    }

    /// The main area's top row: the selected session as "project / session". Its left
    /// edge follows the sidebar's spring so the text clears the toggle at every
    /// width instead of jumping when the sidebar closes.
    fn main_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let crumbs = workspace.selected_session().and_then(|session| {
            let session = workspace.session(session)?;
            let project = workspace.display_name(session.project)?;
            Some((
                SharedString::from(project),
                SharedString::from(session.title.clone()),
            ))
        });
        let toggle_end = traffic_lights::content_start() + CONTROL_SIZE + SPACE_2;
        div()
            .h(HEADER_HEIGHT)
            .flex_none()
            .pr(SPACE_3)
            .flex()
            .items_center()
            .gap(SPACE_1)
            .whitespace_nowrap()
            .when_some(crumbs, |row, (project, session)| {
                // Both crumbs truncate; the project gives way first, at half the row.
                row.child(
                    div()
                        .min_w_0()
                        .max_w(relative(0.5))
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(colors.text_secondary)
                        .child(project),
                )
                .child(
                    div()
                        .flex_none()
                        .text_color(colors.text_secondary)
                        .child("/"),
                )
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(colors.text_primary)
                        .child(session),
                )
            })
            .with_spring(
                "main-header",
                self.sidebar_spring(),
                move |row, phase: f32| {
                    let sidebar_edge = SIDEBAR_WIDTH * phase.clamp(0., 1.);
                    let clear_of_toggle: Pixels = toggle_end - sidebar_edge;
                    row.pl(if clear_of_toggle > SPACE_3 {
                        clear_of_toggle
                    } else {
                        SPACE_3
                    })
                },
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
        let has_projects = self.store.read(cx).workspace().has_projects();
        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|shell, _: &ToggleSidebar, _, cx| shell.toggle_sidebar(cx)))
            .on_action(cx.listener(|shell, _: &NewSession, _, cx| {
                shell
                    .sidebar
                    .update(cx, |sidebar, cx| sidebar.new_session(cx))
            }))
            .on_action(
                cx.listener(|shell, _: &AddProject, window, cx| shell.open_add_project(window, cx)),
            )
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
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .child(self.main_header(cx))
                    .when(!has_projects, |main| {
                        main.child(welcome::no_projects(
                            cx.listener(|shell, _: &ClickEvent, window, cx| {
                                shell.open_add_project(window, cx)
                            }),
                            cx,
                        ))
                    }),
            )
            // The titlebar spans the top row on macOS, where it replaces AppKit's.
            .when(cfg!(target_os = "macos"), |root| {
                root.child(self.titlebar(cx))
            })
            // Last, so it draws over the rest: it stays put while the sidebar moves.
            .child(self.sidebar_toggle(cx))
            // Over everything, the top row included, while it is open.
            .children(self.add_project.as_ref().map(|open| open.palette.clone()))
    }
}
