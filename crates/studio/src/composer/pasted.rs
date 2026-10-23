//! An image pasted from the clipboard, saved as a file in the data folder's
//! `attachments/` so it attaches by its path like any other image.

use std::{
    borrow::Cow,
    fs,
    io::{self, Cursor},
    path::{Path, PathBuf},
};

use gpui::{Image, ImageFormat};

/// Saves `image` in `folder` under a name taken from its bytes, so pasting the
/// same image again finds the same file. A PNG, JPEG, GIF or WebP keeps its
/// bytes; any other format becomes a PNG. On failure, says why in plain words.
pub fn save(image: &Image, folder: &Path) -> Result<PathBuf, String> {
    let (extension, bytes) = match image.format {
        ImageFormat::Png => ("png", Cow::Borrowed(image.bytes.as_slice())),
        ImageFormat::Jpeg => ("jpg", Cow::Borrowed(image.bytes.as_slice())),
        ImageFormat::Gif => ("gif", Cow::Borrowed(image.bytes.as_slice())),
        ImageFormat::Webp => ("webp", Cow::Borrowed(image.bytes.as_slice())),
        _ => (
            "png",
            Cow::Owned(to_png(&image.bytes).ok_or(
                "This image's format can't be attached. Save it as a PNG and attach the file.",
            )?),
        ),
    };
    let path = folder.join(format!("pasted-{:016x}.{extension}", image.id));
    if path.is_file() {
        return Ok(path);
    }
    write(folder, &path, &bytes).map_err(|error| {
        let folder = workspace::home_relative(folder);
        match error.kind() {
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
                format!("Nerda Studio isn't allowed to write to {folder}, so the pasted image wasn't attached.")
            }
            _ => format!("Couldn't save the pasted image in {folder}. Try again."),
        }
    })?;
    Ok(path)
}

/// Writes beside `path` first, so a half-written file never sits under the
/// name a later paste would take as already saved.
fn write(folder: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    let partial = path.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(&partial, path)
}

fn to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let decoded = image::load_from_memory(bytes).ok()?;
    let mut png = Cursor::new(Vec::new());
    decoded.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(png.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("nerda-pasted-{name}-{}", std::process::id()));
        fs::remove_dir_all(&folder).ok();
        folder
    }

    #[test]
    fn the_same_image_saves_once_under_a_name_from_its_bytes() {
        let folder = folder("same");
        let image = Image::from_bytes(ImageFormat::Png, vec![1, 2, 3]);
        let first = save(&image, &folder).unwrap();
        let second = save(&image, &folder).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.extension().unwrap(), "png");
        assert_eq!(fs::read(&first).unwrap(), [1, 2, 3]);
        let other = save(&Image::from_bytes(ImageFormat::Png, vec![4]), &folder).unwrap();
        assert_ne!(first, other);
        fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn another_format_becomes_a_png_and_an_unreadable_one_is_refused() {
        let folder = folder("convert");
        let mut bmp = Cursor::new(Vec::new());
        image::RgbImage::new(2, 2)
            .write_to(&mut bmp, image::ImageFormat::Bmp)
            .unwrap();
        let saved = save(
            &Image::from_bytes(ImageFormat::Bmp, bmp.into_inner()),
            &folder,
        )
        .unwrap();
        assert_eq!(saved.extension().unwrap(), "png");
        let png = image::load_from_memory(&fs::read(&saved).unwrap()).unwrap();
        assert_eq!((png.width(), png.height()), (2, 2));
        let svg = Image::from_bytes(ImageFormat::Svg, b"<svg/>".to_vec());
        assert!(
            save(&svg, &folder)
                .unwrap_err()
                .contains("can't be attached")
        );
        fs::remove_dir_all(&folder).ok();
    }
}
