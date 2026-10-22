//! The list by project: each project a header that folds its sessions away, the
//! guide its sessions hang from, and the rows showing more or fewer of them.

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, div, prelude::*, px, svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, Colors, ICON_SIZE, ROW_GAP, ROW_HEIGHT, SPACE_1, SPACE_2, Theme,
};
use workspace::{ProjectId, Workspace};

use super::{Row, Sidebar, header::icon_button};
use crate::assets::Icon;

/// Sessions a project lists before "Show more", newest first under the sort.
const LATEST_SESSIONS: usize = 5;

/// The slot a list row fills: it spans the list inside the side padding and is
/// `ROW_GAP` taller than the row's `ROW_HEIGHT` surface, which it centers, so
/// neighboring surfaces stand apart while a project's guide runs through the gap.
pub(super) fn row_slot() -> Div {
    div()
        .w_full()
        .h(ROW_HEIGHT + ROW_GAP)
        .px(SPACE_2)
        .flex()
        .items_center()
}

/// The column a project's sessions hang in: as wide as the header's inset and
/// icon, with a hairline under the icon's center.
pub(super) fn indent_guide(colors: Colors) -> impl IntoElement {
    div()
        .flex_none()
        .w(SPACE_1 + ICON_SIZE)
        .h_full()
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(SPACE_1 + ICON_SIZE / 2. - px(0.5))
                .w(px(1.))
                .bg(colors.border),
        )
}

impl Sidebar {
    /// Fills `rows` for the list by project: each header, then unless it is folded
    /// its latest sessions and a row to show more or fewer.
    pub(super) fn push_project_rows(&mut self, workspace: &Workspace) {
        let searching = self.searching();
        let selected = workspace.selected_session();
        for group in workspace.session_groups(&self.query) {
            let project = group.project.id;
            let total = group.sessions.len();
            self.rows.push(Row::Project(project, total));
            if group.collapsed {
                continue;
            }
            // The selection always shows, so the list never hides what the main area has open.
            let selection_beyond = group
                .sessions
                .iter()
                .skip(LATEST_SESSIONS)
                .any(|session| Some(session.id) == selected);
            let all = searching || selection_beyond || self.show_all.contains(&project);
            let shown = if all {
                total
            } else {
                total.min(LATEST_SESSIONS)
            };
            self.rows.extend(
                group.sessions[..shown]
                    .iter()
                    .map(|session| Row::Session(session.id)),
            );
            if shown < total {
                self.rows.push(Row::ShowMore(project, total - shown));
            } else if total > LATEST_SESSIONS
                && self.show_all.contains(&project)
                && !searching
                && !selection_beyond
            {
                self.rows.push(Row::ShowLess(project));
            }
        }
    }

    /// A project's header. At rest a folder and the name, dimmed with its session
    /// count while folded; under the pointer the folder turns into the chevron that
    /// clicking acts on, and the count into a New session button for the project.
    pub(super) fn render_project_row(
        &self,
        project: ProjectId,
        sessions: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let Some(name) = workspace.display_name(project) else {
            return row_slot().into_any_element();
        };
        // A search unfolds every project and folding waits for it to clear, so
        // the header offers no chevron meanwhile.
        let searching = self.searching();
        let collapsed = !searching && workspace.is_collapsed(project);
        let group = SharedString::from(format!("project-{}", project.0));
        let chevron = if collapsed {
            Icon::Collapsed
        } else {
            Icon::Expanded
        };
        let icon = |icon: Icon| {
            svg()
                .path(icon.path())
                .size(ICON_SIZE)
                .text_color(colors.text_secondary)
        };
        let leading = div()
            .relative()
            .flex_none()
            .size(ICON_SIZE)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .when(!searching, |slot| {
                        slot.group_hover(group.clone(), |slot| slot.invisible())
                    })
                    .child(icon(Icon::Folder)),
            )
            .when(!searching, |slot| {
                slot.child(
                    div()
                        .absolute()
                        .inset_0()
                        .invisible()
                        .group_hover(group.clone(), |slot| slot.visible())
                        .child(icon(chevron)),
                )
            });
        let new_session = icon_button(
            format!("new-session-in-{}", project.0),
            Icon::NewSession,
            format!("New session in {name}"),
            true,
            colors,
            |button| {
                button.on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    sidebar.new_session_in(project, cx);
                }))
            },
        );
        let trailing = div()
            .relative()
            .flex_none()
            .h_full()
            .min_w(CONTROL_SIZE)
            .flex()
            .items_center()
            .justify_end()
            .when(collapsed, |slot| {
                slot.child(
                    div()
                        .pr(SPACE_2)
                        .text_color(colors.text_secondary)
                        .group_hover(group.clone(), |count| count.invisible())
                        .child(sessions.to_string()),
                )
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .invisible()
                    .group_hover(group.clone(), |slot| slot.visible())
                    .child(new_session),
            );
        row_slot()
            .child(
                div()
                    .id(("project", project.0))
                    .group(group.clone())
                    .w_full()
                    .h(ROW_HEIGHT)
                    .pl(SPACE_1)
                    .flex()
                    .items_center()
                    .gap(SPACE_2)
                    .rounded(CONTROL_RADIUS)
                    .when(!searching, |row| row.hover(|row| row.bg(colors.hover)))
                    .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                        sidebar.toggle_project(project, cx)
                    }))
                    .child(leading)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if collapsed {
                                colors.text_secondary
                            } else {
                                colors.text_primary
                            })
                            .child(name),
                    )
                    .child(trailing),
            )
            .into_any_element()
    }

    /// "Show 12 more" under a project's latest sessions, or "Show less" under all of them.
    pub(super) fn render_show_row(
        &self,
        project: ProjectId,
        hidden: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let label = match hidden {
            Some(count) => format!("Show {count} more"),
            None => "Show less".to_owned(),
        };
        let id = if hidden.is_some() {
            "show-more"
        } else {
            "show-less"
        };
        row_slot()
            .child(indent_guide(colors))
            .child(
                div()
                    .id((id, project.0))
                    .flex_1()
                    .min_w_0()
                    .h(ROW_HEIGHT)
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .rounded(CONTROL_RADIUS)
                    .text_color(colors.text_secondary)
                    .hover(|row| row.bg(colors.hover))
                    .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                        if hidden.is_some() {
                            sidebar.show_all.insert(project);
                        } else {
                            sidebar.show_all.remove(&project);
                        }
                        cx.notify();
                    }))
                    .child(label),
            )
            .into_any_element()
    }
}
