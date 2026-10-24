//! A session row, three lines. Over, in small type, its project's icon and
//! name, and how long ago it was active; then its title; under, in small
//! type, its project's branch, and the faded logo of the agent it runs at
//! its end.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, Styled, Task, div, prelude::*, svg,
};
use theme::{
    ICON_SIZE_ROW, ROW_GAP, SESSION_LINE_HEIGHT, SESSION_ROW_HEIGHT, SESSION_ROW_INSET,
    SESSION_ROW_RADIUS, SESSION_ROW_SPACING, SPACE_1, SPACE_2, TEXT_SESSION_TITLE, TEXT_SMALL,
    Theme,
};
use workspace::{AgentId, Head, ProjectId, SessionId, Timestamp};

use super::{
    Sidebar,
    fade::Key,
    project_face::ProjectFace,
    project_icon::{self, ProjectIcon},
};
use crate::agents::logo;

/// What a session row shows, read from the workspace when it changes rather
/// than on every frame.
pub(super) struct Row {
    pub id: SessionId,
    pub title: SharedString,
    /// When the session was last active, and how long ago that reads: "now",
    /// "5m", "3h".
    pub activity: Timestamp,
    pub age: SharedString,
    pub agent: AgentId,
    pub project: ProjectFace,
}

/// A project's icon as last found: where, as of when, and drawn; `None` when
/// the file there didn't read as an icon. Kept between reads, so an icon
/// that didn't change is never drawn again.
pub(super) struct FoundIcon {
    path: PathBuf,
    modified: Option<SystemTime>,
    pub icon: Option<ProjectIcon>,
}

/// The narrowest the age's column gets, so a short age ("5m") and a long
/// one ("12h") leave the project's name the same room.
const AGE_MIN_WIDTH: gpui::Pixels = gpui::px(32.);

/// Height of a session row's branch line, under its title.
const BRANCH_LINE_HEIGHT: gpui::Pixels = gpui::px(16.);

/// How strongly the agent's logo shows: it marks the row without
/// competing with the title.
const LOGO_OPACITY: f32 = 0.6;

/// Longest the launch waits for the heads before showing the window without
/// them.
const LAUNCH_HEADS_WAIT: Duration = Duration::from_millis(50);

/// What the projects' folders hold that the sidebar shows: the head of each
/// checkout, and each project's own icon, for those that have them.
#[derive(Default)]
struct Folders {
    heads: HashMap<ProjectId, Head>,
    icons: HashMap<ProjectId, Arc<FoundIcon>>,
}

/// Reads the folders; an icon in `known` whose file is where it was, as of
/// when it was, is kept as it was drawn.
fn read_folders(
    projects: Vec<(ProjectId, PathBuf)>,
    known: HashMap<ProjectId, Arc<FoundIcon>>,
) -> Folders {
    let mut folders = Folders::default();
    for (id, path) in projects {
        if let Some(head) = workspace::head(&path) {
            folders.heads.insert(id, head);
        }
        let Some(icon_path) = workspace::project_icon(&path) else {
            continue;
        };
        let modified = std::fs::metadata(&icon_path)
            .and_then(|meta| meta.modified())
            .ok();
        let found = match known.get(&id) {
            Some(found) if found.path == icon_path && found.modified == modified => found.clone(),
            _ => Arc::new(FoundIcon {
                icon: project_icon::load(&icon_path),
                path: icon_path,
                modified,
            }),
        };
        folders.icons.insert(id, found);
    }
    folders
}

impl Sidebar {
    /// Reads the heads at launch, waiting up to `LAUNCH_HEADS_WAIT` so the
    /// first frame already shows the branches; a folder slow to answer, as on a
    /// network volume gone away, leaves the read to finish behind the window.
    pub(super) fn read_heads_now(&mut self, cx: &mut Context<Self>) {
        let projects = self.projects_to_head(cx);
        let known = self.icons.clone();
        let read = cx
            .background_executor()
            .spawn(async move { read_folders(projects, known) });
        match cx
            .foreground_executor()
            .block_with_timeout(LAUNCH_HEADS_WAIT, read)
        {
            Ok(folders) => self.take_folders(folders, cx),
            Err(read) => self.heads_task = self.take_heads(read, cx),
        }
    }

    /// Every folder project's folder, noting which projects the heads are
    /// read for. The computer has no head: its home folder is no checkout,
    /// even one that happens to be a repository.
    fn projects_to_head(&mut self, cx: &mut Context<Self>) -> Vec<(ProjectId, PathBuf)> {
        let workspace = self.store.read(cx).workspace();
        self.headed_projects = workspace.project_ids();
        workspace
            .projects()
            .into_iter()
            .filter(|project| project.kind.is_folder())
            .map(|project| (project.id, project.path.clone()))
            .collect()
    }

    /// The window came back to front: the clock may have run on while the Mac
    /// slept, so the ages are drawn again, and branches may have changed.
    pub fn window_returned(&mut self, cx: &mut Context<Self>) {
        self.stale = true;
        self.refresh_heads(cx);
        cx.notify();
    }

    /// Reads every project's head off the UI thread; the composer redraws if one
    /// changed. Runs when projects come and go, when the window comes to
    /// front, and when the ages tick, so a branch switched elsewhere shows.
    pub fn refresh_heads(&mut self, cx: &mut Context<Self>) {
        let projects = self.projects_to_head(cx);
        let known = self.icons.clone();
        let read = cx
            .background_executor()
            .spawn(async move { read_folders(projects, known) });
        self.heads_task = self.take_heads(read, cx);
    }

    /// Takes what `read` finds once it does: the composer redraws if a head
    /// changed, the rows if an icon did.
    fn take_heads(
        &mut self,
        read: impl Future<Output = Folders> + 'static,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |sidebar, cx| {
            let folders = read.await;
            sidebar
                .update(cx, |sidebar, cx| sidebar.take_folders(folders, cx))
                .ok();
        })
    }

    /// Hands the store the heads; the rows read again if an icon changed.
    fn take_folders(&mut self, folders: Folders, cx: &mut Context<Self>) {
        let same = |a: &HashMap<ProjectId, Arc<FoundIcon>>,
                    b: &HashMap<ProjectId, Arc<FoundIcon>>| {
            a.len() == b.len()
                && a.iter()
                    .all(|(id, found)| b.get(id).is_some_and(|other| Arc::ptr_eq(found, other)))
        };
        if !same(&self.icons, &folders.icons) {
            self.icons = folders.icons;
            self.stale = true;
            cx.notify();
        }
        (self.store).update(cx, |store, cx| store.set_heads(folders.heads, cx));
    }

    /// The row at `index`, `SESSION_ROW_SPACING` clear above and below it
    /// and `ROW_GAP` over the next. The title is `text_primary`; the project
    /// and the age `text_secondary`, and the branch `text_tertiary`, at rest;
    /// on the wash of the selected row, or the one under the pointer or the
    /// search highlight, both turn `text_secondary_on_surface`, which keeps
    /// its contrast there.
    pub(super) fn render_session_row(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let slot = div()
            .w_full()
            .h(SESSION_ROW_HEIGHT + SESSION_ROW_SPACING * 2. + ROW_GAP)
            .pt(SESSION_ROW_SPACING)
            .pb(SESSION_ROW_SPACING + ROW_GAP)
            .px(SPACE_2);
        let Some(row) = self.rows.get(index) else {
            return slot.into_any_element();
        };
        let id = row.id;
        let selected = self.drawn_selection == Some(id);
        let hovered = self.hover_target() == Some(index) || self.row_menu_session() == Some(id);
        let target = if selected {
            colors.row_selected
        } else if hovered {
            colors.hover
        } else {
            Hsla::transparent_black()
        };
        let fill = self.fills.at(Key::Session(id), target, Instant::now());
        let on_surface = selected || hovered;
        // Under the pointer the row's button fades in over the age as the
        // age fades out. The age keeps its column, so nothing reflows: the
        // project's name truncates the same either way.
        let pointed = self.pointer.row == Some(index);
        let shown = Hsla {
            a: if pointed { 1. } else { 0. },
            ..Hsla::default()
        };
        let action_opacity = self.fills.at(Key::Action(id), shown, Instant::now()).a;
        let action = (action_opacity > 0.).then(|| self.render_row_action(id, cx));
        let (secondary, faint) = if on_surface {
            let text = colors.text_secondary_on_surface;
            (text, text)
        } else {
            (colors.text_secondary, colors.text_tertiary)
        };
        let truncated = |text: SharedString| {
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(text)
        };
        let line_gap = SPACE_1 + SPACE_1 / 2.;
        let project = div()
            .h(SESSION_LINE_HEIGHT)
            .flex()
            .items_center()
            .gap(line_gap)
            .text_size(TEXT_SMALL)
            .text_color(secondary)
            .child(row.project.icon(&colors, secondary))
            .child(truncated(row.project.name.clone()).font_weight(FontWeight::MEDIUM))
            .child(
                div()
                    .min_w(AGE_MIN_WIDTH)
                    .flex_none()
                    .flex()
                    .justify_end()
                    .opacity(1. - action_opacity)
                    .child(row.age.clone()),
            );
        let title = div()
            .mt(SPACE_1)
            .h(SESSION_LINE_HEIGHT)
            .flex()
            .items_center()
            .text_size(TEXT_SESSION_TITLE)
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors.text_primary)
            .child(truncated(row.title.clone()));
        // A branch's two ends both say something, so it is cut in the middle.
        let branch = row.project.branch.clone().map(|branch| {
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_middle()
                .child(branch)
        });
        let details = div()
            .mt(SPACE_1 / 2.)
            .h(BRANCH_LINE_HEIGHT)
            .flex()
            .items_center()
            .gap(line_gap)
            .text_size(TEXT_SMALL)
            .text_color(faint)
            .children(branch)
            .child(
                div()
                    .ml_auto()
                    .flex_none()
                    .flex()
                    .children(logo(&row.agent, colors).map(|(icon, color)| {
                        svg()
                            .path(icon.path())
                            .size(ICON_SIZE_ROW)
                            .text_color(color.opacity(LOGO_OPACITY))
                    })),
            );
        slot.id(("session-slot", id.0))
            .on_hover(cx.listener(move |sidebar, hovered: &bool, _, cx| {
                sidebar.hover_row(index, *hovered, cx)
            }))
            .child(
                div()
                    .id(("session", id.0))
                    .relative()
                    .h(SESSION_ROW_HEIGHT)
                    .px(SESSION_ROW_INSET)
                    .py(SPACE_2)
                    .flex()
                    .flex_col()
                    .rounded(SESSION_ROW_RADIUS)
                    .bg(fill)
                    .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                        sidebar.open_session(id, cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |sidebar, event: &MouseDownEvent, window, cx| {
                            sidebar.open_row_menu(id, event.position, window, cx)
                        }),
                    )
                    .child(project)
                    .child(title)
                    .child(details)
                    .children(action.map(|action| {
                        // On the project's line, its icon's trailing edge
                        // on the content's, where the age ends.
                        div()
                            .absolute()
                            .top(SPACE_2)
                            .right(SESSION_ROW_INSET - (SESSION_LINE_HEIGHT - ICON_SIZE_ROW) / 2.)
                            .opacity(action_opacity)
                            .child(action)
                    })),
            )
            .into_any_element()
    }
}
