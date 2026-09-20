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
pub(crate) mod tests {
    use super::*;

    /// Test-only activation. `ACTIVE` is a `OnceLock` (activation is
    /// process-permanent by design), so this is a leak on purpose: exactly one
    /// test per process may use it. That test must tolerate the sticky state.
    #[cfg(test)]
    pub(crate) fn test_activate(profile: SealedProfile) {
        let _ = ACTIVE.set(profile);
    }

    fn fake_home(tag: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("sealed-home-{tag}-{}", std::process::id()));
        for dir in HOME_TOOLCHAIN_ALLOW.iter().chain(HOME_DENY.iter()) {
            std::fs::create_dir_all(home.join(dir)).unwrap();
        }
        std::fs::write(home.join(".ssh/config"), b"SEAL-SECRET-KEY").unwrap();
        std::fs::write(home.join(".angel0/KEY-sealed-test"), b"SEAL-SECRET-KEY").unwrap();
        home
    }

    fn fake_workspace(tag: &str) -> PathBuf {
        let ws = std::env::temp_dir().join(format!("sealed-ws-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&ws).unwrap();
        ws
    }

    #[test]
    fn sealed_rejects_home_workspace_and_symlinked_toolchain_grants() {
        let _guard = crate::tests::env_lock();
        let home = fake_home("overlap");
        assert!(validate_policy(&build(&home, Some(&home)).policy).is_err());
        let mut profile = build(&fake_workspace("overlap"), Some(&home));
        profile.policy.writable_roots.push(home.join(".ssh"));
        assert!(validate_policy(&profile.policy).is_err());
        let alias = home.join("secret-alias");
        std::os::unix::fs::symlink(home.join(".ssh"), &alias).unwrap();
        profile.policy.writable_roots.pop();
        profile.policy.sealed_reads.push(alias);
        assert!(validate_policy(&profile.policy).is_err());
    }

    #[test]
    fn sealed_bind_plan_grants_toolchains_and_denies_home_secrets() {
        let _guard = crate::tests::env_lock();
        let home = fake_home("plan");
        let ws = fake_workspace("plan");
        let profile = build(&ws, Some(&home));
        // Writable: the workspace (and nothing else in this fixture).
        assert!(profile.policy.writable_roots.contains(&ws));
        assert!(
            profile
                .policy
                .writable_roots
                .iter()
                .all(|root| root.starts_with(&ws))
        );
        // Read allow-list: toolchain homes yes, secret homes no, /etc no.
        for allow in HOME_TOOLCHAIN_ALLOW {
            assert!(
                profile.read_roots.contains(&home.join(allow)),
                "missing {allow}"
            );
        }
        for deny in [".ssh", ".angel0", ".config", ".gnupg", ".mozilla"] {
            assert!(
                !profile.read_roots.contains(&home.join(deny)),
                "{deny} readable"
            );
            assert!(
                profile.deny_roots.contains(&home.join(deny)),
                "{deny} not in deny list"
            );
        }
        assert!(!profile.read_roots.contains(&PathBuf::from("/etc")));
        assert!(profile.deny_roots.contains(&PathBuf::from("/etc/shadow")));
        // Posture: netless, enforced, mandatory — never widened, never lazy.
        assert!(!profile.policy.allow_network);
        assert!(profile.policy.enforce);
        assert!(profile.policy.mandatory);
        assert_eq!(profile.name, "sealed");
    }

    #[test]
    fn sealed_digest_is_stable_and_bind_plan_sensitive() {
        let _guard = crate::tests::env_lock();
        let home = fake_home("digest");
        let ws = fake_workspace("digest");
        let a = build(&ws, Some(&home));
        let b = build(&ws, Some(&home));
        assert_eq!(
            a.digest, b.digest,
            "same inputs must give the same identity"
        );
        let ws2 = fake_workspace("digest2");
        std::fs::create_dir_all(ws2.join("sub")).unwrap();
        let c = build(&ws2, Some(&home));
        assert_ne!(
            a.digest, c.digest,
            "a different workspace must change the digest"
        );
        assert_eq!(a.digest.len(), 64);
        let mut reads = a.read_roots.clone();
        reads.pop();
        assert_ne!(
            a.digest,
            identity_digest(&ws, &reads, &a.deny_roots, &a.policy.writable_roots)
        );
        let mut invalid = a.policy.clone();
        invalid.allow_network = true;
        assert!(validate_policy(&invalid).is_err());
        invalid.allow_network = false;
        invalid.mandatory = false;
        assert!(validate_policy(&invalid).is_err());
    }

    #[test]
    fn sealed_activation_refuses_yolo() {
        let _guard = crate::tests::env_lock();
        let old = std::env::var_os("ANGEL_YOLO");
        unsafe { std::env::set_var("ANGEL_YOLO", "1") };
        let err = activate(build(&fake_workspace("yolo"), None)).expect_err("must refuse YOLO");
        assert!(
            err.contains("YOLO cannot widen a sealed sandbox profile"),
            "{err}"
        );
        unsafe {
            match old {
                Some(value) => std::env::set_var("ANGEL_YOLO", value),
                None => std::env::remove_var("ANGEL_YOLO"),
            }
        };
        // A refused activation must not install its own profile. OnceLock
        // cannot be unset, so if some earlier test activated one, the digest
        // must differ from this refused attempt's.
        if let Some(active) = identity() {
            assert_ne!(
                active["digest"].as_str().unwrap(),
                build(&fake_workspace("yolo"), None).digest,
                "a refused activation must not stick"
            );
        }
    }

    #[test]
    fn sealed_override_merges_only_workspace_scoped_writable_roots() {
        let _guard = crate::tests::env_lock();
        // Activation is process-permanent (`OnceLock`): an in-process
        // activation here would leak the sealed posture into every later
        // `run_sandboxed` test in this binary (real landlock flakes). The
        // active branch runs in a forked test binary of exactly this test.
        if std::env::var("ANGEL_T_SEALED_MERGE_CHILD").as_deref() != Ok("1") {
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args([
                "--exact",
                "sandbox::sealed::tests::sealed_override_merges_only_workspace_scoped_writable_roots",
                "--nocapture",
            ])
            .env("ANGEL_T_SEALED_MERGE_CHILD", "1");
            let output = cmd.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let home = fake_home("merge");
        let ws = fake_workspace("merge");
        let profile = build(&ws, Some(&home));
        test_activate(profile);
        let adhoc = SandboxPolicy {
            writable_roots: vec![ws.join("target"), PathBuf::from("/tmp/escape")],
            allow_network: true,
            enforce: false,
            mandatory: false,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        };
        let merged = override_for(&adhoc).expect("sealed active must override");
        assert!(merged.writable_roots.contains(&ws.join("target")));
        assert!(
            !merged
                .writable_roots
                .contains(&PathBuf::from("/tmp/escape"))
        );
        assert!(!merged.writable_roots.contains(&ws));
        let readonly = SandboxPolicy {
            writable_roots: Vec::new(),
            ..adhoc
        };
        assert!(override_for(&readonly).unwrap().writable_roots.is_empty());
        let mut cmd = std::process::Command::new("true");
        cmd.env_clear();
        crate::sandbox::set_helper_policy(&mut cmd, &readonly).unwrap();
        let vars: std::collections::HashMap<_, _> = cmd
            .get_envs()
            .filter_map(|(key, value)| {
                value.map(|value| {
                    (
                        key.to_string_lossy().into_owned(),
                        value.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect();
        assert_eq!(vars[crate::sandbox::HELPER_BACKEND_ENV], "bwrap");
        let encoded: SandboxPolicy =
            serde_json::from_str(&vars[crate::sandbox::HELPER_POLICY_ENV]).unwrap();
        assert!(encoded.writable_roots.is_empty());
        assert!(!encoded.sealed_reads.is_empty());
        assert!(!encoded.allow_network);
        assert!(encoded.mandatory);
        assert!(!merged.allow_network);
        assert!(merged.mandatory);
        // An already-sealed policy passes through untouched (no double wrap).
        assert!(override_for(&merged).is_none());
    }

    #[test]
    fn sealed_read_roots_never_include_deny_list_even_when_nested() {
        let _guard = crate::tests::env_lock();
        let home = fake_home("nest");
        let ws = fake_workspace("nest").join("deep"); // nested under ws
        std::fs::create_dir_all(&ws).unwrap();
        let profile = build(&ws, Some(&home));
        // No deny root may be at or under a read root: the allow-list is
        // constructed only from workspace/toolchain/OS trees, never $HOME
        // secret subtrees or /etc.
        for deny in &profile.deny_roots {
            assert!(
                profile
                    .read_roots
                    .iter()
                    .all(|root| !deny.starts_with(root)),
                "deny root {deny:?} is readable via the bind plan"
            );
        }
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;

    #[test]
    fn d02_sealed_policy_requires_all_three_confinement_flags() {
        for enforce in [false, true] {
            for mandatory in [false, true] {
                for allow_network in [false, true] {
                    let policy = SandboxPolicy {
                        writable_roots: Vec::new(),
                        sealed_reads: Vec::new(),
                        deny_reads: Vec::new(),
                        enforce,
                        mandatory,
                        allow_network,
                    };
                    assert_eq!(
                        validate_policy(&policy).is_ok(),
                        enforce && mandatory && !allow_network,
                        "enforce={enforce}, mandatory={mandatory}, allow_network={allow_network}"
                    );
                }
            }
        }
    }
}
