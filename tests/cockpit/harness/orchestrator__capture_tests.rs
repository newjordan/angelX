use super::*;

#[test]
fn complete_delegate_command_keeps_metadata_digest_and_failure_behavior() {
    let mut command = Command::new("sh");
    command.args(["-c", "printf abc"]);
    assert_eq!(delegate_git_command(command, None).unwrap(), "abc");
    let mut command = Command::new("sh");
    command.args(["-c", "printf abc"]);
    assert_eq!(
        delegate_diff_command(command, None).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let mut command = Command::new("sh");
    command.args(["-c", "printf 'red diagnostic' >&2; exit 17"]);
    assert!(
        delegate_git_command(command, None)
            .unwrap_err()
            .contains("red diagnostic")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn delegate_incomplete_pipe_fixture() {
    use crate::agent::service_process::tests::{
        FixtureCleanup, capture_evidence_fixture::EvidenceFixture,
    };
    let Some(root) = std::env::var_os("ANGEL_T_DELEGATE_EOF") else {
        return;
    };
    let _cleanup = FixtureCleanup::new();
    let root = Path::new(&root);
    for kind in ["metadata", "diff"] {
        for stream in ["stdout", "stderr"] {
            let mut fixture = EvidenceFixture::new(root, kind, stream, "partial evidence\n");
            let started = Instant::now();
            let result = if kind == "metadata" {
                delegate_git_command(fixture.command(), None)
            } else {
                delegate_diff_command(fixture.command(), None)
            };
            let elapsed = started.elapsed();
            fixture.finish();
            assert!(
                elapsed < Duration::from_secs(3),
                "{kind}/{stream}: {elapsed:?}"
            );
            assert!(
                result.unwrap_err().contains("did not complete"),
                "{kind}/{stream}"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn delegate_evidence_rejects_missing_stdout_or_stderr_eof() {
    let filter = format!(
        "{}::delegate_incomplete_pipe_fixture",
        module_path!().split_once("::").unwrap().1
    );
    crate::agent::service_process::tests::isolated_fixture(&filter, "ANGEL_T_DELEGATE_EOF");
}
