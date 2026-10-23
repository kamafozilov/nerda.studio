//! The surfaces under a column of rows: the selection's, and the one under the
//! pointer or the keyboard highlight. Each is a single shape that glides from
//! row to row on a spring instead of one going out under a row and another
//! coming in under the next, so the eye follows it. A surface that appears
//! fades in where it lands; one whose row moved because the list changed, not
//! the choice, takes its place at once.

use gpui::{
    AnimationExt, AnyElement, App, Bounds, Hsla, IntoElement, ParentElement, Pixels, Point,
    SpringAnimation, SpringConfig, SpringPlayback, Styled, UniformListDecoration, Window, div, px,
};
use theme::{
    CONTROL_RADIUS, ROW_GAP,
    motion::{
        HOVER_ENTER, HOVER_EXIT, PHASE_EPSILON, PIXEL_EPSILON, SELECTION_ENTER, SELECTION_EXIT,
    },
};

/// Where one surface sits and whether it moves there or is put there.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Track {
    /// The row under the surface; it stays on the last one while it fades out.
    row: usize,
    visible: bool,
    /// Whether the last frame drew the surface: one that was hidden there
    /// lands without gliding. Within a frame the pointer can leave one row just
    /// before it enters the next (moving up, the row above is told last), and
    /// that must still glide.
    drawn_visible: bool,
    /// The next frame puts the surface on its row instead of gliding there.
    snap: bool,
}

impl Track {
    /// Moves the surface to `row`, or fades it out where it is for `None`. A
    /// surface coming back from hidden lands without gliding across the list.
    pub fn aim(&mut self, row: Option<usize>) {
        match row {
            Some(row) => {
                self.snap |= !self.drawn_visible;
                self.row = row;
                self.visible = true;
            }
            None => self.visible = false,
        }
    }

    /// Puts the surface on `row` at once: its row changed under it.
    pub fn place(&mut self, row: Option<usize>) {
        self.aim(row);
        self.snap = true;
    }

    /// The row the surface sits on, or last sat on while hidden.
    pub fn row(&self) -> usize {
        self.row
    }

    /// The track for one frame; a snap applies to that frame only.
    pub fn take(&mut self) -> Track {
        let track = *self;
        self.snap = false;
        self.drawn_visible = self.visible;
        track
    }
}

/// The two surfaces for one frame of a list of rows `slot` tall, each
/// `slot - ROW_GAP` high and centered in its slot, `inset` in from the sides.
#[derive(Clone, Copy)]
pub(super) struct Glide {
    pub name: &'static str,
    pub slot: Pixels,
    pub inset: Pixels,
    pub selected: Track,
    /// How far the selected row is drawn off its slot this frame, while the
    /// list's cards glide to new places: the selection rides its card.
    pub selected_offset: f32,
    pub hovered: Track,
    pub selected_color: Hsla,
    pub hovered_color: Hsla,
}

impl Glide {
    /// The surfaces over a box the rows fill from its top: for a list that does
    /// not scroll, drawn under the rows.
    pub fn render(self) -> AnyElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(surface(
                (self.name, 0),
                self.selected,
                self.selected_offset,
                (SELECTION_ENTER, SELECTION_EXIT),
                self.slot,
                self.inset,
                self.selected_color,
            ))
            .child(surface(
                (self.name, 1),
                self.hovered,
                0.,
                (HOVER_ENTER, HOVER_EXIT),
                self.slot,
                self.inset,
                self.hovered_color,
            ))
            .into_any_element()
    }
}

/// In a uniform list the surfaces are drawn over the rows, in the rows' own
/// scrolled space, so they scroll with them. Both washes are text primary at a
/// low alpha, so over text primary they leave the text as it was.
impl UniformListDecoration for Glide {
    fn compute(
        &self,
        _visible_range: std::ops::Range<usize>,
        _bounds: Bounds<Pixels>,
        _scroll_offset: Point<Pixels>,
        _item_height: Pixels,
        _item_count: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> AnyElement {
        self.render()
    }
}

/// One surface, gliding and fading in on `enter` and fading out on `exit`,
/// drawn `offset` off its row.
fn surface(
    id: (&'static str, u64),
    track: Track,
    offset: f32,
    (enter, exit): (SpringConfig, SpringConfig),
    slot: Pixels,
    inset: Pixels,
    color: Hsla,
) -> impl IntoElement {
    let top = slot * track.row as f32 + ROW_GAP / 2.;
    let playback = if track.snap {
        SpringPlayback::Completed
    } else {
        SpringPlayback::Running
    };
    div()
        .absolute()
        .left(inset)
        .right(inset)
        .h(slot - ROW_GAP)
        .rounded(CONTROL_RADIUS)
        .bg(color)
        .with_spring(
            (id.0, id.1 * 2),
            SpringAnimation::new(enter)
                .to(top.as_f32())
                .with_epsilon(PIXEL_EPSILON)
                .playback(playback),
            move |surface, top: f32| surface.top(px(top + offset)),
        )
        .with_spring(
            (id.0, id.1 * 2 + 1),
            SpringAnimation::new(if track.visible { enter } else { exit })
                .to(if track.visible { 1. } else { 0. })
                .with_epsilon(PHASE_EPSILON),
            |surface, opacity: f32| surface.map_element(|s| s.opacity(opacity.clamp(0., 1.))),
        )
}
