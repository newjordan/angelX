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
            self_test_filter!(
                "sealed_override_merges_only_workspace_scoped_writable_roots"
            )
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
