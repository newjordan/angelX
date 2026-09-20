use super::*;

#[test]
fn worker_panic_returns_an_actionable_error_instead_of_disconnect() {
    let error = run_worker_guarded(|| -> Result<(), String> {
        panic!("synthetic provider panic");
    })
    .expect_err("panic must become a turn error");

    assert!(error.contains("worker panicked"), "{error}");
    assert!(error.contains("retry"), "{error}");
}

#[test]
fn turn_idle_timeout_defaults_disables_and_falls_back() {
    let _env = crate::tests::env_lock();

    {
        let _unset = crate::tests::TestEnvGuard::unset("ANGEL_TURN_IDLE_TIMEOUT_SECS");
        assert_eq!(configured_turn_idle_timeout_secs(), None);
    }
    {
        let _disabled = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "0");
        assert_eq!(configured_turn_idle_timeout_secs(), None);
    }
    {
        let _invalid =
            crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "not-seconds");
        assert_eq!(configured_turn_idle_timeout_secs(), None);
    }
}

#[test]
fn steer_idle_interrupt_defaults_disables_and_falls_back() {
    let _env = crate::tests::env_lock();

    {
        let _unset = crate::tests::TestEnvGuard::unset("ANGEL_STEER_IDLE_INTERRUPT_SECS");
        assert_eq!(configured_steer_idle_interrupt_secs(), 60);
    }
    {
        let _disabled = crate::tests::TestEnvGuard::set("ANGEL_STEER_IDLE_INTERRUPT_SECS", "0");
        assert_eq!(configured_steer_idle_interrupt_secs(), 0);
    }
    {
        let _invalid =
            crate::tests::TestEnvGuard::set("ANGEL_STEER_IDLE_INTERRUPT_SECS", "not-seconds");
        assert_eq!(configured_steer_idle_interrupt_secs(), 60);
    }
}

#[test]
fn steer_interrupt_due_covers_off_none_queued_under_and_over() {
    // Knob off: never, no matter how idle or queued.
    assert!(!steer_interrupt_due(10_000, 3, 0));
    // Nothing queued: never — there is nothing to deliver.
    assert!(!steer_interrupt_due(10_000, 0, 60));
    // Under the threshold: the stream may still be warming up.
    assert!(!steer_interrupt_due(59, 1, 60));
    // At and over the threshold with steers waiting: fire.
    assert!(steer_interrupt_due(60, 1, 60));
    assert!(steer_interrupt_due(600, 3, 60));
}

#[test]
fn stream_progress_rearms_the_steer_interrupt_flag() {
    let mut thinking = Thinking::pending_for_test("practice");
    thinking.steer_interrupt_fired = true;
    thinking.note_stream_progress();
    assert!(
        !thinking.steer_interrupt_fired,
        "a fresh stall earns one new interrupt"
    );
    thinking.steer_interrupt_fired = true;
    assert!(thinking.steer_interrupt_fired, "otherwise it stays spent");
}

#[test]
fn turn_idle_timeout_is_stable_in_flight_and_refreshes_next_turn() {
    let _env = crate::tests::env_lock();
    let _first_value = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17");
    let first = Thinking::pending_for_test("first");

    {
        let _next_value = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "23");
        let second = Thinking::pending_for_test("second");
        assert_eq!(first.idle_timeout_secs, Some(17));
        assert_eq!(second.idle_timeout_secs, Some(23));
    }

    assert_eq!(
        first.idle_timeout_secs,
        Some(17),
        "later environment changes cannot rewrite an in-flight watchdog"
    );
}

fn aged(secs: u64) -> Instant {
    Instant::now()
        .checked_sub(std::time::Duration::from_secs(secs))
        .expect("monotonic clock supports backdating in tests")
}

#[test]
fn idle_warnings_fire_exactly_once_per_stage_with_aged_stream() {
    let mut thinking = Thinking::pending_for_test("practice");
    thinking.idle_timeout_secs = Some(600);
    // Aged past 50% (300s) but not 80% (480s).
    thinking.last_stream_at = aged(301);
    let warning = thinking.idle_warning_due().expect("50% stage due");
    assert_eq!(warning.stage_pct, 50);
    assert!(warning.idle_secs >= 300);
    assert_eq!(warning.idle_timeout_secs, 600);
    assert!(
        thinking.idle_warning_due().is_none(),
        "50% is one-shot across frames"
    );
    // The stall deepens past 80%.
    thinking.last_stream_at = aged(481);
    let warning = thinking.idle_warning_due().expect("80% stage due");
    assert_eq!(warning.stage_pct, 80);
    assert!(
        thinking.idle_warning_due().is_none(),
        "80% is one-shot across frames"
    );
}

#[test]
fn idle_warning_80_subsumes_50_and_progress_rearms_both() {
    let mut thinking = Thinking::pending_for_test("practice");
    thinking.idle_timeout_secs = Some(100);
    // First observation is already past 80%: exactly one notice.
    thinking.last_stream_at = aged(90);
    assert_eq!(thinking.idle_warning_due().expect("80% due").stage_pct, 80);
    assert!(
        thinking.idle_warning_due().is_none(),
        "the skipped 50% stage must not fire late"
    );
    // Stream progress re-arms; a fresh stall escalates from 50% again.
    thinking.note_stream_progress();
    assert!(
        thinking.idle_warning_due().is_none(),
        "fresh stream activity means no warning"
    );
    thinking.last_stream_at = aged(50);
    assert_eq!(thinking.idle_warning_due().expect("re-armed").stage_pct, 50);
}

#[test]
fn idle_warnings_respect_disabled_and_tiny_timeouts() {
    let mut thinking = Thinking::pending_for_test("practice");
    thinking.idle_timeout_secs = None;
    thinking.last_stream_at = aged(10_000);
    assert!(
        thinking.idle_warning_due().is_none(),
        "disabled watchdog never warns"
    );
    // Thresholds that truncate to zero must never warn at idle 0.
    thinking.idle_timeout_secs = Some(1);
    thinking.last_stream_at = Instant::now();
    assert!(thinking.idle_warning_due().is_none());
}
