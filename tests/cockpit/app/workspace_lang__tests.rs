use super::*;

// Cases are read at test time so runner formats can be extended without a Rust rebuild.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerCase {
    id: String,
    lang: String,
    stdout: String,
    stderr: String,
    passed: Option<usize>,
    failed: Option<usize>,
    skipped: Option<usize>,
}

fn assert_runner_cases(group: &str, expected_cases: usize) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/cockpit/fixtures/runner-output-cases.json");
    let bytes = std::fs::read(path).expect("runner output fixture");
    let all: std::collections::BTreeMap<String, Vec<RunnerCase>> =
        serde_json::from_slice(&bytes).expect("valid runner output fixture");
    let cases = all.get(group).expect("known runner output group");
    assert_eq!(cases.len(), expected_cases, "{group}: case count");
    let mut ids = std::collections::HashSet::new();
    for case in cases {
        assert!(ids.insert(&case.id), "{group}: duplicate case {}", case.id);
        let lang = match case.lang.as_str() {
            "js" => Lang::Js,
            "python" => Lang::Python,
            "go" => Lang::Go,
            "cpp" => Lang::Cpp,
            _ => panic!("{group}/{}: unknown language", case.id),
        };
        let actual = parse_runner_output(lang, &case.stdout, &case.stderr);
        assert!(case.passed.is_some() || case.failed.is_some() || case.skipped.is_some());
        if let Some(expected) = case.passed {
            assert_eq!(actual.passed, expected, "{group}/{}: passed", case.id);
        }
        if let Some(expected) = case.failed {
            assert_eq!(actual.failed, expected, "{group}/{}: failed", case.id);
        }
        if let Some(expected) = case.skipped {
            assert_eq!(actual.skipped, expected, "{group}/{}: skipped", case.id);
        }
    }
}

fn parse_runner_output(lang: Lang, stdout: &str, stderr: &str) -> RunnerCounts {
    let _guard = crate::tests::env_lock();
    super::parse_runner_output(lang, stdout, stderr, None).expect("test summary module")
}

#[test]
fn changed_embedded_runner_sources_do_not_take_the_inline_path() {
    let wrapper = include_str!("../../../scripts/runtime/test-runner-report.mjs");
    let module = include_str!("../../../lib/test-runner/report.mjs");
    assert!(!inline_runner_generation_compatible(
        wrapper,
        &module.replace("export function project", "export const project = function")
    ));
    assert!(!inline_runner_generation_compatible(
        wrapper,
        &format!("import 'other';\n{module}")
    ));
    assert!(!inline_runner_generation_compatible(
        &format!("{wrapper}\n// changed"),
        module
    ));
}

#[test]
fn runner_timeout_fallback_uses_the_captured_generation() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let _guard = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("runner-pinned-fallback");
    let _resource =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let _node = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    let wrapper = root.path().join("scripts/runtime/test-runner-report.mjs");
    let module = root.path().join("lib/test-runner/report.mjs");
    std::fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
    std::fs::create_dir_all(module.parent().unwrap()).unwrap();
    std::fs::write(
        &wrapper,
        include_str!("../../../scripts/runtime/test-runner-report.mjs"),
    )
    .unwrap();
    let baseline = include_str!("../../../lib/test-runner/report.mjs");
    for error in ["projection capacity timed out", "projection timed out"] {
        std::fs::write(&module, baseline).unwrap();
        let result = super::parse_runner_output_with(
            Lang::Python, "=== 3 passed in 0.1s ===", "", None,
            |_, _, _| {
                std::fs::write(&module, "export function project() { return {version:1,passed:'99',failed:'0',skipped:'0'}; }").unwrap();
                Err(error.to_string())
            },
        ).unwrap();
        assert_eq!(result.passed, 3, "fallback must use the captured parser");
    }

    std::fs::write(&module, baseline).unwrap();
    let error = super::parse_runner_output_with(Lang::Python, "", "", None, |_, _, _| {
        Err("projection failed".to_string())
    })
    .unwrap_err();
    assert!(error.contains("projection failed"), "{error}");
    let cancelled = AtomicBool::new(false);
    let error =
        super::parse_runner_output_with(Lang::Python, "", "", Some(&cancelled), |_, _, _| {
            cancelled.store(true, Ordering::Release);
            Err("projection timed out".to_string())
        })
        .unwrap_err();
    assert!(error.contains("interrupted"), "{error}");
}

#[test]
fn embedded_runner_report_matches_pinned_node_for_real_summaries() {
    let _guard = crate::tests::env_lock();
    let _resource = crate::tests::TestEnvGuard::unset("ANGEL_RESOURCE_DIR");
    let _node = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    let node = crate::agent::tools::runtime_missing::resolve("node").unwrap();
    let cases = [
        (Lang::Js, "# pass 9\n# fail 1\n# skipped 2\n", ""),
        (
            Lang::Python,
            "=== 3 passed, 1 failed, 2 skipped in 0.1s ===",
            "",
        ),
        (Lang::Go, "--- PASS: café\n--- FAIL: test\n", ""),
        (
            Lang::Cpp,
            "100% tests passed out of 18446744073709551615",
            "",
        ),
        (Lang::Swift, "Executed 3 tests, with 1 failures", ""),
        (Lang::Js, "# pass 18446744073709551615\n# fail 1\n", ""),
    ];
    let fast: Vec<_> = cases
        .iter()
        .map(|(lang, out, err)| super::parse_runner_output(*lang, out, err, None))
        .collect();
    let _pin = crate::tests::TestEnvGuard::set("ANGEL_NODE_BIN", node.to_str().unwrap());
    for ((lang, out, err), expected) in cases.into_iter().zip(fast) {
        let legacy = super::parse_runner_output(lang, out, err, None);
        assert_eq!(legacy.is_ok(), expected.is_ok());
        if let (Ok(legacy), Ok(expected)) = (legacy, expected) {
            assert_eq!(legacy, expected);
        }
    }
}

#[test]
fn test_summary_reload_and_fail_closed() {
    let _guard = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("test-summary-reload");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/test-runner");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/runtime/test-runner-report.mjs"),
        scripts.join("test-runner-report.mjs"),
    )
    .unwrap();
    let module = library.join("report.mjs");
    let write_generation = |value: &str| {
        std::fs::write(&module, format!(
            "export function project() {{ return {{version:1,passed:'{value}',failed:'0',skipped:'0'}} }}"
        )).unwrap();
    };
    let _resource =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let _node = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    write_generation("7");
    assert_eq!(
        super::parse_runner_output(Lang::Python, "", "", None)
            .unwrap()
            .passed,
        7
    );
    write_generation("8");
    assert_eq!(
        super::parse_runner_output(Lang::Python, "", "", None)
            .unwrap()
            .passed,
        8
    );
    // A hotloaded wrapper remains authoritative too: the fast path must
    // notice it changed and run the existing Node worker protocol.
    let wrapper = scripts.join("test-runner-report.mjs");
    std::fs::write(
        &wrapper,
        "import {readFileSync} from 'node:fs'; readFileSync(0, 'utf8'); process.stdout.write(JSON.stringify({version:1,passed:'9',failed:'0',skipped:'0'}));",
    )
    .unwrap();
    assert_eq!(
        super::parse_runner_output(Lang::Python, "", "", None)
            .unwrap()
            .passed,
        9
    );
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/runtime/test-runner-report.mjs"),
        &wrapper,
    )
    .unwrap();
    write_generation("-1");
    assert!(super::parse_runner_output(Lang::Python, "", "", None).is_err());
    std::fs::write(&module, "invalid javascript {").unwrap();
    assert!(super::parse_runner_output(Lang::Python, "", "", None).is_err());
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    assert!(
        super::parse_runner_output(Lang::Python, "", "", Some(&cancelled))
            .unwrap_err()
            .contains("interrupted")
    );
}

#[test]
#[cfg(unix)]
fn test_summary_cancellation_terminates_inflight_worker() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    let _guard = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("test-summary-cancel");
    let fake = root.path().join("node-stall");
    let marker = root.path().join("worker-started");
    let quoted = marker.to_string_lossy().replace('\'', "'\\''");
    std::fs::write(
        &fake,
        format!("#!/bin/sh\nprintf started > '{quoted}'\nsleep 5\n"),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _node = crate::tests::TestEnvGuard::set("ANGEL_NODE_BIN", fake.to_str().unwrap());
    let _resource = crate::tests::TestEnvGuard::unset("ANGEL_RESOURCE_DIR");
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let notifier = scope.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !marker.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            let started = marker.exists();
            cancel.store(true, Ordering::Release);
            started
        });
        let started = Instant::now();
        let error = super::parse_runner_output(Lang::Python, "", "", Some(&cancel)).unwrap_err();
        assert!(error.contains("interrupted"), "{error}");
        assert!(
            notifier.join().unwrap(),
            "worker did not start before cancellation"
        );
        assert!(started.elapsed() < Duration::from_secs(3));
    });
}

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
    // The default pattern must match suffix-named suites (`foo_test.py`,
    // exercism style) as well as prefix-named ones; unittest's own default
    // `test*.py` silently discovers zero suffix-named tests (exit 5).
    assert!(plan.args.contains(&"-p".to_string()));
    assert!(plan.args.contains(&"*test*.py".to_string()));
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
    assert_runner_cases("node", 3);
}

#[test]
fn parses_jest_vitest_mocha() {
    assert_runner_cases("jest", 3);
}

#[test]
fn parses_pytest_and_unittest() {
    assert_runner_cases("python", 4);
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
    assert_runner_cases("go", 2);
}

#[test]
fn cmake_project_plans_configure_build_and_ctest() {
    let root = scratch("cmake_plan");
    std::fs::write(root.join("CMakeLists.txt"), "project(t CXX)\n").unwrap();
    let hits = detect(&root);
    assert_eq!(hits[0].lang, Lang::Cpp);
    let plan = plan_tests(&root, &hits, None).expect("a C++ plan");
    assert_eq!(plan.program, "sh");
    assert!(plan.args[1].starts_with("cmake -S . -B build && cmake --build build"));
    assert!(plan.args[1].contains("ctest --test-dir build"));
    assert_eq!(parse_lang("c++"), Some(Lang::Cpp));
    assert_eq!(parse_lang("cmake"), Some(Lang::Cpp));
}

/// A CMakeLists.txt beside another language's manifest (a native addon) must
/// not take over that project's own test runner.
#[test]
fn cmake_never_outranks_another_manifest_in_the_same_directory() {
    let root = scratch("cmake_beside_node");
    std::fs::write(root.join("CMakeLists.txt"), "project(addon CXX)\n").unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"test":"node --test"}}"#,
    )
    .unwrap();
    let plan = plan_tests(&root, &detect(&root), None).expect("a plan");
    assert_eq!(plan.lang, Lang::Js);
}

#[test]
fn cpp_runner_output_reads_catch2_ctest_and_googletest() {
    assert_runner_cases("cpp", 4);
}

#[test]
fn given_ctest_all_pass_when_parsed_then_registered_totals_win() {
    assert_runner_cases("ctest_pass", 2);
}

#[test]
fn given_ctest_malformed_when_parsed_then_nested_success_is_not_substituted() {
    assert_runner_cases("ctest_malformed", 11);
}

#[test]
fn given_ctest_conflicting_streams_when_parsed_then_no_all_pass_is_reported() {
    assert_runner_cases("ctest_conflict", 3);
}

#[test]
fn given_ctest_zero_registered_when_parsed_then_no_nested_tests_are_counted() {
    assert_runner_cases("ctest_zero", 2);
}

#[test]
fn given_ctest_failed_suite_when_parsed_then_failure_and_rounded_totals_survive() {
    assert_runner_cases("ctest_failed", 3);
}

#[test]
fn runner_commands_parse_to_their_language_entrypoint_and_width() {
    for (command, lang, entrypoint, words) in [
        ("npm test", Lang::Js, None, 2),
        ("pnpm run test", Lang::Js, None, 3),
        ("npx jest connect", Lang::Js, None, 2),
        ("./node_modules/.bin/jest", Lang::Js, None, 1),
        ("node --test test.mjs", Lang::Js, Some("node"), 2),
        ("cargo test --features x", Lang::Rust, None, 2),
        ("python3 -m pytest -q", Lang::Python, Some("pytest"), 3),
        (
            "python -m unittest discover",
            Lang::Python,
            Some("unittest"),
            3,
        ),
        ("pytest -k x", Lang::Python, Some("pytest"), 1),
        ("go test ./...", Lang::Go, None, 2),
        ("ctest --test-dir build", Lang::Cpp, None, 1),
    ] {
        let parts = command.split_whitespace().collect::<Vec<_>>();
        assert_eq!(
            parse_runner_command(&parts),
            Some(RunnerCommand {
                lang,
                entrypoint,
                words
            }),
            "{command}"
        );
    }
    for command in [
        "test",
        "npm install",
        "cargo build",
        "python3 x.py",
        "make test",
        "",
    ] {
        let parts = command.split_whitespace().collect::<Vec<_>>();
        assert_eq!(parse_runner_command(&parts), None, "{command}");
    }
}

#[test]
fn a_named_pytest_entrypoint_runs_pytest_with_the_models_own_arguments() {
    let root = std::env::temp_dir().join(format!("angel-plan-pytest-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("bowling_test.py"), "import unittest\n").unwrap();
    std::fs::write(root.join("bowling.py"), "").unwrap();
    let hits = detect(&root);
    let asked = plan_tests_for(&root, &hits, Some(Lang::Python), Some("pytest")).unwrap();
    assert_eq!(asked.program, "python3");
    assert_eq!(asked.args, ["-m", "pytest"]);
    assert_eq!(asked.label, "python3 -m pytest");
    // Unnamed (or unittest), the workspace's files choose, exactly as before.
    for plan in [
        plan_tests(&root, &hits, Some(Lang::Python)).unwrap(),
        plan_tests_for(&root, &hits, None, None).unwrap(),
        plan_tests_for(&root, &hits, None, Some("unittest")).unwrap(),
    ] {
        assert_eq!(
            plan.args,
            ["-m", "unittest", "discover", "-v", "-p", "*test*.py"]
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_named_node_entrypoint_runs_nodes_runner_beside_a_package_script() {
    let root = std::env::temp_dir().join(format!("angel-plan-node-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"test":"jest ./*"}}"#,
    )
    .unwrap();
    let hits = detect(&root);
    assert_eq!(plan_tests(&root, &hits, None).unwrap().program, "npm");
    assert_eq!(
        plan_tests_for(&root, &hits, None, Some("node"))
            .unwrap()
            .label,
        "node --test"
    );
    let _ = std::fs::remove_dir_all(&root);
}
