//! Where a draft runs, in a drawer under the box: its project on the
//! leading side, and the branch its checkout is on, or the commit, on the
//! trailing side. The drawer hangs from the box's bottom edge, narrower by
//! `DRAWER_INSET` on each side so it starts past the box's rounded corners,
//! its sides and bottom under the box's hairline and its bottom corners
//! rounded as the box's are; the box's own bottom hairline is the seam.
//! Its fill is the card's, or the box's glass over a pattern or an image, so
//! its small words read wherever it stands. On a dark card a faint shade
//! falls from the seam into it, so it reads as tucked under the box.
//!
//! Each side is a control, small `text_secondary` words after an
//! `ICON_SIZE_ROW` icon and a chevron, washed with `hover` under the pointer
//! and while its menu is open. Once the session has seen work, where it
//! runs is settled: the drawer goes, with both choices.

use gpui::{
    AnyElement, Div, ElementId, Hsla, IntoElement, ParentElement, Pixels, SharedString, Stateful,
    Styled, Window, div, linear_color_stop, linear_gradient, prelude::*, px, svg,
};
use theme::{
    COMPOSER_RADIUS, CONTROL_RADIUS, FIELD_TARGET_SIZE, ICON_SIZE_ROW, ICON_SIZE_SMALL, SPACE_1,
    SPACE_2, TEXT_SMALL, Theme,
};
use workspace::ProjectKind;

use super::Composer;
use crate::assets::Icon;

/// How much narrower than the box the drawer is on each side: past the
/// box's corner radius, so the drawer meets a straight edge.
const DRAWER_INSET: Pixels = px(20.);

/// How far the shade under the seam reaches into the drawer, and how dark
/// it starts.
const SEAM_SHADE: Pixels = px(8.);
const SEAM_SHADE_OPACITY: f32 = 0.18;

/// One side of the drawer: a control that opens its menu.
pub(super) struct Side {
    pub id: &'static str,
    pub icon: Icon,
    pub label: SharedString,
    /// A branch is cut in the middle, where its two ends both say something.
    pub cut_middle: bool,
    /// Its menu is open: the wash stays.
    pub open: bool,
}

impl Side {
    /// The side drawn: its wash, icon, words and chevron. The caller adds
    /// the click.
    pub fn render(self, colors: &theme::Colors) -> Stateful<Div> {
        let text = colors.text_secondary;
        let label = div()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .child(self.label);
        let label = if self.cut_middle {
            label.text_ellipsis_middle()
        } else {
            label.text_ellipsis()
        };
        div()
            .id(ElementId::Name(self.id.into()))
            .group(self.id)
            .relative()
            .min_w_0()
            .h(FIELD_TARGET_SIZE)
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_1 + SPACE_1 / 2.)
            .cursor_pointer()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(CONTROL_RADIUS)
                    .when(self.open, |wash| wash.bg(colors.hover))
                    .group_hover(self.id, |wash| wash.bg(colors.hover)),
            )
            .child(icon(self.icon, ICON_SIZE_ROW, text))
            .child(label)
            .child(icon(
                Icon::ChevronDown,
                ICON_SIZE_SMALL,
                colors.text_tertiary,
            ))
    }
}

fn icon(icon: Icon, size: Pixels, color: Hsla) -> impl IntoElement {
    svg()
        .path(icon.path())
        .flex_none()
        .size(size)
        .text_color(color)
}

impl Composer {
    /// The drawer under a draft's box; `None` once the session has seen
    /// work.
    pub(super) fn render_context(
        &self,
        draft: bool,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        if !draft {
            return None;
        }
        let colors = Theme::of(cx).colors;
        // Light words are a dark appearance's.
        let dark = colors.text_primary.l > 0.5;
        let workspace = self.store.read(cx).workspace();
        let project_id = workspace.session(self.session)?.project;
        let project = workspace.project(project_id)?;
        let name: SharedString = workspace.display_name(project_id)?.into();
        let icon = match project.kind {
            ProjectKind::Folder => Icon::Folder,
            ProjectKind::Computer => Icon::Computer,
        };
        let fill = self.surfaces(cx).quiet;
        let project = self.render_project_pill(icon, name, window, cx);
        let branch = self.render_branch(window, cx);
        let seam = dark.then(|| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(SEAM_SHADE)
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(gpui::black().opacity(SEAM_SHADE_OPACITY), 0.),
                    linear_color_stop(gpui::black().opacity(0.), 1.),
                ))
        });
        Some(
            div()
                .relative()
                .mx(DRAWER_INSET)
                .p(SPACE_1)
                .flex()
                .items_center()
                .justify_between()
                .gap(SPACE_2)
                .rounded_b(COMPOSER_RADIUS)
                .border_1()
                .border_t_0()
                .border_color(colors.border)
                .bg(fill)
                .text_size(TEXT_SMALL)
                .text_color(colors.text_secondary)
                .children(seam)
                .child(project)
                .children(branch)
                .into_any_element(),
        )
    }
}
