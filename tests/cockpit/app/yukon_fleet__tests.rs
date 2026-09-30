use super::*;

#[test]
fn cli_pipe_capture_drains_beyond_its_retained_prefix() {
    let payload = vec![b'x'; MAX_CLI_OUTPUT_BYTES + 256 * 1024];
    let captured = join_pipe(drain_pipe(std::io::Cursor::new(payload)), "fixture").unwrap();
    assert_eq!(captured.bytes.len(), MAX_CLI_OUTPUT_BYTES);
    assert!(captured.bytes.iter().all(|byte| *byte == b'x'));
    assert!(captured.overflow);

    let exact = join_pipe(
        drain_pipe(std::io::Cursor::new(vec![b'y'; MAX_CLI_OUTPUT_BYTES])),
        "exact fixture",
    )
    .unwrap();
    assert_eq!(exact.bytes.len(), MAX_CLI_OUTPUT_BYTES);
    assert!(!exact.overflow);
}

#[cfg(unix)]
#[test]
fn local_cli_capture_preserves_complete_status_and_output_rules() {
    let mut command = Command::new("python3");
    command.args(["-c", "print('\\033[32mcomplete output\\033[0m')"]);
    assert_eq!(
        run_yukon_command(&mut command, false, Duration::from_secs(2)).unwrap(),
        "complete output\n"
    );

    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import sys; print('reported failure', file=sys.stderr); sys.exit(17)",
    ]);
    assert_eq!(
        run_yukon_command(&mut command, false, Duration::from_secs(2)).unwrap_err(),
        "Yukon CLI: reported failure"
    );

    for allowed in [false, true] {
        let mut command = Command::new("python3");
        command.args(["-c", "raise SystemExit(17)"]);
        let result = run_yukon_command(&mut command, allowed, Duration::from_secs(2));
        if allowed {
            assert_eq!(result.unwrap(), "");
        } else {
            assert!(result.unwrap_err().contains("Yukon CLI exited with"));
        }
    }
}

#[cfg(unix)]
#[test]
fn local_cli_rejects_overflow_on_either_output_stream() {
    for stream in ["stdout", "stderr"] {
        let mut command = Command::new("python3");
        command.args([
            "-c",
            &format!(
                "import sys; sys.{stream}.write('x' * {})",
                MAX_CLI_OUTPUT_BYTES + 1
            ),
        ]);
        assert_eq!(
            run_yukon_command(&mut command, false, Duration::from_secs(2)).unwrap_err(),
            "Yukon CLI output exceeded the watcher limit"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn fleet_cli_ownership_fixture() {
    use crate::agent::service_process::tests::{FixtureCleanup, ServiceFixture};
    let Some(root) = std::env::var_os("ANGEL_T_FLEET_PIPE_OWNERSHIP") else {
        return;
    };
    let _cleanup = FixtureCleanup::new();
    let root = std::path::Path::new(&root);
    for mode in ["same-group", "escaped-stdout", "escaped-stderr", "running"] {
        let marker = root.join(format!("{mode}.pid"));
        let mut command = Command::new("python3");
        command.args([
            "-c",
            r#"import os, signal, sys, time
from pathlib import Path
mode, marker = sys.argv[1], Path(sys.argv[2])
read_fd, write_fd = os.pipe()
descendant = os.fork()
if descendant == 0:
    os.close(read_fd)
    if mode.startswith('escaped'):
        os.setsid()
        os.close(2 if mode == 'escaped-stdout' else 1)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    os.write(write_fd, b'ready')
    os.close(write_fd)
    time.sleep(30)
    os._exit(0)
os.close(write_fd)
os.read(read_fd, 5)
os.close(read_fd)
marker.write_text(f'{os.getpid()} {descendant} {os.getpgrp()}')
print('bounded complete output', flush=True)
if mode == 'running':
    time.sleep(30)
os._exit(0)
"#,
            mode,
            marker.to_str().unwrap(),
        ]);
        let started = Instant::now();
        let result = run_yukon_command(&mut command, true, Duration::from_millis(150));
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(700),
            "{mode} {elapsed:?}: {result:?}"
        );
        let pids = std::fs::read_to_string(marker)
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse::<i32>().unwrap())
            .collect::<Vec<_>>();
        let [leader, descendant, group] = pids.as_slice() else {
            panic!("fixture identity receipt malformed");
        };
        assert_eq!(leader, group, "CLI must own a private group");
        ServiceFixture::assert_reaped(*leader);
        if mode == "same-group" {
            assert_eq!(result.unwrap(), "bounded complete output\n");
        } else if mode.starts_with("escaped") {
            assert!(
                result
                    .unwrap_err()
                    .contains("read Yukon CLI output: timed out")
            );
            // Escaped groups remain outside service retirement. This isolated
            // subreaper owns the fixture's final cleanup, including assertions.
            assert!(std::path::Path::new(&format!("/proc/{descendant}/stat")).exists());
            unsafe { libc::kill(*descendant, libc::SIGKILL) };
        } else {
            assert!(result.unwrap_err().contains("Yukon CLI timed out"));
        }
        ServiceFixture::assert_reaped(*descendant);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn fleet_cli_group_cleanup_and_pipe_eof_share_the_command_budget() {
    let filter = format!(
        "{}::fleet_cli_ownership_fixture",
        module_path!().split_once("::").unwrap().1
    );
    crate::agent::service_process::tests::isolated_fixture(&filter, "ANGEL_T_FLEET_PIPE_OWNERSHIP");
}

#[test]
fn parses_open_benchmarks_from_colored_table() {
    let table = "\u{1b}[2mbenchmark status\u{1b}[22m\n\
                     eigenlabs/flock-challenge \u{1b}[32mopen\u{1b}[39m rust\n\
                     eigenlabs/closed-challenge closed rust\n\
                     Lulu-Zhou-EigenLabs/ssi-ordering-challenge open optimization\n";
    assert_eq!(
        parse_benchmark_list(&strip_ansi(table)),
        vec![
            "eigenlabs/flock-challenge",
            "Lulu-Zhou-EigenLabs/ssi-ordering-challenge"
        ]
    );
}

#[test]
fn parses_bounded_explicit_benchmark_scope() {
    assert_eq!(
        parse_benchmark_scope(
            " davidtai/mlxfast-gemma4-26b-a4b,\
                 eigenlabs/flock-challenge-multi/x86,\
                 davidtai/mlxfast-gemma4-26b-a4b "
        )
        .unwrap(),
        vec![
            "davidtai/mlxfast-gemma4-26b-a4b",
            "eigenlabs/flock-challenge-multi/x86"
        ]
    );
    assert!(parse_benchmark_scope(" , ").is_err());
    assert!(parse_benchmark_scope("setter/valid,not a benchmark").is_err());
}

#[test]
fn parses_personal_submission_statuses_and_scores() {
    let table = "submission solver status score metrics diff commit created\n\
                     3347e70 newjordan validating n/a n/a n/a - now\n\
                     7871bd4 newjordan promoted 519469.35 {} +1% 380b04d yesterday\n\
                     ddddddd newjordan rejected n/a n/a n/a deadbee yesterday\n";
    let rows = parse_submission_table("eigenlabs/flock-challenge", table);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].phase, YukonSubmissionPhase::Running);
    assert_eq!(rows[1].phase, YukonSubmissionPhase::Accepted);
    assert_eq!(rows[1].score.as_deref(), Some("519469.35"));
    assert_eq!(rows[2].phase, YukonSubmissionPhase::Rejected);
}

#[test]
fn parses_multiword_promotion_failure_status_and_score() {
    let table = "submission solver status score metrics diff commit created\n\
                     2a5c395 newjordan promotion failed 116.55 {} -0.22% 0331e8b yesterday\n";
    let rows = parse_submission_table("proximity-prize/upper", table);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "promotion failed");
    assert_eq!(rows[0].phase, YukonSubmissionPhase::Rejected);
    assert_eq!(rows[0].score.as_deref(), Some("116.55"));
}

#[test]
fn latest_submission_keeps_one_current_frontier_per_benchmark() {
    let table = "submission solver status score metrics diff commit created\n\
                     aaaaaaa newjordan promoted 1.0 {} +1% 1111111 yesterday\n\
                     bbbbbbb newjordan rejected 1.1 {} -1% 2222222 today\n\
                     ccccccc newjordan validating n/a n/a n/a - now\n";
    let latest = latest_submission("bench/frontier", table).unwrap();
    assert_eq!(latest.id, "ccccccc");
    assert_eq!(latest.phase, YukonSubmissionPhase::Running);
}

#[test]
fn fleet_failure_preserves_last_good_rows_as_stale() {
    let mut state = YukonFleetState::default();
    state.apply(YukonFleetSnapshot {
        entries: parse_submission_table("bench/fleet", "aaaaaaa jordan accepted 1.0 rest\n"),
        benchmark_count: 1,
        failed_benchmarks: 0,
    });
    state.fail("offline".into());
    assert_eq!(state.entries().len(), 1);
    assert!(state.stale());
    assert!(state.has_error());
}
