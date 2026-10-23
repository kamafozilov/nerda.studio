//! The text as painted last frame, with how far it was scrolled: it maps the
//! pointer and the IME's ranges to offsets and back.

use std::ops::Range;

use gpui::{Bounds, Pixels, Point, ShapedLine, point};

use super::wrapped::Wrapped;

pub(super) enum Painted {
    /// One line, shifted left so the cursor stays in view.
    Line {
        line: Box<ShapedLine>,
        scroll_x: Pixels,
    },
    /// Wrapped lines, shifted up so the cursor stays in view.
    Wrapped { text: Wrapped, scroll_y: Pixels },
}

impl Painted {
    /// Whether it shows `content`, rather than the placeholder or older text.
    pub fn shows(&self, content: &str) -> bool {
        !content.is_empty()
            && match self {
                Self::Line { line, .. } => line.text == content,
                Self::Wrapped { text, .. } => text.text() == content,
            }
    }

    /// The offset nearest `position` in a field painted in `bounds`, clamped
    /// to the text: above a single line is its start, below it its end.
    pub fn index_for_position(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> usize {
        match self {
            Self::Line { line, scroll_x } => {
                if position.y < bounds.top() {
                    0
                } else if position.y > bounds.bottom() {
                    line.len()
                } else {
                    line.closest_index_for_x(position.x - bounds.left() + *scroll_x)
                }
            }
            Self::Wrapped { text, scroll_y } => text.index_at(point(
                position.x - bounds.left(),
                position.y - bounds.top() + *scroll_y,
            )),
        }
    }

    /// Where `range` sits in a field painted in `bounds`: one box per line it
    /// spans, each the line's full height.
    pub fn range_bounds(&self, range: Range<usize>, bounds: Bounds<Pixels>) -> Vec<Bounds<Pixels>> {
        match self {
            Self::Line { line, scroll_x } => {
                let left = bounds.left() - *scroll_x;
                vec![Bounds::from_corners(
                    point(left + line.x_for_index(range.start), bounds.top()),
                    point(left + line.x_for_index(range.end), bounds.bottom()),
                )]
            }
            Self::Wrapped { text, scroll_y } => {
                let origin = point(bounds.left(), bounds.top() - *scroll_y);
                text.range_rects(range)
                    .into_iter()
                    .map(|rect| Bounds::new(rect.origin + origin, rect.size))
                    .collect()
            }
        }
    }
}
