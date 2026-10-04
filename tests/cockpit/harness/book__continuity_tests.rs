use super::*;

fn route(model: &str) -> RouteIdentity {
    RouteIdentity {
        driver: "fixture".into(),
        model: Some(model.into()),
        reasoning_effort: None,
    }
}

#[test]
fn swapping_models_reteaches_used_signals_and_keeps_the_cached_prefix() {
    let mut history = vec![
        ChatMsg::system("⠽⠙"),
        ChatMsg::user("research"),
        ChatMsg::harness("⚠⡫⠃"),
    ];
    let mut state = Continuity::new(&history);
    assert!(!state.prepare(&mut history, &route("a")));
    let prefix = history
        .iter()
        .map(|m| m.content.clone())
        .collect::<Vec<_>>();
    assert!(!state.prepare(&mut history, &route("a")));
    assert!(state.prepare(&mut history, &route("b")));
    for (before, after) in prefix.iter().zip(&history) {
        assert_eq!(before, &after.content);
    }
    let introductions = introduction::introductions(&history, None);
    assert!(
        introductions
            .last()
            .unwrap()
            .1
            .contains("the same check returned identical evidence")
    );
    assert_eq!(introductions.last().unwrap().0, history.len() - 1);
}

#[test]
fn compaction_restores_the_inventory_even_when_all_old_signals_disappeared() {
    let mut history = vec![ChatMsg::harness("⚠⡫⠉"), ChatMsg::harness("⠧⠉")];
    let mut state = Continuity::new(&history);
    state.prepare(&mut history, &route("a"));
    history = vec![ChatMsg::harness("summary with no warning addresses")];
    assert!(state.prepare(&mut history, &route("a")));
    let carried = history.last().unwrap().content.clone();
    assert!(carried.contains("⡫⠉") && carried.contains("⠧⠉"));
    // Session restore starts from the compact history, not process-global state.
    let mut restored = Continuity::new(&history);
    assert!(!restored.prepare(&mut history, &route("a")));
    assert!(restored.prepare(&mut history, &route("b")));
}

#[test]
fn effort_change_and_quiet_turn_add_no_handoff() {
    let mut history = vec![ChatMsg::system("⠽⠙"), ChatMsg::user("hello")];
    let mut state = Continuity::new(&history);
    assert!(!state.prepare(&mut history, &route("a")));
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|message| message.book_memory.is_none()));
    history.push(ChatMsg::harness("⚠⡫⠙"));
    state.prepare(&mut history, &route("a"));
    let mut changed = route("a");
    changed.reasoning_effort = Some("high".into());
    assert!(!state.prepare(&mut history, &changed));
}

#[test]
fn warning_is_decodable_and_removed_from_distilled_prose() {
    assert!(ledger::is_warpath_message("⚠⡫⠃\n{}"));
    let history = [ChatMsg::harness("⚠⡫⠃")];
    assert!(
        introduction::introductions(&history, None)[0]
            .1
            .contains("intentional replication")
    );
}

#[test]
fn recalled_hypothesis_braille_is_data_and_does_not_raise_a_new_route() {
    let root = std::path::Path::new("/fixture/book-data");
    let advice = serde_json::json!({"relation_count":1,"checks":[{"latest":{
        "hypothesis":"⠇⠁", "expected":"⠟⠁"}, "count":1}], "signals":[]});
    let note = live::context_turn(root, &advice);
    let intros = introduction::introductions(&[ChatMsg::harness(note)], None);
    assert!(!intros[0].1.contains("⠇⠁ "));
    assert!(!intros[0].1.contains("⠟⠁ "));
    assert!(intros[0].1.contains("⡫⠁ "));
}

#[test]
fn session_restore_keeps_inventory_without_changing_the_provider_wire() {
    let mut history = vec![ChatMsg::user("research"), ChatMsg::harness("⚠⡫⠃")];
    let wire = crate::agent::club::messages_to_json(&history, false);
    let mut state = Continuity::new(&history);
    assert!(!state.prepare(&mut history, &route("a")));
    assert_eq!(crate::agent::club::messages_to_json(&history, false), wire);
    let saved = serde_json::to_vec(&history).unwrap();
    let mut restored: Vec<ChatMsg> = serde_json::from_slice(&saved).unwrap();
    let mut state = Continuity::new(&restored);
    assert!(!state.prepare(&mut restored, &route("a")));
    restored.push(ChatMsg::user("continue"));
    assert!(!state.prepare(&mut restored, &route("a")));
    assert_eq!(
        restored.iter().filter(|m| m.book_memory.is_some()).count(),
        1
    );
    assert!(state.prepare(&mut restored, &route("b")));
    assert!(restored.last().unwrap().content.contains("⡫⠃"));
}

#[test]
fn compaction_between_turns_carries_even_uncheckpointed_signals() {
    let mut history = vec![ChatMsg::user("research"), ChatMsg::harness("⚠⡫⠃")];
    Continuity::new(&history).prepare(&mut history, &route("a"));
    history.push(ChatMsg::harness("⚠⡫⠉"));
    let mut summary = ChatMsg::harness("summary with no signals");
    summary.book_memory = compaction_memory(&history);
    let mut compacted = vec![summary];
    let mut state = Continuity::new(&compacted);
    assert!(state.prepare(&mut compacted, &route("a")));
    let note = compacted.last().unwrap();
    assert!(note.content.contains("⡫⠃") && note.content.contains("⡫⠉"));
    assert!(!Continuity::new(&compacted).prepare(&mut compacted, &route("a")));
    assert!(Continuity::new(&compacted).prepare(&mut compacted, &route("b")));
}
