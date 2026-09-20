//! Named `sealed` sandbox profile (S02 host read/network isolation).
//!
//! Unlike the ad-hoc permissive/read-only postures, `sealed` is an explicit,
//! named profile: the host filesystem is unreadable outside a fixed
//! allow-list (the workspace, the OS trees, and the pinned toolchains' own
//! directories), the network is always unshared, every descendant inherits
//! the same namespace ceilings (`no_new_privs` + inherited mount/seccomp
//! rules), and the posture is `enforce=true, mandatory=true` so a helper
//! failure is a hard stop — never a silent fall-back to permissive. YOLO
//! cannot widen it.
//!
//! Bubblewrap starts from an empty mount tree and exposes only resolved roots.
//! The private procfs contains namespace-local processes; host HOME secrets and
//! `/etc/shadow` have no mount. `/etc/alternatives` is read-only so standard
//! compiler aliases resolve. Overlapping secret grants are rejected.

use super::SandboxPolicy;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const NAME: &str = "sealed";

/// Read-only host trees every sealed run needs: the OS payload and optional
/// package roots plus compiler alternatives. Excludes broad `/etc`, `/home`, `/root`, `/var`,
/// `/srv`, `/mnt`, `/media` and `/run`.
const HOST_READ_ROOTS: [&str; 8] = [
    "/usr",
    "/bin",
    "/sbin",
    "/lib",
    "/lib64",
    "/libx32",
    "/opt",
    "/etc/alternatives",
];

/// `$HOME` subtrees that must never be readable. Kept even when absent so the
/// identity digest and the qualification cohort agree on the contract.
const HOME_DENY: [&str; 5] = [".ssh", ".angel0", ".config", ".gnupg", ".mozilla"];

/// `$HOME` subtrees granted read-only for the pinned toolchains.
const HOME_TOOLCHAIN_ALLOW: [&str; 3] = [".cargo", ".rustup", ".nvm"];

static ACTIVE: OnceLock<SealedProfile> = OnceLock::new();

/// Resolve `name` on `PATH` without spawning a process: the sealed profile
/// must be assembled deterministically before any confinement decision.
fn resolve_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The pinned toolchains' own directories, resolved the way the verifier
/// registry resolves them: `$HOME/.cargo`, `$HOME/.rustup`, `$HOME/.nvm`
/// read-only, plus the directory of each resolved toolchain binary.
fn toolchain_read_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = HOME_TOOLCHAIN_ALLOW
        .iter()
        .map(|dir| home.join(dir))
        .filter(|path| path.exists())
        .collect();
    for tool in ["cargo", "rustc", "node", "npm", "python3"] {
        if let Some(binary) = resolve_on_path(tool)
            && let Some(dir) = binary.parent()
        {
            roots.push(dir.to_path_buf());
        }
    }
    roots
}

fn canonical(existing: &Path) -> PathBuf {
    existing
        .canonicalize()
        .unwrap_or_else(|_| existing.to_path_buf())
}

/// A fully resolved sealed profile: the bind plan plus its identity digest.
#[derive(Clone, Debug)]
pub struct SealedProfile {
    pub name: &'static str,
    /// Canonical, sorted, deduplicated read-only bind plan.
    #[cfg_attr(not(test), allow(dead_code))]
    pub read_roots: Vec<PathBuf>,
    /// Denied paths: any overlapping read or write grant is rejected.
    #[cfg_attr(not(test), allow(dead_code))]
    pub deny_roots: Vec<PathBuf>,
    pub policy: SandboxPolicy,
    pub digest: String,
}

pub fn build(workspace: &Path, home: Option<&Path>) -> SealedProfile {
    let workspace = canonical(workspace);
    let home = home
        .map(canonical)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .map(|h| canonical(&h));

    let mut read_roots: Vec<PathBuf> = HOST_READ_ROOTS.iter().map(PathBuf::from).collect();
    read_roots.push(workspace.clone());
    // Bubblewrap mounts a fresh procfs in a private PID namespace.
    if let Some(home) = &home {
        read_roots.extend(toolchain_read_roots(home));
    }
    for key in ["CARGO_HOME", "RUSTUP_HOME", "NVM_DIR"] {
        if let Some(root) = std::env::var_os(key) {
            read_roots.push(PathBuf::from(root));
        }
    }
    read_roots.retain(|path| path.exists());
    read_roots = read_roots.iter().map(|p| canonical(p)).collect();
    read_roots.sort();
    read_roots.dedup();

    let deny_roots: Vec<PathBuf> = home
        .iter()
        .flat_map(|home| {
            HOME_DENY
                .iter()
                .map(|dir| home.join(dir))
                .collect::<Vec<_>>()
        })
        .chain([PathBuf::from("/etc/gshadow"), PathBuf::from("/etc/shadow")])
        .collect();

    let mut writable_roots = vec![workspace.clone()];
    writable_roots.extend(SandboxPolicy::git_worktree_roots(&workspace));

    let digest = identity_digest(&workspace, &read_roots, &deny_roots, &writable_roots);
    let sealed_reads = read_roots.clone();
    let deny_list = deny_roots.clone();
    SealedProfile {
        name: NAME,
        read_roots,
        deny_roots,
        policy: SandboxPolicy {
            writable_roots,
            allow_network: false,
            enforce: true,
            mandatory: true,
            sealed_reads,
            deny_reads: deny_list,
        },
        digest,
    }
}

fn identity_digest(
    workspace: &Path,
    read_roots: &[PathBuf],
    deny_roots: &[PathBuf],
    writable_roots: &[PathBuf],
) -> String {
    let say = |roots: &[PathBuf]| -> Vec<String> {
        roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    };
    let aliases: Vec<_> = ["/bin", "/sbin", "/lib", "/lib64", "/libx32"]
        .iter()
        .filter_map(|path| {
            std::fs::read_link(path)
                .ok()
                .map(|target| (path, target.to_string_lossy().into_owned()))
        })
        .collect();
    let canonical = serde_json::json!({
        "plan_version": 1,
        "backend": "bwrap",
        "private_namespaces": ["user", "pid", "net"],
        "proc": "private-pid-namespace",
        "root": "read-only-tmpfs",
        "devices": ["null", "zero", "full", "random", "urandom", "tty"],
        "inet_socket_filter": true,
        "aliases": aliases,
        "name": NAME,
        "workspace": workspace.to_string_lossy(),
        "allow_network": false,
        "enforce": true,
        "mandatory": true,
        "read_roots": say(read_roots),
        "deny_roots": say(deny_roots),
        "writable_roots": say(writable_roots),
    });
    super::sha256_hex(canonical.to_string().as_bytes())
}

/// Reject overlapping grants: additive filesystem permissions cannot subtract secrets.
pub(super) fn validate_policy(policy: &SandboxPolicy) -> Result<(), String> {
    if !policy.enforce || !policy.mandatory || policy.allow_network {
        return Err("sealed requires enforced, mandatory, netless confinement".into());
    }
    for root in policy.sealed_reads.iter().chain(&policy.writable_roots) {
        let root = canonical(root);
        if root == Path::new("/") || root.starts_with("/proc") || root.starts_with("/sys") {
            return Err(format!(
                "sealed rejects broad or kernel grant {}",
                root.display()
            ));
        }
        for deny in &policy.deny_reads {
            let deny = canonical(deny);
            if root.starts_with(&deny) || deny.starts_with(&root) {
                return Err(format!(
                    "sealed grant {} overlaps denied path {}",
                    root.display(),
                    deny.display()
                ));
            }
        }
    }
    Ok(())
}

/// Activate the sealed profile for this process. Fails closed — never widens.
#[cfg(target_os = "linux")]
pub fn activate(profile: SealedProfile) -> Result<(), String> {
    if crate::yolo::enabled() {
        return Err(
            "YOLO cannot widen a sealed sandbox profile; disable YOLO before requesting sealed"
                .to_string(),
        );
    }
    validate_policy(&profile.policy)?;
    ACTIVE
        .set(profile)
        .map_err(|_| "sealed profile is already active".to_string())
}

#[cfg(not(target_os = "linux"))]
pub fn activate(_profile: SealedProfile) -> Result<(), String> {
    Err("the sealed sandbox profile requires Linux (Landlock + bwrap)".to_string())
}

/// Identity carried into the ledger record (`identity.sandbox`) and the
/// `--task-json` envelope: profile name plus resolved bind plan digest.
pub fn identity() -> Option<serde_json::Value> {
    ACTIVE.get().map(|profile| {
        serde_json::json!({
            "name": profile.name,
            "digest": profile.digest,
        })
    })
}

/// Replace an ad-hoc tool policy with the sealed posture while preserving
/// tool-requested writable roots that stay inside the workspace. Returns
/// `None` when sealed is inactive or the policy is already sealed.
pub(crate) fn override_for(policy: &SandboxPolicy) -> Option<SandboxPolicy> {
    let profile = ACTIVE.get()?;
    if !policy.sealed_reads.is_empty() {
        return None; // already the sealed posture
    }
    let mut merged = profile.policy.clone();
    // Intersect with the requested authority, preserving read-only delegates.
    merged.writable_roots.clear();
    for requested in &policy.writable_roots {
        let requested = canonical(requested);
        for allowed in &profile.policy.writable_roots {
            let allowed = canonical(allowed);
            let root = if requested.starts_with(&allowed) {
                requested.clone()
            } else if allowed.starts_with(&requested) {
                allowed
            } else {
                continue;
            };
            if !merged.writable_roots.contains(&root) {
                merged.writable_roots.push(root);
            }
        }
    }
    Some(merged)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/sandbox__sealed__tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/sandbox__sealed__mutation_tests.rs"]
mod mutation_tests;
