//! Moving between the rows of wrapped text, and typing a line break.

use std::ops::Range;

use gpui::{Context, Pixels, Window};

use super::keys::{
    Down, InsertNewline, SelectDown, SelectToTextEnd, SelectToTextStart, SelectUp, TextEnd,
    TextStart, Up,
};
use super::{Painted, TextInput};

impl TextInput {
    /// The row holding the cursor at `offset`, from its start to the last place
    /// the cursor takes on it; one line is the whole text.
    pub(super) fn row_span(&self, offset: usize) -> Range<usize> {
        match self.painted.as_ref() {
            Some(Painted::Wrapped { text, .. }) if text.text() == self.content.as_ref() => {
                text.row_span(text.row_of(offset))
            }
            _ => 0..self.content.len(),
        }
    }

    /// Where the cursor at `from` lands `step` rows away, aiming at the goal
    /// column, and that column.
    fn row_away(&self, from: usize, step: isize) -> (usize, Option<Pixels>) {
        let Some(Painted::Wrapped { text, .. }) = self.painted.as_ref() else {
            return (self.text_edge(step), None);
        };
        if text.text() != self.content.as_ref() {
            return (self.text_edge(step), None);
        }
        let goal = self.goal_x.unwrap_or_else(|| text.position(from).x);
        let row = text.row_of(from) as isize + step;
        let target = if row < 0 || row as usize >= text.row_count() {
            self.text_edge(step)
        } else {
            text.index_on_row(row as usize, goal)
        };
        (target, Some(goal))
    }

    /// The start going up, the end going down.
    fn text_edge(&self, step: isize) -> usize {
        if step < 0 { 0 } else { self.content.len() }
    }

    fn step_rows(
        &mut self,
        step: isize,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let from = match (extend, step < 0) {
            (true, _) => self.cursor_offset(),
            (false, true) => self.selected_range.start,
            (false, false) => self.selected_range.end,
        };
        let (target, goal) = self.row_away(from, step);
        if extend {
            self.select_to(target, window, cx);
        } else {
            self.move_to(target, window, cx);
        }
        self.goal_x = goal;
    }

    pub(super) fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        self.step_rows(-1, false, window, cx);
    }

    pub(super) fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        self.step_rows(1, false, window, cx);
    }

    pub(super) fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        self.step_rows(-1, true, window, cx);
    }

    pub(super) fn select_down(
        &mut self,
        _: &SelectDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_rows(1, true, window, cx);
    }

    pub(super) fn text_start(
        &mut self,
        _: &TextStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(0, window, cx);
    }

    pub(super) fn text_end(&mut self, _: &TextEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), window, cx);
    }

    pub(super) fn select_to_text_start(
        &mut self,
        _: &SelectToTextStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, window, cx);
    }

    pub(super) fn select_to_text_end(
        &mut self,
        _: &SelectToTextEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.content.len(), window, cx);
    }

    /// Replaces the selection with a line break.
    pub(super) fn insert_newline(
        &mut self,
        _: &InsertNewline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(self.selected_range.clone(), "\n", window, cx);
    }
}
