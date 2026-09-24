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

/// The yukon loop-worker surface: every harness tool the session registers,
/// background jobs (`proc_run`) and the GPU included.
pub(crate) const WORKER_PROFILE: WorkerProfile = WorkerProfile {
    system_prompt: "You are a competition loop worker on the yukon benchmark family. \
Work the repo: read, edit, build, run the benchmark, submit through the board \
CLI exactly as the loop task directs. Every harness tool is yours: run long \
benchmarks as background jobs with proc_run and keep working while they finish.",
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
