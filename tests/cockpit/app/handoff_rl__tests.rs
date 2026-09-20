use super::*;

#[test]
fn endless_command_clears_caps_and_rearms_cap_stopped_campaign() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.handoff_rl.start(None);
    app.handoff_rl.max_iters = 2;
    app.handoff_rl.handoff_count = 2;
    app.handoff_rl.deadline_secs = 900;
    app.handoff_rl.token_budget = 123;
    let reason = app.handoff_rl.budget_tripped().unwrap();
    // L01 names the operator's own command in the notice: "max rolls (/handoff-rl max 2)".
    assert!(
        reason.starts_with("max rolls (") && reason.contains('2'),
        "{reason}"
    );
    app.handoff_rl.stop_for_budget(&reason);
    let reply = app.handoff_rl_command(Some("endless"));
    assert!(reply.contains("resumed"), "{reply}");
    assert!(app.handoff_rl.active);
    assert_eq!(
        (
            app.handoff_rl.max_iters,
            app.handoff_rl.deadline_secs,
            app.handoff_rl.token_budget
        ),
        (0, 0, 0)
    );
    assert!(app.handoff_rl.budget_tripped().is_none());
}

#[test]
fn default_injection_has_no_budget_language() {
    let mut st = HandoffRlState::default();
    st.start(None);
    st.tokens_spent = usize::MAX;
    st.handoff_count = 100_000;
    assert!(st.budget_tripped().is_none());
    let prompt = st.build_forced_injection(None);
    assert!(prompt.contains("iteration 100001 · no cap"));
    assert!(!prompt.to_lowercase().contains("budget"));
    assert!(!prompt.to_lowercase().contains("deadline"));
}

#[test]
fn l01_handoff_defaults_pass_old_limits_and_explicit_cap_names_value() {
    let mut state = HandoffRlState::default();
    state.apply_launch_settings(
        &crate::loop_dialog::LoopLaunchDialog::handoff_rl("fixture", 0).settings(),
    );
    state.handoff_count = 151;
    state.tokens_spent = 2_000_001;
    state.started_ms = now_ms().saturating_sub(3_601_000);
    assert_eq!(state.budget_tripped(), None);
    state.deadline_secs = 3600;
    assert!(
        state
            .budget_tripped()
            .unwrap()
            .contains("/handoff-rl duration 3600s")
    );
}

#[test]
fn handoff_rl_initial_state() {
    let st = HandoffRlState::new();
    assert!(!st.active);
    assert_eq!(st.handoff_count, 0);
    assert!(!st.pending_submit);
    assert!(st.winning_baseline.is_none());
    assert!(st.winning_score.is_none());
    assert_eq!(st.max_iters, 0);
    assert_eq!(st.token_budget, 0);
    assert_eq!(st.deadline_secs, 0);
}

#[test]
fn handoff_rl_start_stop() {
    let mut st = HandoffRlState::new();
    let res = st.start(Some("optimize search query throughput"));
    assert!(st.active);
    assert!(res.contains("hit it chewy"));
    assert_eq!(
        st.current_hypothesis.as_deref(),
        Some("optimize search query throughput")
    );

    let stop_res = st.stop();
    assert!(!st.active);
    assert!(!st.pending_submit);
    assert_eq!(stop_res, "handoff RL stopped");
}

#[test]
fn apply_launch_settings_matches_loop_workshop() {
    let mut st = HandoffRlState::new();
    st.apply_launch_settings(&LoopLaunchSettings {
        deadline_secs: 3600,
        max_iters: 25,
        token_budget: 2_000_000,
        podrace: false,
    });
    assert_eq!(st.deadline_secs, 3600);
    assert_eq!(st.max_iters, 25);
    assert_eq!(st.token_budget, 2_000_000);
    assert!(!st.podrace);

    st.apply_launch_settings(&LoopLaunchSettings {
        deadline_secs: 5 * 24 * 3600,
        max_iters: 0,
        token_budget: 0,
        podrace: true,
    });
    assert!(st.podrace);
    assert_eq!(st.max_iters, 0);
    assert_eq!(st.token_budget, 0);
}

#[test]
fn budget_tripped_on_max_rolls() {
    let mut st = HandoffRlState::new();
    st.start(None);
    st.max_iters = 2;
    st.build_forced_injection(None);
    assert!(st.budget_tripped().is_none());
    st.build_forced_injection(None);
    assert!(st.budget_tripped().unwrap().contains("max rolls"));
    assert!(st.can_force_roll().is_err());
}

#[test]
fn handoff_rl_record_victories() {
    let mut st = HandoffRlState::new();
    st.start(None);

    let msg1 = st.record_victory("cand-1", 85.5, "inline hot path cache");
    assert!(msg1.contains("NEW WINNING BASELINE"));
    assert!(msg1.contains("handoff demanded"));
    assert_eq!(st.winning_baseline.as_deref(), Some("cand-1"));
    assert_eq!(st.winning_score, Some(85.5));
    assert!(st.pending_submit);
    assert!(st.victory_demands_handoff());

    let msg2 = st.record_victory("cand-2", 82.0, "slower variant");
    assert!(!msg2.contains("NEW WINNING BASELINE"));
    assert_eq!(st.winning_baseline.as_deref(), Some("cand-1"));

    let msg3 = st.record_victory("cand-3", 92.3, "vectorized loop");
    assert!(msg3.contains("NEW WINNING BASELINE"));
    assert_eq!(st.winning_baseline.as_deref(), Some("cand-3"));
    assert_eq!(st.winning_score, Some(92.3));
    assert_eq!(st.victory_board.len(), 3);
}

#[test]
fn observe_turn_requires_submit_then_result() {
    let mut st = HandoffRlState::new();
    st.start(None);

    let d1 = st.observe_turn(
        &["shell:popcorn submit --mode benchmark".into()],
        "submitted cand",
    );
    assert!(d1.is_none());
    assert!(st.pending_submit);

    let d2 = st.observe_turn(&[], "SCORE: 12.3 looking good");
    assert!(d2.is_none());
    assert!(st.pending_submit);

    let d3 = st.observe_turn(
        &["outcome:shell:hilbert submissions --all".into()],
        "polled board",
    );
    assert!(d3.is_some());
    let demand = d3.unwrap();
    assert!(demand.summary.contains("submit:"));
    assert!(demand.summary.contains("result:"));
}

#[test]
fn observe_turn_same_turn_submit_and_result() {
    let mut st = HandoffRlState::new();
    st.start(None);
    let d = st.observe_turn(
        &[
            "shell:hilbert submit".into(),
            "outcome:shell:hilbert status abc123".into(),
        ],
        "done",
    );
    assert!(d.is_some());
}

#[test]
fn observe_turn_result_without_prior_submit_is_silent() {
    let mut st = HandoffRlState::new();
    st.start(None);
    let d = st.observe_turn(
        &["outcome:shell:hilbert submissions --all".into()],
        "just checking board",
    );
    assert!(d.is_none());
    assert!(!st.pending_submit);
}

#[test]
fn forced_injection_starts_with_hit_it_chewy_and_wipes_pending() {
    let mut st = HandoffRlState::new();
    st.start(Some("hypothesis A"));
    st.record_victory("cand-win", 99.1, "hypothesis A");
    assert!(st.pending_submit);

    let note = st.build_forced_injection(Some("promoted cand-win after 100 tests passed"));
    assert!(note.starts_with("hit it chewy"));
    assert!(note.contains("[FORCED HANDOFF — CONTEXT WIPED BY COCKPIT · roll #1]"));
    assert!(note.contains("cand-win"));
    assert!(note.contains("99.1000"));
    assert!(note.contains(HANDOFF_RL_SEQUENCE_DIRECTIVE));
    assert!(note.contains("prompt injection procedure"));
    assert_eq!(st.handoff_count, 1);
    assert!(!st.pending_submit);
    assert!(!st.victory_demands_handoff());
    assert!(st.tokens_spent > 0);
}

#[test]
fn classify_submit_vs_result_actions() {
    assert!(is_submit_action("shell:popcorn submit --mode benchmark"));
    assert!(is_submit_action("shell:hilbert submit"));
    assert!(!is_result_action("shell:popcorn submit --mode benchmark"));
    assert!(is_result_action("outcome:shell:hilbert submissions --all"));
    assert!(is_result_action("outcome:shell:popcorn status"));
    assert!(is_result_action("shell:leaderboard check"));
    assert!(!is_submit_action("outcome:shell:hilbert status"));
}

/// The operator loop: force inject → (work) → submit → result → force inject…
#[test]
fn handoff_cycle_submit_result_then_force_ad_infinitum() {
    let mut st = HandoffRlState::new();
    st.start(Some("compete on board"));
    st.max_iters = 0; // endless
    st.deadline_secs = 0;
    st.token_budget = 0;

    for roll in 1..=8 {
        // First inject (or re-inject after result).
        let note = st.build_forced_injection(Some(&format!("roll-{roll} start")));
        assert!(
            note.starts_with("hit it chewy"),
            "roll {roll}: injection must start with hit it chewy"
        );
        assert!(note.contains("[FORCED HANDOFF"));
        assert_eq!(st.handoff_count, roll);
        assert!(!st.pending_submit);

        // Work happens; submit alone does not demand.
        assert!(
            st.observe_turn(&["shell:hilbert submit cand".into()], "submitted")
                .is_none()
        );
        assert!(st.pending_submit);

        // Result lands → demand.
        let demand = st
            .observe_turn(
                &["outcome:shell:hilbert status abc".into()],
                "terminal score 1.0",
            )
            .expect("result after submit must demand handoff");
        assert!(demand.summary.contains("submit:"));
        assert!(demand.summary.contains("result:"));
        // Pending stays latched until the next build_forced_injection.
        assert!(st.pending_submit);
    }
    assert_eq!(st.handoff_count, 8);
    assert!(st.can_force_roll().is_ok());
}
