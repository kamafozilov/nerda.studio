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
//!
//! Dark is opaque and neutral: its grays and text carry no hue, and hover and
//! selection are the same 8% white wash, so a hovered row reads as a chosen one.

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
    pub popover_border: u32,
    pub popover_shadow: u32,
    pub scrim: u32,
    pub key: u32,
    pub chip: u32,
    pub composer: u32,
    pub composer_focus: u32,
    pub tab: u32,
    pub tab_selected: u32,
    pub claude: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_secondary_on_surface: u32,
    pub text_tertiary: u32,
    /// Project badges as (fill, initials), one per hue: red, orange, yellow,
    /// green, teal, blue, violet, pink. Fill at L 0.38 C 0.075 (dark) or L 0.91
    /// C 0.045 (light), initials at L 0.87 C 0.10 or L 0.43 C 0.12, chroma cut to
    /// stay in sRGB: the initials keep at least 6.7:1 (dark) and 5.98:1 (light).
    pub badges: [(u32, u32); 8],
}

pub(crate) const DARK: Palette = Palette {
    window_glass: 0x0a0a0bff,
    window_solid: 0x0a0a0bff,
    panel: 0x131314ff,
    border: 0xffffff14,
    hover: 0xffffff14,
    row_selected: 0xffffff14,
    text_selection: 0xffffff33,
    field: 0xffffff0f,
    focus_ring: 0xffffff7b,
    popover: 0x202021ff,
    popover_border: 0xffffff1f,
    popover_shadow: 0x00000059,
    scrim: 0x151619a3,
    key: 0xffffff1a,
    chip: 0xffffff0f,
    composer: 0x202021ff,
    composer_focus: 0xffffff38,
    tab: 0x131313ff,
    tab_selected: 0x202021ff,
    claude: 0xd97757ff,
    text_primary: 0xf5f5f5ff,
    text_secondary: 0xa0a0a1ff,
    text_secondary_on_surface: 0xa0a0a1ff,
    text_tertiary: 0x797979ff,
    badges: [
        (0x64312eff, 0xfec4beff),
        (0x5f3813ff, 0xffc79bff),
        (0x4f4105ff, 0xe9d487ff),
        (0x264d28ff, 0xabe6acff),
        (0x004e47ff, 0x81eaddff),
        (0x0b4864ff, 0xa5ddfeff),
        (0x3f3c68ff, 0xcfcfffff),
        (0x583355ff, 0xfabdf4ff),
    ],
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
    popover_border: 0x191b1f24,
    popover_shadow: 0x00000021,
    scrim: 0xfbfdff99,
    key: 0x191b1f1a,
    chip: 0x191b1f0f,
    composer: 0xffffffff,
    composer_focus: 0x191b1f38,
    tab: 0x191b1f0a,
    tab_selected: 0x191b1f1a,
    claude: 0xc6613fff,
    text_primary: 0x191b1fff,
    text_secondary: 0x505358ff,
    text_secondary_on_surface: 0x45474cff,
    text_tertiary: 0x505358ff,
    badges: [
        (0xfed6d2ff, 0x852e2cff),
        (0xf9dbc4ff, 0x764002ff),
        (0xeae2c0ff, 0x5f4f01ff),
        (0xd0eacfff, 0x176020ff),
        (0xc1ece5ff, 0x065d55ff),
        (0xc5e7fbff, 0x03577aff),
        (0xdddeffff, 0x4b438eff),
        (0xf3d7f0ff, 0x73336fff),
    ],
};
