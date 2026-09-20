//! Cargo tool version and nonzero-exit coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;
use crate::tools::build::{PinnedCargo, bounded_rustup_cargo_path, parse_direct_argv};

#[cfg(unix)]
fn write_executable(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, body).unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
#[test]
fn pinned_cargo_rustup_resolution_has_a_fixed_preflight_deadline() {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "sleep 30 & wait"]);
    let started = std::time::Instant::now();

    let path = bounded_rustup_cargo_path(command, Duration::from_millis(50));

    assert!(path.is_none());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "hung rustup resolver outlived its preflight deadline: {:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn pinned_cargo_deduplicates_resolver_aliases_before_eager_capture() {
    let _lock = crate::tests::env_lock();
    let root = scratch("pinned_cargo_rustup_alias_budget");
    let workspace = root.join("workspace");
    let resolver_bin = root.join("resolver-bin");
    let cargo_home_bin = root.join("cargo-home/bin");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&resolver_bin).unwrap();
    std::fs::create_dir_all(&cargo_home_bin).unwrap();
    let marker = root.join("resolver-invocations");
    let resolver = resolver_bin.join("rustup");
    write_executable(
        &resolver,
        &format!("#!/bin/sh\nprintf x >> '{}'\nsleep 30\n", marker.display()),
    );
    std::os::unix::fs::symlink(&resolver, cargo_home_bin.join("rustup")).unwrap();
    let _cargo = crate::tests::TestEnvGuard::set("CARGO", resolver.to_str().unwrap());
    let _cargo_home =
        crate::tests::TestEnvGuard::set("CARGO_HOME", root.join("cargo-home").to_str().unwrap());
    let _pinned = PinnedCargo::capture(&workspace);

    // The deadline itself is exercised above with a deliberately hung process.
    // Do not time the whole capture here: registry construction now hashes the
    // complete toolchain eagerly, and debug-mode digest cost is unrelated to
    // whether canonical resolver aliases were deduplicated.
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap(),
        "x",
        "the same canonical rustup resolver must run exactly once"
    );
    let _ = std::fs::remove_dir_all(root);
}

// --- cargo tool suite ---

#[test]
fn cargo_tool_reports_version() {
    if !cfg!(target_os = "linux") {
        eprintln!(
            "SKIP cargo_tool_reports_version: Landlock unavailable on {}",
            std::env::consts::OS
        );
        return;
    }
    if !sandbox::available() {
        eprintln!("SKIP cargo_tool_reports_version: Landlock");
        return;
    }
    let _lock = crate::tests::env_lock();
    let out = CargoTool::in_dir(default_workspace())
        .call(&serde_json::json!({ "args": "--version" }))
        .unwrap();
    assert!(out.contains("cargo"), "got: {out}");
    assert!(out.contains("[cargo verdict: pass]"), "got: {out}");
}

#[test]
fn cargo_tool_surfaces_nonzero_exit_as_a_tool_error() {
    if !cfg!(target_os = "linux") {
        eprintln!(
            "SKIP cargo_tool_surfaces_nonzero_exit_as_a_tool_error: Landlock unavailable on {}",
            std::env::consts::OS
        );
        return;
    }
    if !sandbox::available() {
        eprintln!("SKIP cargo_tool_surfaces_nonzero_exit_as_a_tool_error: Landlock");
        return;
    }
    let _lock = crate::tests::env_lock();
    let error = CargoTool::in_dir(default_workspace())
        .call(&serde_json::json!({ "args": "definitely-not-a-real-subcommand" }))
        .unwrap_err();
    assert!(error.contains("cargo command failed (exit"), "got: {error}");
    assert!(error.contains("no such command"), "got: {error}");
}

#[test]
fn raw_cargo_explicit_manifest_resolves_from_the_caller_workspace() {
    if !sandbox::available() {
        eprintln!("sandbox unavailable; skipping explicit Cargo manifest test");
        return;
    }
    let _env = crate::tests::env_lock();
    let root = scratch("cargo_explicit_manifest");
    let manifest = root.join("nested crate/Cargo.toml");
    std::fs::create_dir_all(root.join("nested crate/src")).unwrap();
    std::fs::write(
        &manifest,
        "[package]\nname='manifest-route-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(root.join("nested crate/src/lib.rs"), "pub fn marker() {}\n").unwrap();
    let cargo = CargoTool::in_dir(root.clone());
    for flag in [
        "--manifest-path 'nested crate/Cargo.toml'".to_string(),
        "--manifest-path='nested crate/Cargo.toml'".to_string(),
        format!("--manifest-path '{}'", manifest.display()),
    ] {
        let args = format!("metadata --offline --no-deps --format-version 1 {flag}");
        let output = cargo
            .call(&serde_json::json!({"args": args}))
            .unwrap_or_else(|error| panic!("{flag}: {error}"));
        assert!(
            output.contains("manifest-route-fixture"),
            "{flag}: {output}"
        );
        assert!(output.contains("[cargo verdict: pass]"), "{flag}: {output}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn direct_cargo_argv_preserves_quotes_escapes_and_literal_shell_syntax() {
    assert_eq!(
        parse_direct_argv(
            r#"run -- "two words" 'three words' escaped\ space "" '$HOME' '*.rs' '$(nope)' ';'"#
        )
        .unwrap(),
        vec![
            "run",
            "--",
            "two words",
            "three words",
            "escaped space",
            "",
            "$HOME",
            "*.rs",
            "$(nope)",
            ";",
        ]
    );
    assert_eq!(
        parse_direct_argv(r#"test pre"joined value"post "✓""#).unwrap(),
        vec!["test", "prejoined valuepost", "✓"]
    );
}

#[test]
fn direct_cargo_argv_fails_closed_on_malformed_or_excessive_input() {
    assert_eq!(
        parse_direct_argv(r#"run "unterminated"#).unwrap_err(),
        "unterminated double quote"
    );
    assert_eq!(
        parse_direct_argv("run trailing\\").unwrap_err(),
        "trailing backslash escape"
    );
    assert!(
        parse_direct_argv(&"x ".repeat(257))
            .unwrap_err()
            .contains("more than 256 arguments")
    );
    assert!(
        parse_direct_argv(&"x".repeat(64 * 1024 + 1))
            .unwrap_err()
            .contains("65536-byte limit")
    );
}

#[test]
fn cargo_tool_delivers_a_quoted_program_argument_as_one_exact_argv_item() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping quoted Cargo argv integration test");
        return;
    }
    let _lock = crate::tests::env_lock();
    let root = scratch("quoted_cargo_argv");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"quoted-argv\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"ARG={}\", std::env::args().nth(1).unwrap()); }\n",
    )
    .unwrap();

    let output = CargoTool::in_dir(root.clone())
        .call(&serde_json::json!({"args": "run --quiet -- \"two words\""}))
        .expect("quoted cargo run");
    assert!(output.contains("ARG=two words"), "{output}");
    assert!(!output.contains("ARG=\"two"), "{output}");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn typed_cargo_and_check_ignore_hostile_path_and_replaced_rustup_proxy() {
    let _lock = crate::tests::env_lock();
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping pinned Cargo integration test");
        return;
    }
    // The test helper itself is built through Cargo; prime it before poisoning
    // process PATH so this regression measures the typed build tool only.
    sandbox::prime_helper();

    let root = scratch("pinned_cargo_hostile_path");
    let workspace = root.join("workspace");
    let hostile = workspace.join("hostile-bin");
    let cargo_home = workspace.join("cargo-home");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::create_dir_all(&hostile).unwrap();
    std::fs::create_dir_all(cargo_home.join("bin")).unwrap();
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname='pinned-cargo-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(workspace.join("src/lib.rs"), "pub fn answer()->u8{42}\n").unwrap();

    let cargo_marker = workspace.join("HOSTILE_CARGO_RAN");
    write_executable(
        &hostile.join("cargo"),
        &format!(
            "#!/bin/sh\nprintf hostile > '{}'\nprintf 'cargo hostile\\n'\nexit 0\n",
            cargo_marker.display()
        ),
    );

    let real_rustup = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".cargo/bin/rustup"))
        .and_then(|path| std::fs::canonicalize(path).ok())
        .expect("test host needs rustup");
    std::os::unix::fs::symlink(&real_rustup, cargo_home.join("bin/rustup")).unwrap();

    let hostile_path = format!("{}:/usr/local/bin:/usr/bin:/bin", hostile.display());
    let _cargo = crate::tests::TestEnvGuard::unset("CARGO");
    let _cargo_home = crate::tests::TestEnvGuard::set(
        "CARGO_HOME",
        cargo_home.to_str().expect("UTF-8 scratch path"),
    );
    let _path = crate::tests::TestEnvGuard::set("PATH", &hostile_path);

    let pinned = PinnedCargo::capture(&workspace);
    let pinned_path = pinned.executable_path().expect("pin real Cargo");
    assert_ne!(pinned_path, hostile.join("cargo"));
    assert!(!path_within_test(&pinned_path, &workspace));

    // Replace the resolver after capture. Typed tools must never consult it or
    // PATH again during this live registry.
    std::fs::remove_file(cargo_home.join("bin/rustup")).unwrap();
    write_executable(
        &cargo_home.join("bin/rustup"),
        &format!(
            "#!/bin/sh\nprintf rustup > '{}'\nexit 0\n",
            workspace.join("HOSTILE_RUSTUP_RAN").display()
        ),
    );
    // While the replaced shim is live, hold the capture lock so a concurrent
    // registry construction cannot resolve (and execute) it — that would be a
    // foreign capture, not this registry reusing its pin, and the marker
    // assert must only ever trip on a real dispatch-time reuse.
    let _capture = crate::tools::build::capture_lock();

    let cargo_output = CargoTool::in_dir_with_cargo(workspace.clone(), pinned.clone())
        .call(&serde_json::json!({"args":"--version"}))
        .expect("pinned generic cargo");
    assert!(cargo_output.contains("cargo 1."), "{cargo_output}");
    let check_output = CheckTool::in_dir_with_cargo(workspace.clone(), pinned)
        .call(&serde_json::json!({}))
        .expect("pinned curated check");
    assert!(
        check_output.contains("check: 0 warnings, 0 errors"),
        "{check_output}"
    );
    assert!(!cargo_marker.exists(), "workspace PATH cargo shim executed");
    assert!(
        !workspace.join("HOSTILE_RUSTUP_RAN").exists(),
        "captured registry reused a replaced rustup proxy"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn pinned_cargo_rejects_replacement_before_first_dispatch() {
    let root = scratch("pinned_cargo_pre_dispatch_replacement");
    let bin = root.join("toolchain-bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_executable(&bin.join("cargo"), "#!/bin/sh\nprintf 'cargo 1.0\\n'\n");
    write_executable(&bin.join("rustc"), "#!/bin/sh\nexit 0\n");

    let pinned = PinnedCargo::for_test_executable(bin.join("cargo"));
    // Keep the replacement the same length: an early path/length-only pin is
    // insufficient, while the captured inode timestamps must fail closed even
    // before the lazy whole-binary digest has been established.
    write_executable(&bin.join("cargo"), "#!/bin/sh\nprintf 'cargo 2.0\\n'\n");

    let error = pinned.verify_for_test().unwrap_err();
    assert!(
        error.contains("identity changed") || error.contains("captured sha256"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
fn path_within_test(path: &std::path::Path, root: &std::path::Path) -> bool {
    path == root || path.starts_with(root)
}

#[cfg(unix)]
#[test]
fn green_looking_nonzero_cargo_processes_are_never_typed_green() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping nonzero Cargo integration test");
        return;
    }
    let root = scratch("pinned_cargo_nonzero_green_text");
    let bin = root.join("toolchain-bin");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname='fake-cargo-exit-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    write_executable(
        &bin.join("cargo"),
        "#!/bin/sh\ncase \"$1\" in\n  test) printf 'test result: ok. 9 passed; 0 failed; 0 ignored\\n' ;;\n  check) printf 'Finished dev profile\\n' >&2 ;;\n  clippy) printf 'Finished dev profile\\n' >&2 ;;\nesac\nexit 7\n",
    );
    write_executable(&bin.join("rustc"), "#!/bin/sh\nexit 0\n");
    write_executable(&bin.join("rustdoc"), "#!/bin/sh\nexit 0\n");
    // PinnedCargo now binds every external Cargo subcommand and its backing
    // driver, even though this fake `cargo` exits before dispatching them. Keep
    // the adversarial toolchain complete so the assertion continues to test
    // pass-shaped output plus a nonzero Cargo status—not a missing component.
    for tool in ["cargo-clippy", "clippy-driver", "cargo-fmt", "rustfmt"] {
        write_executable(&bin.join(tool), "#!/bin/sh\nexit 0\n");
    }
    let pinned = PinnedCargo::for_test_executable(bin.join("cargo"));

    let cases: Vec<(Box<dyn Tool>, Value, &str)> = vec![
        (
            Box::new(RunTestsTool::in_dir_with_cargo(
                workspace.clone(),
                pinned.clone(),
            )),
            serde_json::json!({}),
            "cargo test failed (exit 7)",
        ),
        (
            Box::new(CheckTool::in_dir_with_cargo(
                workspace.clone(),
                pinned.clone(),
            )),
            serde_json::json!({}),
            "cargo check failed (exit 7)",
        ),
        (
            Box::new(LintTool::in_dir_with_cargo(
                workspace.clone(),
                pinned.clone(),
            )),
            serde_json::json!({}),
            "cargo clippy failed (exit 7)",
        ),
        (
            Box::new(CargoTool::in_dir_with_cargo(workspace.clone(), pinned)),
            serde_json::json!({"args":"check"}),
            "cargo command failed (exit 7)",
        ),
    ];
    for (tool, args, expected) in cases {
        let error = tool.call(&args).expect_err(tool.name());
        assert!(error.contains(expected), "{}: {error}", tool.name());
        let call = ToolCall {
            id: format!("{}-nonzero", tool.name()),
            name: tool.name().to_string(),
            args,
        };
        assert_eq!(
            verification_outcome(&call, &format!("tool error: {error}")),
            Some(VerificationOutcome::Failed),
            "{} nonzero process must be verifier-red",
            tool.name()
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(all(unix, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn curated_tests_ignore_workspace_cargo_runner_and_wrapper_config() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping Cargo-config isolation test");
        return;
    }
    let _lock = crate::tests::env_lock();
    let root = scratch("curated_cargo_config_isolation");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join(".cargo")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='cargo-config-isolation'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn answer()->u8{42}\n#[cfg(test)]mod tests{#[test]fn real_test_binary_ran(){assert_eq!(super::answer(),42);}}\n",
    )
    .unwrap();
    let wrapper_marker = root.join("WORKSPACE_WRAPPER_RAN");
    let runner_marker = root.join("WORKSPACE_RUNNER_RAN");
    let linker_marker = root.join("WORKSPACE_LINKER_RAN");
    let wrapper = root.join("fake-wrapper");
    let runner = root.join("fake-runner");
    let linker = root.join("fake-linker");
    write_executable(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf wrapper > '{}'\nexec \"$@\"\n",
            wrapper_marker.display()
        ),
    );
    write_executable(
        &runner,
        &format!(
            "#!/bin/sh\nprintf runner > '{}'\nprintf 'test result: ok. 999 passed; 0 failed; 0 ignored\\n'\nexit 0\n",
            runner_marker.display()
        ),
    );
    write_executable(
        &linker,
        &format!(
            "#!/bin/sh\nprintf linker > '{}'\nexit 1\n",
            linker_marker.display()
        ),
    );
    std::fs::write(
        root.join(".cargo/config.toml"),
        format!(
            "[build]\nrustc-wrapper = '{}'\n\n[target.x86_64-unknown-linux-gnu]\nrunner = '{}'\nlinker = '{}'\n",
            wrapper.display(),
            runner.display(),
            linker.display()
        ),
    )
    .unwrap();

    let error = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({}))
        .expect_err("workspace Cargo config must fail closed");
    assert!(
        error.contains("refuses workspace-controlled Cargo/toolchain semantics"),
        "{error}"
    );
    for marker in [wrapper_marker, runner_marker, linker_marker] {
        assert!(
            !marker.exists(),
            "workspace Cargo configuration executed {}",
            marker.display()
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn curated_verifier_rejects_workspace_cargo_semantic_config() {
    let _lock = crate::tests::env_lock();
    let root = scratch("curated_cargo_semantic_config");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join(".cargo")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='cargo-semantic-config'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn answer()->u8{42}\n").unwrap();
    std::fs::write(
        root.join(".cargo/config.toml"),
        "[build]\nrustflags=['-Dwarnings']\n",
    )
    .unwrap();
    let call = CheckTool::in_dir(root.clone());
    let error = call
        .call(&serde_json::json!({}))
        .expect_err("semantic Cargo config must not be silently ignored");
    assert!(
        error.contains("refuses workspace-controlled Cargo/toolchain semantics"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn structured_cargo_verifiers_publish_progress_without_changing_verdicts() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping structured verifier progress test");
        return;
    }
    // The verifier gate reads process-global ANGEL_YOLO while other tests
    // hold the env lock to flip it — a reader without the lock raced them
    // under parallel execution ("typed Cargo verification is disabled…").
    let _env = crate::tests::env_lock();
    let root = scratch("structured_verifier_progress");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"progress-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn answer()->u8{42}\n#[cfg(test)]mod tests{#[test]fn answer_is_42(){assert_eq!(super::answer(),42);}}\n",
    )
    .unwrap();

    type CapturedProcessChunks = Vec<(ProcessStream, Vec<u8>)>;

    fn call_with_chunks(
        tool: &dyn Tool,
        args: Value,
    ) -> (Result<String, String>, CapturedProcessChunks) {
        let chunks = Arc::new(Mutex::new(Vec::new()));
        let progress_chunks = Arc::clone(&chunks);
        let progress: Arc<ToolOutputProgress> = Arc::new(move |stream, bytes| {
            progress_chunks
                .lock()
                .unwrap()
                .push((stream, bytes.to_vec()));
        });
        let result = tool.call_with_cancel_and_progress(&args, None, Some(progress));
        let chunks = Arc::try_unwrap(chunks)
            .expect("progress readers should release the sink after dispatch")
            .into_inner()
            .unwrap();
        (result, chunks)
    }

    let cases: Vec<(Box<dyn Tool>, Value, &str)> = vec![
        (
            Box::new(CheckTool::in_dir(root.clone())),
            serde_json::json!({"args": "--package \"progress-fixture\""}),
            "check: 0 warnings, 0 errors",
        ),
        (
            Box::new(RunTestsTool::in_dir(root.clone())),
            serde_json::json!({}),
            "tests: 1 passed, 0 failed",
        ),
        (
            Box::new(LintTool::in_dir(root.clone())),
            serde_json::json!({}),
            "lint: 0 warnings, 0 errors",
        ),
        (
            Box::new(FmtTool::in_dir(root.clone())),
            serde_json::json!({"check": true}),
            "fmt: needs formatting",
        ),
    ];
    for (tool, args, expected) in cases {
        let name = tool.name().to_string();
        let (result, chunks) = call_with_chunks(tool.as_ref(), args);
        let receipt = result.unwrap_or_else(|error| panic!("{name} failed: {error}"));
        assert!(receipt.contains(expected), "{name}: {receipt}");
        assert!(
            chunks.iter().any(|(_, bytes)| !bytes.is_empty()),
            "{name} emitted no live verifier output"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_tests_dispatches_node_test_on_a_js_workspace() {
    // PinnedCargo::capture reads process-global CARGO_HOME/PATH that sibling
    // tests flip under the env lock; hold it so the resolver-dedup marker test
    // never sees this test's capture (it failed twice in full-suite runs).
    let _env = crate::tests::env_lock();
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping node run_tests integration test");
        return;
    }
    if crate::tools::build::resolve_on_path_for_test("node").is_none() {
        eprintln!("node unavailable; skipping node run_tests integration test");
        return;
    }
    let root = scratch("run_tests_node_dispatch");
    std::fs::write(root.join("index.mjs"), "export const add=(a,b)=>a+b;\n").unwrap();
    std::fs::write(
        root.join("test.mjs"),
        "import test from 'node:test';import assert from 'node:assert';import {add} from './index.mjs';\ntest('adds',()=>assert.equal(add(1,2),3));\ntest('fails',()=>assert.equal(add(1,2),4));\n",
    )
    .unwrap();
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({}))
        .expect_err("a failing fallback runner preserves its unsuccessful exit");
    assert!(
        receipt.starts_with("node --test failed (exit 1)"),
        "{receipt}"
    );
    assert!(
        receipt.lines().any(|line| line.ends_with("pass 1")),
        "{receipt}"
    );
    assert!(
        receipt.lines().any(|line| line.ends_with("fail 1")),
        "{receipt}"
    );
    assert!(!receipt.contains("reward 0.50"), "{receipt}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_tests_dispatches_unittest_on_a_python_workspace() {
    // PinnedCargo::capture reads process-global CARGO_HOME/PATH that sibling
    // tests flip under the env lock; hold it so the resolver-dedup marker test
    // never sees this test's capture (it failed twice in full-suite runs).
    let _env = crate::tests::env_lock();
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping python run_tests integration test");
        return;
    }
    if crate::tools::build::resolve_on_path_for_test("python3").is_none() {
        eprintln!("python3 unavailable; skipping python run_tests integration test");
        return;
    }
    let root = scratch("run_tests_py_dispatch");
    std::fs::write(root.join("lib.py"), "def add(a,b):\n    return a+b\n").unwrap();
    std::fs::write(
        root.join("test_lib.py"),
        "import unittest\nfrom lib import add\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(1,2),3)\n    def test_two(self):\n        self.assertEqual(add(2,2),4)\n",
    )
    .unwrap();
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({}))
        .expect("unittest discover produces a receipt");
    assert!(receipt.contains("tests: 2 passed, 0 failed"), "{receipt}");
    assert!(receipt.contains("reward unlabeled"), "{receipt}");
    assert!(!receipt.contains("reward 1.00"), "{receipt}");
    assert!(receipt.contains("unittest"), "{receipt}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_tests_names_the_missing_runner_instead_of_trying_cargo() {
    // PinnedCargo::capture reads process-global CARGO_HOME/PATH that sibling
    // tests flip under the env lock; hold it so the resolver-dedup marker test
    // never sees this test's capture (it failed twice in full-suite runs).
    let _env = crate::tests::env_lock();
    let root = scratch("run_tests_no_runner");
    std::fs::write(root.join("README.md"), "nothing to test\n").unwrap();
    let error = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({}))
        .expect_err("a workspace with no markers is an explicit error, not a cargo attempt");
    assert!(error.contains("no test runner detected"), "{error}");
    assert!(!error.contains("cargo test failed"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_tests_finds_the_one_nested_crate_like_angel0_cockpit() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping nested-crate run_tests test");
        return;
    }
    let _env = crate::tests::env_lock();
    let root = scratch("run_tests_nested_crate");
    std::fs::create_dir_all(root.join("cockpit/src")).unwrap();
    std::fs::write(root.join("README.md"), "crate lives under cockpit/\n").unwrap();
    std::fs::write(
        root.join("cockpit/Cargo.toml"),
        "[package]\nname = \"nested-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("cockpit/src/lib.rs"),
        "pub fn answer()->u8{42}\n#[cfg(test)]mod tests{#[test]fn answer_is_42(){assert_eq!(super::answer(),42);}}\n",
    )
    .unwrap();
    assert_eq!(
        crate::tools::build::cargo_workspace_root(&root),
        Some(root.join("cockpit"))
    );
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({}))
        .expect("the nested crate's suite runs through the typed verifier");
    assert!(receipt.contains("tests: 1 passed, 0 failed"), "{receipt}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_tests_picks_the_largest_of_several_nested_crates_and_honours_a_crate_pin() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping multi-crate run_tests test");
        return;
    }
    let _env = crate::tests::env_lock();
    let root = scratch("run_tests_multi_crate");
    for (name, tests) in [("cockpit", 3usize), ("harness", 1), ("render-kit", 1)] {
        std::fs::create_dir_all(root.join(name).join("src")).unwrap();
        std::fs::write(
            root.join(name).join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .unwrap();
        let body: String = (0..tests)
            .map(|i| format!("#[test]fn t{i}(){{assert!(true);}}"))
            .collect();
        std::fs::write(
            root.join(name).join("src/lib.rs"),
            format!("pub fn f()->u8{{1}}\n#[cfg(test)]mod tests{{{body}}}\n"),
        )
        .unwrap();
        // cockpit is the biggest crate by source files
        if name == "cockpit" {
            for extra in ["a.rs", "b.rs"] {
                std::fs::write(root.join(name).join("src").join(extra), "// filler\n").unwrap();
            }
        }
    }
    for dir in [
        &root,
        &root.join("cockpit"),
        &root.join("harness"),
        &root.join("render-kit"),
    ] {
        assert_k5c_no_local_controls(dir);
    }
    assert_eq!(
        crate::tools::build::cargo_workspace_root(&root),
        Some(root.join("cockpit")),
        "the largest crate wins when several sit one level down"
    );
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({}))
        .expect("default picks cockpit");
    assert!(receipt.contains("tests: 3 passed, 0 failed"), "{receipt}");
    assert!(receipt.contains("(crate cockpit)"), "{receipt}");
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({ "crate": "harness" }))
        .expect("crate pin picks harness");
    assert!(receipt.contains("tests: 1 passed, 0 failed"), "{receipt}");
    assert!(receipt.contains("(crate harness)"), "{receipt}");
    let error = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({ "crate": "nope" }))
        .expect_err("unknown crate pin names the known ones");
    assert!(
        error.contains("crates here: cockpit, harness, render-kit"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_tests_dir_runs_a_python_suite_under_a_js_and_rust_mono_repo() {
    // PinnedCargo::capture reads process-global CARGO_HOME/PATH that sibling
    // tests flip under the env lock; hold it so the resolver-dedup marker test
    // never sees this test's capture (it failed twice in full-suite runs).
    let _env = crate::tests::env_lock();
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping run_tests dir test");
        return;
    }
    if crate::tools::build::resolve_on_path_for_test("python3").is_none() {
        eprintln!("python3 unavailable; skipping run_tests dir test");
        return;
    }
    let root = scratch("run_tests_dir_mono");
    std::fs::write(root.join("package.json"), "{\"name\":\"mono\"}\n").unwrap();
    std::fs::create_dir_all(root.join("cockpit/src")).unwrap();
    std::fs::write(
        root.join("cockpit/Cargo.toml"),
        "[package]\nname=\"c\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    std::fs::write(root.join("cockpit/src/lib.rs"), "pub fn f()->u8{1}\n").unwrap();
    std::fs::create_dir_all(root.join("sidecar/forge")).unwrap();
    std::fs::write(
        root.join("sidecar/forge/lib.py"),
        "def add(a,b):\n    return a+b\n",
    )
    .unwrap();
    std::fs::write(
        root.join("sidecar/forge/test_lib.py"),
        "import unittest\nfrom lib import add\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(1,2),3)\n",
    )
    .unwrap();
    for dir in [&root, &root.join("cockpit"), &root.join("sidecar/forge")] {
        assert_k5c_no_local_controls(dir);
    }
    let tool = RunTestsTool::in_dir(root.clone());
    let receipt = tool
        .call(&serde_json::json!({ "dir": "sidecar/forge" }))
        .expect("the python suite two levels down runs when pointed at");
    assert!(receipt.contains("tests: 1 passed, 0 failed"), "{receipt}");
    assert!(receipt.contains("unittest"), "{receipt}");
    let error = tool
        .call(&serde_json::json!({ "dir": "../outside" }))
        .expect_err("a dir outside the workspace is refused");
    assert!(
        error.contains("not a directory inside the workspace"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_tests_forwards_args_through_npm_with_a_separator() {
    // PinnedCargo::capture reads process-global CARGO_HOME/PATH that sibling
    // tests flip under the env lock; hold it so the resolver-dedup marker test
    // never sees this test's capture (it failed twice in full-suite runs).
    let _env = crate::tests::env_lock();
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping npm args test");
        return;
    }
    if crate::tools::build::resolve_on_path_for_test("npm").is_none()
        || crate::tools::build::resolve_on_path_for_test("node").is_none()
    {
        eprintln!("npm/node unavailable; skipping npm args test");
        return;
    }
    let root = scratch("run_tests_npm_args");
    std::fs::write(
        root.join("package.json"),
        "{\"name\":\"x\",\"scripts\":{\"test\":\"node --test\"}}\n",
    )
    .unwrap();
    for name in ["a", "b"] {
        std::fs::write(
            root.join(format!("{name}.test.mjs")),
            format!("import test from 'node:test';\ntest('{name}', () => {{}});\n"),
        )
        .unwrap();
    }
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({ "args": "a.test.mjs" }))
        .expect("a file arg selects one file");
    // The typed native route (pinned node) answers first when it can; the
    // workspace-scan fallback answers otherwise. Either way one file runs.
    assert!(receipt.contains("tests: 1 passed, 0 failed"), "{receipt}");
    // Without args both files run.
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({}))
        .expect("no args → the whole suite");
    assert!(receipt.contains("tests: 2 passed, 0 failed"), "{receipt}");
    // A script with hardcoded globs: the arg must not merely add to them.
    std::fs::write(
        root.join("package.json"),
        "{\"name\":\"x\",\"scripts\":{\"test\":\"node --test 'b.test.mjs'\"}}\n",
    )
    .unwrap();
    let receipt = RunTestsTool::in_dir(root.clone())
        .call(&serde_json::json!({ "args": "a.test.mjs" }))
        .expect("the given file runs");
    // Typed route: the script's file plus the given one (2); scan fallback:
    // only the given one (1). Both are green and both include a.test.mjs.
    assert!(
        receipt.contains("tests: 1 passed, 0 failed")
            || receipt.contains("tests: 2 passed, 0 failed"),
        "{receipt}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reward_text_never_rounds_a_red_suite_to_one() {
    use crate::tools::build::reward_text;
    assert_eq!(reward_text(764, 1), "reward 0.999");
    assert_eq!(reward_text(1, 1), "reward 0.500");
    assert_eq!(reward_text(3, 0), "reward 1.00");
    assert_eq!(reward_text(0, 0), "reward 0.00");
}

#[test]
fn k5c_run_tests_missing_lookups_never_return_a_bare_os_error() {
    let _env = crate::tests::env_lock();
    let root = scratch("k5c_missing_lookups");
    let tool = RunTestsTool::in_dir(root.clone());
    for key in ["dir", "crate"] {
        let error = tool.call(&serde_json::json!({key: "missing"})).unwrap_err();
        assert!(
            error.contains("not a directory inside the workspace"),
            "{error}"
        );
        assert!(error.contains("resolve subdirectory"), "{error}");
        assert!(
            error.contains(&root.join("missing").display().to_string()),
            "{error}"
        );
        assert!(!error.contains("os error"), "{error}");
    }
    std::fs::remove_dir_all(&root).unwrap();
    let error = tool
        .call(&serde_json::json!({"dir": "missing"}))
        .unwrap_err();
    assert!(error.contains("resolve workspace"), "{error}");
    assert!(error.contains(&root.display().to_string()), "{error}");
}

fn assert_k5c_no_local_controls(dir: &std::path::Path) {
    for name in [
        ".git",
        ".cargo/config",
        ".cargo/config.toml",
        "rust-toolchain",
        "rust-toolchain.toml",
    ] {
        assert!(
            !dir.join(name).exists(),
            "unexpected fixture control: {}",
            dir.join(name).display()
        );
    }
}
