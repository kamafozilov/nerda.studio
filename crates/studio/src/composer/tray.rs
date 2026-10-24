//! A draft card's tray: a small chevron centered 16 pt above the card's
//! bottom edge, and the options it raises over it, for now the backdrop's
//! button. Closed, only the chevron shows, pointing up; a click raises the
//! options, which rise `TRAY_RISE` and fade in on the panel spring while the
//! chevron turns to point down, and a second click lowers them again.

use std::f32::consts::PI;

use gpui::{
    AnimationExt, ClickEvent, Context, IntoElement, ParentElement, Pixels, SpringAnimation,
    SpringState, Styled, Transformation, Window, div, prelude::*, px, radians, svg,
};
use theme::{CONTROL_SIZE, ICON_SIZE_SMALL, SPACE_1, SPACE_2, SPACE_3, Theme, motion};

use super::Composer;
use crate::{assets::Icon, tooltip::TooltipExt};

/// How far the options rise as they come in.
const TRAY_RISE: Pixels = px(8.);

impl Composer {
    pub(super) fn render_tray(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let surfaces = self.surfaces(cx);
        let open = self.tray_open;
        // 1 open, 0 closed; the options and the chevron move on one spring.
        let spring = |open: bool| {
            let (target, config) = if open {
                (1., motion::PANEL_ENTER)
            } else {
                (0., motion::PANEL_EXIT)
            };
            SpringAnimation::new(config)
                .to(target)
                .with_epsilon(motion::PHASE_EPSILON)
        };
        let options = (open || self.tray_closing.is_some()).then(|| {
            div()
                .flex()
                .items_center()
                .gap(SPACE_2)
                .child(self.render_backdrop_button(open, window, cx))
                .with_spring("composer-tray", spring(open).from(0.), |row, phase: f32| {
                    let phase = phase.clamp(0., 1.);
                    row.opacity(phase).relative().top(TRAY_RISE * (1. - phase))
                })
        });
        let toggle =
            div()
                .id("composer-tray-toggle")
                .group("composer-tray-toggle")
                .size(CONTROL_SIZE)
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(colors.border)
                .cursor_pointer()
                .when(open, |toggle| toggle.bg(surfaces.raised))
                .when(!open, |toggle| {
                    toggle
                        .bg(surfaces.quiet)
                        .hover(|toggle| toggle.bg(colors.composer_background))
                })
                .on_click(cx.listener(|composer, _: &ClickEvent, window, cx| {
                    composer.toggle_tray(window, cx)
                }))
                .child(
                    svg()
                        .path(Icon::ChevronDown.path())
                        .size(ICON_SIZE_SMALL)
                        .text_color(if open {
                            colors.text_primary
                        } else {
                            colors.text_tertiary
                        })
                        .group_hover("composer-tray-toggle", |icon| {
                            icon.text_color(colors.text_primary)
                        })
                        // Down while open; turned half round, up, while closed.
                        .with_spring("composer-tray-chevron", spring(open), |icon, phase: f32| {
                            icon.with_transformation(Transformation::rotate(radians(
                                PI * (1. - phase.clamp(0., 1.)),
                            )))
                        }),
                );
        // Only the closed chevron says what it does; open, its options do.
        let toggle = if open {
            toggle.into_any_element()
        } else {
            toggle.with_tooltip("More options").into_any_element()
        };
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(SPACE_3 + SPACE_1)
            .flex()
            .flex_col()
            .items_center()
            .gap(SPACE_2)
            .children(options)
            .child(toggle)
    }

    /// Raises the options, or lowers them, closing the backdrop's panel; while
    /// they go they stay drawn, taking no clicks, until the spring settles.
    fn toggle_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tray_open = !self.tray_open;
        if self.tray_open {
            self.tray_closing = None;
        } else {
            self.close_backdrop_panel(window, cx);
            let settle = motion::PANEL_EXIT.settle_time(
                SpringState {
                    position: 1.,
                    velocity: 0.,
                },
                0.,
                motion::PHASE_EPSILON,
            );
            self.tray_closing = (!cx.reduce_motion()).then(|| {
                cx.spawn(async move |composer, cx| {
                    cx.background_executor().timer(settle).await;
                    composer
                        .update(cx, |composer, cx| {
                            composer.tray_closing = None;
                            cx.notify();
                        })
                        .ok();
                })
            });
        }
        cx.notify();
    }
}
