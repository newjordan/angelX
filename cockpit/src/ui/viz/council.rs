//! The Round Table panel's live council: the stage and the seats around it,
//! followed from the activity signal ([`crate::ui::viz::agentviz`]).
//!
//! On Kitty the panel shows the painted station table
//! ([`crate::ui::viz::council_table`]), composed in Rust from this council.
//! Without Kitty the follower is inert and the panel stays the terminal-native
//! visualization.

use crate::ui::viz::agentviz::{ActivitySnapshot, SeatState};
use crate::ui::viz::council_table::Deed;

pub const MAX_SEATS: usize = 16;
pub const MAX_STAGE_BYTES: usize = 64;
pub const MAX_SEAT_LABEL_BYTES: usize = 48;

#[derive(Clone, Debug)]
pub struct CouncilPresentation {
    pub stage: String,
    pub omitted_seats: usize,
    pub seats: Vec<CouncilSeat>,
    pub deed: Deed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CouncilSeat {
    pub label: String,
    pub state: SeatState,
}

#[derive(Clone, Debug)]
struct ActiveStage {
    stage: String,
    omitted_seats: usize,
    seats: Vec<CouncilSeat>,
}

pub struct Council {
    live: bool,
    observed_sequence: Option<u64>,
    active_stage: Option<ActiveStage>,
}

impl Council {
    pub fn discover(kitty_capable: bool) -> Self {
        if cfg!(test) || !kitty_capable {
            return Self::with_live(false);
        }
        Self::painted()
    }

    pub fn painted() -> Self {
        Self::with_live(true)
    }

    fn with_live(live: bool) -> Self {
        Self {
            live,
            observed_sequence: None,
            active_stage: None,
        }
    }

    pub fn is_live(&self) -> bool {
        self.live
    }

    pub fn tick(&mut self) {
        if !self.live {
            return;
        }
        if self.observed_sequence != Some(crate::ui::viz::agentviz::activity_sequence()) {
            self.note_activity(&crate::ui::viz::agentviz::activity());
        }
    }

    fn note_activity(&mut self, activity: &ActivitySnapshot) {
        if self.observed_sequence == Some(activity.sequence) {
            return;
        }
        self.observed_sequence = Some(activity.sequence);
        self.active_stage = activity.stage.as_ref().map(|stage| {
            let shown = stage.agents.len().min(MAX_SEATS);
            ActiveStage {
                stage: truncate_utf8(&stage.name, MAX_STAGE_BYTES),
                omitted_seats: stage.agents.len().saturating_sub(shown),
                seats: stage
                    .agents
                    .iter()
                    .take(MAX_SEATS)
                    .enumerate()
                    .map(|(slot, label)| CouncilSeat {
                        label: truncate_utf8(label, MAX_SEAT_LABEL_BYTES),
                        state: stage.seat_states.get(slot).copied().unwrap_or_default(),
                    })
                    .collect(),
            }
        });
    }

    pub fn presentation(&self) -> Option<CouncilPresentation> {
        let active = self.active_stage.as_ref()?;
        Some(CouncilPresentation {
            stage: active.stage.clone(),
            omitted_seats: active.omitted_seats,
            seats: active.seats.clone(),
            deed: Deed::of_stage(&active.stage),
        })
    }

    #[cfg(test)]
    pub fn presentation_for_test(stage: &str, active_seats: usize) -> Self {
        let states = vec![SeatState::Running; active_seats];
        Self::council_for_test(stage, &states)
    }

    #[cfg(test)]
    pub fn council_for_test(stage: &str, states: &[SeatState]) -> Self {
        let mut council = Self::painted();
        council.observed_sequence = Some(1);
        council.active_stage = Some(ActiveStage {
            stage: truncate_utf8(stage, MAX_STAGE_BYTES),
            omitted_seats: states.len().saturating_sub(MAX_SEATS),
            seats: states
                .iter()
                .take(MAX_SEATS)
                .enumerate()
                .map(|(slot, state)| CouncilSeat {
                    label: format!("seat {}", slot + 1),
                    state: *state,
                })
                .collect(),
        });
        council
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_string()
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/council__tests.rs"]
mod tests;
