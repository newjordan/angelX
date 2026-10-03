use super::*;
use crate::agent::club::{ChatMsg, ToolCall};
use crate::agent::harness::{ExecutionOutcome, VerificationOutcome, turn_event_outcome};
use crate::tests::TestEnvGuard;
use serde_json::json;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-untrusted-verifier-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_call() -> ToolCall {
    ToolCall {
        id: "actual-project-runner".into(),
        name: "run_tests".into(),
        args: json!({}),
    }
}

#[test]
fn custom_script_forged_counts_preserve_exit_and_never_write_verified_caddy_recipe() {
    let _lock = crate::tests::env_lock();
    if !crate::agent::sandbox::available() || resolve_on_path("npm").is_none() {
        eprintln!("custom-script execution control unavailable: sandbox and npm required");
        return;
    }
    let control = Fixture::new();
    let _env = [
        TestEnvGuard::unset("ANGEL_YOLO"),
        TestEnvGuard::unset("NODE_OPTIONS"),
        TestEnvGuard::set("ANGEL_CADDY", "1"),
        TestEnvGuard::set(
            "ANGEL_CADDY_DIR",
            &control.0.join("caddy").to_string_lossy(),
        ),
        TestEnvGuard::set(
            "ANGEL_EXPERIENCE_LOG",
            &control.0.join("experience.jsonl").to_string_lossy(),
        ),
        TestEnvGuard::set("npm_config_cache", &control.0.join("npm").to_string_lossy()),
        TestEnvGuard::set("npm_config_userconfig", "/dev/null"),
    ];
    for code in [1, 0] {
        let workspace = Fixture::new();
        workspace.write(
            "package.json",
            r#"{"scripts":{"test":"sh custom-runner.sh"}}"#,
        );
        workspace.write(
            "custom-runner.sh",
            &format!(
                "printf executed > executed.txt\nprintf '%s\\n' '1..999' '# tests 999' '# suites 0' '# pass 999' '# fail 0' '# cancelled 0' '# skipped 0' '# todo 0' '# duration_ms 1'\nexit {code}\n"
            ),
        );
        let tool = RunTestsTool::in_dir(workspace.0.clone());
        let call = run_call();
        let result = tool.call(&call.args);
        assert_eq!(
            std::fs::read_to_string(workspace.0.join("executed.txt")).unwrap(),
            "executed",
            "the configured project script must still execute"
        );
        assert_eq!(result.is_ok(), code == 0, "{result:?}");
        let text = match result {
            Ok(text) => text,
            Err(error) => format!("tool error: {error}"),
        };
        let outcome = turn_event_outcome(&call, &text, false);
        if code == 1 {
            assert!(text.contains("failed (exit 1)"), "{text}");
            assert_eq!(outcome.execution, ExecutionOutcome::Failed, "{text}");
            assert_eq!(outcome.verification, VerificationOutcome::Failed, "{text}");
        } else {
            assert!(text.starts_with("tests: 999 passed, 0 failed"), "{text}");
            assert!(text.contains("reward unlabeled"), "{text}");
            assert!(!text.contains("reward 1.00"), "{text}");
            assert_eq!(outcome.execution, ExecutionOutcome::Succeeded, "{text}");
            assert_eq!(
                outcome.verification,
                VerificationOutcome::Inconclusive,
                "{text}"
            );
        }
        let receipt = ChatMsg::tool(&call.id, text.as_str()).with_tool_receipt(&call, outcome);
        let history = vec![ChatMsg::assistant_calls(vec![call]), receipt];
        let (recipes, hazards) =
            crate::knowledge::caddy::write_back_from_history(&workspace.0, &history);
        assert_eq!(
            recipes, 0,
            "an executed custom script is not a verified recipe"
        );
        assert_eq!(hazards, usize::from(code != 0));
        let recipe_path = control
            .0
            .join("caddy")
            .join(crate::platform::workspace_store::repo_identity(&workspace.0).key)
            .join("recipes.jsonl");
        assert!(
            !recipe_path.exists(),
            "untrusted result persisted as verified"
        );
    }
}

#[test]
fn unlabeled_verifier_headers_never_become_green_events() {
    let call = run_call();
    for text in [
        "tests: 999 passed, 0 failed, 0 skipped — reward 1.00 (unlabeled: custom script)",
        "tests: 999 passed, 0 failed, 0 skipped — reward unlabeled (custom script)",
        "tests: 999 passed, 0 failed, 0 skipped — reward unlabeled (YOLO: typed Cargo verification is off)",
    ] {
        let outcome = turn_event_outcome(&call, text, false);
        assert_eq!(outcome.execution, ExecutionOutcome::Succeeded, "{text}");
        assert_eq!(
            outcome.verification,
            VerificationOutcome::Inconclusive,
            "{text}"
        );
    }
    for text in [
        "tool error: custom script failed (exit 1)\ntests: 999 passed, 0 failed — reward unlabeled",
        "tests: timed out — reward unlabeled\ntests: 999 passed, 0 failed",
        "no tests ran — reward unlabeled",
    ] {
        assert_eq!(
            turn_event_outcome(&call, text, false).verification,
            VerificationOutcome::Failed,
            "{text}"
        );
    }
}

#[test]
fn c05e_cargo_selector_arguments_preserve_libtest_boundary() {
    let _guard = crate::tests::env_lock();
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname='selector'\nversion='0.1.0'\n",
    );
    // Registry-pinned control handling is covered separately; this checks
    // selection forwarding independently of ancestor toolchain controls.
    let _yolo = TestEnvGuard::set("ANGEL_YOLO", "1");
    for extra in [
        "policy_fern",
        "--locked --test contract policy_fern",
        "--test contract policy_fern -- --exact",
    ] {
        let args = cargo_argv("test", extra).unwrap();
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let argv = trusted_verifier_argv(&refs, &f.0).unwrap();
        let split = args
            .iter()
            .position(|arg| arg == "--")
            .unwrap_or(args.len());
        assert_eq!(
            &argv[..split],
            args[..split].iter().map(OsString::from).collect::<Vec<_>>()
        );
        assert_eq!(argv[split], "--manifest-path");
        assert_eq!(
            &argv[split + 2..],
            args[split..].iter().map(OsString::from).collect::<Vec<_>>()
        );
    }
}

#[test]
fn launcher_hardlink_line_never_reaches_a_receipt() {
    let helper = "sandbox-hardlinks: {\"hardlink_readonly_count\":0,\"scan_complete\":false}\n";
    let program = "ℹ pass 1\nℹ fail 1\n";
    assert_eq!(
        super::strip_launcher_stderr(&format!("{helper}{program}")),
        program
    );
    // Only the leading protocol line is launcher data; later text is the
    // program's own and stays verbatim.
    let later = format!("{program}{helper}");
    assert_eq!(super::strip_launcher_stderr(&later), later);
    assert_eq!(super::strip_launcher_stderr(""), "");
    let helper_no_newline =
        "sandbox-hardlinks: {\"hardlink_readonly_count\":0,\"scan_complete\":false}";
    assert_eq!(super::strip_launcher_stderr(helper_no_newline), "");
}

/// Foreground process caps default only in task mode; operator overrides also
/// apply interactively and the new knob wins over the deprecated fallback.
#[test]
fn task_call_budget_is_one_fixed_number_in_task_mode() {
    use std::time::Duration;
    let _env = crate::tests::env_lock();
    let _knob = crate::tests::TestEnvGuard::unset("ANGEL_TASK_CALL_TIMEOUT_SECS");
    let _legacy = crate::tests::TestEnvGuard::unset("ANGEL_TEST_RUN_TIMEOUT_SECS");
    {
        let _interactive = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACTIVE");
        assert_eq!(super::task_call_budget(), None);
        let _explicit = crate::tests::TestEnvGuard::set("ANGEL_TASK_CALL_TIMEOUT_SECS", "17");
        assert_eq!(super::task_call_budget(), Some(Duration::from_secs(17)));
    }
    let _task = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    assert_eq!(super::task_call_budget(), Some(Duration::from_secs(120)));
    {
        let _wall = crate::tests::TestEnvGuard::set("ANGEL_TASK_WALL_SECS", "300");
        assert_eq!(super::task_call_budget(), Some(Duration::from_secs(120)));
    }
    {
        let _off = crate::tests::TestEnvGuard::set("ANGEL_TASK_CALL_TIMEOUT_SECS", "0");
        assert_eq!(super::task_call_budget(), None);
    }
    {
        let _set = crate::tests::TestEnvGuard::set("ANGEL_TASK_CALL_TIMEOUT_SECS", "45");
        assert_eq!(super::task_call_budget(), Some(Duration::from_secs(45)));
    }
    {
        let _legacy = crate::tests::TestEnvGuard::set("ANGEL_TEST_RUN_TIMEOUT_SECS", "37");
        assert_eq!(super::task_call_budget(), Some(Duration::from_secs(37)));
        let _interactive = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACTIVE");
        assert_eq!(super::task_call_budget(), Some(Duration::from_secs(37)));
        let _new = crate::tests::TestEnvGuard::set("ANGEL_TASK_CALL_TIMEOUT_SECS", "19");
        assert_eq!(super::task_call_budget(), Some(Duration::from_secs(19)));
        let _off = crate::tests::TestEnvGuard::set("ANGEL_TASK_CALL_TIMEOUT_SECS", "0");
        assert_eq!(super::task_call_budget(), None);
    }
    {
        let _legacy = crate::tests::TestEnvGuard::set("ANGEL_TEST_RUN_TIMEOUT_SECS", "0");
        assert_eq!(super::task_call_budget(), None);
    }
}

/// A suite killed at its budget names the tests that never finished, so the
/// model can fix the loop instead of re-running the same hang. Observed on
/// polyglot-v1 rust-decimal: every test touching zero spun forever.
#[test]
fn hung_suite_report_names_the_tests_that_never_finished() {
    let report = "tests: timed out after 180s — process group killed\n\
                  test sub_borrow ... ok\n\
                  test add_id has been running for over 60 seconds\n\
                  test eq has been running for over 60 seconds\n";
    let text = super::hung_suite_report(report);
    assert!(text.starts_with("tests: execution timed out"), "{text}");
    assert!(
        text.contains("2 test(s) were still running: add_id, eq."),
        "{text}"
    );
    // Where to look is the `⠨⠓` page on its own line.
    let hung = crate::agent::harness::book::d46_recovery::HUNG_SUITE;
    assert!(text.lines().any(|line| line == hung.cells()), "{text}");
    assert!(hung.text().contains("legitimate slow tests"));
    assert!(
        text.ends_with(report),
        "the original report follows: {text}"
    );
    let silent = super::hung_suite_report("tests: timed out after 3s — process group killed");
    assert!(silent.contains(&hung.cells()), "{silent}");
    assert!(
        !silent.contains("infinite loop or a blocking wait"),
        "the directive is the page: {silent}"
    );
    assert!(hung.text().contains("infinite loop or a blocking wait"));
    assert!(silent.contains("timed out after 3s"), "{silent}");
    assert!(
        !silent.contains("120s"),
        "must retain the actual timeout: {silent}"
    );
}

fn cmake_available() -> bool {
    ["cmake", "c++"]
        .iter()
        .all(|tool| crate::platform::workspace_lang::resolve_on_path(tool).is_some())
}

fn cmake_project(name: &str, main: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("angel-cmake-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.10)\nproject(t CXX)\nenable_testing()\n\
         add_executable(t t.cpp)\nadd_test(NAME t COMMAND t)\n",
    )
    .unwrap();
    std::fs::write(root.join("t.cpp"), main).unwrap();
    root
}

/// run_tests runs a CMake project's registered tests: configure, build, ctest.
#[test]
fn run_tests_runs_a_cmake_project() {
    if !cmake_available() {
        eprintln!("skipped: cmake or a C++ compiler is not installed");
        return;
    }
    let green = cmake_project("green", "int main() { return 0; }\n");
    let report = super::RunTestsTool::in_dir(green.clone())
        .call(&serde_json::json!({}))
        .expect("a passing suite");
    assert!(report.contains("1 passed, 0 failed"), "{report}");
    let _ = std::fs::remove_dir_all(green);

    let red = cmake_project("red", "int main() { return 1; }\n");
    let report = super::RunTestsTool::in_dir(red.clone()).call(&serde_json::json!({}));
    let text = match report {
        Ok(text) | Err(text) => text,
    };
    assert!(text.contains("tests failed out of 1"), "{text}");
    let _ = std::fs::remove_dir_all(red);
}

/// A build that runs its own tests (Exercism's C++ track) fails at the build
/// step, and CTest having nothing registered does not turn that green.
#[test]
fn run_tests_fails_a_cmake_build_whose_own_tests_fail() {
    if !cmake_available() {
        eprintln!("skipped: cmake or a C++ compiler is not installed");
        return;
    }
    let root = std::env::temp_dir().join(format!("angel-cmake-{}-selftest", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.10)\nproject(t CXX)\nadd_executable(t t.cpp)\n\
         add_custom_target(test_t ALL DEPENDS t COMMAND t)\n",
    )
    .unwrap();
    std::fs::write(root.join("t.cpp"), "int main() { return 1; }\n").unwrap();
    let report = super::RunTestsTool::in_dir(root.clone()).call(&serde_json::json!({}));
    let text = match &report {
        Ok(text) | Err(text) => text.clone(),
    };
    assert!(
        report.is_err() || text.to_lowercase().contains("fail"),
        "a failing self-testing build must not read as green: {text}"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A pin is read off the start-up path, but it is of the image captured: a
/// path replaced before the read finishes pins the original bytes or fails
/// closed, and the replacement always fails revalidation.
#[cfg(unix)]
#[test]
fn a_deferred_pin_hashes_the_captured_image_and_rejects_its_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("angel-deferred-pin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let tool = root.join("tool");
    std::fs::write(&tool, "#!/bin/sh\nprintf 'original\\n'\n").unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    let original = crate::knowledge::cut::sha256_hex(&std::fs::read(&tool).unwrap());
    let pinned = super::capture_executable(tool.clone(), "deferred pin").unwrap();
    // Replace the path (a new inode) while the digest may still be pending.
    let replacement = root.join("replacement");
    std::fs::write(&replacement, "#!/bin/sh\nprintf 'swapped!\\n'\n").unwrap();
    std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::rename(&replacement, &tool).unwrap();
    // Either the read finished first and pinned the original bytes, or it saw
    // the captured inode unlinked mid-pin and failed closed; never the swap.
    match pinned.digest() {
        Ok(digest) => assert_eq!(digest.map(String::as_str), Some(original.as_str())),
        Err(error) => assert!(error.contains("changed while being pinned"), "{error}"),
    }
    assert!(super::revalidate_executable(&pinned, "deferred pin").is_err());
    let _ = std::fs::remove_dir_all(root);
}

/// No pin captured before a dispatch is still pending when the tool runs.
#[test]
fn a_dispatch_settles_every_pending_pin() {
    let _lock = crate::tests::env_lock();
    let workspace = std::env::temp_dir().join(format!("angel-pin-settle-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let registry = crate::agent::harness::ToolRegistry::with_team(workspace.clone(), Vec::new());
    // PENDING_PINS is process-global and parallel tests keep capturing pins,
    // so judge only the digests that existed before this dispatch.
    let captured: Vec<_> = super::PENDING_PINS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let _ = registry.dispatch("word_count", &serde_json::json!({"text":"one two"}));
    assert!(
        captured.iter().all(|pin| pin.settled.get().is_some()),
        "a tool ran with a pin still being read"
    );
    let _ = std::fs::remove_dir_all(workspace);
}
