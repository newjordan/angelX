//! Tool implementations for the agent harness.
//!
//! The [`Tool`](crate::harness::Tool) trait, the `ToolRegistry`, and the
//! `run_turn` loop live in [`crate::harness`]; this module holds the concrete
//! `Tool` impls, grouped by capability so the harness file stays focused on the
//! loop and registry. Shared helpers (descriptor-confined filesystem access,
//! `cap_tool_output`, `run_sandboxed`, …) remain `pub(crate)` in `harness` and
//! are imported here.

pub(crate) mod benchmark;
pub mod build;
pub mod consult;
pub mod embed;
pub mod file;
pub mod fleet;
pub mod git;
pub mod goal;
pub(crate) mod http_transport;
pub(crate) mod jev;
pub mod llm;
pub(crate) mod loop_research;
pub mod nav;
pub mod plan;
pub mod proc;
pub mod repair;
pub mod repos;
pub(crate) mod rl_campaign;
pub(crate) mod runtime_missing;
pub mod science;
pub mod self_model;
pub mod shell;
pub mod solo;
#[path = "../harness/comp_packages/yukon/submit_identity.rs"]
pub(crate) mod submit_identity;
pub mod utilities;
pub mod video;
pub mod vision;
pub mod web;
pub mod work_landing;
