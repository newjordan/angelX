//! Unattended controllers that run the engine over many turns.

#[macro_use]
mod wire_status;

pub(crate) mod campaign;
pub(crate) mod comp_mode;
#[allow(dead_code)]
// Contract-first scaffold; consumers land in Wave 1 (board-sync is the first live path).
pub(crate) mod competition;
pub(crate) mod conductor;
pub(crate) mod continual_harness;
#[allow(dead_code)]
pub(crate) mod deli;
pub(crate) mod goal;
pub(crate) mod guardian;
pub(crate) mod graph_ctl;
pub(crate) mod habits;
pub(crate) mod handoff_rl;
pub(crate) mod iterate;
pub(crate) mod labyrinth;
pub(crate) mod loop_ctl;
pub(crate) mod loop_dialog;
pub(crate) mod realm_support;
#[allow(dead_code)] // reinforce-loop core; wired to DICE/GEPA in a follow-up
pub(crate) mod reinforce;
pub(crate) mod research_workspace;
pub(crate) mod round_table;
pub(crate) mod rl_ctl;
pub(crate) mod science;
pub(crate) mod self_loop;
pub(crate) mod together_settlement;
