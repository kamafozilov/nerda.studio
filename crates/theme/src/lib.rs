//! The look of Nerda Studio: glass materials, semantic colors, metrics and motion.
//!
//! Components read colors only through [`Theme::of`] and its [`Colors`] roles,
//! never as literals, so the light and dark palettes and the solid fallback
//! can change without touching them. `docs/design.md` explains the rules.

mod metrics;
pub mod motion;
mod palette;

pub use metrics::*;

use gpui::{App, Global, Hsla, WindowAppearance, WindowBackgroundAppearance, rgba};

/// The platform's monospaced font, for code: SF Mono on macOS, by the name
/// CoreText gives the system's monospaced face; Consolas on Windows.
pub const MONOSPACE_FONT: &str = if cfg!(target_os = "macos") {
    ".AppleSystemUIFontMonospaced"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "DejaVu Sans Mono"
};

/// Whether the window shows the desktop through a blur or paints a solid base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// The platform blurs what is behind the window (macOS, Windows).
    Glass,
    /// The base is opaque: on Linux, where no blur is guaranteed, and wherever
    /// the user asked the system to reduce transparency.
    Solid,
}

impl Material {
    fn new(reduce_transparency: bool) -> Self {
        if cfg!(any(target_os = "macos", target_os = "windows")) && !reduce_transparency {
            Self::Glass
        } else {
            Self::Solid
        }
    }
}

/// Colors by role. Each field names the job it does, not its value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    /// The window's base: tints the blurred desktop, or covers it when solid.
    pub window_background: Hsla,
    /// The main area, raised over the window base.
    pub panel_background: Hsla,
    /// Hairline between regions, such as the sidebar's edge.
    pub border: Hsla,
    /// Wash under a hovered control or row.
    pub hover: Hsla,
    /// Wash under the selected row; its text uses `text_primary`.
    pub row_selected: Hsla,
    /// Highlight behind selected text in a text field.
    pub text_selection: Hsla,
    /// Fill of a text field, so it reads as a place to type before it's focused.
    pub field_background: Hsla,
    /// Edge of the focused text field: the faintest that keeps 3:1 against the field
    /// and the sidebar over any desktop (WCAG 1.4.11).
    pub focus_ring: Hsla,
    /// A menu or popover over the window. Opaque: GPUI does not blur the app's
    /// own content, so nothing translucent may float over it.
    pub popover_background: Hsla,
    /// Dims the window behind a dialog, so the dialog reads as the one thing to act
    /// on. Nothing is read on it, so it may be translucent.
    pub scrim: Hsla,
    /// Fill of a key cap, a shortcut shown beside a label; its text uses
    /// `text_secondary`.
    pub key_background: Hsla,
    /// Fill of an attachment chip in the composer, at rest; its name uses
    /// `text_primary`, its icon `text_secondary`.
    pub chip_background: Hsla,
    /// Fill of the composer's box and the project pill over it: opaque, the
    /// selected tab's gray, so the place to type stands out of the card.
    pub composer_background: Hsla,
    /// Edge of the composer's box while its text has focus: quieter than
    /// `focus_ring`, since the box is large and the cursor already shows
    /// where typing goes.
    pub composer_focus: Hsla,
    /// Fill of a tab that isn't selected, so each tab reads as a shape.
    pub tab_background: Hsla,
    /// Fill of the selected tab, under a `border` hairline.
    pub tab_selected: Hsla,
    /// Claude Code's logo, in its brand's orange.
    pub claude: Hsla,
    /// Titles and body text.
    pub text_primary: Hsla,
    /// Supporting text and icons at rest.
    pub text_secondary: Hsla,
    /// Supporting text on a row's `hover` or `row_selected` surface, where
    /// `text_secondary` would fall under 4.5:1 over the worst desktop.
    pub text_secondary_on_surface: Hsla,
    /// Fill of the button that does something that can't be taken back, such
    /// as Quit; its text uses `text_on_danger`.
    pub danger: Hsla,
    /// Text and icons on `danger`: 5.18:1.
    pub text_on_danger: Hsla,
    /// The badges that tell projects apart by color; a project keeps its hue.
    pub badges: [BadgeColors; 8],
}

/// One project badge: its fill and the initials on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadgeColors {
    pub fill: Hsla,
    pub text: Hsla,
}

/// The active theme, one per app, held as a GPUI global.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub material: Material,
    pub colors: Colors,
}

impl Global for Theme {}

impl Theme {
    /// The theme for the system's appearance and its reduce-transparency
    /// setting, on this platform.
    pub fn new(appearance: WindowAppearance, reduce_transparency: bool) -> Self {
        let material = Material::new(reduce_transparency);
        let palette = match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => &palette::DARK,
            WindowAppearance::Light | WindowAppearance::VibrantLight => &palette::LIGHT,
        };
        let window_background = match material {
            Material::Glass => palette.window_glass,
            Material::Solid => palette.window_solid,
        };
        Self {
            material,
            colors: Colors {
                window_background: rgba(window_background).into(),
                panel_background: rgba(palette.panel).into(),
                border: rgba(palette.border).into(),
                hover: rgba(palette.hover).into(),
                row_selected: rgba(palette.row_selected).into(),
                text_selection: rgba(palette.text_selection).into(),
                field_background: rgba(palette.field).into(),
                focus_ring: rgba(palette.focus_ring).into(),
                popover_background: rgba(palette.popover).into(),
                scrim: rgba(palette.scrim).into(),
                key_background: rgba(palette.key).into(),
                chip_background: rgba(palette.chip).into(),
                composer_background: rgba(palette.composer).into(),
                composer_focus: rgba(palette.composer_focus).into(),
                tab_background: rgba(palette.tab).into(),
                tab_selected: rgba(palette.tab_selected).into(),
                claude: rgba(palette.claude).into(),
                text_primary: rgba(palette.text_primary).into(),
                text_secondary: rgba(palette.text_secondary).into(),
                text_secondary_on_surface: rgba(palette.text_secondary_on_surface).into(),
                danger: rgba(palette.danger).into(),
                text_on_danger: rgba(palette.text_on_danger).into(),
                badges: palette.badges.map(|(fill, text)| BadgeColors {
                    fill: rgba(fill).into(),
                    text: rgba(text).into(),
                }),
            },
        }
    }

    pub fn of(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// What the platform draws behind the window's own pixels.
    pub fn window_background_appearance(&self) -> WindowBackgroundAppearance {
        match self.material {
            Material::Glass => WindowBackgroundAppearance::Blurred,
            Material::Solid => WindowBackgroundAppearance::Opaque,
        }
    }
}
