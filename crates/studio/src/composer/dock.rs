//! A draft's composer docking as its first message goes. The draft's column
//! stands above the middle of the card; once the session has seen work it
//! docks to the card's bottom. Rather than jump there, it glides down from
//! where it stood on `COMPOSER_DOCK`: the docked layout takes over at once,
//! drawn shifted up by the distance it moved, and the shift springs to
//! nothing. The bottom edges are what is compared, so a box that shrinks as
//! its text clears keeps its foot on the path. Under Reduce motion it docks at
//! once.

use std::{cell::Cell, rc::Rc};

use gpui::{
    AnimationExt, AnyElement, Context, IntoElement, Pixels, SpringAnimation, SpringState, Styled,
    canvas, div, prelude::*, px,
};
use theme::{SPACE_3, motion};

use super::Composer;

/// Where a draft's composer last stood, read as it was laid out.
#[derive(Clone, Copy, Default)]
pub(super) struct Placed {
    /// The bottom of the column: the box, or the problem line under it.
    column: Pixels,
    /// The bottom of the composer's own area, the card's.
    area: Pixels,
}

/// The draft's places, shared with the canvases that read them.
#[derive(Clone, Default)]
pub(super) struct Placement {
    placed: Rc<Cell<Option<Placed>>>,
}

impl Placement {
    /// An empty element that notes the bottom of what it fills: the column,
    /// or the composer's area.
    pub fn reader(&self, column: bool) -> impl IntoElement + use<> {
        let placed = self.placed.clone();
        canvas(
            move |bounds, _, _| {
                let mut now = placed.get().unwrap_or_default();
                if column {
                    now.column = bounds.bottom();
                } else {
                    now.area = bounds.bottom();
                }
                placed.set(Some(now));
            },
            |_, (), _, _| {},
        )
        .absolute()
        .inset_0()
    }

    /// How far above its docked place the column stood: what the docked
    /// layout must start shifted by.
    fn rise(&self) -> Option<Pixels> {
        let placed = self.placed.get()?;
        let docked = placed.area - SPACE_3;
        Some(docked - placed.column).filter(|rise| *rise > px(0.))
    }
}

/// The glide under way: how far above its place it starts, kept until the
/// spring settles.
pub(super) struct Docking {
    rise: Pixels,
    _settle: gpui::Task<()>,
}

impl Composer {
    /// The draft is sending its first message: the composer will dock, and
    /// glides there from where it stands.
    pub(super) fn start_docking(&mut self, cx: &mut Context<Self>) {
        let Some(rise) = self.placement.rise() else {
            return;
        };
        if cx.reduce_motion() {
            return;
        }
        let settle = motion::COMPOSER_DOCK.settle_time(
            SpringState {
                position: 1.,
                velocity: 0.,
            },
            0.,
            motion::PHASE_EPSILON,
        );
        let settled = cx.spawn(async move |composer, cx| {
            cx.background_executor().timer(settle).await;
            composer
                .update(cx, |composer, cx| {
                    composer.docking = None;
                    cx.notify();
                })
                .ok();
        });
        self.docking = Some(Docking {
            rise,
            _settle: settled,
        });
    }

    /// The docked composer, shifted up by what is left of its glide.
    pub(super) fn docked(&self, placement: gpui::Div) -> AnyElement {
        let Some(docking) = &self.docking else {
            return placement.into_any_element();
        };
        let rise = docking.rise;
        let spring = SpringAnimation::new(motion::COMPOSER_DOCK)
            .to(1.)
            .with_epsilon(motion::PHASE_EPSILON)
            .from(0.);
        div()
            .w_full()
            .child(placement)
            .with_spring(
                ("composer-dock", self.session.0),
                spring,
                move |glide, phase: f32| glide.relative().top(-rise * (1. - phase.clamp(0., 1.))),
            )
            .into_any_element()
    }
}
