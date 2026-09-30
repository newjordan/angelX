//! Active harness lane — product policy that shapes root LID behavior.
//!
//! Treebeard is the default RLM/HiQ lane: strategy-only root preamble, handle
//! disclosure, aggressive offload floors, eager bulk veto, and bounded
//! recursive subcall depth. `ANGEL_LANE=default` remains the explicit classic
//! ReAct ablation.

use super::*;
use std::cell::Cell;

/// Product lane for the agent harness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lane {
    /// Explicit classic coding-agent ReAct ablation.
    Default,
    /// Zhang/Khattab-style RLM: strategy-only root, bulk under handles.
    #[default]
    Treebeard,
}

impl Lane {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Treebeard => "treebeard",
        }
    }

    pub(crate) fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "treebeard" | "rlm" | "hiq" | "hi/q" => Self::Treebeard,
            _ => Self::Default,
        }
    }
}

/// Active lane from `ANGEL_LANE` (unset → Treebeard/HiQ).
///
/// The historical ReAct path remains available as the explicit
/// `ANGEL_LANE=default` off-control. Unknown explicit values fail conservative
/// through [`Lane::parse`] to that same control rather than silently opting in.
///
/// Product reads once: Treebeard header paint asks this every frame.
/// Tests keep the live getenv so `EnvGuard` overrides stay visible.
pub(crate) fn active_lane() -> Lane {
    #[cfg(not(test))]
    {
        static LANE: std::sync::OnceLock<Lane> = std::sync::OnceLock::new();
        *LANE.get_or_init(lane_from_env)
    }
    #[cfg(test)]
    lane_from_env()
}

fn lane_from_env() -> Lane {
    std::env::var("ANGEL_LANE")
        .map(|v| Lane::parse(&v))
        .unwrap_or(Lane::Treebeard)
}

#[inline]
pub(crate) fn is_treebeard() -> bool {
    active_lane() == Lane::Treebeard
}

/// The active lane's entry route: Treebeard's decomposition / LID contract is
/// `⠍⠚`, its words on the ledger. `None` for the default lane.
pub(crate) fn lane_route() -> Option<super::book::Route> {
    is_treebeard().then_some(super::book::m_method::TREEBEARD)
}

/// Whether the model-facing `handle_read` tool should register.
/// Treebeard enables it by default; `ANGEL_HANDLE_READ_TOOL=0` forces off.
pub(crate) fn handle_read_tool_enabled() -> bool {
    if !handle_store_enabled() {
        return false;
    }
    match std::env::var("ANGEL_HANDLE_READ_TOOL") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => is_treebeard(),
    }
}

/// Optional env `usize` — `None` when unset or unparsable.
fn env_usize_opt(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
}

/// Floor for offloading successful `code_mode` bodies from root history.
/// Explicit `ANGEL_HANDLE_CODE_MODE_MIN_BYTES` always wins; Treebeard defaults
/// lower (1024) so more intermediate bulk leaves the root.
pub(crate) fn code_mode_offload_min_bytes() -> usize {
    env_usize_opt("ANGEL_HANDLE_CODE_MODE_MIN_BYTES").unwrap_or(if is_treebeard() {
        1024
    } else {
        4096
    })
}

/// Floor for offloading spawn/delegate digests. Treebeard default 512.
pub(crate) fn lane_subcall_offload_min_bytes() -> usize {
    env_usize_opt("ANGEL_HANDLE_SUBCALL_MIN_BYTES").unwrap_or(if is_treebeard() {
        512
    } else {
        2048
    })
}

/// Floor for *eager* tool-result offload before the body enters root history.
///
/// Keep the implicit floor at one complete `handle_read` disclosure. Parking a
/// smaller result only makes the root spend another provider hop to recover the
/// same bytes, which is strictly worse for both latency and history size. An
/// explicit env value remains an experiment/operator override.
pub(crate) fn eager_tool_offload_min_bytes() -> usize {
    env_usize_opt("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES").unwrap_or(16 * 1024)
}

/// Maximum nested LM depth for spawn/delegate seats.
/// Depth 0 = root turn. Default: Treebeard allows two nested levels (root→seat→seat);
/// other lanes allow only root-level spawn (max depth 1 means seats cannot re-spawn).
pub(crate) fn treebeard_max_depth() -> usize {
    env_usize_opt("ANGEL_TREEBEARD_MAX_DEPTH").unwrap_or(if is_treebeard() { 2 } else { 1 })
}

thread_local! {
    static SUBCALL_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Current nested-LM depth (0 at the root driver turn).
pub(crate) fn subcall_depth() -> usize {
    SUBCALL_DEPTH.with(|c| c.get())
}

/// Whether a new spawn/delegate nesting level is allowed from the current depth.
pub(crate) fn spawn_nesting_allowed() -> bool {
    subcall_depth() < treebeard_max_depth()
}

/// RAII bump for seat/delegate threads so nested depth is restored on exit.
pub(crate) struct SubcallDepthGuard {
    prev: usize,
}

impl SubcallDepthGuard {
    pub(crate) fn enter() -> Self {
        let prev = SUBCALL_DEPTH.with(|c| {
            let cur = c.get();
            c.set(cur.saturating_add(1));
            cur
        });
        Self { prev }
    }
}

impl Drop for SubcallDepthGuard {
    fn drop(&mut self) {
        SUBCALL_DEPTH.with(|c| c.set(self.prev));
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/lane__tests.rs"]
mod tests;
