//! A text field of one line, or of wrapped lines that grow: keyboard and mouse
//! editing, selection, clipboard and IME.
//!

mod blink;
mod editing;
mod element;
mod ime;
pub mod keys;
mod painted;
mod vertical;
mod wrapped;

use std::borrow::Cow;
use std::ops::Range;

use gpui::{
    App, Bounds, Context, CursorStyle, EventEmitter, FocusHandle, Focusable, MouseButton, Pixels,
    ScrollEvent, SharedString, Window, div, prelude::*, px,
};

use element::TextElement;
pub use keys::bind_keys;
use painted::Painted;
use wrapped::WrappedElement;

/// How many lines a `TextInput` holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lines {
    /// One line: line breaks become spaces, and a line wider than the field
    /// scrolls sideways.
    Single,
    /// Lines that wrap at the field's width and keep their breaks: the field is
    /// `min` lines tall, grows a line at a time to `max`, then scrolls.
    Multi { min: usize, max: usize },
}

impl Lines {
    /// `text` as this field holds it: on one line, each `\r\n`, `\n` and `\r`
    /// is one space; on several, each is one `\n`.
    fn normalize(self, text: &str) -> Cow<'_, str> {
        if !text.contains(['\n', '\r']) {
            return Cow::Borrowed(text);
        }
        let break_as = match self {
            Self::Single => " ",
            Self::Multi { .. } => "\n",
        };
        Cow::Owned(
            text.replace("\r\n", break_as)
                .replace(['\n', '\r'], break_as),
        )
    }

    fn normalize_shared(self, text: SharedString) -> SharedString {
        match self.normalize(&text) {
            Cow::Borrowed(_) => text,
            Cow::Owned(text) => text.into(),
        }
    }
}

/// What a `TextInput` tells its owner.
pub enum TextInputEvent {
    /// The text changed; pure cursor or selection moves never emit it.
    Changed,
}

/// Editable text. Renders only the text; the caller draws any box around it.
pub struct TextInput {
    focus_handle: FocusHandle,
    lines: Lines,
    content: SharedString,
    placeholder: SharedString,
    /// The placeholder in `text_tertiary` rather than `text_secondary`, for a
    /// field without a box of its own.
    quiet_placeholder: bool,
    /// Byte range of the selection; empty when it is only a cursor.
    selected_range: Range<usize>,
    /// The cursor sits at the selection's start rather than its end.
    selection_reversed: bool,
    /// Byte range of IME composition text still being edited.
    marked_range: Option<Range<usize>>,
    /// The text painted last frame and how far it was scrolled, used to map
    /// positions to offsets.
    painted: Option<Painted>,
    last_bounds: Option<Bounds<Pixels>>,
    /// The next frame scrolls the cursor into view; scrolling by the wheel
    /// clears it until the cursor or the text changes.
    reveal_cursor: bool,
    /// Where Up and Down aim on the line they reach: the cursor's place when
    /// the first of them was pressed, so a short line passed through doesn't
    /// pull the cursor to its end.
    goal_x: Option<Pixels>,
    /// A left-button drag is extending the selection.
    is_selecting: bool,
    /// `set_text` changed the text outside a window; the platform IME learns on the next render.
    ime_content_stale: bool,
    blink: blink::Blink,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, lines: Lines, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            lines,
            content: SharedString::default(),
            placeholder: lines.normalize_shared(placeholder.into()),
            quiet_placeholder: false,
            selected_range: 0..0,
            selection_reversed: false,
            blink: blink::Blink::default(),
            marked_range: None,
            painted: None,
            last_bounds: None,
            reveal_cursor: true,
            goal_x: None,
            is_selecting: false,
            ime_content_stale: false,
        }
    }

    /// Shows the placeholder in `text_tertiary`.
    pub fn with_quiet_placeholder(mut self) -> Self {
        self.quiet_placeholder = true;
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// Replaces the whole text, line breaks normalized for its `Lines`, puts the
    /// cursor at the end, and emits `Changed` only if the text differs.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = self.lines.normalize_shared(text.into());
        let changed = text != self.content;
        self.content = text;
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.is_selecting = false;
        self.reveal_cursor = true;
        self.goal_x = None;
        self.ime_content_stale = true;
        cx.notify();
        if changed {
            cx.emit(TextInputEvent::Changed);
        }
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
        self.keep_blinking(window, cx);
        let input = div()
            .key_context(match self.lines {
                Lines::Single => "TextInput",
                Lines::Multi { .. } => "TextInput MultiLine",
            })
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
            .on_mouse_move(cx.listener(Self::on_mouse_move));
        let Lines::Multi { min, max } = self.lines else {
            return input.child(TextElement::new(cx.entity()).into_any_element());
        };
        input
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::text_start))
            .on_action(cx.listener(Self::text_end))
            .on_action(cx.listener(Self::select_to_text_start))
            .on_action(cx.listener(Self::select_to_text_end))
            .on_action(cx.listener(Self::insert_newline))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(WrappedElement::new(cx.entity(), min..=max).into_any_element())
    }
}

impl TextInput {
    /// Scrolls wrapped text past its tallest by the wheel; the cursor may
    /// leave the view until it moves again.
    fn on_scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let (Some(Painted::Wrapped { text, scroll_y }), Some(bounds)) =
            (self.painted.as_mut(), self.last_bounds)
        else {
            return;
        };
        let max_scroll = (text.height() - bounds.size.height).max(px(0.));
        let scrolled = (*scroll_y - delta).clamp(px(0.), max_scroll);
        if scrolled != *scroll_y {
            *scroll_y = scrolled;
            self.reveal_cursor = false;
            cx.stop_propagation();
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Lines;

    #[test]
    fn one_line_reads_each_break_as_a_space_and_several_keep_them() {
        let text = "a\r\nb\nc\rd";
        assert_eq!(Lines::Single.normalize(text), "a b c d");
        assert_eq!(
            Lines::Multi { min: 2, max: 10 }.normalize(text),
            "a\nb\nc\nd"
        );
        assert_eq!(Lines::Single.normalize("plain"), "plain");
    }
}
