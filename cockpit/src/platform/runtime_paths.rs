//! Locate shipped resources after a checkout is moved or a binary is installed.

use angel_live::host::runtime_paths as host;
use std::path::PathBuf;

#[cfg(test)]
pub(crate) use host::resolve_root;
#[cfg(test)]
use std::path::Path;

pub(crate) fn install() {
    host::install(host::BuildPaths {
        source_cockpit_dir: option_env!("ANGEL_SOURCE_COCKPIT_DIR"),
        bundle_identity: bundle_identity(),
    });
}

pub(crate) fn root() -> PathBuf {
    install();
    host::root()
}

pub(crate) fn bundle_identity() -> &'static str {
    option_env!("ANGEL_BUILD_RESOURCE_SHA256").unwrap_or("unbound")
}

pub(crate) fn status() -> serde_json::Value {
    let root = root();
    let required = [
        "scripts/runtime/repo-dossier.mjs",
        "scripts/runtime/habitsmith-tick.mjs",
        "scripts/runtime/pxpipe-transform.mjs",
        "cockpit/assets/excalibur/rise.png",
    ];
    serde_json::json!({"sha256": bundle_identity(), "available": required.iter().all(|path| root.join(path).is_file())})
}

pub(crate) fn cockpit_dir() -> PathBuf {
    install();
    host::cockpit_dir()
}

pub(crate) fn script(name: &str) -> PathBuf {
    root().join("scripts").join(name)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/runtime_paths__tests.rs"]
mod tests;
