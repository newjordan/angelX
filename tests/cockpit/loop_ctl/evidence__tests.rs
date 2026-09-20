use super::super::DEFAULT_LOOP_MAX_ITERS;
use super::*;

fn block(dir: &str, findings: &[&str]) -> String {
    let mut s = format!("DIRECTION: {dir}\nFINDINGS:\n");
    for f in findings {
        s.push_str(&format!("- {f} [evidence: file:src/drive/loop_ctl.rs:1]\n"));
    }
    s
}

#[test]
fn submission_journal_preserves_attempts_without_counting_receipts_twice() {
    let _env = crate::tests::env_lock();
    let workspace = std::env::temp_dir().join(format!("angel-journal-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let _owner =
        crate::agent::harness::run_identity::LiveTurnScope::enter(Some("journal-fixture".into()));
    let _model = crate::agent::harness::run_identity::LiveModelScope::enter(Some(
        "deepseek-v4-flash".into(),
    ));
    let _ = crate::agent::tools::submit_identity::drain_journal();
    for (exit, output) in [
        (2, "refused"),
        (0, "rejected"),
        (0, "not accepted"),
        (0, "Submission queued\n11111111-2222-3333-4444-555555555555"),
    ] {
        crate::agent::tools::submit_identity::journal_execution(
            "shell",
            "yukon submit",
            Some(&workspace),
            Some(exit),
            output,
        );
    }
    let mut st = LoopState::default();
    drain_submission_journal(&mut st);
    assert!(
        st.submissions_log.is_empty(),
        "unbound loops cannot claim evidence"
    );
    st.workspace = Some(workspace.clone());
    drain_submission_journal(&mut st);
    assert!(
        st.submissions_log.is_empty(),
        "another loop in the same checkout cannot claim evidence"
    );
    st.id = "journal-fixture".into();
    drain_submission_journal(&mut st);
    assert_eq!(st.submissions, 0);
    assert_eq!(
        st.submissions_log
            .iter()
            .map(|row| row.outcome.as_str())
            .collect::<Vec<_>>(),
        ["refused", "rejected", "unknown", "dispatched"]
    );
    let receipts = vec!["submitted:11111111-2222-3333-4444-555555555555".into()];
    assert_eq!(
        register_verified_outcome_actions(&mut st, &receipts),
        (1, 1)
    );
    drain_submission_journal(&mut st);
    assert_eq!(
        register_verified_outcome_actions(&mut st, &receipts),
        (0, 0)
    );
    assert_eq!(st.submissions, 1);
    assert_eq!(st.submissions_log.len(), 4);
}

#[test]
fn apply_reply_accumulates_distinct_and_tracks_stall() {
    let mut st = LoopState::default();
    assert_eq!(apply_reply(&mut st, &block("a", &["f1", "f2"])), 2);
    assert_eq!(apply_reply(&mut st, &block("b", &["f3"])), 1);
    // Repeat of a known finding → no fresh ground → stall climbs.
    assert_eq!(apply_reply(&mut st, &block("c", &["f1"])), 0);
    assert_eq!(st.findings.len(), 3);
    assert_eq!(st.iteration, 3);
    assert_eq!(st.stale_count, 1);
    assert_eq!(st.directions_tried, vec!["a", "b", "c"]);
}

#[test]
fn unsupported_novelty_is_a_hypothesis_not_progress() {
    let mut st = LoopState::default();
    let reply = "DIRECTION: speculate\nFINDINGS:\n- top competitors probably use a library call";
    assert_eq!(apply_reply(&mut st, reply), 0);
    assert!(st.findings.is_empty());
    assert_eq!(st.hypotheses.len(), 1);
    assert_eq!(st.stale_count, 1);
    assert_eq!(st.log[0].unverified_findings, 1);
    assert!(
        st.last_setback
            .as_deref()
            .unwrap_or("")
            .contains("unverified")
    );
}

#[test]
fn file_evidence_must_exist_and_prefix_matching_is_case_insensitive() {
    let mut st = LoopState::default();
    assert_eq!(
        apply_reply(
            &mut st,
            "DIRECTION: fake source\nFINDINGS:\n- claim [evidence: file:no-such-evidence.txt:1]",
        ),
        0
    );
    assert!(st.findings.is_empty());

    let mut st = LoopState::default();
    assert_eq!(
        apply_reply(
            &mut st,
            "DIRECTION: real source\nFINDINGS:\n- claim [evidence: File:src/drive/loop_ctl.rs:1]",
        ),
        1
    );
    assert_eq!(st.findings.len(), 1);
}

#[test]
fn coordinator_fallback_is_never_admitted_as_a_finding() {
    let mut st = LoopState::default();
    let reply =
        "Turbo driver is not reachable: club returned an empty reply (no text and no tool calls)";
    assert_eq!(apply_reply(&mut st, reply), 0);
    assert!(st.findings.is_empty());
    assert!(st.hypotheses.is_empty());
    assert_eq!(st.stale_count, 1);
    assert!(
        st.last_error
            .as_deref()
            .unwrap_or("")
            .contains("not reachable")
    );
}

#[test]
fn error_heavy_tool_chain_cannot_reset_stall_with_fresh_prose() {
    let mut st = LoopState::default();
    let tools = ToolStripSnapshot {
        calls: 4,
        errors: 1,
        incomplete: 0,
        diagnostics: 0,
        costly_actions: Vec::new(),
        outcome_actions: Vec::new(),
        ..Default::default()
    };
    let reply = block("claim after errors", &["a source-backed fact"]);
    assert_eq!(apply_reply_with_tools(&mut st, &reply, &tools), 0);
    assert_eq!(st.findings.len(), 1, "fact remains in the evidence ledger");
    assert_eq!(st.stale_count, 1, "tool churn cannot manufacture progress");
    assert_eq!(st.log[0].tool_errors, 1);
}

#[test]
fn repeated_costly_action_is_persisted_for_the_next_iteration() {
    let mut st = LoopState::default();
    let tools = ToolStripSnapshot {
        calls: 1,
        errors: 0,
        incomplete: 0,
        diagnostics: 0,
        costly_actions: vec!["shell:popcorn submit --mode benchmark candidate.py".into()],
        outcome_actions: vec!["shell:popcorn submit --mode benchmark candidate.py".into()],
        ..Default::default()
    };
    assert_eq!(
        apply_reply_with_tools(&mut st, &block("first", &["m1"]), &tools),
        1
    );
    assert_eq!(
        apply_reply_with_tools(&mut st, &block("repeat", &["m2"]), &tools),
        1
    );
    assert_eq!(st.log[1].duplicate_costly_actions, 1);
    assert!(
        st.last_setback
            .as_deref()
            .unwrap_or("")
            .contains("repeated 1 costly")
    );
}

#[test]
fn podrace_receipts_record_execution_without_objective_progress() {
    let mut st = LoopState {
        podrace: true,
        stale_count: 2,
        ..Default::default()
    };
    let prose_only = ToolStripSnapshot::default();
    assert_eq!(
        apply_reply_with_tools(
            &mut st,
            &block("more reading", &["novel fact"]),
            &prose_only
        ),
        1
    );
    assert_eq!(
        st.stale_count, 3,
        "source-backed prose is not competition progress"
    );

    // An unverified outcome fingerprint (status poll / listing) is still
    // not a measured candidate.
    let polling = ToolStripSnapshot {
        calls: 1,
        outcome_actions: vec!["outcome:shell:git status:result=aa".into()],
        ..Default::default()
    };
    apply_reply_with_tools(&mut st, "DIRECTION: poll status", &polling);
    assert_eq!(st.stale_count, 4, "polling is not competition progress");

    let submitted = ToolStripSnapshot {
        calls: 1,
        verified_outcome_actions: vec![
            "submitted:shell:hilbert submit cand.py:result=deadbeef".into(),
        ],
        ..Default::default()
    };
    apply_reply_with_tools(&mut st, "DIRECTION: submit candidate", &submitted);
    assert_eq!(st.stale_count, 5);
    assert_eq!(st.submissions, 1);
    assert_eq!(
        st.measured_candidates, 0,
        "a submit is a submission, not a local measurement"
    );
    assert_eq!(st.log.last().unwrap().verified_outcome_actions, 1);

    // Replaying the identical receipt must not keep the run alive.
    apply_reply_with_tools(&mut st, "DIRECTION: submit again", &submitted);
    assert_eq!(st.stale_count, 6, "a repeated receipt is not new progress");
    assert_eq!(st.submissions, 1);
}

#[test]
fn podrace_stall_climbs_through_findings_workspace_change_and_status_polls() {
    use std::path::PathBuf;
    let mut st = LoopState {
        podrace: true,
        workspace: Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
        ..Default::default()
    };
    // Seed the remembered fingerprint so the next observation can be
    // desynchronized into a forced workspace change.
    apply_reply_with_tools(&mut st, &block("seed", &[]), &ToolStripSnapshot::default());
    let current = st.last_workspace_fingerprint;
    st.last_workspace_fingerprint = current.map(|f| f.wrapping_add(1));

    // 5 fresh findings + a workspace change + a git status poll.
    let mut tools = ToolStripSnapshot {
        calls: 1,
        ..Default::default()
    };
    tools.outcome_actions = vec!["outcome:shell:git status --short:result=bb".into()];
    let reply = block(
        "micro-optimization direction",
        &["f1", "f2", "f3", "f4", "f5"],
    );
    assert_eq!(apply_reply_with_tools(&mut st, &reply, &tools), 5);
    assert!(
        st.log.last().unwrap().workspace_changed,
        "fixture must force a change"
    );
    assert_eq!(
        st.stale_count, 2,
        "findings, workspace edits, and status polls are not measured candidates"
    );

    // One verified benchmark receipt records execution, not improvement.
    let bench = ToolStripSnapshot {
        calls: 1,
        verified_outcome_actions: vec![
            "measured:shell:./benchmark.sh --local-iterate:result=cafe".into(),
        ],
        ..Default::default()
    };
    apply_reply_with_tools(&mut st, "DIRECTION: measure locally", &bench);
    assert_eq!(st.stale_count, 3);
    assert_eq!(st.measured_candidates, 1);
}

#[test]
fn measured_candidate_row_and_state_round_trip() {
    let mut st = LoopState {
        podrace: true,
        ..Default::default()
    };
    let bench = ToolStripSnapshot {
        calls: 1,
        verified_outcome_actions: vec![
            "measured:shell:./benchmark.sh --local-iterate:result=cafe".into(),
        ],
        ..Default::default()
    };
    apply_reply_with_tools(&mut st, "DIRECTION: measure locally", &bench);
    assert_eq!(st.measured_candidates, 1);
    assert_eq!(st.measured_candidates_n, 1);
    assert_eq!(st.measured_candidates_log.len(), 1);
    assert!(
        st.measured_candidates_log[0]
            .verifier
            .command
            .contains("benchmark.sh")
    );
    let json = serde_json::to_string(&st).expect("ser");
    let back: LoopState = serde_json::from_str(&json).expect("de");
    assert_eq!(back.measured_candidates, 1);
    assert_eq!(back.measured_candidates_log.len(), 1);
    let mut bare = serde_json::to_value(LoopState::default()).expect("default");
    bare["measured_candidates"] = serde_json::json!(3);
    bare["tokens_spent"] = serde_json::json!(7);
    if let Some(obj) = bare.as_object_mut() {
        obj.remove("measured_candidates_log");
        obj.remove("measured_candidates_n");
        obj.remove("submissions_log");
        obj.remove("binary");
        obj.remove("tokens");
    }
    let loaded: LoopState = serde_json::from_value(bare).expect("old shape");
    assert_eq!(loaded.measured_candidates, 3);
    assert!(loaded.measured_candidates_log.is_empty());
    assert_eq!(loaded.tokens_spent, 7);
    assert_eq!(loaded.tokens.total, 0);
}

#[test]
fn verifier_blocked_set_by_failure_and_checkpoint_cleared_by_receipt() {
    let failed = ToolStripSnapshot {
        calls: 1,
        errors: 1,
        verifier_failures: vec![(
            "shell:./benchmark.sh".into(),
            "benchctl measure-job: missing required --golden".into(),
        )],
        ..Default::default()
    };
    let mut st = LoopState {
        podrace: true,
        ..Default::default()
    };
    apply_reply_with_tools(&mut st, "DIRECTION: preflight", &failed);
    assert_eq!(
        st.verifier_blocked.as_deref(),
        Some("benchctl measure-job: missing required --golden")
    );

    // Prose novelty cannot clear a blocked verification path.
    apply_reply_with_tools(
        &mut st,
        &block("explore a new kernel direction", &["novel fact"]),
        &ToolStripSnapshot::default(),
    );
    assert_eq!(
        st.verifier_blocked.as_deref(),
        Some("benchctl measure-job: missing required --golden")
    );

    // A `VERIFY … blocked` / `DECISION … blocked` checkpoint refreshes the
    // diagnostic from the reply itself.
    apply_reply_with_tools(
        &mut st,
        "DIRECTION: report\nDECISION blocked: organizer must reconcile",
        &ToolStripSnapshot::default(),
    );
    assert_eq!(
        st.verifier_blocked.as_deref(),
        Some("blocked: organizer must reconcile")
    );
    apply_reply_with_tools(
        &mut st,
        "VERIFY blocked · benchctl requires --golden\nDIRECTION: x",
        &ToolStripSnapshot::default(),
    );
    assert_eq!(
        st.verifier_blocked.as_deref(),
        Some("blocked · benchctl requires --golden")
    );

    // A verified receipt clears it.
    let ok = ToolStripSnapshot {
        calls: 1,
        verified_outcome_actions: vec![
            "measured:shell:./benchmark.sh --local-iterate:result=11".into(),
        ],
        ..Default::default()
    };
    apply_reply_with_tools(&mut st, "DIRECTION: local benchmark", &ok);
    assert!(st.verifier_blocked.is_none());
    assert_eq!(st.measured_candidates, 1);

    // Ordinary loops keep today's behaviour: no blocker arming.
    let mut ordinary = LoopState::default();
    apply_reply_with_tools(&mut ordinary, "DIRECTION: preflight", &failed);
    assert!(ordinary.verifier_blocked.is_none());
}

#[test]
fn ordinary_loop_credits_novel_tool_outcomes_but_repeated_polling_stays_stale() {
    let mut st = LoopState {
        stale_count: 3,
        stall_stop: 4,
        ..Default::default()
    };
    let outcome = ToolStripSnapshot {
        calls: 69,
        errors: 5,
        outcome_actions: vec!["outcome:status:submission 829".into()],
        ..Default::default()
    };

    apply_reply_with_tools(&mut st, "DIRECTION: inspect terminal score", &outcome);
    assert_eq!(st.stale_count, 0, "a new successful outcome is progress");
    assert_eq!(st.log[0].outcome_progress, 1);
    assert_eq!(st.log[0].novel_outcome_actions, 1);

    apply_reply_with_tools(&mut st, "DIRECTION: inspect terminal score", &outcome);
    assert_eq!(
        st.stale_count, 1,
        "replaying the identical status receipt must not mask a stall"
    );
    assert_eq!(st.log[1].outcome_progress, 1);
    assert_eq!(st.log[1].novel_outcome_actions, 0);
}

#[test]
fn budget_trips_on_iters_and_tokens() {
    let mut st = LoopState {
        max_iters: 2,
        ..Default::default()
    };
    st.iteration = 1;
    assert!(budget_tripped(&st).is_none());
    st.iteration = 2;
    assert!(budget_tripped(&st).is_some(), "max iters trips");

    let st2 = LoopState {
        token_budget: 100,
        tokens_spent: 100,
        ..Default::default()
    };
    assert!(budget_tripped(&st2).is_some(), "token budget trips");

    // Deadline in the past trips; far future doesn't.
    let past = LoopState {
        deadline_secs: 1,
        started_ms: 0,
        ..Default::default()
    };
    assert!(
        budget_tripped(&past).is_some(),
        "elapsed past deadline trips"
    );
    let fresh = LoopState {
        deadline_secs: 100_000,
        started_ms: now_ms(),
        ..Default::default()
    };
    assert!(budget_tripped(&fresh).is_none());
}

#[test]
fn default_loop_has_no_iteration_cap() {
    assert_eq!(
        DEFAULT_LOOP_MAX_ITERS, 0,
        "default loop should not have a hard 25-iteration cap"
    );
}

#[test]
fn yolo_preserves_explicit_loop_runaway_budgets() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _iters = crate::tests::TestEnvGuard::set("ANGEL_LOOP_MAX_ITERS", "7");
    let _deadline = crate::tests::TestEnvGuard::set("ANGEL_LOOP_DEADLINE_SECS", "123");
    let _tokens = crate::tests::TestEnvGuard::set("ANGEL_LOOP_TOKEN_BUDGET", "456");
    let _stall = crate::tests::TestEnvGuard::set("ANGEL_LOOP_STALL_STOP", "3");
    let _first = crate::tests::TestEnvGuard::set("ANGEL_LOOP_FIRST_CANDIDATE_ITERS", "9");

    let mut configured = LoopState::configured_from_env();
    assert_eq!(configured.max_iters, 7);
    assert_eq!(configured.deadline_secs, 123);
    assert_eq!(configured.token_budget, 456);
    assert_eq!(configured.stall_stop, 3);
    assert_eq!(configured.first_candidate_iters, 9);

    configured.iteration = 7;
    assert_eq!(
        budget_tripped(&configured).as_deref(),
        Some("max iterations (7)")
    );
    configured.iteration = 0;
    configured.tokens_spent = 456;
    assert_eq!(
        budget_tripped(&configured).as_deref(),
        Some("token budget (~456)")
    );
    configured.tokens_spent = 0;
    configured.started_ms = 0;
    assert_eq!(
        budget_tripped(&configured).as_deref(),
        Some("deadline (123s)")
    );
    configured.stale_count = 3;
    assert!(stall_limit_reached(&configured));
}

#[test]
fn says_done_is_tolerant_but_anchored() {
    assert!(says_done("work done\nLOOP_DONE"));
    assert!(says_done("**LOOP_DONE**"));
    assert!(says_done("- LOOP_DONE."));
    assert!(says_done("loopdone")); // case/underscore tolerant
    // Must be its own line, not buried in prose.
    assert!(!says_done("I will write LOOP_DONE when finished"));
    assert!(!says_done("not finished yet"));
}
