//! Files the person attaches to a message: images the model sees, and any
//! other file named by its path for the agent to read.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The largest image sent inside a message; a larger one goes by its path.
pub const MAX_IMAGE: u64 = 5 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Attachment {
    /// Sent inside the message, so the model sees it.
    Image(Image),
    /// Named in the message by its absolute path; the agent reads it itself.
    File(PathBuf),
}

impl Attachment {
    /// The file at `path` as an attachment: a PNG, JPEG, GIF or WebP of at
    /// most 5 MiB, known by its first bytes whatever its name, is read in.
    /// Any other file goes by its absolute path, and so does an image that is
    /// larger or can't be read, so attaching never fails the message.
    pub fn open(path: &Path) -> Self {
        Image::read(path).map_or_else(
            || Attachment::File(std::path::absolute(path).unwrap_or_else(|_| path.to_owned())),
            Attachment::Image,
        )
    }
}

/// An image the model can see: its format is the one its bytes are in.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    format: ImageFormat,
    bytes: Arc<[u8]>,
}

impl Image {
    /// `bytes` as an image, when they are one the model takes, of at most 5 MiB.
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Option<Self> {
        let bytes = bytes.into();
        let format = ImageFormat::sniff(&bytes)?;
        (bytes.len() as u64 <= MAX_IMAGE).then_some(Self { format, bytes })
    }

    /// The bytes as base64, as a message carries them.
    pub fn base64(&self) -> String {
        STANDARD.encode(&self.bytes)
    }

    pub fn format(&self) -> ImageFormat {
        self.format
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The image at `path`, read only once its size and first bytes say it
    /// is one.
    fn read(path: &Path) -> Option<Self> {
        let mut file = std::fs::File::open(path).ok()?;
        if file.metadata().ok()?.len() > MAX_IMAGE {
            return None;
        }
        let mut head = [0u8; 12];
        let read = file.read(&mut head).ok()?;
        ImageFormat::sniff(&head[..read])?;
        Self::new(std::fs::read(path).ok()?)
    }
}

/// Written as its bytes in base64, and read back only as an image.
impl Serialize for Image {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.base64())
    }
}

impl<'de> Deserialize<'de> for Image {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let bytes = STANDARD.decode(text).map_err(serde::de::Error::custom)?;
        Image::new(bytes)
            .ok_or_else(|| serde::de::Error::custom("not an image the model takes, or over 5 MiB"))
    }
}

/// The image formats the agent's model takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageFormat {
    /// The format `bytes` start like, by their signature rather than a name.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        Some(match bytes {
            [0x89, b'P', b'N', b'G', ..] => ImageFormat::Png,
            [0xFF, 0xD8, 0xFF, ..] => ImageFormat::Jpeg,
            [b'G', b'I', b'F', b'8', ..] => ImageFormat::Gif,
            [
                b'R',
                b'I',
                b'F',
                b'F',
                _,
                _,
                _,
                _,
                b'W',
                b'E',
                b'B',
                b'P',
                ..,
            ] => ImageFormat::Webp,
            _ => return None,
        })
    }

    pub fn media_type(self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Webp => "image/webp",
        }
    }
}
