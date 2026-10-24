//! A draft's backdrop: the pattern behind its composer, and the button at
//! the bottom of the card that opens the backdrop's panel (`backdrop_panel`).
//!
//! The panel stays open while the owner tries choices; a click outside it,
//! Escape or the button closes it and gives focus back to the text.

use gpui::{
    App, ClickEvent, Context, FocusHandle, Focusable, IntoElement, KeyBinding, ParentElement,
    PathPromptOptions, SharedString, Styled, Subscription, Window, actions, div, prelude::*, px,
    svg,
};
use theme::{CONTROL_SIZE, HAIRLINE, ICON_SIZE_SMALL, SPACE_2, SPACE_3, TEXT_SMALL, Theme};
use workspace::{Backdrop, BackdropStyle};

use super::Composer;
use crate::{
    assets::Icon,
    backdrop::{self, ImageState, Look},
};

actions!(
    backdrop_panel,
    [
        /// Closes the backdrop's panel.
        ClosePanel,
        /// Picks the pattern after the chosen one, wrapping.
        NextPattern,
        /// Picks the pattern before the chosen one, wrapping.
        PreviousPattern,
    ]
);

pub(super) const PANEL_CONTEXT: &str = "BackdropPanel";

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", ClosePanel, Some(PANEL_CONTEXT)),
        KeyBinding::new("right", NextPattern, Some(PANEL_CONTEXT)),
        KeyBinding::new("left", PreviousPattern, Some(PANEL_CONTEXT)),
    ]);
}

/// The fills of the composer's surfaces; see `Composer::surfaces`.
pub(super) struct Surfaces {
    /// The box and the project pill, and a tray control while in use.
    pub raised: gpui::Hsla,
    /// The tray's controls at rest.
    pub quiet: gpui::Hsla,
}

/// The backdrop's panel while it is open.
pub(super) struct OpenPanel {
    /// Where focus goes when the panel closes.
    give_back: Option<FocusHandle>,
    /// An image is being read and copied in.
    pub adding: bool,
    /// Why the last image couldn't be added.
    pub problem: Option<SharedString>,
    _focus_out: Subscription,
}

impl Composer {
    /// The chosen backdrop behind a draft; none once the session has seen
    /// work, when the app lets go of its image.
    pub(super) fn render_backdrop(
        &self,
        draft: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let chosen = self.store.read(cx).workspace().backdrop().clone();
        let image = self.backdrop_image(&chosen, cx);
        let source = image.source().filter(|_| !chosen.image_hidden);
        let look = Look::new(chosen.style, &chosen, source, window, cx).filter(|_| draft);
        let Some(look) = look else {
            backdrop::Backdrop::global(cx).update(cx, |backdrop, _| backdrop.release(window));
            return None;
        };
        Some(backdrop::backdrop(look, chosen.moving, cx))
    }

    /// Whether a draft's card shows a pattern or the owner's image.
    pub(super) fn on_backdrop(&self, cx: &mut Context<Self>) -> bool {
        let workspace = self.store.read(cx).workspace();
        let draft = (workspace.session(self.session)).is_none_or(|session| session.is_empty());
        let chosen = workspace.backdrop().clone();
        draft && (chosen.style != BackdropStyle::None || self.image_in_use(&chosen, cx).is_some())
    }

    /// The owner's image, once it reads, unless it is hidden.
    pub(super) fn image_in_use(
        &self,
        chosen: &Backdrop,
        cx: &mut Context<Self>,
    ) -> Option<std::sync::Arc<::backdrop::Source>> {
        let source = self.backdrop_image(chosen, cx).source();
        source.filter(|_| !chosen.image_hidden)
    }

    /// The fills of the composer's surfaces. Over a pattern or the owner's
    /// image both are `composer_glass`, so what is under them shows faintly
    /// through; else the box and the pill are `composer_background`, the
    /// tray's quiet controls the card's own fill.
    pub(super) fn surfaces(&self, cx: &mut Context<Self>) -> Surfaces {
        let colors = Theme::of(cx).colors;
        if self.on_backdrop(cx) {
            Surfaces {
                raised: colors.composer_glass,
                quiet: colors.composer_glass,
            }
        } else {
            Surfaces {
                raised: colors.composer_background,
                quiet: colors.panel_background,
            }
        }
    }

    /// Where reading the chosen image stands.
    pub(super) fn backdrop_image(&self, chosen: &Backdrop, cx: &mut Context<Self>) -> ImageState {
        let data = self.store.read(cx).data_dir().map(|dir| dir.to_path_buf());
        backdrop::Backdrop::global(cx).update(cx, |backdrop, cx| {
            backdrop.image(data.as_deref(), chosen.image.as_deref(), cx)
        })
    }

    /// The button that opens the panel, in the card's tray: a quiet pill,
    /// its small icon and label in `text_tertiary` on
    /// the card's own fill under a faint `border` hairline, so it reads as a
    /// clearing in the pattern rather than a call to act. Under the pointer,
    /// or while the panel is open, it takes the composer's fill, its label
    /// `text_primary`. The panel opens above it. While the tray closes the
    /// pill takes no clicks.
    pub(super) fn render_backdrop_button(
        &self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let surfaces = self.surfaces(cx);
        let open = enabled && self.backdrop_panel.is_some();
        let panel = open.then(|| self.render_backdrop_panel(window, cx));
        let ink = if open {
            colors.text_primary
        } else {
            colors.text_tertiary
        };
        let pill = div()
            .id("composer-backdrop")
            .group("composer-backdrop")
            .h(CONTROL_SIZE)
            .px(SPACE_3 - HAIRLINE)
            .flex()
            .items_center()
            .gap(SPACE_2 - px(2.))
            .rounded_full()
            .border_1()
            .text_size(TEXT_SMALL)
            .text_color(ink)
            .cursor_pointer()
            .when(open, |pill| {
                pill.bg(surfaces.raised).border_color(colors.border)
            })
            .when(!open, |pill| {
                pill.bg(surfaces.quiet)
                    .border_color(colors.border)
                    .hover(|pill| {
                        pill.bg(colors.composer_background)
                            .border_color(colors.border)
                            .text_color(colors.text_primary)
                    })
            })
            .when(enabled, |pill| {
                pill.on_click(cx.listener(|composer, _: &ClickEvent, window, cx| {
                    composer.toggle_backdrop_panel(window, cx)
                }))
            })
            .child(
                svg()
                    .path(Icon::Backdrop.path())
                    .flex_none()
                    .size(ICON_SIZE_SMALL)
                    .text_color(ink)
                    .group_hover("composer-backdrop", |icon| {
                        icon.text_color(colors.text_primary)
                    }),
            )
            .child("Background");
        div().relative().child(pill).children(panel)
    }

    fn toggle_backdrop_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.backdrop_panel.is_some() {
            self.close_backdrop_panel(window, cx);
            return;
        }
        let give_back = Some(self.input.focus_handle(cx));
        // Focus taken elsewhere, as by a shortcut or a hidden tab, closes the
        // panel and stays where it went.
        let focus_out = cx.on_focus_out(&self.panel_focus, window, |composer, _, _, cx| {
            if composer.backdrop_panel.take().is_some() {
                cx.notify();
            }
        });
        self.backdrop_panel = Some(OpenPanel {
            give_back,
            adding: false,
            problem: None,
            _focus_out: focus_out,
        });
        window.focus(&self.panel_focus, cx);
        cx.notify();
    }

    pub(super) fn close_backdrop_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.backdrop_panel.take() {
            if let Some(focus) = open.give_back {
                window.focus(&focus, cx);
            }
            cx.notify();
        }
    }

    /// Changes the backdrop as `change` does, and saves it.
    pub(super) fn change_backdrop(&self, cx: &mut App, change: impl FnOnce(&mut Backdrop)) {
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                let mut backdrop = workspace.backdrop().clone();
                change(&mut backdrop);
                workspace.set_backdrop(backdrop);
            })
        })
    }

    /// Picks the pattern `step` places after the chosen one, wrapping.
    pub(super) fn step_pattern(&mut self, step: isize, cx: &mut Context<Self>) {
        let styles = BackdropStyle::ALL;
        let chosen = self.store.read(cx).workspace().backdrop().style;
        let at = styles
            .iter()
            .position(|style| *style == chosen)
            .unwrap_or(0);
        let next = styles[(at as isize + step).rem_euclid(styles.len() as isize) as usize];
        self.change_backdrop(cx, |backdrop| backdrop.style = next);
    }

    /// Opens the system's picker for one image; the picked one is copied in
    /// and replaces the image before it, whose copy is deleted.
    pub(super) fn pick_backdrop_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = self.store.read(cx).data_dir().map(|dir| dir.to_path_buf()) else {
            return;
        };
        if self.backdrop_panel.as_ref().is_none_or(|open| open.adding) {
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        self.set_adding(true, None, cx);
        cx.spawn_in(window, async move |composer, cx| {
            let path = match picked.await {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.swap_remove(0),
                _ => {
                    (composer.update(cx, |composer, cx| composer.set_adding(false, None, cx))).ok();
                    return;
                }
            };
            let Ok(added) = cx.update(|_, cx| backdrop::add_image(path, data.clone(), cx)) else {
                return;
            };
            let added = added.await;
            composer
                .update_in(cx, |composer, window, cx| match added {
                    Ok(name) => {
                        composer.replace_backdrop_image(Some(name), cx);
                        // The system's picker took focus; it comes back to the panel.
                        if composer.backdrop_panel.is_some() {
                            window.focus(&composer.panel_focus, cx);
                        }
                    }
                    Err(problem) => composer.set_adding(false, Some(problem), cx),
                })
                .ok();
        })
        .detach();
    }

    /// Puts `image` behind the pattern, or none, and deletes the copy of the
    /// image before it.
    pub(super) fn replace_backdrop_image(&mut self, image: Option<String>, cx: &mut Context<Self>) {
        let data = self.store.read(cx).data_dir().map(|dir| dir.to_path_buf());
        let before = self.store.read(cx).workspace().backdrop().image.clone();
        self.change_backdrop(cx, |backdrop| backdrop.image = image);
        if let (Some(data), Some(before)) = (data, before) {
            backdrop::forget_image(&data, &before, cx);
        }
        self.set_adding(false, None, cx);
    }

    fn set_adding(&mut self, adding: bool, problem: Option<SharedString>, cx: &mut Context<Self>) {
        if let Some(open) = self.backdrop_panel.as_mut() {
            open.adding = adding;
            open.problem = problem;
        }
        cx.notify();
    }
}
