//! The draft's project, under the box: a control whose menu offers No project, searches
//! the folder projects, opens a draft in the one picked, and adds a new one.

use gpui::{
    Anchor, App, ClickEvent, Context, EventEmitter, Focusable, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::*,
};
use theme::Theme;
use workspace::{NO_PROJECT, ProjectId, ProjectKind, Timestamp, matches_search};

use super::{Composer, context::Side};
use crate::{
    assets::Icon,
    menu::{Entry, Item, Placement},
    shell::AddProject,
};

/// Asks for a draft in another project: its own, reopened if it has one.
pub struct OpenProject(pub ProjectId);

impl EventEmitter<OpenProject> for Composer {}

impl Composer {
    /// No project, then the folder projects, each while it matches the
    /// search, the session's checked at the end; then New project.
    fn project_entries(&self, cx: &App) -> Vec<Entry<Self>> {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let current = workspace
            .session(self.session)
            .map(|session| session.project);
        let on_computer = current
            .and_then(|id| workspace.project(id))
            .is_some_and(|project| project.kind == ProjectKind::Computer);
        let query = self.projects.query(cx);
        let mut entries: Vec<Entry<Self>> = Vec::new();
        if matches_search(NO_PROJECT, &query) {
            let pick =
                move |composer: &mut Composer, _: &mut Window, cx: &mut Context<Composer>| {
                    if on_computer {
                        return;
                    }
                    let computer = composer.store.update(cx, |store, cx| {
                        store.update(cx, |workspace| workspace.computer_project(Timestamp::now()))
                    });
                    if let Some(id) = computer {
                        cx.emit(OpenProject(id));
                    }
                };
            entries.push(Entry::Item(
                Item::new("project-none", NO_PROJECT, pick)
                    .icon(Icon::Computer, colors.text_secondary)
                    .checked_at_end(on_computer),
            ));
        }
        let folders: Vec<Entry<Self>> = (workspace.matching_projects(&query).into_iter())
            .map(|project| {
                let id = project.id;
                let name = workspace
                    .display_name(id)
                    .unwrap_or_else(|| project.title.clone());
                let pick = move |_: &mut Composer, _: &mut Window, cx: &mut Context<Composer>| {
                    if current != Some(id) {
                        cx.emit(OpenProject(id));
                    }
                };
                Entry::Item(
                    Item::new(("project", id.0), name, pick)
                        .icon(Icon::Folder, colors.text_secondary)
                        .checked_at_end(current == Some(id)),
                )
            })
            .collect();
        if !entries.is_empty() && !folders.is_empty() {
            entries.push(Entry::Separator);
        }
        entries.extend(folders);
        if entries.is_empty() {
            entries.push(Entry::Section("No matching projects".into()));
        }
        entries.push(Entry::Separator);
        entries.push(Entry::Item(
            Item::new("project-new", "New project", |_, window, cx| {
                window.dispatch_action(Box::new(AddProject), cx)
            })
            .icon(Icon::NewTab, colors.text_secondary),
        ));
        entries
    }

    /// The drawer's leading side: the project's icon, name and a chevron.
    /// Its menu hangs downward from its leading edge.
    pub(super) fn render_project_pill(
        &self,
        icon: Icon,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let open = self.projects.opened().is_some();
        let menu = if open {
            let entries = self.project_entries(cx);
            (self.projects).render(
                "composer-project-menu",
                entries,
                Placement::Hang(Anchor::TopLeft),
                window,
                cx,
            )
        } else {
            None
        };
        let side = Side {
            id: "composer-project",
            icon,
            label: name,
            cut_middle: false,
            open,
        }
        .render(&colors)
        .on_click(cx.listener(|composer, _: &ClickEvent, window, cx| {
            let text = composer.input.focus_handle(cx);
            composer.projects.open_from((), Some(text), window, cx);
        }));
        div()
            .relative()
            .min_w_0()
            .child(side)
            .children(menu.map(|menu| div().absolute().left_0().bottom_0().child(menu)))
    }
}
