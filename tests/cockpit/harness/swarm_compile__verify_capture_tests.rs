use super::*;
use std::time::{Duration, Instant};

#[test]
fn complete_verifier_captures_preserve_green_and_red_process_evidence() {
    for status in [0, 17] {
        let mut command = Command::new("sh");
        command.args(["-c", &format!("printf 'test result: ok. 2 passed; 0 failed;\\n'; printf 'red diagnostic' >&2; exit {status}")]);
        let capture = capture_verifier_output(command, Duration::from_secs(5)).unwrap();
        assert_eq!(capture.output.status.code(), Some(status));
        assert!(capture.output_complete());
        assert!(String::from_utf8_lossy(&capture.output.stdout).contains("2 passed"));
        assert_eq!(capture.output.stderr, b"red diagnostic");
    }
}

#[test]
fn verifier_evidence_rejects_either_streams_capture_byte_limit() {
    for stream in ["stdout", "stderr"] {
        let mut command = Command::new("python3");
        command.args([
            "-c",
            &format!("import sys; sys.{stream}.write('x' * (1024 * 1024 + 1))"),
        ]);
        assert!(
            capture_verifier_output(command, Duration::from_secs(5))
                .unwrap_err()
                .contains("capture byte limit"),
            "{stream}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn verifier_incomplete_pipe_fixture() {
    use crate::agent::service_process::tests::{
        FixtureCleanup, capture_evidence_fixture::EvidenceFixture,
    };
    let Some(root) = std::env::var_os("ANGEL_T_VERIFIER_EOF") else {
        return;
    };
    let _cleanup = FixtureCleanup::new();
    for stream in ["stdout", "stderr"] {
        let mut fixture = EvidenceFixture::new(
            Path::new(&root),
            "verifier",
            stream,
            "PROOF_MARKER\ntest result: ok. 2 passed; 0 failed;\n",
        );
        let started = Instant::now();
        let result = capture_verifier_output(fixture.command(), Duration::from_secs(5));
        let elapsed = started.elapsed();
        fixture.finish();
        assert!(elapsed < Duration::from_secs(3));
        assert!(
            result
                .unwrap_err()
                .contains("verification output is incomplete"),
            "{stream}: passing prefix is not completed evidence"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn verification_rejects_passing_prefix_with_missing_stdout_or_stderr_eof() {
    let filter = format!(
        "{}::verifier_incomplete_pipe_fixture",
        module_path!().split_once("::").unwrap().1
    );
    crate::agent::service_process::tests::isolated_fixture(&filter, "ANGEL_T_VERIFIER_EOF");
}
