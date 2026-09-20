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
