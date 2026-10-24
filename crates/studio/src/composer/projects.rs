//! The project over the box: a pill whose menu offers No project, searches
//! the folder projects, opens a draft in the one picked, and adds a new one.

use gpui::{
    Anchor, App, ClickEvent, Context, EventEmitter, Focusable, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::*, svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, HAIRLINE, ICON_SIZE, ICON_SIZE_SMALL, SPACE_2, SPACE_3, Theme,
};
use workspace::{NO_PROJECT, ProjectId, ProjectKind, Timestamp, matches_search};

use super::Composer;
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

    /// The project's icon, name and a chevron on the composer's fill
    /// under its hairline, lighter under the pointer and while the menu is
    /// open. The pill stands on the box's leading edge and pads its content
    /// as the box does, so its icon is in the column of the box's text. The
    /// menu hangs from that same edge, downward from a draft, which has the
    /// card below it, and upward from a docked composer, which sits at the
    /// card's bottom: three edges on one line, never two side by side.
    pub(super) fn render_project_pill(
        &self,
        icon: Icon,
        name: SharedString,
        draft: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let open = self.projects.opened().is_some();
        let menu = if open {
            let entries = self.project_entries(cx);
            let anchor = if draft {
                Anchor::TopLeft
            } else {
                Anchor::BottomLeft
            };
            (self.projects).render(
                "composer-project-menu",
                entries,
                Placement::Hang(anchor),
                window,
                cx,
            )
        } else {
            None
        };
        let pill = div()
            .id("composer-project")
            .group("composer-project")
            .relative()
            .min_w_0()
            .h(CONTROL_SIZE)
            .px(SPACE_3 - HAIRLINE)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(self.surfaces(cx).raised)
            // The wash goes over the fill, so the pill lightens as a
            // control does under the pointer.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(CONTROL_RADIUS)
                    .when(open, |wash| wash.bg(colors.hover))
                    .group_hover("composer-project", |wash| wash.bg(colors.hover)),
            )
            .on_click(cx.listener(|composer, _: &ClickEvent, window, cx| {
                let text = composer.input.focus_handle(cx);
                composer.projects.open_from((), Some(text), window, cx);
            }))
            .child(
                svg()
                    .path(icon.path())
                    .flex_none()
                    .size(ICON_SIZE)
                    .text_color(colors.text_secondary),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(name),
            )
            .child(
                svg()
                    .path(Icon::ChevronDown.path())
                    .flex_none()
                    .size(ICON_SIZE_SMALL)
                    .text_color(colors.text_secondary),
            );
        div()
            .relative()
            .min_w_0()
            .child(pill)
            .children(menu.map(|menu| {
                let hang = div().absolute().left_0();
                match draft {
                    true => hang.bottom_0(),
                    false => hang.top_0(),
                }
                .child(menu)
            }))
    }
}
