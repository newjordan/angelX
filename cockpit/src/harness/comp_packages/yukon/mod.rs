//! The yukon competition package — the default family.
//!
//! Pure move of the former `crate::harness::comp_packages::yukon::status`, `crate::harness::comp_packages::yukon::fleet`, and
//! `harness::comp_watch::yukon_source` modules. Re-exports keep the old
//! `crate::yukon_*` paths working while call sites migrate to the registry.

pub(crate) mod fleet;
pub(crate) mod status;
pub(crate) mod submit_identity;
pub(crate) mod watch;

pub(crate) use status::*;

use super::CompetitionPackage;

pub(crate) const PACKAGE: CompetitionPackage = CompetitionPackage::new("yukon", "Yukon");

impl CompetitionPackage {
    /// Open this package's configured submission watch, if operator
    /// configuration adopts a live slot for this family.
    pub(crate) fn configured_watch(
        &self,
    ) -> Result<Option<(String, crate::harness::comp_watch::ConfiguredWatchSource)>, String> {
        match self.id {
            "yukon" => watch::configured_yukon_watch(),
            _ => Ok(None),
        }
    }
}
