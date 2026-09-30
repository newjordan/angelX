use super::*;

#[test]
fn task_accept_rejects_green_prefix_when_stdout_or_stderr_exceeds_capture_limit() {
    for stream in ["stdout", "stderr"] {
        let mut command = std::process::Command::new("python3");
        command.args([
            "-c",
            "import os, sys; os.write(1, b'test result: ok. 1 passed; 0 failed\\n'); os.write(1 if sys.argv[1] == 'stdout' else 2, b'x' * (1024 * 1024 + 1))",
            stream,
        ]);
        let started = Instant::now();
        let capture = crate::agent::harness::exec::output_timed_captured(
            command,
            Some(Duration::from_secs(5)),
        );
        let result = task_accept_result(capture, started);
        assert!(!result.passed, "{stream}: {result:?}");
        assert_eq!(result.result_class, "incomplete", "{result:?}");
        assert!(result.summary.contains("1 passed / 0 failed"), "{result:?}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn task_accept_incomplete_output_fixture() {
    use crate::agent::service_process::tests::{
        FixtureCleanup, capture_evidence_fixture::EvidenceFixture,
    };
    let Some(root) = std::env::var_os("ANGEL_T_ACCEPT_INCOMPLETE_OUTPUT") else {
        return;
    };
    let _cleanup = FixtureCleanup::new();
    let _env = crate::tests::env_lock();
    let _repeats = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACCEPT_REPEATS", "3");
    let root = Path::new(&root);
    for stream in ["stdout", "stderr"] {
        for earlier_pass in [false, true] {
            let mut fixture = EvidenceFixture::new(
                root,
                if earlier_pass { "repeat" } else { "first" },
                stream,
                "test result: ok. 1 passed; 0 failed\n",
            );
            let mut runs = 0;
            // Exercise actual pipe capture and the acceptance evidence consumer
            // directly. The public sandbox path has its own descendant cleanup
            // and can legitimately complete both pipes before capture returns.
            let result = run_task_accept_with(|| {
                runs += 1;
                let command = if earlier_pass && runs == 1 {
                    let mut command = std::process::Command::new("sh");
                    command.args(["-c", "printf 'test result: ok. 1 passed; 0 failed\\n'"]);
                    command
                } else {
                    fixture.command()
                };
                let started = Instant::now();
                let capture = crate::agent::harness::exec::output_timed_captured(
                    command,
                    Some(Duration::from_secs(5)),
                );
                task_accept_result(capture, started)
            });
            fixture.finish();
            assert!(!result.passed, "{stream}/{earlier_pass}: {result:?}");
            assert_eq!(result.result_class, "incomplete", "{result:?}");
            assert!(result.summary.contains("capture incomplete"), "{result:?}");
            assert!(result.output_tail.contains("1 passed"), "{result:?}");
            assert_eq!(runs, if earlier_pass { 2 } else { 1 });
            if earlier_pass {
                assert!(
                    result.summary.contains("after 1 completed pass"),
                    "{result:?}"
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn task_accept_rejects_incomplete_stdout_or_stderr_without_calling_it_flaky() {
    let filter = format!(
        "{}::task_accept_incomplete_output_fixture",
        module_path!().split_once("::").unwrap().1
    );
    crate::agent::service_process::tests::isolated_fixture(
        &filter,
        "ANGEL_T_ACCEPT_INCOMPLETE_OUTPUT",
    );
}
