//! Each sidebar row's own fill, fading over `ROW_FADE` to what the row should
//! show: the selection's wash, the hover's, or none. Nothing slides from row
//! to row; the row the pointer leaves fades out as the next fades in.

use std::{collections::HashMap, time::Instant};

use gpui::Hsla;
use theme::motion::ROW_FADE;
use workspace::SessionId;

/// A row with a fill of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Key {
    /// New session or a page, by its place among them.
    Nav(usize),
    Session(SessionId),
}

#[derive(Clone, Copy)]
struct Fade {
    from: Hsla,
    to: Hsla,
    start: Instant,
}

impl Fade {
    fn progress(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.start);
        (elapsed.as_secs_f32() / ROW_FADE.as_secs_f32()).min(1.)
    }

    fn shown(&self, now: Instant) -> Hsla {
        let t = self.progress(now);
        // Eases in and out, as a color transition does.
        mix(self.from, self.to, t * t * (3. - 2. * t))
    }
}

/// `from` on its way to `to`; a clear end takes the other's color, so only
/// its alpha fades.
fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let from = if from.a == 0. {
        Hsla { a: 0., ..to }
    } else {
        from
    };
    let to = if to.a == 0. {
        Hsla { a: 0., ..from }
    } else {
        to
    };
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Hsla {
        h: lerp(from.h, to.h),
        s: lerp(from.s, to.s),
        l: lerp(from.l, to.l),
        a: lerp(from.a, to.a),
    }
}

#[derive(Default)]
pub(super) struct Fills {
    fades: HashMap<Key, Fade>,
}

impl Fills {
    /// The fill `key` shows this frame, on its way to `target`. A new target
    /// fades from what the row shows now; a row drawn for the first time, and
    /// any row under Reduce motion, shows its target at once.
    pub fn at(&mut self, key: Key, target: Hsla, now: Instant) -> Hsla {
        let fade = self.fades.entry(key).or_insert(Fade {
            from: target,
            to: target,
            start: now,
        });
        if fade.to != target {
            let from = if crate::accessibility::reduce_motion() {
                target
            } else {
                fade.shown(now)
            };
            *fade = Fade {
                from,
                to: target,
                start: now,
            };
        }
        fade.shown(now)
    }

    /// Whether a row is still on its way, so the list draws again next frame.
    pub fn fading(&self, now: Instant) -> bool {
        (self.fades.values()).any(|fade| fade.from != fade.to && fade.progress(now) < 1.)
    }

    /// Forgets the rows of a replaced list, so a row coming back shows its
    /// fill at once rather than fading from a stale one.
    pub fn forget_sessions(&mut self) {
        self.fades.retain(|key, _| matches!(key, Key::Nav(_)));
    }
}
