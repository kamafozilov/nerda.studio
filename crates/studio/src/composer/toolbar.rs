//! The composer's toolbar: Attach, the session's choices as pills, and Send,
//! which turns into Stop while the agent works and there is nothing to send.
//! Attach opens the system's file picker and each pill its menu.

use gpui::{
    Anchor, ClickEvent, Context, Div, Focusable, IntoElement, ParentElement, PathPromptOptions,
    Pixels, Stateful, Styled, Window, div, prelude::*, px, svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, Colors, DISABLED_OPACITY, ICON_SIZE, ICON_SIZE_SMALL, SPACE_1,
    SPACE_2, Theme,
};

use super::{Composer, Send, pickers::Picker};
use crate::{
    assets::Icon,
    menu::Placement,
    sidebar::icon_button,
    tooltip::{TooltipExt, WithTooltip},
};

/// The stop glyph: the icon set's 16 of 24 units at `ICON_SIZE`, as the
/// send arrow fills it, with its corners.
const STOP_GLYPH: Pixels = px(10.);
const STOP_GLYPH_RADIUS: Pixels = px(2.);

impl Composer {
    pub(super) fn render_toolbar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let has_message = self.has_message(cx);
        let stop = !has_message && self.agent.read(cx).is_busy();
        let pills: Vec<_> = Picker::ALL
            .into_iter()
            .map(|picker| self.render_pill(picker, window, cx))
            .collect();
        div()
            .h(CONTROL_SIZE)
            .flex()
            .items_center()
            .gap(SPACE_1)
            .cursor_default()
            .child(icon_button(
                "composer-attach",
                Icon::NewTab,
                "Attach files",
                true,
                colors,
                |button| button.on_click(cx.listener(Self::pick_attachments)),
            ))
            .children(pills)
            .child(div().flex_1())
            .child(if stop {
                stop_button(colors)
                    .on_click(cx.listener(|composer, _: &ClickEvent, _, cx| composer.stop(cx)))
                    .with_tooltip("Stop")
            } else {
                send_button(has_message, colors)
            })
    }

    /// Opens the system's picker for files and folders, several at once;
    /// cancelling it changes nothing.
    fn pick_attachments(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.picking {
            return;
        }
        self.picking = true;
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn_in(window, async move |composer, cx| {
            let picked = picked.await;
            composer
                .update(cx, |composer, cx| {
                    composer.picking = false;
                    match picked {
                        Ok(Ok(Some(paths))) => composer.attach(paths, cx),
                        Ok(Err(error)) => {
                            composer.problem = Some(
                                format!("Couldn't open the file picker ({error}). Try again.")
                                    .into(),
                            );
                            cx.notify();
                        }
                        // Cancelled, or the picker went away with the window.
                        Ok(Ok(None)) | Err(_) => {}
                    }
                })
                .ok();
        })
        .detach();
    }

    /// A choice's current value and a chevron, filled under the pointer and
    /// while its menu is open. The menu opens upward from the pill's leading
    /// edge: the composer sits low in the card, and a menu that never covers
    /// its pill can't take the next click on it as a pick.
    fn render_pill(
        &self,
        picker: Picker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let open = self.menu.opened() == Some(&picker);
        let menu = if open {
            let entries = self.picker_entries(picker, cx);
            let placement = Placement::Hang(Anchor::BottomLeft);
            self.menu
                .render(picker.menu_id(), entries, placement, window, cx)
        } else {
            None
        };
        let pill = div()
            .id(picker.id())
            .min_w_0()
            .h(CONTROL_SIZE)
            .px(SPACE_2)
            .flex()
            .items_center()
            .gap(SPACE_1)
            .rounded(CONTROL_RADIUS)
            .hover(|pill| pill.bg(colors.hover))
            .when(open, |pill| pill.bg(colors.hover))
            .text_color(colors.text_secondary)
            .whitespace_nowrap()
            .on_click(cx.listener(move |composer, _: &ClickEvent, window, cx| {
                // The box's own click would take focus back to the text.
                cx.stop_propagation();
                let text = composer.input.focus_handle(cx);
                composer.menu.open_from(picker, Some(text), window, cx);
            }))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(picker.value(&self.choice(cx))),
            )
            .child(
                svg()
                    .path(Icon::ChevronDown.path())
                    .flex_none()
                    .size(ICON_SIZE_SMALL)
                    .text_color(colors.text_secondary),
            )
            .with_tooltip(picker.name());
        div()
            .relative()
            .min_w_0()
            .child(pill)
            .children(menu.map(|menu| div().absolute().top_0().left_0().child(menu)))
    }
}

/// The round button Send and Stop share, in `text_primary`.
fn round_button(id: &'static str, colors: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(CONTROL_SIZE)
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(colors.text_primary)
}

/// Send's circle with a square cut in the card's color: drawn from a quad,
/// since the icon set's stop is an outline.
fn stop_button(colors: Colors) -> Stateful<Div> {
    round_button("composer-stop", colors).child(
        div()
            .size(STOP_GLYPH)
            .rounded(STOP_GLYPH_RADIUS)
            .bg(colors.panel_background),
    )
}

/// A round button in `text_primary` with an arrow cut in the card's color.
/// Faded until there is a message, since whitespace alone isn't one.
fn send_button(has_message: bool, colors: Colors) -> WithTooltip {
    let label = match (has_message, cfg!(target_os = "macos")) {
        (false, _) => "Type a message to send",
        (true, true) => "Send (↩)",
        (true, false) => "Send (Enter)",
    };
    round_button("composer-send", colors)
        .when(!has_message, |button| button.opacity(DISABLED_OPACITY))
        .when(has_message, |button| {
            button.on_click(|_, window, cx| window.dispatch_action(Box::new(Send), cx))
        })
        .child(
            svg()
                .path(Icon::Send.path())
                .size(ICON_SIZE)
                .text_color(colors.panel_background),
        )
        .with_tooltip(label)
}
