/// This module compiles into both the cockpit and the `angel-sandbox` helper,
/// where it sits at a different path, so the libtest `--exact` filter for a
/// sibling test is derived from `module_path!()` rather than hard-coded.
macro_rules! self_test_filter {
    ($name:literal) => {
        format!(
            "{}::{}",
            module_path!()
                .split_once("::")
                .map_or(module_path!(), |(_crate, rest)| rest),
            $name
        )
    };
}

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
    std::fs::write(home.join(".angelX/KEY-sealed-test"), b"SEAL-SECRET-KEY").unwrap();
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
    for deny in [".ssh", ".angelX", ".config", ".gnupg", ".mozilla"] {
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
fn sealed_cargo_payload_grants_exclude_default_and_custom_home_credentials() {
    struct RestoreCargoHome(Option<std::ffi::OsString>);
    impl Drop for RestoreCargoHome {
        fn drop(&mut self) {
            // This fixture holds the crate's environment lock until restore.
            unsafe {
                match &self.0 {
                    Some(value) => std::env::set_var("CARGO_HOME", value),
                    None => std::env::remove_var("CARGO_HOME"),
                }
            }
        }
    }
    let _guard = crate::tests::env_lock();
    let home = fake_home("cargo-credentials").canonicalize().unwrap();
    let ws = fake_workspace("cargo-credentials").canonicalize().unwrap();
    let cargo_home = fake_workspace("custom-cargo-home").canonicalize().unwrap();
    for dir in ["bin", "registry", "git"] {
        std::fs::create_dir_all(cargo_home.join(dir)).unwrap();
    }
    let _cargo_home = RestoreCargoHome(std::env::var_os("CARGO_HOME"));
    // Serialized by the environment lock; the helper test target shares this fixture.
    unsafe { std::env::set_var("CARGO_HOME", &cargo_home) };
    let mut credentials = Vec::new();
    for root in [home.join(".cargo"), cargo_home.clone()] {
        for name in ["credentials", "credentials.toml"] {
            let path = root.join(name);
            std::fs::write(&path, b"sealed fixture credential").unwrap();
            credentials.push(path);
        }
    }

    let profile = build(&ws, Some(&home));
    assert!(validate_policy(&profile.policy).is_ok());
    for root in [home.join(".cargo"), cargo_home] {
        for dir in ["bin", "registry", "git"] {
            assert!(profile.read_roots.contains(&root.join(dir)));
        }
        assert!(!profile.read_roots.contains(&root));
    }
    for credential in credentials {
        assert!(profile.deny_roots.contains(&credential));
        assert!(profile.read_roots.iter().all(|root| !credential.starts_with(root)));
    }
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
    struct RestoreYolo(Option<std::ffi::OsString>);
    impl Drop for RestoreYolo {
        fn drop(&mut self) {
            unsafe {
                match &self.0 {
                    Some(value) => std::env::set_var("ANGEL_YOLO", value),
                    None => std::env::remove_var("ANGEL_YOLO"),
                }
            }
        }
    }
    let _yolo = RestoreYolo(std::env::var_os("ANGEL_YOLO"));
    unsafe { std::env::set_var("ANGEL_YOLO", "1") };
    let err = activate(build(&fake_workspace("yolo"), None)).expect_err("must refuse YOLO");
    #[cfg(target_os = "linux")]
    assert!(
        err.contains("YOLO cannot widen a sealed sandbox profile"),
        "{err}"
    );
    #[cfg(not(target_os = "linux"))]
    assert!(err.contains("requires Linux"), "{err}");
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
            self_test_filter!("sealed_override_merges_only_workspace_scoped_writable_roots")
                .as_str(),
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
    super::super::set_helper_policy(&mut cmd, &readonly).unwrap();
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
    assert_eq!(vars[super::super::HELPER_BACKEND_ENV], "bwrap");
    let encoded: SandboxPolicy =
        serde_json::from_str(&vars[super::super::HELPER_POLICY_ENV]).unwrap();
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

#[test]
#[cfg(target_os = "linux")]
fn sealed_runtime_activation_allows_toolchain_and_denies_credentials() {
    struct TestEnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }
    impl TestEnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            // This exact child holds env_lock; restoration also runs on unwind.
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }
    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    let _guard = crate::tests::env_lock();
    const CHILD: &str = "ANGEL_T_SEALED_RUNTIME_CHILD";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            self_test_filter!("sealed_runtime_activation_allows_toolchain_and_denies_credentials")
                .as_str(),
            "--nocapture",
        ]);
        command.env(CHILD, "1");
        let output = command.output().expect("spawn isolated sealed runtime test");
        assert!(
            output.status.success(),
            "sealed runtime child failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    // ACTIVE is process-sticky. This exact child starts with an empty OnceLock.
    assert!(identity().is_none(), "runtime child must start unactivated");
    // The helper this run executes: a qualified runner pins it with
    // ANGEL_T_SANDBOX_HELPER, and a plain `cargo test` gets the one the sandbox
    // builds itself. Resolve it before the fixture swaps HOME and CARGO_HOME.
    let helper = super::super::helper_executable().expect("sandbox helper for this run");
    let home = fake_home("runtime").canonicalize().unwrap();
    let workspace = fake_workspace("runtime").canonicalize().unwrap();
    let cargo_home = fake_workspace("runtime-cargo-home").canonicalize().unwrap();
    let path = std::env::var_os("PATH").expect("qualified PATH");
    let path_string = path.to_string_lossy().into_owned();
    for dir in ["bin", "registry", "git"] {
        std::fs::create_dir_all(cargo_home.join(dir)).unwrap();
    }

    let _home = TestEnvGuard::set("HOME", home.to_str().unwrap());
    let _cargo = TestEnvGuard::set("CARGO_HOME", cargo_home.to_str().unwrap());
    let _path = TestEnvGuard::set("PATH", &path_string);
    let _yolo = TestEnvGuard::set("ANGEL_YOLO", "0");
    let _smart = TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
    let _sandbox = TestEnvGuard::set("ANGEL_SANDBOX", "1");

    let mut marker_paths = Vec::new();
    for (label, root) in [
        ("default", home.join(".cargo")),
        ("custom", cargo_home.clone()),
    ] {
        for dir in ["bin", "registry", "git"] {
            let value = format!("f01-{label}-{dir}-marker-v1");
            let marker = root.join(dir).join("sealed-runtime-marker");
            std::fs::write(&marker, value.as_bytes()).unwrap();
            marker_paths.push(marker);
        }
    }
    let mut credentials = Vec::new();
    for (label, root) in [
        ("default", home.join(".cargo")),
        ("custom", cargo_home.clone()),
    ] {
        for name in ["credentials", "credentials.toml"] {
            let credential = root.join(name);
            std::fs::write(
                &credential,
                format!("synthetic-f01-{label}-{name}-must-not-be-readable"),
            )
            .unwrap();
            credentials.push(credential);
        }
    }
    let hashes_before: Vec<_> = marker_paths
        .iter()
        .chain(&credentials)
        .map(|path| super::super::sha256_hex(&std::fs::read(path).unwrap()))
        .collect();

    let helper = std::fs::canonicalize(helper).expect("sandbox helper path");
    assert!(std::fs::metadata(&helper).unwrap().is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(std::fs::metadata(&helper).unwrap().permissions().mode() & 0o111, 0);
    }

    // Exercise the public HOME/CARGO_HOME discovery branch used by startup.
    activate(build(&workspace, None)).expect("activate public sealed profile");

    // Start with the ordinary caller posture. The public command constructor
    // must replace it with ACTIVE's mandatory, netless allow-list policy.
    let mut ordinary = super::super::SandboxPolicy::permissive();
    ordinary.writable_roots.push(workspace.clone());
    assert!(ordinary.enforce);
    assert!(!ordinary.mandatory);
    assert!(ordinary.allow_network);
    assert!(ordinary.sealed_reads.is_empty());
    let mut command = super::super::command(
        "python3",
        std::iter::once("-c".to_owned())
            .chain(std::iter::once(SEALED_RUNTIME_PAYLOAD.to_owned()))
            .chain(marker_paths.iter().map(|path| path.to_string_lossy().into_owned()))
            .chain(credentials.iter().map(|path| path.to_string_lossy().into_owned())),
        &ordinary,
    )
    .expect("construct production sandbox command");

    // Keep synthetic fixture paths, locale-independent tool lookup, and the
    // test flags, while removing every ambient credential-bearing variable.
    command.env_clear();
    command
        .env("HOME", &home)
        .env("CARGO_HOME", &cargo_home)
        .env("PATH", &path)
        .env("ANGEL_YOLO", "0")
        .env("ANGEL_YOLO_SMART", "0");
    super::super::set_helper_policy(&mut command, &ordinary)
        .expect("apply active sealed override after clearing child env");

    let encoded = command
        .get_envs()
        .find_map(|(key, value)| {
            (key == super::super::HELPER_POLICY_ENV)
                .then(|| value.unwrap().to_string_lossy())
        })
        .expect("serialized helper policy");
    let effective: super::super::SandboxPolicy = serde_json::from_str(&encoded).unwrap();
    assert!(!effective.allow_network);
    assert!(effective.enforce && effective.mandatory);
    assert!(!effective.sealed_reads.is_empty());
    for credential in &credentials {
        assert!(effective.deny_reads.contains(credential));
    }

    let output = command.output().expect("run qualified bwrap payload");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in [
        "synthetic-f01-default-credentials-must-not-be-readable",
        "synthetic-f01-default-credentials.toml-must-not-be-readable",
        "synthetic-f01-custom-credentials-must-not-be-readable",
        "synthetic-f01-custom-credentials.toml-must-not-be-readable",
    ] {
        assert!(!stdout.contains(secret));
        assert!(!stderr.contains(secret));
    }
    assert!(
        output.status.success(),
        "sandboxed runtime probe failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "sealed-runtime-allow-deny-ok");
    let hashes_after: Vec<_> = marker_paths
        .iter()
        .chain(&credentials)
        .map(|path| super::super::sha256_hex(&std::fs::read(path).unwrap()))
        .collect();
    assert_eq!(hashes_after, hashes_before, "sandbox probe modified fixtures");
}

#[cfg(target_os = "linux")]
const SEALED_RUNTIME_PAYLOAD: &str = r#"
import sys
expected = [
    b"f01-default-bin-marker-v1",
    b"f01-default-registry-marker-v1",
    b"f01-default-git-marker-v1",
    b"f01-custom-bin-marker-v1",
    b"f01-custom-registry-marker-v1",
    b"f01-custom-git-marker-v1",
]
for path, value in zip(sys.argv[1:7], expected, strict=True):
    with open(path, "rb") as marker:
        assert marker.read() == value
for path in sys.argv[7:11]:
    try:
        with open(path, "rb"):
            raise AssertionError("credential unexpectedly readable")
    except OSError:
        pass
print("sealed-runtime-allow-deny-ok")
"#;
