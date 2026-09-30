use super::*;

fn shell(script: &str) -> std::process::Command {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", script]);
    command
}

fn drain_build_result(result: Result<PathBuf, String>) -> crate::App {
    let mut app = crate::seed_preview_app();
    let (tx, rx) = std::sync::mpsc::channel();
    app.reborn_rx = Some(rx);
    tx.send(result).unwrap();
    app.drain_reborn();
    assert!(app.reborn_rx.is_none());
    app
}

#[test]
fn complete_reborn_build_preserves_restart_and_failure_diagnostic() {
    let executable = PathBuf::from("/offline/angel");
    let result = reborn_build(shell("printf 'complete build'"), executable.clone()).unwrap();
    assert_eq!(result, executable);
    let app = drain_build_result(Ok(result));
    assert_eq!(app.reborn_exec, Some(executable.clone()));
    assert!(app.should_quit);

    let error = reborn_build(
        shell("printf 'first line\n  actionable red tail  \n' >&2; exit 17"),
        executable,
    )
    .unwrap_err();
    assert_eq!(error, "actionable red tail");
    let app = drain_build_result(Err(error));
    assert!(app.reborn_exec.is_none());
    assert!(!app.should_quit);
}

#[test]
fn capped_reborn_build_does_not_stage_restart() {
    for stream in [1, 2] {
        let mut command = std::process::Command::new("/usr/bin/python3");
        command.args([
            "-c",
            &format!("import os; os.write({stream}, b'x' * 1048577)"),
        ]);
        let error = reborn_build(command, PathBuf::from("/offline/angel")).unwrap_err();
        assert!(error.contains("capture incomplete"), "{stream}: {error}");
        let app = drain_build_result(Err(error));
        assert!(app.reborn_exec.is_none());
        assert!(!app.should_quit);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn reborn_incomplete_pipe_fixture() {
    use crate::agent::process_test_support::{
        FixtureCleanup, capture_evidence_fixture::EvidenceFixture,
    };
    let Some(root) = std::env::var_os("ANGEL_T_REBORN_EOF") else {
        return;
    };
    let _cleanup = FixtureCleanup::new();
    let root = Path::new(&root);
    for stream in ["stdout", "stderr"] {
        let mut fixture = EvidenceFixture::new(root, "reborn", stream, "offline build complete\n");
        let started = std::time::Instant::now();
        let result = reborn_build(fixture.command(), root.join("offline-angel"));
        let elapsed = started.elapsed();
        fixture.finish();
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "{stream}: {elapsed:?}"
        );
        let error = result.unwrap_err();
        assert!(error.contains("capture incomplete"), "{stream}: {error}");
        let app = drain_build_result(Err(error));
        assert!(app.reborn_exec.is_none());
        assert!(!app.should_quit);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn reborn_build_requires_stdout_and_stderr_eof_before_restart() {
    let filter = format!(
        "{}::reborn_incomplete_pipe_fixture",
        module_path!().split_once("::").unwrap().1
    );
    crate::agent::process_test_support::isolated_fixture(&filter, "ANGEL_T_REBORN_EOF");
}
