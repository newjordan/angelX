//! Minimal filesystem sandbox for the agent's autonomous shell tool.
//!
//! Linux confines with Landlock (+ an optional seccomp network filter); macOS
//! confines with Seatbelt (`sandbox_init(3)`, the same primitive behind
//! `sandbox-exec`) using a generated SBPL profile that mirrors the Landlock
//! posture. Other platforms fail closed.
//!
//! Adapted from codex-rs `linux-sandbox/src/landlock.rs` (Apache-2.0) — the lean
//! landlock primitive Codex keeps "for reference" (its live path uses bubblewrap,
//! which is far heavier and drags in five more codex crates). We take just the
//! landlock filesystem ruleset: the whole filesystem stays **readable**, writes
//! are confined to a set of roots, and `no_new_privs` blocks privilege
//! escalation. Commands first exec a fresh, single-threaded Angel helper, which
//! installs the ruleset and then execs the requested program. The domain is
//! inherited by every descendant, so the whole process tree is contained
//! without running allocator-heavy setup after `fork` in the cockpit process.
//!
//! Default posture is **confined**: write-mode agents use Landlock with the
//! practical roots required by local developer tools. Set `ANGEL_SANDBOX=0`
//! only for an explicit operator-approved unconfined session. Confinement is
//! read-all; write to the
//! active tool workspace, explicit cache/runtime roots, `/tmp`, `/var/tmp`, and
//! `/dev/shm`; network left ON (no seccomp filter). The whole home directory and
//! process cwd are deliberately not implicit write roots: either may contain a
//! different project after `/cd`. There is no read-only posture: every seat's
//! shell gets the same developer roots, GPU device nodes and network.
//!
//! When armed, this sandbox guards against the agent's *commands* doing damage
//! (writing outside the workspace, escalating privs). It does NOT cap GPU/VRAM
//! — a runaway compute kernel is the brick-guard's job, not landlock's.

#[path = "sandbox/process_owner.rs"]
pub(crate) mod process_owner;

use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

// `#[path]` keeps the declaration correct for both crate roots: plain
// `mod bwrap;` resolves only when sandbox.rs is loaded as `src/sandbox.rs`
// owning `src/sandbox/`, but the tiny angel-sandbox helper includes this file
// via `#[path = "sandbox.rs"]` from src/sandbox_main.rs, where directory
// ownership is lost and the child would resolve to `src/bwrap.rs`. The
// explicit path is relative to `src/` from both roots.
#[cfg(target_os = "linux")]
#[path = "sandbox/compatibility.rs"]
pub(crate) mod compatibility;
#[path = "sandbox/status.rs"]
pub(crate) mod status;

#[cfg(target_os = "linux")]
#[path = "sandbox/alias_mounts.rs"]
mod alias_mounts;
#[cfg(target_os = "linux")]
#[path = "sandbox/bwrap.rs"]
mod bwrap;

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn bwrap_diagnostic_plan(policy: &SandboxPolicy) -> String {
    bwrap::diagnostic_plan(policy)
}

#[cfg(target_os = "linux")]
#[path = "sandbox/hardlinks.rs"]
mod hardlinks;

#[cfg(all(test, target_os = "linux"))]
pub(crate) use hardlinks::TestRoot as HardlinkTestRoot;

#[path = "sandbox/sealed.rs"]
pub mod sealed;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SandboxPolicy {
    /// Directories the sandboxed process may write to. Everything else on the
    /// filesystem is read-only.
    pub writable_roots: Vec<PathBuf>,
    /// Network access. Permissive leaves this true; `false` denies creation of
    /// IPv4/IPv6 sockets with a helper-local seccomp filter and additionally
    /// asks Landlock to deny TCP bind/connect when the kernel supports it.
    pub allow_network: bool,
    /// Whether [`apply`] actually installs the landlock ruleset. `false` makes
    /// the policy a no-op (unconfined). [`SandboxPolicy::permissive`] reads
    /// this from `ANGEL_SANDBOX` (default on).
    pub enforce: bool,
    /// Semantic capability ceilings (review/read-only seats) remain mandatory
    /// even when the root operator enables YOLO. Ordinary confinement is an
    /// operator posture; delegated authority is not.
    #[serde(default)]
    pub mandatory: bool,
    /// Sealed posture: the ONLY host subtrees granted read access. Empty for
    /// ad-hoc policies, which keep the whole-filesystem-read default. When
    /// non-empty the Bubblewrap plan binds exactly these paths read-only
    /// and nothing else.
    #[serde(default)]
    pub sealed_reads: Vec<PathBuf>,
    /// Any read or write grant overlapping these paths is rejected for sealed
    /// helpers; additive permission rules cannot subtract a secret subtree.
    #[serde(default)]
    pub deny_reads: Vec<PathBuf>,
}

fn cargo_install_metadata_paths(home: &Path) -> [PathBuf; 5] {
    [
        home.join(".cargo/.crates.toml"),
        home.join(".cargo/.crates2.json"),
        home.join(".cargo/.global-cache"),
        home.join(".cargo/.package-cache"),
        home.join(".cargo/.package-cache-mutate"),
    ]
}

fn configured_enabled() -> bool {
    crate::agent::harness::env_flag("ANGEL_SANDBOX", true)
}

/// Whether write-confinement is effectively armed. YOLO is the higher-level
/// operator override for ordinary root tools. Mandatory delegated capability
/// postures still enforce inside [`prepare`] even while this convenience status
/// reports the root sandbox as off; turning YOLO off restores ordinary
/// confinement live.
pub fn enabled() -> bool {
    configured_enabled() && !crate::platform::yolo::enabled()
}

/// SHA-256 hex digest used by sealed-profile identities. Anchored here so the
/// sandbox module (also compiled standalone by angel-sandbox) has one helper.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    // Same digest the cockpit's cut module produces; implemented locally to
    // keep the helper binary independent of cockpit-only modules.
    use ring::digest;
    let out = digest::digest(&digest::SHA256, bytes);
    let mut hex = String::with_capacity(64);
    for byte in out.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// macOS per-user temp and cache roots (`/var/folders/<x>/<y>/T` and `/C`),
/// exactly what the system hands processes whose `TMPDIR` is unset.
#[cfg(target_os = "macos")]
fn darwin_user_dirs() -> Vec<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    [
        libc::_CS_DARWIN_USER_TEMP_DIR,
        libc::_CS_DARWIN_USER_CACHE_DIR,
    ]
    .into_iter()
    .filter_map(|name| {
        let mut buf = vec![0u8; libc::PATH_MAX as usize];
        let len = unsafe { libc::confstr(name, buf.as_mut_ptr().cast(), buf.len()) };
        (len > 1 && (len as usize) <= buf.len()).then(|| {
            buf.truncate(len as usize - 1);
            PathBuf::from(std::ffi::OsString::from_vec(buf))
        })
    })
    .collect()
}

#[cfg(not(target_os = "macos"))]
fn darwin_user_dirs() -> Vec<PathBuf> {
    Vec::new()
}

impl SandboxPolicy {
    /// Permissive posture: read everything; network on; when the sandbox is
    /// armed (`ANGEL_SANDBOX=1`), writes confined to explicit tool roots plus
    /// bounded scratch/cache/runtime directories. Callers that own a workspace
    /// append it; this constructor never guesses from process cwd or grants all
    /// of `$HOME`.
    pub fn permissive() -> Self {
        let mut writable_roots = vec![
            PathBuf::from("/tmp"),
            PathBuf::from("/var/tmp"),
            PathBuf::from("/dev/shm"),
            // NVIDIA's CUDA runtime names helper threads by writing their
            // `comm` files here during cuInit.  This is process-local metadata,
            // not a route to mutate another process or the host filesystem.
            PathBuf::from("/proc/self/task"),
        ];
        // TMPDIR is macOS's per-user scratch root (/var/folders/…/T); compilers
        // and test runners write there constantly. Bounded scratch, same class
        // as /tmp.
        for key in ["XDG_RUNTIME_DIR", "XDG_CACHE_HOME", "TMPDIR"] {
            if let Some(path) = std::env::var_os(key) {
                writable_roots.push(PathBuf::from(path));
            }
        }
        // A shared cargo build directory the operator exported is build
        // output, the same class as the caches below. Without it every
        // sandboxed `cargo` call fails on `.cargo-lock: Permission denied`.
        // Never widened to `/` or the home directory itself.
        let home_dir = std::env::var_os("HOME").map(PathBuf::from);
        for key in ["CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"] {
            if let Some(path) = std::env::var_os(key).map(PathBuf::from)
                && path.is_absolute()
                && path.parent().is_some()
                && home_dir.as_deref() != Some(path.as_path())
            {
                writable_roots.push(path);
            }
        }
        // A headless worker or a launcher that scrubbed its environment may
        // carry no TMPDIR at all, yet macOS compilers still resolve the same
        // per-user scratch and cache roots through confstr(3). Without them
        // `swiftc -print-target-info` fails with a bare `permissionDenied` and
        // SwiftPM reports an internal error, so grant the kernel-derived
        // directories directly instead of trusting the environment to name them.
        writable_roots.extend(darwin_user_dirs());
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            // Dependency and compiler caches are disposable supporting state,
            // not project source or user configuration.
            writable_roots.extend([
                home.join(".cache"),
                home.join(".cargo/registry"),
                home.join(".cargo/git"),
                home.join(".npm/_cacache"),
                // macOS cache home (the ~/.cache analogue); dropped on Linux by
                // the existence filter below.
                home.join("Library/Caches"),
            ]);
            // `cargo install` updates these bounded package ledgers beside the
            // writable bin/registry roots. Without them, installation compiles
            // successfully and then fails at the final metadata write, sending
            // the agent into repeated workaround attempts. Never grant the
            // whole `.cargo` directory: config and credentials stay read-only.
            writable_roots.extend(
                cargo_install_metadata_paths(&home)
                    .into_iter()
                    .filter(|path| path.exists()),
            );
            // User-level install destinations. `no_new_privs` makes system
            // package managers (sudo apt …) unusable by design, so these are
            // the sanctioned way to self-serve a missing binary: `pip install
            // --user`, `cargo install`, or a static build dropped into
            // `~/.local/bin` (which exec already prefers on PATH). Created
            // best-effort because Landlock can only grant paths that exist.
            for dir in [".local/bin", ".local/lib", ".cargo/bin"] {
                let path = home.join(dir);
                let _ = std::fs::create_dir_all(&path);
                writable_roots.push(path);
            }
        }
        // Accelerator drivers open their character devices read/write even
        // when the workload only mutates device memory.  A whole-filesystem
        // read rule is therefore insufficient: NVML queries can succeed while
        // CUDA context creation fails with CUDA_ERROR_OPERATING_SYSTEM.  Grant
        // only recognized compute/display device nodes, never all of `/dev`.
        if let Ok(entries) = std::fs::read_dir("/dev") {
            writable_roots.extend(entries.flatten().filter_map(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("nvidia")
                    .then(|| entry.path())
            }));
        }
        writable_roots.extend(
            [PathBuf::from("/dev/dri"), PathBuf::from("/dev/kfd")]
                .into_iter()
                .filter(|path| path.exists()),
        );
        writable_roots.retain(|path| path.exists());
        Self {
            writable_roots,
            allow_network: true,
            // Capture the configured posture, not the live YOLO override. That
            // lets `/yolo off` restore confinement without rebuilding tools.
            enforce: configured_enabled(),
            mandatory: false,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        }
    }

    /// Git metadata roots a confined workspace at `dir` needs for commits.
    ///
    /// A linked worktree keeps its git state OUTSIDE the worktree: `dir/.git`
    /// is a gitfile pointing at `<repo>/.git/worktrees/<name>`, and a commit
    /// also writes the shared object store and refs under the repository's
    /// common `.git`. Without these roots an agent confined to a worktree can
    /// edit code but never commit — run4 of the dogfood loop died on
    /// `index.lock: Permission denied`. A regular repository (`.git` is a
    /// directory inside `dir`) needs nothing extra and returns empty.
    pub fn git_worktree_roots(dir: &std::path::Path) -> Vec<PathBuf> {
        let Ok(contents) = std::fs::read_to_string(dir.join(".git")) else {
            return Vec::new();
        };
        let Some(gitdir) = contents.strip_prefix("gitdir:").map(str::trim) else {
            return Vec::new();
        };
        let gitdir = PathBuf::from(gitdir);
        let mut roots = Vec::new();
        if let Ok(common) = std::fs::read_to_string(gitdir.join("commondir")) {
            let common = PathBuf::from(common.trim());
            let common = if common.is_absolute() {
                common
            } else {
                gitdir.join(common)
            };
            roots.push(common.canonicalize().unwrap_or(common));
        }
        roots.push(gitdir);
        roots.retain(|path| path.exists());
        roots
    }
}

/// A fully-built Landlock ruleset ready for the restriction step. Path lookup,
/// rule construction, and their allocations happen while the dedicated helper
/// is still unrestricted and single-threaded.
#[cfg(target_os = "linux")]
pub struct PreparedSandbox {
    ruleset: Option<landlock::RulesetCreated>,
    allow_network: bool,
}

/// Build the sandbox in the dedicated helper before restricting it. The
/// returned value owns only the completed ruleset fd and fixed-size state.
#[cfg(target_os = "linux")]
#[allow(deprecated)] // landlock 0.4 deprecated set_no_new_privs; still correct here
pub fn prepare(policy: &SandboxPolicy) -> Result<PreparedSandbox, String> {
    use landlock::{
        ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, Ruleset, RulesetAttr,
        RulesetCreatedAttr,
    };

    // Root YOLO can lift ordinary containment, but never a delegated semantic
    // capability ceiling such as read-only/research. Otherwise a seat can ask
    // for read-only tools, fan out concurrently, and write through its shell.
    if (crate::platform::yolo::enabled() && !policy.mandatory) || !policy.enforce {
        return Ok(PreparedSandbox {
            ruleset: None,
            allow_network: true,
        });
    }

    let abi = ABI::V5;
    let access_rw = AccessFs::from_all(abi);
    let access_ro = AccessFs::from_read(abi);

    let mut ruleset_builder = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(access_rw)
        .map_err(|e| format!("landlock handle filesystem access: {e}"))?;
    if !policy.allow_network {
        ruleset_builder = ruleset_builder
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(AccessNet::from_all(ABI::V4))
            .map_err(|e| format!("landlock handle network access: {e}"))?
            .set_compatibility(CompatLevel::BestEffort);
    }
    let mut ruleset = ruleset_builder
        .create()
        .map_err(|e| format!("landlock create: {e}"))?
        .add_rules(landlock::path_beneath_rules(
            // Whole filesystem readable — EXCEPT under a sealed profile,
            // where Landlock's deny-by-default ruleset means granting only
            // the resolved allow-list confines every read (host secrets
            // like ~/.ssh, ~/.angel, ~/.config, ~/.gnupg and /etc/shadow
            // become unreachable, not merely unwritable).
            if policy.sealed_reads.is_empty() {
                vec![PathBuf::from("/")]
            } else {
                policy.sealed_reads.clone()
            },
            access_ro,
        ))
        .map_err(|e| format!("landlock read rule: {e}"))?
        // /dev/null is expected writable by most programs.
        .add_rules(landlock::path_beneath_rules(["/dev/null"], access_rw))
        .map_err(|e| format!("landlock /dev/null rule: {e}"))?
        .set_no_new_privs(true);

    if !policy.writable_roots.is_empty() {
        ruleset = ruleset
            .add_rules(landlock::path_beneath_rules(
                &policy.writable_roots,
                access_rw,
            ))
            .map_err(|e| format!("landlock writable rule: {e}"))?;
    }

    Ok(PreparedSandbox {
        ruleset: Some(ruleset),
        allow_network: policy.allow_network,
    })
}

#[cfg(target_os = "linux")]
impl PreparedSandbox {
    /// Restrict the current helper immediately before `exec`. On the success
    /// path this performs only `prctl`, Landlock, seccomp, and fd-close work;
    /// all allocation and filesystem traversal happened in [`prepare`].
    #[allow(deprecated)]
    pub fn restrict(self) -> Result<(), String> {
        use landlock::RulesetStatus;

        let Some(ruleset) = self.ruleset else {
            return Ok(());
        };
        let status = ruleset
            .restrict_self()
            .map_err(|e| format!("landlock restrict_self: {e}"))?;
        if status.ruleset == RulesetStatus::NotEnforced {
            return Err("landlock not enforced by this kernel".to_string());
        }

        if !self.allow_network {
            install_inet_socket_filter()?;
        }
        Ok(())
    }
}

#[cfg(all(target_os = "linux", test))]
pub fn apply(policy: &SandboxPolicy) -> Result<(), String> {
    prepare(policy)?.restrict()
}

const HELPER_POLICY_ENV: &str = "ANGEL_INTERNAL_SANDBOX_POLICY";
const HELPER_BACKEND_ENV: &str = "ANGEL_INTERNAL_SANDBOX_BACKEND";
const HELPER_LIFECYCLE_ENV: &str = "ANGEL_INTERNAL_SANDBOX_LIFECYCLE";

/// Trusted process tools may outlive the cockpit. This is helper metadata,
/// never a model-facing shell option; the caller must own a process group and
/// provide detached stdio so process-stop can still reap the complete tree.
#[cfg(test)]
pub(crate) fn set_detached_lifecycle(command: &mut Command) {
    command.env(HELPER_LIFECYCLE_ENV, "detached");
}

/// Build a command that enters confinement through a fresh Angel process.
/// There is deliberately no `pre_exec` hook in the multithreaded cockpit: the
/// helper applies Landlock while single-threaded and then replaces itself with
/// the requested program.
pub fn command<I, S>(
    program: impl AsRef<OsStr>,
    args: I,
    policy: &SandboxPolicy,
) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let helper = helper_executable()?;
    if !helper.exists() {
        return Err(format!(
            "sandbox helper missing at {} — the angel binary was rebuilt or moved \
             under this live session; restart the cockpit to realign",
            helper.display()
        ));
    }
    let mut command = Command::new(helper);
    // Tool helpers have no interactive input unless a caller explicitly overrides it.
    command.stdin(std::process::Stdio::null());
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    command
        .arg("--sandbox-exec")
        .arg("--")
        .arg(program)
        .args(args);
    set_helper_policy(&mut command, policy)?;
    Ok(command)
}

/// Resolve (and cache) the helper path at startup, before any live rebuild can
/// poison `/proc/self/exe`. Called from `main`; harmless anywhere else.
pub fn prime_helper() {
    let _ = helper_executable();
}

/// A binary rebuilt or renamed under a live session reads as
/// "<path> (deleted)" from `/proc/self/exe`. The path itself now holds the
/// freshly built angel, whose `--sandbox-exec` contract is exactly what the
/// helper needs — spawning the literal "(deleted)" string is what used to turn
/// every shell tool call into `spawn failed: No such file or directory`.
fn sanitize_deleted_exe_path(path: PathBuf) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    match text.strip_suffix(" (deleted)") {
        Some(clean) => PathBuf::from(clean),
        None => path,
    }
}

/// Restore the private helper policy after a caller intentionally used
/// `Command::env_clear()` on the wrapper command.
pub fn set_helper_policy(command: &mut Command, policy: &SandboxPolicy) -> Result<(), String> {
    let overridden = sealed::override_for(policy);
    let mut policy = overridden.as_ref().unwrap_or(policy).clone();
    // Grants are selected in the caller's directory, but the helper starts in
    // the tool's working directory. Preserve their identity across that chdir:
    // a relative workspace must not become workspace/workspace inside the
    // helper. Keep absent deny paths and symlinks intact; do not drop grants or
    // widen confinement to recover from a resolution error.
    for root in policy
        .writable_roots
        .iter_mut()
        .chain(&mut policy.sealed_reads)
        .chain(&mut policy.deny_reads)
    {
        if root.is_relative() {
            *root = std::path::absolute(&*root).map_err(|error| {
                format!("resolve sandbox policy path {}: {error}", root.display())
            })?;
        }
    }
    let encoded = serde_json::to_string(&policy)
        .map_err(|error| format!("serialize sandbox policy: {error}"))?;
    command.env(HELPER_POLICY_ENV, encoded);
    command.env(HELPER_LIFECYCLE_ENV, "attached");
    // Keep the selected backend across callers' env_clear(), just like the
    // authoritative policy. Tool payloads cannot select a weaker outer domain.
    command.env(
        HELPER_BACKEND_ENV,
        if !policy.sealed_reads.is_empty() {
            "bwrap".into()
        } else {
            std::env::var("ANGEL_SANDBOX_BACKEND").unwrap_or_else(|_| "landlock".into())
        },
    );
    Ok(())
}

/// Decide which executable answers `--sandbox-exec`: an explicit pin wins (a
/// path, or `self` to re-exec the cockpit binary as before), else a deployed
/// sibling `angel-sandbox` beside the cockpit binary, else the cockpit binary
/// itself. Pure so the ladder stays unit-testable.
fn choose_helper(current_exe: &Path, sibling_exists: bool, pin: Option<&str>) -> PathBuf {
    match pin.map(str::trim).filter(|pin| !pin.is_empty()) {
        Some("self") => current_exe.to_path_buf(),
        Some(path) => sanitize_deleted_exe_path(PathBuf::from(path)),
        None if sibling_exists => current_exe
            .parent()
            .map(|dir| dir.join(format!("angel-sandbox{}", std::env::consts::EXE_SUFFIX)))
            .unwrap_or_else(|| current_exe.to_path_buf()),
        None => current_exe.to_path_buf(),
    }
}

// An explicit helper pin must work even when the outer sandbox deliberately
// omits procfs. Only self/sibling discovery needs the running executable path.
fn resolve_helper_path(
    current_exe: impl FnOnce() -> std::io::Result<PathBuf>,
    pin: Option<&str>,
) -> Result<PathBuf, String> {
    let pin = pin.map(str::trim).filter(|value| !value.is_empty());
    if let Some(path) = pin.filter(|value| *value != "self") {
        return Ok(sanitize_deleted_exe_path(PathBuf::from(path)));
    }
    let current_exe = current_exe()
        .map(sanitize_deleted_exe_path)
        .map_err(|error| format!("locate sandbox helper: {error}"))?;
    let sibling_exists = current_exe
        .parent()
        .map(|dir| dir.join(format!("angel-sandbox{}", std::env::consts::EXE_SUFFIX)))
        .is_some_and(|sibling| sibling.is_file());
    Ok(choose_helper(&current_exe, sibling_exists, pin))
}

#[cfg(not(test))]
fn helper_executable() -> Result<PathBuf, String> {
    use std::sync::OnceLock;

    static HELPER: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    HELPER
        .get_or_init(|| {
            resolve_helper_path(
                std::env::current_exe,
                std::env::var("ANGEL_SANDBOX_HELPER").ok().as_deref(),
            )
        })
        .clone()
}

#[cfg(test)]
fn helper_executable() -> Result<PathBuf, String> {
    use std::sync::OnceLock;

    static HELPER: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    HELPER
        .get_or_init(|| {
            // Qualified and instrumented test runners own immutable helpers.
            // Rebuilding here would replace their bound child executable and
            // silently mix source/profile identities during the same test run.
            // This override is absent from the production helper selector.
            if let Some(path) = std::env::var_os("ANGEL_T_SANDBOX_HELPER") {
                let path = std::fs::canonicalize(path)
                    .map_err(|error| format!("locate explicit test sandbox helper: {error}"))?;
                let metadata = std::fs::metadata(&path)
                    .map_err(|error| format!("inspect explicit test sandbox helper: {error}"))?;
                if !metadata.is_file() {
                    return Err("explicit test sandbox helper must be a regular file".into());
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;
                    if metadata.permissions().mode() & 0o111 == 0 {
                        return Err("explicit test sandbox helper is not executable".into());
                    }
                }
                return Ok(path);
            }
            let test_exe = std::env::current_exe()
                .map_err(|error| format!("locate test executable: {error}"))?;
            let profile_dir = test_exe.parent().and_then(Path::parent).ok_or_else(|| {
                format!("unexpected test executable path: {}", test_exe.display())
            })?;
            // Use the dedicated helper. Building the full cockpit without its
            // defaults here used to replace the default-feature PTY child
            // behind Cargo's already-compiled integration harness.
            let helper = profile_dir.join(format!("angel-sandbox{}", std::env::consts::EXE_SUFFIX));
            let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
            let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .args([
                    "build",
                    "--locked",
                    "--offline",
                    "--quiet",
                    "--no-default-features",
                    "--bin",
                    "angel-sandbox",
                ])
                .arg("--manifest-path")
                .arg(&manifest)
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .map_err(|error| format!("build sandbox helper: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "build sandbox helper failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            if !helper.is_file() {
                return Err(format!(
                    "sandbox helper was not built at {}",
                    helper.display()
                ));
            }
            Ok(helper)
        })
        .clone()
}

fn sandbox_trace(phase: &str) {
    if !matches!(
        std::env::var("ANGEL_SANDBOX_TRACE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    ) {
        return;
    }
    let pid = std::process::id();
    eprintln!("[angel-sandbox pid={pid} phase={phase}]");
}

/// Hidden single-threaded helper entrypoint used by [`command`].
pub fn exec_helper(args: impl IntoIterator<Item = OsString>) -> std::io::Result<()> {
    let args: Vec<_> = args.into_iter().collect();
    #[cfg(target_os = "linux")]
    if args.first().is_some_and(|arg| arg == "--mount-aliases") {
        return alias_mounts::exec(&args[1..]);
    }
    exec_helper_inner(
        args,
        #[cfg(target_os = "linux")]
        compatibility::detect,
    )
}

fn exec_helper_inner(
    args: impl IntoIterator<Item = OsString>,
    #[cfg(target_os = "linux")] detect: fn() -> Option<compatibility::Cause>,
) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;

    sandbox_trace("start");
    let encoded = std::env::var(HELPER_POLICY_ENV).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("missing sandbox helper policy: {error}"),
        )
    })?;
    let policy: SandboxPolicy = serde_json::from_str(&encoded).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid sandbox helper policy: {error}"),
        )
    })?;
    sandbox_trace("profile");
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(OsStr::new("--")) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "sandbox helper requires `-- program [args...]`",
        ));
    }
    let program = args.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "sandbox helper requires a program",
        )
    })?;
    let backend = std::env::var(HELPER_BACKEND_ENV)
        .or_else(|_| std::env::var("ANGEL_SANDBOX_BACKEND"))
        .unwrap_or_else(|_| "landlock".into());
    let detached = std::env::var(HELPER_LIFECYCLE_ENV).as_deref() == Ok("detached");
    // This entrypoint is reached before the cockpit starts any threads.
    // Scrub helper metadata before fork so neither supervisor nor command
    // child re-reads a trusted lifecycle choice from the environment.
    unsafe {
        std::env::remove_var(HELPER_POLICY_ENV);
        std::env::remove_var(HELPER_BACKEND_ENV);
        std::env::remove_var(HELPER_LIFECYCLE_ENV);
    }
    process_owner::supervise(detached)?;
    let confined = policy.enforce && (!crate::platform::yolo::enabled() || policy.mandatory);
    // A named sealed profile cannot be satisfied by a helper failure: refuse
    // unconfined exec up front so a broken backend is a hard stop, and YOLO
    // can never widen it (mandatory + this explicit guard).
    if !policy.sealed_reads.is_empty() && crate::platform::yolo::enabled() {
        return Err(std::io::Error::other(
            "YOLO cannot widen a sealed sandbox profile; refusing unconfined fallback",
        ));
    }
    #[cfg(target_os = "linux")]
    if !policy.sealed_reads.is_empty() {
        sealed::validate_policy(&policy).map_err(std::io::Error::other)?;
        sandbox_trace("bwrap-sealed");
        status::publish(&serde_json::json!({"sandbox_profile": "sealed"}));
        return bwrap::exec(&policy, program, args, detached);
    }
    #[cfg(not(target_os = "linux"))]
    if !policy.sealed_reads.is_empty() {
        return Err(std::io::Error::other("sealed requires Linux Bubblewrap"));
    }
    if confined && !matches!(backend.as_str(), "" | "landlock" | "bwrap") {
        return Err(std::io::Error::other(format!(
            "unknown sandbox backend {backend:?}"
        )));
    }
    #[cfg(target_os = "linux")]
    let fallback = if confined { detect() } else { None };
    #[cfg(target_os = "linux")]
    let mut receipt =
        serde_json::json!({"sandbox_profile": if confined { "landlock" } else { "unconfined" }});
    #[cfg(target_os = "linux")]
    if confined {
        let scan = hardlinks::scan(&policy.writable_roots)?;
        if let Some(cause) = fallback {
            receipt = hardlinks::privatize(&scan);
            receipt["sandbox_profile"] = serde_json::json!("landlock-only");
            receipt["cause"] = serde_json::json!(cause.class());
            receipt["notice"] = serde_json::json!(cause.notice());
            receipt["doctor"] = serde_json::json!(cause.remedy());
        } else if backend == "bwrap" || !scan.paths.is_empty() {
            sandbox_trace("bwrap");
            status::publish(&serde_json::json!({"sandbox_profile": "bwrap"}));
            return bwrap::exec(&policy, program, args, detached);
        } else {
            eprintln!("sandbox-hardlinks: {}", scan.stderr_line());
        }
    }
    sandbox_trace("landlock");
    #[cfg(target_os = "linux")]
    let restriction = match prepare(&policy) {
        Ok(prepared) => prepared.restrict(),
        // Preparation has not restricted this helper yet. Older kernels may
        // support filesystem Landlock but lack its network ABI. Use the same
        // policy in Bubblewrap; never retry after partial restriction, and
        // never fall back to an unconfined command.
        Err(error) if confined && fallback.is_none() => {
            sandbox_trace("bwrap-compatibility");
            status::publish(&serde_json::json!({
                "sandbox_profile": "bwrap",
                "cause": format!("Landlock policy unavailable: {error}"),
            }));
            return bwrap::exec(&policy, program, args, detached);
        }
        Err(error) => Err(error),
    };
    #[cfg(not(target_os = "linux"))]
    let restriction = apply(&policy);
    if let Err(error) = restriction {
        #[cfg(target_os = "linux")]
        {
            receipt.as_object_mut().unwrap().remove("notice");
            receipt["helper_phase"] = serde_json::json!("landlock");
            receipt["helper_exit"] = serde_json::json!(1);
            receipt["helper_error"] = serde_json::json!(format!(
                "Landlock unavailable: {error}; {}",
                compatibility::doctor()
            ));
            status::publish(&receipt);
        }
        return Err(std::io::Error::other(error));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(notice) = receipt["notice"].as_str() {
            eprintln!("{notice}");
            if receipt["aliases_unprotected"]
                .as_array()
                .is_some_and(|paths| !paths.is_empty())
            {
                eprintln!(
                    "sandbox: alias copy incomplete; remaining aliases_unprotected are recorded in the sandbox receipt"
                );
            }
        }
        status::publish(&receipt);
    }
    sandbox_trace("exec");
    Err(Command::new(program).args(args).exec())
}

/// Deny creation of Internet sockets while retaining AF_UNIX socketpairs used
/// by local compilers and test runners. This runs in the single-threaded helper
/// between its two execs after `no_new_privs` has been set by Landlock.
#[cfg(target_os = "linux")]
fn install_inet_socket_filter() -> Result<(), String> {
    const LD_W_ABS: u16 = (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16;
    const JMP_JEQ_K: u16 = (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16;
    const RET_K: u16 = (libc::BPF_RET | libc::BPF_K) as u16;
    const SECCOMP_DATA_NR_OFFSET: u32 = 0;
    const SECCOMP_DATA_ARG0_OFFSET: u32 = 16;

    let deny = libc::SECCOMP_RET_ERRNO | libc::EPERM as u32;
    let mut filter = [
        libc::sock_filter {
            code: LD_W_ABS,
            jt: 0,
            jf: 0,
            k: SECCOMP_DATA_NR_OFFSET,
        },
        libc::sock_filter {
            code: JMP_JEQ_K,
            jt: 0,
            jf: 4,
            k: libc::SYS_socket as u32,
        },
        libc::sock_filter {
            code: LD_W_ABS,
            jt: 0,
            jf: 0,
            k: SECCOMP_DATA_ARG0_OFFSET,
        },
        libc::sock_filter {
            code: JMP_JEQ_K,
            jt: 1,
            jf: 0,
            k: libc::AF_INET as u32,
        },
        libc::sock_filter {
            code: JMP_JEQ_K,
            jt: 0,
            jf: 1,
            k: libc::AF_INET6 as u32,
        },
        libc::sock_filter {
            code: RET_K,
            jt: 0,
            jf: 0,
            k: deny,
        },
        libc::sock_filter {
            code: RET_K,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ALLOW,
        },
    ];
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: `program` references the live fixed-size filter for the duration
    // of prctl; the kernel copies it before returning.
    let result = unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            &program as *const libc::sock_fprog,
        )
    };
    if result != 0 {
        return Err(format!(
            "seccomp network filter: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Build the SBPL (Seatbelt) profile mirroring the Linux Landlock posture:
/// whole filesystem readable, writes confined to the policy roots plus device
/// sinks, network optionally denied. SBPL is last-match-wins, so the broad
/// deny comes first and the allowances after; the network deny goes last so
/// nothing can shadow it.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn seatbelt_profile(policy: &SandboxPolicy) -> String {
    fn sb_quote(path: &std::path::Path) -> String {
        // Canonicalize so symlinked roots (/tmp and /var/tmp resolve under
        // /private on macOS) match the paths Seatbelt actually evaluates;
        // escape SBPL string metacharacters.
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        canon
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }
    let mut profile = String::from("(version 1)\n(allow default)\n(deny file-write*)\n");
    // Device sinks most tools expect to open for write even in a read-only
    // posture. None of them is persistent state.
    for sink in [
        "/dev/null",
        "/dev/zero",
        "/dev/tty",
        "/dev/stdout",
        "/dev/stderr",
        "/dev/dtracehelper",
    ] {
        profile.push_str(&format!("(allow file-write* (literal \"{sink}\"))\n"));
    }
    // Slave pty devices and fd re-opens (process substitution).
    profile.push_str("(allow file-write* (regex #\"^/dev/ttys[0-9]+$\"))\n");
    profile.push_str("(allow file-write* (subpath \"/dev/fd\"))\n");
    for root in &policy.writable_roots {
        profile.push_str(&format!(
            "(allow file-write* (subpath \"{}\"))\n",
            sb_quote(root)
        ));
    }
    if !policy.allow_network {
        profile.push_str("(deny network*)\n");
    }
    profile
}

/// Seatbelt profile prepared before restriction. All allocation and path
/// canonicalization happen while the helper is still unrestricted;
/// [`PreparedSandbox::restrict`] performs only the `sandbox_init` call.
#[cfg(target_os = "macos")]
pub struct PreparedSandbox {
    profile: Option<String>,
}

#[cfg(target_os = "macos")]
pub fn prepare(policy: &SandboxPolicy) -> Result<PreparedSandbox, String> {
    // Same override ladder as Linux: root YOLO lifts ordinary containment but
    // never a delegated mandatory (read-only/review) capability ceiling.
    if (crate::platform::yolo::enabled() && !policy.mandatory) || !policy.enforce {
        return Ok(PreparedSandbox { profile: None });
    }
    Ok(PreparedSandbox {
        profile: Some(seatbelt_profile(policy)),
    })
}

#[cfg(target_os = "macos")]
impl PreparedSandbox {
    /// Confine the current helper — and, because Seatbelt is inherited, every
    /// descendant — immediately before `exec`.
    pub fn restrict(self) -> Result<(), String> {
        let Some(profile) = self.profile else {
            return Ok(());
        };
        seatbelt_init(&profile)
    }
}

#[cfg(target_os = "macos")]
pub fn apply(policy: &SandboxPolicy) -> Result<(), String> {
    prepare(policy)?.restrict()
}

/// `sandbox_init(3)`. Apple marks it deprecated, but `sandbox-exec` itself is
/// a thin wrapper over exactly this call and it remains the only public way to
/// confine an already-running process before `exec`.
#[cfg(target_os = "macos")]
fn seatbelt_init(profile: &str) -> Result<(), String> {
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int};
    unsafe extern "C" {
        fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> c_int;
        fn sandbox_free_error(errorbuf: *mut c_char);
    }
    let profile =
        CString::new(profile).map_err(|error| format!("seatbelt profile contains NUL: {error}"))?;
    let mut error: *mut c_char = std::ptr::null_mut();
    // flags=0: `profile` is SBPL source text, not a named builtin profile.
    let rc = unsafe { sandbox_init(profile.as_ptr(), 0, &mut error) };
    if rc == 0 {
        return Ok(());
    }
    let detail = if error.is_null() {
        "unknown error".to_string()
    } else {
        let text = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        unsafe { sandbox_free_error(error) };
        text
    };
    Err(format!("seatbelt sandbox_init: {detail}"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[allow(dead_code)]
pub struct PreparedSandbox;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[allow(dead_code)]
pub fn prepare(policy: &SandboxPolicy) -> Result<PreparedSandbox, String> {
    if (crate::platform::yolo::enabled() && !policy.mandatory) || !policy.enforce {
        return Ok(PreparedSandbox);
    }
    Err("sandbox is only implemented on Linux and macOS".to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl PreparedSandbox {
    #[allow(dead_code)]
    pub fn restrict(self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn apply(policy: &SandboxPolicy) -> Result<(), String> {
    prepare(policy)?.restrict()
}

/// Probe whether the kernel supports landlock — does NOT restrict the caller
/// (only `restrict_self` does). Lets tests skip enforcement checks elsewhere.
#[allow(dead_code)] // diagnostic / test capability probe
#[cfg(target_os = "linux")]
pub fn available() -> bool {
    use landlock::{ABI, Access, AccessFs, Ruleset, RulesetAttr};
    Ruleset::default()
        .handle_access(AccessFs::from_all(ABI::V1))
        .and_then(|r| r.create())
        .is_ok()
}

#[allow(dead_code)] // diagnostic / test capability probe
#[cfg(target_os = "macos")]
pub fn available() -> bool {
    // Seatbelt ships with the OS; there is no kernel feature to probe.
    true
}

#[allow(dead_code)] // diagnostic / test capability probe
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn available() -> bool {
    false
}

/// The sandbox helper announces its hardlink scan on the child's stderr before
/// exec (`sandbox-hardlinks: {json}`); it is launcher protocol, not program
/// output, and never belongs in a receipt whose tail the model reads. Only a
/// leading line is removed: later matching text is the program's own.
pub(crate) fn strip_launcher_stderr(stderr: &str) -> String {
    if let Some(rest) = stderr.strip_prefix("sandbox-hardlinks: {") {
        if let Some(end) = rest.find('\n') {
            return rest[end + 1..].to_string();
        }
        return String::new();
    }
    stderr.to_string()
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/sandbox__seatbelt_profile_tests.rs"]
mod seatbelt_profile_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/sandbox__helper_path_tests.rs"]
mod helper_path_tests;
