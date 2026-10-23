//! A session row: a card of three lines. Its project's badge, name and how long
//! ago it was active; its title; then where it runs, the project's branch, or
//! its folder outside git. The badge's hue and initials tell projects apart at a
//! glance, and two projects of the same name by their names.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    Task, div, prelude::*, px, svg,
};
use theme::{
    BADGE_SIZE, BADGE_TEXT, CARD_LINE, CARD_LINE_GAP, CARD_TITLE_LINE, ICON_SIZE_SMALL, ROW_GAP,
    SESSION_ROW_HEIGHT, SPACE_1, SPACE_2, TEXT_SMALL, Theme,
};
use workspace::{Head, Project, ProjectId, Session, SessionId, Timestamp, compact_age};

use super::Sidebar;
use crate::assets::Icon;

/// What a session row shows, read from the workspace when it changes rather
/// than on every frame.
pub(super) struct Row {
    pub id: SessionId,
    pub title: SharedString,
    /// When the session was last active, and how long ago that reads: "now",
    /// "5m", "3h".
    pub activity: Timestamp,
    pub age: SharedString,
    pub project: ProjectFace,
}

/// A project as its sessions' cards show it, worked out once per refresh and
/// shared by its cards.
#[derive(Clone)]
pub(super) struct ProjectFace {
    pub name: SharedString,
    /// The name is the end of the project's path ("north/web"), telling it
    /// apart from another project of the same name, so it is cut at its start.
    pub name_is_path: bool,
    /// The badge's initials and hue.
    pub initials: SharedString,
    pub hue: usize,
    pub place: Place,
}

/// Where a project's sessions run.
#[derive(Clone)]
pub(super) enum Place {
    /// The branch its checkout is on, or the commit when on none; cut in the
    /// middle, keeping its prefix and its end.
    Head(SharedString),
    /// Its folder, outside git, with the home folder as `~`; cut at its
    /// start, keeping the folder's own name.
    Folder(SharedString),
}

impl ProjectFace {
    pub fn new(project: &Project, name: String, head: Option<&Head>) -> Self {
        let place = match head {
            Some(Head::Branch(name) | Head::Detached(name)) => Place::Head(name.clone().into()),
            None => Place::Folder(workspace::home_relative(&project.path).into()),
        };
        Self {
            initials: initials(&project.title).into(),
            hue: hue(&project.path),
            name_is_path: name != project.title,
            name: name.into(),
            place,
        }
    }

    pub fn row(&self, session: &Session, now: Timestamp) -> Row {
        Row {
            id: session.id,
            title: session.display_title().into(),
            activity: session.activity(),
            age: compact_age(session.activity(), now).into(),
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
            Ok(heads) => self.heads = heads,
            Err(read) => self.heads_task = self.take_heads(read, cx),
        }
    }

    /// Every project's folder, noting which projects the heads are read for.
    fn projects_to_head(&mut self, cx: &mut Context<Self>) -> Vec<(ProjectId, PathBuf)> {
        let workspace = self.store.read(cx).workspace();
        self.headed_projects = workspace.project_ids();
        workspace
            .projects()
            .into_iter()
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

    /// Reads every project's head off the UI thread; the cards redraw if one
    /// changed. Runs when projects come and go, when the window comes to
    /// front, and when the ages tick, so a branch switched elsewhere shows.
    pub fn refresh_heads(&mut self, cx: &mut Context<Self>) {
        let projects = self.projects_to_head(cx);
        let read = cx
            .background_executor()
            .spawn(async move { read_heads(projects) });
        self.heads_task = self.take_heads(read, cx);
    }

    /// Takes the heads `read` finds once it does; the cards redraw if one
    /// changed.
    fn take_heads(
        &mut self,
        read: impl Future<Output = HashMap<ProjectId, Head>> + 'static,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |sidebar, cx| {
            let heads = read.await;
            sidebar
                .update(cx, |sidebar, cx| {
                    if sidebar.heads != heads {
                        sidebar.heads = heads;
                        sidebar.stale = true;
                        cx.notify();
                    }
                })
                .ok();
        })
    }

    /// The card for row `index`. Its slot spans the list, `ROW_GAP` taller than
    /// the card, and tracks the pointer for the hover surface; the surfaces
    /// themselves are the list's.
    pub(super) fn render_session_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let slot = div()
            .w_full()
            .h(SESSION_ROW_HEIGHT + ROW_GAP)
            .px(SPACE_2)
            .flex()
            .items_center();
        let Some(row) = self.rows.get(index) else {
            return slot.into_any_element();
        };
        let id = row.id;
        let now = std::time::Instant::now();
        let shift = self.shifts.offset(id, now);
        let opacity = self.shifts.opacity(id, now);
        let face = &row.project;
        let badge = colors.badges[face.hue % colors.badges.len()];
        let truncated = || div().overflow_hidden().whitespace_nowrap().text_ellipsis();
        // On a surface, the supporting lines take the color that keeps their
        // contrast there; the selection and the hover redraw the card anyway.
        let on_surface = self.drawn_selection == Some(id) || self.hover_target() == Some(index);
        let secondary = if on_surface {
            colors.text_secondary_on_surface
        } else {
            colors.text_secondary
        };
        let (place_icon, place) = match &face.place {
            Place::Head(name) => (
                Icon::GitBranch,
                truncated().text_ellipsis_middle().child(name.clone()),
            ),
            Place::Folder(path) => (
                Icon::Folder,
                truncated().text_ellipsis_start().child(path.clone()),
            ),
        };
        // The badge and the place's icon share a column, so the project's name
        // and the place line up after them.
        let line = |lead: AnyElement| {
            div()
                .h(CARD_LINE)
                .flex()
                .items_center()
                .gap(SPACE_2)
                .text_size(TEXT_SMALL)
                .line_height(CARD_LINE)
                .text_color(secondary)
                .child(
                    div()
                        .size(BADGE_SIZE)
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(lead),
                )
        };
        slot.id(("session-slot", id.0))
            .on_hover(cx.listener(move |sidebar, hovered: &bool, _, cx| {
                sidebar.hover_row(index, *hovered, cx)
            }))
            .child(
                div()
                    .id(("session", id.0))
                    .flex_1()
                    .min_w_0()
                    .h(SESSION_ROW_HEIGHT)
                    .p(SPACE_2)
                    .flex()
                    .flex_col()
                    // Mid-glide, the card is drawn off its slot, which keeps the
                    // pointer's place; see `shift`.
                    .when(shift != 0., |card| card.relative().top(px(shift)))
                    .when(opacity < 1., |card| card.opacity(opacity))
                    .on_click(cx.listener(move |sidebar, _: &ClickEvent, _, cx| {
                        sidebar.open_session(id, cx)
                    }))
                    .child(
                        line(
                            div()
                                .size(BADGE_SIZE)
                                .rounded(SPACE_1)
                                .bg(badge.fill)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(BADGE_TEXT)
                                .line_height(BADGE_SIZE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(badge.text)
                                .child(face.initials.clone())
                                .into_any_element(),
                        )
                        .child(
                            truncated()
                                .when(face.name_is_path, |name| name.text_ellipsis_start())
                                .flex_1()
                                .min_w_0()
                                .child(face.name.clone()),
                        )
                        .child(div().flex_none().child(row.age.clone())),
                    )
                    .child(
                        truncated()
                            .mt(SPACE_1)
                            .h(CARD_TITLE_LINE)
                            .line_height(CARD_TITLE_LINE)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors.text_primary)
                            .child(row.title.clone()),
                    )
                    .child(
                        line(
                            svg()
                                .path(place_icon.path())
                                .size(ICON_SIZE_SMALL)
                                .text_color(secondary)
                                .into_any_element(),
                        )
                        .mt(CARD_LINE_GAP)
                        .child(place.flex_1().min_w_0()),
                    ),
            )
            .into_any_element()
    }
}
