//! What stands behind a new session's composer: a pattern, its color, and
//! whether it moves.

use serde::{Deserialize, Deserializer, Serialize};

/// The pattern behind a draft's composer, or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BackdropStyle {
    None,
    #[default]
    Horizon,
    Clouds,
    Contours,
    Halftone,
    Characters,
}

/// The ink a pattern is drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BackdropTint {
    #[default]
    Gray,
    Warm,
}

/// The backdrop as chosen: a horizon in gray that holds still, until the
/// owner picks otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Backdrop {
    #[serde(default, deserialize_with = "named")]
    pub style: BackdropStyle,
    #[serde(default, deserialize_with = "named")]
    pub tint: BackdropTint,
    /// The pattern drifts slowly; it holds still while the system reduces motion.
    #[serde(default)]
    pub moving: bool,
}

impl BackdropStyle {
    pub const ALL: [BackdropStyle; 6] = [
        BackdropStyle::None,
        BackdropStyle::Horizon,
        BackdropStyle::Clouds,
        BackdropStyle::Contours,
        BackdropStyle::Halftone,
        BackdropStyle::Characters,
    ];

    pub fn title(self) -> &'static str {
        match self {
            BackdropStyle::None => "None",
            BackdropStyle::Horizon => "Horizon",
            BackdropStyle::Clouds => "Clouds",
            BackdropStyle::Contours => "Contours",
            BackdropStyle::Halftone => "Halftone",
            BackdropStyle::Characters => "Characters",
        }
    }
}

impl BackdropTint {
    pub const ALL: [BackdropTint; 2] = [BackdropTint::Gray, BackdropTint::Warm];

    pub fn title(self) -> &'static str {
        match self {
            BackdropTint::Gray => "Gray",
            BackdropTint::Warm => "Warm",
        }
    }
}

/// A choice saved by name, so a name from a newer build reads as the default
/// rather than as a corrupt file.
trait Named: Copy + Default + 'static {
    const ALL: &'static [Self];
    /// The name the saved workspace knows it by; never changes once shipped.
    fn key(self) -> &'static str;
}

impl Named for BackdropStyle {
    const ALL: &'static [Self] = &BackdropStyle::ALL;
    fn key(self) -> &'static str {
        match self {
            BackdropStyle::None => "none",
            BackdropStyle::Horizon => "horizon",
            BackdropStyle::Clouds => "clouds",
            BackdropStyle::Contours => "contours",
            BackdropStyle::Halftone => "halftone",
            BackdropStyle::Characters => "characters",
        }
    }
}

impl Named for BackdropTint {
    const ALL: &'static [Self] = &BackdropTint::ALL;
    fn key(self) -> &'static str {
        match self {
            BackdropTint::Gray => "gray",
            BackdropTint::Warm => "warm",
        }
    }
}

fn named<'de, D: Deserializer<'de>, T: Named>(deserializer: D) -> Result<T, D::Error> {
    let key = String::deserialize(deserializer)?;
    Ok((T::ALL.iter().copied())
        .find(|each| each.key() == key)
        .unwrap_or_default())
}

impl Serialize for BackdropStyle {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

impl Serialize for BackdropTint {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}
