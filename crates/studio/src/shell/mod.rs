//! The main window's layout. The window's glass holds the top row (the traffic
//! lights, the sidebar toggle, Back and Forward, then the tabs) and, under it, the
//! sidebar; the main area is a card raised beside the sidebar, rounded and inset
//! from the window's edges. Settings takes over the sidebar and the card while
//! it is open (`settings_mode`).

mod commands;
mod composers;
mod layout;
mod quit;
mod settings_mode;
mod sidebar_edge;
mod top_row;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui::{
    App, Context, DismissEvent, Entity, FocusHandle, IntoElement, ParentElement, Pixels, Render,
    SpringAnimation, SpringConfig, SpringState, Styled, Subscription, Task, Window, actions, div,
    prelude::*,
};
use theme::{TEXT_BODY, Theme, motion};
use workspace::{SessionId, Tab, Timestamp, Visits};

use crate::{
    accessibility,
    add_project::AddProjectPalette,
    app_menu,
    instance::Screen,
    native_window,
    settings::{Leave, SettingsPage},
    sidebar::Sidebar,
    store::Store,
    system_theme,
    tabs::TabStrip,
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
        /// Shows the Settings page's tab, as the sidebar's button does.
        OpenSettings,
        /// Cmd-Q in the window: the first press asks for a second, which quits.
        RequestQuit,
    ]
);

pub struct Shell {
    focus_handle: FocusHandle,
    store: Entity<Store>,
    sidebar: Entity<Sidebar>,
    tabs: Entity<TabStrip>,
    /// Kept while the app runs, so the section chosen stays between visits.
    settings: Entity<SettingsPage>,
    /// Settings has the sidebar and the card, in place of the tabs.
    settings_open: bool,
    /// Back and Forward through the tabs shown.
    visits: Visits,
    /// Each open session's composer; see `composers`.
    composers: HashMap<SessionId, composers::OpenComposer>,
    /// The tab on screen as the composers last saw it, to tell a newly shown
    /// session, whose composer takes focus.
    tab_shown: Option<Tab>,
    sidebar_open: bool,
    /// The open sidebar's width, as its edge was last dragged.
    sidebar_width: Pixels,
    /// While the sidebar's edge is dragged; see `sidebar_edge`.
    resizing: Option<sidebar_edge::Resize>,
    /// The left button went down on the titlebar; the next move drags the window.
    titlebar_pressed: bool,
    add_project: Option<OpenPalette>,
    /// The timer of the hint Cmd-Q put up, while it is up; see `quit`.
    quit_hint: Option<Task<()>>,
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
        let settings = cx.new(|cx| SettingsPage::new(store.clone(), cx));
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&settings, window, |shell, _, _: &Leave, window, cx| {
            shell.leave_settings(window, cx);
        })
        .detach();
        let sidebar_width = sidebar_edge::saved_width(store.read(cx));
        let mut shell = Self {
            focus_handle,
            store,
            sidebar,
            tabs,
            settings,
            settings_open: false,
            visits,
            composers: HashMap::new(),
            tab_shown: None,
            sidebar_open,
            sidebar_width,
            resizing: None,
            titlebar_pressed: false,
            add_project: None,
            quit_hint: None,
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
    #[cfg(all(target_os = "macos", test))]
    pub fn sidebar_open(&self) -> bool {
        self.sidebar_open
    }

    #[cfg(all(target_os = "macos", test))]
    pub fn sidebar_width(&self) -> Pixels {
        self.sidebar_width
    }

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
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let root =
            div()
                .key_context(CONTEXT)
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(|shell, _: &ToggleSidebar, window, cx| {
                    shell.toggle_sidebar(window, cx)
                }))
                .on_action(cx.listener(|shell, _: &SearchSessions, window, cx| {
                    // The palette keeps focus while it is open over the window.
                    if shell.add_project.is_some() {
                        return;
                    }
                    shell.leave_settings(window, cx);
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
                .on_action(cx.listener(|shell, _: &NewSession, window, cx| {
                    shell.leave_settings(window, cx);
                    shell
                        .sidebar
                        .update(cx, |sidebar, cx| sidebar.new_session(cx))
                }))
                .on_action(cx.listener(|shell, _: &AddProject, window, cx| {
                    shell.open_add_project(window, cx)
                }))
                .on_action(cx.listener(|shell, _: &RequestQuit, _, cx| shell.request_quit(cx)));
        let root = app_menu::on_window_actions(root);
        self.on_tab_actions(root, cx)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(crate::background::over_desktop(colors.window_background))
            .text_color(colors.text_primary)
            .text_size(TEXT_BODY)
            .child(self.top_row(window, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(window, cx))
                    .child(self.main_area(cx))
                    // Only at rest: while the sidebar slides its edge moves.
                    .when(self.sidebar_open && self.tabs_held.is_none(), |row| {
                        row.child(self.sidebar_edge(cx))
                    }),
            )
            // Over everything, the top row included, while it is open.
            .children(self.add_project.as_ref().map(|open| open.palette.clone()))
            .children(self.quit_hint(cx))
            .children(self.resize_cover(cx))
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
