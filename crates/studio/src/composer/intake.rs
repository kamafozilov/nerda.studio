//! Attaching without the picker: files and folders dropped on the box, and a
//! paste that holds files or an image rather than text.

use gpui::{
    App, ClipboardEntry, Context, Div, ExternalPaths, Focusable, Image, IntoElement, ParentElement,
    Stateful, Styled, Window, div, prelude::*,
};
use theme::Theme;

use super::{Composer, pasted};
use crate::text_input::keys::Paste;

/// The box, as the group its parts look to while files are dragged over it.
pub(super) const BOX_GROUP: &str = "composer-box";

impl Composer {
    /// Takes paths dropped on the box; while they hover it, its hairline turns
    /// `focus_ring`.
    pub(super) fn accept_drops(
        &self,
        target: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ring = Theme::of(cx).colors.focus_ring;
        target
            .group(BOX_GROUP)
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.border_color(ring))
            .on_drop(cx.listener(|composer, paths: &ExternalPaths, window, cx| {
                composer.attach(paths.paths().to_vec(), cx);
                window.focus(&composer.input.focus_handle(cx), cx);
            }))
    }

    /// The text, which gives way to "Drop to attach" while paths hover the box.
    /// Each part sets the box's cursor: GPUI applies a group's drag-over style
    /// only to an element with a hitbox, and a cursor of its own gives it one.
    pub(super) fn render_text(&self, cx: &App) -> impl IntoElement + use<> {
        div()
            .relative()
            .child(
                div()
                    .cursor_text()
                    .group_drag_over::<ExternalPaths>(BOX_GROUP, |text| text.invisible())
                    .child(self.input.clone()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .cursor_text()
                    .invisible()
                    .group_drag_over::<ExternalPaths>(BOX_GROUP, |hint| hint.visible())
                    .text_color(Theme::of(cx).colors.text_secondary)
                    .child("Drop to attach"),
            )
    }

    /// Ahead of the text's own paste: files on the clipboard (a copy in the
    /// Finder) attach, and so does an image with no text, once saved as a
    /// file. Anything else pastes as text.
    pub(super) fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let mut image = None;
        let mut text = false;
        for entry in item.into_entries() {
            match entry {
                ClipboardEntry::ExternalPaths(paths) => {
                    cx.stop_propagation();
                    self.attach(paths.paths().to_vec(), cx);
                    return;
                }
                ClipboardEntry::Image(pasted) => image = Some(pasted),
                ClipboardEntry::String(_) => text = true,
            }
        }
        if let Some(image) = image.filter(|_| !text) {
            cx.stop_propagation();
            self.attach_pasted(image, cx);
        }
    }

    fn attach_pasted(&mut self, image: Image, cx: &mut Context<Self>) {
        let Some(folder) = workspace::default_data_dir().map(|data| data.join("attachments"))
        else {
            self.problem = Some("Couldn't find a folder to save the pasted image in.".into());
            cx.notify();
            return;
        };
        let saved = cx.background_spawn(async move { pasted::save(&image, &folder) });
        cx.spawn(async move |composer, cx| {
            let saved = saved.await;
            composer
                .update(cx, |composer, cx| match saved {
                    Ok(path) => composer.attach(vec![path], cx),
                    Err(problem) => {
                        composer.problem = Some(problem.into());
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
    }
}
