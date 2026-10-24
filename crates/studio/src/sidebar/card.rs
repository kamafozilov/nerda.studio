//! A session row, two lines. Over, in small type, its project's icon and
//! name, and how long ago it was active; under, its title from the row's
//! leading edge, and the logo of the agent it runs at its end. The icon is
//! the project's own favicon or logo where it keeps one, else a folder in the
//! project's hue; the computer's is its icon.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use gpui::{
    AnyElement, ClickEvent, Context, Hsla, IntoElement, MouseButton, MouseDownEvent, ObjectFit,
    ParentElement, SharedString, Styled, StyledImage, Task, div, img, prelude::*, svg,
};
use theme::{
    CONTROL_RADIUS, Colors, ICON_SIZE_SMALL, PROJECT_ICON, PROJECT_ICON_RADIUS, ROW_GAP,
    SESSION_ROW_HEIGHT, SPACE_1, SPACE_2, TEXT_CAPTION, Theme,
};
use workspace::{
    AgentId, Head, Project, ProjectId, ProjectKind, Session, SessionId, Timestamp, compact_age,
};

use super::{
    Sidebar,
    fade::Key,
    project_icon::{self, ProjectIcon, Tone},
};
use crate::{agents::logo, assets::Icon};

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

/// A project as its sessions' rows show it, worked out once per refresh and
/// shared by its rows: its name as the sidebar shows it, and its icon.
#[derive(Clone)]
pub(super) enum ProjectFace {
    /// A folder: its own icon if it keeps one, and its hue.
    Folder {
        name: SharedString,
        icon: Option<Arc<FoundIcon>>,
        hue: usize,
    },
    /// The computer: no folder to draw or color it by.
    Computer { name: SharedString },
}

impl ProjectFace {
    pub fn new(project: &Project, name: String, icon: Option<Arc<FoundIcon>>) -> Self {
        let name = name.into();
        match project.kind {
            ProjectKind::Folder => Self::Folder {
                name,
                icon,
                hue: hue(&project.path),
            },
            ProjectKind::Computer => Self::Computer { name },
        }
    }

    /// The icon before the project's name in its `PROJECT_ICON` slot: the
    /// project's own, with `PROJECT_ICON_RADIUS` corners, set on a plate of
    /// the opposite tone when its mark would vanish on the glass; else a
    /// folder at `ICON_SIZE_SMALL` in the hue of its badge's initials, which
    /// reads on the glass and on a row's surface; the computer the same size
    /// in `secondary`, the line's color.
    pub fn icon(&self, colors: &Colors, secondary: Hsla) -> AnyElement {
        let slot = || {
            div()
                .size(PROJECT_ICON)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
        };
        let glyph = |icon: Icon, color: Hsla| {
            slot()
                .child(
                    svg()
                        .path(icon.path())
                        .size(ICON_SIZE_SMALL)
                        .text_color(color),
                )
                .into_any_element()
        };
        match self {
            Self::Folder { icon, hue, .. } => {
                match icon.as_ref().and_then(|found| found.icon.as_ref()) {
                    Some(icon) => own_icon(slot(), icon, colors),
                    None => glyph(Icon::Folder, colors.badges[hue % colors.badges.len()].text),
                }
            }
            Self::Computer { .. } => glyph(Icon::Computer, secondary),
        }
    }

    pub fn name(&self) -> SharedString {
        match self {
            Self::Folder { name, .. } | Self::Computer { name } => name.clone(),
        }
    }

    pub fn row(&self, session: &Session, now: Timestamp) -> Row {
        Row {
            id: session.id,
            title: session.display_title().into(),
            activity: session.activity(),
            age: compact_age(session.activity(), now).into(),
            agent: session.choice.agent.clone(),
            project: self.clone(),
        }
    }
}

/// A project's own icon in its slot. A dark mark on the dark glass, or a
/// light one on the light, stands on a plate of `text_primary`, which is
/// the opposite tone, at 90%, inset by `PLATE_INSET` so its corners show.
fn own_icon(slot: gpui::Div, icon: &ProjectIcon, colors: &Colors) -> AnyElement {
    let dark_glass = colors.text_primary.l > 0.5;
    let plated = match icon.tone {
        Tone::Dark => dark_glass,
        Tone::Light => !dark_glass,
        Tone::Own => false,
    };
    let picture = img(icon.image.clone()).object_fit(ObjectFit::Contain);
    if plated {
        slot.rounded(PROJECT_ICON_RADIUS)
            .bg(colors.text_primary.opacity(0.9))
            .child(picture.size(PROJECT_ICON - PLATE_INSET * 2.))
            .into_any_element()
    } else {
        slot.child(picture.size_full().rounded(PROJECT_ICON_RADIUS))
            .into_any_element()
    }
}

/// How far a plated icon stands inside its plate.
const PLATE_INSET: gpui::Pixels = gpui::px(2.);

/// A project's icon as last found: where, as of when, and drawn; `None` when
/// the file there didn't read as an icon. Kept between reads, so an icon
/// that didn't change is never drawn again.
pub(super) struct FoundIcon {
    path: PathBuf,
    modified: Option<SystemTime>,
    icon: Option<ProjectIcon>,
}

/// A project's hue, from its folder, so it keeps it across launches.
fn hue(path: &Path) -> usize {
    // FNV-1a: stable across builds, unlike the standard library's hasher.
    let hash = path
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    (hash % 8) as usize
}

/// The column a session row keeps at its trailing edge for its age and logo.
/// The button under the pointer, `CONTROL_SIZE` (28) and `SPACE_1` clear of
/// the row's edge, covers it and the `SPACE_2` padding beside it, less
/// `SPACE_1`: 28 - 4.
const TRAILING_WIDTH: gpui::Pixels = gpui::px(24.);

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

    /// The row at `index`, `ROW_GAP` over the next. The selected row and the
    /// one under the pointer or the search highlight take their wash and a
    /// title in `text_primary`, and their project and age `text_secondary`,
    /// which keeps its contrast there.
    pub(super) fn render_session_row(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let slot = div()
            .w_full()
            .h(SESSION_ROW_HEIGHT + ROW_GAP)
            .pb(ROW_GAP)
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
        // Under the pointer the row's button fades in over its trailing
        // edge as the age and the logo fade out. They share one reserved
        // column, so nothing reflows: the title truncates the same either way.
        let pointed = self.pointer.row == Some(index);
        let shown = Hsla {
            a: if pointed { 1. } else { 0. },
            ..Hsla::default()
        };
        let action_opacity = self.fills.at(Key::Action(id), shown, Instant::now()).a;
        let action = (action_opacity > 0.).then(|| self.render_row_action(id, cx));
        let trailing = |child: AnyElement| {
            div()
                .w(TRAILING_WIDTH)
                .flex_none()
                .flex()
                .justify_end()
                .opacity(1. - action_opacity)
                .child(child)
        };
        let (primary, secondary) = if on_surface {
            (colors.text_primary, colors.text_secondary)
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
        let project = div()
            .flex()
            .items_center()
            .gap(SPACE_1 + SPACE_1 / 2.)
            .text_size(TEXT_CAPTION)
            .text_color(secondary)
            .child(row.project.icon(&colors, secondary))
            .child(truncated(row.project.name()))
            .child(trailing(row.age.clone().into_any_element()));
        let title = div()
            .flex()
            .items_center()
            .gap(SPACE_2)
            .text_color(primary)
            .child(truncated(row.title.clone()))
            .child(trailing(
                div()
                    .children(logo(&row.agent, colors).map(|(icon, color)| {
                        svg()
                            .path(icon.path())
                            .size(ICON_SIZE_SMALL)
                            .text_color(color)
                    }))
                    .into_any_element(),
            ));
        slot.id(("session-slot", id.0))
            .on_hover(cx.listener(move |sidebar, hovered: &bool, _, cx| {
                sidebar.hover_row(index, *hovered, cx)
            }))
            .child(
                div()
                    .id(("session", id.0))
                    .relative()
                    .h(SESSION_ROW_HEIGHT)
                    .px(SPACE_2)
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(SPACE_1 / 2.)
                    .rounded(CONTROL_RADIUS)
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
                    .children(action.map(|action| {
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(SPACE_1)
                            .flex()
                            .items_center()
                            .opacity(action_opacity)
                            .child(action)
                    })),
            )
            .into_any_element()
    }
}
