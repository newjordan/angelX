//! Locate shipped resources after a checkout is moved or a binary is installed.

use std::path::{Path, PathBuf};

fn resolve_root(
    configured: Option<PathBuf>,
    executable: Option<&Path>,
    manifest: &Path,
    source: &str,
) -> PathBuf {
    if let Some(root) = configured {
        return root;
    }
    if let Some(executable) = executable {
        if let Some(prefix) = executable.parent().and_then(Path::parent) {
            let installed = prefix.join("share/angelX/bundles").join(source);
            if installed.join("cockpit").is_dir() && installed.join("scripts").is_dir() {
                return installed;
            }
        }
        for ancestor in executable.ancestors().skip(1).take(6) {
            if ancestor.join("cockpit/Cargo.toml").is_file() && ancestor.join("scripts").is_dir() {
                return ancestor.to_path_buf();
            }
        }
    }
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

pub(crate) fn root() -> PathBuf {
    resolve_root(
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
        std::env::current_exe().ok().as_deref(),
        Path::new(env!("CARGO_MANIFEST_DIR")),
        bundle_identity(),
    )
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
    root().join("cockpit")
}

pub(crate) fn script(name: &str) -> PathBuf {
    root().join("scripts").join(name)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/runtime_paths__tests.rs"]
mod tests;
