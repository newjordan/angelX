//! K5 coverage: typed Cargo verification under YOLO with a pinned toolchain,
//! plus empty-diagnostic shell classification (mirrored in
//! scripts/trajectory-telemetry.py). Extracted into its own module so the
//! cargo_tool inventory stays focused on argv/version behavior.

use super::*;
use crate::agent::tools::build::CheckTool;
use crate::agent::tools::build::PinnedCargo;
use crate::agent::tools::build::RunTestsTool;

#[cfg(unix)]
fn write_executable(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, body).unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

fn cargo_workspace(root: &std::path::Path, name: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2021'\n"),
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn answer()->u8{42}\n#[cfg(test)]mod tests{#[test]fn answer_is_42(){assert_eq!(super::answer(),42);}}\n",
    )
    .unwrap();
}

/// A synthetic but complete toolchain whose cargo/rustc actually run. The pin
/// (path/size/sha256 captured at registry construction) is what these tests
/// exercise; the scripts only need to exit 0 with a passing summary.
fn synthetic_toolchain(root: &std::path::Path) -> std::path::PathBuf {
    let bin = root.join("k5-toolchain-bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_executable(
        &bin.join("cargo"),
        "#!/bin/sh\ncase \"$1\" in\n  test) printf 'test result: ok. 1 passed; 0 failed; 0 ignored\\n' ;;\n  check|clippy) printf 'Finished dev profile\\n' >&2 ;;\nesac\nexit 0\n",
    );
    write_executable(&bin.join("rustc"), "#!/bin/sh\nexit 0\n");
    write_executable(&bin.join("rustdoc"), "#!/bin/sh\nexit 0\n");
    for tool in ["cargo-clippy", "clippy-driver", "cargo-fmt", "rustfmt"] {
        write_executable(&bin.join(tool), "#!/bin/sh\nexit 0\n");
    }
    bin
}

#[cfg(unix)]
#[test]
fn k5_typed_cargo_verifier_is_accepted_under_yolo_with_a_pinned_toolchain() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("k5_yolo_pinned_accepted");
    cargo_workspace(&root, "k5-yolo-pinned");
    let bin = synthetic_toolchain(&root);
    let pinned = PinnedCargo::for_test_workspace(bin.join("cargo"), &root);

    let receipt = RunTestsTool::in_dir_with_cargo(root.clone(), pinned.clone())
        .call(&serde_json::json!({}))
        .expect("pinned toolchain keeps the typed verifier usable under YOLO");
    assert!(receipt.contains("tests: 1 passed, 0 failed"), "{receipt}");
    assert!(
        !receipt.contains("typed Cargo verification is disabled"),
        "{receipt}"
    );
    assert!(
        receipt.contains("verifier toolchain pin: cargo "),
        "receipt must carry the pin: {receipt}"
    );
    assert!(
        receipt.contains("(sha256 "),
        "receipt must carry the captured digest: {receipt}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn k5_yolo_still_refuses_workspace_toolchain_override_files() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("k5_yolo_toolchain_override");
    cargo_workspace(&root, "k5-yolo-override");
    std::fs::write(
        root.join("rust-toolchain.toml"),
        "[toolchain]\nchannel=\"1.0\"\n",
    )
    .unwrap();
    let bin = synthetic_toolchain(&root);
    let pinned = PinnedCargo::for_test_executable(bin.join("cargo"));

    let error = RunTestsTool::in_dir_with_cargo(root.clone(), pinned)
        .call(&serde_json::json!({}))
        .expect_err("workspace rust-toolchain.toml must refuse even under YOLO");
    assert!(
        error.contains("refuses workspace-controlled Cargo/toolchain semantics"),
        "{error}"
    );
    assert!(
        error.contains("rust-toolchain.toml"),
        "the refusal must name the file: {error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn k5_yolo_refuses_pin_drift_between_capture_and_dispatch() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("k5_yolo_pin_drift");
    cargo_workspace(&root, "k5-yolo-drift");
    let bin = synthetic_toolchain(&root);
    let pinned = PinnedCargo::for_test_workspace(bin.join("cargo"), &root);
    // Replace cargo after the pin was captured (same length, different bytes).
    write_executable(
        &bin.join("cargo"),
        "#!/bin/sh\nprintf 'cargo 2.0\\n'\nexit 0\n",
    );

    let error = RunTestsTool::in_dir_with_cargo(root.clone(), pinned)
        .call(&serde_json::json!({}))
        .expect_err("a changed toolchain binary must fail closed");
    assert!(
        error.contains("toolchain changed since pin") || error.contains("identity changed"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn k5_yolo_without_a_pin_still_refuses_instead_of_unlabeled() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("k5_yolo_no_pin");
    cargo_workspace(&root, "k5-yolo-no-pin");
    // An unparseable capture (no pinned executable at all) has no pin note;
    // the typed verifier must refuse rather than emit an unlabeled receipt.
    let pinned = PinnedCargo::for_test_executable(root.join("does-not-exist"));
    assert!(pinned.pin_note().is_none(), "no pin must be observable");
    let error = RunTestsTool::in_dir_with_cargo(root.clone(), pinned)
        .call(&serde_json::json!({}))
        .expect_err("an unpinnable toolchain must refuse");
    assert!(error.contains("trusted Cargo unavailable"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn k5_yolo_check_receipt_carries_the_verifier_toolchain_pin() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("k5_yolo_check_pin");
    cargo_workspace(&root, "k5-yolo-check-pin");
    let bin = synthetic_toolchain(&root);
    let pinned = PinnedCargo::for_test_workspace(bin.join("cargo"), &root);
    let receipt = CheckTool::in_dir_with_cargo(root.clone(), pinned)
        .call(&serde_json::json!({}))
        .expect("typed check on a pinned toolchain works under YOLO");
    assert!(
        receipt.contains("verifier toolchain pin: cargo "),
        "{receipt}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn k5b_local_and_ancestor_controls_are_pinned_and_drift_refused() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("k5b_controls");
    let child = root.join("crate");
    cargo_workspace(&child, "k5b-controls");
    let ancestor = root.join("rust-toolchain.toml");
    let local = child.join("rust-toolchain.toml");
    for path in [&ancestor, &local] {
        std::fs::write(
            path,
            "[toolchain]\nchannel='1.95.0'\ncomponents=['rustfmt']\n",
        )
        .unwrap();
    }
    let bin = synthetic_toolchain(&root);
    let pin = PinnedCargo::for_test_workspace(bin.join("cargo"), &child);
    let receipt = RunTestsTool::in_dir_with_cargo(child.clone(), pin.clone())
        .call(&serde_json::json!({}))
        .unwrap();
    assert!(receipt.contains("tests: 1 passed, 0 failed"), "{receipt}");
    let note = pin.pin_note().unwrap();
    let json: serde_json::Value =
        serde_json::from_str(note.split_once("\npin: ").unwrap().1).unwrap();
    let files = json["control_files"].as_array().unwrap();
    for path in [&ancestor, &local] {
        assert!(
            files.iter().any(|f| f["path"] == path.to_str().unwrap()
                && f["sha256"].as_str().unwrap().len() == 64)
        );
    }
    std::fs::write(&ancestor, "[toolchain]\nchannel='stable'\n").unwrap();
    let error = RunTestsTool::in_dir_with_cargo(child, pin)
        .call(&serde_json::json!({}))
        .unwrap_err();
    assert!(
        error.contains("changed since pin") && error.contains(ancestor.to_str().unwrap()),
        "{error}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn k5b_workspace_relative_rustc_is_refused() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("k5b_rustc");
    cargo_workspace(&root, "k5b-rustc");
    std::fs::create_dir_all(root.join(".cargo")).unwrap();
    std::fs::write(
        root.join(".cargo/config"),
        "[build]\nrustc='./fake-rustc'\n",
    )
    .unwrap();
    let bin = synthetic_toolchain(&root);
    let pin = PinnedCargo::for_test_workspace(bin.join("cargo"), &root);
    let error = RunTestsTool::in_dir_with_cargo(root.clone(), pin)
        .call(&serde_json::json!({}))
        .unwrap_err();
    assert!(
        error.contains(".cargo/config")
            && error.contains("build.rustc")
            && error.contains("task-writable"),
        "{error}"
    );
    std::fs::remove_dir_all(root).unwrap();
}
