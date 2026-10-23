//! Cards that move when the list changes under them, as when a new session
//! lands on top or the sort changes, glide from where they were to where they
//! now are, and a card that arrives fades in once the cards around it have
//! mostly made room. A search or a project filter replaces the list instead:
//! nothing glides then.
//!
//! The offsets are worked out from the spring on each frame rather than kept
//! in GPUI's per-element spring state, which a virtualized row loses when it
//! scrolls out of sight: a card scrolled into view mid-glide shows where it is,
//! and one scrolled into view afterwards doesn't glide again. A change that
//! lands mid-glide starts each card from where it is drawn.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui::SpringState;
use theme::motion::{PHASE_EPSILON, PIXEL_EPSILON, ROW_SHIFT};
use workspace::SessionId;

/// Farthest a card glides, in slots: one coming from farther, as in a long
/// list re-sorted, glides in from just past the view rather than racing across.
const MAX_TRAVEL_SLOTS: f32 = 12.;

#[derive(Default)]
pub(super) struct Shifts {
    started: Option<Instant>,
    /// Each moving card's offset from its slot when the glide started.
    from: HashMap<SessionId, f32>,
    /// Cards new to the list, fading in, and the opacity each started from: a
    /// card still fading in when the list changes again goes on from where it
    /// is.
    arriving: HashMap<SessionId, f32>,
}

impl Shifts {
    /// Starts the cards gliding from their slots in `before` to those in
    /// `after`, each `slot` tall.
    pub fn start(&mut self, before: &[SessionId], after: &[SessionId], slot: f32) {
        let now = Instant::now();
        let was: HashMap<SessionId, usize> = before
            .iter()
            .enumerate()
            .map(|(index, &id)| (id, index))
            .collect();
        let mut from = HashMap::new();
        let mut arriving = HashMap::new();
        let travel = MAX_TRAVEL_SLOTS * slot;
        for (index, &id) in after.iter().enumerate() {
            match was.get(&id) {
                Some(&old) => {
                    let offset = (old as f32 - index as f32) * slot + self.offset(id, now);
                    if offset.abs() > PIXEL_EPSILON {
                        from.insert(id, offset.clamp(-travel, travel));
                    }
                    let opacity = self.opacity(id, now);
                    if opacity < 1. {
                        arriving.insert(id, opacity);
                    }
                }
                None => {
                    arriving.insert(id, 0.);
                }
            }
        }
        self.started = Some(now);
        self.from = from;
        self.arriving = arriving;
    }

    pub fn stop(&mut self) {
        *self = Self::default();
    }

    /// Whether anything is still moving; frames are needed until it isn't.
    pub fn active(&mut self) -> bool {
        let Some(started) = self.started else {
            return false;
        };
        if started.elapsed() >= settle_time() {
            self.stop();
            return false;
        }
        true
    }

    /// How far card `id` is drawn from its slot now.
    pub fn offset(&self, id: SessionId, now: Instant) -> f32 {
        match (self.started, self.from.get(&id)) {
            (Some(started), Some(&from)) => {
                let state = SpringState {
                    position: from,
                    velocity: 0.,
                };
                let elapsed = now.saturating_duration_since(started).as_secs_f32();
                ROW_SHIFT.step(state, 0., elapsed).position
            }
            _ => 0.,
        }
    }

    /// How far faded in card `id` is now, 1 for one that didn't arrive. The
    /// fade follows the square of the glide, so a new card shows once the card
    /// it takes the place of has mostly moved off it.
    pub fn opacity(&self, id: SessionId, now: Instant) -> f32 {
        match (self.started, self.arriving.get(&id)) {
            (Some(started), Some(&from)) => {
                let state = SpringState {
                    position: 0.,
                    velocity: 0.,
                };
                let elapsed = now.saturating_duration_since(started).as_secs_f32();
                let phase = ROW_SHIFT.step(state, 1., elapsed).position.clamp(0., 1.);
                from + (1. - from) * phase * phase
            }
            _ => 1.,
        }
    }
}

/// How long a glide takes to settle from its farthest start.
fn settle_time() -> Duration {
    let state = SpringState {
        position: 1.,
        velocity: 0.,
    };
    // In units of the travel, settled to a quarter pixel over 1000 px.
    ROW_SHIFT.settle_time(state, 0., PHASE_EPSILON / 4.)
}
