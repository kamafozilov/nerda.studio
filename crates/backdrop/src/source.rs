//! An image a pattern is drawn from: its lightness takes the place of the
//! pattern's own field, and its colors may color the marks.

use std::sync::atomic::{AtomicU64, Ordering};

/// An image, decoded and ready to sample, scaled to cover the frame.
pub struct Source {
    /// Tells two sources apart without comparing their pixels.
    id: u64,
    width: u32,
    height: u32,
    /// Lightness, `0..255`, row by row.
    lightness: Box<[u8]>,
    /// Color, row by row.
    colors: Box<[[u8; 3]]>,
}

impl Source {
    /// A source from `width` by `height` straight RGBA pixels; a transparent
    /// pixel reads as black. `None` when the pixels don't fill that size.
    pub fn new(width: u32, height: u32, rgba: &[u8]) -> Option<Self> {
        let count = width as usize * height as usize;
        if count == 0 || rgba.len() != count * 4 {
            return None;
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let pixels = rgba.as_chunks::<4>().0;
        let colors: Box<[[u8; 3]]> = pixels
            .iter()
            .map(|&[r, g, b, a]| [r, g, b].map(|channel| (channel as u16 * a as u16 / 255) as u8))
            .collect();
        let lightness = colors
            .iter()
            .map(|&[r, g, b]| ((r as u32 * 54 + g as u32 * 183 + b as u32 * 19) >> 8) as u8)
            .collect();
        Some(Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            lightness,
            colors,
        })
    }

    /// The pixel under a point of a `frame_width` by `frame_height` frame,
    /// the image scaled to cover it and centered.
    fn index(&self, frame_width: f32, frame_height: f32, x: f32, y: f32) -> usize {
        let cover = self.cover(frame_width, frame_height);
        self.row(cover, y) + self.column(cover, x)
    }

    /// How the image covers a frame: its scale, and its left and top edges
    /// in the image's pixels.
    pub(crate) fn cover(&self, frame_width: f32, frame_height: f32) -> (f32, f32, f32) {
        let (width, height) = (self.width as f32, self.height as f32);
        let scale = (frame_width / width).max(frame_height / height);
        let left = (width - frame_width / scale) / 2.;
        let top = (height - frame_height / scale) / 2.;
        (scale, left, top)
    }

    /// The image's column under a frame's `x`.
    pub(crate) fn column(&self, (scale, left, _): (f32, f32, f32), x: f32) -> usize {
        (left + x / scale).clamp(0., self.width as f32 - 1.) as usize
    }

    /// The start of the image's row under a frame's `y`.
    pub(crate) fn row(&self, (scale, _, top): (f32, f32, f32), y: f32) -> usize {
        (top + y / scale).clamp(0., self.height as f32 - 1.) as usize * self.width as usize
    }

    /// Lightness in `0..1` and color at a pixel `index` of the image.
    pub(crate) fn at(&self, index: usize) -> (f32, [u8; 3]) {
        (self.lightness[index] as f32 / 255., self.colors[index])
    }

    /// Lightness in `0..1` under a point of the frame.
    pub(crate) fn lightness(&self, frame: (f32, f32), x: f32, y: f32) -> f32 {
        self.lightness[self.index(frame.0, frame.1, x, y)] as f32 / 255.
    }

    /// Color under a point of the frame.
    pub(crate) fn color(&self, frame: (f32, f32), x: f32, y: f32) -> [u8; 3] {
        self.colors[self.index(frame.0, frame.1, x, y)]
    }
}

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Source({}, {}x{})", self.id, self.width, self.height)
    }
}
