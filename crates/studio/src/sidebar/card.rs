//! A session row, two lines. Over, in small type, its project's badge and
//! name, and how long ago it was active; under, its title from the row's
//! leading edge, and the logo of the agent it runs at its end. The badge's
//! hue and initials tell projects apart at a glance; the computer's badge is
//! its icon.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, Styled, Task, div, prelude::*, svg,
};
use theme::{
    BADGE_ICON, BADGE_RADIUS, BADGE_SIZE, BADGE_TEXT, CONTROL_RADIUS, CONTROL_SIZE, Colors,
    ICON_SIZE_SMALL, ROW_GAP, SESSION_ROW_HEIGHT, SPACE_1, SPACE_2, TEXT_CAPTION, Theme,
};
use workspace::{
    AgentId, Head, Project, ProjectId, ProjectKind, Session, SessionId, Timestamp, compact_age,
};

use super::{Sidebar, fade::Key};
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
/// shared by its rows: its name as the sidebar shows it, and its badge.
#[derive(Clone)]
pub(super) enum ProjectFace {
    /// A folder's initials, in its hue.
    Folder {
        name: SharedString,
        initials: SharedString,
        hue: usize,
    },
    /// The computer's icon on a neutral fill: no folder to name or color it by.
    Computer { name: SharedString },
}

impl ProjectFace {
    pub fn new(project: &Project, name: String) -> Self {
        let name = name.into();
        match project.kind {
            ProjectKind::Folder => Self::Folder {
                name,
                initials: initials(&project.title).into(),
                hue: hue(&project.path),
            },
            ProjectKind::Computer => Self::Computer { name },
        }
    }

    /// The badge, `BADGE_SIZE` with `BADGE_RADIUS` corners.
    fn badge(&self, colors: &Colors) -> AnyElement {
        let badge = div()
            .size(BADGE_SIZE)
            .flex_none()
            .rounded(BADGE_RADIUS)
            .flex()
            .items_center()
            .justify_center();
        match self {
            Self::Folder { initials, hue, .. } => {
                let face = colors.badges[hue % colors.badges.len()];
                badge
                    .bg(face.fill)
                    .text_size(BADGE_TEXT)
                    .line_height(BADGE_SIZE)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(face.text)
                    .child(initials.clone())
                    .into_any_element()
            }
            Self::Computer { .. } => badge
                .bg(colors.key_background)
                .child(
                    svg()
                        .path(Icon::Computer.path())
                        .size(BADGE_ICON)
                        .text_color(colors.text_secondary),
                )
                .into_any_element(),
        }
    }

    fn name(&self) -> SharedString {
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

/// Up to two letters for a badge: the first of each of the name's first two
/// words ("arcada.next" is AN), or the first two of a one-word name.
fn initials(title: &str) -> String {
    // An apostrophe is part of the letter before it, as in the Uzbek oʻ:
    // "oʻquv" is OQ, not Oʻ.
    let title: String = title
        .chars()
        .filter(|c| !matches!(c, 'ʻ' | 'ʼ' | '\'' | '‘' | '’' | '`'))
        .collect();
    let mut words = title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty());
    let letters: String = match (words.next(), words.next()) {
        (Some(first), Some(second)) => first
            .chars()
            .take(1)
            .chain(second.chars().take(1))
            .collect(),
        (Some(only), None) => only.chars().take(2).collect(),
        _ => title.chars().take(1).collect(),
    };
    letters.to_uppercase()
}

/// A project's badge hue, from its folder, so it keeps it across launches.
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

/// Longest the launch waits for the heads before showing the window without
/// them.
const LAUNCH_HEADS_WAIT: Duration = Duration::from_millis(50);

/// The head of each project's checkout, for those in one.
fn read_heads(projects: Vec<(ProjectId, PathBuf)>) -> HashMap<ProjectId, Head> {
    projects
        .into_iter()
        .filter_map(|(id, path)| workspace::head(&path).map(|head| (id, head)))
        .collect()
}

impl Sidebar {
    /// Reads the heads at launch, waiting up to `LAUNCH_HEADS_WAIT` so the
    /// first frame already shows the branches; a folder slow to answer, as on a
    /// network volume gone away, leaves the read to finish behind the window.
    pub(super) fn read_heads_now(&mut self, cx: &mut Context<Self>) {
        let projects = self.projects_to_head(cx);
        let read = cx
            .background_executor()
            .spawn(async move { read_heads(projects) });
        match cx
            .foreground_executor()
            .block_with_timeout(LAUNCH_HEADS_WAIT, read)
        {
            Ok(heads) => self
                .store
                .update(cx, |store, cx| store.set_heads(heads, cx)),
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
        let read = cx
            .background_executor()
            .spawn(async move { read_heads(projects) });
        self.heads_task = self.take_heads(read, cx);
    }

    /// Hands the store the heads `read` finds once it does; the composer redraws
    /// if one changed.
    fn take_heads(
        &mut self,
        read: impl Future<Output = HashMap<ProjectId, Head>> + 'static,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |sidebar, cx| {
            let heads = read.await;
            sidebar
                .update(cx, |sidebar, cx| {
                    sidebar
                        .store
                        .update(cx, |store, cx| store.set_heads(heads, cx))
                })
                .ok();
        })
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
        // Under the pointer the row's button takes its trailing edge, where
        // the age and the logo stood.
        let action = (self.pointer.row == Some(index)).then(|| self.render_row_action(id, cx));
        let acting = action.is_some();
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
            .gap(SPACE_2)
            .text_size(TEXT_CAPTION)
            .text_color(secondary)
            .child(row.project.badge(&colors))
            .child(truncated(row.project.name()))
            .when(!acting, |line| {
                line.child(div().flex_none().child(row.age.clone()))
            });
        let title = div()
            .flex()
            .items_center()
            .gap(SPACE_2)
            .text_color(primary)
            .child(truncated(row.title.clone()))
            .children(
                logo(&row.agent, colors)
                    .filter(|_| !acting)
                    .map(|(icon, color)| {
                        svg()
                            .path(icon.path())
                            .flex_none()
                            .size(ICON_SIZE_SMALL)
                            .text_color(color)
                    }),
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
                    .pl(SPACE_2)
                    .pr(if acting {
                        CONTROL_SIZE + SPACE_1
                    } else {
                        SPACE_2
                    })
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
                            .child(action)
                    })),
            )
            .into_any_element()
    }
}
