//! The settlement ledger, from the realm host
//! (`angel_realm_host::together_settlement`). What stays here is the labyrinth
//! adapter beside the planner (`settlement_input`, which reads the cockpit's
//! map) and the ledger's tests, which build an `App`.

#[cfg(test)]
pub(crate) use angel_realm_host::together_settlement::*;
// What the ledger's tests took from the module they sat in.
#[cfg(test)]
use {
    super::loop_summary::LoopSummary,
    angel_delve::rules::{Pack, ROWS, RoomKind, Tile, layout},
    std::collections::BTreeMap,
};

pub(crate) mod exhibits {
    pub(crate) use angel_realm_host::together_settlement::exhibits::*;
}

pub(crate) mod planning {
    pub(crate) use angel_realm_host::together_settlement::planning::*;
    // The labyrinth adapter lives cockpit-side; these keep the old paths resolving.
    pub(crate) use crate::drive::settlement_input::input;
    #[cfg(test)]
    pub(crate) use crate::drive::settlement_input::input_for_door;
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_settlement__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_settlement__artifact_tests.rs"]
mod artifact_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/together_settlement/planning__tests.rs"]
mod planning_tests;
