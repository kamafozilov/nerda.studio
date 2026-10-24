//! Paints the input's line: text or placeholder, IME underline, selection and cursor,
//! clipped to its bounds and scrolled so the cursor stays in view.

use gpui::{
    App, Bounds, ContentMask, ElementId, ElementInputHandler, Entity, GlobalElementId,
    InspectorElementId, LayoutId, PaintQuad, Pixels, ShapedLine, SharedString, Style, TextAlign,
    TextRun, UnderlineStyle, Window, fill, point, prelude::*, px, relative,
};
use theme::Theme;

use super::{Painted, TextInput};

/// Width of the insertion cursor.
pub(super) const CURSOR_WIDTH: Pixels = px(1.);

/// What the input shows, the placeholder while it is empty, with its runs:
/// IME text still being composed is underlined.
pub(super) fn display_text(
    input: &TextInput,
    window: &Window,
    cx: &App,
) -> (SharedString, Vec<TextRun>) {
    let colors = Theme::of(cx).colors;
    let (display_text, color) = if input.content.is_empty() {
        let color = if input.quiet_placeholder {
            colors.text_tertiary
        } else {
            colors.text_secondary
        };
        (input.placeholder.clone(), color)
    } else {
        (input.content.clone(), colors.text_primary)
    };
    let run = TextRun {
        len: display_text.len(),
        font: window.text_style().font(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let runs = match input.marked_range.as_ref() {
        Some(marked) if !input.content.is_empty() => [
            TextRun {
                len: marked.start,
                ..run.clone()
            },
            TextRun {
                len: marked.end - marked.start,
                underline: Some(UnderlineStyle {
                    color: Some(colors.text_primary),
                    thickness: px(1.),
                    wavy: false,
                }),
                ..run.clone()
            },
            TextRun {
                len: display_text.len() - marked.end,
                ..run
            },
        ]
        .into_iter()
        .filter(|run| run.len > 0)
        .collect(),
        _ => vec![run],
    };
    (display_text, runs)
}

pub(super) struct TextElement {
    input: Entity<TextInput>,
}

impl TextElement {
    pub(super) fn new(input: Entity<TextInput>) -> Self {
        Self { input }
    }
}

pub(super) struct PrepaintState {
    line: Option<ShapedLine>,
    /// The line height the cursor was placed by. Painting uses it too: a
    /// deferred element, such as a menu's search field, paints after its
    /// parents' text style is gone, where `window.line_height()` is the
    /// window's default and would set the glyphs lower than the cursor.
    line_height: Pixels,
    scroll_x: Pixels,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
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
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let focused = input.focus_handle.is_focused(window);
        let (display_text, runs) = display_text(input, window, cx);
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let scrolled = match input.painted {
            Some(Painted::Line { scroll_x, .. }) => scroll_x,
            _ => px(0.),
        };
        let scroll_x = scroll_to_cursor(scrolled, &line, cursor, bounds.size.width);
        let left = bounds.left() - scroll_x;
        let cursor_x = line.x_for_index(cursor);
        let (top, bottom) = glyph_box(&line, bounds);
        let (selection, cursor) = if !focused {
            (None, None)
        } else if selected_range.is_empty() {
            let cursor = fill(
                Bounds::from_corners(
                    point(left + cursor_x, top),
                    point(left + cursor_x + CURSOR_WIDTH, bottom),
                ),
                colors.text_primary,
            );
            (None, input.cursor_visible().then_some(cursor))
        } else {
            let selection = fill(
                Bounds::from_corners(
                    point(left + line.x_for_index(selected_range.start), top),
                    point(left + line.x_for_index(selected_range.end), bottom),
                ),
                colors.text_selection,
            );
            (Some(selection), None)
        };
        PrepaintState {
            line: Some(line),
            line_height: window.line_height(),
            scroll_x,
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
        let Some(line) = prepaint.line.take() else {
            return;
        };
        let scroll_x = prepaint.scroll_x;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(selection);
            }
            // A glyph that fails to rasterize leaves a gap this frame; the next frame retries.
            let _ = line.paint(
                point(bounds.left() - scroll_x, bounds.top()),
                prepaint.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });

        self.input.update(cx, |input, _cx| {
            input.painted = Some(Painted::Line {
                line: Box::new(line),
                scroll_x,
            });
            input.last_bounds = Some(bounds);
        });
    }
}

/// The vertical span of the font's ascent and descent, centered in the line as
/// `ShapedLine::paint` centers the glyphs, and snapped to whole points. The cursor and
/// selection cover this span, as in AppKit's fields, not the taller line height.
fn glyph_box(line: &ShapedLine, bounds: Bounds<Pixels>) -> (Pixels, Pixels) {
    glyph_span(line.ascent + line.descent, bounds)
}

/// The span of a glyph box `glyphs` tall, centered in `bounds` and snapped.
pub(super) fn glyph_span(glyphs: Pixels, bounds: Bounds<Pixels>) -> (Pixels, Pixels) {
    let height = glyphs.round();
    let top = (bounds.top() + (bounds.size.height - height) / 2.).round();
    (top, top + height)
}

/// The smallest change to `scroll_x` that keeps the cursor inside `width`, and never
/// scrolls past the line's end so deleting text pulls it back into view.
fn scroll_to_cursor(scroll_x: Pixels, line: &ShapedLine, cursor: usize, width: Pixels) -> Pixels {
    let cursor_x = line.x_for_index(cursor);
    let max_scroll = (line.width + CURSOR_WIDTH - width).max(px(0.));
    let mut scroll_x = scroll_x.clamp(px(0.), max_scroll);
    if cursor_x + CURSOR_WIDTH - scroll_x > width {
        scroll_x = cursor_x + CURSOR_WIDTH - width;
    }
    if cursor_x < scroll_x {
        scroll_x = cursor_x;
    }
    scroll_x
}
