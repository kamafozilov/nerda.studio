//! Text with chips painted under some of its runs: a rounded fill and
//! hairline behind each, wrapping with the text, and an icon in the room
//! the run keeps for it.

use std::ops::Range;

use gpui::{
    AnyElement, App, BorderStyle, Bounds, Element, ElementId, GlobalElementId, Hitbox,
    HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels, SharedString,
    TextLayout, TransformationMatrix, Window, point, px, quad, size,
};
use theme::{HAIRLINE, ICON_SIZE_SMALL};

use super::select::Select;

/// The corners of a chip in a line of text.
const CHIP_RADIUS: Pixels = px(4.);

/// One chip: the run it sits under, its colors, and its icon with the
/// byte where the icon's room starts.
pub struct Paint {
    pub range: Range<usize>,
    pub fill: Hsla,
    pub border: Hsla,
    pub icon: Option<(usize, SharedString, Hsla)>,
}

pub struct Chipped {
    child: AnyElement,
    layout: TextLayout,
    chips: Vec<Paint>,
    select: Option<Select>,
}

impl Chipped {
    /// `child` drawn over its chips; `layout` is the layout of the text
    /// inside it.
    pub fn new(child: AnyElement, layout: TextLayout, chips: Vec<Paint>) -> Self {
        Self {
            child,
            layout,
            chips,
            select: None,
        }
    }

    /// Lets the pointer select the text.
    pub fn selectable(mut self, select: Select) -> Self {
        self.select = Some(select);
        self
    }
}

impl IntoElement for Chipped {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Chipped {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        // Under the text's own, so its links keep their pointer.
        let hitbox =
            (self.select.is_some()).then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal));
        self.child.prepaint(window, cx);
        hitbox
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_height = self.layout.line_height();
        let height = (line_height - px(2.)).min(px(19.));
        let inset = (line_height - height) / 2.;
        for chip in &self.chips {
            for (left, right, top) in lines(&self.layout, &chip.range) {
                let bounds = Bounds::new(point(left, top + inset), size(right - left, height));
                window.paint_quad(quad(
                    bounds,
                    CHIP_RADIUS,
                    chip.fill,
                    HAIRLINE,
                    chip.border,
                    BorderStyle::Solid,
                ));
            }
        }
        if let Some(select) = &self.select {
            select.paint(&self.layout, window);
            if let Some(hitbox) = hitbox {
                select.listen(&self.layout, hitbox, window);
            }
        }
        self.child.paint(window, cx);
        for chip in &self.chips {
            let Some((at, path, color)) = &chip.icon else {
                continue;
            };
            let Some(start) = self.layout.position_for_index(*at) else {
                continue;
            };
            let top = start.y + (line_height - ICON_SIZE_SMALL) / 2.;
            let bounds = Bounds::new(point(start.x, top), size(ICON_SIZE_SMALL, ICON_SIZE_SMALL));
            window
                .paint_svg(
                    bounds,
                    path.clone(),
                    None,
                    TransformationMatrix::unit(),
                    *color,
                    cx,
                )
                .ok();
        }
    }
}

/// Where `range` sits on each line it covers: its left and right edges and
/// the line's top.
pub(super) fn lines(layout: &TextLayout, range: &Range<usize>) -> Vec<(Pixels, Pixels, Pixels)> {
    let text = layout.text();
    let mut lines: Vec<(Pixels, Pixels, Pixels)> = Vec::new();
    let Some(chars) = text.get(range.clone()) else {
        return lines;
    };
    let left_edge = layout.bounds().left();
    let ends =
        (chars.char_indices()).map(|(at, c)| (range.start + at, range.start + at + c.len_utf8()));
    for (start, end) in ends {
        let (Some(from), Some(to)) = (
            layout.position_for_index(start),
            layout.position_for_index(end),
        ) else {
            continue;
        };
        // At a wrap the layout places the index at the end of the line
        // before: a character there starts the next line, at its left edge.
        let (from, right) = match to.y == from.y {
            true => (from, to.x),
            false => (point(left_edge, to.y), to.x),
        };
        match lines.last_mut() {
            Some((_, last_right, top)) if *top == from.y => *last_right = right.max(*last_right),
            _ => lines.push((from.x, right, from.y)),
        }
    }
    lines
}
