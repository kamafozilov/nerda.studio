//! A page before it has anything to show: its title and what it will hold,
//! centered in the main area.

use gpui::{App, FontWeight, IntoElement, ParentElement, Styled, div};
use theme::{SPACE_1, SPACE_3, Theme};
use workspace::Page;

pub fn empty_page(page: Page, cx: &App) -> impl IntoElement {
    let colors = Theme::of(cx).colors;
    div()
        .flex_1()
        .min_h_0()
        .p(SPACE_3)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(SPACE_1)
        .text_center()
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors.text_primary)
                .child(page.title()),
        )
        .child(div().text_color(colors.text_secondary).child(purpose(page)))
}

/// What the page will hold, in one line.
fn purpose(page: Page) -> &'static str {
    match page {
        Page::Notes => "Notes you keep beside your sessions will live here.",
        Page::Design => "A canvas to design on will live here.",
        Page::Routines => "Work your agents repeat on a schedule will live here.",
        Page::Usage => "How much your agents use, by session and project, will show here.",
        Page::PullRequests => "Pull requests from your projects will show here.",
    }
}
