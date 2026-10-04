use super::*;

const GREEN: &str = "test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";

fn shell(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", script]);
    command
}

fn complete_tests() -> Command {
    shell(&format!("printf '{}'", GREEN))
}

#[test]
fn self_gate_complete_commands_preserve_green_red_and_baseline_verdicts() {
    let (green, detail) = run_self_gate_commands(shell("true"), complete_tests(), Some(9));
    assert!(green.passed, "{}: {detail}", green.summary);
    assert_eq!(green.reward, 1.0);
    assert!(detail.is_empty());

    let (baseline, _) = run_self_gate_commands(shell("true"), complete_tests(), Some(10));
    assert!(!baseline.passed);
    assert!(baseline.summary.contains("baseline"));

    let red = shell(
        "printf 'test result: FAILED. 3 passed; 1 failed; 0 ignored\\n'; printf 'error: failing test\\n' >&2; exit 1",
    );
    let (red, detail) = run_self_gate_commands(shell("true"), red, None);
    assert!(!red.passed);
    assert_eq!(red.reward, 0.75);
    assert!(red.summary.contains("1 of 4 tests failed"));
    assert!(detail.contains("failing test"));

    let (build, detail) = run_self_gate_commands(
        shell("printf 'error: build failure\\n' >&2; exit 17"),
        complete_tests(),
        None,
    );
    assert!(!build.passed);
    assert_eq!(build.reward, 0.0);
    assert!(detail.contains("build failure"));
}

#[test]
fn self_gate_rejects_complete_green_summary_from_failed_test_command() {
    let tests = shell(&format!(
        "printf '{}'; printf 'error: cargo failed after tests\\n' >&2; exit 17",
        GREEN
    ));
    let (verdict, detail) = run_self_gate_commands(shell("true"), tests, Some(9));
    assert!(!verdict.passed, "{}: {detail}", verdict.summary);
    assert!(verdict.summary.contains("nonzero"), "{}", verdict.summary);
    assert!(detail.contains("cargo failed after tests"), "{detail}");
}

#[test]
fn self_gate_rejects_capture_byte_limit_in_build_or_tests() {
    for stage in ["build", "test"] {
        for stream in ["stdout", "stderr"] {
            let mut noisy = Command::new("/usr/bin/python3");
            noisy.args([
                "-c",
                &format!(
                    "import os; os.write(1, {}.encode()); os.write({}, b'x' * 1048577)",
                    serde_json::to_string(GREEN).unwrap(),
                    if stream == "stdout" { 1 } else { 2 }
                ),
            ]);
            let (build, tests) = if stage == "build" {
                (noisy, complete_tests())
            } else {
                (shell("true"), noisy)
            };
            let (verdict, detail) = run_self_gate_commands(build, tests, Some(9));
            assert!(
                !verdict.passed,
                "{stage}/{stream}: {}: {detail}",
                verdict.summary
            );
            assert!(detail.contains("incomplete"), "{stage}/{stream}: {detail}");
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn self_gate_incomplete_pipe_fixture() {
    use crate::agent::service_process::tests::{
        FixtureCleanup, capture_evidence_fixture::EvidenceFixture,
    };
    let root = std::env::var_os("ANGEL_T_SELF_GATE_EOF")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    let root = Path::new(&root);
    for stage in ["build", "test"] {
        for stream in ["stdout", "stderr"] {
            let mut fixture = EvidenceFixture::new(root, stage, stream, GREEN);
            let test_marker = root.join(format!("tests-{stream}.ran"));
            let (build, tests) = if stage == "build" {
                let mut tests = shell(&format!("printf '{}'; printf ran > \"$1\"", GREEN));
                tests.args(["test-marker", test_marker.to_str().unwrap()]);
                (fixture.command(), tests)
            } else {
                (shell("true"), fixture.command())
            };
            let started = std::time::Instant::now();
            let (verdict, detail) = run_self_gate_commands(build, tests, Some(9));
            let elapsed = started.elapsed();
            fixture.finish();
            assert!(
                elapsed < std::time::Duration::from_secs(3),
                "{stage}/{stream}: {elapsed:?}"
            );
            assert!(
                !verdict.passed,
                "{stage}/{stream}: {}: {detail}",
                verdict.summary
            );
            assert!(detail.contains("incomplete"), "{stage}/{stream}: {detail}");
            if stage == "build" {
                assert!(!test_marker.exists(), "tests ran after incomplete build");
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn self_gate_rejects_missing_stdout_or_stderr_eof_before_acceptance() {
    let filter = format!(
        "{}::self_gate_incomplete_pipe_fixture",
        module_path!().split_once("::").unwrap().1
    );
    crate::agent::service_process::tests::isolated_fixture(&filter, "ANGEL_T_SELF_GATE_EOF");
}
