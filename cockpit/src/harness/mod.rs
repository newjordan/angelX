//! The agent harness: per-agent tool-loop + the multi-agent orchestrator layer.
//!
//! A [`Tool`] is a named capability a club can call. [`ToolRegistry`] holds them
//! and dispatches by name. [`run_turn`] drives one [`Club`] through the
//! OpenAI-style tool-calling loop until it answers (or `max_hops` is hit).
//!
//! On top of that single-agent loop sits the **orchestrator** (v1, orchestrator-
//! led topology): the in-hand Driver calls [`DelegateTool`] to run a specialist
//! club in an isolated **git worktree** (its own sandboxed `shell`/`cargo`),
//! captures the resulting diff, and then [`IntegrateTool`] merges that branch
//! into the shared workspace **one at a time** — serialized writes, so multiple
//! agents touching the same files can't race. The harness's own git operations
//! run OUTSIDE the sandbox (trusted); only the specialists' tools are sandboxed.

pub(crate) use crate::club::{ChatMsg, ChatRole, Club, ToolCall, ToolDef};
pub(crate) use crate::sandbox::{self, SandboxPolicy};
pub(crate) use crate::tools::build::{
    CargoTool, CheckTool, FmtTool, LintTool, PinnedCargo, RunTestsTool,
};
pub(crate) use crate::tools::embed::maybe_register_embedding_tools;
pub(crate) use crate::tools::file::{
    ApplyPatchTool, MultiEditTool, ReadFileTool, ResolveEditTool, StrReplaceTool, WriteFileTool,
};
pub(crate) use crate::tools::fleet::maybe_register_fleet_tools;
pub(crate) use crate::tools::git::{GitCommitTool, GitDiffTool, GitLogTool, GitStatusTool};
pub(crate) use crate::tools::llm::maybe_register_llm_tools;
pub(crate) use crate::tools::nav::{
    DefsTool, FileSearchTool, FindFilesTool, GrepTool, ListDirTool, OutlineTool,
};
pub(crate) use crate::tools::plan::{HandoffTool, NotesTool, TodoTool};
pub(crate) use crate::tools::proc::maybe_register_proc_tools;
pub(crate) use crate::tools::repair::ToolRepairTool;
pub(crate) use crate::tools::repos::maybe_register_repos;
pub(crate) use crate::tools::science::maybe_register_science;
pub(crate) use crate::tools::shell::ShellTool;
pub(crate) use crate::tools::utilities::{PresentTool, ReverseTool, WordCountTool};
pub(crate) use crate::tools::video::maybe_register_video_tools;
pub(crate) use crate::tools::vision::maybe_register_vision_tools;
pub(crate) use crate::tools::web::{
    maybe_register_grok_research, maybe_register_http_request, maybe_register_web_fetch,
    maybe_register_web_search,
};
// Re-exported for the reinforce loop, which scores on these verifiable signals.
pub use crate::tools::build::{TestOutcome, parse_lint, parse_test_result};
pub(crate) use serde_json::Value;
pub(crate) use std::collections::HashMap;
pub(crate) use std::collections::hash_map::DefaultHasher;
pub(crate) use std::ffi::OsString;
pub(crate) use std::hash::{Hash, Hasher};
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::process::Command;
pub(crate) use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
pub(crate) use std::sync::{Arc, mpsc};
pub(crate) use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const DEFAULT_CONTEXT_BUDGET_TOKENS: usize = 333_000;
pub(crate) const DEFAULT_SOTA_CONTEXT_BUDGET_TOKENS: usize = 120_000;
pub(crate) const DEFAULT_AUTO_RECALL_K: usize = 12;

mod action_capsule;
mod agent_graph;
mod auxiliary;
mod code_mode;
pub(crate) mod coeffect;
mod comp_watch;
pub(crate) mod comp_packages;
mod compact;
mod confined_fs;
mod context;
mod delegated_lineage;
mod descendant_budget;
pub(crate) mod exec;
mod execution_blocker;
mod footprint;
pub(crate) mod formation_budget;
mod handle_store;
mod hooks;
pub(crate) mod independence;
pub(crate) mod interception;
mod knowledge_graph;
mod lane;
mod loop_experiment;
mod needs_pro;
mod orchestrator;
mod project;
mod recall;
pub(crate) mod registration;
mod registry;
mod rollout;
pub(crate) mod run_identity;
mod skills;
mod spawn;
mod swarm_compile;
mod task_mode;
mod task_recon;
mod teacher_watch;
pub(crate) mod tool_errors;
mod trajectory;
mod turn;
mod workspace_state;

pub(crate) use action_capsule::*;
pub(crate) use agent_graph::*;
pub(crate) use code_mode::*;
pub(crate) use comp_watch::*;
pub(crate) use compact::*;
pub(crate) use confined_fs::*;
pub(crate) use context::*;
pub(crate) use descendant_budget::*;
pub(crate) use exec::*;
pub(crate) use execution_blocker::{execution_blocker, is_execution_blocker};
pub(crate) use footprint::*;
pub(crate) use handle_store::*;
pub(crate) use hooks::*;
pub(crate) use knowledge_graph::*;
pub(crate) use lane::*;
pub(crate) use loop_experiment::*;
pub(crate) use needs_pro::*;
pub(crate) use orchestrator::*;
pub(crate) use project::*;
pub(crate) use recall::*;
pub(crate) use registry::*;
pub(crate) use rollout::*;
pub(crate) use skills::*;
pub(crate) use spawn::*;
pub(crate) use swarm_compile::*;
pub(crate) use task_mode::*;
pub(crate) use task_recon::*;
pub(crate) use teacher_watch::*;
pub(crate) use trajectory::*;
pub(crate) use turn::*;
pub(crate) use workspace_state::*;

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/tests.rs"]
mod tests;

pub(crate) mod shell_verifier;
