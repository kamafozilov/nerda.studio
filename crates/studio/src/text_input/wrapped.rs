//! Text wrapped at the field's width: where each visual line, a row, starts
//! and ends, and the element that paints the rows and grows with them.
//!

use std::ops::{Range, RangeInclusive};

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, ElementId, ElementInputHandler, Entity,
    GlobalElementId, InspectorElementId, LayoutId, PaintQuad, Pixels, Point, SharedString, Style,
    TextAlign, TextRun, Window, WrappedLine, fill, point, prelude::*, px, relative, size,
};
use theme::Theme;
use unicode_segmentation::UnicodeSegmentation;

use super::element::{CURSOR_WIDTH, display_text, glyph_span};
use super::{Painted, TextInput};

/// How far a selection reaches past the end of a line whose break it holds,
/// so a selected empty line still shows.
const BREAK_SELECTION_WIDTH: Pixels = px(4.);

/// Text shaped at a width: one `WrappedLine` per line of the text, and the rows
/// they wrap into, in order.
pub(super) struct Wrapped {
    text: SharedString,
    lines: Vec<WrappedLine>,
    rows: Vec<Row>,
    line_height: Pixels,
}

/// One visual line. Offsets are into the whole text; `x` is where the row
/// starts along its line as if unwrapped.
#[derive(Clone, Copy)]
struct Row {
    line: usize,
    line_start: usize,
    start: usize,
    end: usize,
    x: Pixels,
}

/// The size text is set in and its line height, read where the input's text
/// style applies: a measure runs later, outside it.
#[derive(Clone, Copy)]
struct Metrics {
    font_size: Pixels,
    line_height: Pixels,
}

impl Metrics {
    fn of(window: &Window) -> Self {
        Self {
            font_size: window.text_style().font_size.to_pixels(window.rem_size()),
            line_height: window.line_height(),
        }
    }
}

impl Wrapped {
    fn shape(
        text: SharedString,
        runs: &[TextRun],
        width: Pixels,
        metrics: Metrics,
        window: &Window,
    ) -> Self {
        let Metrics {
            font_size,
            line_height,
        } = metrics;
        let lines = window
            .text_system()
            .shape_text(text.clone(), font_size, runs, Some(width), None)
            .map(|lines| lines.into_vec())
            .unwrap_or_default();
        let mut rows = Vec::new();
        let mut line_start = 0;
        for (line, wrapped) in lines.iter().enumerate() {
            let starts = wrapped.wrap_boundaries().iter().map(|boundary| {
                let glyph =
                    &wrapped.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix];
                (line_start + glyph.index, glyph.position.x)
            });
            let mut start = (line_start, px(0.));
            for next in starts.chain([(line_start + wrapped.len(), px(0.))]) {
                rows.push(Row {
                    line,
                    line_start,
                    start: start.0,
                    end: next.0,
                    x: start.1,
                });
                start = next;
            }
            line_start += wrapped.len() + 1;
        }
        Self {
            text,
            lines,
            rows,
            line_height,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Rows painted; empty text still takes one.
    pub fn row_count(&self) -> usize {
        self.rows.len().max(1)
    }

    /// The height of every row, those scrolled out of view included.
    pub fn height(&self) -> Pixels {
        self.line_height * self.row_count() as f32
    }

    /// The row the cursor at `index` stands on: at a wrap, the row it starts.
    pub fn row_of(&self, index: usize) -> usize {
        self.rows
            .iter()
            .rposition(|row| row.start <= index)
            .unwrap_or(0)
    }

    /// The first offset of `row` and the last the cursor takes on it: before
    /// the space or letter where it wraps, so it never shows on the next row.
    pub fn row_span(&self, row: usize) -> Range<usize> {
        let Some(this) = self.rows.get(row) else {
            return 0..0;
        };
        let wraps = self
            .rows
            .get(row + 1)
            .is_some_and(|next| next.line == this.line);
        if !wraps {
            return this.start..this.end;
        }
        let before_wrap = self.text[this.start..this.end]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(index, _)| index);
        this.start..this.start + before_wrap
    }

    fn x_on(&self, row: usize, index: usize) -> Pixels {
        let Some(row) = self.rows.get(row) else {
            return px(0.);
        };
        let layout = &self.lines[row.line].unwrapped_layout;
        layout.x_for_index(index - row.line_start) - row.x
    }

    /// The top left of the cursor at `index`.
    pub fn position(&self, index: usize) -> Point<Pixels> {
        let row = self.row_of(index);
        point(self.x_on(row, index), self.line_height * row as f32)
    }

    /// The offset nearest `position`, measured from the text's top left; above
    /// the first row is the start, below the last the end.
    pub fn index_at(&self, position: Point<Pixels>) -> usize {
        if position.y < px(0.) {
            return 0;
        }
        let row = (position.y / self.line_height).floor() as usize;
        let Some(this) = self.rows.get(row) else {
            return self.text.len();
        };
        let layout = &self.lines[this.line].unwrapped_layout;
        let index = this.line_start + layout.closest_index_for_x(position.x + this.x);
        let span = self.row_span(row);
        index.clamp(span.start, span.end)
    }

    /// The offset on `row` nearest `x`.
    pub fn index_on_row(&self, row: usize, x: Pixels) -> usize {
        self.index_at(point(x, self.line_height * (row as f32 + 0.5)))
    }

    /// One box per row `range` covers, each the row's full height, measured
    /// from the text's top left.
    pub fn range_rects(&self, range: Range<usize>) -> Vec<Bounds<Pixels>> {
        let mut rects = Vec::new();
        for (index, row) in self.rows.iter().enumerate() {
            if range.end < row.start || range.start > row.end {
                continue;
            }
            let ends_line = self
                .rows
                .get(index + 1)
                .is_none_or(|next| next.line != row.line);
            let holds_break = ends_line && range.end > row.end;
            let left = self.x_on(index, range.start.max(row.start));
            let mut right = self.x_on(index, range.end.min(row.end));
            if holds_break {
                right += BREAK_SELECTION_WIDTH;
            }
            if right > left {
                let top = self.line_height * index as f32;
                rects.push(Bounds::from_corners(
                    point(left, top),
                    point(right, top + self.line_height),
                ));
            }
        }
        rects
    }
}

/// Paints the input's wrapped text, `min` to `max` rows tall, scrolled so the
/// cursor stays in view.
pub(super) struct WrappedElement {
    input: Entity<TextInput>,
    /// How many rows tall it stands, the least and the most.
    rows: RangeInclusive<usize>,
}

impl WrappedElement {
    pub(super) fn new(input: Entity<TextInput>, rows: RangeInclusive<usize>) -> Self {
        Self { input, rows }
    }
}

pub(super) struct PrepaintState {
    text: Option<Wrapped>,
    scroll_y: Pixels,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for WrappedElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WrappedElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// Tall enough for the rows the text wraps into at the width it gets,
    /// between the input's least and most.
    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let (min, max) = (*self.rows.start(), *self.rows.end());
        let (text, runs) = display_text(input, window, cx);
        let metrics = Metrics::of(window);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let layout = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.unwrap_or(match available.width {
                AvailableSpace::Definite(width) => width,
                _ => px(0.),
            });
            let rows = Wrapped::shape(text.clone(), &runs, width - CURSOR_WIDTH, metrics, window)
                .row_count()
                .clamp(min, max);
            size(width, metrics.line_height * rows as f32)
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let colors = Theme::of(cx).colors;
        let (display, runs) = display_text(input, window, cx);
        let width = bounds.size.width - CURSOR_WIDTH;
        let text = Wrapped::shape(display, &runs, width, Metrics::of(window), window);
        let line_height = text.line_height;
        let cursor = input.cursor_offset();
        let scrolled = match input.painted {
            Some(Painted::Wrapped { scroll_y, .. }) => scroll_y,
            _ => px(0.),
        };
        let max_scroll = (text.height() - bounds.size.height).max(px(0.));
        let mut scroll_y = scrolled.clamp(px(0.), max_scroll);
        let caret = text.position(cursor);
        if input.reveal_cursor {
            if caret.y < scroll_y {
                scroll_y = caret.y;
            } else if caret.y + line_height > scroll_y + bounds.size.height {
                scroll_y = caret.y + line_height - bounds.size.height;
            }
        }
        let origin = point(bounds.left(), bounds.top() - scroll_y);
        let focused = input.focus_handle.is_focused(window);
        let selected = input.selected_range.clone();
        let (selection, cursor) = if !focused {
            (Vec::new(), None)
        } else if selected.is_empty() {
            let row = Bounds::new(
                point(origin.x + caret.x, origin.y + caret.y),
                size(CURSOR_WIDTH, line_height),
            );
            let glyphs = text.lines.first().map_or(line_height, |line| {
                line.unwrapped_layout.ascent + line.unwrapped_layout.descent
            });
            let (top, bottom) = glyph_span(glyphs, row);
            let cursor = Bounds::from_corners(
                point(row.left(), top),
                point(row.left() + CURSOR_WIDTH, bottom),
            );
            let cursor = fill(cursor, colors.text_primary);
            (Vec::new(), input.cursor_visible().then_some(cursor))
        } else {
            let rects = text.range_rects(selected);
            let quads = rects
                .into_iter()
                .map(|rect| {
                    fill(
                        Bounds::new(rect.origin + origin, rect.size),
                        colors.text_selection,
                    )
                })
                .collect();
            (quads, None)
        };
        PrepaintState {
            text: Some(text),
            scroll_y,
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let Some(text) = prepaint.text.take() else {
            return;
        };
        let scroll_y = prepaint.scroll_y;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for selection in prepaint.selection.drain(..) {
                window.paint_quad(selection);
            }
            let mut top = bounds.top() - scroll_y;
            for line in &text.lines {
                // A glyph that fails to rasterize leaves a gap this frame; the
                // next frame retries.
                let _ = line.paint(
                    point(bounds.left(), top),
                    text.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
                top += line.size(text.line_height).height;
            }
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
        self.input.update(cx, |input, _cx| {
            input.painted = Some(Painted::Wrapped { text, scroll_y });
            input.last_bounds = Some(bounds);
        });
    }
}
