//! Cmd-Q (Ctrl-Q elsewhere) in the window quits only on a second press: the
//! first shows a hint saying so, which goes away if none follows in time. The
//! menu's Quit is a deliberate choice and quits at once.

use std::time::Duration;

use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div};
use theme::{
    DIALOG_RADIUS, HEADER_HEIGHT, KEY_CAP_SIZE_LARGE, SPACE_2, SPACE_3, TEXT_HEADING_2, Theme,
};

use super::Shell;
use crate::kbd::{Kbd, Shortcut};

/// How long after the first press a second one still quits.
const REPEAT_WITHIN: Duration = Duration::from_millis(2000);

impl Shell {
    /// Quits if the hint is up, else puts it up for `REPEAT_WITHIN`. Held down,
    /// the key's repeat is the second press.
    pub(super) fn request_quit(&mut self, cx: &mut Context<Self>) {
        if self.quit_hint.take().is_some() {
            cx.quit();
            return;
        }
        self.quit_hint = Some(cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(REPEAT_WITHIN).await;
            shell
                .update(cx, |shell, cx| {
                    shell.quit_hint = None;
                    cx.notify();
                })
                .ok();
        }));
        cx.notify();
    }

    /// "Press ⌘Q again to quit" at the top center of the window, under the top
    /// row, while it is up. It takes no clicks, so the window under it works as
    /// before.
    pub(super) fn quit_hint(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        self.quit_hint.as_ref()?;
        let colors = Theme::of(cx).colors;
        let hint = div()
            .h(KEY_CAP_SIZE_LARGE + SPACE_2 * 2.)
            .px(SPACE_2 * 2.)
            .flex()
            .items_center()
            .gap(SPACE_2)
            .whitespace_nowrap()
            .rounded(DIALOG_RADIUS)
            .border_1()
            .border_color(colors.popover_border)
            .bg(colors.popover_background)
            .shadow(colors.popover_shadows())
            .text_size(TEXT_HEADING_2)
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors.text_primary)
            .child("Press")
            .child(Kbd::new(Shortcut::key("Q").cmd()).large())
            .child("again to quit");
        Some(
            div()
                .absolute()
                .top(HEADER_HEIGHT + SPACE_3 * 2.)
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(hint),
        )
    }
}
