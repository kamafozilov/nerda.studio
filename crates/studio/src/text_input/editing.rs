//! Cursor movement, selection, deletion, clipboard and mouse handling.

use std::ops::Range;

use gpui::{
    ClipboardItem, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Window,
};
use unicode_segmentation::UnicodeSegmentation;

use super::keys::{
    Backspace, Copy, Cut, Delete, DeleteToStart, DeleteWordLeft, End, Home, Left, Paste, Right,
    SelectAll, SelectLeft, SelectRight, SelectToEnd, SelectToStart, SelectWordLeft,
    SelectWordRight, ShowCharacterPalette, WordLeft, WordRight,
};
use super::{TextInput, TextInputEvent};

impl TextInput {
    pub(super) fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selected_range.is_empty() {
            self.previous_boundary(self.cursor_offset())
        } else {
            self.selected_range.start
        };
        self.move_to(target, window, cx);
    }

    pub(super) fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selected_range.is_empty() {
            self.next_boundary(self.cursor_offset())
        } else {
            self.selected_range.end
        };
        self.move_to(target, window, cx);
    }

    /// Moves to the start of the word before the cursor, or before the selection.
    pub(super) fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(
            self.previous_word_start(self.selected_range.start),
            window,
            cx,
        );
    }

    /// Moves to the end of the word after the cursor, or after the selection.
    pub(super) fn word_right(
        &mut self,
        _: &WordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(self.next_word_end(self.selected_range.end), window, cx);
    }

    pub(super) fn select_left(
        &mut self,
        _: &SelectLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.previous_boundary(self.cursor_offset()), window, cx);
    }

    pub(super) fn select_right(
        &mut self,
        _: &SelectRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.next_boundary(self.cursor_offset()), window, cx);
    }

    pub(super) fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.previous_word_start(self.cursor_offset()), window, cx);
    }

    pub(super) fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.next_word_end(self.cursor_offset()), window, cx);
    }

    pub(super) fn select_all(
        &mut self,
        _: &SelectAll,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.selection_changed(window, cx);
    }

    pub(super) fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.row_span(self.cursor_offset()).start, window, cx);
    }

    pub(super) fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.row_span(self.cursor_offset()).end, window, cx);
    }

    pub(super) fn select_to_start(
        &mut self,
        _: &SelectToStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.row_span(self.cursor_offset()).start, window, cx);
    }

    pub(super) fn select_to_end(
        &mut self,
        _: &SelectToEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.row_span(self.cursor_offset()).end, window, cx);
    }

    /// Deletes the selection, else the grapheme before the cursor.
    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        let range = self.selection_or(|input, cursor| input.previous_boundary(cursor)..cursor);
        self.delete_range(range, window, cx);
    }

    /// Deletes the selection, else the grapheme after the cursor.
    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        let range = self.selection_or(|input, cursor| cursor..input.next_boundary(cursor));
        self.delete_range(range, window, cx);
    }

    /// Deletes the selection, else back to the start of the word before the cursor.
    pub(super) fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.selection_or(|input, cursor| input.previous_word_start(cursor)..cursor);
        self.delete_range(range, window, cx);
    }

    /// Deletes the selection, else everything before the cursor on its row.
    pub(super) fn delete_to_start(
        &mut self,
        _: &DeleteToStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.selection_or(|input, cursor| input.row_span(cursor).start..cursor);
        self.delete_range(range, window, cx);
    }

    pub(super) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    /// Replaces the selection with the clipboard's text, its line breaks
    /// normalized for the input's `Lines`.
    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = self.lines.normalize(&text).into_owned();
            self.replace(self.selected_range.clone(), &text, window, cx);
        }
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let selected = self.content[self.selected_range.clone()].to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(selected));
        }
    }

    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let selected = self.content[self.selected_range.clone()].to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(selected));
            self.replace(self.selected_range.clone(), "", window, cx);
        }
    }

    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        window.request_virtual_keyboard();
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, window, cx);
        } else {
            self.move_to(offset, window, cx);
        }
    }

    pub(super) fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A release the input never saw (another window, a lost grab) ends the drag here.
        if event.pressed_button != Some(MouseButton::Left) {
            self.is_selecting = false;
        }
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), window, cx);
        }
    }

    /// Replaces `range` with `text`, puts the cursor after it and ends any IME composition.
    pub(super) fn replace(
        &mut self,
        range: Range<usize>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = [
            &self.content[..range.start],
            text,
            &self.content[range.end..],
        ]
        .concat();
        let cursor = range.start + text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.set_content(content, window, cx);
    }

    /// Stores new text, tells the platform IME, and emits `Changed` when the text differs.
    pub(super) fn set_content(
        &mut self,
        content: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = content != self.content.as_ref();
        self.content = content.into();
        self.reveal_cursor = true;
        self.goal_x = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
        if changed {
            cx.emit(TextInputEvent::Changed);
        }
    }

    pub(super) fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub(super) fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.selection_changed(window, cx);
    }

    /// Moves the cursor end of the selection to `offset`, keeping the other end as the anchor.
    pub(super) fn select_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.selection_changed(window, cx);
    }

    pub(super) fn selection_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal_cursor = true;
        self.goal_x = None;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    /// The selection if there is one, else the range `around` derives from the cursor.
    fn selection_or(&self, around: impl FnOnce(&Self, usize) -> Range<usize>) -> Range<usize> {
        if self.selected_range.is_empty() {
            around(self, self.cursor_offset())
        } else {
            self.selected_range.clone()
        }
    }

    /// Deletes `range`, or rings the bell when there is nothing to delete.
    fn delete_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        if range.is_empty() {
            window.play_system_bell();
        } else {
            self.replace(range, "", window, cx);
        }
    }

    /// The grapheme boundary nearest the pointer on last frame's text, clamped to the text.
    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(painted)) = (self.last_bounds, self.painted.as_ref()) else {
            return 0;
        };
        self.grapheme_start(painted.index_for_position(position, bounds))
    }

    /// The start of the grapheme containing `offset`; the text's end maps to itself.
    fn grapheme_start(&self, offset: usize) -> usize {
        if offset >= self.content.len() {
            return self.content.len();
        }
        self.content
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .take_while(|&index| index <= offset)
            .last()
            .unwrap_or(0)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }

    /// Start of the last word beginning before `offset`, skipping spaces and punctuation.
    fn previous_word_start(&self, offset: usize) -> usize {
        self.content
            .unicode_word_indices()
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    /// End of the first word ending after `offset`, skipping spaces and punctuation.
    fn next_word_end(&self, offset: usize) -> usize {
        self.content
            .unicode_word_indices()
            .map(|(index, word)| index + word.len())
            .find(|&end| end > offset)
            .unwrap_or(self.content.len())
    }
}
