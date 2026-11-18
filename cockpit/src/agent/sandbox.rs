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
    pub writable_roots: Vec<PathBuf>,
    pub allow_network: bool,
    pub enforce: bool,
    #[serde(default)]
    pub mandatory: bool,
    #[serde(default)]
    pub sealed_reads: Vec<PathBuf>,
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

pub fn enabled() -> bool {
    configured_enabled() && !crate::platform::yolo::enabled()
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use ring::digest;
    let out = digest::digest(&digest::SHA256, bytes);
    let mut hex = String::with_capacity(64);
    for byte in out.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

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
    pub fn permissive() -> Self {
        let mut writable_roots = vec![
            PathBuf::from("/tmp"),
            PathBuf::from("/var/tmp"),
            PathBuf::from("/dev/shm"),
            PathBuf::from("/proc/self/task"),
        ];
        for key in ["XDG_RUNTIME_DIR", "XDG_CACHE_HOME", "TMPDIR"] {
            if let Some(path) = std::env::var_os(key) {
                writable_roots.push(PathBuf::from(path));
            }
        }
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
        writable_roots.extend(darwin_user_dirs());
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let private_task_cache = std::env::var_os("ANGEL_TASK_CACHE_HOME")
                .is_some_and(|value| !value.is_empty());
            if !private_task_cache {
                writable_roots.push(home.join(".cache"));
            }
            writable_roots.extend([
                home.join(".cargo/registry"),
                home.join(".cargo/git"),
                home.join(".npm/_cacache"),
                home.join("Library/Caches"),
            ]);
            writable_roots.extend(
                cargo_install_metadata_paths(&home)
                    .into_iter()
                    .filter(|path| path.exists()),
            );
            for dir in [".local/bin", ".local/lib", ".cargo/bin"] {
                let path = home.join(dir);
                let _ = std::fs::create_dir_all(&path);
                writable_roots.push(path);
            }
        }
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
            enforce: configured_enabled(),
            mandatory: false,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        }
    }

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

#[cfg(target_os = "linux")]
pub struct PreparedSandbox {
    ruleset: Option<landlock::RulesetCreated>,
    allow_network: bool,
}

#[cfg(target_os = "linux")]
#[allow(deprecated)]
pub fn prepare(policy: &SandboxPolicy) -> Result<PreparedSandbox, String> {
    use landlock::{
        ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, Ruleset, RulesetAttr,
        RulesetCreatedAttr,
    };

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
            if policy.sealed_reads.is_empty() {
                vec![PathBuf::from("/")]
            } else {
                policy.sealed_reads.clone()
            },
            access_ro,
        ))
        .map_err(|e| format!("landlock read rule: {e}"))?
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
const HELPER_NAMESPACES_ENV: &str = "ANGEL_INTERNAL_SANDBOX_NAMESPACES";

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn set_detached_lifecycle(command: &mut Command) {
    command.env(HELPER_LIFECYCLE_ENV, "detached");
}

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

pub fn prime_helper() {
    let _ = helper_executable();
}

fn sanitize_deleted_exe_path(path: PathBuf) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    match text.strip_suffix(" (deleted)") {
        Some(clean) => PathBuf::from(clean),
        None => path,
    }
}

pub fn set_helper_policy(command: &mut Command, policy: &SandboxPolicy) -> Result<(), String> {
    let overridden = sealed::override_for(policy);
    let mut policy = overridden.as_ref().unwrap_or(policy).clone();
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
    #[cfg(target_os = "linux")]
    if compatibility::detect().is_none() {
        command.env(HELPER_NAMESPACES_ENV, "verified");
    }
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
            let helper = profile_dir.join(format!("angel-sandbox{}", std::env::consts::EXE_SUFFIX));
            if helper.is_file() {
                return Ok(helper);
            }
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
    #[cfg(target_os = "linux")]
    let namespaces_verified = std::env::var(HELPER_NAMESPACES_ENV).as_deref() == Ok("verified");
    unsafe {
        std::env::remove_var(HELPER_POLICY_ENV);
        std::env::remove_var(HELPER_BACKEND_ENV);
        std::env::remove_var(HELPER_LIFECYCLE_ENV);
        std::env::remove_var(HELPER_NAMESPACES_ENV);
    }
    process_owner::supervise(detached)?;
    let confined = policy.enforce && (!crate::platform::yolo::enabled() || policy.mandatory);
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
    let fallback = if confined && !namespaces_verified {
        detect()
    } else {
        None
    };
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

#[cfg(any(target_os = "macos", test))]
pub(crate) fn seatbelt_profile(policy: &SandboxPolicy) -> String {
    fn sb_quote(path: &std::path::Path) -> String {
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        canon
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }
    let mut profile = String::from("(version 1)\n(allow default)\n(deny file-write*)\n");
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

#[cfg(target_os = "macos")]
pub struct PreparedSandbox {
    profile: Option<String>,
}

#[cfg(target_os = "macos")]
pub fn prepare(policy: &SandboxPolicy) -> Result<PreparedSandbox, String> {
    if (crate::platform::yolo::enabled() && !policy.mandatory) || !policy.enforce {
        return Ok(PreparedSandbox { profile: None });
    }
    Ok(PreparedSandbox {
        profile: Some(seatbelt_profile(policy)),
    })
}

#[cfg(target_os = "macos")]
impl PreparedSandbox {
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

#[cfg(target_os = "linux")]
pub fn available() -> bool {
    use landlock::{ABI, Access, AccessFs, Ruleset, RulesetAttr};
    Ruleset::default()
        .handle_access(AccessFs::from_all(ABI::V1))
        .and_then(|r| r.create())
        .is_ok()
}

#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub fn available() -> bool {
    true
}

#[allow(dead_code)]
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn available() -> bool {
    false
}

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
