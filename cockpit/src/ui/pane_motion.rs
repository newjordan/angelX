//! Bounded, interruptible motion for the trace/miniviz divider.
//! Content is available immediately; only the panel geometry eases.

use crate::ui::viz::lifecycle_viz::MotionMode;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct Divider {
    state: Option<Tween>,
}

struct Tween {
    from: f32,
    target: u16,
    started: Instant,
    duration: Duration,
    column_height: u16,
    motion: MotionMode,
}

impl Tween {
    fn sample(&self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32().max(0.001))
        .clamp(0.0, 1.0);
        if t >= 1.0 {
            return self.target as f32;
        }
        let delta = self.target as f32 - self.from;
        let q = t - 1.0;
        // Quick catch, one small stretch, then settle. The overshoot is
        // capped in rows as well as fraction, so large panels stay composed.
        let progress = if self.motion == MotionMode::Full {
            let back = 1.35;
            (1.0 + (back + 1.0) * q.powi(3) + back * q.powi(2))
                .min(1.0 + 1.5 / delta.abs().max(1.0))
        } else {
            1.0 + q.powi(3)
        };
        self.from + delta * progress
    }
}

impl Divider {
    pub(crate) fn height(
        &mut self,
        target: u16,
        column_height: u16,
        now: Instant,
        motion: MotionMode,
        allowed: bool,
    ) -> u16 {
        let reset = !allowed
            || motion == MotionMode::Off
            || self
                .state
                .as_ref()
                .is_none_or(|s| s.column_height != column_height || s.motion != motion);
        if reset {
            self.state = Some(Tween {
                from: target as f32,
                target,
                started: now,
                duration: Duration::ZERO,
                column_height,
                motion,
            });
            return target;
        }
        let previous = self.state.as_ref().expect("initialized above");
        if previous.target != target {
            let from = previous.sample(now);
            let millis = if motion == MotionMode::Reduced {
                120
            } else if target as f32 > from {
                340
            } else {
                420
            };
            self.state = Some(Tween {
                from,
                target,
                started: now,
                duration: Duration::from_millis(millis),
                column_height,
                motion,
            });
        }
        self.state
            .as_ref()
            .unwrap()
            .sample(now)
            .round()
            .clamp(0.0, column_height as f32) as u16
    }

    pub(crate) fn active(&self, now: Instant) -> bool {
        self.state.as_ref().is_some_and(|s| {
            s.from != s.target as f32 && now.saturating_duration_since(s.started) < s.duration
        })
    }

    #[cfg(test)]
    pub(crate) fn advance_for_test(&mut self, elapsed: Duration) {
        if let Some(state) = &mut self.state {
            state.started -= elapsed;
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/pane_motion__tests.rs"]
mod tests;
