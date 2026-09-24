//! Sandbox posture, landlock write confinement, and YOLO bypasses for children.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory. Process timeout and
//! sealed-env coverage already live in `sandbox_exec.rs`.

use super::*;

// --- sandbox policy / landlock suite ----------------------------------------

#[test]
fn shell_tool_runs_under_sandbox() {
    let _lock = crate::tests::env_lock();
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping shell sandbox test");
        return;
    }
    // Arm enforcement explicitly — permissive() follows ANGEL_SANDBOX,
    // which defaults to off; this test is about the armed path.
    let mut policy = SandboxPolicy::permissive();
    policy.enforce = true;
    let out = run_sandboxed("sh", &["-c", "echo hello-sandbox"], None, &policy).unwrap();
    #[cfg(target_os = "linux")]
    if !out.contains("hello-sandbox") {
        eprintln!("{}", sandbox::bwrap_diagnostic_plan(&policy));
    }
    assert!(out.contains("hello-sandbox"), "got: {out}");
}

/// An exported shared cargo build directory is writable build output, like the
/// caches; `/`, the home directory and relative paths are never granted.
#[test]
fn permissive_sandbox_grants_an_exported_cargo_target_dir() {
    let _guard = crate::tests::env_lock();
    let root = scratch("exported_cargo_target");
    let target = root.join("target");
    let home = root.join("home");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let _home = EnvGuard::set("HOME", home.to_str().unwrap());
    let _build = EnvGuard::unset("CARGO_BUILD_TARGET_DIR");
    let _target = EnvGuard::set("CARGO_TARGET_DIR", target.to_str().unwrap());
    assert!(SandboxPolicy::permissive().writable_roots.contains(&target));
    for denied in ["/", home.to_str().unwrap(), "relative/target"] {
        let _target = EnvGuard::set("CARGO_TARGET_DIR", denied);
        let roots = SandboxPolicy::permissive().writable_roots;
        assert!(!roots.contains(&PathBuf::from(denied)), "{denied}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn permissive_sandbox_allows_local_nvidia_compute_devices() {
    if !sandbox::available() || !Path::new("/dev/nvidiactl").exists() {
        eprintln!("landlock or NVIDIA devices unavailable; skipping GPU sandbox test");
        return;
    }
    let mut policy = SandboxPolicy::permissive();
    policy.enforce = true;
    let out = run_sandboxed(
        "nvidia-smi",
        &[
            "--query-gpu=index,name,memory.total",
            "--format=csv,noheader",
        ],
        None,
        &policy,
    )
    .expect("the ordinary coding sandbox must permit local NVIDIA compute access");
    assert!(!out.trim().is_empty(), "nvidia-smi returned no GPU rows");
}

#[test]
#[ignore = "requires a local NVIDIA GPU and CUDA driver"]
fn permissive_sandbox_allows_cuda_context_and_allocation() {
    if !sandbox::available() || !Path::new("/dev/nvidiactl").exists() {
        eprintln!("landlock or NVIDIA devices unavailable; skipping CUDA allocation test");
        return;
    }
    let mut policy = SandboxPolicy::permissive();
    policy.enforce = true;
    let script = r#"
import ctypes

cuda = ctypes.CDLL("libcuda.so.1")

def check(name, result):
    if result:
        raise RuntimeError(f"{name}: CUDA driver error {result}")

check("cuInit", cuda.cuInit(0))
device = ctypes.c_int()
check("cuDeviceGet", cuda.cuDeviceGet(ctypes.byref(device), 0))
context = ctypes.c_void_p()
check("cuCtxCreate", cuda.cuCtxCreate_v2(ctypes.byref(context), 0, device))
pointer = ctypes.c_uint64()
check("cuMemAlloc", cuda.cuMemAlloc_v2(ctypes.byref(pointer), 4096))
check("cuMemFree", cuda.cuMemFree_v2(pointer))
check("cuCtxDestroy", cuda.cuCtxDestroy_v2(context))
print("cuda-driver-ok")
"#;
    let out = run_sandboxed("python3", &["-c", script], None, &policy)
        .expect("the ordinary coding sandbox must permit CUDA context creation and allocation");
    assert!(
        out.contains("cuda-driver-ok"),
        "unexpected CUDA probe: {out}"
    );
}

#[test]
fn sandbox_armed_by_default_and_read_only_always_enforced() {
    let policy = SandboxPolicy::permissive();
    assert!(
        policy.enforce,
        "write-mode agents are confined unless ANGEL_SANDBOX explicitly disables it"
    );
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        assert!(
            !policy.writable_roots.contains(&home),
            "granting all of HOME lets one project write every sibling repository"
        );
    }
    if let Ok(cwd) = std::env::current_dir() {
        assert!(
            !policy.writable_roots.contains(&cwd),
            "process cwd is not the active workspace after /cd"
        );
    }
    // An unenforced policy applies as a no-op even where landlock would
    // reject the caller (e.g. bogus roots / already-restricted threads).
    let unconfined = SandboxPolicy {
        writable_roots: vec![PathBuf::from("/definitely/not/a/real/root")],
        allow_network: true,
        enforce: false,
        mandatory: false,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    };
    assert!(sandbox::apply(&unconfined).is_ok());
}

#[test]
fn yolo_disables_external_command_deadlines() {
    let _lock = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _timeout = EnvGuard::set("ANGEL_TOOL_TIMEOUT", "1");
    assert_eq!(tool_timeout(), None);
}

#[test]
fn disarmed_sandbox_spawns_unrestricted_children() {
    // enforce=false must mean a plain child: writes land outside the
    // policy's roots and no_new_privs is NOT set (setuid tools keep
    // working). NoNewPrivs=1 here would break `sudo` for the agent.
    let dir = std::env::temp_dir().join(format!("angel_nosbx_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let policy = SandboxPolicy {
        writable_roots: Vec::new(), // would deny everything if enforced
        allow_network: true,
        enforce: false,
        mandatory: false,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    };
    // The NoNewPrivs probe is Linux-only (/proc); macOS covers the same
    // disarmed guarantee with the free write alone.
    #[cfg(target_os = "linux")]
    let cmd = format!(
        "echo free > {}/free.txt && grep NoNewPrivs /proc/self/status",
        dir.display()
    );
    #[cfg(not(target_os = "linux"))]
    let cmd = format!("echo free > {}/free.txt", dir.display());
    let out = run_sandboxed("sh", &["-c", &cmd], None, &policy).unwrap();
    assert!(
        dir.join("free.txt").exists(),
        "disarmed child must write freely: {out}"
    );
    #[cfg(target_os = "linux")]
    assert!(
        out.contains("NoNewPrivs:\t0"),
        "disarmed child must not carry no_new_privs: {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn landlock_confines_writes_to_allowed_roots() {
    // Serialize against YOLO suite — unrestricted posture can weaken confinement claims.
    let _lock = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping enforcement test");
        return;
    }
    let base = std::env::temp_dir();
    let pid = std::process::id();
    let allowed = base.join(format!("angel_sbx_{pid}_allowed"));
    let denied = base.join(format!("angel_sbx_{pid}_denied"));
    let _ = std::fs::create_dir_all(&allowed);
    let _ = std::fs::create_dir_all(&denied);

    let policy = SandboxPolicy {
        writable_roots: vec![allowed.clone()],
        allow_network: true,
        enforce: true,
        mandatory: true,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    };
    let cmd = format!(
        "echo ok > {}/ok.txt; echo bad > {}/bad.txt",
        allowed.display(),
        denied.display(),
    );
    let probe = run_sandboxed("sh", &["-c", &cmd], None, &policy);
    if !allowed.join("ok.txt").exists() {
        eprintln!("DIAG probe result: {probe:?}");
    }

    assert!(
        allowed.join("ok.txt").exists(),
        "write to an allowed root should succeed"
    );
    assert!(
        !denied.join("bad.txt").exists(),
        "write outside allowed roots should be denied by landlock"
    );

    let _ = std::fs::remove_dir_all(&allowed);
    let _ = std::fs::remove_dir_all(&denied);
}
