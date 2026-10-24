use std::sync::Arc;

use backdrop::{Frame, Ink, Pattern, Source};

/// Black on the left half, white on the right.
fn halves() -> Arc<Source> {
    let mut rgba = Vec::new();
    for _ in 0..10 {
        for x in 0..20 {
            let value = if x < 10 { 0 } else { 255 };
            rgba.extend([value, value, value, 255]);
        }
    }
    Arc::new(Source::new(20, 10, &rgba).unwrap())
}

fn frame(pattern: Pattern, ink: Ink, light: bool) -> Frame {
    Frame {
        width: 400,
        height: 200,
        scale: 2.,
        pattern,
        ink,
        source: Some(halves()),
        light,
        time: 0.,
    }
}

/// Marked pixels in the left and the right half.
fn marked_halves(frame: &Frame) -> (usize, usize) {
    let pixels = frame.render();
    let mut halves = (0, 0);
    for (at, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if pixel[3] > 0 {
            if (at as u32 % frame.width) < frame.width / 2 {
                halves.0 += 1;
            } else {
                halves.1 += 1;
            }
        }
    }
    halves
}

#[test]
fn a_source_needs_pixels_that_fill_its_size() {
    assert!(Source::new(2, 2, &[0; 16]).is_some());
    assert!(Source::new(2, 2, &[0; 15]).is_none());
    assert!(Source::new(0, 2, &[]).is_none());
}

#[test]
fn on_a_dark_surface_light_areas_draw_the_marks_and_on_a_light_one_dark_areas() {
    let ink = Ink::Solid([255, 255, 255, 60]);
    for pattern in [
        Pattern::Clouds,
        Pattern::Halftone,
        Pattern::Characters,
        Pattern::Image,
    ] {
        let (left, right) = marked_halves(&frame(pattern, ink, false));
        assert!(right > left * 4, "{pattern:?} dark: {left} / {right}");
        let (left, right) = marked_halves(&frame(pattern, ink, true));
        assert!(left > right * 4, "{pattern:?} light: {left} / {right}");
    }
}

#[test]
fn the_source_ink_colors_each_mark_with_the_image() {
    let frame = frame(Pattern::Image, Ink::Source { alpha: 90 }, false);
    let pixels = frame.render();
    let right = (100 * frame.width + 300) as usize * 4;
    assert_eq!(pixels[right..right + 4], [255, 255, 255, 90]);
    let left = (100 * frame.width + 100) as usize * 4;
    assert_eq!(pixels[left..left + 4], [0, 0, 0, 90]);
}

#[test]
fn without_a_source_the_image_and_its_colors_draw_nothing() {
    let mut image = frame(Pattern::Image, Ink::Solid([255, 255, 255, 60]), false);
    image.source = None;
    assert_eq!(marked_halves(&image), (0, 0));
    let mut colored = frame(Pattern::Clouds, Ink::Source { alpha: 90 }, false);
    colored.source = None;
    assert_eq!(marked_halves(&colored), (0, 0));
}

#[test]
fn the_image_alone_shows_every_pixel_as_it_is() {
    let white = Arc::new(Source::new(4, 4, &[255; 64]).unwrap());
    let frame = Frame {
        width: 400,
        height: 200,
        scale: 1.,
        pattern: Pattern::Image,
        ink: Ink::Source { alpha: 200 },
        source: Some(white),
        light: false,
        time: 0.,
    };
    let pixels = frame.render();
    for (x, y) in [(5, 5), (200, 100), (160, 80)] {
        let at = (y * frame.width + x) as usize * 4;
        assert_eq!(pixels[at..at + 4], [255, 255, 255, 200], "at {x}, {y}");
    }
}
