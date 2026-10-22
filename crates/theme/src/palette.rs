//! Light and dark values, as `0xRRGGBBAA`.
//!
//! Computed from OKLCH with one cool neutral hue (h 265, C 0.006; text C 0.009),
//! never picked by eye; `docs/design.md` has the source values and the contrast
//! each text color keeps on glass over a white or a black desktop.
//!
//! The glass tints are the most transparent 8-bit alphas that keep
//! `text_secondary` at 4.5:1 over the worst desktop for each appearance (white
//! behind dark, black behind light). macOS adds no tint of its own (measured), so
//! nothing else holds the contrast.

pub(crate) struct Palette {
    pub window_glass: u32,
    pub window_solid: u32,
    pub panel: u32,
    pub border: u32,
    pub hover: u32,
    pub row_selected: u32,
    pub text_selection: u32,
    pub field: u32,
    pub focus_ring: u32,
    pub popover: u32,
    pub scrim: u32,
    pub key: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
}

pub(crate) const DARK: Palette = Palette {
    window_glass: 0x0e0f12c7,
    window_solid: 0x0e0f12ff,
    panel: 0x151619b8,
    border: 0xeff2f81a,
    hover: 0xeff2f80f,
    row_selected: 0xeff2f81a,
    text_selection: 0xeff2f833,
    field: 0xeff2f80f,
    focus_ring: 0xeff2f87b,
    popover: 0x1d1e21ff,
    scrim: 0x151619a3,
    key: 0xeff2f81a,
    text_primary: 0xeff2f8ff,
    text_secondary: 0xaeb1b7ff,
};

pub(crate) const LIGHT: Palette = Palette {
    window_glass: 0xf3f5f9ce,
    window_solid: 0xf3f5f9ff,
    panel: 0xfbfdffb8,
    border: 0x191b1f1a,
    hover: 0x191b1f0f,
    row_selected: 0x191b1f1a,
    text_selection: 0x191b1f33,
    field: 0x191b1f0f,
    focus_ring: 0x191b1f88,
    popover: 0xfbfdffff,
    scrim: 0xfbfdff99,
    key: 0x191b1f1a,
    text_primary: 0x191b1fff,
    text_secondary: 0x505358ff,
};
