//! The sessions' menu, from the button by their title: add a project, how
//! sessions are ordered, and which project they come from. The order and the
//! project open beside the menu, each naming its current choice.

use gpui::{
    Anchor, ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*,
};
use theme::Theme;
use workspace::{NO_PROJECT, ProjectScope, SessionSort, Workspace};

use super::{Sidebar, header::row_button};
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
            Placement::Hang(Anchor::TopLeft),
            window,
            cx,
        );
        div()
            .relative()
            .flex_none()
            .child(row_button(
                "menu",
                Icon::More,
                "Sessions menu",
                colors,
                |button| {
                    button
                        .when(open || filtered, |button| button.bg(colors.hover))
                        .on_click(cx.listener(|sidebar, _: &ClickEvent, window, cx| {
                            sidebar.menu.open((), window, cx)
                        }))
                },
            ))
            // The menu hangs from the button's bottom-left corner, out over the card.
            .children(menu.map(|menu| div().absolute().top_full().left_0().child(menu)))
    }

    fn menu_entries(&self, cx: &mut Context<Self>) -> Vec<Entry<Self>> {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let sort = workspace.sort();
        let scope = workspace.scope();
        let sorts = [
            ("sort-activity", "Last activity", SessionSort::LastActivity),
            ("sort-created", "Created", SessionSort::Created),
        ];
        let sort_name = sorts
            .iter()
            .find(|(_, _, each)| *each == sort)
            .map_or("", |(_, name, _)| *name);
        let sort_entries = sorts
            .into_iter()
            .map(|(id, name, each)| {
                choice(id, name, sort == each, move |workspace| {
                    workspace.set_sort(each)
                })
            })
            .collect();
        let mut project_entries = vec![choice(
            "scope-all",
            "All projects",
            scope == ProjectScope::All,
            |workspace| workspace.set_scope(ProjectScope::All),
        )];
        if let Some(computer) = workspace.computer() {
            let id = computer.id;
            project_entries.push(choice(
                "scope-none",
                NO_PROJECT,
                scope == ProjectScope::Project(id),
                move |workspace| workspace.set_scope(ProjectScope::Project(id)),
            ));
            project_entries.push(Entry::Separator);
        }
        for project in (workspace.projects().into_iter()).filter(|project| project.kind.is_folder())
        {
            let id = project.id;
            let name = SharedString::from(workspace.display_name(id).unwrap_or_default());
            project_entries.push(choice(
                ("scope", id.0),
                name,
                scope == ProjectScope::Project(id),
                move |workspace| workspace.set_scope(ProjectScope::Project(id)),
            ));
        }
        let project_name: SharedString = match scope {
            ProjectScope::All => "All".into(),
            ProjectScope::Project(id) => workspace.display_name(id).unwrap_or_default().into(),
        };
        let muted = colors.text_tertiary;
        vec![
            Entry::Item(
                Item::new("add-project", "Add project", |_, window, cx| {
                    window.dispatch_action(Box::new(crate::shell::AddProject), cx)
                })
                .icon(Icon::FolderAdd, muted),
            ),
            Entry::Separator,
            Entry::Item(
                Item::submenu("sort", "Sort", sort_name, sort_entries).icon(Icon::Sort, muted),
            ),
            Entry::Item(
                Item::submenu("project", "Project", project_name, project_entries)
                    .icon(Icon::Folder, muted),
            ),
        ]
    }
}

/// A choice in a submenu, checked at its end while it applies: picking it
/// applies it.
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
        .checked_at_end(checked),
    )
}
