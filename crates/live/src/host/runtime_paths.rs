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

/// What the cockpit's build knows about where it ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildPaths {
    /// The cockpit crate's source folder (`ANGEL_SOURCE_COCKPIT_DIR`), absent
    /// in a portable build so no shipped binary carries the builder's folders.
    pub source_cockpit_dir: Option<&'static str>,
    /// The release resource identity (`ANGEL_BUILD_RESOURCE_SHA256`), or
    /// `"unbound"` when the build has none.
    pub bundle_identity: &'static str,
}

static BUILD: OnceLock<BuildPaths> = OnceLock::new();

/// Record the cockpit's build paths. The first call wins and later calls are
/// no-ops, so the cockpit can install from every entry point. Returns whether
/// this call installed them.
pub fn install(build: BuildPaths) -> bool {
    BUILD.set(build).is_ok()
}

/// The source tree the installing build ran in. `None` before [`install`] and
/// in a portable build.
pub fn source_cockpit_dir() -> Option<&'static Path> {
    BUILD
        .get()
        .and_then(|build| build.source_cockpit_dir)
        .map(Path::new)
}

/// The release resource identity, `"unbound"` before [`install`] or when the
/// build has none.
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
    // A portable build has no source tree to fall back on.
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
