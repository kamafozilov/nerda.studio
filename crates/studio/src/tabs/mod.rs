//! The top row's tabs: the pages and the sessions open in the project on
//! screen, then a New session button. Tabs share the row up to `TAB_MAX_WIDTH`
//! each; an opened tab's slot grows from nothing and a closed one collapses in
//! place before it leaves. A tab can be dragged along the row, closed by its
//! button, a middle click or its menu, and the selected one by `Cmd-W`. A
//! project's unused draft alone in the row has no close button: closing it
//! would only open it again.
//!
//! The strip redraws on every frame of a drag or a tab's motion, so a frame only
//! reads what it keeps: the workspace is read again only after it changes, and
//! each tab's title and element names are made once, not per frame.

mod drag;
mod menu;
mod order;
mod tab;

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::{
    Bounds, ClickEvent, Context, Entity, IntoElement, ParentElement, Pixels, Render, SharedString,
    Styled, Task, Window, div, prelude::*,
};
use theme::{
    TAB_GAP, Theme,
    motion::{PHASE_EPSILON, TAB_EXIT},
};
use workspace::Tab;

use crate::{
    accessibility, assets::Icon, kbd::Shortcut, menu::Menu, sidebar::icon_button, store::Store,
};

pub struct TabStrip {
    store: Entity<Store>,
    /// The workspace changed since the strip last read it.
    stale: bool,
    has_projects: bool,
    selected: Option<Tab>,
    /// The tabs as last drawn, live and closing, in row order.
    order: Vec<Tab>,
    /// The tabs on screen when the row last changed.
    live: Vec<Tab>,
    /// Opened since the row last changed: their slots grow from nothing.
    opening: HashSet<Tab>,
    /// Closed tabs collapsing in place, drawn from what they last showed.
    closing: HashMap<Tab, Closing>,
    /// Each drawn tab's title and element names, made when the workspace changes.
    labels: HashMap<Tab, Label>,
    /// Where each slot was laid out last frame, for dragging and for whether a
    /// tab is wide enough for its close button.
    bounds: Rc<RefCell<HashMap<Tab, Bounds<Pixels>>>>,
    drag: Option<drag::TabDrag>,
    /// Names the tabs' glide springs; renamed on each drop, so every spring
    /// starts again from where its tab was drawn, kept in `settle_from`.
    settle: SharedString,
    drops: usize,
    settle_from: HashMap<Tab, f32>,
    /// The press that ended a drag also ends in a click; that click is not a choice.
    skip_click: bool,
    menu: Menu<TabStrip, menu::TabMenu>,
}

/// What a tab shows and the names its elements go by.
#[derive(Clone)]
struct Label {
    title: SharedString,
    /// A page's icon before its title; sessions have none.
    icon: Option<Icon>,
    /// The tab's hover group, which shows its close button.
    group: SharedString,
    close: SharedString,
    /// Closing it would change something; see `Workspace::can_close_tab`.
    closable: bool,
}

struct Closing {
    label: Label,
    /// Removes the slot once it has collapsed; dropped if the tab reopens first.
    _remove: Task<()>,
}

impl TabStrip {
    pub fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |strip, _, cx| {
            strip.stale = true;
            cx.notify();
        })
        .detach();
        Self {
            store,
            stale: true,
            has_projects: false,
            selected: None,
            order: Vec::new(),
            live: Vec::new(),
            opening: HashSet::new(),
            closing: HashMap::new(),
            labels: HashMap::new(),
            bounds: Rc::default(),
            drag: None,
            settle: "tab-shift-0".into(),
            drops: 0,
            settle_from: HashMap::new(),
            skip_click: false,
            menu: Menu::new(|strip| &mut strip.menu, cx),
        }
    }

    /// Closes the tab menu, as when Cmd-K takes focus to the search.
    pub fn dismiss_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu.close(window, cx);
    }

    /// Reads the workspace again after it changed: the tabs on screen, the
    /// selection and the titles.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.stale) {
            return;
        }
        let workspace = self.store.read(cx).workspace();
        let live = workspace.open_tabs();
        self.selected = workspace.selected_tab();
        self.has_projects = workspace.has_projects();
        let labels: HashMap<Tab, Label> = live
            .iter()
            .map(|&id| {
                let title = match id {
                    Tab::Session(session) => workspace
                        .session(session)
                        .map(|session| SharedString::from(session.display_title()))
                        .unwrap_or_default(),
                    Tab::Page(page) => page.title().into(),
                };
                let label = Label {
                    title,
                    icon: id.page().map(Icon::page),
                    group: format!("tab-{}", id.key()).into(),
                    close: format!("close-tab-{}", id.key()).into(),
                    closable: workspace.can_close_tab(id),
                };
                (id, label)
            })
            .collect();
        // Closing tabs take their label from the row they leave.
        self.sync(live, cx);
        self.labels = labels;
    }

    /// Brings the row up to the workspace's tabs: a tab that left starts
    /// collapsing where it was, one that arrived starts growing. Switching to
    /// another project replaces the row at once; that is not a close.
    fn sync(&mut self, live: Vec<Tab>, cx: &mut Context<Self>) {
        if live == self.live {
            return;
        }
        let previous = std::mem::replace(&mut self.live, live);
        self.opening.clear();
        // A change to the row can't wait on a drag that held the old one.
        self.drag = None;
        cx.set_reduce_motion(accessibility::reduce_motion());
        let replace_all = !previous.is_empty()
            && !self.live.is_empty()
            && previous.iter().all(|id| !self.live.contains(id));
        if replace_all || cx.reduce_motion() {
            self.closing.clear();
            self.order = self.live.clone();
        } else {
            for &id in previous.iter().filter(|id| !self.live.contains(id)) {
                let Some(label) = self.labels_for_closing(id) else {
                    continue;
                };
                let remove = cx.spawn(async move |strip, cx| {
                    cx.background_executor().timer(collapse_time()).await;
                    strip
                        .update(cx, |strip, cx| {
                            strip.closing.remove(&id);
                            strip.order.retain(|tab| *tab != id);
                            strip.bounds.borrow_mut().remove(&id);
                            cx.notify();
                        })
                        .ok();
                });
                self.closing.insert(
                    id,
                    Closing {
                        label,
                        _remove: remove,
                    },
                );
            }
            for id in &self.live {
                // Reopened while it was collapsing: it grows back from where it is.
                self.closing.remove(id);
                if !previous.is_empty() && !previous.contains(id) {
                    self.opening.insert(*id);
                }
            }
            self.order =
                order::with_closing(&self.order, &self.live, |id| self.closing.contains_key(id));
        }
        let order = &self.order;
        self.bounds.borrow_mut().retain(|id, _| order.contains(id));
    }

    /// What a tab that just left the row keeps showing while it collapses:
    /// what it showed last.
    fn labels_for_closing(&self, id: Tab) -> Option<Label> {
        self.closing
            .get(&id)
            .map(|closing| closing.label.clone())
            .or_else(|| self.labels.get(&id).cloned())
    }

    fn open(&mut self, id: Tab, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| workspace.open_tab(id))
        });
    }

    fn close(&mut self, id: Tab, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                workspace.close_tab(id, workspace::Timestamp::now())
            })
        });
    }

    fn new_tab_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        icon_button(
            "new-tab",
            Icon::NewTab,
            "New session",
            true,
            colors,
            |button| {
                button
                    .occlude()
                    .ml(TAB_GAP)
                    .on_click(|_: &ClickEvent, window, cx| {
                        window.dispatch_action(Box::new(crate::shell::NewSession), cx)
                    })
            },
        )
        .shortcut(Shortcut::key("T").cmd())
        .below()
    }
}

/// How long a closed tab's slot takes to collapse from fully open.
fn collapse_time() -> std::time::Duration {
    let open = gpui::SpringState {
        position: 1.,
        velocity: 0.,
    };
    TAB_EXIT.settle_time(open, 0., PHASE_EPSILON)
}

impl Render for TabStrip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh(cx);
        // A menu whose tab closed some other way has nothing left to act on.
        if let Some(menu) = self.menu.opened()
            && !self.live.contains(&menu.tab)
        {
            self.menu.close(window, cx);
        }
        let colors = Theme::of(cx).colors;
        let slots: Vec<_> = (0..self.order.len())
            .map(|index| self.render_slot(self.order[index], colors, cx))
            .collect();
        div()
            .id("tab-strip")
            .size_full()
            .flex()
            .items_center()
            .child(
                div()
                    .id("tabs")
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(TAB_GAP)
                    .overflow_hidden()
                    .children(slots),
            )
            .when(self.has_projects, |strip| {
                strip.child(self.new_tab_button(cx))
            })
            .children(self.drag_watcher(cx))
            .children(self.render_menu(window, cx))
    }
}
