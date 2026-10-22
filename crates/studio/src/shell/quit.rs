//! Cmd-Q (Ctrl-Q elsewhere) in the window quits only on a second press: the
//! first shows a hint saying so, which goes away if none follows in time. The
//! menu's Quit is a deliberate choice and quits at once.

use std::time::Duration;

use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div};
use theme::{DIALOG_RADIUS, SPACE_1, SPACE_2, SPACE_3, Theme};

use super::Shell;
use crate::key_cap::{Key, key_cap};

/// How long after the first press a second one still quits.
const REPEAT_WITHIN: Duration = Duration::from_millis(2000);

const MODIFIER: Key = Key::Text(if cfg!(target_os = "macos") {
    "⌘"
} else {
    "Ctrl"
});

impl Shell {
    /// Quits if the hint is up, else puts it up for `REPEAT_WITHIN`.
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

    /// "Press ⌘ Q again to quit", centered over the window while it is up. It
    /// takes no clicks, so the window under it works as before.
    pub(super) fn quit_hint(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        self.quit_hint.as_ref()?;
        let colors = Theme::of(cx).colors;
        let hint = div()
            .flex()
            .items_center()
            .gap(SPACE_1)
            .px(SPACE_3)
            .py(SPACE_2)
            .rounded(DIALOG_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.popover_background)
            .shadow_lg()
            .font_weight(FontWeight::MEDIUM)
            .child("Press")
            .child(key_cap(&[MODIFIER], colors))
            .child(key_cap(&[Key::Text("Q")], colors))
            .child("again to quit");
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(hint),
        )
    }
}
