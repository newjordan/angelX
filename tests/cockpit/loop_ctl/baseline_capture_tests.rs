#[cfg(target_os = "linux")]
use super::*;

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn baseline_incomplete_pipe_fixture() {
    use crate::agent::process_test_support::{
        FixtureCleanup, capture_evidence_fixture::EvidenceFixture,
    };
    let root = std::env::var_os("ANGEL_T_BASELINE_EOF")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    let root = Path::new(&root);
    for stream in ["stdout", "stderr"] {
        let mut fixture = EvidenceFixture::new(
            root,
            "baseline",
            stream,
            "test result: ok. 7 passed; 0 failed;\n",
        );
        let capture = crate::agent::harness::output_timed_captured(
            fixture.command(),
            Some(Duration::from_secs(5)),
        )
        .unwrap();
        fixture.finish();
        assert_eq!(count_passed_from_capture(&capture), 0, "{stream}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn baseline_requires_stdout_and_stderr_eof_before_counting_tests() {
    let filter = format!(
        "{}::baseline_incomplete_pipe_fixture",
        module_path!().split_once("::").unwrap().1
    );
    crate::agent::process_test_support::isolated_fixture(&filter, "ANGEL_T_BASELINE_EOF");
}
