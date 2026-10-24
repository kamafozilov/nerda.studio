//! The backdrop's panel, over the button at the card's bottom: the patterns
//! as cards that show each one as the card would, the owner's image, the
//! ink, the strength, and whether the pattern moves.

use std::sync::Arc;

use gpui::{
    Anchor, AnyElement, ClickEvent, Context, FontWeight, IntoElement, MouseButton, ParentElement,
    Pixels, SharedString, Styled, Window, anchored, deferred, div, point, prelude::*, px, relative,
    svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, DIALOG_RADIUS, DISABLED_OPACITY, HAIRLINE, HEADER_HEIGHT,
    ICON_SIZE, ICON_SIZE_SMALL, MAIN_INSET, SPACE_1, SPACE_2, SPACE_3, TEXT_SMALL, Theme,
};
use workspace::{Backdrop, BackdropStrength, BackdropStyle, BackdropTint};

use super::{
    Composer,
    backdrop::{ClosePanel, NextPattern, PANEL_CONTEXT, PreviousPattern},
    backdrop_cards::{card_face, image_picture, pattern_card},
    backdrop_controls::{choice_chip, section, switch, text_button, two_lines},
};
use crate::{
    assets::Icon,
    backdrop::{self, ImageState, Look},
    tooltip::TooltipExt,
};

/// How much of the owner's image shows while it is set aside.
const HIDDEN_IMAGE_OPACITY: f32 = 0.5;
/// The panel's width: three pattern cards and their gaps.
const PANEL_WIDTH: Pixels = px(392.);
/// A pattern card's ring, outside its picture.
const RING: Pixels = px(2.);
/// The picture of the owner's image.
const IMAGE_SIZE: (Pixels, Pixels) = (px(84.), px(52.));

impl Composer {
    /// The panel, hung above the button, and under it a window-wide layer
    /// whose click closes it.
    pub(super) fn render_backdrop_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = Theme::of(cx).colors;
        let chosen = self.store.read(cx).workspace().backdrop().clone();
        let image = self.backdrop_image(&chosen, cx);
        let viewport = window.viewport_size();
        // Under the top row and over the button: a short window scrolls the
        // panel rather than cutting it off.
        // The card's inset and gap, the tray's chevron and button, each with
        // the gap over it.
        let button_room = MAIN_INSET + SPACE_3 + SPACE_1 + (CONTROL_SIZE + SPACE_2) * 2.;
        let room = viewport.height - HEADER_HEIGHT - button_room - SPACE_2;
        let panel = div()
            .id("backdrop-panel")
            .occlude()
            .max_h(room.max(px(160.)))
            .overflow_y_scroll()
            .key_context(PANEL_CONTEXT)
            .track_focus(&self.panel_focus)
            .on_action(cx.listener(|composer, _: &ClosePanel, window, cx| {
                composer.close_backdrop_panel(window, cx)
            }))
            .on_action(cx.listener(|composer, _: &NextPattern, _, cx| composer.step_pattern(1, cx)))
            .on_action(
                cx.listener(|composer, _: &PreviousPattern, _, cx| composer.step_pattern(-1, cx)),
            )
            .w(PANEL_WIDTH)
            .p(SPACE_3)
            .flex()
            .flex_col()
            .gap(SPACE_3)
            .rounded(DIALOG_RADIUS)
            .border_1()
            .border_color(colors.popover_border)
            .bg(colors.popover_background)
            .shadow(colors.popover_shadows())
            .cursor_default()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors.text_primary)
                    .child("Background"),
            )
            .child(section(
                "Pattern",
                self.render_patterns(&chosen, &image, window, cx),
                cx,
            ))
            .child(section(
                "Image",
                self.render_image_row(&chosen, &image, window, cx),
                cx,
            ))
            .child(section(
                "Color",
                self.render_tints(&chosen, &image, window, cx),
                cx,
            ))
            .child(section("Strength", self.render_strengths(&chosen, cx), cx))
            .child(self.render_motion(&chosen, cx));
        let catcher = div()
            .id("backdrop-panel-catcher")
            .w(viewport.width)
            .h(viewport.height)
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|composer, _, window, cx| composer.close_backdrop_panel(window, cx)),
            );
        div()
            .absolute()
            .top_0()
            .left(relative(0.5))
            .child(deferred(anchored().position(point(px(0.), px(0.))).child(catcher)).priority(1))
            .child(
                deferred(
                    anchored()
                        .anchor(Anchor::BottomCenter)
                        .offset(point(px(0.), -SPACE_2))
                        .snap_to_window_with_margin(SPACE_2)
                        .child(panel),
                )
                .priority(2),
            )
            .into_any_element()
    }

    /// The patterns as cards, three to a row, each showing the pattern as
    /// the card would with the image and the ink chosen.
    fn render_patterns(
        &self,
        chosen: &Backdrop,
        image: &ImageState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        // What a grid cell holds, for drawing the card's pattern; the grid
        // itself splits the row, so the cards fill it exactly.
        let width = ((PANEL_WIDTH - SPACE_3 * 2. - HAIRLINE * 2. - SPACE_2 * 2.) / 3.).floor();
        let in_use = image.source().filter(|_| !chosen.image_hidden);
        let cards = BackdropStyle::ALL.map(|style| {
            let selected = chosen.style == style;
            let look = Look::new(style, chosen, in_use.clone(), window, cx);
            let picture = look.and_then(|look| pattern_card(look, width - RING * 4., window, cx));
            let label = match (style, &in_use) {
                (BackdropStyle::None, Some(_)) => "Image only",
                _ => style.title(),
            };
            let id = SharedString::from(format!("backdrop-style-{}", style.title()));
            div()
                .id(id.clone())
                .group(id.clone())
                .min_w_0()
                .flex()
                .flex_col()
                .gap(SPACE_1)
                .cursor_pointer()
                .on_click(cx.listener(move |composer, _: &ClickEvent, _, cx| {
                    composer.change_backdrop(cx, |backdrop| backdrop.style = style)
                }))
                .child(
                    div()
                        .p(RING)
                        .rounded(CONTROL_RADIUS + RING * 2.)
                        .border(RING)
                        .border_color(if selected {
                            colors.focus_ring
                        } else {
                            gpui::transparent_black()
                        })
                        .when(!selected, |ring| {
                            ring.group_hover(id.clone(), |ring| ring.border_color(colors.border))
                        })
                        .child(card_face(
                            picture,
                            colors.panel_background,
                            colors.border,
                            cx,
                        )),
                )
                .child(
                    div()
                        .text_size(TEXT_SMALL)
                        .text_center()
                        .text_color(if selected {
                            colors.text_primary
                        } else {
                            colors.text_secondary
                        })
                        .child(label),
                )
        });
        div().grid().grid_cols(3).gap(SPACE_2).children(cards)
    }

    /// The owner's image: a box to add one, or the image with Replace and
    /// Remove, and why the last one couldn't be added.
    fn render_image_row(
        &self,
        chosen: &Backdrop,
        image: &ImageState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let open = self.backdrop_panel.as_ref();
        let adding = open.is_some_and(|open| open.adding);
        let problem = open.and_then(|open| open.problem.clone());
        let can_keep = self.store.read(cx).data_dir().is_some();
        let row = if chosen.image.is_none() {
            let (title, detail) = if adding {
                ("Adding the image…", "Reading it and keeping a copy")
            } else {
                (
                    "Add an image",
                    "PNG, JPEG, WebP or GIF; the pattern draws it",
                )
            };
            div()
                .id("backdrop-add-image")
                .group("backdrop-add-image")
                .h(IMAGE_SIZE.1 + SPACE_2)
                .px(SPACE_3)
                .flex()
                .items_center()
                .gap(SPACE_3)
                .rounded(CONTROL_RADIUS)
                .border_1()
                .border_dashed()
                .border_color(colors.popover_border)
                .when(can_keep && !adding, |button| {
                    button
                        .cursor_pointer()
                        .hover(|button| button.bg(colors.hover))
                        .on_click(cx.listener(|composer, _: &ClickEvent, window, cx| {
                            composer.pick_backdrop_image(window, cx)
                        }))
                })
                .when(!can_keep, |button| button.opacity(DISABLED_OPACITY))
                .child(
                    svg()
                        .path(Icon::Image.path())
                        .flex_none()
                        .size(ICON_SIZE)
                        .text_color(colors.text_secondary),
                )
                .child(two_lines(title, detail, cx))
                .into_any_element()
        } else {
            let picture =
                (image.source()).and_then(|source| image_picture(source, IMAGE_SIZE, window, cx));
            let status = match image {
                ImageState::Failed => "It no longer reads",
                ImageState::Loading => "Reading it…",
                _ if adding => "Adding the new image…",
                _ if chosen.image_hidden => "Hidden for now",
                _ => "The pattern draws it",
            };
            let hidden = chosen.image_hidden;
            div()
                .flex()
                .items_center()
                .gap(SPACE_3)
                .child(self.render_image_switch(picture, hidden, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(two_lines("Your image", status, cx)),
                )
                .child(
                    text_button("backdrop-replace", "Replace", !adding, cx).on_click(cx.listener(
                        |composer, _: &ClickEvent, window, cx| {
                            composer.pick_backdrop_image(window, cx)
                        },
                    )),
                )
                .child(
                    text_button("backdrop-remove", "Remove", !adding, cx).on_click(cx.listener(
                        |composer, _: &ClickEvent, _, cx| composer.replace_backdrop_image(None, cx),
                    )),
                )
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap(SPACE_2)
            .child(row)
            .children(problem.map(|problem| {
                div()
                    .text_size(TEXT_SMALL)
                    .text_color(colors.text_primary)
                    .child(problem)
            }))
    }

    /// The image, small, as the switch that sets it aside. Shown, it
    /// takes a struck-through eye under the pointer, and a click hides it;
    /// hidden, it stays in gray and dimmed with an open eye on it, and a
    /// click shows it again.
    fn render_image_switch(
        &self,
        picture: Option<Arc<gpui::RenderImage>>,
        hidden: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let group = "backdrop-image-switch";
        let badge = div()
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(colors.popover_border)
            .bg(colors.popover_background)
            .shadow(colors.popover_shadows())
            .when(!hidden, |badge| {
                badge
                    .opacity(0.)
                    .group_hover(group, |badge| badge.opacity(1.))
            })
            .child(
                svg()
                    .path(if hidden { Icon::View } else { Icon::ViewOff }.path())
                    .size(ICON_SIZE_SMALL)
                    .text_color(colors.text_primary),
            );
        div()
            .id(group)
            .group(group)
            .relative()
            .flex_none()
            .w(IMAGE_SIZE.0)
            .h(IMAGE_SIZE.1)
            .rounded(CONTROL_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .overflow_hidden()
            .cursor_pointer()
            .on_click(cx.listener(move |composer, _: &ClickEvent, _, cx| {
                composer.change_backdrop(cx, |backdrop| backdrop.image_hidden = !hidden)
            }))
            .child(
                div()
                    .size_full()
                    .when(hidden, |picture| picture.opacity(HIDDEN_IMAGE_OPACITY))
                    .children(
                        picture.map(|picture| {
                            backdrop::picture_in_gray(picture, CONTROL_RADIUS, hidden)
                        }),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(badge),
            )
            .with_tooltip(if hidden { "Show image" } else { "Hide image" })
    }

    /// The inks as chips, each with a dot of its color; the image's colors
    /// wait for an image.
    fn render_tints(
        &self,
        chosen: &Backdrop,
        image: &ImageState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let source = image.source().filter(|_| !chosen.image_hidden);
        // The image's colors show only once the image reads.
        let in_use = match chosen.tint_in_use() {
            BackdropTint::Image if source.is_none() => BackdropTint::Gray,
            tint => tint,
        };
        let chips = BackdropTint::ALL.map(|tint| {
            let enabled = tint != BackdropTint::Image || source.is_some();
            let dot = div()
                .flex_none()
                .size(px(14.))
                .rounded_full()
                .overflow_hidden();
            let dot = match tint {
                BackdropTint::Gray => dot.bg(colors.text_secondary),
                BackdropTint::Warm => dot.bg(colors.claude),
                BackdropTint::Image => {
                    let picture = (source.clone())
                        .and_then(|source| image_picture(source, (px(14.), px(14.)), window, cx));
                    dot.bg(colors.key_background)
                        .children(picture.map(|picture| backdrop::picture(picture, px(7.))))
                }
            };
            let chip = choice_chip(
                SharedString::from(format!("backdrop-tint-{}", tint.title())),
                in_use == tint,
                enabled,
                cx,
            )
            .child(dot)
            .child(tint.title())
            .when(enabled, |chip| {
                chip.on_click(cx.listener(move |composer, _: &ClickEvent, _, cx| {
                    composer.change_backdrop(cx, |backdrop| backdrop.tint = tint)
                }))
            });
            if enabled {
                chip.into_any_element()
            } else {
                chip.with_tooltip("Add an image first").into_any_element()
            }
        });
        div().flex().gap(SPACE_2).children(chips)
    }

    /// The strengths as one segmented control.
    fn render_strengths(
        &self,
        chosen: &Backdrop,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let segments = BackdropStrength::ALL.map(|strength| {
            let selected = chosen.strength == strength;
            div()
                .id(SharedString::from(format!(
                    "backdrop-strength-{}",
                    strength.title()
                )))
                .flex_1()
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(CONTROL_RADIUS - px(2.))
                .border_1()
                .border_color(if selected {
                    colors.border
                } else {
                    gpui::transparent_black()
                })
                .when(selected, |segment| {
                    segment
                        .bg(colors.composer_background)
                        .text_color(colors.text_primary)
                })
                .when(!selected, |segment| {
                    segment
                        .cursor_pointer()
                        .text_color(colors.text_secondary)
                        .hover(|segment| segment.text_color(colors.text_primary))
                })
                .on_click(cx.listener(move |composer, _: &ClickEvent, _, cx| {
                    composer.change_backdrop(cx, |backdrop| backdrop.strength = strength)
                }))
                .child(strength.title())
        });
        div()
            .p(px(2.))
            .flex()
            .gap(px(2.))
            .rounded(CONTROL_RADIUS)
            .bg(colors.field_background)
            .children(segments)
    }

    /// Move slowly, with what it does, and a switch.
    fn render_motion(&self, chosen: &Backdrop, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let on = chosen.moving;
        let detail = if cx.reduce_motion() {
            "Holds still while the system reduces motion"
        } else {
            "The pattern drifts, a few frames a second"
        };
        div()
            .id("backdrop-moving")
            .pt(SPACE_3)
            .border_t_1()
            .border_color(colors.border)
            .flex()
            .items_center()
            .gap(SPACE_3)
            .cursor_pointer()
            .on_click(cx.listener(move |composer, _: &ClickEvent, _, cx| {
                composer.change_backdrop(cx, |backdrop| backdrop.moving = !on)
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(two_lines("Move slowly", detail, cx)),
            )
            .child(switch(on, cx))
    }
}
