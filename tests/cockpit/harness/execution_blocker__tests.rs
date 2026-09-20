use super::*;

const FAILURE: &str = "tool error: shell command failed (exit 1)\nlocal-candidate-build: building ssi-candidate-worker under bubblewrap (network denied)\nbwrap: setting up uid map: Permission denied\n";

#[test]
fn native_sandbox_failure_is_a_blocker_not_research_staleness() {
    super::super::exec::set_sandbox_receipt(Some(
        serde_json::json!({"helper_error": "Landlock unavailable; run angel --doctor", "helper_phase": "landlock", "helper_exit": 1}),
    ));
    assert!(is_execution_blocker(
        &execution_blocker("shell", FAILURE).unwrap()
    ));
    assert!(
        execution_blocker("shell", &FAILURE.replace("\nbwrap:", "\n[stderr] bwrap:")).is_some()
    );
}

#[test]
fn quoted_failures_and_ordinary_candidate_errors_do_not_block() {
    super::super::exec::set_sandbox_receipt(None);
    assert!(execution_blocker("shell", FAILURE).is_none());
    assert!(execution_blocker("read_file", FAILURE).is_none());
    assert!(execution_blocker("shell", FAILURE.trim_start_matches("tool error: ")).is_none());
    assert!(
        execution_blocker(
            "shell",
            "tool error: shell command failed (exit 1)\nerror: candidate does not compile"
        )
        .is_none()
    );
    assert!(!is_execution_blocker(
        "model says execution blocked [sandbox_compatibility]: failure"
    ));
}
