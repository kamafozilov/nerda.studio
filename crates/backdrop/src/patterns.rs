//! The patterns: what each one draws and how it moves.
//!
//! Each pattern reads a field, a lightness in `0..1` over the frame, and
//! marks it in its own way: a dot per cell where the field beats the ordered
//! dither's threshold, a dot sized by the field, or a character as dense.
//! Given an image, the image's lightness is the field, shaped by the pattern.

use crate::{Canvas, Frame, noise};

/// A pattern behind the composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// Light rising from below the frame's bottom edge, dithered.
    Horizon,
    /// Drifting fractal clouds, dithered.
    Clouds,
    /// The contour lines of a fractal terrain, dithered.
    Contours,
    /// Round dots on a grid, larger toward the top-left corner.
    Halftone,
    /// Characters, denser where the clouds are brighter.
    Characters,
    /// The image itself, every pixel; nothing without one.
    Image,
}

/// The horizon's brightest: never every dot, so its brightest band keeps
/// its texture.
const HORIZON_PEAK: f32 = 0.8;
/// A dithered dot's side, in points.
const DOT: f32 = 2.;
/// A halftone cell's side, in points.
const HALFTONE_CELL: f32 = 6.;
/// A character's cell: its 5 by 7 glyph, in points, and the room around it.
const GLYPH_WIDTH: u32 = 5;
const GLYPH_HEIGHT: u32 = 7;
const CHARACTER_CELL: (u32, u32) = (6, 9);
/// Points to a unit of the clouds' and the terrain's noise.
const CLOUD_SCALE: f32 = 220.;
const TERRAIN_SCALE: f32 = 300.;

/// The 8 by 8 ordered-dither thresholds, `0..64`.
const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Darkest to densest: space, `.`, `:`, `-`, `=`, `+`, `*`, `#`, each row's
/// five pixels in its low bits, leftmost highest.
const GLYPHS: [[u8; GLYPH_HEIGHT as usize]; 8] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0b00100, 0],
    [0, 0b00100, 0, 0, 0, 0b00100, 0],
    [0, 0, 0, 0b01110, 0, 0, 0],
    [0, 0, 0b11111, 0, 0b11111, 0, 0],
    [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
    [0, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0],
    [
        0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010,
    ],
];

/// Where a point stands in the frame, in points, and when.
struct Place {
    /// From the top-left corner, in points.
    x: f32,
    y: f32,
    /// The same as a share of the frame's width and height.
    u: f32,
    v: f32,
    /// Width over height, so round shapes stay round.
    aspect: f32,
    time: f32,
    /// The image's call for marks here, when there is an image.
    density: Option<f32>,
}

impl Pattern {
    /// The field the pattern marks, in `0..1` (it may run over 1): its own,
    /// or the image's shaped by it.
    fn field(self, at: &Place) -> f32 {
        let Some(density) = at.density else {
            return self.own_field(at);
        };
        match self {
            // Rising from the bottom edge as the horizon does.
            Pattern::Horizon => density * self.own_field(at) / HORIZON_PEAK,
            // Thinned where the clouds part.
            Pattern::Clouds => density * (0.4 + 0.6 * self.own_field(at)),
            // The image's own contour lines.
            Pattern::Contours => {
                let line = (density * 24. - at.time * 0.4).sin().abs();
                (1. - noise::smoothstep(0., 0.3, line)) * 0.85
            }
            Pattern::Halftone => {
                density * (0.9 + 0.1 * (at.u * 9. + at.v * 5. - at.time * 0.8).sin())
            }
            Pattern::Characters | Pattern::Image => density,
        }
    }

    /// The pattern's own field, without an image.
    fn own_field(self, at: &Place) -> f32 {
        let Place { x, y, u, v, .. } = *at;
        let time = at.time;
        match self {
            Pattern::Image => 0.,
            Pattern::Horizon => {
                let wobble = noise::fractal(x / 240. + time * 0.05, y / 240.) - 0.5;
                let breath = 1. + 0.03 * (time * 0.6).sin();
                let distance = ((u - 0.5) * 0.8).hypot(v - 1.3) * breath + wobble * 0.12;
                ((1.05 - distance).max(0.) * 1.6).min(HORIZON_PEAK)
            }
            Pattern::Clouds | Pattern::Characters => {
                let n = noise::fractal(
                    x / CLOUD_SCALE + 3. + time * 0.04,
                    y / CLOUD_SCALE + 1. + time * 0.015,
                );
                noise::smoothstep(0.3, 0.8, n)
            }
            Pattern::Contours => {
                let height = noise::fractal(
                    x / TERRAIN_SCALE + 7. + time * 0.01,
                    y / TERRAIN_SCALE + time * 0.015,
                );
                let line = (height * 30. - time * 0.4).sin().abs();
                (1. - noise::smoothstep(0., 0.3, line)) * 0.85
            }
            Pattern::Halftone => {
                let distance = (u * 1.1 * at.aspect.sqrt()).hypot(v);
                let ripple = 0.9 + 0.1 * (distance * 12. - time * 0.8).sin();
                (1.05 - distance * 1.15).max(0.) * ripple
            }
        }
    }
}

/// Draws the frame's pattern onto `canvas`.
pub(crate) fn draw(frame: &Frame, canvas: &mut Canvas) {
    match frame.pattern {
        Pattern::Horizon | Pattern::Clouds | Pattern::Contours => dither(frame, canvas),
        Pattern::Halftone => halftone(frame, canvas),
        Pattern::Characters => characters(frame, canvas),
        Pattern::Image => image(frame, canvas),
    }
}

/// The place of a point given in device pixels.
fn place(frame: &Frame, x: f32, y: f32) -> Place {
    let (width, height) = (frame.width as f32, frame.height as f32);
    Place {
        x: x / frame.scale,
        y: y / frame.scale,
        u: x / width,
        v: y / height,
        aspect: width / height,
        time: frame.time,
        density: (frame.source.as_deref()).map(|source| frame.density(source, x, y)),
    }
}

/// The field at a point given in device pixels, faded by the clear area.
fn shown(frame: &Frame, x: f32, y: f32) -> f32 {
    let open = frame.openness(x, y);
    if open <= 0. {
        return 0.;
    }
    frame.pattern.field(&place(frame, x, y)) * open
}

/// One square dot per cell where the field beats the cell's threshold, so
/// lighter areas hold more dots.
fn dither(frame: &Frame, canvas: &mut Canvas) {
    let dot = (DOT * frame.scale).round().max(1.) as u32;
    for (row, y) in (0..frame.height).step_by(dot as usize).enumerate() {
        for (column, x) in (0..frame.width).step_by(dot as usize).enumerate() {
            let center = dot as f32 / 2.;
            let field = shown(frame, x as f32 + center, y as f32 + center);
            let threshold = (BAYER[row % 8][column % 8] as f32 + 0.5) / 64.;
            if field > threshold
                && let Some(mark) = frame.mark(x as f32 + center, y as f32 + center)
            {
                canvas.block(x, y, dot, dot, mark);
            }
        }
    }
}

/// A round dot centered in each cell, its radius following the field, its
/// edge antialiased.
fn halftone(frame: &Frame, canvas: &mut Canvas) {
    let cell = HALFTONE_CELL * frame.scale;
    let rows = (frame.height as f32 / cell).ceil() as u32;
    let columns = (frame.width as f32 / cell).ceil() as u32;
    for row in 0..rows {
        for column in 0..columns {
            let center_x = (column as f32 + 0.5) * cell;
            let center_y = (row as f32 + 0.5) * cell;
            let field = shown(frame, center_x, center_y).min(1.);
            let radius = field * cell * 0.48;
            if radius < 0.35 * frame.scale {
                continue;
            }
            let Some(mark) = frame.mark(center_x, center_y) else {
                continue;
            };
            let left = (center_x - radius - 1.).max(0.) as u32;
            let top = (center_y - radius - 1.).max(0.) as u32;
            let right = (center_x + radius + 1.).ceil() as u32;
            let bottom = (center_y + radius + 1.).ceil() as u32;
            for y in top..bottom {
                for x in left..right {
                    let distance = (x as f32 + 0.5 - center_x).hypot(y as f32 + 0.5 - center_y);
                    let coverage = radius + 0.5 - distance;
                    if coverage > 0. {
                        canvas.blend(x, y, coverage, mark);
                    }
                }
            }
        }
    }
}

/// A character per cell, from space to `#` as the field rises, its glyph's
/// pixels a point each.
fn characters(frame: &Frame, canvas: &mut Canvas) {
    let pixel = frame.scale.round().max(1.) as u32;
    let (cell_width, cell_height) = (CHARACTER_CELL.0 * pixel, CHARACTER_CELL.1 * pixel);
    for top in (0..frame.height).step_by(cell_height as usize) {
        for left in (0..frame.width).step_by(cell_width as usize) {
            let (center_x, center_y) = (
                left as f32 + cell_width as f32 / 2.,
                top as f32 + cell_height as f32 / 2.,
            );
            let field = shown(frame, center_x, center_y);
            let Some(mark) = frame.mark(center_x, center_y) else {
                continue;
            };
            let density = ((field * GLYPHS.len() as f32) as usize).min(GLYPHS.len() - 1);
            for (row, bits) in GLYPHS[density].iter().enumerate() {
                for column in 0..GLYPH_WIDTH {
                    if bits & (1 << (GLYPH_WIDTH - 1 - column)) != 0 {
                        let x = left + column * pixel;
                        let y = top + row as u32 * pixel;
                        canvas.block(x, y, pixel, pixel, mark);
                    }
                }
            }
        }
    }
}

/// The image itself, each pixel its own color, or the ink at the image's
/// density. It covers the card whole, clear area included: the composer's
/// glass sets it apart. Each pixel is read between four of the image's, so a
/// small image scaled up stays smooth, and each column and row is placed
/// once, not once a pixel.
fn image(frame: &Frame, canvas: &mut Canvas) {
    let Some(source) = frame.source.as_deref() else {
        return;
    };
    let cover = source.cover(frame.width as f32, frame.height as f32);
    let columns: Vec<_> = (0..frame.width)
        .map(|x| source.columns_around(cover, x as f32 + 0.5))
        .collect();
    for y in 0..frame.height {
        let rows = source.rows_around(cover, y as f32 + 0.5);
        for (x, &columns) in (0..frame.width).zip(&columns) {
            let [r, g, b] = source
                .color_between(columns, rows)
                .map(|channel| channel.round() as u8);
            let (mark, coverage) = match frame.ink {
                crate::Ink::Solid([ir, ig, ib, a]) => {
                    let lightness = (r as f32 * 54. + g as f32 * 183. + b as f32 * 19.) / 65280.;
                    let density = if frame.light {
                        1. - lightness
                    } else {
                        lightness
                    };
                    ([ib, ig, ir, a], density)
                }
                crate::Ink::Source { alpha } => ([b, g, r, alpha], 1.),
            };
            canvas.blend(x, y, coverage, mark);
        }
    }
}
