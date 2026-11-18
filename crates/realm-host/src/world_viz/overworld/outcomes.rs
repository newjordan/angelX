//! The receipts behind the lantern seals: one per place, kept three seconds.
//!
//! These show public execution/verification receipts, never inferred reasoning.
//! One seal per place keeps a busy village legible; no timers, allocations or
//! persistence are added to the tool path beyond the scene's existing snapshot.

use super::map::Place;
use crate::telemetry::{ExecutionOutcome, ToolOutcome, VerificationOutcome};

#[cfg(test)]
pub use angel_delve::render::overworld::outcomes::lantern;
pub use angel_delve::render::overworld::outcomes::{Echo, Seal};

fn seal_of(outcome: ToolOutcome) -> Seal {
    match (outcome.execution, outcome.verification) {
        (ExecutionOutcome::Failed | ExecutionOutcome::Panicked, _)
        | (ExecutionOutcome::Succeeded, VerificationOutcome::Failed) => Seal::Failed,
        (ExecutionOutcome::Succeeded, VerificationOutcome::Passed) => Seal::Verified,
        (ExecutionOutcome::Succeeded, VerificationOutcome::NotApplicable) => Seal::Completed,
        _ => Seal::Unresolved,
    }
}

const LIFETIME: u64 = 120;
const FRAME_TICKS: u64 = 10;

#[derive(Clone, Copy, Debug)]
struct Stamp {
    seal: Seal,
    tick: u64,
}

#[derive(Default)]
pub struct Outcomes {
    places: [Option<Stamp>; Place::ALL.len()],
}

impl Outcomes {
    pub fn clear(&mut self, place: Place) {
        self.places[place as usize] = None;
    }

    pub fn note(&mut self, place: Place, outcome: ToolOutcome, tick: u64) {
        self.places[place as usize] = Some(Stamp {
            seal: seal_of(outcome),
            tick,
        });
    }

    pub fn shown(&self, tick: u64) -> Vec<Echo> {
        let mut shown: Vec<(u64, Echo)> = Place::ALL
            .into_iter()
            .filter_map(|place| {
                let stamp = self.places[place as usize]?;
                let age = tick.checked_sub(stamp.tick)?;
                (age < LIFETIME).then_some((
                    stamp.tick,
                    Echo {
                        place,
                        seal: stamp.seal,
                        phase: (age / FRAME_TICKS).min(6) as u8,
                    },
                ))
            })
            .collect();
        if angel_delve::world::plan::tabula() {
            shown.sort_by_key(|&(tick, _)| tick);
            shown.drain(..shown.len().saturating_sub(1));
        }
        shown.into_iter().map(|(_, echo)| echo).collect()
    }
}

#[cfg(test)]
#[path = "../../../tests/world_viz/overworld__outcomes_tests.rs"]
mod tests;
