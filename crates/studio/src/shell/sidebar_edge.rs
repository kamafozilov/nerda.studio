//! The open sidebar's edge, where it meets the card: dragging it resizes the
//! sidebar between `SIDEBAR_MIN_WIDTH` and `SIDEBAR_MAX_WIDTH`, and a
//! double-click puts it back to `SIDEBAR_WIDTH`. The width is saved with the
//! workspace once the drag ends, so every launch opens at the width left.

use gpui::{
    Context, CursorStyle, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Styled, div, prelude::*, px,
};
use theme::{SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_RESIZE_TARGET, SIDEBAR_WIDTH};

use super::Shell;
use crate::store::Store;

/// A drag of the edge under way: where it started, and the width then.
#[derive(Clone, Copy)]
pub(super) struct Resize {
    from_x: Pixels,
    from_width: Pixels,
}

/// `width` within the sidebar's bounds, on whole points so its text and
/// its edge stay crisp.
fn bounded(width: Pixels) -> Pixels {
    px(f32::from(width).round()).clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

/// The width the workspace saved, within bounds; the default when it saved
/// none, or one that is no number.
pub(super) fn saved_width(store: &Store) -> Pixels {
    store
        .workspace()
        .sidebar_width()
        .filter(|width| width.is_finite())
        .map_or(SIDEBAR_WIDTH, |width| bounded(px(width)))
}

impl Shell {
    /// The strip over the edge that drags it. Drawn after the sidebar and the
    /// card, it occludes them, so a press on it never reaches the card.
    pub(super) fn sidebar_edge(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sidebar-edge")
            .absolute()
            .top_0()
            .bottom_0()
            .left(self.sidebar_width - SIDEBAR_RESIZE_TARGET / 2.)
            .w(SIDEBAR_RESIZE_TARGET)
            .occlude()
            .cursor(CursorStyle::ResizeLeftRight)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if event.click_count >= 2 {
                        shell.resizing = None;
                        shell.sidebar_width = SIDEBAR_WIDTH;
                        shell.save_sidebar_width(cx);
                    } else {
                        shell.resizing = Some(Resize {
                            from_x: event.position.x,
                            from_width: shell.sidebar_width,
                        });
                    }
                    cx.notify();
                }),
            )
    }

    /// Over the whole window while the edge is dragged: the pointer keeps the
    /// resize cursor wherever it goes, nothing under it lights up, and the
    /// button let go anywhere, outside the window too, ends the drag.
    pub(super) fn resize_cover(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.resizing.is_some().then(|| {
            div()
                .id("sidebar-resize")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .cursor(CursorStyle::ResizeLeftRight)
                .on_mouse_move(cx.listener(|shell, event: &MouseMoveEvent, _, cx| {
                    let Some(resize) = shell.resizing else {
                        return;
                    };
                    // The button went up where the window couldn't see it.
                    if event.pressed_button != Some(MouseButton::Left) {
                        return shell.end_resize(cx);
                    }
                    let width = bounded(resize.from_width + event.position.x - resize.from_x);
                    if width != shell.sidebar_width {
                        shell.sidebar_width = width;
                        cx.notify();
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|shell, _: &MouseUpEvent, _, cx| shell.end_resize(cx)),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|shell, _: &MouseUpEvent, _, cx| shell.end_resize(cx)),
                )
        })
    }

    fn end_resize(&mut self, cx: &mut Context<Self>) {
        if self.resizing.take().is_some() {
            self.save_sidebar_width(cx);
            cx.notify();
        }
    }

    /// The default is saved as none, so a later default reaches it.
    fn save_sidebar_width(&self, cx: &mut Context<Self>) {
        let width = (self.sidebar_width != SIDEBAR_WIDTH).then(|| f32::from(self.sidebar_width));
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| workspace.set_sidebar_width(width))
        });
    }
}
