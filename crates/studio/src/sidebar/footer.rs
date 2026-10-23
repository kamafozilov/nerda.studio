//! The bottom row: usage, pull requests and settings, at the trailing edge.
//! They open nothing yet; each names its future place.

use gpui::{Context, IntoElement, ParentElement, Styled, div};
use theme::{MAIN_INSET, SPACE_1, SPACE_2, Theme};

use super::{Sidebar, header::icon_button};
use crate::assets::Icon;

impl Sidebar {
    pub(super) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        // The buttons' bottom sits on the card's bottom edge, `MAIN_INSET` up.
        div()
            .flex_none()
            .px(SPACE_2)
            .pt(SPACE_2)
            .pb(MAIN_INSET)
            .flex()
            .items_center()
            .justify_end()
            .gap(SPACE_1)
            .child(icon_button(
                "usage",
                Icon::Usage,
                "Usage",
                true,
                colors,
                |b| b,
            ))
            .child(icon_button(
                "pull-requests",
                Icon::PullRequests,
                "Pull requests",
                true,
                colors,
                |b| b,
            ))
            .child(icon_button(
                "settings",
                Icon::Settings,
                "Settings",
                true,
                colors,
                |b| b,
            ))
    }
}
