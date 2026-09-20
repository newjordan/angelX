use super::*;

#[test]
fn delegate_activity_is_live_owned_and_released_even_with_retained_sender() {
    let parent = AtomicBool::new(false);
    let nested = AtomicBool::new(false);
    let leaf = AtomicBool::new(false);
    let foreign = AtomicBool::new(false);
    let _link = link_child_owner(&nested, &parent);
    let _leaf_link = link_child_owner(&leaf, &nested);
    let owner = &parent as *const _ as usize;
    let mut retained = None;
    let (result, failures) = observe_delegate_turn(&leaf, "test", |tx| {
        retained = Some(tx.clone());
        tx.send(TurnEvent::Reasoning("private payload".into()))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let state = owned_delegate_snapshot(owner).unwrap();
            if state.phase == "thinking" {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "activity was buffered until completion"
            );
            std::thread::yield_now();
        }
        assert!(owned_delegate_snapshot(&foreign as *const _ as usize).is_none());
        tx.send(TurnEvent::ToolResult {
            id: ToolEventId("failed".into()),
            name: "shell".into(),
            summary: "failed".into(),
            outcome: ToolOutcome {
                execution: ExecutionOutcome::Failed,
                verification: VerificationOutcome::NotApplicable,
            },
        })
        .unwrap();
        Err::<(), _>("original failure")
    });
    assert_eq!(result, Err("original failure"));
    assert_eq!(failures, 1);
    assert!(owned_delegate_snapshot(owner).is_none());
    drop(retained);
}

#[test]
fn delegate_heartbeat_does_not_erase_silence_and_panic_cleans_up() {
    let cancel = AtomicBool::new(false);
    let owner = &cancel as *const _ as usize;
    let activity = Activity::new(&cancel, "test\n\u{1b}[m");
    let mut running = std::collections::BTreeMap::new();
    activity.observe(&TurnEvent::Heartbeat, &mut running);
    let state = owned_delegate_snapshot(owner).unwrap();
    assert_eq!(state.event_age_secs, Some(0));
    assert_eq!(state.progress_age_secs, None);
    assert_eq!(state.phase, "waiting for model");
    assert!(!state.label.contains('\u{1b}'));
    drop(activity);
    let result = std::panic::catch_unwind(|| {
        observe_delegate_turn(&cancel, "test", |_| panic!("worker panic"));
    });
    assert!(result.is_err());
    assert!(owned_delegate_snapshot(owner).is_none());
}
