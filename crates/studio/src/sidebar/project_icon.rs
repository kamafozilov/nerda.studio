//! A project's own icon, made ready off the UI thread for a session row's
//! `PROJECT_ICON` slot. A vector is drawn at the slot's pixel size rather
//! than its own, which may be far smaller; a bitmap is scaled down once.
//! Either is centered on a transparent square, so a mark that isn't square
//! keeps its shape. A wordmark wider or taller than twice the other side
//! reads as nothing at this size and is refused, so the initials show.
//!
//! Its tone is read from its drawn pixels: a dark mark on transparency
//! would vanish on the dark glass, a light one on the light, so the row
//! sets such a mark on a plate of the opposite tone.

use std::{path::Path, sync::Arc};

use gpui::RenderImage;
use image::{RgbaImage, imageops};

/// The square an icon is drawn to: the slot's 16 points at 3x, so it stays
/// sharp on any display.
const SIDE: u32 = 48;

/// How far from square a mark may be, as its longer side over its shorter.
const MAX_ASPECT: f32 = 2.;

/// Which way an icon's own colors lean, read from its opaque pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    /// A dark mark on transparency.
    Dark,
    /// A light mark on transparency.
    Light,
    /// Colors of its own, or a ground of its own: it reads on either glass.
    Own,
}

pub(super) struct ProjectIcon {
    pub image: Arc<RenderImage>,
    pub tone: Tone,
}

/// The icon at `path` made ready for the slot; `None` for a file that
/// doesn't read as an image, or a mark too far from square.
pub(super) fn load(path: &Path) -> Option<ProjectIcon> {
    let bytes = std::fs::read(path).ok()?;
    let is_svg = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
    let square = if is_svg {
        draw_svg(&bytes)?
    } else {
        scale_bitmap(&bytes)?
    };
    let tone = tone(&square);
    let mut pixels = square;
    // GPUI's images are BGRA.
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(ProjectIcon {
        image: Arc::new(RenderImage::new([image::Frame::new(pixels)])),
        tone,
    })
}

fn fits(width: f32, height: f32) -> bool {
    width > 0. && height > 0. && width.max(height) / width.min(height) <= MAX_ASPECT
}

fn draw_svg(bytes: &[u8]) -> Option<RgbaImage> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let (width, height) = (size.width(), size.height());
    if !fits(width, height) {
        return None;
    }
    let scale = SIDE as f32 / width.max(height);
    let offset = |side: f32| (SIDE as f32 - side * scale) / 2.;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIDE, SIDE)?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale)
        .post_translate(offset(width), offset(height));
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // The pixmap is premultiplied; images here are not.
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    RgbaImage::from_raw(SIDE, SIDE, pixels)
}

fn scale_bitmap(bytes: &[u8]) -> Option<RgbaImage> {
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    if !fits(width as f32, height as f32) {
        return None;
    }
    let scale = SIDE as f32 / width.max(height) as f32;
    let side = |length: u32| ((length as f32 * scale).round() as u32).clamp(1, SIDE);
    let scaled = imageops::resize(
        &image,
        side(width),
        side(height),
        imageops::FilterType::Lanczos3,
    );
    let mut square = RgbaImage::new(SIDE, SIDE);
    imageops::overlay(
        &mut square,
        &scaled,
        i64::from((SIDE - scaled.width()) / 2),
        i64::from((SIDE - scaled.height()) / 2),
    );
    Some(square)
}

/// Dark or light when the mark's opaque pixels are all of one lightness and
/// leave most of the square clear; its own otherwise.
fn tone(square: &RgbaImage) -> Tone {
    let opaque: Vec<f32> = square
        .pixels()
        .filter(|pixel| pixel.0[3] >= 128)
        .map(|pixel| {
            let [r, g, b, _] = pixel.0.map(|channel| f32::from(channel) / 255.);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        })
        .collect();
    let coverage = opaque.len() as f32 / (SIDE * SIDE) as f32;
    if opaque.is_empty() || coverage > 0.85 {
        return Tone::Own;
    }
    let darkest = opaque.iter().copied().fold(1., f32::min);
    let lightest = opaque.iter().copied().fold(0., f32::max);
    if lightest < 0.3 {
        Tone::Dark
    } else if darkest > 0.75 {
        Tone::Light
    } else {
        Tone::Own
    }
}
