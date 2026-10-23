//! The filter and sort menu: how sessions are ordered and which project they come
//! from.

use gpui::{
    Anchor, ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*,
};
use theme::Theme;
use workspace::{ProjectScope, SessionSort, Workspace};

use super::{Sidebar, header::icon_button};
use crate::{
    assets::Icon,
    menu::{Entry, Item, Placement},
};

impl Sidebar {
    pub(super) fn render_menu_button(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let open = self.menu.opened().is_some();
        // A project filter stays marked on the button, so a list narrowed to one
        // project never passes for all of them.
        let filtered = self.store.read(cx).workspace().scope() != ProjectScope::All;
        let menu = self.menu.render(
            "menu",
            self.menu_entries(cx),
            Placement::Hang(Anchor::TopRight),
            window,
            cx,
        );
        div()
            .relative()
            .flex_none()
            .child(icon_button(
                "menu",
                Icon::Sort,
                "Sort and filter sessions",
                true,
                colors,
                |button| {
                    button
                        .when(open || filtered, |button| button.bg(colors.hover))
                        .on_click(cx.listener(|sidebar, _: &ClickEvent, window, cx| {
                            sidebar.menu.open((), window, cx)
                        }))
                },
            ))
            // The menu hangs from the button's bottom-right corner.
            .children(menu.map(|menu| div().absolute().top_full().right_0().child(menu)))
    }

    fn menu_entries(&self, cx: &mut Context<Self>) -> Vec<Entry<Self>> {
        let workspace = self.store.read(cx).workspace();
        let sort = workspace.sort();
        let scope = workspace.scope();
        let mut entries = vec![
            Entry::Section("Sort by"),
            choice(
                "sort-activity",
                "Last activity",
                sort == SessionSort::LastActivity,
                |workspace| workspace.set_sort(SessionSort::LastActivity),
            ),
            choice(
                "sort-created",
                "Created",
                sort == SessionSort::Created,
                |workspace| workspace.set_sort(SessionSort::Created),
            ),
            Entry::Separator,
            Entry::Section("Project"),
            choice(
                "scope-all",
                "All projects",
                scope == ProjectScope::All,
                |workspace| workspace.set_scope(ProjectScope::All),
            ),
        ];
        for project in workspace.projects() {
            let id = project.id;
            let name = SharedString::from(workspace.display_name(id).unwrap_or_default());
            entries.push(choice(
                ("scope", id.0),
                name,
                scope == ProjectScope::Project(id),
                move |workspace| workspace.set_scope(ProjectScope::Project(id)),
            ));
        }
        entries
    }
}

/// A choice in the menu, checked while it applies: picking it applies it.
fn choice(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    choose: impl Fn(&mut Workspace) + 'static,
) -> Entry<Sidebar> {
    Entry::Item(
        Item::new(id, label, move |sidebar: &mut Sidebar, _, cx| {
            sidebar.store.update(cx, |store, cx| {
                store.update(cx, |workspace| choose(workspace))
            })
        })
        .checked(checked),
    )
}
