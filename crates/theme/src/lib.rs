//! The look of Nerda Studio: glass materials, semantic colors, metrics and motion.
//!
//! Components read colors only through [`Theme::of`] and its [`Colors`] roles,
//! never as literals, so the light and dark palettes and the solid fallback
//! can change without touching them. `docs/design.md` explains the rules.

mod metrics;
pub mod motion;
mod palette;

pub use metrics::*;

use gpui::{
    App, BoxShadow, Global, Hsla, WindowAppearance, WindowBackgroundAppearance, point, px, rgba,
};

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
    /// Edge of a menu: brighter than `border`, so a menu over a box of the
    /// same fill, such as the composer, keeps its own edge.
    pub popover_border: Hsla,
    /// The layered shadow under a menu, lifting it off what it covers.
    pub popover_shadow: Hsla,
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
    /// The composer's box, its project pill, its branch and the tray's
    /// controls while a draft's card draws the owner's image:
    /// `composer_background` at 88%, so the picture shows faintly through.
    /// Its text keeps 4.5:1 over the brightest image.
    pub composer_glass: Hsla,
    /// Fill of a tab that isn't selected, so each tab reads as a shape.
    pub tab_background: Hsla,
    /// Fill of the selected tab, under a `border` hairline.
    pub tab_selected: Hsla,
    /// Claude Code's logo, in its brand's orange.
    pub claude: Hsla,
    /// The pattern behind a new session's composer, in gray. Nothing is read
    /// on it: it clears away around the composer, so it may be faint.
    pub backdrop: Hsla,
    /// The same pattern in a warm orange.
    pub backdrop_warm: Hsla,
    /// Titles and body text.
    pub text_primary: Hsla,
    /// Supporting text and icons at rest.
    pub text_secondary: Hsla,
    /// Supporting text on a row's `hover` or `row_selected` surface, where
    /// `text_secondary` would fall under 4.5:1 over the worst desktop.
    pub text_secondary_on_surface: Hsla,
    /// Labels over a group and the quietest supporting text, such as a list's
    /// section title or a row's age; on a row's surface such text turns
    /// `text_secondary`. Dark keeps 4.55:1 on the base; light has no lighter
    /// gray that keeps 4.5:1 on glass, so it is `text_secondary` there.
    pub text_tertiary: Hsla,
    /// What failed, such as a tool call's mark and output.
    pub danger: Hsla,
    /// The count of lines a change added; the removed count uses `danger`.
    pub diff_added: Hsla,
    /// Under a diff's added line.
    pub diff_added_background: Hsla,
    /// Under a diff's removed line.
    pub diff_removed_background: Hsla,
    /// A link in the agent's text.
    pub link: Hsla,
    /// A file or folder the agent's text names: the chip's icon, fill and
    /// hairline.
    pub mention: Hsla,
    pub mention_background: Hsla,
    pub mention_border: Hsla,
    /// Code's tokens by what they are; comments use `text_secondary`, a
    /// diff's lines `diff_added` and `danger`.
    pub syntax: SyntaxColors,
    /// The working indicator's dots, by row from the top; nothing is read
    /// on them.
    pub activity: [Hsla; 3],
    /// The badges that tell projects apart by color; a project keeps its hue.
    pub badges: [BadgeColors; 8],
}

impl Colors {
    /// A menu's shadow: three tight layers rather than one wide blur, so the
    /// menu reads as lifted just off what it covers.
    pub fn popover_shadows(&self) -> Vec<BoxShadow> {
        [(3., 8.), (2., 5.), (1., 1.)]
            .into_iter()
            .map(|(y, blur)| BoxShadow {
                color: self.popover_shadow,
                offset: point(px(0.), px(y)),
                blur_radius: px(blur),
                spread_radius: px(0.),
                inset: false,
            })
            .collect()
    }
}

/// The colors of code's tokens, on a code block's `field_background`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyntaxColors {
    pub keyword: Hsla,
    pub string: Hsla,
    /// Numbers, `true`, constants.
    pub constant: Hsla,
    pub function: Hsla,
    pub type_name: Hsla,
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
                popover_border: rgba(palette.popover_border).into(),
                popover_shadow: rgba(palette.popover_shadow).into(),
                scrim: rgba(palette.scrim).into(),
                key_background: rgba(palette.key).into(),
                chip_background: rgba(palette.chip).into(),
                composer_background: rgba(palette.composer).into(),
                composer_glass: rgba(palette.composer_glass).into(),
                tab_background: rgba(palette.tab).into(),
                tab_selected: rgba(palette.tab_selected).into(),
                claude: rgba(palette.claude).into(),
                backdrop: rgba(palette.backdrop).into(),
                backdrop_warm: rgba(palette.backdrop_warm).into(),
                text_primary: rgba(palette.text_primary).into(),
                text_secondary: rgba(palette.text_secondary).into(),
                text_secondary_on_surface: rgba(palette.text_secondary_on_surface).into(),
                text_tertiary: rgba(palette.text_tertiary).into(),
                danger: rgba(palette.danger).into(),
                diff_added: rgba(palette.diff_added).into(),
                diff_added_background: rgba(palette.diff_added_background).into(),
                diff_removed_background: rgba(palette.diff_removed_background).into(),
                link: rgba(palette.link).into(),
                mention: rgba(palette.mention).into(),
                mention_background: rgba(palette.mention_background).into(),
                mention_border: rgba(palette.mention_border).into(),
                syntax: SyntaxColors {
                    keyword: rgba(palette.syntax[0]).into(),
                    string: rgba(palette.syntax[1]).into(),
                    constant: rgba(palette.syntax[2]).into(),
                    function: rgba(palette.syntax[3]).into(),
                    type_name: rgba(palette.syntax[4]).into(),
                },
                activity: palette.activity.map(|color| rgba(color).into()),
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
