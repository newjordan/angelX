//! Who holds the cockpit's one flight slot.
//!
//! The slot is "one turn at a time". Its owners are separate fields on `App`
//! (`thinking`, `bg_job`, `pending_turn`, `loop_pending`, `campaign_pending`,
//! `loop_experiment`) meant to be exclusive, with nothing stored that says who
//! holds it. `App::flight_owner` derives it in one precedence order so a gate
//! names an owner instead of re-spelling the field list. Nothing here is
//! stored or sent; the fields stay where they are.
//!
//! Gates that deliberately test a different set of owners keep their own
//! spelling (exit, Esc, campaign start, `/self`, peer activity), because a
//! single owner cannot say "any of these but not that one" when two fields
//! are set together.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlightOwner {
    Idle,
    Turn { draining: bool },
    BgJob,
    ParkedTurn,
    LoopWork,
    Campaign,
    Experiment,
}

impl FlightOwner {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Turn { draining: false } => "turn",
            Self::Turn { draining: true } => "turn_draining",
            Self::BgJob => "bg_job",
            Self::ParkedTurn => "parked_turn",
            Self::LoopWork => "loop_work",
            Self::Campaign => "campaign",
            Self::Experiment => "experiment",
        }
    }

    pub(crate) fn is_agent_busy(self) -> bool {
        matches!(self, Self::Turn { .. } | Self::BgJob)
    }

    pub(crate) fn holds_turn_slot(self) -> bool {
        !matches!(self, Self::Idle | Self::Experiment)
    }
}

impl App {
    pub(crate) fn flight_owner(&self) -> FlightOwner {
        if let Some(turn) = &self.thinking {
            FlightOwner::Turn { draining: turn.is_draining() }
        } else if self.bg_job.is_some() {
            FlightOwner::BgJob
        } else if self.pending_turn.is_some() {
            FlightOwner::ParkedTurn
        } else if self.loop_pending.is_some() {
            FlightOwner::LoopWork
        } else if self.campaign_pending.is_some() {
            FlightOwner::Campaign
        } else if self.loop_experiment.is_some() {
            FlightOwner::Experiment
        } else {
            FlightOwner::Idle
        }
    }
}
