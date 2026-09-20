use super::*;

#[test]
fn failure_backoff_arms_and_escalates() {
    let _guard = crate::tests::env_lock();
    let mut state = BgCompactState::default();
    let now = Instant::now();
    assert!(!state.cooling_down(now), "fresh state has no cooldown");
    let first = state.note_failure(now);
    assert_eq!(first, Duration::from_secs(120), "default base cooldown");
    assert!(state.cooling_down(now), "failure arms the cooldown");
    assert!(
        !state.cooling_down(now + first),
        "cooldown expires exactly at the boundary"
    );
    state.note_failure(now);
    let third = state.note_failure(now);
    assert_eq!(
        third,
        Duration::from_secs(600),
        "three consecutive failures escalate to 5x base"
    );
    state.note_success();
    assert!(!state.cooling_down(now), "success clears the backoff");
    assert_eq!(
        state.note_failure(now),
        Duration::from_secs(120),
        "success also resets the escalation counter"
    );
}
