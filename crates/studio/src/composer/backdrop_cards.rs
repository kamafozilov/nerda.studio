//! The backdrop panel's pictures: a pattern card's face, the pattern drawn
//! small for it, and the owner's image as it is.

use std::sync::Arc;

use ::backdrop::{Ink, Pattern, Source};
use gpui::{
    App, Hsla, IntoElement, ParentElement, Pixels, RenderImage, Styled, Window, div, px, relative,
};
use theme::{CONTROL_RADIUS, Theme};

use crate::backdrop::{self, Look};

/// A pattern card's picture: the card's shape at a fifth of a laptop's.
const THUMB_HEIGHT: Pixels = px(68.);

/// A pattern card's face: the card's color, the pattern over it, and a
/// small composer in its middle, over it.
pub(super) fn card_face(
    picture: Option<Arc<RenderImage>>,
    fill: Hsla,
    border: Hsla,
    cx: &App,
) -> impl IntoElement {
    let colors = Theme::of(cx).colors;
    div()
        .relative()
        .h(THUMB_HEIGHT)
        .rounded(CONTROL_RADIUS)
        .border_1()
        .border_color(border)
        .bg(fill)
        .overflow_hidden()
        .children(picture.map(|picture| backdrop::picture(picture, CONTROL_RADIUS)))
        .child(
            div()
                .absolute()
                .top(relative(MINI_COMPOSER.1))
                .left(relative(MINI_COMPOSER.0))
                .w(relative(MINI_COMPOSER.2))
                .h(relative(MINI_COMPOSER.3))
                .rounded(px(3.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.composer_background),
        )
}

/// The small composer on a pattern card, as shares of the card: left, top,
/// width and height, where a draft's composer stands on the real card.
const MINI_COMPOSER: (f32, f32, f32, f32) = (0.25, 0.4, 0.5, 0.2);

/// How much smaller a pattern card is than the card it stands for.
const THUMB_SHRINK: f32 = 5.;
/// How much stronger a pattern card's ink is than the card's: its dots are
/// a fifth the size, and the cards are there to tell patterns apart.
const THUMB_GAIN: f32 = 2.5;

/// A pattern drawn for a card `width` wide: the card's shape, shrunk by
/// `THUMB_SHRINK` with its pattern and its ink `THUMB_GAIN` stronger, under
/// the small composer as the card is under the real one.
pub(super) fn pattern_card(
    look: Look,
    width: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    let scale = window.scale_factor();
    let width = (f32::from(width) * scale).round();
    let height = (f32::from(THUMB_HEIGHT) * scale).round();
    let gain = |alpha: u8| (alpha as f32 * THUMB_GAIN).min(255.) as u8;
    let ink = match look.ink {
        Ink::Solid([r, g, b, a]) => Ink::Solid([r, g, b, gain(a)]),
        Ink::Source { alpha } => Ink::Source { alpha: gain(alpha) },
    };
    let look = Look { ink, ..look };
    backdrop::Backdrop::global(cx).update(cx, |backdrop, _| {
        let time = backdrop.still_time();
        let frame = look.frame(width as u32, height as u32, scale / THUMB_SHRINK, time);
        backdrop.thumbnail(frame)
    })
}

/// The owner's image as it is, `size` large.
pub(super) fn image_picture(
    source: Arc<Source>,
    size: (Pixels, Pixels),
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    let scale = window.scale_factor();
    let look = Look {
        pattern: Pattern::Image,
        ink: Ink::Source { alpha: 255 },
        source: Some(source),
        light: false,
    };
    let (width, height) = (f32::from(size.0) * scale, f32::from(size.1) * scale);
    backdrop::Backdrop::global(cx).update(cx, |backdrop, _| {
        let frame = look.frame(width.round() as u32, height.round() as u32, scale, 0.);
        backdrop.thumbnail(frame)
    })
}
