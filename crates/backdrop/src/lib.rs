//! The patterns behind a new session's composer, drawn into pixels: ordered
//! dither, halftone dots, contour lines or characters, in one ink or in the
//! colors of an image, over transparency.
//!
//! A pattern is drawn in points and scaled to the display, so it keeps its size
//! on any screen. It covers the whole card, the composer included: the
//! composer's surfaces set it apart. Given an image, a pattern draws the image's
//! lightness instead of its own. Drawing is pure and quick enough to repeat
//! several times a second, which is how a pattern moves: [`Frame::time`]
//! shifts it.

mod noise;
mod patterns;
mod source;

use std::sync::Arc;

pub use patterns::Pattern;
pub use source::Source;

/// What the marks are colored with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    /// One color, straight RGBA; a mark's alpha is scaled by its coverage.
    Solid([u8; 4]),
    /// The image's color under each mark, at this alpha. Without an image,
    /// nothing is marked.
    Source { alpha: u8 },
}

/// What to draw: a pattern over `width` by `height` device pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Device pixels to a point.
    pub scale: f32,
    pub pattern: Pattern,
    pub ink: Ink,
    /// The image the pattern draws, if any.
    pub source: Option<Arc<Source>>,
    /// The frame lies on a light surface, where a mark darkens: the image's
    /// dark areas then draw the most marks.
    pub light: bool,
    /// Seconds into the pattern's slow drift; a still pattern stays at one time.
    pub time: f32,
}

impl Frame {
    /// The frame's pixels, BGRA with straight alpha, row by row.
    pub fn render(&self) -> Vec<u8> {
        // Bare pixels carry a solid ink's color at no alpha, so filtering
        // between them and a mark never darkens its edge.
        let bare = match self.ink {
            Ink::Solid([r, g, b, _]) => [b, g, r, 0],
            Ink::Source { .. } => [0; 4],
        };
        let mut pixels = bare.repeat(self.width as usize * self.height as usize);
        if self.width == 0 || self.height == 0 || self.scale <= 0. {
            return pixels;
        }
        let mut canvas = Canvas {
            pixels: &mut pixels,
            width: self.width,
            height: self.height,
        };
        patterns::draw(self, &mut canvas);
        pixels
    }

    /// How strongly the image asks for marks at a point, in `0..1`: its
    /// lightness on a dark surface, its darkness on a light one.
    pub(crate) fn density(&self, source: &Source, x: f32, y: f32) -> f32 {
        let lightness = source.lightness(self.size(), x, y);
        if self.light {
            1. - lightness
        } else {
            lightness
        }
    }

    /// The mark's color at a point given in device pixels, as BGRA.
    pub(crate) fn mark(&self, x: f32, y: f32) -> Option<[u8; 4]> {
        match (self.ink, &self.source) {
            (Ink::Solid([r, g, b, a]), _) => Some([b, g, r, a]),
            (Ink::Source { alpha }, Some(source)) => {
                let [r, g, b] = source.color(self.size(), x, y);
                Some([b, g, r, alpha])
            }
            (Ink::Source { .. }, None) => None,
        }
    }

    fn size(&self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }
}

/// The pixels being drawn.
pub(crate) struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
}

impl Canvas<'_> {
    /// Paints the `width` by `height` block whose top-left corner is at `x`,
    /// `y` in `bgra`, clipped to the frame.
    pub(crate) fn block(&mut self, x: u32, y: u32, width: u32, height: u32, bgra: [u8; 4]) {
        if x >= self.width {
            return;
        }
        let right = (x + width).min(self.width);
        for row in y..(y + height).min(self.height) {
            let start = (row * self.width + x) as usize * 4;
            let end = (row * self.width + right) as usize * 4;
            for pixel in self.pixels[start..end].as_chunks_mut::<4>().0 {
                *pixel = bgra;
            }
        }
    }

    /// Paints one pixel in `bgra` at `coverage` of its alpha.
    pub(crate) fn blend(&mut self, x: u32, y: u32, coverage: f32, bgra: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let at = (y * self.width + x) as usize * 4;
        let [b, g, r, a] = bgra;
        let alpha = (a as f32 * coverage.clamp(0., 1.)).round() as u8;
        self.pixels[at..at + 4].copy_from_slice(&[b, g, r, alpha]);
    }
}
