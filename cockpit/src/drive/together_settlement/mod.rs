//! The settlement ledger, from the realm host
//! (`angel_realm_host::together_settlement`). What stays here is the plug's
//! view of it: the planner's input, which the game derives from the
//! cockpit's labyrinth map, and the worker's contract.

pub(crate) mod exhibits {
    use crate::app::control::realm::GameHost;
    use angel_plug::drive::DriveWorld;

    pub(crate) fn worker_contract(site_id: &str, loop_id: &str, iteration: usize) -> String {
        <GameHost as DriveWorld>::settlement_contract(site_id, loop_id, iteration)
    }
}

pub(crate) mod planning {
    use crate::app::control::realm::GameHost;
    use angel_plug::drive::DriveWorld;
    use std::path::Path;

    pub(crate) type Input = <GameHost as DriveWorld>::SettlementInput;

    pub(crate) fn input(root: &Path, task: &str) -> Result<Option<Input>, String> {
        <GameHost as DriveWorld>::settlement_input(root, task, &crate::drive::labyrinth::Maps)
    }
}
