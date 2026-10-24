//! The owner's image behind the pattern. Adding one copies it into the
//! app's folder, under `backdrops/`, so it outlives the file it came from;
//! it is decoded off the UI thread, scaled down to `LARGEST` pixels a side.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use backdrop::Source;
use gpui::{App, Context, SharedString, Task};

use super::Backdrop;

/// The folder, beside the workspace, that keeps the images.
const FOLDER: &str = "backdrops";
/// The longest side an image is decoded to: more than a card shows.
const LARGEST: u32 = 2048;
/// The largest file taken, so a stray huge file can't stall the app.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// The owner's image and where reading it stands.
#[derive(Default)]
pub(super) struct Image {
    /// The file the state is for.
    name: Option<String>,
    state: ImageState,
}

/// Where the image stands.
#[derive(Clone, Debug, Default)]
pub enum ImageState {
    /// No image was chosen.
    #[default]
    None,
    Loading,
    Ready(Arc<Source>),
    /// The file is gone or no longer reads.
    Failed,
}

impl ImageState {
    pub fn source(&self) -> Option<Arc<Source>> {
        match self {
            ImageState::Ready(source) => Some(source.clone()),
            _ => None,
        }
    }
}

impl Backdrop {
    /// The image named `name` in the backdrop folder beside `data`, read the
    /// first time it is asked for.
    pub fn image(
        &mut self,
        data: Option<&Path>,
        name: Option<&str>,
        cx: &mut Context<Self>,
    ) -> ImageState {
        if self.image.name.as_deref() != name {
            self.image = Image {
                name: name.map(str::to_owned),
                state: ImageState::None,
            };
            if let (Some(data), Some(name)) = (data, name) {
                self.image.state = ImageState::Loading;
                let path = data.join(FOLDER).join(name);
                let name = name.to_owned();
                cx.spawn(async move |backdrop, cx| {
                    let read = (cx.background_executor())
                        .spawn(async move { fs::read(path).ok().and_then(|bytes| decode(&bytes)) })
                        .await;
                    backdrop
                        .update(cx, |backdrop, cx| {
                            if backdrop.image.name.as_deref() == Some(&name) {
                                backdrop.image.state = match read {
                                    Some(source) => ImageState::Ready(Arc::new(source)),
                                    None => ImageState::Failed,
                                };
                                cx.notify();
                            }
                        })
                        .ok();
                })
                .detach();
            }
        }
        self.image.state.clone()
    }
}

/// Copies the image at `path` into the backdrop folder beside `data`, once
/// it decodes: the copy's name, or why the image can't be used.
pub fn add_image(path: PathBuf, data: PathBuf, cx: &mut App) -> Task<Result<String, SharedString>> {
    let backdrop = Backdrop::global(cx);
    cx.spawn(async move |cx| {
        let (name, source) = (cx.background_executor())
            .spawn(async move { copy_in(&path, &data) })
            .await?;
        backdrop.update(cx, |backdrop, cx| {
            backdrop.image = Image {
                name: Some(name.clone()),
                state: ImageState::Ready(Arc::new(source)),
            };
            cx.notify();
        });
        Ok(name)
    })
}

/// Deletes an image the backdrop no longer uses, off the UI thread; one
/// already gone is no matter.
pub fn forget_image(data: &Path, name: &str, cx: &App) {
    if !workspace::Backdrop::is_image_name(name) {
        return;
    }
    let path = data.join(FOLDER).join(name);
    cx.background_executor()
        .spawn(async move { fs::remove_file(path).ok() })
        .detach();
}

fn copy_in(path: &Path, data: &Path) -> Result<(String, Source), SharedString> {
    let size = fs::metadata(path)
        .map_err(|_| SharedString::from("That file can't be opened."))?
        .len();
    if size > MAX_BYTES {
        return Err("That image is larger than 64 MB.".into());
    }
    let bytes = fs::read(path).map_err(|_| SharedString::from("That file can't be opened."))?;
    let unreadable =
        || SharedString::from("That image can't be read. Try a PNG, JPEG, WebP or GIF.");
    let format = image::guess_format(&bytes).map_err(|_| unreadable())?;
    let source = decode(&bytes).ok_or_else(unreadable)?;
    let extension = format.extensions_str().first().copied().unwrap_or("image");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let name = format!("{stamp:x}.{extension}");
    let folder = data.join(FOLDER);
    fs::create_dir_all(&folder)
        .and_then(|()| fs::write(folder.join(&name), &bytes))
        .map_err(|error| SharedString::from(format!("The image couldn't be kept: {error}.")))?;
    Ok((name, source))
}

/// The image in `bytes`, at most `LARGEST` pixels a side.
fn decode(bytes: &[u8]) -> Option<Source> {
    let image = image::load_from_memory(bytes).ok()?;
    let image = if image.width().max(image.height()) > LARGEST {
        image.thumbnail(LARGEST, LARGEST)
    } else {
        image
    };
    let rgba = image.to_rgba8();
    Source::new(rgba.width(), rgba.height(), rgba.as_raw())
}
