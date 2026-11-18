//! The spin as the picture reads it: where the wheel is in its four beats,
//! how long since it landed, how fast it turns, what the room should feel.
//! Everything is read off the run (`Spin` and the tick), so a friend's
//! mirror and a browser guest draw the same moment the host does.

use crate::rules::Run;
use crate::rules::fortune::{Beat, Luck, Mode, SPIN_TICKS, Spin};

pub(super) const LONG: u32 = 1_000_000;

pub(super) struct Show {
    pub spin: Spin,
    pub n: u32,
    pub beat: Beat,
    pub since: Option<u32>,
    pub mode: Mode,
    pub luck: Luck,
    pub speed: f32,
    pub rot: f32,
    pub held: u32,
}

pub(super) fn of(run: &Run) -> Option<Show> {
    let spin = run.spin?;
    let n = run.tick.saturating_sub(spin.started).min(u64::from(u32::MAX / 4)) as u32;
    let done = spin.done(run.tick);
    let since = done.then(|| if spin.settled && n < SPIN_TICKS { LONG } else { n.saturating_sub(SPIN_TICKS) });
    let mode = Mode::WHEEL[usize::from(spin.wedge)];
    Some(Show {
        spin,
        n,
        beat: if done { Beat::Landed } else { spin.beat(n) },
        since,
        mode,
        luck: mode.luck(),
        speed: if done { 0.0 } else { spin.speed(n) },
        rot: spin.angle(run.tick),
        held: run.hitstop.left,
    })
}

impl Show {
    pub fn clock(&self, run: &Run) -> u32 {
        run.tick as u32 * 4 + (6 - self.held.min(6))
    }

    pub fn since_click(&self, most: u32) -> Option<u32> {
        if self.since.is_some() {
            return None;
        }
        (0..=most.min(self.n)).find(|&a| self.spin.clicked(self.n - a))
    }
}

pub(super) fn push(rot: f32) -> f32 {
    let w = std::f32::consts::TAU / Mode::WHEEL.len() as f32;
    let f = (rot / w).rem_euclid(1.0);
    ((f - 0.7) / 0.3).clamp(0.0, 1.0)
}
