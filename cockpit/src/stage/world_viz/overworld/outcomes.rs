//! Small lantern seals left at a landmark when its tool reports back.
//!
//! These show public execution/verification receipts, never inferred reasoning.
//! One seal per place keeps a busy village legible; no timers, allocations or
//! persistence are added to the tool path beyond the scene's existing snapshot.

use super::ink::Img;
use super::map::Place;
use crate::agent::harness::{ExecutionOutcome, ToolOutcome, VerificationOutcome};
use crate::ui::viz::lifecycle_viz::MotionMode;

/// Three seconds on the world's 40 Hz clock, including a quiet final hold.
const LIFETIME: u64 = 120;
const FRAME_TICKS: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Seal {
    Completed,
    Verified,
    Failed,
    Unresolved,
}

impl Seal {
    fn from_outcome(outcome: ToolOutcome) -> Self {
        match (outcome.execution, outcome.verification) {
            (ExecutionOutcome::Failed | ExecutionOutcome::Panicked, _)
            | (ExecutionOutcome::Succeeded, VerificationOutcome::Failed) => Self::Failed,
            (ExecutionOutcome::Succeeded, VerificationOutcome::Passed) => Self::Verified,
            (ExecutionOutcome::Succeeded, VerificationOutcome::NotApplicable) => Self::Completed,
            _ => Self::Unresolved,
        }
    }

    fn ink(self) -> char {
        match self {
            Self::Completed => '5',
            Self::Verified => '2',
            Self::Failed => '7',
            Self::Unresolved => 'v',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Echo {
    pub(crate) place: Place,
    pub(crate) seal: Seal,
    pub(crate) phase: u8,
}

#[derive(Clone, Copy, Debug)]
struct Stamp {
    seal: Seal,
    tick: u64,
}

#[derive(Default)]
pub(crate) struct Outcomes {
    // Fixed storage: a storm of results can never grow a particle queue.
    places: [Option<Stamp>; Place::ALL.len()],
}

impl Outcomes {
    pub(crate) fn clear(&mut self, place: Place) {
        self.places[place as usize] = None;
    }

    pub(crate) fn note(&mut self, place: Place, outcome: ToolOutcome, tick: u64) {
        self.places[place as usize] = Some(Stamp {
            seal: Seal::from_outcome(outcome),
            tick,
        });
    }

    pub(crate) fn shown(&self, tick: u64) -> Vec<Echo> {
        Place::ALL
            .into_iter()
            .filter_map(|place| {
                let stamp = self.places[place as usize]?;
                let age = tick.checked_sub(stamp.tick)?;
                (age < LIFETIME).then_some(Echo {
                    place,
                    seal: stamp.seal,
                    // Motes have settled by frame six; the final hold must
                    // not invalidate the frame cache with invisible phases.
                    phase: (age / FRAME_TICKS).min(6) as u8,
                })
            })
            .collect()
    }
}

/// A little metal lantern with a distinct seal inside. Reduced/off motion
/// retain the same receipt as a still shape: no motes, bob, pulse or flashing.
pub(crate) fn lantern(echo: Echo, motion: MotionMode) -> Img {
    let mut im = Img::new(23, 25);
    let color = echo.seal.ink();
    im.line(11, 16, 11, 23, 'G');
    im.line(8, 23, 14, 23, 'j');
    for dy in -6i32..=6 {
        let half = 6 - dy.abs();
        im.rect(11 - half, 10 + dy, half * 2 + 1, 1, 'k');
    }
    for (x0, y0, x1, y1) in [
        (11, 4, 17, 10),
        (17, 10, 11, 16),
        (11, 16, 5, 10),
        (5, 10, 11, 4),
    ] {
        im.line(x0, y0, x1, y1, 'G');
    }
    match echo.seal {
        Seal::Completed => {
            im.line(11, 7, 14, 10, color);
            im.line(14, 10, 11, 13, color);
            im.line(11, 13, 8, 10, color);
            im.line(8, 10, 11, 7, color);
            im.put(11, 10, '@');
        }
        Seal::Verified => {
            im.line(8, 10, 10, 12, color);
            im.line(10, 12, 14, 8, color);
            im.put(14, 7, '3');
        }
        Seal::Failed => {
            im.line(9, 8, 13, 12, color);
            im.line(13, 8, 9, 12, color);
        }
        Seal::Unresolved => {
            im.line(9, 8, 9, 12, color);
            im.line(13, 8, 13, 12, color);
        }
    }
    if motion == MotionMode::Full && echo.seal != Seal::Unresolved && echo.phase < 6 {
        let phase = i32::from(echo.phase);
        // Completed work lifts its motes; failures settle as embers. The
        // glyph stays put so meaning never depends on catching one frame.
        let drift = if echo.seal == Seal::Failed {
            phase / 2
        } else {
            -(phase / 2)
        };
        for (i, (x, y)) in [(2, 8), (20, 9), (6, 3), (17, 3)].into_iter().enumerate() {
            if phase < 3 || i % 2 == 0 {
                im.put(x, y + drift, color);
            }
        }
    }
    im
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__outcomes_tests.rs"]
mod tests;
