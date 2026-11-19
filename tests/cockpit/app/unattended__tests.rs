use super::*;

fn paused(setback: &str) -> Seen<'_> {
    Seen {
        status: LoopStatus::Paused,
        setback: Some(setback),
        error: Some("inner turn stopped"),
        blocked: false,
        at_cap: false,
    }
}

fn stopped_for(reason: TurnStopReason) -> String {
    // The exact text `loop_harvest_stopped` records.
    format!("{} reason={}", TURN_STOPPED, reason.as_str())
}

fn code(step: Step) -> i32 {
    match step {
        Step::End(ending) => ending.code,
        other => panic!("expected an ending, got {other:?}"),
    }
}

#[test]
fn a_loop_at_work_is_left_alone() {
    for status in [
        LoopStatus::Baselining,
        LoopStatus::Running,
        LoopStatus::Verifying,
        LoopStatus::AwaitingApproval,
    ] {
        let seen = Seen { status, setback: None, error: None, blocked: false, at_cap: false };
        assert_eq!(next_step(&seen, 0), Step::Wait, "{status:?}");
    }
}

#[test]
fn a_turn_that_ran_out_its_time_hops_or_provider_is_resumed() {
    for reason in [
        TurnStopReason::IdleTimeout,
        TurnStopReason::Deadline,
        TurnStopReason::MaxHops,
        TurnStopReason::ProviderError,
    ] {
        let setback = stopped_for(reason);
        assert_eq!(next_step(&paused(&setback), 0), Step::Resume, "{}", reason.as_str());
    }
}

#[test]
fn a_stop_that_trying_again_does_not_repair_ends_the_run() {
    for reason in [
        TurnStopReason::Interrupt,
        TurnStopReason::ExecutionBlocked,
        TurnStopReason::CaptureFailure,
        TurnStopReason::CheckpointFailure,
        TurnStopReason::Declined,
        TurnStopReason::NeedsPro,
    ] {
        let setback = stopped_for(reason);
        assert_eq!(code(next_step(&paused(&setback), 0)), NEEDS_A_PERSON, "{}", reason.as_str());
    }
    // A pause the loop did not record a stopped turn for (a person's `/loop pause`,
    // a verifier that keeps failing the same way) is not resumed either.
    assert_eq!(code(next_step(&paused("verifier blocked"), 0)), NEEDS_A_PERSON);
    let unexplained = Seen {
        status: LoopStatus::Paused,
        setback: None,
        error: None,
        blocked: false,
        at_cap: false,
    };
    assert_eq!(code(next_step(&unexplained, 0)), NEEDS_A_PERSON);
}

#[test]
fn a_cap_or_a_blocked_host_ends_the_run_even_after_a_stopped_turn() {
    let setback = stopped_for(TurnStopReason::Deadline);
    let at_cap = Seen { at_cap: true, ..paused(&setback) };
    assert_eq!(code(next_step(&at_cap, 0)), ENDED);
    for text in ["budget reached: 40 iterations", "operator cap reached: 2h"] {
        assert_eq!(code(next_step(&paused(text), 0)), ENDED, "{text}");
    }
    let blocked = Seen { blocked: true, ..paused(&setback) };
    assert_eq!(code(next_step(&blocked, 0)), NEEDS_A_PERSON);
}

#[test]
fn pauses_straight_after_a_resume_are_bounded() {
    let setback = stopped_for(TurnStopReason::ProviderError);
    assert_eq!(next_step(&paused(&setback), QUICK_PAUSE_LIMIT - 1), Step::Resume);
    let Step::End(ending) = next_step(&paused(&setback), QUICK_PAUSE_LIMIT) else {
        panic!("the limit must end the run");
    };
    assert_eq!(ending.code, KEEPS_PAUSING);
    assert!(ending.why.contains("provider_error"), "{}", ending.why);
    assert!(ending.why.contains("inner turn stopped"), "the loop's own words are kept: {}", ending.why);
}

#[test]
fn a_finished_loop_ends_the_run_with_its_outcome() {
    let seen = |status| Seen { status, setback: None, error: Some("why"), blocked: false, at_cap: false };
    assert_eq!(code(next_step(&seen(LoopStatus::Done), 0)), ENDED);
    assert_eq!(code(next_step(&seen(LoopStatus::Stopped), 0)), ENDED);
    assert_eq!(code(next_step(&seen(LoopStatus::Failed), 0)), FAILED);
    assert_eq!(code(next_step(&seen(LoopStatus::Idle), 0)), FAILED);
}

#[test]
fn a_turn_that_worked_is_resumed_at_once_and_one_that_did_not_waits_longer_each_time() {
    let secs: Vec<u64> = (0..=6).map(|n| resume_delay(n).as_secs()).collect();
    assert_eq!(secs, [0, 10, 20, 40, 80, 160, 160]);
}

#[test]
fn only_a_launch_that_names_a_goal_or_a_loop_drives_one() {
    assert!(LaunchLoop::from_launch(&InteractiveLaunch::default()).is_none());
    let launch = InteractiveLaunch {
        loop_start: Some("podrace now read BRIEF.md".to_string()),
        unattended: true,
        ..InteractiveLaunch::default()
    };
    let driven = LaunchLoop::from_launch(&launch).expect("a loop was named");
    assert!(driven.unattended && !driven.started && !driven.ended);
}

#[test]
fn gateway_stop_directive_ends_unattended_run_immediately() {
    let setback = stopped_for(TurnStopReason::ProviderError);
    let stop_error = r#"HTTP 402: {"error":{"code":"model_budget_exhausted","run_action":"stop","retryable":false}}"#;
    let seen = Seen {
        status: LoopStatus::Paused,
        setback: Some(&setback),
        error: Some(stop_error),
        blocked: false,
        at_cap: false,
    };
    let Step::End(ending) = next_step(&seen, 0) else {
        panic!("gateway run_action=stop must end unattended run immediately");
    };
    assert_eq!(ending.code, ENDED);
    assert!(ending.why.contains("gateway stopped"), "{}", ending.why);
}
