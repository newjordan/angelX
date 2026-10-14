use super::*;

fn budget() -> Option<Duration> {
    Some(Duration::from_secs(100))
}

fn obs(secs: u64, mutated: bool, green: bool, missing: Option<usize>) -> Observation {
    Observation {
        elapsed: Duration::from_secs(secs),
        mutated_this_hop: mutated,
        green,
        artifacts_missing: missing,
    }
}

#[test]
fn no_budget_is_always_quiet() {
    let mut s = Sentinel::new(None, SentinelMode::Intervene);
    assert_eq!(s.observe(obs(10_000, false, false, Some(3))), Verdict::Quiet);
    let mut zero = Sentinel::new(Some(Duration::ZERO), SentinelMode::Intervene);
    assert_eq!(zero.observe(obs(10, false, false, None)), Verdict::Quiet);
}

#[test]
fn off_mode_never_fires() {
    let mut s = Sentinel::new(budget(), SentinelMode::Off);
    assert_eq!(s.observe(obs(99, false, false, Some(1))), Verdict::Quiet);
}

#[test]
fn early_hops_are_quiet_and_drift_fires_once() {
    let mut s = Sentinel::new(budget(), SentinelMode::Observe);
    assert_eq!(s.observe(obs(10, false, false, None)), Verdict::Quiet);
    assert!(matches!(
        s.observe(obs(50, false, false, None)),
        Verdict::Drift { budget_spent_bp: 5_000 }
    ));
    // Latched: a later hop says nothing more.
    assert_eq!(s.observe(obs(60, false, false, None)), Verdict::Quiet);
}

#[test]
fn an_edit_before_the_midpoint_disarms_drift() {
    let mut s = Sentinel::new(budget(), SentinelMode::Observe);
    assert_eq!(s.observe(obs(20, true, false, None)), Verdict::Quiet);
    assert_eq!(s.observe(obs(70, false, false, None)), Verdict::Quiet);
}

#[test]
fn green_silences_both_verdicts() {
    let mut s = Sentinel::new(budget(), SentinelMode::Intervene);
    assert_eq!(s.observe(obs(60, false, true, Some(2))), Verdict::Quiet);
    assert_eq!(s.observe(obs(95, false, true, Some(2))), Verdict::Quiet);
}

#[test]
fn jumping_straight_past_deliver_fires_deliver_not_drift() {
    let mut s = Sentinel::new(budget(), SentinelMode::Observe);
    assert!(matches!(
        s.observe(obs(90, false, false, None)),
        Verdict::Deliver { budget_spent_bp: 9_000, artifacts_missing: None }
    ));
    assert_eq!(s.observe(obs(95, false, false, None)), Verdict::Quiet);
}

#[test]
fn deliver_after_edits_needs_named_artifacts_missing() {
    // Edits landed and every named artifact exists: stay quiet.
    let mut landed = Sentinel::new(budget(), SentinelMode::Observe);
    landed.observe(obs(30, true, false, Some(0)));
    assert_eq!(landed.observe(obs(95, false, false, Some(0))), Verdict::Quiet);
    // Edits landed, artifacts un-named: nothing factual to demand.
    let mut unnamed = Sentinel::new(budget(), SentinelMode::Observe);
    unnamed.observe(obs(30, true, false, None));
    assert_eq!(unnamed.observe(obs(95, false, false, None)), Verdict::Quiet);
    // Edits landed but a named artifact is absent: deliver.
    let mut missing = Sentinel::new(budget(), SentinelMode::Observe);
    missing.observe(obs(30, true, false, Some(1)));
    assert!(matches!(
        missing.observe(obs(90, false, false, Some(1))),
        Verdict::Deliver { artifacts_missing: Some(1), .. }
    ));
}

#[test]
fn mode_parsing_fails_toward_observe() {
    assert_eq!(parse_mode("off"), SentinelMode::Off);
    assert_eq!(parse_mode(" 0 "), SentinelMode::Off);
    assert_eq!(parse_mode("intervene"), SentinelMode::Intervene);
    assert_eq!(parse_mode("ON"), SentinelMode::Intervene);
    assert_eq!(parse_mode("observe"), SentinelMode::Observe);
    assert_eq!(parse_mode("bogus"), SentinelMode::Observe);
    assert_eq!(parse_mode(""), SentinelMode::Observe);
}

#[test]
fn missing_artifacts_counts_only_named_absences() {
    let dir = std::env::temp_dir().join("angel-sentinel-test");
    let present = PathBuf::from("present.txt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(&present), "x").unwrap();
    assert_eq!(missing_artifacts(&[], &dir), None);
    assert_eq!(
        missing_artifacts(&[present, PathBuf::from("absent.txt")], &dir),
        Some(1)
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn edits_elsewhere_do_not_disarm_drift_when_a_named_artifact_is_missing() {
    let mut s = Sentinel::new(budget(), SentinelMode::Intervene);
    // An edit landed at 20s, but the named deliverable is still absent:
    // Drift must still fire at the midpoint (the v1 failure mode).
    assert_eq!(s.observe(obs(20, true, false, Some(1))), Verdict::Quiet);
    assert!(matches!(
        s.observe(obs(55, false, false, Some(1))),
        Verdict::Drift { .. }
    ));
}

#[test]
fn gate_blocks_only_when_armed_actionable_and_budgeted() {
    // Armed + missing artifact + budget left: block, twice at most.
    assert!(gate_should_block(true, 1, 0, false, false, 0, true));
    assert!(gate_should_block(true, 1, 0, false, false, 1, true));
    assert!(!gate_should_block(true, 1, 0, false, false, GATE_MAX_BLOCKS, true));
    // Failed liveness, exact-dir violation, or an Angel-owned server each
    // block alone; all-clear never does.
    assert!(gate_should_block(true, 0, 0, true, false, 0, true));
    assert!(gate_should_block(true, 0, 1, false, false, 0, true));
    assert!(gate_should_block(true, 0, 0, false, true, 0, true));
    assert!(!gate_should_block(true, 0, 0, false, false, 0, true));
    // Observe-only and no-budget-left turns are never blocked.
    assert!(!gate_should_block(false, 2, 1, true, true, 0, true));
    assert!(!gate_should_block(true, 2, 1, true, true, 0, false));
}

#[test]
fn exact_dir_contracts_parse_and_report_extras() {
    let specs = parse_exact_dirs("/app/polyglot=main.rs; rel=out.txt+log.txt ; bogus");
    assert_eq!(specs.len(), 2);
    let dir = std::env::temp_dir().join("angel-sentinel-exact-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.rs"), "x").unwrap();
    std::fs::write(dir.join("cmain"), "x").unwrap();
    let spec = vec![ExactDir {
        dir: dir.clone(),
        expected: ["main.rs".to_string()].into_iter().collect(),
    }];
    let violations = exact_dir_violations(&spec, Path::new("/"));
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("extra: [cmain]"), "{violations:?}");
    std::fs::remove_file(dir.join("cmain")).unwrap();
    assert!(exact_dir_violations(&spec, Path::new("/")).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_artifact_paths_names_what_is_absent() {
    let dir = std::env::temp_dir().join("angel-sentinel-gate-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("here.txt"), "x").unwrap();
    let named = vec![PathBuf::from("here.txt"), PathBuf::from("gone.txt")];
    assert_eq!(
        missing_artifact_paths(&named, &dir),
        vec![dir.join("gone.txt")]
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn verdicts_describe_themselves_for_the_notice_stream() {
    assert!(Verdict::Drift { budget_spent_bp: 5_500 }.describe().contains("55%"));
    assert!(
        Verdict::Deliver { budget_spent_bp: 9_000, artifacts_missing: Some(2) }
            .describe()
            .contains("2 named artifact")
    );
}
