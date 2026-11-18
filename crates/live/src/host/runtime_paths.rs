//! Locate shipped resources after a checkout is moved or a binary is installed.
//!
//! The lookup itself is the cockpit's own and runs on every call: the
//! launcher's `ANGEL_RESOURCE_DIR`, then where the executable sits, then the
//! source tree the build ran in. Only that last fact is a compile-time
//! constant of the *cockpit's* build (its build script names the source
//! folder, and a portable build names none), so the cockpit installs it once
//! with [`install`] and everything below it, the realm host included, reads
//! the same answer. Nothing is frozen into a path: a changed environment
//! variable or a moved executable is seen on the next call, exactly as before.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildPaths {
    pub source_cockpit_dir: Option<&'static str>,
    pub bundle_identity: &'static str,
}

static BUILD: OnceLock<BuildPaths> = OnceLock::new();

pub fn install(build: BuildPaths) -> bool {
    BUILD.set(build).is_ok()
}

pub fn source_cockpit_dir() -> Option<&'static Path> {
    BUILD
        .get()
        .and_then(|build| build.source_cockpit_dir)
        .map(Path::new)
}

pub fn bundle_identity() -> &'static str {
    BUILD.get().map_or("unbound", |build| build.bundle_identity)
}

pub fn resolve_root(
    configured: Option<PathBuf>,
    executable: Option<&Path>,
    manifest: Option<&Path>,
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
    manifest.map_or_else(
        || PathBuf::from("."),
        |manifest| manifest.parent().unwrap_or(manifest).to_path_buf(),
    )
}

pub fn root() -> PathBuf {
    resolve_root(
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
        std::env::current_exe().ok().as_deref(),
        source_cockpit_dir(),
        bundle_identity(),
    )
}

pub fn cockpit_dir() -> PathBuf {
    root().join("cockpit")
}
