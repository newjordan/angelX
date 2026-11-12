pub(crate) use angel_realm_host::together_settlement::*;
use {
    crate::delve::loop_summary::LoopSummary,
    angel_delve::rules::{Pack, ROWS, RoomKind, Tile, layout},
    std::collections::BTreeMap,
};

pub(crate) mod planning {
    pub(crate) use angel_realm_host::together_settlement::planning::*;
    use std::path::Path;

    pub(crate) fn input(root: &Path, task: &str) -> Result<Option<Input>, String> {
        crate::delve::settlement_input::input(root, task, &crate::drive::labyrinth::Maps)
    }

    pub(crate) fn input_for_door(
        root: &Path,
        task: &str,
        door: &str,
    ) -> Result<Option<Input>, String> {
        crate::delve::settlement_input::input_for_door(
            root,
            task,
            door,
            &crate::drive::labyrinth::Maps,
        )
    }
}

#[path = "../../../../../tests/cockpit/app/together_settlement__tests.rs"]
mod tests;

#[path = "../../../../../tests/cockpit/app/together_settlement__artifact_tests.rs"]
mod artifact_tests;

#[path = "../../../../../tests/cockpit/together_settlement/planning__tests.rs"]
mod planning_tests;
