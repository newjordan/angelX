//! The Round Table panel's live council: the stage and the seats around it,
//! followed from the activity signal ([`crate::ui::viz::agentviz`]).
//!
//! On Kitty the panel shows the painted station table
//! ([`crate::ui::viz::council_table`]), composed in Rust from this council.
//! Without Kitty the follower is inert and the panel stays the terminal-native
//! visualization.

use crate::ui::viz::agentviz::{ActivitySnapshot, SeatState};
use crate::ui::viz::council_table::Deed;

/// The table seats sixteen; further seats are counted, not drawn.
pub const MAX_SEATS: usize = 16;
pub const MAX_STAGE_BYTES: usize = 64;
pub const MAX_SEAT_LABEL_BYTES: usize = 48;

#[derive(Clone, Debug)]
pub struct CouncilPresentation {
    pub stage: String,
    /// Seats beyond the sixteen the table can show.
    pub omitted_seats: usize,
    /// The seats at the table, in slot order (sixteen at most).
    pub seats: Vec<CouncilSeat>,
    /// What the centre of the table shows for this stage.
    pub deed: Deed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CouncilSeat {
    pub label: String,
    pub state: SeatState,
}

/// The stage the table is currently presenting, folded from the activity
/// signal: it tracks every sequence bump.
#[derive(Clone, Debug)]
struct ActiveStage {
    stage: String,
    omitted_seats: usize,
    seats: Vec<CouncilSeat>,
}

/// Follows the council for the painted Round Table.
pub struct Council {
    /// Kitty-capable: the panel follows the council.
    live: bool,
    observed_sequence: Option<u64>,
    active_stage: Option<ActiveStage>,
}

impl Council {
    /// Live only for a Kitty-capable interactive viewer.
    pub fn discover(kitty_capable: bool) -> Self {
        if cfg!(test) || !kitty_capable {
            return Self::with_live(false);
        }
        Self::painted()
    }

    /// The painted station table follows the council.
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

    /// A7: without Kitty the panel is inert — skip activity locks and
    /// stage folding on the UI advance hot path.
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// One UI tick: follow the council when the activity moved (the stage is
    /// cloned only then).
    pub fn tick(&mut self) {
        if !self.live {
            return;
        }
        if self.observed_sequence != Some(crate::ui::viz::agentviz::activity_sequence()) {
            self.note_activity(&crate::ui::viz::agentviz::activity());
        }
    }

    /// Fold a new activity revision into the council: refresh the presented
    /// stage and its seats (a seat-state update re-publishes the same stage
    /// under a newer sequence).
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

    /// A painted-table council seated with `states`, labelled `seat 1`….
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
