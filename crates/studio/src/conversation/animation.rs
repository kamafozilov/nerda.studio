//! The conversation's live motion, each a function of the time since one
//! shared start, so every copy on screen moves in step: a band of light
//! sweeping through a label while what it names goes on, the working
//! indicator's dots rising in a wave, and a new row settling in. Under
//! Reduce motion each holds still.

use std::{
    f32::consts::TAU,
    sync::OnceLock,
    time::{Duration, Instant},
};

use gpui::{HighlightStyle, Hsla, Rgba, SharedString, StyledText};

/// How often the conversation redraws while something moves: 30 frames a
/// second, a quarter of the display's, enough for a sweep and a wave.
pub const FRAME: Duration = Duration::from_millis(33);
/// One sweep of light through a label, edge to edge.
const SHIMMER_PERIOD: f32 = 2.2;
/// Half the band of light, in characters; a long label widens it.
const SHIMMER_BAND: f32 = 4.;
/// One wave through the working indicator's dots.
const WAVE_PERIOD: f32 = 1.2;
/// How faint a dot gets between waves.
const WAVE_DIM: f32 = 0.15;
/// A new row's response: it settles, without overshoot, in about twice this.
const ENTER_RESPONSE: f32 = 0.3;
/// How far below its place a new row starts.
pub const ENTER_RISE: f32 = 4.;
/// How long text that just streamed in takes to fade in.
const ARRIVE: Duration = Duration::from_millis(320);

/// Seconds since the conversation's motion began.
fn clock() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// `text` with a band of `peak` sweeping through `base`, or in `base` alone
/// when `still`.
pub fn shimmer(text: impl Into<SharedString>, base: Hsla, peak: Hsla, still: bool) -> StyledText {
    let text: SharedString = text.into();
    let base_style = HighlightStyle {
        color: Some(base),
        ..HighlightStyle::default()
    };
    if still || text.is_empty() {
        let all = (0..text.len(), base_style);
        return StyledText::new(text.clone()).with_highlights([all]);
    }
    let chars = text.chars().count() as f32;
    let band = SHIMMER_BAND.max(chars * 0.12);
    let phase = (clock() / SHIMMER_PERIOD).fract();
    // The band starts out past the left edge and leaves past the right.
    let center = -band + phase * (chars + band * 2.);
    let highlights = text.char_indices().enumerate().map(|(at, (start, char))| {
        let distance = (at as f32 + 0.5 - center).abs();
        let amount = (1. - distance / band).clamp(0., 1.);
        // Eased, so the band's edge fades rather than steps.
        let amount = amount * amount * (3. - 2. * amount);
        let style = HighlightStyle {
            color: Some(mix(base, peak, amount)),
            ..HighlightStyle::default()
        };
        (start..start + char.len_utf8(), style)
    });
    StyledText::new(text.clone()).with_highlights(highlights.collect::<Vec<_>>())
}

/// `from` turned `amount` of the way to `to`, in sRGB.
pub fn mix(from: Hsla, to: Hsla, amount: f32) -> Hsla {
    let (a, b) = (Rgba::from(from), Rgba::from(to));
    let lerp = |x: f32, y: f32| x + (y - x) * amount;
    Rgba {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    }
    .into()
}

/// The opacity of the working indicator's dot at `row` and `column` of a
/// square of `side`: a wave enters at the bottom row and meets at the top
/// one's middle, so the square reads as rising. Still, every dot shows.
pub fn wave(row: usize, column: usize, side: usize, still: bool) -> f32 {
    if still {
        return 0.8;
    }
    let center = (side as f32 - 1.) / 2.;
    let farthest = side as f32 - 1. + center;
    let distance = side as f32 - 1. - row as f32 + (column as f32 - center).abs();
    let offset = distance / (farthest + 1.);
    let t = (clock() / WAVE_PERIOD - offset).rem_euclid(1.);
    // Bright at once, fading to dim by 45%, resting there, then back up.
    let lerp = |from: f32, to: f32, t: f32| from + (to - from) * t;
    if t < 0.45 {
        lerp(1., WAVE_DIM, t / 0.45)
    } else if t < 0.92 {
        WAVE_DIM
    } else {
        lerp(WAVE_DIM, 1., (t - 0.92) / 0.08)
    }
}

/// How far a row that appeared `elapsed` ago has settled, 0 to 1: a
/// critically damped spring, as the rest of the app's motion.
pub fn entered(elapsed: Duration) -> f32 {
    let omega = TAU / ENTER_RESPONSE;
    let t = elapsed.as_secs_f32();
    1. - (1. + omega * t) * (-omega * t).exp()
}

/// Whether a row that appeared `elapsed` ago still moves visibly.
pub fn entering(elapsed: Duration) -> bool {
    entered(elapsed) < 0.995
}

/// How far text that came `elapsed` ago has faded in, 0 to 1: quick at
/// first, easing into place.
pub fn arrived(elapsed: Duration) -> f32 {
    let progress = (elapsed.as_secs_f32() / ARRIVE.as_secs_f32()).clamp(0., 1.);
    1. - (1. - progress).powf(1.6)
}

/// Whether text that came `elapsed` ago still fades in.
pub fn arriving(elapsed: Duration) -> bool {
    elapsed < ARRIVE
}

/// A soft pulse between `low` and 1 for a mark that waits, such as a
/// running tool's icon.
pub fn breathe(low: f32, still: bool) -> f32 {
    if still {
        return 1.;
    }
    let wave = 0.5 - 0.5 * (clock() / 1.6 * TAU).cos();
    low + (1. - low) * wave
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_row_settles_without_overshoot() {
        assert_eq!(entered(Duration::ZERO), 0.);
        let mut last = 0.;
        for ms in (0..1000).step_by(10) {
            let now = entered(Duration::from_millis(ms));
            assert!(now >= last && now <= 1.);
            last = now;
        }
        assert!(!entering(Duration::from_millis(700)));
    }

    #[test]
    fn still_dots_all_show() {
        assert_eq!(wave(0, 0, 3, true), wave(2, 1, 3, true));
    }
}
