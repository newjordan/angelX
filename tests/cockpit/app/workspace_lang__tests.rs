use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("angel-wslang-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parse_lang_accepts_typescript_aliases() {
    assert_eq!(parse_lang("ts"), Some(Lang::Js));
    assert_eq!(parse_lang("typescript"), Some(Lang::Js));
    assert_eq!(parse_lang("node"), Some(Lang::Js));
    assert_eq!(parse_lang("cobol"), None);
}

#[test]
fn js_test_files_without_manifest_plan_node_test() {
    let d = scratch("js-bare");
    std::fs::write(d.join("index.mjs"), "export const x=1;\n").unwrap();
    std::fs::write(d.join("test.mjs"), "import test from 'node:test';\n").unwrap();
    let hits = detect(&d);
    assert_eq!(lang_names(&hits), vec!["js"]);
    let plan = plan_tests(&d, &hits, None).unwrap();
    assert_eq!(plan.program, "node");
    assert_eq!(plan.args, vec!["--test"]);
    assert_eq!(plan.because, "1 js test file(s)");
}

#[test]
fn nested_test_dir_counts_and_python_unittest_is_default() {
    let d = scratch("py-nested");
    std::fs::create_dir_all(d.join("tests")).unwrap();
    std::fs::write(d.join("tests/test_a.py"), "").unwrap();
    std::fs::write(d.join("lib.py"), "").unwrap();
    let hits = detect(&d);
    assert_eq!(lang_names(&hits), vec!["python"]);
    let plan = plan_tests(&d, &hits, None).unwrap();
    assert_eq!(plan.label, "python3 -m unittest discover -v");
    // pytest config flips the runner
    std::fs::write(d.join("pytest.ini"), "[pytest]\n").unwrap();
    let hits = detect(&d);
    let plan = plan_tests(&d, &hits, None).unwrap();
    assert_eq!(plan.label, "pytest -q");
    assert_eq!(plan.dir, PathBuf::from("."));
}

#[test]
fn rust_under_a_subdir_is_seen_and_root_manifest_wins_order() {
    let d = scratch("rust-sub");
    std::fs::create_dir_all(d.join("cockpit")).unwrap();
    std::fs::write(d.join("cockpit/Cargo.toml"), "[package]\n").unwrap();
    std::fs::write(d.join("package.json"), "{\"name\":\"x\"}").unwrap();
    std::fs::create_dir_all(d.join("node_modules/dep")).unwrap();
    std::fs::write(d.join("node_modules/dep/Cargo.toml"), "").unwrap();
    let hits = detect(&d);
    assert_eq!(lang_names(&hits), vec!["js", "rust"]);
    assert!(hits.iter().all(|h| !h.dir.starts_with("node_modules")));
    // an explicit preference reaches the nested rust hit, which the Cargo path owns
    assert!(plan_tests(&d, &hits, Some(Lang::Rust)).is_none());
    // package.json without a test script → node --test
    let plan = plan_tests(&d, &hits, None).unwrap();
    assert_eq!(plan.program, "node");
}

#[test]
fn package_json_test_script_plans_npm_test() {
    let d = scratch("npm");
    std::fs::write(
        d.join("package.json"),
        "{\"scripts\":{\"test\":\"vitest run\"}}",
    )
    .unwrap();
    let hits = detect(&d);
    let plan = plan_tests(&d, &hits, None).unwrap();
    assert_eq!(plan.program, "npm");
    assert_eq!(plan.label, "npm test (vitest run)");
    std::fs::write(
        d.join("package.json"),
        "{\"scripts\":{\"test\":\"echo \\\"Error: no test specified\\\" && exit 1\"}}",
    )
    .unwrap();
    let plan = plan_tests(&d, &detect(&d), None).unwrap();
    assert_eq!(plan.program, "node");
}

#[test]
fn parses_node_tap_and_spec() {
    let c = parse_runner_output(
        Lang::Js,
        "TAP version 13\nok 1 - a\nnot ok 2 - b\n# tests 2\n# pass 1\n# fail 1\n# skipped 0\n",
        "",
    );
    assert_eq!(
        c,
        RunnerCounts {
            passed: 1,
            failed: 1,
            skipped: 0
        }
    );
    let c = parse_runner_output(
        Lang::Js,
        "✔ a (1ms)\nℹ tests 3\nℹ pass 3\nℹ fail 0\nℹ skipped 1\n",
        "",
    );
    assert_eq!(
        c,
        RunnerCounts {
            passed: 3,
            failed: 0,
            skipped: 1
        }
    );
    let c = parse_runner_output(Lang::Js, "ok 1 - a\nok 2 - b\n", "");
    assert_eq!(c.passed, 2);
}

#[test]
fn parses_jest_vitest_mocha() {
    let c = parse_runner_output(Lang::Js, "", "Tests:       1 failed, 3 passed, 4 total\n");
    assert_eq!(
        c,
        RunnerCounts {
            passed: 3,
            failed: 1,
            skipped: 0
        }
    );
    let c = parse_runner_output(Lang::Js, "      Tests  3 passed | 1 failed (4)\n", "");
    assert_eq!(
        c,
        RunnerCounts {
            passed: 3,
            failed: 1,
            skipped: 0
        }
    );
    let c = parse_runner_output(
        Lang::Js,
        "  3 passing (12ms)\n  1 failing\n  2 pending\n",
        "",
    );
    assert_eq!(
        c,
        RunnerCounts {
            passed: 3,
            failed: 1,
            skipped: 2
        }
    );
}

#[test]
fn parses_pytest_and_unittest() {
    let c = parse_runner_output(
        Lang::Python,
        "=== 3 passed, 1 failed, 2 skipped in 0.12s ===\n",
        "",
    );
    assert_eq!(
        c,
        RunnerCounts {
            passed: 3,
            failed: 1,
            skipped: 2
        }
    );
    let c = parse_runner_output(
        Lang::Python,
        "",
        "test_a ... ok\ntest_b ... FAIL\n\nRan 4 tests in 0.001s\n\nFAILED (failures=1, errors=1)\n",
    );
    assert_eq!(
        c,
        RunnerCounts {
            passed: 2,
            failed: 2,
            skipped: 0
        }
    );
    let c = parse_runner_output(
        Lang::Python,
        "",
        "Ran 4 tests in 0.001s\n\nOK (skipped=1)\n",
    );
    assert_eq!(
        c,
        RunnerCounts {
            passed: 3,
            failed: 0,
            skipped: 1
        }
    );
    let c = parse_runner_output(Lang::Python, "", "Ran 2 tests in 0.001s\n\nOK\n");
    assert_eq!(
        c,
        RunnerCounts {
            passed: 2,
            failed: 0,
            skipped: 0
        }
    );
}

#[test]
fn missing_program_hint_names_the_sibling() {
    // `sh` exists everywhere; a nonsense program never does.
    assert!(missing_program_hint("cd /x && sh -c true").is_none());
    let hint = missing_program_hint("cd /x && FOO=1 zz-no-such-program -m unittest").unwrap();
    assert!(
        hint.starts_with("`zz-no-such-program` is not on PATH"),
        "{hint}"
    );
    if resolve_on_path("python").is_none() && resolve_on_path("python3").is_some() {
        let hint = missing_program_hint("python -m unittest -v 2>&1").unwrap();
        assert!(hint.contains("use `python3`"), "{hint}");
        let rt = host_runtimes();
        assert!(
            rt.contains("python3 (no `python`)")
                || rt.contains("python3 (`python` → python3 shim)"),
            "{rt}"
        );
    }
}

#[cfg(unix)]
#[test]
fn runtime_shims_dir_links_python_to_python3_when_python_is_absent() {
    // Exercise the production reconciler with explicit roots instead of
    // poisoning process-global HOME/PATH for concurrent runtime spawners.
    let root = scratch("runtime-shims");
    let home = root.join("home");
    let bins = root.join("bin");
    std::fs::create_dir(&bins).unwrap();
    let path = std::env::join_paths([&bins]).unwrap();
    assert!(runtime_shims_in(&home, &path).is_none());
    let python3 = bins.join("python3");
    std::fs::write(&python3, "owned runtime fixture").unwrap();
    let first = runtime_shims_in(&home, &path).expect("newly installed runtime");
    assert_eq!(std::fs::read_link(first.join("python")).unwrap(), python3);
    assert_eq!(runtime_shims_in(&home, &path), Some(first.clone()));
    // Removing the prior HOME must not leave a permanently cached dead path.
    std::fs::remove_dir_all(&home).unwrap();
    let rebuilt = runtime_shims_in(&home, &path).unwrap();
    assert!(rebuilt.join("python").is_file());
    let other_home = root.join("other-home");
    assert!(
        runtime_shims_in(&other_home, &path)
            .unwrap()
            .starts_with(&other_home)
    );

    let preferred = root.join("preferred-bin");
    std::fs::create_dir(&preferred).unwrap();
    std::fs::write(preferred.join("python3"), "new runtime fixture").unwrap();
    let changed_path = std::env::join_paths([&preferred, &bins]).unwrap();
    let changed = runtime_shims_in(&home, &changed_path).unwrap();
    assert_ne!(changed, rebuilt);
    assert_eq!(
        std::fs::read_link(changed.join("python")).unwrap(),
        preferred.join("python3")
    );
    assert_eq!(std::fs::read_link(rebuilt.join("python")).unwrap(), python3);
    // Installing the real name takes precedence immediately, including when
    // PATH's directory string itself has not changed.
    std::fs::write(preferred.join("python"), "real python fixture").unwrap();
    assert!(runtime_shims_in(&home, &changed_path).is_none());
    std::fs::remove_file(preferred.join("python")).unwrap();
    std::fs::remove_file(preferred.join("python3")).unwrap();
    assert_eq!(
        runtime_shims_in(&home, &changed_path),
        Some(rebuilt.clone())
    );
    // A replaced link is no longer ours to overwrite, even at an owned path.
    std::fs::remove_file(rebuilt.join("python")).unwrap();
    std::fs::write(rebuilt.join("python"), "operator replacement").unwrap();
    assert!(runtime_shims_in(&home, &path).is_none());
    assert_eq!(
        std::fs::read_to_string(rebuilt.join("python")).unwrap(),
        "operator replacement"
    );
    std::fs::remove_file(python3).unwrap();
    assert!(runtime_shims_in(&other_home, &path).is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn parses_go() {
    let c = parse_runner_output(
        Lang::Go,
        "--- PASS: TestA (0.00s)\n--- FAIL: TestB (0.00s)\n--- SKIP: TestC\nFAIL\n",
        "",
    );
    assert_eq!(
        c,
        RunnerCounts {
            passed: 1,
            failed: 1,
            skipped: 1
        }
    );
    let c = parse_runner_output(Lang::Go, "ok  \texample.com/x\t0.003s\n", "");
    assert_eq!(c.passed, 1);
}
