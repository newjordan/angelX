use super::*;
use crate::club::ToolCall;
use crate::harness::{VerificationOutcome, verification_outcome};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-native-verifier-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    fn call(&self, pins: &PinnedNativeRuntimes, kind: Kind, args: Value) -> String {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(self.0.clone());
        match dispatch(pins, kind, &args, &self.0, &policy, None, None).expect("native route") {
            Ok(text) => text,
            Err(error) => format!("tool error: {error}"),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn outcome(kind: Kind, text: &str) -> Option<VerificationOutcome> {
    verification_outcome(
        &ToolCall {
            id: "native".into(),
            name: kind.tool().into(),
            args: json!({}),
        },
        text,
    )
}

#[test]
fn native_verifier_discovery_is_explicit_and_mixed_roots_are_inconclusive() {
    let f = Fixture::new();
    assert!(plan::select(Kind::Tests, &json!({}), &f.0).is_err());
    assert!(matches!(
        plan::select(
            Kind::Tests,
            &json!({"entrypoint":"unittest"}),
            &f.0
        ),
        Ok(plan::Selection::Native(plan)) if plan.runtime == Runtime::Python
    ));
    f.write("Cargo.toml", "[package]\nname='fixture'\nversion='0.1.0'\n");
    assert!(matches!(
        plan::select(Kind::Tests, &json!({}), &f.0),
        Ok(plan::Selection::Rust)
    ));
    f.write("package.json", r#"{"scripts":{"test":"node --test"}}"#);
    f.write(
        "answer.test.cjs",
        "require('node:test')('answer',()=>{});\n",
    );
    assert!(plan::select(Kind::Tests, &json!({}), &f.0).is_err());
    assert!(matches!(
        plan::select(Kind::Tests, &json!({"runtime":"rust"}), &f.0),
        Ok(plan::Selection::Rust)
    ));
    assert!(matches!(
        plan::select(Kind::Tests, &json!({"runtime":"node"}), &f.0),
        Ok(plan::Selection::Native(_))
    ));
    f.write("package.json", "{bad json");
    assert!(matches!(
        plan::select(Kind::Tests, &json!({"runtime":"rust"}), &f.0),
        Ok(plan::Selection::Rust)
    ));
}

#[test]
fn native_verifier_rejects_scripts_loaders_and_selector_escapes() {
    let f = Fixture::new();
    let outside = Fixture::new();
    outside.write("outside.test.cjs", "");
    f.write("answer.test.cjs", "");
    for script in [
        "echo 'tests: 9 passed, 0 failed'",
        "node test.js",
        "node --test; true",
        "node --test --require ./shim.cjs",
    ] {
        f.write(
            "package.json",
            &json!({"scripts":{"test":script}}).to_string(),
        );
        assert!(
            plan::select(Kind::Tests, &json!({}), &f.0).is_err(),
            "{script}"
        );
    }
    f.write("package.json", r#"{"scripts":{"test":"node --test"}}"#);
    for args in [
        "--eval 1",
        "--test-reporter=./fake.js",
        "--import ./fake.js",
        outside.0.join("outside.test.cjs").to_str().unwrap(),
    ] {
        assert!(
            plan::select(Kind::Tests, &json!({"args":args}), &f.0).is_err(),
            "{args}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            outside.0.join("outside.test.cjs"),
            f.0.join("escape.test.cjs"),
        )
        .unwrap();
        assert!(plan::select(Kind::Tests, &json!({"args":"escape.test.cjs"}), &f.0).is_err());
        // This is a new isolated fixture namespace, never the real archive.
        f.write(
            "off-limits/probe.cjs",
            "throw Error('must not be selected');\n",
        );
        std::os::unix::fs::symlink(f.0.join("off-limits/probe.cjs"), f.0.join("hidden.cjs"))
            .unwrap();
        let error = plan::select(
            Kind::Check,
            &json!({"runtime":"node","args":"hidden.cjs"}),
            &f.0,
        )
        .err()
        .unwrap();
        assert!(error.contains("quarantined"), "{error}");
    }
    f.write(
        "pyproject.toml",
        "[tool.pytest.ini_options]\ntestpaths=['tests']\n",
    );
    assert!(plan::select(Kind::Tests, &json!({"runtime":"python"}), &f.0).is_err());
    assert!(plan::select(Kind::Lint, &json!({"runtime":"node"}), &f.0).is_err());
    assert!(plan::select(Kind::Lint, &json!({"runtime":"python"}), &f.0).is_err());
}

#[test]
fn native_node_pattern_only_arguments_keep_bounded_file_selection() {
    let f = Fixture::new();
    f.write("package.json", "{}");
    f.write("answer.test.cjs", "require('node:test')('works',()=>{});\n");
    let plan::Selection::Native(plan) = plan::select(
        Kind::Tests,
        &json!({"args":"--test-name-pattern=works"}),
        &f.0,
    )
    .unwrap_or_else(|e| panic!("{e}")) else {
        panic!("Node route required")
    };
    assert!(
        plan.argvs[0]
            .iter()
            .any(|arg| arg.ends_with("answer.test.cjs"))
    );
    assert!(plan.argvs[0].contains(&"--test-name-pattern=works".to_string()));
}

#[test]
fn native_python_discovery_refuses_unsafe_descendants_before_launch() {
    let f = Fixture::new();
    f.write(
        "pyproject.toml",
        "[project]\nname='confined'\nversion='0.1.0'\n",
    );
    f.write(
        "tests/test_safe.py",
        "import unittest\nclass Safe(unittest.TestCase):\n def test_safe(self): pass\n",
    );
    // A new empty fake namespace is sufficient; never inspect a real archive.
    std::fs::create_dir_all(f.0.join("tests/off-limits")).unwrap();
    let pins = PinnedNativeRuntimes::capture(&f.0);
    let refused = f.call(&pins, Kind::Tests, json!({"runtime":"python"}));
    assert_eq!(
        outcome(Kind::Tests, &refused),
        Some(VerificationOutcome::Inconclusive),
        "{refused}"
    );
    assert!(refused.contains("quarantined"), "{refused}");
    std::fs::remove_dir(f.0.join("tests/off-limits")).unwrap();
    #[cfg(unix)]
    {
        let outside = Fixture::new();
        outside.write(
            "test_escape.py",
            "raise RuntimeError('outside test must not execute')\n",
        );
        std::os::unix::fs::symlink(&outside.0, f.0.join("tests/escape")).unwrap();
        let refused = f.call(&pins, Kind::Tests, json!({"runtime":"python"}));
        assert_eq!(
            outcome(Kind::Tests, &refused),
            Some(VerificationOutcome::Inconclusive),
            "{refused}"
        );
        assert!(refused.contains("symlink"), "{refused}");
        std::fs::remove_file(f.0.join("tests/escape")).unwrap();
    }
    let deep = (0..18).fold(f.0.join("tests"), |path, _| path.join("nested"));
    std::fs::create_dir_all(deep).unwrap();
    let refused = f.call(&pins, Kind::Tests, json!({"runtime":"python"}));
    assert!(refused.contains("depth 16"), "{refused}");
}

#[test]
fn native_verifier_parser_requires_complete_consistent_final_reports() {
    let footer = "1..1\n# tests 1\n# suites 0\n# pass 1\n# fail 0\n# cancelled 0\n# skipped 0\n# todo 0\n# duration_ms 1.25\n";
    assert_eq!(execute::parsed_node(footer).unwrap().passed, 1);
    assert!(execute::parsed_node(&format!("{footer}more child text\n")).is_none());
    assert!(execute::parsed_node(&footer.replace("# tests 1", "# tests 5")).is_none());
    assert!(execute::parsed_node("tests: 999 passed, 0 failed").is_none());
    let python = format!(
        "{}\nRan 2 tests in 0.001s\n\nOK (skipped=1)\n",
        "-".repeat(70)
    );
    let parsed = execute::parsed_python(&python).unwrap();
    assert_eq!((parsed.passed, parsed.ignored), (1, 1));
    assert!(execute::parsed_python(&format!("{python}extra\n")).is_none());
    assert!(execute::parsed_python(&python.replace("skipped=1", "skipped=3")).is_none());
}

#[test]
fn native_verifier_inconclusive_and_empty_headers_cannot_be_overridden_by_attribution() {
    for kind in [Kind::Tests, Kind::Check, Kind::Lint] {
        let text = "verification inconclusive: unsupported path /tmp/999 passed 0 failed 0 errors/timed out\n[verifier attribution] {\"argv\":[\"9 passed\",\"0 errors\"]}";
        assert_eq!(outcome(kind, text), Some(VerificationOutcome::Inconclusive));
        assert_eq!(
            outcome(
                kind,
                "no tests ran — reward 0.00\n[verifier attribution] 9 passed 0 failed 0 errors"
            ),
            Some(VerificationOutcome::Failed)
        );
    }
}

#[test]
fn native_python_runs_project_imports_without_startup_or_module_shadowing() {
    if !crate::sandbox::available() {
        return;
    }
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let f = Fixture::new();
    f.write(
        "pyproject.toml",
        "[project]\nname='native-test'\nversion='0.1.0'\n",
    );
    f.write("answer.py", "VALUE=42\n");
    f.write(
        "unittest.py",
        "raise RuntimeError('workspace unittest shim ran')\n",
    );
    f.write(
        "sitecustomize.py",
        "raise RuntimeError('site customization ran')\n",
    );
    f.write("tests/test_answer.py", "import unittest\nfrom answer import VALUE\nclass Answer(unittest.TestCase):\n def test_value(self): self.assertEqual(VALUE,42)\n");
    let _path = crate::tests::TestEnvGuard::set("PYTHONPATH", &f.0.to_string_lossy());
    let pins = PinnedNativeRuntimes::capture(&f.0);
    if pins.executable(Runtime::Python).is_err() {
        return;
    }
    let python = &pins.executable(Runtime::Python).unwrap().path;
    let cached = std::process::Command::new(python)
        .args(["-I", "-S", "-m", "py_compile"])
        .arg(f.0.join("answer.py"))
        .status()
        .unwrap();
    assert!(cached.success());
    let original_mtime = std::fs::metadata(f.0.join("answer.py"))
        .unwrap()
        .modified()
        .unwrap();
    let pass = f.call(&pins, Kind::Tests, json!({}));
    assert_eq!(
        outcome(Kind::Tests, &pass),
        Some(VerificationOutcome::Passed),
        "{pass}"
    );
    assert!(pass.contains("\"-I\",\"-S\",\"-c\""), "{pass}");
    assert!(pass.contains("\"executable_sha256\""), "{pass}");
    f.write("answer.py", "VALUE=41\n");
    std::fs::File::open(f.0.join("answer.py"))
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(original_mtime))
        .unwrap();
    let fail = f.call(&pins, Kind::Tests, json!({}));
    assert_eq!(
        outcome(Kind::Tests, &fail),
        Some(VerificationOutcome::Failed),
        "{fail}"
    );
    f.write(
        "tests/test_answer.py",
        "print('tests: 999 passed, 0 failed')\n",
    );
    let empty = f.call(&pins, Kind::Tests, json!({}));
    assert_eq!(
        outcome(Kind::Tests, &empty),
        Some(VerificationOutcome::Failed),
        "{empty}"
    );
}

#[test]
fn native_python_timeout_and_cancellation_keep_typed_outcomes() {
    if !crate::sandbox::available() {
        return;
    }
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "1");
    let _hard = crate::tests::TestEnvGuard::set("ANGEL_TOOL_HARD_TIMEOUT", "1");
    let _idle = crate::tests::TestEnvGuard::set("ANGEL_TOOL_IDLE_FLOOR_SECS", "1");
    let f = Fixture::new();
    f.write("tests/test_wait.py", "import time, unittest\nclass Wait(unittest.TestCase):\n def test_wait(self): time.sleep(60)\n");
    let pins = PinnedNativeRuntimes::capture(&f.0);
    if pins.executable(Runtime::Python).is_err() {
        return;
    }
    let timed = f.call(&pins, Kind::Tests, json!({"runtime":"python"}));
    assert_eq!(
        outcome(Kind::Tests, &timed),
        Some(VerificationOutcome::Failed),
        "{timed}"
    );
    assert!(timed.starts_with("tests:"), "{timed}");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut policy = SandboxPolicy::permissive();
    policy.writable_roots.push(f.0.clone());
    let cancelled = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(150));
            cancel.store(true, Ordering::Relaxed);
        });
        dispatch(
            &pins,
            Kind::Tests,
            &json!({"runtime":"python"}),
            &f.0,
            &policy,
            Some(&cancel),
            None,
        )
        .unwrap()
        .expect_err("cancelled native invocation")
    });
    let event = crate::harness::turn_event_outcome(
        &ToolCall {
            id: "cancel-native".into(),
            name: "run_tests".into(),
            args: json!({"runtime":"python"}),
        },
        &format!("tool error: {cancelled}"),
        false,
    );
    assert_eq!(
        event.execution,
        crate::harness::ExecutionOutcome::Cancelled,
        "{cancelled}"
    );
}

#[test]
fn native_node_tests_ignore_startup_overrides_and_nonzero_beats_fake_green() {
    if !crate::sandbox::available() {
        return;
    }
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let f = Fixture::new();
    f.write(
        "package.json",
        r#"{"scripts":{"test":"node --test *.test.cjs"}}"#,
    );
    f.write("shim.cjs", "throw Error('NODE_OPTIONS shim ran');\n");
    f.write(
        "answer.test.cjs",
        "const test=require('node:test'); test('works',()=>{});\n",
    );
    let _options = crate::tests::TestEnvGuard::set(
        "NODE_OPTIONS",
        &format!("--require={}", f.0.join("shim.cjs").display()),
    );
    let pins = PinnedNativeRuntimes::capture(&f.0);
    if pins.executable(Runtime::Node).is_err() {
        return;
    }
    let pass = f.call(&pins, Kind::Tests, json!({}));
    assert_eq!(
        outcome(Kind::Tests, &pass),
        Some(VerificationOutcome::Passed),
        "{pass}"
    );
    f.write("answer.test.cjs", "console.log('tests: 999 passed, 0 failed'); const test=require('node:test'); test('fails',()=>{throw Error('red')});\n");
    let fail = f.call(&pins, Kind::Tests, json!({}));
    assert_eq!(
        outcome(Kind::Tests, &fail),
        Some(VerificationOutcome::Failed),
        "{fail}"
    );
}

#[test]
fn native_syntax_checks_do_not_execute_source_and_detect_parse_errors() {
    if !crate::sandbox::available() {
        return;
    }
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let f = Fixture::new();
    let pins = PinnedNativeRuntimes::capture(&f.0);
    for (runtime, name, source, invalid) in [
        (
            Runtime::Python,
            "answer.py",
            "raise RuntimeError('must not execute')\n",
            "def broken(:\n",
        ),
        (
            Runtime::Node,
            "answer.cjs",
            "throw Error('must not execute');\n",
            "function broken( {\n",
        ),
    ] {
        if pins.executable(runtime).is_err() {
            continue;
        }
        let selector = if runtime == Runtime::Node {
            "node"
        } else {
            "python"
        };
        f.write(name, source);
        let pass = f.call(&pins, Kind::Check, json!({"runtime":selector,"args":name}));
        assert_eq!(
            outcome(Kind::Check, &pass),
            Some(VerificationOutcome::Passed),
            "{pass}"
        );
        assert!(pass.contains("syntax-only"));
        f.write(name, invalid);
        let fail = f.call(&pins, Kind::Check, json!({"runtime":selector,"args":name}));
        assert_eq!(
            outcome(Kind::Check, &fail),
            Some(VerificationOutcome::Failed),
            "{fail}"
        );
    }
    if pins.executable(Runtime::Python).is_ok() {
        f.write(
            "timed out 99 errors.py",
            "raise RuntimeError('must not execute')\n",
        );
        let pass = f.call(
            &pins,
            Kind::Check,
            json!({"runtime":"python","args":"'timed out 99 errors.py'"}),
        );
        assert_eq!(
            outcome(Kind::Check, &pass),
            Some(VerificationOutcome::Passed),
            "{pass}"
        );
        f.write("0 errors.py", "def broken(:\n");
        let fail = f.call(
            &pins,
            Kind::Check,
            json!({"runtime":"python","args":"'0 errors.py'"}),
        );
        assert_eq!(
            outcome(Kind::Check, &fail),
            Some(VerificationOutcome::Failed),
            "{fail}"
        );
        let unsupported = f.call(
            &pins,
            Kind::Tests,
            json!({"runtime":"python","args":"'--custom=9 passed 0 failed'"}),
        );
        assert_eq!(
            outcome(Kind::Tests, &unsupported),
            Some(VerificationOutcome::Inconclusive),
            "{unsupported}"
        );
    }
}

#[cfg(unix)]
#[test]
fn native_runtime_shims_and_replaced_pins_are_not_trusted() {
    let f = Fixture::new();
    f.write("node", "#!/bin/sh\nprintf 'tests: 999 passed, 0 failed'\n");
    assert!(!native_image(&f.0.join("node")));
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(f.0.join("node"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let pinned =
        crate::tools::build::capture_executable(f.0.join("node"), "test-only pin").unwrap();
    f.write("node", "#!/bin/sh\nexit 0\n");
    assert!(revalidate_executable(&pinned, "test-only pin").is_err());
}

#[cfg(unix)]
#[test]
fn nvm_node_is_trusted_by_path_shape_even_under_a_foreign_home() {
    let _lock = crate::tests::env_lock();
    let f = Fixture::new();
    let original_path = std::env::var_os("PATH");
    // Discovery is by path shape, never $HOME: a private HOME that owns no
    // NVM tree must not matter.
    let _home = crate::tests::TestEnvGuard::set("HOME", &f.0.to_string_lossy());
    let foreign = f.0.join("elsewhere/.nvm/versions/node/v24.13.0/bin");
    assert!(is_nvm_versioned_bin(&foreign));
    let _path = crate::tests::TestEnvGuard::set("PATH", &foreign.to_string_lossy());
    let candidates = runtime_candidates(Runtime::Node);
    assert_eq!(candidates.last(), Some(&foreign.join("node")));
    assert_eq!(candidates.len(), 5);
    // Shape lookalikes that are not versioned NVM bins stay untrusted.
    for bad in [
        f.0.join("node_modules/.bin"),
        f.0.join(".nvm/versions/node/bin"),
        f.0.join(".nvm/versions/node/latest/bin"),
        f.0.join(".nvm/versions/node/v1/bin/extra"),
    ] {
        assert!(!is_nvm_versioned_bin(&bad), "{}", bad.display());
    }

    // End-to-end against the host's real operator NVM install, which lives
    // outside every task-writable root, with the foreign HOME still set.
    let Some(path) = original_path.clone() else {
        return;
    };
    let _restore_path = crate::tests::TestEnvGuard::set("PATH", &path.to_string_lossy());
    let host_nvm = std::env::split_paths(&path)
        .find(|dir| is_nvm_versioned_bin(dir) && dir.join("node").is_file());
    let Some(dir) = host_nvm else {
        return; // host has no NVM node on PATH; shape logic covered above
    };
    let pins = PinnedNativeRuntimes::capture(&f.0);
    let pinned = pins
        .executable(Runtime::Node)
        .expect("host NVM node must be trusted without consulting $HOME");
    assert_eq!(
        pinned.path.canonicalize().unwrap(),
        dir.join("node").canonicalize().unwrap()
    );

    // A workspace-local shim named node on PATH is never a candidate, let
    // alone a pinned executable: shape and writable-root guards both hold.
    f.write("shim-bin/node", "#!/bin/sh\nprintf 'tests: 999 passed'\n");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        f.0.join("shim-bin/node"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let _path2 = crate::tests::TestEnvGuard::set(
        "PATH",
        &format!("{}:{}", f.0.join("shim-bin").display(), dir.display()),
    );
    assert!(!runtime_candidates(Runtime::Node).contains(&f.0.join("shim-bin/node")));
    let pins = PinnedNativeRuntimes::capture(&f.0);
    let pinned = pins
        .executable(Runtime::Node)
        .expect("real NVM node still pinned");
    assert_ne!(
        pinned.path,
        f.0.join("shim-bin/node"),
        "a workspace-local node shim must never be pinned"
    );
}

#[cfg(unix)]
#[test]
fn runtime_env_pins_are_honored_and_bad_pins_fail_loudly() {
    let _lock = crate::tests::env_lock();
    let f = Fixture::new();
    // A fixed trusted host binary, outside every task-writable root.
    let outside_bin = PathBuf::from("/usr/bin/python3");
    if !outside_bin.is_file() {
        return;
    }
    let _pin = crate::tests::TestEnvGuard::set("ANGEL_PYTHON_BIN", &outside_bin.to_string_lossy());
    let pins = PinnedNativeRuntimes::capture(&f.0);
    let pinned = pins
        .executable(Runtime::Python)
        .expect("valid ANGEL_PYTHON_BIN pin must be trusted");
    assert_eq!(pinned.path, outside_bin.canonicalize().unwrap());
    drop(_pin);

    // A shim pin is not a native image: loud error naming the env var.
    use std::os::unix::fs::PermissionsExt;
    f.write("not-native", "#!/bin/sh\nexit 0\n");
    std::fs::set_permissions(
        f.0.join("not-native"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let _bad = crate::tests::TestEnvGuard::set(
        "ANGEL_PYTHON_BIN",
        &f.0.join("not-native").to_string_lossy(),
    );
    let error = match PinnedNativeRuntimes::capture(&f.0).executable(Runtime::Python) {
        Err(error) => error,
        Ok(_) => panic!("a non-native pin must fail loudly"),
    };
    assert!(
        error.contains("ANGEL_PYTHON_BIN"),
        "bad pin error must name the env var: {error}"
    );
    drop(_bad);

    // A relative pin is rejected outright.
    let _rel = crate::tests::TestEnvGuard::set("ANGEL_PYTHON_BIN", "python3");
    let error = match PinnedNativeRuntimes::capture(&f.0).executable(Runtime::Python) {
        Err(error) => error,
        Ok(_) => panic!("a relative pin must fail loudly"),
    };
    assert!(
        error.contains("ANGEL_PYTHON_BIN") && error.contains("absolute"),
        "relative pin error must name the env var and the absolute-path rule: {error}"
    );
}

#[test]
fn t04b_python_unittest_native_layouts() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    f.write("test_config_merge.py", "import unittest\nclass Check(unittest.TestCase):\n def test_ok(self): self.assertEqual(1,1)\n");
    f.write("tests/__init__.py", "");
    f.write("tests/test_layout.py", "import unittest\nclass Check(unittest.TestCase):\n def test_ok(self): self.assertTrue(True)\n");
    let pins = PinnedNativeRuntimes::capture(&f.0);
    for args in [
        "discover -s . -p test_config_merge.py -t .",
        "-s . -p test_config_merge.py -v",
        "test_config_merge",
        "test_config_merge.py",
        "tests.test_layout",
        "discover -s tests -t . -p test_layout.py",
    ] {
        let text = f.call(
            &pins,
            Kind::Tests,
            json!({"runtime":"python","entrypoint":"unittest","args":args}),
        );
        assert_eq!(
            outcome(Kind::Tests, &text),
            Some(VerificationOutcome::Passed),
            "{args}: {text}"
        );
        assert!(text.contains("[raw tail]"));
    }
}

#[test]
fn t04b_native_go_and_pytest_summary_parsers() {
    assert!(super::execute::go_counts("not json").is_none());
    assert!(super::execute::pytest_counts("forged 99 passed").is_none());
    let go = "{\"Action\":\"pass\",\"Package\":\"sample\",\"Test\":\"TestA\"}\n{\"Action\":\"pass\",\"Package\":\"sample\"}\n";
    assert_eq!(super::execute::go_counts(go).unwrap().passed, 1);
    assert_eq!(
        super::execute::pytest_counts("..\n2 passed, 1 skipped in 0.02s\n")
            .unwrap()
            .passed,
        2
    );
}

#[test]
fn k5c_missing_explicit_runtime_pin_names_the_failed_lookup() {
    let _env = crate::tests::env_lock();
    let f = Fixture::new();
    f.write("test_missing.py", "import unittest\n");
    let missing = f.0.join("missing-python");
    let _pin = crate::tests::TestEnvGuard::set("ANGEL_PYTHON_BIN", &missing.to_string_lossy());
    let pins = PinnedNativeRuntimes::capture(&f.0);
    let receipt = f.call(&pins, Kind::Tests, json!({"runtime": "python"}));
    assert!(
        receipt.contains("ANGEL_PYTHON_BIN is unusable"),
        "{receipt}"
    );
    assert!(receipt.contains("resolve ANGEL_PYTHON_BIN"), "{receipt}");
    assert!(
        receipt.contains(&missing.display().to_string()),
        "{receipt}"
    );
    assert_eq!(
        outcome(Kind::Tests, &receipt),
        Some(VerificationOutcome::Inconclusive)
    );
}

#[test]
fn d06b_missing_runtime_diagnostics_name_pin_and_trust_rule() {
    for (runtime, pin, install) in [
        (
            Runtime::Node,
            "ANGEL_NODE_BIN",
            ".nvm/versions/node/vX/bin/node",
        ),
        (Runtime::Python, "ANGEL_PYTHON_BIN", "/usr/bin/python3"),
    ] {
        let message = missing_runtime(runtime);
        assert!(message.contains(&format!("set {pin}=/abs/path")));
        assert!(message.contains(install));
        assert!(message.contains("native executable outside all task-writable roots"));
        assert_eq!(message.lines().count(), 1);
    }
}

#[test]
fn runtime_missing_fallback_checks_node_before_npm_and_python_before_spawn() {
    let _env = crate::tests::env_lock();
    let f = Fixture::new();
    let _path = crate::tests::TestEnvGuard::set("PATH", &f.0.to_string_lossy());
    let _node = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    let _python = crate::tests::TestEnvGuard::unset("ANGEL_PYTHON_BIN");
    for (program, runtime) in [("npm", "node"), ("python3", "python3")] {
        let error = super::super::sandboxed_program_output(
            program,
            &[],
            &f.0,
            &SandboxPolicy::permissive(),
            None,
            (None, &json!({})),
        )
        .err()
        .expect("runtime must be absent");
        let parsed = crate::tools::runtime_missing::RuntimeMissing::decode(&error).unwrap();
        assert_eq!(parsed.runtime, runtime);
        assert!(parsed.looked_for.contains("PATH"));
        assert!(parsed.hint.contains("install"));
    }
}

#[test]
fn runtime_missing_cargo_pin_is_actionable_even_with_inherited_toolchain() {
    let _env = crate::tests::env_lock();
    let _capture = super::super::capture_lock();
    let f = Fixture::new();
    f.write("rust-toolchain.toml", "[toolchain]\nchannel='1.95.0'\n");
    let _pin = crate::tests::TestEnvGuard::set(
        "ANGEL_CARGO_BIN",
        &f.0.join("absent-cargo").to_string_lossy(),
    );
    let error = super::super::capture_pinned_cargo(&f.0)
        .err()
        .expect("explicit missing pin must not discover a fallback");
    let parsed = crate::tools::runtime_missing::RuntimeMissing::decode(&error).unwrap();
    assert_eq!(parsed.runtime, "cargo");
    assert!(parsed.looked_for.contains("ANGEL_CARGO_BIN="));
}

#[test]
fn d02_nvm_shape_accepts_exactly_five_components() {
    assert!(is_nvm_versioned_bin(Path::new(".nvm/versions/node/v1/bin")));
    assert!(!is_nvm_versioned_bin(Path::new("versions/node/v1/bin")));
}

#[test]
fn d02_nvm_shape_requires_a_version_after_v() {
    assert!(!is_nvm_versioned_bin(Path::new(
        "/opt/.nvm/versions/node/v/bin"
    )));
    assert!(is_nvm_versioned_bin(Path::new(
        "/opt/.nvm/versions/node/v1/bin"
    )));
}

#[test]
fn c05e_unittest_selectors_report_selected_counts_and_empty_selection() {
    let _guard = crate::tests::env_lock();
    let control = Fixture::new();
    let _caddy = crate::tests::TestEnvGuard::set("ANGEL_CADDY", "1");
    let _store = crate::tests::TestEnvGuard::set("ANGEL_CADDY_DIR", control.0.to_str().unwrap());
    let f = Fixture::new();
    f.write("tests/test_policy.py", "import unittest\nclass Policy(unittest.TestCase):\n def test_policy_fern(self): pass\n def test_policy_other(self): self.fail('not selected')\n");
    let pins = PinnedNativeRuntimes::capture(&f.0);
    for args in [
        "-k policy_fern",
        "discover -v -k policy_fern",
        "discover -s tests -p test_policy.py -v -k policy_fern",
    ] {
        let text = f.call(&pins, Kind::Tests, json!({"runtime":"python","args":args}));
        assert_eq!(
            outcome(Kind::Tests, &text),
            Some(VerificationOutcome::Passed),
            "{text}"
        );
        assert!(text.contains("tests: 1 passed, 0 failed"), "{text}");
        assert!(text.contains("policy_fern"), "{text}");
        let call = ToolCall {
            id: args.into(),
            name: "run_tests".into(),
            args: json!({"runtime":"python","args":args}),
        };
        let receipt = crate::club::ChatMsg::tool(&call.id, text.as_str()).with_tool_receipt(
            &call,
            crate::harness::turn_event_outcome(&call, &text, false),
        );
        let history = vec![crate::club::ChatMsg::assistant_calls(vec![call]), receipt];
        assert_eq!(crate::caddy::write_back_from_history(&f.0, &history).0, 1);
    }
    let tool = crate::tools::build::RunTestsTool::in_dir(f.0.clone());
    let text = crate::harness::Tool::call(
        &tool,
        &json!({"runtime":"python","args":"discover -k missing_policy"}),
    )
    .unwrap();
    assert_eq!(
        outcome(Kind::Tests, &text),
        Some(VerificationOutcome::Inconclusive),
        "{text}"
    );
    let text = f.call(
        &pins,
        Kind::Tests,
        json!({"runtime":"python","args":"discover -k policy_other"}),
    );
    assert_eq!(
        outcome(Kind::Tests, &text),
        Some(VerificationOutcome::Failed),
        "{text}"
    );
    assert!(
        plan::select(
            Kind::Tests,
            &json!({"runtime":"python","args":"discover -k"}),
            &f.0
        )
        .is_err()
    );
}
