//! The patterns behind a new session's composer, drawn into pixels: ordered
//! dither, halftone dots or characters in one ink over transparency.
//!
//! A pattern is drawn in points and scaled to the display, so it keeps its size
//! on any screen, and it clears away around the composer, so nothing read
//! there ever sits on it. Drawing is pure and quick enough to repeat several
//! times a second, which is how a pattern moves: [`Frame::time`] shifts it.

mod noise;
mod patterns;

pub use patterns::Pattern;

/// A rectangle in device pixels, from the top-left corner of the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// What to draw: a pattern over `width` by `height` device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Device pixels to a point.
    pub scale: f32,
    pub pattern: Pattern,
    /// Straight RGBA: each mark is this color, its alpha scaled by coverage.
    pub ink: [u8; 4],
    /// The pattern fades out toward this area and leaves it empty.
    pub clear: Option<Rect>,
    /// Seconds into the pattern's slow drift; a still pattern stays at one time.
    pub time: f32,
}

/// The room the pattern fades over around the clear area, in points: none
/// within `CLEAR_NEAR`, full by `CLEAR_FAR`.
const CLEAR_NEAR: f32 = 16.;
const CLEAR_FAR: f32 = 120.;

impl Frame {
    /// The frame's pixels, BGRA with straight alpha, row by row.
    pub fn render(&self) -> Vec<u8> {
        // Bare pixels carry the ink's color at no alpha, so filtering between
        // them and a mark never darkens its edge.
        let [r, g, b, _] = self.ink;
        let mut pixels = [b, g, r, 0].repeat(self.width as usize * self.height as usize);
        if self.width == 0 || self.height == 0 || self.scale <= 0. {
            return pixels;
        }
        patterns::draw(self, &mut Canvas::new(self, &mut pixels));
        pixels
    }

    /// How much of the pattern shows at a point given in device pixels: 0 in
    /// and near the clear area, rising to 1 away from it.
    pub(crate) fn openness(&self, x: f32, y: f32) -> f32 {
        let Some(clear) = self.clear else {
            return 1.;
        };
        let dx = (clear.x - x).max(x - (clear.x + clear.width)).max(0.);
        let dy = (clear.y - y).max(y - (clear.y + clear.height)).max(0.);
        let distance = dx.hypot(dy) / self.scale;
        noise::smoothstep(CLEAR_NEAR, CLEAR_FAR, distance)
    }
}

/// The pixels being drawn, and the ink to draw them in.
pub(crate) struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
    bgra: [u8; 4],
}

impl<'a> Canvas<'a> {
    fn new(frame: &Frame, pixels: &'a mut [u8]) -> Self {
        let [r, g, b, a] = frame.ink;
        Self {
            pixels,
            width: frame.width,
            height: frame.height,
            bgra: [b, g, r, a],
        }
    }

    /// Inks the `width` by `height` block whose top-left corner is at `x`,
    /// `y`, clipped to the frame.
    pub(crate) fn block(&mut self, x: u32, y: u32, width: u32, height: u32) {
        if x >= self.width {
            return;
        }
        let right = (x + width).min(self.width);
        for row in y..(y + height).min(self.height) {
            let start = (row * self.width + x) as usize * 4;
            let end = (row * self.width + right) as usize * 4;
            for pixel in self.pixels[start..end].as_chunks_mut::<4>().0 {
                *pixel = self.bgra;
            }
        }
    }

    /// Inks one pixel at `coverage` of the ink's alpha.
    pub(crate) fn blend(&mut self, x: u32, y: u32, coverage: f32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let at = (y * self.width + x) as usize * 4;
        let [b, g, r, a] = self.bgra;
        let alpha = (a as f32 * coverage.clamp(0., 1.)).round() as u8;
        self.pixels[at..at + 4].copy_from_slice(&[b, g, r, alpha]);
    }
}
