use super::*;
use crate::drive::loop_ctl;
use angel_realm_host::together_settlement::planning::{Outcome, ResearchPassage};
use angel_plug::feed::{LoopPhase, LoopTier};

fn log_row(
    iteration: usize,
    tool_calls: usize,
    tool_errors: usize,
    workspace_changed: bool,
) -> loop_ctl::LoopIterLog {
    loop_ctl::LoopIterLog {
        iteration,
        direction: String::new(),
        new_findings: 0,
        reported_findings: 0,
        unverified_findings: 0,
        tool_calls,
        tool_errors,
        duplicate_costly_actions: 0,
        outcome_progress: 0,
        novel_outcome_actions: 0,
        verified_outcome_actions: 0,
        workspace_changed,
        evidence_review: false,
        stale_count: 0,
        ts_ms: 0,
    }
}

fn plan(door: &str) -> Input {
    Input {
        map_sha256: "a".repeat(64),
        task_id: "q.angel-task".into(),
        door_id: door.into(),
        route: vec![door.into()],
        passage: ResearchPassage::Door,
        outcome: Outcome::Unreviewed,
    }
}

fn settled(
    id: &str,
    status: &str,
    promotion: Option<&str>,
    improved: Option<bool>,
    score: &str,
) -> crate::agent::harness::cartridges::records::TerminalEvidence {
    serde_json::from_value(serde_json::json!({
        "official": {
            "id": id,
            "benchmark_id": "kernel",
            "status": status,
            "promotion_status": promotion,
            "promoted_source_ref": promotion.filter(|p| *p == "promoted").map(|_| "refs/heads/c"),
            "official_score": score,
            "improved": improved,
            "fetched_at_ms": 1
        },
        "benchmark_id": "kernel",
        "submission_id": id,
        "status": status,
        "frontier_win_verified": false,
        "source_id": "test"
    }))
    .unwrap()
}

#[test]
fn summary_copies_each_field() {
    let workspace = std::env::temp_dir().join("loop-summary-copies");
    let mut st = LoopState {
        id: "loop-1".into(),
        workspace: Some(workspace.clone()),
        task: "measure the kernel".into(),
        status: loop_ctl::LoopStatus::AwaitingApproval,
        tier: loop_ctl::EscalationTier::Sota,
        iteration: 7,
        submissions: 3,
        podrace: true,
        pivot: 9,
        measured_candidates: 5,
        verifier_blocked: Some("benchmark missing --golden".into()),
        stale_count: 4,
        ..LoopState::default()
    };
    st.log = vec![log_row(1, 1, 0, true), log_row(2, 1, 0, true)];
    st.settlement_plans.insert(2, plan("q.door"));
    st.confirm_settlement_checkpoint_for_test();

    let s = st.summary();
    assert_eq!(s.status, LoopStatus::AwaitingApproval);
    assert_eq!(s.iteration, 7);
    assert_eq!(s.tier, EscalationTier::Sota);
    assert_eq!(s.submissions, 3);
    assert!(s.podrace);
    assert_eq!(s.pivot, 9);
    assert_eq!(s.measured_candidates, 5);
    assert_eq!(
        s.verifier_blocked.as_deref(),
        Some("benchmark missing --golden")
    );
    assert_eq!(s.task, "measure the kernel");
    assert_eq!(s.stale_count, 4);
    assert_eq!(
        s.durable_settlement_events,
        Some(DurableEvents {
            loop_id: "loop-1".into(),
            workspace,
            rows: vec![(1, true), (2, true)],
            plans: [(2, plan("q.door"))].into_iter().collect(),
        })
    );
    // Active rows: nothing idle, nothing failed, no settled submissions.
    assert!(!s.last_log_all_errors);
    assert_eq!(s.idle_log_tail, 0);
    assert_eq!(
        (s.promotions, s.promotion_failures),
        (0, 0),
        "no settled submissions"
    );
    assert_eq!(s.last_promotion_score, None);
    assert_eq!(s.last_promotion_failure_score, None);
}

#[test]
fn summary_of_a_fresh_state_is_the_default_summary() {
    assert_eq!(LoopState::default().summary(), LoopSummary::default());
    let unconfirmed = LoopState {
        id: "loop-2".into(),
        workspace: Some(std::env::temp_dir()),
        ..LoopState::default()
    };
    assert_eq!(
        unconfirmed.summary().durable_settlement_events,
        None,
        "a checkpoint nobody confirmed is not durable"
    );
}

#[test]
fn summary_mirrors_every_status_and_tier() {
    use loop_ctl::{EscalationTier as Tier, LoopStatus as Status};
    let statuses = [
        (Status::Idle, LoopStatus::Idle),
        (Status::Baselining, LoopStatus::Baselining),
        (Status::Running, LoopStatus::Running),
        (Status::Verifying, LoopStatus::Verifying),
        (Status::AwaitingApproval, LoopStatus::AwaitingApproval),
        (Status::Paused, LoopStatus::Paused),
        (Status::Done, LoopStatus::Done),
        (Status::Stopped, LoopStatus::Stopped),
        (Status::Failed, LoopStatus::Failed),
    ];
    for (state, mirror) in statuses {
        assert_eq!(format!("{state:?}"), format!("{mirror:?}"));
        assert_eq!(LoopStatus::from(LoopPhase::from(state)), mirror);
    }
    for (state, mirror) in [
        (Tier::Local, EscalationTier::Local),
        (Tier::Swarm, EscalationTier::Swarm),
        (Tier::Sota, EscalationTier::Sota),
    ] {
        assert_eq!(format!("{state:?}"), format!("{mirror:?}"));
        assert_eq!(EscalationTier::from(LoopTier::from(state)), mirror);
    }
    assert_eq!(
        LoopStatus::default(),
        LoopStatus::from(LoopPhase::from(Status::default()))
    );
    assert_eq!(
        EscalationTier::default(),
        EscalationTier::from(LoopTier::from(Tier::default()))
    );
}

#[test]
fn summary_derives_the_inactivity_tail_from_the_log() {
    let mut st = LoopState::default();
    // Newest first: two rows of pure failure after a row that did something.
    st.log = vec![
        log_row(1, 3, 0, true),
        log_row(2, 2, 2, false),
        log_row(3, 1, 1, false),
    ];
    let s = st.summary();
    assert_eq!(s.idle_log_tail, 2);
    assert!(s.last_log_all_errors);
    // A workspace change, finding or outcome ends the tail.
    st.log[2].workspace_changed = true;
    let s = st.summary();
    assert_eq!(s.idle_log_tail, 0);
    assert!(
        s.last_log_all_errors,
        "the newest row still failed every call"
    );
    st.log[2] = log_row(3, 2, 1, false);
    let s = st.summary();
    assert_eq!(s.idle_log_tail, 0, "a successful call is activity");
    assert!(!s.last_log_all_errors, "one call succeeded");
    st.log[2].verified_outcome_actions = 1;
    st.log[2].tool_errors = 2;
    assert_eq!(st.summary().idle_log_tail, 0, "a receipt is activity");
    assert!(st.summary().last_log_all_errors);
}

#[test]
fn summary_counts_promotions_and_failures_with_their_newest_scores() {
    let mut st = LoopState::default();
    st.submission_results = vec![
        settled("waiting", "pending", None, None, "1.0s"),
        settled("deciding", "accepted", Some("unavailable"), None, "2.0s"),
        settled("first", "accepted", Some("promoted"), Some(true), "3.0s"),
        settled("no", "rejected", None, None, "4.0s"),
        settled("held", "accepted", Some("held"), Some(false), "5.0s"),
        settled("second", "accepted", Some("promoted"), Some(true), "6.0s"),
    ];
    let s = st.summary();
    assert_eq!(s.promotions, 2);
    assert_eq!(s.last_promotion_score.as_deref(), Some("6.0s"));
    assert_eq!(s.promotion_failures, 2);
    assert_eq!(s.last_promotion_failure_score.as_deref(), Some("5.0s"));
}
