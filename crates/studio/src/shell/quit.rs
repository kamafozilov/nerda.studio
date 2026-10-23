//! Cmd-Q (Ctrl-Q elsewhere) in the window asks first, so a slip of the finger
//! doesn't close the app: a dialog over the window, where Return or a second
//! Cmd-Q quits and Escape stays. The menu's Quit is a deliberate choice and
//! quits at once.

use gpui::{
    App, ClickEvent, Context, Div, FocusHandle, FontWeight, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, ParentElement, Pixels, SharedString, Stateful, Styled, Window, actions, div,
    img, prelude::*, px, svg,
};
use theme::{ICON_SIZE, KEY_CAP_SIZE, SPACE_1, SPACE_2, TEXT_BODY, Theme};

use super::Shell;
use crate::assets::{APP_ICON, Icon};

actions!(
    quit_dialog,
    [
        /// Quits: Return in the dialog.
        ConfirmQuit,
        /// Closes the dialog and stays: Escape.
        CancelQuit,
    ]
);

/// The dialog's key context. It sits outside the shell's, so a second Cmd-Q
/// there reaches the app's own Quit, and the window's shortcuts rest while the
/// question is up.
const CONTEXT: &str = "QuitDialog";

const WIDTH: Pixels = px(424.);
const PADDING: Pixels = px(24.);
/// Larger than the palettes' corners: the dialog is larger and stands alone.
const RADIUS: Pixels = px(20.);
const ICON: Pixels = px(32.);
const TITLE: Pixels = px(18.);
const BUTTON_HEIGHT: Pixels = px(34.);
const BUTTON_RADIUS: Pixels = px(10.);
const BUTTON_TEXT: Pixels = px(14.);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", ConfirmQuit, Some(CONTEXT)),
        KeyBinding::new("escape", CancelQuit, Some(CONTEXT)),
    ]);
}

/// The dialog while it is up.
pub(super) struct QuitDialog {
    focus: FocusHandle,
    /// Where focus was before it opened, to give it back on Cancel.
    focus_before: Option<FocusHandle>,
}

impl Shell {
    /// Cmd-Q in the window: puts the dialog up and gives it the keyboard.
    pub(super) fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.quit_dialog.is_some() {
            cx.quit();
            return;
        }
        let focus = cx.focus_handle();
        let focus_before = window.focused(cx);
        window.focus(&focus, cx);
        self.quit_dialog = Some(QuitDialog {
            focus,
            focus_before,
        });
        cx.notify();
    }

    /// Takes the dialog down and gives focus back to where it was.
    fn cancel_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.quit_dialog.take() else {
            return;
        };
        let focus = dialog
            .focus_before
            .unwrap_or_else(|| self.focus_handle.clone());
        window.focus(&focus, cx);
        cx.notify();
    }

    /// The dialog over a window-wide scrim, whose click is Cancel.
    pub(super) fn quit_dialog(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let dialog = self.quit_dialog.as_ref()?;
        let colors = Theme::of(cx).colors;
        let cancel_on_click = cx.listener(|shell, event: &MouseDownEvent, window, cx| {
            // The second press of a double-click leaves it up, as palettes do.
            if event.click_count < 2 {
                shell.cancel_quit(window, cx);
            }
        });
        let buttons = div()
            .mt(px(32.))
            .flex()
            .justify_end()
            .gap(SPACE_2)
            .child(
                button("quit-cancel", "Cancel")
                    .bg(colors.key_background)
                    .text_color(colors.text_primary)
                    .child(key_cap("esc", cx))
                    .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| {
                        shell.cancel_quit(window, cx)
                    })),
            )
            .child(
                button("quit-confirm", "Quit")
                    .bg(colors.danger)
                    .text_color(colors.text_on_danger)
                    .child(
                        svg()
                            .path(Icon::Return.path())
                            .size(ICON_SIZE)
                            .text_color(colors.text_on_danger.opacity(0.6)),
                    )
                    .on_click(|_, _, cx| cx.quit()),
            );
        let card = div()
            .id("quit-dialog")
            .key_context(CONTEXT)
            .track_focus(&dialog.focus)
            .on_action(cx.listener(|_, _: &ConfirmQuit, _, cx| cx.quit()))
            .on_action(
                cx.listener(|shell, _: &CancelQuit, window, cx| shell.cancel_quit(window, cx)),
            )
            .occlude()
            .w(WIDTH)
            .p(PADDING)
            .flex()
            .flex_col()
            .rounded(RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.popover_background)
            .shadow_lg()
            .child(img(APP_ICON).size(ICON))
            .child(
                div()
                    .mt(px(14.))
                    .text_size(TITLE)
                    .font_weight(FontWeight::BOLD)
                    .text_color(colors.text_primary)
                    .child("Quit Nerda Studio?"),
            )
            .child(
                div()
                    .mt(SPACE_2)
                    .text_color(colors.text_secondary)
                    .child("Your projects, sessions and tabs open again next time."),
            )
            .child(buttons);
        Some(
            div()
                .id("quit-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.scrim)
                .text_size(TEXT_BODY)
                .occlude()
                .on_mouse_down(MouseButton::Left, cancel_on_click)
                .child(card),
        )
    }
}

/// A dialog button: its label, then the key that presses it.
fn button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .h(BUTTON_HEIGHT)
        .px(px(16.))
        .flex()
        .items_center()
        .gap(px(10.))
        .rounded(BUTTON_RADIUS)
        .text_size(BUTTON_TEXT)
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        // A press on a button is not a press on the scrim behind it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

/// A key cap drawn as an outline, so it reads on the button's fill.
fn key_cap(key: impl Into<SharedString>, cx: &App) -> Div {
    let colors = Theme::of(cx).colors;
    div()
        .h(px(18.))
        .min_w(px(18.))
        .px(SPACE_1)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .border_1()
        .border_color(colors.text_secondary.opacity(0.6))
        .text_color(colors.text_secondary)
        .text_size(px(9.))
        .font_weight(FontWeight::BOLD)
        .line_height(KEY_CAP_SIZE)
        .child(key.into())
}
