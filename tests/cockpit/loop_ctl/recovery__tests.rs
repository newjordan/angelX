use super::*;

#[test]
fn healthy_fast_lane_still_admits_a_deep_experiment() {
    let mut state = LoopState {
        podrace: true,
        iteration: 3,
        measured_candidates: 2,
        submissions: 1,
        stale_count: 0,
        stall_stop: 4,
        ..Default::default()
    };
    assert!(experiment_due(&state));
    state.podrace = false;
    assert!(!experiment_due(&state));
    state.stale_count = 4;
    assert!(experiment_due(&state));
}

#[test]
fn experiment_reservation_preserves_parent_allowance_and_deadline() {
    let mut state = LoopState {
        token_budget: 40_000,
        tokens_spent: 8_000,
        started_ms: now_ms(),
        deadline_secs: 90,
        ..Default::default()
    };
    assert_eq!(allowance(&state), Some((8_000, 90)));
    state.tokens_spent = 39_000;
    assert_eq!(allowance(&state), None);
    state.token_budget = 0;
    state.deadline_secs = 0;
    assert_eq!(
        allowance(&state),
        Some((EXPERIMENT_TOKENS, EXPERIMENT_SECONDS))
    );
    state.max_iters = 3;
    state.iteration = 3;
    assert_eq!(allowance(&state), None);
}

#[test]
fn dropping_owner_requests_cancellation() {
    let cancel = Arc::new(AtomicBool::new(false));
    let (_tx, rx) = std::sync::mpsc::channel();
    let owner = ExperimentPending {
        run_id: "test".into(),
        workspace: PathBuf::new(),
        key: "test".into(),
        cancel: Arc::clone(&cancel),
        rx,
    };
    drop(owner);
    assert!(cancel.load(Ordering::Acquire));
}
