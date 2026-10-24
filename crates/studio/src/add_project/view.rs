//! How the palette draws: the field with its add button, the folders or what stands
//! in for them, and a footer of key hints with the system's folder picker.

use gpui::{
    AnyElement, ClickEvent, Context, DismissEvent, Div, Focusable, FontWeight, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px, svg, uniform_list,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, Colors, DIALOG_RADIUS, DISABLED_OPACITY, FIELD_TARGET_SIZE,
    HEADER_HEIGHT, ICON_SIZE, KEY_CAP_SIZE, PALETTE_FIELD_HEIGHT, PALETTE_LIST_HEIGHT,
    PALETTE_WIDTH, ROW_HEIGHT, SPACE_1, SPACE_2, SPACE_3, Theme,
};
use workspace::browse::{BrowseQuery, ListError, Target};

use super::{AddProjectPalette, CONTEXT, Row};
use crate::{
    assets::Icon,
    kbd::{Kbd, Key, Shortcut},
};

const MAC: bool = cfg!(target_os = "macos");

impl AddProjectPalette {
    /// Covers the window and dims it: a click outside the palette closes it and
    /// presses nothing beneath. The second press of a double-click is ignored, so
    /// a double-click on Add project leaves the palette open.
    fn render_backdrop(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let close = cx.listener(|_, event: &MouseDownEvent, _, cx| {
            if event.click_count < 2 {
                cx.emit(DismissEvent);
            }
        });
        let close_right = cx.listener(|_, _: &MouseDownEvent, _, cx| cx.emit(DismissEvent));
        div()
            .id("add-project-backdrop")
            .absolute()
            .inset_0()
            .bg(Theme::of(cx).colors.scrim)
            .occlude()
            .on_mouse_down(MouseButton::Left, close)
            .on_mouse_down(MouseButton::Right, close_right)
    }

    fn render_field(&self, target: Option<&Target>, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let input_focus = self.input.focus_handle(cx);
        let enabled = self.browse().is_some() && !matches!(target, Some(Target::File(_)));
        let label = if matches!(target, Some(Target::Create(_))) {
            "Create & Add"
        } else {
            "Add"
        };
        // While a folder is highlighted Enter opens it, so adding takes the modifier.
        let shortcut = if self.highlighted.is_some() {
            Shortcut::of(Key::Return).cmd()
        } else {
            Shortcut::of(Key::Return)
        };
        // The key cap sits as far from the button's right edge as from its top and
        // bottom; the button's hairline counts toward it.
        let cap_inset = (FIELD_TARGET_SIZE - KEY_CAP_SIZE) / 2. - px(1.);
        div()
            .id("add-project-field")
            .h(PALETTE_FIELD_HEIGHT)
            .flex_none()
            .pl(SPACE_3)
            // The button sits as far from the right edge as from the top and bottom.
            .pr((PALETTE_FIELD_HEIGHT - FIELD_TARGET_SIZE) / 2.)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .border_b_1()
            .border_color(colors.border)
            .cursor_text()
            .on_click(
                cx.listener(move |_, _: &ClickEvent, window, cx| window.focus(&input_focus, cx)),
            )
            .child(
                svg()
                    .path(Icon::FolderAdd.path())
                    .size(ICON_SIZE)
                    .flex_none()
                    .text_color(colors.text_secondary),
            )
            .child(div().flex_1().min_w_0().child(self.input.clone()))
            .child(
                div()
                    .id("add-project-submit")
                    .flex_none()
                    .h(FIELD_TARGET_SIZE)
                    .pl(SPACE_2)
                    .pr(cap_inset)
                    .flex()
                    .items_center()
                    .gap(SPACE_2)
                    .rounded(CONTROL_RADIUS)
                    .border_1()
                    .border_color(colors.border)
                    .text_color(colors.text_primary)
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_default()
                    .when(enabled, |button| {
                        button
                            .cursor_pointer()
                            .hover(|button| button.bg(colors.hover))
                            .on_click(cx.listener(|palette, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                palette.add_typed(cx);
                            }))
                    })
                    .when(!enabled, |button| button.opacity(DISABLED_OPACITY))
                    .child(label)
                    .child(Kbd::new(shortcut)),
            )
    }

    fn render_problem(&self, colors: Colors) -> Option<impl IntoElement> {
        let problem = self.problem.clone()?;
        Some(
            div()
                .flex_none()
                .px(SPACE_3)
                .py(SPACE_2)
                .border_b_1()
                .border_color(colors.border)
                .text_color(colors.text_primary)
                .child(problem),
        )
    }

    fn render_list(&self, target: Option<&Target>, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let has_folders = self.rows.iter().any(|row| matches!(row, Row::Folder(_)));
        let area = div()
            .h(PALETTE_LIST_HEIGHT)
            .flex_none()
            .p(SPACE_2)
            .flex()
            .flex_col();
        if has_folders {
            let palette = cx.entity();
            return area
                .child(section_label("Folders", colors))
                .child(
                    uniform_list("add-project-rows", self.rows.len(), move |range, _, cx| {
                        palette.update(cx, |palette, cx| {
                            range.map(|row| palette.render_row(row, cx)).collect()
                        })
                    })
                    .track_scroll(&self.scroll)
                    .flex_1(),
                )
                .into_any_element();
        }
        // No subfolders to show: the parent, if any, then why the list is empty.
        let up = (!self.rows.is_empty()).then(|| self.render_row(0, cx));
        area.children(up)
            .children(self.message(target).map(|text| {
                div()
                    .flex_1()
                    .px(SPACE_3)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_center()
                    .text_color(colors.text_secondary)
                    .child(text)
            }))
            .into_any_element()
    }

    /// What the list says when it shows no subfolders.
    fn message(&self, target: Option<&Target>) -> Option<SharedString> {
        let start = if cfg!(windows) {
            "~\\ or C:\\"
        } else {
            "~/ or /"
        };
        let Some((query, _)) = self.browse() else {
            let text = match BrowseQuery::parse(self.query.trim()) {
                Some(query) if query.is_relative() => format!(
                    "./ and ../ start from the current project. Add one first, or start with {start}."
                ),
                _ => format!("Type a path that starts with {start}."),
            };
            return Some(text.into());
        };
        if self.loading_shown() {
            return Some("Loading folders…".into());
        }
        let (_, _, listing) = self.listing()?;
        let text = match (listing, target?) {
            (Err(ListError::Unreadable(kind)), _) => {
                format!("Couldn't list this folder ({kind}). Press Enter to add it anyway.")
            }
            (_, Target::File(name)) => format!("{name} is a file. Choose a folder."),
            (Err(_), Target::Create(_)) => {
                "This folder doesn't exist yet. Press Enter to create it and add it as a project."
                    .to_owned()
            }
            (Ok(_), Target::Create(_)) => format!(
                "No folder named “{}” here. Press Enter to create it and add it as a project.",
                query.leaf
            ),
            (_, Target::Add(_)) => "No folders inside. Press Enter to add this one.".to_owned(),
        };
        Some(text.into())
    }

    fn render_row(&self, row: usize, cx: &mut Context<Self>) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let (icon, label): (Icon, SharedString) = match &self.rows[row] {
            Row::Up(_) => (Icon::FolderUp, "..".into()),
            Row::Folder(name) => (Icon::Folder, name.clone()),
        };
        div()
            .id(("add-project-row", row))
            .w_full()
            .h(ROW_HEIGHT)
            .flex_none()
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .when(self.highlighted == Some(row), |item| item.bg(colors.hover))
            .cursor_pointer()
            .hover(|item| item.bg(colors.hover))
            .on_click(cx.listener(move |palette, _: &ClickEvent, _, cx| palette.open_row(row, cx)))
            .child(
                svg()
                    .path(icon.path())
                    .size(ICON_SIZE)
                    .flex_none()
                    .text_color(colors.text_secondary),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_color(colors.text_primary)
                    .child(label),
            )
            .into_any_element()
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let picker_label = if MAC {
            "Choose in Finder…"
        } else if cfg!(windows) {
            "Choose in File Explorer…"
        } else {
            "Choose in Files…"
        };
        div()
            .h(CONTROL_SIZE + SPACE_2)
            .flex_none()
            .pl(SPACE_3)
            .pr(SPACE_1)
            .flex()
            .items_center()
            .gap(SPACE_3)
            .border_t_1()
            .border_color(colors.border)
            .text_color(colors.text_secondary)
            .child(hint(&[Shortcut::key("↑"), Shortcut::key("↓")], "Navigate"))
            .when(self.highlighted.is_some(), |footer| {
                footer.child(hint(&[Shortcut::of(Key::Return)], "Open"))
            })
            .child(hint(&[Shortcut::of(Key::Escape)], "Close"))
            .child(div().flex_1())
            .child(
                div()
                    .id("add-project-picker")
                    .h(CONTROL_SIZE)
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .rounded(CONTROL_RADIUS)
                    .text_color(colors.text_primary)
                    .cursor_pointer()
                    .hover(|button| button.bg(colors.hover))
                    .on_click(cx.listener(|palette, _: &ClickEvent, window, cx| {
                        palette.choose_in_picker(window, cx)
                    }))
                    .child(picker_label),
            )
    }
}

impl Render for AddProjectPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_rows();
        let colors = Theme::of(cx).colors;
        let target = self.target();
        let viewport = window.viewport_size();
        // A tenth of the way down, clear of the top row.
        let top = (viewport.height * 0.1).max(HEADER_HEIGHT);
        div()
            .absolute()
            .inset_0()
            .child(self.render_backdrop(cx))
            .child(
                div()
                    .absolute()
                    .top(top)
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .id("add-project")
                            .key_context(CONTEXT)
                            .on_action(cx.listener(Self::select_next))
                            .on_action(cx.listener(Self::select_previous))
                            .on_action(cx.listener(Self::confirm))
                            .on_action(cx.listener(|palette, _: &super::AddTyped, _, cx| {
                                palette.add_typed(cx)
                            }))
                            .on_action(cx.listener(Self::close))
                            .occlude()
                            .w(PALETTE_WIDTH)
                            .max_w(viewport.width - SPACE_3 * 2.)
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .rounded(DIALOG_RADIUS)
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.popover_background)
                            .shadow_lg()
                            .child(self.render_field(target.as_ref(), cx))
                            .children(self.render_problem(colors))
                            .child(self.render_list(target.as_ref(), cx))
                            .child(self.render_footer(cx)),
                    ),
            )
    }
}

fn section_label(text: &'static str, colors: Colors) -> Div {
    div()
        .h(ROW_HEIGHT)
        .flex_none()
        .px(SPACE_2)
        .flex()
        .items_center()
        .text_color(colors.text_secondary)
        .child(text)
}

/// Key caps, one per key, then what they do.
fn hint(keys: &[Shortcut], label: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(SPACE_1)
        .children(keys.iter().map(|keys| Kbd::new(*keys)))
        .child(label)
}
