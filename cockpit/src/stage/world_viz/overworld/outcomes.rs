//! The receipts behind the lantern seals: one per place, kept three seconds.
//!
//! These show public execution/verification receipts, never inferred reasoning.
//! One seal per place keeps a busy village legible; no timers, allocations or
//! persistence are added to the tool path beyond the scene's existing snapshot.

use super::map::Place;
use crate::agent::harness::{ExecutionOutcome, ToolOutcome, VerificationOutcome};

#[cfg(test)]
pub(crate) use angel_delve::render::overworld::outcomes::lantern;
pub(crate) use angel_delve::render::overworld::outcomes::{Echo, Seal};

/// A tool's receipt as the seal it leaves.
fn seal_of(outcome: ToolOutcome) -> Seal {
    match (outcome.execution, outcome.verification) {
        (ExecutionOutcome::Failed | ExecutionOutcome::Panicked, _)
        | (ExecutionOutcome::Succeeded, VerificationOutcome::Failed) => Seal::Failed,
        (ExecutionOutcome::Succeeded, VerificationOutcome::Passed) => Seal::Verified,
        (ExecutionOutcome::Succeeded, VerificationOutcome::NotApplicable) => Seal::Completed,
        _ => Seal::Unresolved,
    }
}

/// Three seconds on the world's 40 Hz clock, including a quiet final hold.
const LIFETIME: u64 = 120;
const FRAME_TICKS: u64 = 10;

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
            seal: seal_of(outcome),
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

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__outcomes_tests.rs"]
mod tests;
