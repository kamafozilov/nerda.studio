//! The backdrop panel's small patterns. Each is a few hundred pixels a side,
//! drawn on the UI thread in well under a millisecond, and kept while the
//! panel asks for the same again.

use std::sync::Arc;

use backdrop::Frame;
use gpui::{Corners, IntoElement, Pixels, RenderImage, Styled, canvas};

use super::Backdrop;

/// How many small patterns stay drawn: the panel's, twice over.
const KEPT: usize = 24;

impl Backdrop {
    /// The image of `frame`, drawn now unless it was already; `None` for an
    /// empty frame.
    pub fn thumbnail(&mut self, frame: Frame) -> Option<Arc<RenderImage>> {
        if let Some((_, image)) = self.thumbnails.iter().find(|(shown, _)| *shown == frame) {
            return Some(image.clone());
        }
        let pixels = image::RgbaImage::from_raw(frame.width, frame.height, frame.render())?;
        let image = Arc::new(RenderImage::new([image::Frame::new(pixels)]));
        self.thumbnails.push((frame, image.clone()));
        if self.thumbnails.len() > KEPT {
            let (_, old) = self.thumbnails.remove(0);
            self.retired.push(old);
        }
        Some(image)
    }
}

/// `image` filling its bounds with `radius` corners, as the panel shows a
/// pattern or the owner's image.
pub fn picture(image: Arc<RenderImage>, radius: Pixels) -> impl IntoElement {
    picture_in_gray(image, radius, false)
}

/// `picture`, in gray when `grayscale`, as the panel shows an image set
/// aside.
pub fn picture_in_gray(
    image: Arc<RenderImage>,
    radius: Pixels,
    grayscale: bool,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let retired = Backdrop::global(cx).update(cx, |backdrop, _| backdrop.take_retired());
            for old in retired {
                window.drop_image(old).ok();
            }
            window
                .paint_image(bounds, bounds, Corners::all(radius), image, 0, grayscale)
                .ok();
        },
    )
    .size_full()
}
