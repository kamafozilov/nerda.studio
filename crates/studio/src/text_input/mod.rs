//! A single-line text field: keyboard and mouse editing, selection, clipboard and IME.
//!

mod editing;
mod element;
mod ime;
pub mod keys;

use std::borrow::Cow;
use std::ops::Range;

use gpui::{
    App, Bounds, Context, CursorStyle, EventEmitter, FocusHandle, Focusable, MouseButton, Pixels,
    ShapedLine, SharedString, Window, div, prelude::*, px,
};

use element::TextElement;
pub use keys::bind_keys;

/// What a `TextInput` tells its owner.
pub enum TextInputEvent {
    /// The text changed; pure cursor or selection moves never emit it.
    Changed,
}

/// A single line of editable text. Renders only the line; the caller draws any box around it.
pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    /// Byte range of the selection; empty when it is only a cursor.
    selected_range: Range<usize>,
    /// The cursor sits at the selection's start rather than its end.
    selection_reversed: bool,
    /// Byte range of IME composition text still being edited.
    marked_range: Option<Range<usize>>,
    /// The line painted last frame, used to map positions to offsets.
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// How far the line is shifted left so the cursor stays in view.
    scroll_x: Pixels,
    /// A left-button drag is extending the selection.
    is_selecting: bool,
    /// `set_text` changed the text outside a window; the platform IME learns on the next render.
    ime_content_stale: bool,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: single_line_shared(placeholder.into()),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.),
            is_selecting: false,
            ime_content_stale: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// Replaces the whole text with line breaks turned into spaces, puts the cursor at the
    /// end, and emits `Changed` only if the text differs.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = single_line_shared(text.into());
        let changed = text != self.content;
        self.content = text;
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.is_selecting = false;
        self.ime_content_stale = true;
        cx.notify();
        if changed {
            cx.emit(TextInputEvent::Changed);
        }
    }
}

/// Turns each `\r\n`, `\n` and `\r` into one space, since the field holds a single line.
fn single_line(text: &str) -> Cow<'_, str> {
    if text.contains(['\n', '\r']) {
        Cow::Owned(text.replace("\r\n", " ").replace(['\n', '\r'], " "))
    } else {
        Cow::Borrowed(text)
    }
}

fn single_line_shared(text: SharedString) -> SharedString {
    match single_line(&text) {
        Cow::Borrowed(_) => text,
        Cow::Owned(line) => line.into(),
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.ime_content_stale) {
            window.notify_text_input_changed(&self.focus_handle);
        }
        div()
            .key_context("TextInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_to_start))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_to_start))
            .on_action(cx.listener(Self::select_to_end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement::new(cx.entity()))
    }
}
