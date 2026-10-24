//! What stands behind a new session's composer: a pattern, its color, how
//! strong it is, an image it may draw (or keep aside), and whether it moves.

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
    /// The image's own colors; gray while there is no image.
    Image,
}

/// How strongly the pattern shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BackdropStrength {
    Subtle,
    #[default]
    Medium,
    Bold,
}

/// The backdrop as chosen: a horizon in gray at medium strength that holds
/// still, with no image, until the owner picks otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Backdrop {
    #[serde(default, deserialize_with = "named")]
    pub style: BackdropStyle,
    #[serde(default, deserialize_with = "named")]
    pub tint: BackdropTint,
    #[serde(default, deserialize_with = "named")]
    pub strength: BackdropStrength,
    /// The pattern drifts slowly; it holds still while the system reduces motion.
    #[serde(default)]
    pub moving: bool,
    /// The image the pattern draws: a file's name in the app's backdrop
    /// folder. A name that would reach outside that folder reads as none.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "file_name"
    )]
    pub image: Option<String>,
    /// The image is kept but set aside: the pattern draws without it until
    /// it is shown again.
    #[serde(default, skip_serializing_if = "is_false")]
    pub image_hidden: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl Backdrop {
    /// Whether `name` can name an image in the backdrop folder: a file right
    /// in it, not hidden, with no separator, so it never reaches elsewhere.
    pub fn is_image_name(name: &str) -> bool {
        !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', ':'])
    }

    /// The image the pattern draws: the chosen one, unless it is hidden.
    pub fn image_in_use(&self) -> Option<&str> {
        self.image.as_deref().filter(|_| !self.image_hidden)
    }

    /// The ink in use: the image's colors only while an image is in use.
    pub fn tint_in_use(&self) -> BackdropTint {
        match (self.tint, self.image_in_use()) {
            (BackdropTint::Image, None) => BackdropTint::Gray,
            (tint, _) => tint,
        }
    }
}

fn file_name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    let name = Option::<String>::deserialize(deserializer)?;
    Ok(name.filter(|name| Backdrop::is_image_name(name)))
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
    pub const ALL: [BackdropTint; 3] =
        [BackdropTint::Gray, BackdropTint::Warm, BackdropTint::Image];

    pub fn title(self) -> &'static str {
        match self {
            BackdropTint::Gray => "Gray",
            BackdropTint::Warm => "Warm",
            BackdropTint::Image => "Image",
        }
    }
}

impl BackdropStrength {
    pub const ALL: [BackdropStrength; 3] = [
        BackdropStrength::Subtle,
        BackdropStrength::Medium,
        BackdropStrength::Bold,
    ];

    pub fn title(self) -> &'static str {
        match self {
            BackdropStrength::Subtle => "Subtle",
            BackdropStrength::Medium => "Medium",
            BackdropStrength::Bold => "Bold",
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
            BackdropTint::Image => "image",
        }
    }
}

impl Named for BackdropStrength {
    const ALL: &'static [Self] = &BackdropStrength::ALL;
    fn key(self) -> &'static str {
        match self {
            BackdropStrength::Subtle => "subtle",
            BackdropStrength::Medium => "medium",
            BackdropStrength::Bold => "bold",
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

impl Serialize for BackdropStrength {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

impl Serialize for BackdropTint {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}
