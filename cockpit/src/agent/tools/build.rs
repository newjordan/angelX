//! Build / verify tools: `cargo`, `run_tests`, `lint` (clippy), `check`
//! (cargo check), and `fmt`. The test/lint reward parsers (`parse_test_result`,
//! `parse_lint`) and their `TestOutcome`/`LintOutcome` types live here too and
//! are re-exported from `harness` for the reinforce loop. All commands run under
//! the landlock sandbox.

mod cargo_controls;
#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/build__fallback_tests.rs"]
mod fallback_tests;
mod targets;
mod verifier;
pub(crate) use targets::MutationTargets;
pub(crate) use verifier::PinnedNativeRuntimes;

use crate::agent::club::ToolDef;
use crate::agent::harness::{
    Tool, ToolOutputProgress, output_timed_extensible_cancellable_with_progress, tool_timeout,
};
use crate::agent::sandbox::{self, SandboxPolicy};
use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

/// One early-bound Rust toolchain shared by every typed Cargo-backed tool in a
/// registry. Resolution deliberately never searches the task's `PATH`: a
/// writable workspace can put a `cargo` shim first there. Rustup is consulted
/// once, before the model can mutate the workspace, solely to preserve a
/// repository's `rust-toolchain` selection. Every later dispatch executes the
/// canonical toolchain Cargo directly and supplies a sealed, host-only PATH.
#[derive(Clone)]
pub(crate) struct PinnedCargo {
    inner: Arc<Result<PinnedCargoExecutable, String>>,
}

/// Hash independent toolchains concurrently, but publish neither until both
/// captures finish. Every executable remains eagerly pinned before model code
/// can run; avoiding serial image reads keeps registry startup responsive.
pub(crate) fn capture_verifier_runtimes(workspace: &Path) -> (PinnedCargo, PinnedNativeRuntimes) {
    std::thread::scope(|scope| {
        let native = std::thread::Builder::new()
            .name("pin-native-verifiers".into())
            .spawn_scoped(scope, || PinnedNativeRuntimes::capture(workspace));
        let cargo = PinnedCargo::capture(workspace);
        let native = match native {
            Ok(worker) => worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
            // Thread pressure must not disable verification or defer pinning.
            Err(_) => PinnedNativeRuntimes::capture(workspace),
        };
        (cargo, native)
    })
}

struct PinnedCargoExecutable {
    cargo: PinnedExecutable,
    rustc: PinnedExecutable,
    rustdoc: Option<PinnedExecutable>,
    cargo_clippy: Option<PinnedExecutable>,
    clippy_driver: Option<PinnedExecutable>,
    cargo_fmt: Option<PinnedExecutable>,
    rustfmt: Option<PinnedExecutable>,
    trusted_path: OsString,
    private_home_parent: PathBuf,
    cache_source: Option<PathBuf>,
    controls: cargo_controls::CargoControls,
}

/// Serializes toolchain captures against tests that poison Cargo/rustup env.
/// See [`PinnedCargo::capture`]; tests holding this during an env swap must
/// never call `capture` themselves while holding it (non-reentrant).
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn capture_lock() -> std::sync::MutexGuard<'static, ()> {
    CAPTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct PinnedExecutable {
    path: PathBuf,
    captured: PinnedFileIdentity,
    /// Established at registry construction and checked on every dispatch.
    /// Filesystems are allowed to coalesce consecutive same-length in-place
    /// writes onto identical inode/mtime/ctime values, so metadata alone cannot
    /// safely defer this digest until the first tool call.
    sha256: OnceLock<String>,
}

const MAX_PINNED_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PinnedFileIdentity {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

#[cfg(not(unix))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct PinnedFileIdentity {
    len: u64,
    modified: Option<SystemTime>,
    readonly: bool,
}

impl PinnedCargo {
    /// Capture once per registry, before any model-owned tool dispatch.
    pub(crate) fn capture(workspace: &Path) -> Self {
        // Serialize against env-poisoning tests: capture reads process-global
        // Cargo/rustup env (CARGO_HOME, CARGO, PATH fallbacks), and a test that
        // swaps those vars mid-window must hold [`capture_lock`] while the swap
        // is live — otherwise a concurrent capture resolves (and executes) its
        // replaced resolver shim, the "captured registry reused a replaced
        // rustup proxy" flake. Production captures are rare and fast.
        let _guard = CAPTURE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self {
            inner: Arc::new(capture_pinned_cargo(workspace)),
        }
    }

    fn executable(&self) -> Result<&PinnedCargoExecutable, String> {
        self.inner
            .as_ref()
            .as_ref()
            .map_err(|error| format!("trusted Cargo unavailable: {error}"))
    }

    /// Revalidate the named executable immediately before dispatch. The path
    /// is outside every task-writable sandbox root; the identity check also
    /// makes an operator-side replacement fail closed instead of silently
    /// changing what a typed verifier means during a live session.
    fn verified_executable(&self) -> Result<&PinnedCargoExecutable, String> {
        let executable = self.executable()?;
        executable.controls.revalidate()?;
        revalidate_executable(&executable.cargo, "Cargo")?;
        revalidate_executable(&executable.rustc, "rustc")?;
        if let Some(rustdoc) = &executable.rustdoc {
            revalidate_executable(rustdoc, "rustdoc")?;
        }
        for (tool, label) in [
            (executable.cargo_clippy.as_ref(), "cargo-clippy"),
            (executable.clippy_driver.as_ref(), "clippy-driver"),
            (executable.cargo_fmt.as_ref(), "cargo-fmt"),
            (executable.rustfmt.as_ref(), "rustfmt"),
        ] {
            if let Some(tool) = tool {
                revalidate_executable(tool, label)?;
            }
        }
        // Re-land every canonical path after the complete toolchain has been
        // hashed. Without this aggregate pass, a replacement could race while
        // a later (large) binary was still being read.
        require_landed_toolchain(executable)?;
        Ok(executable)
    }

    fn configure_command(
        &self,
        command: &mut Command,
        executable: &PinnedCargoExecutable,
    ) -> Result<(), String> {
        command.env("PATH", &executable.trusted_path);
        command.env("CARGO", &executable.cargo.path);
        command.env("RUSTC", &executable.rustc.path);
        command.env("CARGO_BUILD_RUSTC", &executable.rustc.path);
        if let Some(rustdoc) = &executable.rustdoc {
            command.env("RUSTDOC", &rustdoc.path);
            command.env("CARGO_BUILD_RUSTDOC", &rustdoc.path);
        } else {
            command.env_remove("RUSTDOC");
            command.env_remove("CARGO_BUILD_RUSTDOC");
        }
        for name in [
            "RUSTFLAGS",
            "RUSTDOCFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_ENCODED_RUSTDOCFLAGS",
            "RUSTC_BOOTSTRAP",
            "RUSTUP_TOOLCHAIN",
            "CC",
            "CXX",
            "AR",
            "RANLIB",
            "LD",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        ] {
            command.env_remove(name);
        }
        for (name, _) in std::env::vars_os() {
            let text = name.to_string_lossy();
            if text.starts_with("CARGO_TARGET_")
                || (text.starts_with("CARGO_BUILD_")
                    && text != "CARGO_BUILD_RUSTC"
                    && text != "CARGO_BUILD_RUSTDOC")
                || text.starts_with("HOST_CC")
                || text.starts_with("TARGET_CC")
                || text.starts_with("CC_")
                || text.starts_with("CXX_")
                || text.starts_with("AR_")
            {
                command.env_remove(name);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn for_test_executable(path: PathBuf) -> Self {
        let private_home_parent = path
            .parent()
            .unwrap_or_else(|| Path::new("/tmp"))
            .join(".angel-cargo-homes");
        Self {
            inner: Arc::new(capture_direct_cargo(path, Path::new("/nonexistent")).map(
                |mut executable| {
                    // Synthetic toolchains live under their scratch fixture;
                    // keep their equally synthetic Cargo home there instead
                    // of mutating the operator's real ~/.rustup in tests.
                    executable.private_home_parent = private_home_parent;
                    executable.cache_source = None;
                    executable
                },
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_workspace(path: PathBuf, workspace: &Path) -> Self {
        let mut pin = Self::for_test_executable(path);
        if let Ok(executable) = Arc::get_mut(&mut pin.inner).expect("new pin") {
            match cargo_controls::CargoControls::capture(workspace, workspace) {
                Ok(controls) => executable.controls = controls,
                Err(error) => {
                    return Self {
                        inner: Arc::new(Err(error)),
                    };
                }
            }
        }
        pin
    }

    #[cfg(test)]
    pub(crate) fn executable_path(&self) -> Result<PathBuf, String> {
        self.executable()
            .map(|executable| executable.cargo.path.clone())
    }

    #[cfg(test)]
    pub(crate) fn verify_for_test(&self) -> Result<(), String> {
        self.verified_executable().map(|_| ())
    }

    /// Ledger-visible identity of what verified the task: the pinned cargo
    /// path plus the sha256 captured at registry construction. `None` when
    /// no pin could be established (the verifier then refuses elsewhere).
    pub(crate) fn pin_note(&self) -> Option<String> {
        let executable = self.executable().ok()?;
        let sha256 = executable.cargo.sha256.get()?;
        let short = &sha256[..sha256.len().min(16)];
        Some(format!(
            "[verifier toolchain pin: cargo {} (sha256 {short})]\npin: {}",
            executable.cargo.path.display(),
            serde_json::json!({
                "cargo": {"path": executable.cargo.path, "sha256": sha256},
                "rustc": {"path": executable.rustc.path, "sha256": executable.rustc.sha256.get()},
                "control_files": executable.controls.receipt(),
            })
        ))
    }
}

struct EphemeralCargoHome {
    path: PathBuf,
    target: PathBuf,
}

impl Drop for EphemeralCargoHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn ephemeral_cargo_home(executable: &PinnedCargoExecutable) -> Result<EphemeralCargoHome, String> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(&executable.private_home_parent).map_err(|error| {
        format!(
            "create private Cargo-home parent {}: {error}",
            executable.private_home_parent.display()
        )
    })?;
    #[cfg(unix)]
    {
        let mut permissions = std::fs::metadata(&executable.private_home_parent)
            .map_err(|error| format!("inspect private Cargo-home parent: {error}"))?
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&executable.private_home_parent, permissions)
            .map_err(|error| format!("secure private Cargo-home parent: {error}"))?;
    }
    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let path = executable.private_home_parent.join(format!(
        "{}-{nonce}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path)
        .map_err(|error| format!("create private Cargo home {}: {error}", path.display()))?;
    let target = path.join("target");
    std::fs::create_dir(&target)
        .map_err(|error| format!("create private Cargo target {}: {error}", target.display()))?;
    #[cfg(unix)]
    {
        let mut permissions = std::fs::metadata(&path)
            .map_err(|error| format!("inspect private Cargo home: {error}"))?
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions)
            .map_err(|error| format!("secure private Cargo home: {error}"))?;
        if let Some(source) = &executable.cache_source {
            for directory in ["registry", "git"] {
                let source = source.join(directory);
                if source.is_dir() {
                    std::os::unix::fs::symlink(&source, path.join(directory)).map_err(|error| {
                        format!("link Cargo {directory} cache {}: {error}", source.display())
                    })?;
                }
            }
            for file in ["credentials", "credentials.toml"] {
                let source = source.join(file);
                if source.is_file() {
                    std::os::unix::fs::symlink(&source, path.join(file)).map_err(|error| {
                        format!("link Cargo {file} {}: {error}", source.display())
                    })?;
                }
            }
        }
    }
    Ok(EphemeralCargoHome { path, target })
}

fn revalidate_executable(executable: &PinnedExecutable, label: &str) -> Result<(), String> {
    let mut file = std::fs::File::open(&executable.path).map_err(|error| {
        format!(
            "pinned {label} disappeared at {}: {error}; restart Angel to re-pin the toolchain",
            executable.path.display()
        )
    })?;
    let before = file.metadata().map_err(|error| {
        format!(
            "inspect open pinned {label} {}: {error}; restart Angel to re-pin the toolchain",
            executable.path.display()
        )
    })?;
    if before.len() > MAX_PINNED_EXECUTABLE_BYTES {
        return Err(format!(
            "pinned {label} {} exceeds the {}-byte verifier identity limit",
            executable.path.display(),
            MAX_PINNED_EXECUTABLE_BYTES
        ));
    }
    require_captured_identity(executable, &before, label)?;
    let observed = crate::agent::harness::sha256_reader_hex(&mut file).map_err(|error| {
        format!(
            "read pinned {label} {}: {error}; restart Angel to re-pin the toolchain",
            executable.path.display()
        )
    })?;
    let after = file.metadata().map_err(|error| {
        format!(
            "reinspect open pinned {label} {}: {error}; restart Angel to re-pin the toolchain",
            executable.path.display()
        )
    })?;
    require_captured_identity(executable, &after, label)?;
    let landed = std::fs::symlink_metadata(&executable.path).map_err(|error| {
        format!(
            "reinspect pinned {label} path {}: {error}; restart Angel to re-pin the toolchain",
            executable.path.display()
        )
    })?;
    require_captured_identity(executable, &landed, label)?;
    let pinned = executable.sha256.get_or_init(|| observed.clone());
    if observed != *pinned {
        return Err(format!(
            "pinned {label} changed at {} (captured sha256 {}); toolchain changed since pin; restart Angel to re-pin the toolchain",
            executable.path.display(),
            pinned
        ));
    }
    Ok(())
}

fn require_landed_toolchain(executable: &PinnedCargoExecutable) -> Result<(), String> {
    require_landed_identity(&executable.cargo, "Cargo")?;
    require_landed_identity(&executable.rustc, "rustc")?;
    if let Some(tool) = &executable.rustdoc {
        require_landed_identity(tool, "rustdoc")?;
    }
    for (tool, label) in [
        (executable.cargo_clippy.as_ref(), "cargo-clippy"),
        (executable.clippy_driver.as_ref(), "clippy-driver"),
        (executable.cargo_fmt.as_ref(), "cargo-fmt"),
        (executable.rustfmt.as_ref(), "rustfmt"),
    ] {
        if let Some(tool) = tool {
            require_landed_identity(tool, label)?;
        }
    }
    Ok(())
}

fn require_landed_identity(executable: &PinnedExecutable, label: &str) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(&executable.path).map_err(|error| {
        format!(
            "reinspect pinned {label} path {}: {error}; restart Angel to re-pin the toolchain",
            executable.path.display()
        )
    })?;
    require_captured_identity(executable, &metadata, label)
}

fn require_captured_identity(
    executable: &PinnedExecutable,
    metadata: &std::fs::Metadata,
    label: &str,
) -> Result<(), String> {
    let observed = pinned_file_identity(metadata);
    if observed != executable.captured {
        return Err(format!(
            "pinned {label} identity changed at {} after registry construction; restart Angel to re-pin the toolchain",
            executable.path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn pinned_file_identity(metadata: &std::fs::Metadata) -> PinnedFileIdentity {
    PinnedFileIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        len: metadata.len(),
        mtime: metadata.mtime(),
        mtime_nsec: metadata.mtime_nsec(),
        ctime: metadata.ctime(),
        ctime_nsec: metadata.ctime_nsec(),
    }
}

#[cfg(not(unix))]
fn pinned_file_identity(metadata: &std::fs::Metadata) -> PinnedFileIdentity {
    PinnedFileIdentity {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: metadata.permissions().readonly(),
    }
}

fn canonical_existing_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| format!("resolve {label} {}: {error}", path.display()))?;
    let metadata = std::fs::metadata(&canonical)
        .map_err(|error| format!("inspect {label} {}: {error}", canonical.display()))?;
    if !metadata.is_file() {
        return Err(format!("{label} is not a file: {}", canonical.display()));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "{label} is not executable: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn resolution_dir(workspace: &Path) -> PathBuf {
    workspace
        .ancestors()
        .find(|candidate| candidate.is_dir())
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn path_within(path: &Path, root: &Path) -> bool {
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    path == root || path.starts_with(root)
}

fn task_writable_roots(workspace: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        workspace.to_path_buf(),
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/dev/shm"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.extend([
            home.join(".cache"),
            home.join(".cargo/registry"),
            home.join(".cargo/git"),
            home.join(".cargo/bin"),
            home.join(".local/bin"),
            home.join(".local/lib"),
        ]);
    }
    for key in ["XDG_RUNTIME_DIR", "XDG_CACHE_HOME"] {
        if let Some(path) = std::env::var_os(key) {
            roots.push(PathBuf::from(path));
        }
    }
    roots
}

fn validate_pinned_path(path: PathBuf, workspace: &Path) -> Result<PathBuf, String> {
    let path = canonical_existing_file(&path, "Cargo executable")?;
    if path.file_name() != Some(OsStr::new("cargo")) {
        return Err(format!(
            "resolved Cargo target is not a direct toolchain cargo binary: {}",
            path.display()
        ));
    }
    if let Some(root) = task_writable_roots(workspace)
        .into_iter()
        .find(|root| path_within(&path, root))
    {
        return Err(format!(
            "resolved Cargo {} is inside task-writable root {}; refusing typed verifier identity",
            path.display(),
            root.display()
        ));
    }
    Ok(path)
}

fn rustup_resolvers(workspace: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(cargo) = std::env::var_os("CARGO").map(PathBuf::from)
        && cargo.is_absolute()
        && let Ok(canonical) = std::fs::canonicalize(&cargo)
        && canonical.file_name() == Some(OsStr::new("rustup"))
        && !path_within(&canonical, workspace)
    {
        candidates.push(canonical);
    }
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    if let Some(home) = cargo_home {
        candidates.push(home.join("bin/rustup"));
    }
    candidates.extend(
        ["/usr/bin/rustup", "/usr/local/bin/rustup"]
            .into_iter()
            .map(PathBuf::from),
    );
    candidates
}

fn direct_cargo_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for raw in [
        std::env::var_os("CARGO"),
        option_env!("CARGO").map(OsString::from),
    ]
    .into_iter()
    .flatten()
    {
        let path = PathBuf::from(raw);
        if path.is_absolute() {
            candidates.push(path);
        }
    }
    candidates.extend(
        [
            "/usr/bin/cargo",
            "/usr/local/bin/cargo",
            "/opt/homebrew/bin/cargo",
            "/bin/cargo",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    candidates
}

const RUSTUP_RESOLVE_TIMEOUT: Duration = Duration::from_secs(3);

/// Ask one already-pinned rustup resolver for Cargo without allowing registry
/// construction to inherit an unbounded subprocess. This runs before any
/// ordinary tool executor exists and while the capture lock is held, so its
/// deadline must be owned here rather than delegated to the later tool timer.
pub(crate) fn bounded_rustup_cargo_path(command: Command, timeout: Duration) -> Option<PathBuf> {
    let out = crate::platform::workspace_store::capture_bounded_command(command, timeout)?;
    let path = PathBuf::from(String::from_utf8_lossy(&out).trim());
    (!path.as_os_str().is_empty()).then_some(path)
}

fn capture_pinned_cargo(workspace: &Path) -> Result<PinnedCargoExecutable, String> {
    let effective_workspace =
        cargo_workspace_root(workspace).unwrap_or_else(|| workspace.to_path_buf());
    let cwd = resolution_dir(&effective_workspace);
    let controls = cargo_controls::CargoControls::capture(&cwd, workspace)?;
    if let Some(raw) = std::env::var_os("ANGEL_CARGO_BIN") {
        let path = PathBuf::from(raw);
        if !path.is_absolute() {
            return Err("ANGEL_CARGO_BIN must be an absolute direct toolchain cargo path".into());
        }
        if !path.is_file() {
            return Err(crate::agent::tools::runtime_missing::RuntimeMissing::new(
                "cargo",
                format!("ANGEL_CARGO_BIN={}", path.display()),
            )
            .encode());
        }
        return validate_pinned_path(path, workspace)
            .and_then(|path| capture_direct_cargo(path, workspace))
            .and_then(|executable| controls.attach(executable));
    }
    let mut failures = Vec::new();
    let resolver_deadline = Instant::now() + RUSTUP_RESOLVE_TIMEOUT;
    let mut seen_resolvers = Vec::new();
    for resolver in rustup_resolvers(workspace) {
        let Ok(resolver) = canonical_existing_file(&resolver, "rustup resolver") else {
            continue;
        };
        if seen_resolvers.iter().any(|seen| seen == &resolver) {
            continue;
        }
        seen_resolvers.push(resolver.clone());
        let remaining = resolver_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            failures.push(format!(
                "rustup resolver budget expired after {}s",
                RUSTUP_RESOLVE_TIMEOUT.as_secs()
            ));
            break;
        }
        let mut command = Command::new(&resolver);
        command
            .args(["which", "cargo"])
            .current_dir(&cwd)
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("RUSTUP_AUTO_INSTALL", "0");
        match bounded_rustup_cargo_path(command, remaining) {
            Some(candidate) => {
                match validate_pinned_path(candidate, workspace)
                    .and_then(|path| capture_direct_cargo(path, workspace))
                {
                    Ok(executable) => return controls.attach(executable),
                    Err(error) => failures.push(error),
                }
            }
            None => failures.push(format!(
                "{} did not return a bounded Cargo path for {} within {}s",
                resolver.display(),
                workspace.display(),
                RUSTUP_RESOLVE_TIMEOUT.as_secs()
            )),
        }
    }
    if controls.has_toolchain() {
        return Err(format!(
            "{}; effective toolchain resolution failed: {}",
            if failures.is_empty()
                || failures
                    .iter()
                    .all(|reason| reason.contains("did not return a bounded Cargo path"))
            {
                crate::agent::tools::runtime_missing::RuntimeMissing::new(
                    "cargo", "ANGEL_CARGO_BIN unset; rustup in CARGO_HOME/HOME and system directories; inherited rust-toolchain selection (task PATH is not trusted)",
                ).encode()
            } else {
                "Cargo toolchain resolution rejected".into()
            },
            failures.join(" | ")
        ));
    }
    for candidate in direct_cargo_candidates() {
        match validate_pinned_path(candidate, workspace)
            .and_then(|path| capture_direct_cargo(path, workspace))
        {
            Ok(executable) => return controls.attach(executable),
            Err(error) => failures.push(error),
        }
    }
    // Every resolver's failure rides the error: reporting only the last one
    // (`/bin/cargo: No such file`) hid why the rustup proxy itself was
    // rejected, and the model spent 24 hops inventorying toolchains it could
    // not see (arena rust-capped-backoff, 2026-09-06). The fallback hint is
    // the escape hatch the model would otherwise have to discover.
    let detail = failures
        .iter()
        .take(8)
        .map(|error| error.chars().take(140).collect::<String>())
        .collect::<Vec<_>>();
    Err(format!(
        "{}; no direct Cargo executable could be pinned for {}{}. Fallback: run `cargo test` \
         through the `shell` tool (the toolchain on PATH is not affected by this pin)",
        crate::agent::tools::runtime_missing::RuntimeMissing::new(
            "cargo", "ANGEL_CARGO_BIN unset; CARGO, rustup and trusted installation paths (task PATH is not trusted)",
        ).encode(),
        workspace.display(),
        if detail.is_empty() {
            String::new()
        } else {
            format!("; resolver errors: {}", detail.join(" | "))
        }
    ))
}

fn capture_direct_cargo(path: PathBuf, workspace: &Path) -> Result<PinnedCargoExecutable, String> {
    let path = if cfg!(test) && workspace == Path::new("/nonexistent") {
        canonical_existing_file(&path, "test Cargo executable")?
    } else {
        validate_pinned_path(path, workspace)?
    };
    let bin = path
        .parent()
        .ok_or_else(|| format!("Cargo has no toolchain directory: {}", path.display()))?;
    let rustc = capture_toolchain_executable(bin.join("rustc"), "toolchain rustc", workspace)?;
    let rustdoc =
        capture_toolchain_executable(bin.join("rustdoc"), "toolchain rustdoc", workspace).ok();
    let cargo_clippy = capture_toolchain_executable(
        bin.join("cargo-clippy"),
        "toolchain cargo-clippy",
        workspace,
    )
    .ok();
    let clippy_driver = capture_toolchain_executable(
        bin.join("clippy-driver"),
        "toolchain clippy-driver",
        workspace,
    )
    .ok();
    let cargo_fmt =
        capture_toolchain_executable(bin.join("cargo-fmt"), "toolchain cargo-fmt", workspace).ok();
    let rustfmt =
        capture_toolchain_executable(bin.join("rustfmt"), "toolchain rustfmt", workspace).ok();
    let mut trusted_dirs = vec![bin.to_path_buf()];
    for path in ["/usr/local/bin", "/usr/bin", "/bin"] {
        let path = PathBuf::from(path);
        if path.is_dir() && !trusted_dirs.iter().any(|existing| existing == &path) {
            trusted_dirs.push(path);
        }
    }
    let trusted_path = std::env::join_paths(trusted_dirs)
        .map_err(|error| format!("construct trusted Cargo PATH: {error}"))?;
    // The private Cargo homes live under the rustup home: `RUSTUP_HOME` when
    // the operator relocated it (a redirected HOME must not drag the verifier
    // into a task-writable tree), `~/.rustup` otherwise.
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is unavailable; cannot create an isolated Cargo home".to_string())?;
    let rustup_home = match std::env::var_os("RUSTUP_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home.join(".rustup"),
    };
    let rustup_home = std::fs::canonicalize(&rustup_home).unwrap_or(rustup_home);
    let private_home_parent = rustup_home.join(".angel-cargo-homes");
    if let Some(root) = task_writable_roots(workspace)
        .into_iter()
        .find(|root| path_within(&private_home_parent, root))
    {
        return Err(format!(
            "private Cargo-home parent {} is inside task-writable root {}",
            private_home_parent.display(),
            root.display()
        ));
    }
    let cache_source = Some(home.join(".cargo")).filter(|path| path.is_dir());
    Ok(PinnedCargoExecutable {
        cargo: capture_executable(path, "pinned Cargo")?,
        rustc,
        rustdoc,
        cargo_clippy,
        clippy_driver,
        cargo_fmt,
        rustfmt,
        trusted_path,
        private_home_parent,
        cache_source,
        controls: cargo_controls::CargoControls::default(),
    })
}

fn capture_executable(path: PathBuf, label: &str) -> Result<PinnedExecutable, String> {
    let path = canonical_existing_file(&path, label)?;
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    Ok(PinnedExecutable {
        sha256: initial_executable_digest(&path, label)?,
        path,
        captured: pinned_file_identity(&metadata),
    })
}

fn initial_executable_digest(path: &Path, label: &str) -> Result<OnceLock<String>, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("open {label} {} for hashing: {error}", path.display()))?;
    let hash = crate::agent::harness::sha256_reader_hex(&mut file)
        .map_err(|error| format!("hash {label} {}: {error}", path.display()))?;
    let digest = OnceLock::new();
    let _ = digest.set(hash);
    Ok(digest)
}

fn capture_toolchain_executable(
    path: PathBuf,
    label: &str,
    workspace: &Path,
) -> Result<PinnedExecutable, String> {
    let path = canonical_existing_file(&path, label)?;
    if !(cfg!(test) && workspace == Path::new("/nonexistent"))
        && let Some(root) = task_writable_roots(workspace)
            .into_iter()
            .find(|root| path_within(&path, root))
    {
        return Err(format!(
            "resolved {label} {} is inside task-writable root {}; refusing typed verifier identity",
            path.display(),
            root.display()
        ));
    }
    capture_executable(path, label)
}

/// Bounded raw output for verifier-style commands. Unlike the ordinary shell
/// receipt, callers need the terminal cargo/libtest summary to derive a reward,
/// so this keeps the capped head+tail supplied by `output_timed` rather than
/// flattening it to a short display snippet.
/// The sandbox helper announces its hardlink scan on the child's stderr before
/// exec (`sandbox-hardlinks: {json}`); it is launcher protocol, not program
/// output, and never belongs in a receipt whose tail the model reads. Only a
/// leading line is removed: later matching text is the program's own.
fn strip_launcher_stderr(stderr: &str) -> String {
    if let Some(rest) = stderr.strip_prefix("sandbox-hardlinks: {")
        && let Some(end) = rest.find('\n')
    {
        return rest[end + 1..].to_string();
    }
    stderr.to_string()
}

struct CapturedCommand {
    checked_sources: Option<std::collections::BTreeSet<PathBuf>>,
    stdout: String,
    stderr: String,
    success: bool,
    exit: Option<i32>,
    timed_out: bool,
    cancelled: bool,
    dur_ms: u128,
    /// Kill-time hang evidence from the shared capture path, present only when
    /// the command timed out (what state the child was in, how long it had been
    /// silent, whether a SIGTERM grace was honored).
    timeout_diag: Option<crate::agent::harness::TimeoutDiagnostics>,
}

const MAX_DIRECT_ARG_BYTES: usize = 64 * 1024;
const MAX_DIRECT_ARGS: usize = 256;

/// Split a direct-process argument string without invoking a shell. Quoting and
/// backslash escaping preserve argv boundaries, but no variables, globs,
/// substitutions, or operators are interpreted.
pub(crate) fn parse_direct_argv(raw: &str) -> Result<Vec<String>, String> {
    if raw.len() > MAX_DIRECT_ARG_BYTES {
        return Err(format!(
            "argument string exceeds the {MAX_DIRECT_ARG_BYTES}-byte limit"
        ));
    }
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote = None;
    let mut escaped = false;

    let push = |args: &mut Vec<String>, current: &mut String| -> Result<(), String> {
        if args.len() >= MAX_DIRECT_ARGS {
            return Err(format!("more than {MAX_DIRECT_ARGS} arguments"));
        }
        args.push(std::mem::take(current));
        Ok(())
    };

    for ch in raw.chars() {
        if escaped {
            current.push(ch);
            in_token = true;
            escaped = false;
            continue;
        }
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            Some('"') => match ch {
                '"' => quote = None,
                '\\' => escaped = true,
                _ => current.push(ch),
            },
            Some(_) => unreachable!("only single and double quote states exist"),
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    in_token = true;
                }
                '\\' => {
                    escaped = true;
                    in_token = true;
                }
                ch if ch.is_whitespace() => {
                    if in_token {
                        push(&mut args, &mut current)?;
                        in_token = false;
                    }
                }
                _ => {
                    current.push(ch);
                    in_token = true;
                }
            },
        }
    }
    if escaped {
        return Err("trailing backslash escape".to_string());
    }
    if let Some(quote) = quote {
        let kind = if quote == '\'' { "single" } else { "double" };
        return Err(format!("unterminated {kind} quote"));
    }
    if in_token {
        push(&mut args, &mut current)?;
    }
    Ok(args)
}

fn cargo_argv(subcommand: &str, extra: &str) -> Result<Vec<String>, String> {
    let mut argv = vec![subcommand.to_string()];
    argv.extend(
        parse_direct_argv(extra)
            .map_err(|error| format!("invalid cargo {subcommand} arguments: {error}"))?,
    );
    Ok(argv)
}

fn cargo_args_are_typed_verifier(args: &[&str]) -> bool {
    match args.first().copied() {
        Some("test" | "check" | "clippy") => true,
        Some("fmt") => args.contains(&"--check"),
        _ => false,
    }
}

/// The directory Cargo should run in: the workspace when it has a root
/// `Cargo.toml`, else the single first-level directory that has one (angel0
/// itself keeps its crate under `cockpit/`, and every typed verifier used to
/// fail there with "no Cargo.toml"). Ambiguous (several nested crates) or none
/// → `None`; callers keep their existing error paths.
pub(crate) fn cargo_workspace_root(workspace: &Path) -> Option<PathBuf> {
    if workspace.join("Cargo.toml").is_file() {
        return Some(workspace.to_path_buf());
    }
    let mut nested: Vec<PathBuf> = nested_crate_dirs(workspace);
    match nested.len() {
        0 => None,
        1 => nested.pop(),
        // Several crates one level down (angel0: cockpit/, harness/, render-kit/):
        // the one with the most source files is the product; the receipt names
        // it and `run_tests {crate: ...}` pins another.
        _ => {
            nested.sort_by_key(|dir| std::cmp::Reverse(source_file_count(&dir.join("src"))));
            nested.into_iter().next()
        }
    }
}

/// Depth-1 directories holding a `Cargo.toml` (vendored/build dirs skipped),
/// alphabetical.
pub(crate) fn nested_crate_dirs(workspace: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = crate::platform::workspace_lang::detect(workspace)
        .into_iter()
        .filter(|h| {
            h.lang == crate::platform::workspace_lang::Lang::Rust
                && h.manifest
                && h.dir != Path::new(".")
                && h.dir.components().count() == 1
        })
        .map(|h| workspace.join(h.dir))
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

fn source_file_count(dir: &Path) -> usize {
    let mut count = 0usize;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => count += 1,
                _ => {}
            }
        }
    }
    count
}

fn workspace_cargo_control_file(workspace: &Path) -> Option<PathBuf> {
    workspace
        .ancestors()
        .flat_map(|dir| {
            [
                dir.join(".cargo/config.toml"),
                dir.join(".cargo/config"),
                dir.join("rust-toolchain.toml"),
                dir.join("rust-toolchain"),
            ]
        })
        .find(|path| path.exists())
}

fn trusted_verifier_argv(args: &[&str], workspace: &Path) -> Result<Vec<OsString>, String> {
    // Under YOLO the typed Cargo verifier survives only on the pinned
    // toolchain: `capture_pinned_cargo` resolved the cargo/rustc this
    // workspace would select (through rustup) and refused any candidate
    // inside a task-writable root, and `verified_executable` re-checks the
    // captured size+sha256 immediately before every spawn. When no pin could
    // be established the executable lookup below refuses instead. The
    // Non-YOLO keeps its existing control-file refusal; YOLO uses the pin.
    if !cargo_args_are_typed_verifier(args) {
        return Err("internal error: non-verifier requested trusted Cargo mode".to_string());
    }
    for argument in args.iter().skip(1) {
        if matches!(*argument, "--config" | "--manifest-path" | "--target-dir")
            || argument.starts_with("--config=")
            || argument.starts_with("--manifest-path=")
            || argument.starts_with("--target-dir=")
        {
            return Err(format!(
                "trusted Cargo verification rejects configuration-routing argument {argument:?}"
            ));
        }
    }
    let workspace = std::fs::canonicalize(workspace).map_err(|error| {
        format!(
            "resolve trusted-verifier workspace {}: {error}",
            workspace.display()
        )
    })?;
    if !crate::platform::yolo::enabled()
        && let Some(control) = workspace_cargo_control_file(&workspace)
    {
        return Err(format!(
            "trusted Cargo verification refuses workspace-controlled Cargo/toolchain semantics at {} — this only blocks TYPED (reward-labeled) verifier evidence, not compilation. Run the same check through the `shell` tool instead (e.g. `cargo +<pinned> check`), which is legal unlabeled verification for this workspace.",
            control.display()
        ));
    }
    let manifest = workspace.join("Cargo.toml");
    let manifest = std::fs::canonicalize(&manifest)
        .map_err(|error| format!("resolve verifier manifest {}: {error}", manifest.display()))?;
    if !path_within(&manifest, &workspace) {
        return Err(format!(
            "verifier manifest {} escapes workspace {}",
            manifest.display(),
            workspace.display()
        ));
    }
    if !crate::platform::yolo::enabled()
        && (Path::new("/.cargo/config").exists() || Path::new("/.cargo/config.toml").exists())
    {
        return Err("system-root Cargo configuration prevents a neutral verifier cwd".to_string());
    }
    let split = args
        .iter()
        .position(|argument| *argument == "--")
        .unwrap_or(args.len());
    let mut owned = args[..split].iter().map(OsString::from).collect::<Vec<_>>();
    owned.push(OsString::from("--manifest-path"));
    owned.push(manifest.into_os_string());
    owned.extend(args[split..].iter().map(OsString::from));
    Ok(owned)
}

fn sandboxed_cargo_output(
    cargo: &PinnedCargo,
    args: &[&str],
    workspace: &Path,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
    trusted_verifier: bool,
) -> Result<CapturedCommand, String> {
    sandboxed_cargo_output_with_env(
        cargo,
        args,
        workspace,
        policy,
        cancel,
        progress,
        (trusted_verifier, &serde_json::json!({})),
    )
}

fn sandboxed_cargo_output_with_env(
    cargo: &PinnedCargo,
    args: &[&str],
    workspace: &Path,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
    context: (bool, &Value),
) -> Result<CapturedCommand, String> {
    let (trusted_verifier, env_args) = context;
    let executable = cargo.verified_executable()?;
    if args.first() == Some(&"clippy")
        && (executable.cargo_clippy.is_none() || executable.clippy_driver.is_none())
    {
        return Err("trusted Cargo clippy unavailable in the pinned toolchain".to_string());
    }
    if args.first() == Some(&"fmt")
        && (executable.cargo_fmt.is_none() || executable.rustfmt.is_none())
    {
        return Err("trusted Cargo fmt unavailable in the pinned toolchain".to_string());
    }
    // Raw explicit manifests are relative to the caller's workspace. Only
    // auto-detect a crate when Cargo is choosing the manifest for the caller.
    // Arguments after `--` belong to the program, not Cargo.
    let explicit_manifest = !trusted_verifier
        && args
            .iter()
            .take_while(|argument| **argument != "--")
            .any(|argument| {
                *argument == "--manifest-path" || argument.starts_with("--manifest-path=")
            });
    let resolved_workspace = (!explicit_manifest)
        .then(|| cargo_workspace_root(workspace))
        .flatten();
    let workspace: &Path = resolved_workspace.as_deref().unwrap_or(workspace);
    let mut trusted_args = trusted_verifier
        .then(|| trusted_verifier_argv(args, workspace))
        .transpose()?;
    if trusted_verifier && crate::platform::yolo::enabled() {
        executable.controls.validate_dispatch(workspace)?;
        let argv = trusted_args.as_mut().expect("trusted verifier argv");
        let insert = argv
            .iter()
            .position(|arg| arg == "--")
            .unwrap_or(argv.len());
        let config_args = executable.controls.config_args();
        argv.splice(insert..insert, config_args);
    }
    let collect_sources = trusted_verifier && args.first() == Some(&"check");
    if collect_sources {
        let argv = trusted_args.as_mut().expect("trusted check argv");
        // The internal protocol is fixed; caller-selected formats cannot remove
        // records or inject a second presentation channel.
        if argv.iter().any(|arg| {
            arg == "--message-format" || arg.as_encoded_bytes().starts_with(b"--message-format=")
        }) {
            return Err("typed Cargo check owns --message-format; omit that flag".into());
        }
        let insert = argv
            .iter()
            .position(|arg| arg == "--")
            .unwrap_or(argv.len());
        argv.insert(insert, "--message-format=json-render-diagnostics".into());
    }
    let cargo_home = trusted_verifier
        .then(|| ephemeral_cargo_home(executable))
        .transpose()?;
    let mut cargo_target = cargo_home.as_ref().map(|home| home.target.clone());
    let mut effective_policy = policy.clone();
    if let Some(home) = &cargo_home {
        effective_policy.writable_roots.push(home.path.clone());
    }
    let mut cmd = match trusted_args.as_ref() {
        Some(args) => sandbox::command(&executable.cargo.path, args, &effective_policy)?,
        None => sandbox::command(
            &executable.cargo.path,
            args.iter().copied(),
            &effective_policy,
        )?,
    };
    cargo.configure_command(&mut cmd, executable)?;
    apply_verifier_env(&mut cmd, env_args)?;
    if let Some(home) = &cargo_home {
        cmd.current_dir("/");
        cmd.env("CARGO_HOME", &home.path);
        cmd.env("CARGO_TARGET_DIR", &home.target);
    } else {
        cmd.current_dir(workspace);
    }
    if policy.mandatory || crate::agent::sandbox::sealed::identity().is_some() {
        let scratch = workspace.join(".angel-experiment-tmp");
        std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
        cmd.env("TMPDIR", &scratch)
            .env("TMP", &scratch)
            .env("TEMP", &scratch)
            .env("XDG_CACHE_HOME", scratch.join("cache"));
        let target = scratch.join("target");
        cmd.env("CARGO_TARGET_DIR", &target);
        cargo_target = Some(target);
    }
    let progress = if collect_sources {
        progress.map(|sink| {
            Arc::new(move |stream, bytes: &[u8]| {
                if stream == crate::agent::harness::ProcessStream::Stderr {
                    sink(stream, bytes);
                }
            }) as Arc<ToolOutputProgress>
        })
    } else {
        progress
    };
    executable.controls.revalidate()?;
    let started = std::time::Instant::now();
    let capture =
        output_timed_extensible_cancellable_with_progress(cmd, tool_timeout(), cancel, progress)?;
    let out = capture.output;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let (checked_sources, stdout) = if collect_sources {
        targets::cargo_evidence(
            &stdout,
            cargo_target.as_deref().expect("private check target"),
            workspace,
        )
    } else {
        (None, stdout)
    };
    Ok(CapturedCommand {
        checked_sources,
        stdout,
        stderr: strip_launcher_stderr(&String::from_utf8_lossy(&out.stderr)),
        success: out.status.success(),
        exit: out.status.code(),
        timed_out: capture.timed_out,
        cancelled: capture.cancelled,
        dur_ms: started.elapsed().as_millis(),
        timeout_diag: capture.timeout_diag,
    })
}

fn timeout_detail(output: &CapturedCommand) -> String {
    if output.cancelled {
        return "cancelled — process group killed".to_string();
    }
    if output.timed_out {
        match output.timeout_diag.as_ref() {
            Some(diag) => format!(
                "timed out after {}s — {}; process group killed; raise/disable via ANGEL_TOOL_TIMEOUT",
                tool_timeout()
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0),
                diag.summary()
            ),
            None => format!(
                "timed out after {}s — process group killed; raise/disable via ANGEL_TOOL_TIMEOUT",
                tool_timeout()
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0)
            ),
        }
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Cargo tool — pull crates, build, test, in a sandboxed workspace.
// ---------------------------------------------------------------------------

pub(crate) struct CargoTool {
    workspace: PathBuf,
    policy: SandboxPolicy,
    cargo: PinnedCargo,
    mutation_targets: Arc<MutationTargets>,
}

impl CargoTool {
    pub(crate) fn with_mutation_targets(mut self, targets: Arc<MutationTargets>) -> Self {
        self.mutation_targets = targets;
        self
    }

    #[cfg(test)]
    pub(crate) fn in_dir(workspace: PathBuf) -> Self {
        let cargo = PinnedCargo::capture(&workspace);
        Self::in_dir_with_cargo(workspace, cargo)
    }

    pub(crate) fn confined_in_dir_with_cargo(workspace: PathBuf, cargo: PinnedCargo) -> Self {
        Self {
            policy: SandboxPolicy {
                writable_roots: vec![workspace.clone()],
                allow_network: false,
                enforce: true,
                mandatory: true,
                sealed_reads: Vec::new(),
                deny_reads: Vec::new(),
            },
            workspace,
            cargo,
            mutation_targets: Arc::new(MutationTargets::default()),
        }
    }

    pub(crate) fn in_dir_with_cargo(workspace: PathBuf, cargo: PinnedCargo) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(workspace.clone());
        Self {
            workspace,
            policy,
            cargo,
            mutation_targets: Arc::new(MutationTargets::default()),
        }
    }
}

impl Tool for CargoTool {
    fn name(&self) -> &str {
        "cargo"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "cargo".to_string(),
            description: format!(
                "Run a cargo command in the workspace (sandboxed, network {}). Pass subcommand + flags as `args`, e.g. build or test. {}",
                if self.policy.allow_network {
                    "on"
                } else {
                    "off"
                },
                if self.policy.allow_network {
                    "Use this to pull crates and build/calibrate code."
                } else {
                    "Use available local dependencies; network fetches are unavailable in this confined experiment."
                }
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": { "args": { "type": "string", "description": "cargo subcommand + flags" } },
                "required": ["args"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        self.call_with_cancel_and_progress(args, cancel, None)
    }

    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        let raw = args["args"].as_str().ok_or("missing 'args'")?;
        std::fs::create_dir_all(&self.workspace)
            .map_err(|e| format!("workspace create failed: {e}"))?;
        let parts =
            parse_direct_argv(raw).map_err(|error| format!("invalid cargo arguments: {error}"))?;
        if parts.is_empty() {
            return Err("empty cargo args".to_string());
        }
        let required = (parts.first().is_some_and(|part| part == "check"))
            .then(|| targets::required(&self.workspace, &self.mutation_targets));
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        // Raw build/run/custom Cargo subcommands can execute arbitrary package
        // code and edit ignored source. They cannot reset the source inventory.
        if !cargo_args_are_typed_verifier(&part_refs) {
            self.mutation_targets.mark_opaque();
        }
        let out = sandboxed_cargo_output(
            &self.cargo,
            &part_refs,
            &self.workspace,
            &self.policy,
            cancel,
            progress,
            cargo_args_are_typed_verifier(&part_refs),
        )?;
        let mut combined = out.stdout.clone();
        if !out.stderr.trim().is_empty() {
            combined.push_str("\n[stderr] ");
            combined.push_str(out.stderr.trim());
        }
        let output = crate::agent::harness::cap_text_owned(combined, 4000, 0)
            .trim()
            .to_string();
        let text = format!("cargo {raw}");
        let experience = crate::knowledge::experience::CmdExperience {
            tool: "cargo",
            text: &text,
            exit: out.exit,
            timed_out: out.timed_out,
            dur_ms: out.dur_ms,
            bytes_out: out.stdout.len() + out.stderr.len(),
            // Exec'd directly: no shell, so no pipeline can steal the status.
            shell: crate::knowledge::experience::CmdShell::Direct,
        };
        crate::knowledge::experience::record_cmd_event(&experience, &self.workspace);
        if out.cancelled {
            return Err(format!("cargo command cancelled\n{}", output));
        }
        let (verdict, reason) = crate::knowledge::experience::cmd_verdict(&experience);
        match verdict {
            crate::knowledge::experience::VERDICT_FAIL => {
                let code = out
                    .exit
                    .map(|exit| exit.to_string())
                    .unwrap_or_else(|| "signal".to_string());
                let mut error = format!("cargo command failed (exit {code})");
                if !output.trim().is_empty() {
                    error.push('\n');
                    error.push_str(&output);
                }
                Err(error)
            }
            crate::knowledge::experience::VERDICT_NONE => {
                let why = reason.unwrap_or("untrusted status");
                Ok(append_command_verdict(
                    output,
                    &format!("[cargo verdict unavailable: reason={why}]"),
                ))
            }
            _ => {
                let output = append_command_verdict(output, "[cargo verdict: pass]");
                Ok(if let Some(required) = required {
                    targets::annotate(
                        format!("cargo check: command passed\n{output}"),
                        &required,
                        out.checked_sources.as_ref(),
                        targets::explicit_selection(&parts),
                    )
                } else {
                    output
                })
            }
        }
    }
}

fn append_command_verdict(mut output: String, marker: &str) -> String {
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(marker);
    output
}

// Verifiable test reward — run the suite, parse pass/fail, derive an RL reward.
// A ground-truth signal for software-dev RL (complements the DICE reward):
// trajectories that make tests pass score higher. `TestOutcome` /
// `parse_test_result` are public so the reinforce loop can score on them too.
// ---------------------------------------------------------------------------

/// Structured outcome of a test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TestOutcome {
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
}
impl TestOutcome {
    pub fn ran(&self) -> usize {
        self.passed + self.failed
    }
    #[allow(dead_code)] // public predicate on the outcome; reward()/ran() are the current callers
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.passed > 0
    }
    /// Reward in [0,1]: fraction of executed tests that passed; 0 if none ran
    /// (a build error or empty suite earns nothing).
    pub fn reward(&self) -> f32 {
        match self.ran() {
            0 => 0.0,
            n => self.passed as f32 / n as f32,
        }
    }
}

/// Parse libtest/cargo summary lines ("test result: ok. 12 passed; 0 failed;
/// 1 ignored; …"), summing across the per-binary lines cargo emits.
pub fn parse_test_result(output: &str) -> TestOutcome {
    let mut o = TestOutcome::default();
    for line in output.lines() {
        if !line.trim_start().starts_with("test result:") {
            continue;
        }
        let toks: Vec<&str> = line
            .split(|c: char| c == ';' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        for w in toks.windows(2) {
            if let Ok(n) = w[0].parse::<usize>() {
                match w[1] {
                    "passed" => o.passed += n,
                    "failed" => o.failed += n,
                    "ignored" => o.ignored += n,
                    _ => {}
                }
            }
        }
    }
    o
}

pub(crate) struct RunTestsTool {
    policy: SandboxPolicy,
    workspace: PathBuf,
    cargo: PinnedCargo,
    native: PinnedNativeRuntimes,
}
impl RunTestsTool {
    #[cfg(test)]
    pub(crate) fn in_dir(dir: PathBuf) -> Self {
        let cargo = PinnedCargo::capture(&dir);
        Self::in_dir_with_cargo(dir, cargo)
    }

    #[cfg(test)]
    pub(crate) fn in_dir_with_cargo(dir: PathBuf, cargo: PinnedCargo) -> Self {
        let native = PinnedNativeRuntimes::capture(&dir);
        Self::in_dir_with_runtimes(dir, cargo, native)
    }

    pub(crate) fn in_dir_with_runtimes(
        dir: PathBuf,
        cargo: PinnedCargo,
        native: PinnedNativeRuntimes,
    ) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(dir.clone());
        Self {
            policy,
            workspace: dir,
            cargo,
            native,
        }
    }
}
impl Tool for RunTestsTool {
    fn name(&self) -> &str {
        "run_tests"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "run_tests".to_string(),
            description: "Run the workspace test suite under the sandbox and return a \
                          structured pass/fail summary plus a [0,1] reward. Typed (pinned, \
                          reward-labeled) native runners come first: Cargo for Rust, Node's \
                          builtin test runner for JavaScript, isolated stdlib unittest for \
                          Python. When the typed route declines (mixed roots, package scripts, \
                          other frameworks, YOLO) the suite still runs through the runner the \
                          workspace's own files choose — `npm test` / `node --test`, `pytest` / \
                          `unittest discover`, `go test`, `swift test` — and the receipt says \
                          why the reward is unlabeled. Reports observed tests; project tests do \
                          not replace an independent evaluator."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "runtime": { "type": "string", "enum": ["auto", "rust", "node", "python", "go"],
                        "description": "typed runtime selection; an explicit entrypoint selects its runtime, otherwise auto requires one root language (optional)" },
                    "entrypoint": { "type": "string", "enum": ["", "node", "unittest", "pytest"], "description": "Explicit native test entrypoint; node bypasses package scripts, unittest accepts discover/modules/files, pytest requires an immutable system installation." },
                    "runner": { "type": "string", "description": "alias of runtime for the scan fallback: rust|js|python|go|swift (optional)" },
                    "args": { "type": "string", "description": "Rust: cargo test flags. Node: confined test files/globs and test-name/skip-pattern. Python: unittest -s directory, -p pattern, -k. Other runners: appended verbatim (optional)" },
                    "dir": { "type": "string", "description": "run the suite of this subdirectory of the workspace (mono-repos: e.g. `sidecar/forge` for its python tests, `cockpit` for that crate); the runner is chosen from that directory's own files (optional)" },
                    "crate": { "type": "string", "description": "alias of `dir` for repos whose crates live one directory down (e.g. cockpit/, harness/); default = the largest (optional)" }
                },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        self.call_with_cancel_and_progress(args, cancel, None)
    }

    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        use crate::platform::workspace_lang::{self, Lang};
        // Typed native verification first (pinned runtimes, reward-labeled
        // evidence). It declines with "verification inconclusive: …" for mixed
        // roots, package scripts and unsupported frameworks, and errors under
        // YOLO; every one of those used to cost the model a hop (2026-09-06
        // arena capture, 2026-09-07 dogfoods), so instead of returning them the
        // workspace-scan route below runs the suite and carries the reason.
        let sub_requested = args["dir"]
            .as_str()
            .or_else(|| args["crate"].as_str())
            .filter(|d| !d.trim().is_empty());
        let typed_workspace = match sub_requested {
            Some(dir) => {
                if std::path::Path::new(dir)
                    .components()
                    .any(|p| p == std::path::Component::Normal("off-limits".as_ref()))
                {
                    return Err(
                        "run_tests: quarantined subdirectory is not a verifier workspace".into(),
                    );
                }
                let root = std::fs::canonicalize(&self.workspace).map_err(|e| {
                    format!(
                        "run_tests: resolve workspace {}: {e}",
                        self.workspace.display()
                    )
                })?;
                let candidate = root.join(dir);
                let invalid_directory = || {
                    let known: Vec<String> = nested_crate_dirs(&root)
                        .iter()
                        .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
                        .collect();
                    format!(
                        "run_tests: resolve subdirectory {}: `{dir}` is not a directory inside the workspace — crates here: {}",
                        candidate.display(),
                        if known.is_empty() {
                            "(none)".to_string()
                        } else {
                            known.join(", ")
                        }
                    )
                };
                let selected = std::fs::canonicalize(&candidate).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        invalid_directory()
                    } else {
                        format!(
                            "run_tests: resolve subdirectory {}: {e}",
                            candidate.display()
                        )
                    }
                })?;
                if !selected.is_dir()
                    || !path_within(&selected, &root)
                    || selected
                        .components()
                        .any(|p| p == std::path::Component::Normal("off-limits".as_ref()))
                {
                    return Err(invalid_directory());
                }
                selected
            }
            None => self.workspace.clone(),
        };
        let mut typed_note: Option<String> = None;
        if let Some(result) = verifier::dispatch(
            &self.native,
            verifier::Kind::Tests,
            args,
            &typed_workspace,
            &self.policy,
            cancel,
            progress.clone(),
        ) {
            match &result {
                // An executed native selection already has its final receipt.
                // In particular, never rerun an empty selector via a fallback
                // that could discover a different suite.
                Ok(text) if text.contains("\n[verifier attribution] ") => return result,
                Ok(text) | Err(text) if text.starts_with("verification inconclusive:") => {
                    typed_note = Some(text.clone());
                }
                Err(text) if text.starts_with("typed native verification is disabled") => {
                    typed_note = Some(text.clone());
                }
                _ => return result,
            }
        }
        std::fs::create_dir_all(&self.workspace)
            .map_err(|e| format!("workspace create failed: {e}"))?;
        let prefer = args["runner"]
            .as_str()
            .or_else(|| args["lang"].as_str())
            .or_else(|| args["runtime"].as_str().filter(|r| *r != "auto"))
            .map(|s| {
                workspace_lang::parse_lang(s)
                    .ok_or_else(|| format!("unknown runner {s:?}: use rust|js|python|go|swift"))
            })
            .transpose()?;
        // `dir` (alias `crate`): a subdirectory whose own files choose the runner —
        // mono-repos keep suites under e.g. sidecar/forge (python) or cockpit (rust).
        let sub = args["dir"]
            .as_str()
            .or_else(|| args["crate"].as_str())
            .map(|d| d.trim().trim_matches('/'))
            .filter(|d| !d.is_empty());
        let scan_root = match sub {
            Some(name) => {
                let dir = self.workspace.join(name);
                let inside = std::fs::canonicalize(&dir)
                    .ok()
                    .zip(std::fs::canonicalize(&self.workspace).ok())
                    .is_some_and(|(d, w)| path_within(&d, &w));
                if !dir.is_dir() || !inside {
                    let known: Vec<String> = nested_crate_dirs(&self.workspace)
                        .iter()
                        .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
                        .collect();
                    return Err(format!(
                        "run_tests: `{name}` is not a directory inside the workspace — crates here: {}",
                        if known.is_empty() {
                            "(none)".to_string()
                        } else {
                            known.join(", ")
                        }
                    ));
                }
                dir
            }
            None => self.workspace.clone(),
        };
        let hits = workspace_lang::detect(&scan_root);
        let cargo_dir = if sub.is_some() && scan_root.join("Cargo.toml").is_file() {
            Some(scan_root.clone())
        } else if sub.is_some() {
            None
        } else {
            cargo_workspace_root(&self.workspace)
        };
        let cargo_route = match prefer {
            Some(Lang::Rust) => true,
            Some(_) => false,
            None => cargo_dir.is_some(),
        };
        if cargo_route {
            let Some(dir) = cargo_dir else {
                return Err(format!(
                    "run_tests: no Cargo.toml in {}",
                    sub.map(|d| format!("{d}/"))
                        .unwrap_or_else(|| "this workspace".to_string())
                ));
            };
            return self.run_cargo(&dir, args, cancel, progress);
        }
        let Some(plan) = workspace_lang::plan_tests(&scan_root, &hits, prefer) else {
            let seen = workspace_lang::lang_names(&hits);
            return Err(if seen.is_empty() {
                "run_tests: no test runner detected — the shallow scan (root + one level) found no \
                 Cargo.toml, package.json, *.test.js, pyproject.toml, test_*.py, go.mod or \
                 Package.swift. Run the suite with `shell` and its own command."
                    .to_string()
            } else {
                format!(
                    "run_tests: detected {} but could not plan a runner; run the suite with `shell`",
                    seen.join(",")
                )
            });
        };
        self.run_plan(&scan_root, plan, args, typed_note, cancel, progress)
    }
}

impl RunTestsTool {
    /// The Cargo route: pinned toolchain, trusted verifier semantics. Under YOLO the
    /// typed (reward-labeled) verifier is unavailable, so the suite still runs on
    /// the pinned cargo but the receipt says the reward is unlabeled — one hop of
    /// real evidence instead of an error the model has to route around.
    fn run_cargo(
        &self,
        cargo_dir: &Path,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        let argv = cargo_argv("test", args["args"].as_str().unwrap_or(""))?;
        let argv_refs = argv.iter().map(String::as_str).collect::<Vec<_>>();
        // Under YOLO the typed verifier runs only when the pinned toolchain
        // identity held for this spawn; a drifted or unpinnable toolchain
        // errors inside `sandboxed_cargo_output` instead, so the receipt can
        // never claim trusted evidence without a live pin.
        let yolo_pin = crate::platform::yolo::enabled()
            .then(|| self.cargo.pin_note())
            .flatten();
        let yolo = yolo_pin.is_none() && crate::platform::yolo::enabled();
        let crate_note = if cargo_dir != self.workspace {
            cargo_dir
                .strip_prefix(&self.workspace)
                .map(|p| format!(" (crate {})", p.display()))
                .unwrap_or_default()
        } else {
            String::new()
        };

        let out = sandboxed_cargo_output_with_env(
            &self.cargo,
            &argv_refs,
            cargo_dir,
            &self.policy,
            cancel,
            progress,
            (!yolo, args),
        )
        .map_err(|e| format!("spawn `cargo test` failed: {e}"))?;
        crate::knowledge::experience::record_cmd_event(
            &crate::knowledge::experience::CmdExperience {
                tool: "run_tests",
                text: &format!("cargo {}", argv.join(" ")),
                exit: out.exit,
                timed_out: out.timed_out,
                dur_ms: out.dur_ms,
                bytes_out: out.stdout.len() + out.stderr.len(),
                // Exec'd directly: no shell, so no pipeline can steal the status.
                shell: crate::knowledge::experience::CmdShell::Direct,
            },
            &self.workspace,
        );
        let outcome = parse_test_result(&out.stdout);

        if out.cancelled {
            return Err(format!(
                "cargo test cancelled\n{}",
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if out.timed_out {
            return Ok(format!(
                "tests: {}\n{}",
                timeout_detail(&out),
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }

        if !out.success {
            let code = out
                .exit
                .map(|exit| exit.to_string())
                .unwrap_or_else(|| "signal".to_string());
            return Err(format!(
                "cargo test failed (exit {code})\n{}",
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }

        if outcome.ran() == 0 {
            let empty = if args["args"]
                .as_str()
                .is_some_and(|value| !value.trim().is_empty())
            {
                "verification inconclusive: no tests ran"
            } else {
                "no tests ran (build error?)"
            };
            return Ok(format!(
                "{empty} — reward 0.00\n[verifier selection] {}\n{}",
                serde_json::json!({"runtime": "rust", "argv": argv}),
                tail(&out.stderr, 1500)
            ));
        }
        let reward = if yolo {
            "reward unlabeled (YOLO: typed Cargo verification is off; the suite ran on the pinned cargo but this receipt is not trusted evidence)".to_string()
        } else {
            reward_text(outcome.passed, outcome.failed)
        };
        let mut summary = format!(
            "tests: {} passed, {} failed, {} ignored — {reward}{crate_note}",
            outcome.passed, outcome.failed, outcome.ignored,
        );
        summary.push_str(&format!(
            "\n[verifier selection] {}",
            serde_json::json!({"runtime": "rust", "argv": argv})
        ));
        let yolo_pin_note = yolo_pin.clone().or_else(|| self.cargo.pin_note());
        if let Some(pin) = yolo_pin_note.as_deref() {
            summary.push('\n');
            summary.push_str(pin);
        }
        if outcome.failed > 0 {
            summary.push('\n');
            summary.push_str(&tail(&out.stdout, 1500));
        }
        Ok(summary)
    }

    /// Non-Cargo runners (node, npm, pytest/unittest, go, swift): the program is
    /// resolved from PATH, run under the same sandbox policy in the directory the
    /// marker was found in, and the counts are parsed per runner.
    fn run_plan(
        &self,
        scan_root: &Path,
        plan: crate::platform::workspace_lang::TestPlan,
        args: &Value,
        typed_note: Option<String>,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        let extra = args["args"].as_str().unwrap_or("");
        let mut plan = plan;
        let mut argv = plan.args.clone();
        if !extra.trim().is_empty() {
            let node_script = plan
                .script
                .as_deref()
                .is_some_and(|sc| sc.trim_start().starts_with("node --test"));
            if plan.program == "npm" && node_script {
                // The script is node's own runner with its globs baked in; an
                // appended file only ADDS to them (a real-repo run on 2026-09-07
                // ran all 765 root tests for one file). Call the runner directly
                // so the caller's files/names are the whole selection.
                plan.program = "node";
                argv = vec!["--test".to_string()];
                plan.label =
                    "node --test (bypassing the npm script for the given args)".to_string();
            } else if plan.program == "npm" && !argv.iter().any(|a| a == "--") {
                // Other scripts (jest, vitest, mocha) take filters after `--`.
                argv.push("--".to_string());
            }
            argv.extend(parse_direct_argv(extra)?);
        }
        let dir = scan_root.join(&plan.dir);
        let text = format!("{} {}", plan.program, argv.join(" "));
        let out = sandboxed_program_output(
            plan.program,
            &argv,
            &dir,
            &self.policy,
            cancel,
            (progress, args),
        )
        .map_err(|e| {
            format!(
                "spawn `{}` failed: {e} (chosen because of {})",
                plan.label, plan.because
            )
        })?;
        crate::knowledge::experience::record_cmd_event(
            &crate::knowledge::experience::CmdExperience {
                tool: "run_tests",
                text: &text,
                exit: out.exit,
                timed_out: out.timed_out,
                dur_ms: out.dur_ms,
                bytes_out: out.stdout.len() + out.stderr.len(),
                shell: crate::knowledge::experience::CmdShell::Direct,
            },
            &self.workspace,
        );
        let counts = crate::platform::workspace_lang::parse_runner_output(
            plan.lang,
            &out.stdout,
            &out.stderr,
        );
        let label = &plan.label;
        let both = || tail(&format!("{}\n{}", out.stdout, out.stderr), 1500);
        if out.cancelled {
            return Err(format!("{label} cancelled\n{}", both()));
        }
        if out.timed_out {
            return Ok(format!("tests: {}\n{}", timeout_detail(&out), both()));
        }
        let ran = counts.passed + counts.failed;
        // A project-controlled runner can print any counts it likes. Its
        // actual unsuccessful exit always wins over the report on stdout.
        if !out.success {
            let code = out
                .exit
                .map(|exit| exit.to_string())
                .unwrap_or_else(|| "signal".to_string());
            return Err(format!(
                "{label} failed (exit {code}) — chosen because of {}; pin another runner with `runner` or use `shell`\n{}",
                plan.because,
                both()
            ));
        }
        if ran == 0 {
            return Ok(format!(
                "no tests ran ({label}, chosen because of {}) — reward unlabeled\n{}",
                plan.because,
                both()
            ));
        }
        // Only the typed route labels a reward; this route always says why it
        // is unlabeled (typed verification declined, or YOLO).
        let reward = if crate::platform::yolo::enabled() {
            "reward unlabeled (YOLO: typed native verification is off; the suite ran on the host runtime but this receipt is not trusted evidence)".to_string()
        } else {
            format!(
                "reward unlabeled ({})",
                typed_note
                    .as_deref()
                    .unwrap_or("typed native verification not available for this layout")
            )
        };
        let mut summary = format!(
            "tests: {} passed, {} failed, {} skipped — {reward} ({label})",
            counts.passed, counts.failed, counts.skipped,
        );
        if counts.failed > 0 {
            summary.push('\n');
            summary.push_str(&both());
        }
        Ok(summary)
    }
}

/// Run a PATH-resolved program under the sandbox policy in `dir`, with the same
/// bounded capture the Cargo verifiers use.
fn sandboxed_program_output(
    program: &str,
    args: &[String],
    dir: &Path,
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    context: (Option<Arc<ToolOutputProgress>>, &Value),
) -> Result<CapturedCommand, String> {
    let (progress, env_args) = context;
    // npm is usually an env-node script: resolve its interpreter before spawn
    // so an absent Node does not surface as a raw /usr/bin/env failure.
    let node = (program == "npm")
        .then(|| crate::agent::tools::runtime_missing::resolve("node"))
        .transpose()?;
    let resolved = crate::agent::tools::runtime_missing::resolve(program)?;
    // Use a pinned Node directly for npm's standard node shebang. The pin's
    // basename need not be `node`; merely prepending its parent to PATH would
    // silently lose such a pin. Non-Node npm wrappers keep their own semantics.
    let npm_uses_node = node.is_some() && {
        use std::io::Read;
        let mut prefix = [0; 256];
        std::fs::File::open(&resolved)
            .and_then(|mut file| file.read(&mut prefix))
            .ok()
            .is_some_and(|n| {
                let text = String::from_utf8_lossy(&prefix[..n]);
                text.lines().next().is_some_and(|line| {
                    line.starts_with("#!")
                        && line
                            .split_whitespace()
                            .last()
                            .is_some_and(|word| word.rsplit('/').next() == Some("node"))
                })
            })
    };
    let mut cmd = if npm_uses_node {
        sandbox::command(
            node.as_ref().expect("npm Node resolved"),
            std::iter::once(resolved.as_os_str()).chain(args.iter().map(OsStr::new)),
            policy,
        )?
    } else {
        sandbox::command(&resolved, args.iter().map(String::as_str), policy)?
    };
    apply_verifier_env(&mut cmd, env_args)?;
    if let Some(node) = node
        && let Some(parent) = node.parent()
    {
        let mut paths = vec![parent.to_path_buf()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        cmd.env(
            "PATH",
            std::env::join_paths(paths).map_err(|e| e.to_string())?,
        );
    }
    cmd.current_dir(dir);
    // Deterministic, machine-readable output where the runner supports it.
    cmd.env("CI", "1");
    cmd.env("NO_COLOR", "1");
    cmd.env("FORCE_COLOR", "0");
    let started = std::time::Instant::now();
    let capture =
        output_timed_extensible_cancellable_with_progress(cmd, tool_timeout(), cancel, progress)
            .map_err(|error| {
                if error.contains("No such file or directory")
                    && dir.is_dir()
                    && crate::agent::tools::runtime_missing::pin(program).is_some()
                {
                    crate::agent::tools::runtime_missing::RuntimeMissing::new(
                        program,
                        format!("{} (PATH or runtime pin)", resolved.display()),
                    )
                    .encode()
                } else {
                    error
                }
            })?;
    let out = capture.output;
    if out.status.code() == Some(127)
        && let Some(error) = crate::agent::tools::runtime_missing::from_command_not_found(
            &String::from_utf8_lossy(&out.stderr),
        )
    {
        return Err(error);
    }
    Ok(CapturedCommand {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: strip_launcher_stderr(&String::from_utf8_lossy(&out.stderr)),
        success: out.status.success(),
        exit: out.status.code(),
        timed_out: capture.timed_out,
        cancelled: capture.cancelled,
        dur_ms: started.elapsed().as_millis(),
        checked_sources: None,
        timeout_diag: capture.timeout_diag,
    })
}

#[cfg(test)]
use crate::platform::workspace_lang::resolve_on_path;

/// Reward text that never rounds a red suite up to "1.00": two decimals when
/// everything passed, three when anything failed (764 passed / 1 failed printed
/// as "reward 1.00" in a real-repo run on 2026-09-07).
pub(crate) fn reward_text(passed: usize, failed: usize) -> String {
    let ran = passed + failed;
    let reward = if ran == 0 {
        0.0
    } else {
        passed as f32 / ran as f32
    };
    if failed > 0 {
        format!("reward {reward:.3}")
    } else {
        format!("reward {reward:.2}")
    }
}

/// Last `max` bytes of `s` (char-boundary safe), for surfacing failure detail.
pub(crate) fn tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.trim().to_string();
    }
    let mut start = s.len() - max;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    format!("…[earlier output omitted]\n{}", s[start..].trim())
}

// ---------------------------------------------------------------------------
// Lint reward — run clippy, count warnings/errors, derive a quality reward.
// A second verifiable signal alongside run_tests: fewer lints = higher reward.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LintOutcome {
    pub warnings: usize,
    pub errors: usize,
}
impl LintOutcome {
    pub fn clean(&self) -> bool {
        self.errors == 0 && self.warnings == 0
    }
    /// Reward in [0,1]: 0 if anything errored, else 1/(1+warnings).
    pub fn reward(&self) -> f32 {
        if self.errors > 0 {
            0.0
        } else {
            1.0 / (1.0 + self.warnings as f32)
        }
    }
}

/// Count clippy/rustc diagnostics, skipping cargo's summary lines ("generated
/// N warnings", "aborting due to …", "could not compile …").
pub fn parse_lint(output: &str) -> LintOutcome {
    let mut o = LintOutcome::default();
    for line in output.lines() {
        let l = line.trim_start();
        if l.starts_with("warning:") {
            if !l.contains("generated") {
                o.warnings += 1;
            }
        } else if l.starts_with("error[")
            || (l.starts_with("error:")
                && !l.contains("aborting due to")
                && !l.contains("could not compile")
                && !l.contains("generated"))
        {
            // `error[E…]` is always a real diagnostic; bare `error:` counts too
            // unless it's one of cargo's summary lines.
            o.errors += 1;
        }
    }
    o
}

pub(crate) struct LintTool {
    policy: SandboxPolicy,
    workspace: PathBuf,
    cargo: PinnedCargo,
    native: PinnedNativeRuntimes,
}
impl LintTool {
    #[cfg(test)]
    pub(crate) fn in_dir(dir: PathBuf) -> Self {
        let cargo = PinnedCargo::capture(&dir);
        Self::in_dir_with_cargo(dir, cargo)
    }

    #[cfg(test)]
    pub(crate) fn in_dir_with_cargo(dir: PathBuf, cargo: PinnedCargo) -> Self {
        let native = PinnedNativeRuntimes::capture(&dir);
        Self::in_dir_with_runtimes(dir, cargo, native)
    }

    pub(crate) fn in_dir_with_runtimes(
        dir: PathBuf,
        cargo: PinnedCargo,
        native: PinnedNativeRuntimes,
    ) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(dir.clone());
        Self {
            policy,
            workspace: dir,
            cargo,
            native,
        }
    }
}
impl Tool for LintTool {
    fn name(&self) -> &str {
        "lint"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "lint".to_string(),
            description: "Run pinned Cargo clippy for Rust and return warning/error counts. \
                          Select runtime explicitly for mixed roots. Node/Python custom linters \
                          currently return inconclusive; syntax checks are not lint evidence."
                .to_string(),
            params: verifier::schema_args(
                "Extra cargo clippy arguments on the Rust route; other lint adapters are unsupported.",
            ),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        self.call_with_cancel_and_progress(args, cancel, None)
    }

    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        if let Some(result) = verifier::dispatch(
            &self.native,
            verifier::Kind::Lint,
            args,
            &self.workspace,
            &self.policy,
            cancel,
            progress.clone(),
        ) {
            return result;
        }
        let extra = args["args"].as_str().unwrap_or("");
        std::fs::create_dir_all(&self.workspace)
            .map_err(|e| format!("workspace create failed: {e}"))?;
        let argv = cargo_argv("clippy", extra)?;
        let argv_refs = argv.iter().map(String::as_str).collect::<Vec<_>>();

        let out = sandboxed_cargo_output(
            &self.cargo,
            &argv_refs,
            &self.workspace,
            &self.policy,
            cancel,
            progress,
            true,
        )
        .map_err(|e| format!("spawn `cargo clippy` failed: {e}"))?;
        if out.cancelled {
            return Err(format!(
                "cargo clippy cancelled\n{}",
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if out.timed_out {
            return Ok(format!(
                "lint: {}\n{}",
                timeout_detail(&out),
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if !out.success {
            let code = out
                .exit
                .map(|exit| exit.to_string())
                .unwrap_or_else(|| "signal".to_string());
            return Err(format!(
                "cargo clippy failed (exit {code})\n{}",
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        let o = parse_lint(&out.stderr);
        let mut summary = format!(
            "lint: {} warnings, {} errors — reward {:.2}",
            o.warnings,
            o.errors,
            o.reward()
        );
        if let Some(pin) = self.cargo.pin_note() {
            summary.push('\n');
            summary.push_str(&pin);
        }
        if !o.clean() {
            summary.push('\n');
            summary.push_str(&tail(&out.stderr, 1500));
        }
        Ok(summary)
    }
}

pub(crate) struct CheckTool {
    policy: SandboxPolicy,
    workspace: PathBuf,
    cargo: PinnedCargo,
    native: PinnedNativeRuntimes,
    mutation_targets: Arc<MutationTargets>,
}
impl CheckTool {
    pub(crate) fn with_mutation_targets(mut self, targets: Arc<MutationTargets>) -> Self {
        self.mutation_targets = targets;
        self
    }

    #[cfg(test)]
    pub(crate) fn in_dir(dir: PathBuf) -> Self {
        let cargo = PinnedCargo::capture(&dir);
        Self::in_dir_with_cargo(dir, cargo)
    }

    #[cfg(test)]
    pub(crate) fn in_dir_with_cargo(dir: PathBuf, cargo: PinnedCargo) -> Self {
        let native = PinnedNativeRuntimes::capture(&dir);
        Self::in_dir_with_runtimes(dir, cargo, native)
    }

    pub(crate) fn in_dir_with_runtimes(
        dir: PathBuf,
        cargo: PinnedCargo,
        native: PinnedNativeRuntimes,
    ) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(dir.clone());
        Self {
            policy,
            workspace: dir,
            cargo,
            native,
            mutation_targets: Arc::new(MutationTargets::default()),
        }
    }
}
impl Tool for CheckTool {
    fn name(&self) -> &str {
        "check"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "check".to_string(),
            description: "Run pinned cargo check for Rust, or syntax-only checks for JavaScript \
                          and Python without executing source. Native syntax checks do not \
                          establish types, dependencies, lint cleanliness or test correctness. \
                          Auto selection needs one root language; mixed roots need runtime."
                .to_string(),
            params: verifier::schema_args(
                "Rust: cargo check flags. Node/Python: confined source files or simple filename globs; default scans up to 256 visible source files, excluding generated/vendor directories.",
            ),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        self.call_with_cancel_and_progress(args, cancel, None)
    }

    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        if let Some(result) = verifier::dispatch(
            &self.native,
            verifier::Kind::Check,
            args,
            &self.workspace,
            &self.policy,
            cancel,
            progress.clone(),
        ) {
            return result;
        }
        let extra = args["args"].as_str().unwrap_or("");
        std::fs::create_dir_all(&self.workspace)
            .map_err(|e| format!("workspace create failed: {e}"))?;
        let required = targets::required(&self.workspace, &self.mutation_targets);
        let argv = cargo_argv("check", extra)?;
        let explicit = targets::explicit_selection(&argv);
        let argv_refs = argv.iter().map(String::as_str).collect::<Vec<_>>();

        let out = sandboxed_cargo_output(
            &self.cargo,
            &argv_refs,
            &self.workspace,
            &self.policy,
            cancel,
            progress,
            true,
        )
        .map_err(|e| format!("spawn `cargo check` failed: {e}"))?;
        if out.cancelled {
            return Err(format!(
                "cargo check cancelled\n{}",
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if out.timed_out {
            return Ok(format!(
                "check: {}\n{}",
                timeout_detail(&out),
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if !out.success {
            let code = out
                .exit
                .map(|exit| exit.to_string())
                .unwrap_or_else(|| "signal".to_string());
            return Err(format!(
                "cargo check failed (exit {code})\n{}",
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        let o = parse_lint(&out.stderr);
        let mut summary = format!(
            "check: {} warnings, {} errors — reward {:.2}",
            o.warnings,
            o.errors,
            o.reward()
        );
        if let Some(pin) = self.cargo.pin_note() {
            summary.push('\n');
            summary.push_str(&pin);
        }
        if !o.clean() {
            summary.push('\n');
            summary.push_str(&tail(&out.stderr, 1500));
        }
        Ok(targets::annotate(
            summary,
            &required,
            out.checked_sources.as_ref(),
            explicit,
        ))
    }
}

pub(crate) struct FmtTool {
    policy: SandboxPolicy,
    workspace: PathBuf,
    cargo: PinnedCargo,
}
impl FmtTool {
    #[cfg(test)]
    pub(crate) fn in_dir(dir: PathBuf) -> Self {
        let cargo = PinnedCargo::capture(&dir);
        Self::in_dir_with_cargo(dir, cargo)
    }

    pub(crate) fn in_dir_with_cargo(dir: PathBuf, cargo: PinnedCargo) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(dir.clone());
        Self {
            policy,
            workspace: dir,
            cargo,
        }
    }
}
impl Tool for FmtTool {
    fn name(&self) -> &str {
        "fmt"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "fmt".to_string(),
            description: "Run `cargo fmt` in the workspace (sandboxed). With check=true, runs \
                          `cargo fmt --check` and reports whether the tree is rustfmt-clean \
                          without modifying files; otherwise formats in place."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": { "check": { "type": "boolean", "description": "check only, don't modify (default false)" } },
                "required": [],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        self.call_with_cancel_and_progress(args, cancel, None)
    }

    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        let check = args["check"].as_bool().unwrap_or(false);
        std::fs::create_dir_all(&self.workspace)
            .map_err(|e| format!("workspace create failed: {e}"))?;
        let mut argv: Vec<&str> = vec!["fmt"];
        if check {
            argv.push("--check");
        }
        let out = sandboxed_cargo_output(
            &self.cargo,
            &argv,
            &self.workspace,
            &self.policy,
            cancel,
            progress,
            check,
        )
        .map_err(|e| format!("spawn `cargo fmt` failed: {e}"))?;
        if out.cancelled {
            return Err(format!(
                "cargo fmt cancelled\n{}",
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if out.timed_out {
            return Ok(format!(
                "fmt: {}\n{}",
                timeout_detail(&out),
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if check {
            if out.success {
                Ok("fmt: the tree is already rustfmt-clean".to_string())
            } else {
                Ok(format!(
                    "fmt: needs formatting\n{}",
                    tail(&out.stdout, 1500)
                ))
            }
        } else if out.success {
            Ok("fmt: workspace formatted (cargo fmt)".to_string())
        } else {
            Err(format!("cargo fmt failed: {}", tail(&out.stderr, 1500)))
        }
    }
}

#[cfg(test)]
pub(crate) fn resolve_on_path_for_test(program: &str) -> Option<PathBuf> {
    resolve_on_path(program)
}

/// Prefix assignments affect only the child, never the process environment.
fn apply_verifier_env(command: &mut std::process::Command, args: &Value) -> Result<(), String> {
    let Some(env) = args.get("env") else {
        return Ok(());
    };
    let env = env.as_object().ok_or("verifier env must be an object")?;
    for (key, value) in env {
        if !crate::agent::harness::shell_verifier::valid_env_key(key)
            || crate::agent::harness::shell_verifier::protected_env_key(key)
        {
            return Err(format!("unsupported verifier environment name: {key}"));
        }
        let value = value
            .as_str()
            .ok_or("verifier environment values must be strings")?;
        if value.contains('\0') {
            return Err("verifier environment contains NUL".into());
        }
        command.env(key, value);
    }
    Ok(())
}
