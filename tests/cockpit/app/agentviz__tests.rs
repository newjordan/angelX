use super::SeatState;

/// Both tests mutate the process-global stage; serialize them so the
/// parallel test runner can't interleave a `clear` into an assertion.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn publish_roundtrips_and_clear_disbands() {
    let _serial = serial();
    super::stage("proposers", vec!["a".into(), "b".into()]);
    let s = super::current().expect("published stage visible");
    assert_eq!(s.name, "proposers");
    assert_eq!(s.agents.len(), 2);
    let seq0 = s.seq;
    super::stage("judge", vec!["j1".into()]);
    assert!(super::current().unwrap().seq > seq0);
    super::clear();
    assert!(super::current().is_none());
    let cleared = super::activity();
    assert!(cleared.sequence > seq0);
    assert!(cleared.stage.is_none());
    super::stage("verify", vec!["v1".into()]);
    assert!(super::current().unwrap().seq > cleared.sequence);
    super::clear();
}

#[test]
fn seat_updates_merge_into_the_snapshot_and_bump_seq() {
    let _serial = serial();
    super::stage("proposer wave 2", vec!["a".into(), "b".into(), "c".into()]);
    let s0 = super::current().expect("published stage visible");
    assert!(s0.seat_states.is_empty(), "publish starts all-Running");
    assert_eq!(s0.seat_state(1), SeatState::Running);
    assert_eq!(s0.returned(), 0);
    assert!(
        super::current_seat_pips().is_none(),
        "strip pips wait for a published seat state"
    );

    super::stage_seat_update(1, SeatState::Returned);
    let s1 = super::current().unwrap();
    assert!(s1.seq > s0.seq, "a seat change bumps the seq");
    assert_eq!(s1.stage_id, s0.stage_id, "seat results preserve identity");
    assert_eq!(
        s1.seat_states,
        vec![SeatState::Running, SeatState::Returned]
    );
    assert_eq!(s1.returned(), 1);
    assert_eq!(
        super::current_seat_pips().as_deref(),
        Some("proposer wave 2 · 1/3 back · ")
    );

    // A repeat of the same state is a no-op — no seq churn.
    super::stage_seat_update(1, SeatState::Returned);
    assert_eq!(super::current().unwrap().seq, s1.seq);

    super::stage_seat_update(0, SeatState::Failed);
    super::stage_seat_update(2, SeatState::Cut);
    let s2 = super::current().unwrap();
    assert_eq!(
        s2.seat_states,
        vec![SeatState::Failed, SeatState::Returned, SeatState::Cut]
    );
    assert_eq!(s2.returned(), 1, "only Returned counts as back");

    // Out-of-roster updates are dropped without a bump.
    super::stage_seat_update(9, SeatState::Returned);
    assert_eq!(super::current().unwrap().seq, s2.seq);

    // No live stage: the update is a no-op, not a resurrection.
    super::clear();
    super::stage_seat_update(0, SeatState::Returned);
    assert!(super::current().is_none());

    // A fresh publish resets every seat to implicit Running.
    super::stage("judge", vec!["j1".into()]);
    assert!(super::current().unwrap().seat_states.is_empty());
    assert!(
        super::current_seat_pips().is_none(),
        "a one-seat stage has no strip pips"
    );
    super::clear();
}
