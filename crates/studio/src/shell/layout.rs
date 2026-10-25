//! Under the top row: the sidebar on the glass, and beside it the main area,
//! a card raised over the glass.

use gpui::{
    AnimationExt, ClickEvent, Context, IntoElement, ParentElement, StyleRefinement, Styled, div,
    prelude::*,
};
use theme::{MAIN_INSET, MAIN_RADIUS, Theme};
use workspace::Tab;

use super::Shell;
use crate::{empty_page::empty_page, welcome};

impl Shell {
    /// The sidebar's contents at full width, pinned to the window's left edge:
    /// the sidebar's edge sweeps over them as it narrows, and the card slides
    /// over the space they leave.
    pub(super) fn sidebar(&self) -> impl IntoElement {
        let width = self.sidebar_width;
        div()
            .relative()
            .flex_none()
            .h_full()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(width)
                    .h_full()
                    // Cached: a frame of the tabs or the card doesn't lay the
                    // list out again; the sidebar redraws when it changes.
                    .child(
                        self.sidebar
                            .clone()
                            .cached(StyleRefinement::default().size_full()),
                    ),
            )
            .with_spring(
                "sidebar",
                self.sidebar_spring(),
                move |sidebar, phase: f32| {
                    let phase = phase.clamp(0., 1.);
                    sidebar.w(width * phase).opacity(phase)
                },
            )
    }

    /// The main area: a rounded card with a hairline, `MAIN_INSET` clear of the
    /// window's right and bottom edges and, as the sidebar closes, of its left.
    /// Before the first project it welcomes, unless a page is on screen; a
    /// session shows its conversation over its composer, and a page what it
    /// will hold.
    pub(super) fn main_area(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let workspace = self.store.read(cx).workspace();
        let page = workspace.selected_tab().and_then(Tab::page);
        let welcome = page.is_none() && !workspace.has_projects();
        let card = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(MAIN_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .children(self.shown_conversation(cx))
            .when_some(self.shown_composer(), |card, composer| card.child(composer))
            .when_some(page, |card, page| card.child(empty_page(page, cx)))
            .when(welcome, |card| {
                card.child(welcome::no_projects(
                    cx.listener(|shell, _: &ClickEvent, window, cx| {
                        shell.open_add_project(window, cx)
                    }),
                    cx,
                ))
            });
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .pr(MAIN_INSET)
            .pb(MAIN_INSET)
            .child(card)
            .with_spring(
                "main-area",
                self.sidebar_spring(),
                move |main, phase: f32| main.pl(MAIN_INSET * (1. - phase.clamp(0., 1.))),
            )
    }
}
