//! The pattern behind a new session's composer, as the card shows it.
//!
//! The pattern is drawn off the UI thread into an image of the card's pixels
//! and drawn again when the card's size, the composer's place, the theme or
//! the choice changes. The last image stays on screen, stretched, until the
//! next is ready, so a resize never flashes the card bare. While the pattern
//! moves, it is drawn `FRAMES_PER_SECOND` times a second, and only while the
//! card shows it: a frame nobody paints asks for no next one.
//!
//! The owner's image, if any, is kept in the app's folder (`source`) and the
//! backdrop's panel shows each pattern small (`thumbnails`).

mod source;
mod thumbnails;

pub use source::{ImageState, add_image, forget_image};
pub use thumbnails::picture;

use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use backdrop::{Frame, Ink, Pattern, Rect, Source};
use gpui::{
    App, AppContext, Bounds, Context, Corners, Entity, Global, Hsla, IntoElement, Pixels,
    RenderImage, Styled, Task, Window, WindowAppearance, canvas, px,
};
use theme::{HAIRLINE, MAIN_RADIUS, Theme};
use workspace::{BackdropStrength, BackdropStyle, BackdropTint};

/// How often a moving pattern is drawn: its drift is slow, and dithered dots
/// step rather than glide, so a few frames a second read as smooth.
const FRAMES_PER_SECOND: f32 = 12.;

/// The alpha of the image's own colors at medium strength.
const IMAGE_ALPHA: f32 = 0.45;

/// The one backdrop of the app, which every draft's composer draws.
pub struct Backdrop {
    /// The image on screen, and the frame it shows.
    shown: Option<(Frame, Arc<RenderImage>)>,
    /// The frame being drawn now, if one is.
    drawing: Option<Frame>,
    /// The frame asked for while another was drawing, drawn next.
    next: Option<Frame>,
    /// Images replaced since the last paint, to free from the window's atlas.
    retired: Vec<Arc<RenderImage>>,
    /// Where the drift stands while the pattern holds still.
    still_at: f32,
    /// Since when the pattern has moved, and where its drift stood then.
    moving_since: Option<(Instant, f32)>,
    /// The timer that asks for a moving pattern's next frame.
    tick: Option<Task<()>>,
    /// The owner's image, decoded.
    image: source::Image,
    /// The panel's small patterns, by the frame each shows.
    thumbnails: Vec<(Frame, Arc<RenderImage>)>,
}

struct GlobalBackdrop(Entity<Backdrop>);

impl Global for GlobalBackdrop {}

pub fn init(cx: &mut App) {
    let backdrop = cx.new(|_| Backdrop {
        shown: None,
        drawing: None,
        next: None,
        retired: Vec::new(),
        still_at: 0.,
        moving_since: None,
        tick: None,
        image: source::Image::default(),
        thumbnails: Vec::new(),
    });
    cx.set_global(GlobalBackdrop(backdrop));
}

/// Everything a frame shows but its size, its place and its time.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub pattern: Pattern,
    pub ink: Ink,
    pub source: Option<Arc<Source>>,
    pub light: bool,
}

impl Look {
    /// How `style` looks as chosen: `None` for no pattern and no image.
    /// The image's colors stand in gray until the image is ready.
    pub fn new(
        style: BackdropStyle,
        choice: &workspace::Backdrop,
        source: Option<Arc<Source>>,
        window: &Window,
        cx: &App,
    ) -> Option<Self> {
        let pattern = match (style, &source) {
            (BackdropStyle::None, None) => return None,
            (BackdropStyle::None, Some(_)) => Pattern::Image,
            (BackdropStyle::Horizon, _) => Pattern::Horizon,
            (BackdropStyle::Clouds, _) => Pattern::Clouds,
            (BackdropStyle::Contours, _) => Pattern::Contours,
            (BackdropStyle::Halftone, _) => Pattern::Halftone,
            (BackdropStyle::Characters, _) => Pattern::Characters,
        };
        let colors = Theme::of(cx).colors;
        let gain = match choice.strength {
            BackdropStrength::Subtle => 0.6,
            BackdropStrength::Medium => 1.,
            BackdropStrength::Bold => 1.7,
        };
        let ink = match (choice.tint_in_use(), &source) {
            (BackdropTint::Image, Some(_)) => Ink::Source {
                alpha: ((IMAGE_ALPHA * gain).min(1.) * 255.).round() as u8,
            },
            (BackdropTint::Warm, _) => Ink::Solid(rgba_bytes(colors.backdrop_warm, gain)),
            (BackdropTint::Gray | BackdropTint::Image, _) => {
                Ink::Solid(rgba_bytes(colors.backdrop, gain))
            }
        };
        let light = matches!(
            window.appearance(),
            WindowAppearance::Light | WindowAppearance::VibrantLight
        );
        Some(Self {
            pattern,
            ink,
            source,
            light,
        })
    }

    /// The frame of this look over `size` device pixels.
    pub fn frame(
        &self,
        width: u32,
        height: u32,
        scale: f32,
        clear: Option<Rect>,
        time: f32,
    ) -> Frame {
        Frame {
            width,
            height,
            scale,
            pattern: self.pattern,
            ink: self.ink,
            source: self.source.clone(),
            light: self.light,
            clear,
            // The image alone holds still, so it is never drawn again for time.
            time: if self.pattern == Pattern::Image {
                0.
            } else {
                time
            },
        }
    }
}

impl Backdrop {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalBackdrop>().0.clone()
    }

    /// Where the pattern's drift stands now: still, or on the last frame tick
    /// while it moves, so frames between ticks ask for nothing new.
    fn time(&mut self, moving: bool) -> f32 {
        match (moving, self.moving_since) {
            (true, None) => {
                self.moving_since = Some((Instant::now(), self.still_at));
                self.still_at
            }
            (true, Some((since, from))) => {
                let ticks = (since.elapsed().as_secs_f32() * FRAMES_PER_SECOND).floor();
                from + ticks / FRAMES_PER_SECOND
            }
            (false, Some(_)) => {
                self.still_at = self.time(true);
                self.moving_since = None;
                self.still_at
            }
            (false, None) => self.still_at,
        }
    }

    /// Where the drift stands while the pattern holds still, for the
    /// panel's patterns.
    pub fn still_time(&self) -> f32 {
        self.still_at
    }

    /// The image to paint for `frame`, and the images to free: the one for
    /// `frame` when it is ready, else the last one while `frame` is drawn.
    fn prepare(
        &mut self,
        frame: Frame,
        moving: bool,
        cx: &mut Context<Self>,
    ) -> (Option<Arc<RenderImage>>, Vec<Arc<RenderImage>>) {
        let current = self
            .shown
            .as_ref()
            .is_some_and(|(shown, _)| *shown == frame);
        if !current {
            self.draw(frame, cx);
        }
        if moving && self.tick.is_none() {
            let wait = Duration::from_secs_f32(1. / FRAMES_PER_SECOND);
            self.tick = Some(cx.spawn(async move |backdrop, cx| {
                cx.background_executor().timer(wait).await;
                backdrop
                    .update(cx, |backdrop, cx| {
                        backdrop.tick = None;
                        cx.notify();
                    })
                    .ok();
            }));
        }
        let image = self.shown.as_ref().map(|(_, image)| image.clone());
        (image, self.take_retired())
    }

    /// Draws `frame` off the UI thread, or after the frame being drawn.
    fn draw(&mut self, frame: Frame, cx: &mut Context<Self>) {
        if self.drawing.is_some() {
            self.next = (self.drawing.as_ref() != Some(&frame)).then_some(frame);
            return;
        }
        self.drawing = Some(frame.clone());
        cx.spawn(async move |backdrop, cx| {
            let job = frame.clone();
            let pixels = (cx.background_executor())
                .spawn(async move { job.render() })
                .await;
            let image = image::RgbaImage::from_raw(frame.width, frame.height, pixels)
                .map(|pixels| Arc::new(RenderImage::new([image::Frame::new(pixels)])));
            backdrop
                .update(cx, |backdrop, cx| {
                    backdrop.drawing = None;
                    if let Some(image) = image
                        && let Some((_, old)) = backdrop.shown.replace((frame, image))
                    {
                        backdrop.retired.push(old);
                    }
                    if let Some(next) = backdrop.next.take() {
                        backdrop.draw(next, cx);
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    /// The images replaced since the last paint, for the window to free.
    fn take_retired(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.retired)
    }

    /// Lets go of the card's images once it shows no pattern, so a large
    /// one doesn't stay in memory or in the window's atlas.
    pub fn release(&mut self, window: &mut Window) {
        self.next = None;
        self.tick = None;
        let shown = self.shown.take().map(|(_, image)| image);
        for image in self.retired.drain(..).chain(shown) {
            window.drop_image(image).ok();
        }
    }
}

/// The pattern behind a draft's composer, filling the card, clear of the
/// area `clear` holds when the card paints: the composer's column, measured
/// as it lays out.
pub fn backdrop(
    look: Look,
    moving: bool,
    clear: Rc<Cell<Option<Bounds<Pixels>>>>,
    cx: &App,
) -> impl IntoElement + use<> {
    let backdrop = Backdrop::global(cx);
    let moving = moving && !cx.reduce_motion();
    canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let scale = window.scale_factor();
            let (frame, image, retired) = backdrop.update(cx, |backdrop, cx| {
                let frame = look.frame(
                    (f32::from(bounds.size.width) * scale).round() as u32,
                    (f32::from(bounds.size.height) * scale).round() as u32,
                    scale,
                    clear.get().map(|area| device_rect(area, bounds, scale)),
                    backdrop.time(moving),
                );
                let (image, retired) = backdrop.prepare(frame.clone(), moving, cx);
                (frame, image, retired)
            });
            for image in retired {
                window.drop_image(image).ok();
            }
            if frame.width == 0 || frame.height == 0 {
                return;
            }
            if let Some(image) = image {
                let corners = Corners::all((MAIN_RADIUS - HAIRLINE).max(px(0.)));
                window
                    .paint_image(bounds, bounds, corners, image, 0, false)
                    .ok();
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// `color` as straight RGBA bytes, its alpha scaled by `gain`.
fn rgba_bytes(color: Hsla, gain: f32) -> [u8; 4] {
    let rgba = color.to_rgb();
    let alpha = (rgba.a * gain).min(1.);
    [rgba.r, rgba.g, rgba.b, alpha].map(|channel| (channel * 255.).round() as u8)
}

/// `area` in the device pixels of `bounds`, from its top-left corner.
fn device_rect(area: Bounds<Pixels>, bounds: Bounds<Pixels>, scale: f32) -> Rect {
    let offset = area.origin - bounds.origin;
    Rect {
        x: f32::from(offset.x) * scale,
        y: f32::from(offset.y) * scale,
        width: f32::from(area.size.width) * scale,
        height: f32::from(area.size.height) * scale,
    }
}
