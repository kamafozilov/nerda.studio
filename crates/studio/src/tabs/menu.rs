//! A tab's menu, opened by a right click on it: close it, the others, those to
//! either side, or all of them.

use gpui::{AnyElement, Context, Pixels, Point, Window};
use workspace::{Tab, Timestamp, Workspace};

use super::TabStrip;
use crate::menu::{Entry, Item, Placement};

const MAC: bool = cfg!(target_os = "macos");

/// The tab the menu opened on, and where.
pub(super) struct TabMenu {
    pub tab: Tab,
    position: Point<Pixels>,
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
        self.menu.open(TabMenu { tab, position }, window, cx);
    }

    pub(super) fn render_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.menu.opened()?;
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
        let entries = choices
            .into_iter()
            .map(|choice| {
                let Choice {
                    id,
                    label,
                    shortcut,
                    enabled,
                    run,
                } = choice;
                let item = Item::new(id, label, move |strip: &mut TabStrip, _, cx| {
                    strip.store.update(cx, |store, cx| {
                        store.update(cx, |workspace| run(workspace, tab, Timestamp::now()))
                    })
                })
                .enabled(enabled);
                Entry::Item(match shortcut {
                    Some(keys) => item.shortcut(keys),
                    None => item,
                })
            })
            .collect();
        self.menu.render(
            "tab-menu",
            entries,
            Placement::At(menu.position),
            window,
            cx,
        )
    }
}
