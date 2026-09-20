//! The yukon competition package — the default family.
//!
//! Pure move of the former `crate::agent::harness::comp_packages::yukon::status`, `crate::agent::harness::comp_packages::yukon::fleet`, and
//! `harness::comp_watch::yukon_source` modules. Re-exports keep the old
//! `crate::yukon_*` paths working while call sites migrate to the registry.

pub(crate) mod fleet;
pub(crate) mod status;
pub(crate) mod submit_identity;
pub(crate) mod watch;

pub(crate) use status::*;

use super::{CompetitionPackage, WorkerProfile};

pub(crate) const PACKAGE: CompetitionPackage = CompetitionPackage::new("yukon", "Yukon");

/// The yukon loop-worker surface. The allowlist is the coding core plus the
/// board/submit journal; secrets-bearing surfaces (`llm`, `embed`,
/// `self_model`, `fleet`, `proc`, `web`, `vision`, `video`, `goal`) stay out.
pub(crate) const WORKER_PROFILE: WorkerProfile = WorkerProfile {
    system_prompt: "You are a competition loop worker on the yukon benchmark family. \
Work the repo: read, edit, build, run the benchmark, submit through the board \
CLI exactly as the loop task directs. You have no access to other harness \
tools; a denial receipt names what was refused. Never wait on permission: if \
an action is denied, choose a different permitted action and continue.",
    allowed_tools: &[
        // coding core
        "read_file",
        "write_file",
        "str_replace",
        "multi_edit",
        "apply_patch",
        "shell",
        "grep",
        "find_files",
        "list_dir",
        "outline",
        "get_context_remaining",
        "code_mode",
        "tool_search",
        "skill",
    ],
};

impl CompetitionPackage {
    /// Open this package's configured submission watch, if operator
    /// configuration adopts a live slot for this family.
    pub(crate) fn configured_watch(
        &self,
    ) -> Result<
        Option<(
            String,
            crate::agent::harness::comp_watch::ConfiguredWatchSource,
        )>,
        String,
    > {
        match self.id {
            "yukon" => watch::configured_yukon_watch(),
            _ => Ok(None),
        }
    }
}
