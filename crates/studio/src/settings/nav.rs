//! Settings' menu, in the sidebar's place while Settings is open: Back to the
//! tabs as "Settings", the search, then the sections in their groups. While
//! the search holds text, the settings it finds take the sections' place,
//! under their sections' titles; a click shows that section.

use std::time::Instant;

use gpui::{
    AnyElement, ClickEvent, Context, Focusable, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, svg,
    transparent_black,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, ICON_SIZE_ROW, ROW_GAP, ROW_HEIGHT, SPACE_1, SPACE_2, TEXT_SMALL,
    Theme,
};
use workspace::matches_search;

use super::{Leave, SettingsPage, section::Section};
use crate::{assets::Icon, sidebar::fade::Key};

/// The menu's rows by their fills' keys: Back, then the sections by their
/// place, then the search's finds by theirs.
const BACK: usize = 0;
const SECTIONS: usize = 1;
const FINDS: usize = 100;

impl SettingsPage {
    pub fn render_nav(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let now = Instant::now();
        let query = self.query(cx);
        let back = self
            .row(BACK, Icon::Back, "Settings".into(), false, now, cx)
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(Leave)));
        let list = if query.is_empty() {
            self.render_sections(now, cx)
        } else {
            self.render_finds(&query, now, cx)
        };
        if self.fills.fading(now) {
            window.request_animation_frame();
        }
        self.keys(div(), cx)
            .size_full()
            .px(SPACE_2)
            .py(SPACE_1)
            .flex()
            .flex_col()
            .gap(ROW_GAP)
            .child(back)
            .child(self.render_search(cx))
            .child(
                div()
                    .id("settings-menu")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(ROW_GAP)
                    .children(list),
            )
            .into_any_element()
    }

    /// The sections, a heading over each group but the first, the groups
    /// `SPACE_2` apart.
    fn render_sections(&mut self, now: Instant, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        let mut group = None;
        for section in Section::ALL {
            if section.group() != group {
                group = section.group();
                rows.push(heading(group.unwrap_or_default(), cx));
            }
            let at = section.index();
            let chosen = section == self.section;
            rows.push(
                self.row(
                    SECTIONS + at,
                    section.icon(),
                    section.title().into(),
                    chosen,
                    now,
                    cx,
                )
                .track_focus(&self.rows_focus[at])
                .on_click(cx.listener(move |page, _: &ClickEvent, _, cx| page.show(section, cx)))
                .into_any_element(),
            );
        }
        rows
    }

    /// The settings `query` finds, under their sections' titles; a section
    /// without settings yet is found by its title.
    fn render_finds(
        &mut self,
        query: &str,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        let mut at = FINDS;
        for section in Section::ALL {
            let mut found: Vec<&'static str> = (section.settings().iter())
                .filter(|setting| {
                    matches_search(setting.label, query) || matches_search(setting.detail, query)
                })
                .map(|setting| setting.label)
                .collect();
            if found.is_empty() && matches_search(section.title(), query) {
                found.push(section.title());
            }
            if found.is_empty() {
                continue;
            }
            rows.push(heading(section.title(), cx));
            for label in found {
                rows.push(
                    self.row(at, section.icon(), label.into(), false, now, cx)
                        .on_click(
                            cx.listener(move |page, _: &ClickEvent, _, cx| page.show(section, cx)),
                        )
                        .into_any_element(),
                );
                at += 1;
            }
        }
        if rows.is_empty() {
            let colors = Theme::of(cx).colors;
            rows.push(
                div()
                    .h(ROW_HEIGHT)
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .text_color(colors.text_secondary)
                    .child("No settings match")
                    .into_any_element(),
            );
        }
        rows
    }

    /// The search: the magnifier and the field, without a box, as the
    /// sidebar's search is.
    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let focus = self.search.focus_handle(cx);
        div()
            .id("settings-search")
            .flex_none()
            .h(CONTROL_SIZE)
            .px(SPACE_2)
            .mb(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .text_size(TEXT_SMALL)
            .font_weight(FontWeight::MEDIUM)
            .cursor_text()
            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| window.focus(&focus, cx)))
            .child(
                svg()
                    .path(Icon::Search.path())
                    .size(ICON_SIZE_ROW)
                    .flex_none()
                    .text_color(colors.text_tertiary),
            )
            .child(div().flex_1().min_w_0().child(self.search.clone()))
    }

    /// A row as the sidebar's are: the chosen one and the one under the
    /// pointer on their washes in `text_primary`, the rest in
    /// `text_secondary`, each fill fading on its own.
    fn row(
        &mut self,
        key: usize,
        icon: Icon,
        title: SharedString,
        chosen: bool,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = Theme::of(cx).colors;
        let hovered = self.hovered == Some(key);
        let target = if chosen {
            colors.row_selected
        } else if hovered {
            colors.hover
        } else {
            Hsla::transparent_black()
        };
        let fill = self.fills.at(Key::Nav(key), target, now);
        let color = if chosen || hovered {
            colors.text_primary
        } else {
            colors.text_secondary
        };
        div()
            .id(("settings-row", key))
            .flex_none()
            .h(ROW_HEIGHT)
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .border_1()
            .border_color(transparent_black())
            .focus_visible(|row| row.border_color(colors.focus_ring))
            .bg(fill)
            .cursor_pointer()
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
            .on_hover(cx.listener(move |page, hovered: &bool, _, cx| {
                if *hovered {
                    page.hovered = Some(key);
                } else if page.hovered == Some(key) {
                    page.hovered = None;
                }
                cx.notify();
            }))
            .child(
                svg()
                    .path(icon.path())
                    .size(ICON_SIZE_ROW)
                    .flex_none()
                    .text_color(color),
            )
            .child(div().flex_1().min_w_0().truncate().child(title))
    }
}

/// A group's heading in the menu, `SPACE_2` under the group before it; the
/// first group's is empty.
fn heading(title: &'static str, cx: &Context<SettingsPage>) -> AnyElement {
    let colors = Theme::of(cx).colors;
    if title.is_empty() {
        return div().into_any_element();
    }
    div()
        .flex_none()
        .h(ROW_HEIGHT)
        .mt(SPACE_2)
        .px(SPACE_2)
        .flex()
        .items_center()
        .text_size(TEXT_SMALL)
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors.text_tertiary)
        .child(title)
        .into_any_element()
}
