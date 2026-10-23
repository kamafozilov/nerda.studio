//! The main window's layout. The window's glass holds the top row (the traffic
//! lights, the sidebar toggle, Back and Forward, then the tabs) and, under it, the
//! sidebar; the main area is a card raised beside the sidebar, rounded and inset
//! from the window's edges.

mod commands;
mod composers;
mod quit;

pub use quit::bind_keys as bind_quit_keys;
mod top_row;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui::{
    AnimationExt, App, ClickEvent, Context, DismissEvent, Entity, FocusHandle, IntoElement,
    ParentElement, Render, SpringAnimation, SpringConfig, SpringState, StyleRefinement, Styled,
    Subscription, Task, Window, actions, div, prelude::*,
};
use theme::{MAIN_INSET, MAIN_RADIUS, SIDEBAR_WIDTH, TEXT_BODY, Theme, motion};
use workspace::{SessionId, Tab, Timestamp, Visits};

use crate::{
    accessibility, add_project::AddProjectPalette, app_menu, instance::Screen, native_window,
    sidebar::Sidebar, store::Store, system_theme, tabs::TabStrip, welcome,
};

pub use commands::ActivateTab;

/// The key context of the main window's root.
pub const CONTEXT: &str = "Shell";

actions!(
    studio,
    [
        ToggleSidebar,
        /// Puts the cursor in the sidebar's search field, opening the sidebar
        /// first if it is closed.
        SearchSessions,
        /// Starts a session in the current project, as the sidebar's button does.
        NewSession,
        /// Opens the add-project palette, as the sidebar's button does.
        AddProject,
        /// Closes the selected tab.
        CloseTab,
        /// Closes every tab on screen but the selected one.
        CloseOtherTabs,
        /// Closes every tab on screen.
        CloseAllTabs,
        /// Selects the tab after the selected one, wrapping.
        NextTab,
        /// Selects the tab before the selected one, wrapping.
        PreviousTab,
        /// Shows the tab shown before.
        GoBack,
        /// Shows the tab Back left.
        GoForward,
        /// Cmd-Q in the window: asks before quitting.
        RequestQuit,
    ]
);

pub struct Shell {
    focus_handle: FocusHandle,
    store: Entity<Store>,
    sidebar: Entity<Sidebar>,
    tabs: Entity<TabStrip>,
    /// Back and Forward through the tabs shown.
    visits: Visits,
    /// Each open session's composer; see `composers`.
    composers: HashMap<SessionId, composers::OpenComposer>,
    /// The tab on screen as the composers last saw it, to tell a newly shown
    /// session, whose composer takes focus.
    tab_shown: Option<Tab>,
    sidebar_open: bool,
    /// The left button went down on the titlebar; the next move drags the window.
    titlebar_pressed: bool,
    add_project: Option<OpenPalette>,
    /// The question Cmd-Q puts up, while it is up; see `quit`.
    quit_dialog: Option<quit::QuitDialog>,
    /// Where the sidebar's spring stood at the last toggle, to know when it
    /// settles: GPUI keeps the spring itself out of reach.
    sidebar_motion: SidebarMotion,
    /// While the sidebar slides, the tabs keep the width they have beside the
    /// open sidebar; this lets them follow the strip again when it settles.
    tabs_held: Option<Task<()>>,
}

/// The add-project palette while it is open.
struct OpenPalette {
    palette: Entity<AddProjectPalette>,
    /// Where focus was before it opened, to give it back.
    focus_before: Option<FocusHandle>,
    _subscription: Subscription,
}

impl Shell {
    pub fn new(
        store: Entity<Store>,
        sidebar_open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // A change made in System Settings shows up when the app comes back to
        // front, so the theme is re-read then as well as on an appearance switch.
        let refresh = |_: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            // Every spring reads Reduce motion from GPUI's flag, which no
            // platform sets: it is read here, so a hover that glides from the
            // start already follows it.
            cx.set_reduce_motion(accessibility::reduce_motion());
            // Redraws every view: the sidebar and the tabs keep a cached frame
            // that a new theme must not reuse.
            if system_theme::refresh(window, cx) {
                window.refresh();
            }
        };
        cx.set_reduce_motion(accessibility::reduce_motion());
        cx.observe_window_appearance(window, refresh).detach();
        cx.observe_window_activation(window, refresh).detach();
        // A branch switched in another app, or ages that went on while the Mac
        // slept, show once the window is back in front.
        cx.observe_window_activation(window, |shell, window, cx| {
            if window.is_window_active() {
                shell
                    .sidebar
                    .update(cx, |sidebar, cx| sidebar.window_returned(cx));
            }
        })
        .detach();
        // Key bindings dispatch from the focused element, so the shell holds focus
        // until something inside it takes it.
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        // Focus given back to a composer whose tab closed meanwhile, as by
        // the tab menu, lands nowhere: it goes to the composer on screen.
        cx.on_focus_lost(window, |shell, window, cx| shell.focus_shown(window, cx))
            .detach();
        // A project on screen always shows something.
        let nothing_open = {
            let workspace = store.read(cx).workspace();
            workspace.has_projects() && workspace.selected_tab().is_none()
        };
        if nothing_open {
            store.update(cx, |store, cx| {
                store.update(cx, |workspace| {
                    workspace.ensure_open_session(Timestamp::now())
                })
            });
        }
        // Every way a tab comes on screen counts as a visit; a closed tab drops
        // out of Back and Forward.
        cx.observe_in(&store, window, |shell, store, window, cx| {
            let workspace = store.read(cx).workspace();
            let selected = workspace.selected_tab();
            shell.visits.prune(workspace.tabs(), selected);
            if let Some(selected) = selected {
                shell.visits.record(selected);
            }
            shell.sync_composers(window, cx);
            cx.notify();
        })
        .detach();
        let mut visits = Visits::default();
        if let Some(selected) = store.read(cx).workspace().selected_tab() {
            visits.record(selected);
        }
        let sidebar = cx.new(|cx| {
            Sidebar::new(
                store.clone(),
                focus_handle.clone(),
                sidebar_open,
                window,
                cx,
            )
        });
        let tabs = cx.new(|cx| TabStrip::new(store.clone(), cx));
        let mut shell = Self {
            focus_handle,
            store,
            sidebar,
            tabs,
            visits,
            composers: HashMap::new(),
            tab_shown: None,
            sidebar_open,
            titlebar_pressed: false,
            add_project: None,
            quit_dialog: None,
            sidebar_motion: SidebarMotion::at_rest(sidebar_open),
            tabs_held: None,
        };
        shell.sync_composers(window, cx);
        shell
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

    /// What a newer instance taking over needs to open where this one is.
    pub fn screen(&self, window: &Window, cx: &App) -> Screen {
        Screen {
            bounds: window.window_bounds(),
            display: window
                .display(cx)
                .and_then(|display| display.uuid().ok())
                .map(|uuid| uuid.as_u128()),
            sidebar_open: self.sidebar_open,
            active: window.is_window_active(),
            window: native_window::number(window),
        }
    }

    /// Every way in slides the sidebar, the click and `Cmd-B` alike, so the
    /// same action always looks the same.
    ///
    /// While it slides, the tabs keep the width they have beside the open
    /// sidebar, the narrower of their two, so their titles truncate once
    /// instead of on every frame and none passes the window's edge; they follow
    /// the strip again the moment the sidebar settles.
    fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.set_reduce_motion(accessibility::reduce_motion());
        self.sidebar_open = !self.sidebar_open;
        let open = self.sidebar_open;
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_open(open, window, cx));
        let settle = self.sidebar_motion.retarget(open);
        self.tabs_held = (!cx.reduce_motion()).then(|| {
            cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(settle).await;
                shell
                    .update(cx, |shell, cx| {
                        shell.tabs_held = None;
                        cx.notify();
                    })
                    .ok();
            })
        });
        cx.notify();
    }

    /// The sidebar's phase, 1 open and 0 closed. Everything that moves with the
    /// sidebar holds a spring built from this, so they move as one.
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

    /// The sidebar's contents at full width, pinned to the window's left edge:
    /// the sidebar's edge sweeps over them as it narrows, and the card slides
    /// over the space they leave.
    fn sidebar(&self) -> impl IntoElement {
        div()
            .relative()
            .flex_none()
            .h_full()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(SIDEBAR_WIDTH)
                    .h_full()
                    // Cached: a frame of the tabs or the card doesn't lay the
                    // list out again; the sidebar redraws when it changes.
                    .child(
                        self.sidebar
                            .clone()
                            .cached(StyleRefinement::default().size_full()),
                    ),
            )
            .with_spring(
                "sidebar",
                self.sidebar_spring(),
                move |sidebar, phase: f32| {
                    let phase = phase.clamp(0., 1.);
                    sidebar.w(SIDEBAR_WIDTH * phase).opacity(phase)
                },
            )
    }

    /// The main area: a rounded card with a hairline, `MAIN_INSET` clear of the
    /// window's right and bottom edges and, as the sidebar closes, of its left.
    /// Before the first project it welcomes, unless a page is on screen; a
    /// session shows its composer, and a page nothing yet.
    fn main_area(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let page_open = matches!(workspace.selected_tab(), Some(Tab::Page(_)));
        let welcome = !page_open && !workspace.has_projects();
        let card = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(MAIN_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .when_some(self.shown_composer(), |card, composer| card.child(composer))
            .when(welcome, |card| {
                card.child(welcome::no_projects(
                    cx.listener(|shell, _: &ClickEvent, window, cx| {
                        shell.open_add_project(window, cx)
                    }),
                    cx,
                ))
            });
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .pr(MAIN_INSET)
            .pb(MAIN_INSET)
            .child(card)
            .with_spring(
                "main-area",
                self.sidebar_spring(),
                move |main, phase: f32| main.pl(MAIN_INSET * (1. - phase.clamp(0., 1.))),
            )
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let root = div()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|shell, _: &ToggleSidebar, window, cx| {
                    shell.toggle_sidebar(window, cx)
                }),
            )
            .on_action(cx.listener(|shell, _: &SearchSessions, window, cx| {
                // The palette keeps focus while it is open over the window.
                if shell.add_project.is_some() {
                    return;
                }
                if !shell.sidebar_open {
                    shell.toggle_sidebar(window, cx);
                }
                shell
                    .tabs
                    .update(cx, |tabs, cx| tabs.dismiss_menu(window, cx));
                shell
                    .sidebar
                    .update(cx, |sidebar, cx| sidebar.focus_search(window, cx))
            }))
            .on_action(cx.listener(|shell, _: &NewSession, _, cx| {
                shell
                    .sidebar
                    .update(cx, |sidebar, cx| sidebar.new_session(cx))
            }))
            .on_action(
                cx.listener(|shell, _: &AddProject, window, cx| shell.open_add_project(window, cx)),
            )
            .on_action(
                cx.listener(|shell, _: &RequestQuit, window, cx| shell.request_quit(window, cx)),
            );
        let shell = self
            .on_tab_actions(root, cx)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.window_background)
            .text_color(colors.text_primary)
            .text_size(TEXT_BODY)
            .child(self.top_row(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar())
                    .child(self.main_area(cx)),
            )
            // Over everything, the top row included, while it is open.
            .children(self.add_project.as_ref().map(|open| open.palette.clone()));
        // The quit dialog sits beside the shell, not in it: out of the shell's key
        // context, so its shortcuts rest while the question is up.
        app_menu::on_window_actions(div())
            .relative()
            .size_full()
            .child(shell)
            .children(self.quit_dialog(cx))
    }
}

/// The sidebar's spring as the shell last set it, worked out with the same
/// math GPUI runs, so the shell knows when it settles.
struct SidebarMotion {
    state: SpringState,
    target: f32,
    config: SpringConfig,
    since: Instant,
}

impl SidebarMotion {
    fn at_rest(open: bool) -> Self {
        let target = if open { 1. } else { 0. };
        Self {
            state: SpringState {
                position: target,
                velocity: 0.,
            },
            target,
            config: motion::PANEL_ENTER,
            since: Instant::now(),
        }
    }

    /// Turns the spring toward open or closed from where it is now, keeping its
    /// speed as GPUI does; returns how long it takes to settle there.
    fn retarget(&mut self, open: bool) -> Duration {
        let now = Instant::now();
        let elapsed = now.duration_since(self.since).as_secs_f32();
        self.state = self.config.step(self.state, self.target, elapsed);
        (self.target, self.config) = if open {
            (1., motion::PANEL_ENTER)
        } else {
            (0., motion::PANEL_EXIT)
        };
        self.since = now;
        self.config
            .settle_time(self.state, self.target, motion::PHASE_EPSILON)
    }
}
