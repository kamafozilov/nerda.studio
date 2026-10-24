//! Text with chips painted under some of its runs: a rounded fill and
//! hairline behind each, wrapping with the text, and an icon in the room
//! the run keeps for it. What opens on a click answers the pointer: a link
//! is underlined and a chip's hairline takes its color.

use std::ops::Range;

use gpui::{
    AnyElement, App, BorderStyle, Bounds, CursorStyle, DispatchPhase, Element, ElementId,
    GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    MouseMoveEvent, Pixels, SharedString, TextLayout, TransformationMatrix, UnderlineStyle, Window,
    point, px, quad, size, underline_y_offset,
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
    /// Its hairline under the pointer, for a chip that opens.
    pub hovered: Option<Hsla>,
}

pub struct Chipped {
    child: AnyElement,
    layout: TextLayout,
    chips: Vec<Paint>,
    /// The links, underlined in their color under the pointer.
    links: Vec<Range<usize>>,
    link_color: Hsla,
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
            links: Vec::new(),
            link_color: Hsla::default(),
            select: None,
        }
    }

    /// Underlines each of `links` in `color` while the pointer is on it.
    pub fn links(mut self, links: Vec<Range<usize>>, color: Hsla) -> Self {
        self.links = links;
        self.link_color = color;
        self
    }

    /// Whether anything in the text answers the pointer.
    fn hoverable(&self) -> bool {
        self.select.is_some()
            || !self.links.is_empty()
            || self.chips.iter().any(|chip| chip.hovered.is_some())
    }

    /// The link or opening chip under the pointer, by its range.
    fn hovered(&self, hitbox: Option<&Hitbox>, window: &Window) -> Option<Range<usize>> {
        let hitbox = hitbox.filter(|hitbox| hitbox.is_hovered(window))?;
        hovered_in(&self.layout, &self.targets(), window.mouse_position())
            .filter(|_| hitbox.bounds.contains(&window.mouse_position()))
    }

    /// The ranges that open on a click.
    fn targets(&self) -> Vec<Range<usize>> {
        let chips = (self.chips.iter())
            .filter(|chip| chip.hovered.is_some())
            .map(|chip| chip.range.clone());
        self.links.iter().cloned().chain(chips).collect()
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
            (self.hoverable()).then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal));
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
        let height = (line_height - px(2.)).min(px(20.));
        let inset = (line_height - height) / 2.;
        let hovered = self.hovered(hitbox.as_ref(), window);
        for chip in &self.chips {
            let border = match chip.hovered {
                Some(color) if hovered.as_ref() == Some(&chip.range) => color,
                _ => chip.border,
            };
            for (left, right, top) in lines(&self.layout, &chip.range) {
                let bounds = Bounds::new(point(left, top + inset), size(right - left, height));
                window.paint_quad(quad(
                    bounds,
                    CHIP_RADIUS,
                    chip.fill,
                    HAIRLINE,
                    border,
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
        if let Some(range) = hovered.as_ref().filter(|range| self.links.contains(range)) {
            self.underline(range, window);
        }
        if let Some(hitbox) = hitbox {
            if hovered.is_some() {
                window.set_cursor_style(CursorStyle::PointingHand, hitbox);
            }
            // Draws again as the pointer comes onto a link or leaves it,
            // the text's edges included.
            let (layout, targets, hitbox) = (self.layout.clone(), self.targets(), hitbox.clone());
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let now = (hitbox.is_hovered(window) && hitbox.bounds.contains(&event.position))
                    .then(|| hovered_in(&layout, &targets, event.position))
                    .flatten();
                if now != hovered {
                    window.refresh();
                }
            });
        }
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

impl Chipped {
    /// A hairline under `range` on each line it covers, where the text
    /// would draw its own.
    fn underline(&self, range: &Range<usize>, window: &mut Window) {
        let Some(line) = self.layout.line_layout_for_index(range.start) else {
            return;
        };
        let line = &line.unwrapped_layout;
        let below = underline_y_offset(self.layout.line_height(), line.ascent, line.descent);
        let style = UnderlineStyle {
            thickness: HAIRLINE,
            color: Some(self.link_color),
            wavy: false,
        };
        for (left, right, top) in lines(&self.layout, range) {
            window.paint_underline(point(left, top + below), right - left, &style);
        }
    }
}

/// Which of `targets` the text holds at `position`.
fn hovered_in(
    layout: &TextLayout,
    targets: &[Range<usize>],
    position: gpui::Point<Pixels>,
) -> Option<Range<usize>> {
    let at = layout.index_for_position(position).ok()?;
    targets.iter().find(|range| range.contains(&at)).cloned()
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
