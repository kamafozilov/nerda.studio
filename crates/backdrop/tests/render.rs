use backdrop::{Frame, Pattern, Rect};

const PATTERNS: [Pattern; 5] = [
    Pattern::Horizon,
    Pattern::Clouds,
    Pattern::Contours,
    Pattern::Halftone,
    Pattern::Characters,
];

fn frame(pattern: Pattern) -> Frame {
    Frame {
        width: 800,
        height: 500,
        scale: 2.,
        pattern,
        ink: [255, 255, 255, 40],
        clear: Some(Rect {
            x: 200.,
            y: 180.,
            width: 400.,
            height: 140.,
        }),
        time: 0.,
    }
}

/// The frame's pixels, four bytes each.
fn each(pixels: &[u8]) -> impl Iterator<Item = &[u8; 4]> {
    pixels.as_chunks::<4>().0.iter()
}

fn alpha_at(pixels: &[u8], width: u32, x: u32, y: u32) -> u8 {
    pixels[(y * width + x) as usize * 4 + 3]
}

#[test]
fn every_pattern_marks_something_and_keeps_the_clear_area_bare() {
    for pattern in PATTERNS {
        let frame = frame(pattern);
        let pixels = frame.render();
        assert_eq!(pixels.len(), 800 * 500 * 4);
        let marked = each(&pixels).filter(|pixel| pixel[3] > 0).count();
        assert!(marked > 1000, "{pattern:?} marked {marked} pixels");
        // The clear area and the 16 points around it stay bare.
        for y in (180 - 32)..(320 + 32) {
            for x in (200 - 32)..(600 + 32) {
                assert_eq!(alpha_at(&pixels, 800, x, y), 0, "{pattern:?} at {x}, {y}");
            }
        }
    }
}

#[test]
fn marks_are_the_ink_in_bgra() {
    let mut frame = frame(Pattern::Clouds);
    frame.ink = [10, 20, 30, 40];
    let pixels = frame.render();
    let mark = each(&pixels).find(|pixel| pixel[3] > 0).unwrap();
    assert_eq!(*mark, [30, 20, 10, 40]);
    let bare = each(&pixels).find(|pixel| pixel[3] == 0).unwrap();
    assert_eq!(*bare, [30, 20, 10, 0]);
}

#[test]
fn a_still_frame_draws_the_same_twice_and_time_moves_it() {
    for pattern in PATTERNS {
        let still = frame(pattern);
        assert_eq!(still.render(), still.render());
        let later = Frame { time: 3., ..still };
        assert_ne!(still.render(), later.render(), "{pattern:?} did not move");
    }
}

#[test]
fn an_empty_or_unscaled_frame_draws_nothing() {
    let empty = Frame {
        width: 0,
        ..frame(Pattern::Horizon)
    };
    assert!(empty.render().is_empty());
    let unscaled = Frame {
        scale: 0.,
        ..frame(Pattern::Horizon)
    };
    assert!(each(&unscaled.render()).all(|pixel| pixel[3] == 0));
}

#[test]
fn a_frame_smaller_than_a_cell_draws_without_panicking() {
    for pattern in PATTERNS {
        for (width, height) in [(1, 1), (3, 700), (700, 3)] {
            Frame {
                width,
                height,
                clear: None,
                ..frame(pattern)
            }
            .render();
        }
    }
}
