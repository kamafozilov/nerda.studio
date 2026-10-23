//! What the tab and navigation shortcuts do. Each acts on the tabs on screen,
//! as the tab menu and the top row's buttons do, so every way in moves the same.

use gpui::{Context, Div, InteractiveElement, Window};
use workspace::{Tab, Timestamp, Workspace};

use super::{
    CloseAllTabs, CloseOtherTabs, CloseTab, GoBack, GoForward, NextTab, PreviousTab, Shell,
};

/// Selects the tab at this index on screen; `ActivateTab::LAST` the last one.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = studio, no_json)]
pub struct ActivateTab(pub usize);

impl ActivateTab {
    pub const LAST: usize = usize::MAX;
}

impl Shell {
    pub(super) fn on_tab_actions(&self, root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(
            cx.listener(|shell, _: &CloseTab, _, cx| shell.with_selected(cx, Workspace::close_tab)),
        )
        .on_action(cx.listener(|shell, _: &CloseOtherTabs, _, cx| {
            shell.with_selected(cx, Workspace::close_other_tabs)
        }))
        .on_action(cx.listener(|shell, _: &CloseAllTabs, _, cx| {
            shell.with_selected(cx, |workspace, _, now| workspace.close_all_tabs(now))
        }))
        .on_action(cx.listener(|shell, _: &NextTab, _, cx| shell.step_tab(1, cx)))
        .on_action(cx.listener(|shell, _: &PreviousTab, _, cx| shell.step_tab(-1, cx)))
        .on_action(cx.listener(|shell, action: &ActivateTab, _, cx| {
            let workspace = shell.store.read(cx).workspace();
            let tab = match action.0 {
                ActivateTab::LAST => workspace.open_tabs().last().copied(),
                index => workspace.tab_at(index),
            };
            if let Some(tab) = tab {
                shell.show(tab, cx);
            }
        }))
        .on_action(cx.listener(|shell, _: &GoBack, window, cx| shell.go(true, window, cx)))
        .on_action(cx.listener(|shell, _: &GoForward, window, cx| shell.go(false, window, cx)))
    }

    /// Runs `change` on the selected tab, if one is on screen.
    fn with_selected(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Workspace, Tab, Timestamp),
    ) {
        let Some(selected) = self.store.read(cx).workspace().selected_tab() else {
            return;
        };
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                change(workspace, selected, Timestamp::now())
            })
        });
    }

    fn step_tab(&mut self, step: isize, cx: &mut Context<Self>) {
        if let Some(tab) = self.store.read(cx).workspace().adjacent_tab(step) {
            self.show(tab, cx);
        }
    }

    fn go(&mut self, back: bool, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.visits.step(back) {
            self.show(tab, cx);
        }
        cx.notify();
    }

    fn show(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| workspace.open_tab(tab))
        });
    }
}
