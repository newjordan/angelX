//! Background compaction (maybe_start_bg_compact / try_splice) coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory. Shared summarizer
//! fixtures (`SummarizerClub`, `PanickingSummarizerClub`, `long_history`,
//! `registry_with_store`) remain in the parent module for context_compact and
//! other suites.

use super::*;

// --- background compaction suite --------------------------------------------

fn bg_test_turn_context(label: &str) -> String {
    format!(
        "{}\n[operator-selected cockpit controls]\n- {label}\n\
         [/operator-selected cockpit controls]\n\n[/harness turn context]",
        crate::app::control::TURN_CONTEXT_HEADER
    )
}

#[test]
fn bg_compact_arms_and_splices_at_a_later_boundary() {
    let _guard = crate::tests::env_lock();
    let mut h = long_history();
    let n0 = h.len();
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    reg.set_aux_clubs(vec![std::sync::Arc::new(SummarizerClub("BG NOTES"))]);
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    assert!(
        bg_compact_inflight(&reg),
        "pass should arm over the 80% threshold"
    );
    assert_eq!(h.len(), n0, "arming must not touch history");
    // The summarizer runs off-thread; poll the boundary hook until it lands.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(std::time::Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut h, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.len() < n0, "history shrank: {n0} -> {}", h.len());
    let note = h
        .iter()
        .find(|m| m.content.contains("BG NOTES"))
        .expect("summary note spliced in");
    assert_eq!(
        note.role,
        ChatRole::Harness,
        "bg summary must use the internal background carrier"
    );
    assert!(crate::agent::compaction::is_compaction_note(note));
    assert_eq!(
        &*h.last().unwrap().content,
        "assistant answer number 7 text here",
        "newest turn preserved"
    );
    // Pairing intact: every kept tool result still has a preceding call.
    for (i, m) in h.iter().enumerate() {
        if m.role == ChatRole::Tool {
            let id = m.tool_call_id.clone().unwrap();
            assert!(
                h[..i]
                    .iter()
                    .any(|p| p.tool_calls.iter().any(|c| c.id == id)),
                "tool {id} kept its call"
            );
        }
    }
}

#[test]
fn background_compaction_preserves_user_role_task_anchor() {
    let _guard = crate::tests::env_lock();
    let task = "keep this exact background-compaction objective";
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user(task)];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "background work step {i} with enough context pressure"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(Arc::new(crate::knowledge::memory::store::NullStore));
    reg.set_aux_clubs(vec![Arc::new(SummarizerClub(
        "## Task\n- lossy background summary",
    ))]);
    maybe_start_bg_compact(&mut history, 60, 3, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::User && message.content.as_ref() == task)
            .count(),
        1
    );
}

#[test]
fn background_compaction_preserves_exact_harness_turn_context() {
    let _guard = crate::tests::env_lock();
    let context = bg_test_turn_context("keep the active goal");
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user("continue the background task"),
        ChatMsg::harness(context.as_str()),
    ];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "background context step {i} with enough pressure"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(Arc::new(crate::knowledge::memory::store::NullStore));
    reg.set_aux_clubs(vec![Arc::new(SummarizerClub(
        "## Task\n- lossy background summary",
    ))]);
    maybe_start_bg_compact(&mut history, 60, 3, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    let contexts = history
        .iter()
        .filter(|message| crate::app::control::is_turn_context_message(message))
        .collect::<Vec<_>>();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].role, ChatRole::Harness);
    assert_eq!(&*contexts[0].content, context);
}

#[test]
fn background_compaction_prefers_newer_surviving_turn_context() {
    let _guard = crate::tests::env_lock();
    let old_context = bg_test_turn_context("old controls");
    let new_context = bg_test_turn_context("new controls");
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user("continue the background task"),
        ChatMsg::harness(old_context.as_str()),
    ];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "background context step {i} with enough pressure"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(Arc::new(crate::knowledge::memory::store::NullStore));
    reg.set_aux_clubs(vec![Arc::new(SummarizerClub(
        "## Task\n- stale background summary",
    ))]);
    maybe_start_bg_compact(&mut history, 60, 3, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));

    // A newer replaceable context can arrive outside the snapshotted window
    // while the summarizer runs. It wins; the captured context may not be
    // resurrected alongside it.
    history.push(ChatMsg::harness(new_context.as_str()));
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!history.iter().any(|message| {
        message.role == ChatRole::Harness && message.content.as_ref() == old_context
    }));
    assert_eq!(
        history
            .iter()
            .filter(|message| {
                message.role == ChatRole::Harness && message.content.as_ref() == new_context
            })
            .count(),
        1
    );
}

#[test]
fn background_compaction_preserves_assistant_role_plan_snapshot() {
    let _guard = crate::tests::env_lock();
    let plan_text = "finish the background compaction proof";
    let plan_state = serde_json::json!({
        "next_id": 1,
        "items": [{"id":1,"text":plan_text,"done":false}],
    });
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user("continue"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "todo-bg".into(),
            name: "todo".into(),
            args: serde_json::json!({"action":"list"}),
        }]),
        ChatMsg::tool(
            "todo-bg",
            format!(
                "{}{}",
                crate::agent::tools::plan::TODO_STATE_PREFIX,
                plan_state
            ),
        ),
    ];
    for i in 0..80 {
        history.push(ChatMsg::assistant(format!("work {i} {}", "x".repeat(200))));
    }
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(Arc::new(crate::knowledge::memory::store::NullStore));
    reg.set_aux_clubs(vec![Arc::new(SummarizerClub(
        "## Task\n- vague summary\n## Facts\n- retained",
    ))]);
    maybe_start_bg_compact(&mut history, 1_000, 3, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    let plans = history
        .iter()
        .filter(|message| message.content.starts_with("[current-plan/v1"))
        .collect::<Vec<_>>();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].role, ChatRole::Assistant);
    assert!(plans[0].content.contains(plan_text));
}

#[test]
fn background_compaction_drops_stale_anchor_when_newer_user_direction_arrives() {
    let _guard = crate::tests::env_lock();
    let old_task = "old objective that enters the compaction window";
    let new_task = "new user direction that must supersede the old anchor";
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user(old_task)];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "background work step {i} with enough context pressure"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(Arc::new(crate::knowledge::memory::store::NullStore));
    reg.set_aux_clubs(vec![Arc::new(SummarizerClub("STALE TASK NOTES"))]);
    maybe_start_bg_compact(&mut history, 60, 3, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));

    // The snapshotted window is unchanged, but a newer user instruction now
    // exists after it. The background result may still land; its old task anchor
    // may not, or the older instruction would be presented as equally current.
    history.push(ChatMsg::user(new_task));
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !history
            .iter()
            .any(|message| message.role == ChatRole::User && message.content.as_ref() == old_task)
    );
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::User && message.content.as_ref() == new_task)
            .count(),
        1
    );
}

#[test]
fn background_compaction_keeps_task_anchor_when_only_harness_direction_arrives() {
    // Serialized: the boundary call below drains the process-global lift
    // notice that the cache-budget-lift tests queue and assert on; without
    // the lock it steals their notice mid-test.
    let _guard = crate::tests::env_lock();
    let task = "operator objective that enters the compaction window";
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user(task)];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "background work step {i} with enough context pressure"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(Arc::new(crate::knowledge::memory::store::NullStore));
    reg.set_aux_clubs(vec![Arc::new(SummarizerClub("TASK ORIGIN NOTES"))]);
    maybe_start_bg_compact(&mut history, 60, 3, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));

    history.push(ChatMsg::harness(FINAL_MILE_NUDGE));
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::User && message.content.as_ref() == task)
            .count(),
        1,
        "harness direction must not supersede the real operator objective"
    );
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Harness && message.content.as_ref() == FINAL_MILE_NUDGE
    }));
}

#[test]
fn bg_compact_discards_when_history_moved_on() {
    let _guard = crate::tests::env_lock();
    let mut h = long_history();
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    reg.set_aux_clubs(vec![std::sync::Arc::new(SummarizerClub("STALE NOTES"))]);
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));
    // History changes inside the snapshot window while the pass runs — what a
    // prune or an emergency sync compaction would do.
    h.remove(2);
    let n_after = h.len();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(std::time::Instant::now() < deadline, "bg pass never landed");
        try_splice_bg_compact(&reg, &mut h, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(h.len(), n_after, "no splice into a history that moved on");
    assert!(
        !h.iter().any(|m| m.content.contains("STALE NOTES")),
        "stale summary discarded"
    );
}

#[test]
fn bg_compact_needs_a_summarizer_and_a_real_overage() {
    let _guard = crate::tests::env_lock();
    let mut h = long_history();
    let (tx, _rx) = mpsc::channel();
    // No aux club and no ANGEL_COMPACT_URL → the in-hand club is the only
    // candidate, and it's borrowed/busy: stay on the sync path.
    let reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    assert!(
        !bg_compact_inflight(&reg),
        "no owned summarizer → no bg pass"
    );
    // Under the threshold → no pass either, even with a summarizer at hand.
    let mut reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    reg.set_aux_clubs(vec![std::sync::Arc::new(SummarizerClub("EARLY"))]);
    maybe_start_bg_compact(&mut h, 1_000_000, 5, 0, &[], &reg, &tx);
    assert!(!bg_compact_inflight(&reg), "under threshold → no bg pass");
}

#[test]
fn bg_compact_worker_panic_releases_inflight_slot() {
    let _guard = crate::tests::env_lock();
    let mut h = long_history();
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    reg.set_aux_clubs(vec![std::sync::Arc::new(PanickingSummarizerClub)]);
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(
            std::time::Instant::now() < deadline,
            "panicking worker left the compaction slot wedged"
        );
        try_splice_bg_compact(&reg, &mut h, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn bg_compact_honors_compact_local_opt_out() {
    let _guard = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_COMPACT_LOCAL", "0") };
    let mut h = long_history();
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    reg.set_aux_clubs(vec![std::sync::Arc::new(SummarizerClub("FLEET"))]);
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_COMPACT_LOCAL") };
    assert!(
        !bg_compact_inflight(&reg),
        "ANGEL_COMPACT_LOCAL=0 must keep the background pass off the fleet"
    );
}

#[test]
fn bg_compact_failed_pass_backs_off_before_respawning() {
    let _guard = crate::tests::env_lock();
    let mut h = long_history();
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(std::sync::Arc::new(
        crate::knowledge::memory::store::NullStore,
    ));
    reg.set_aux_clubs(vec![std::sync::Arc::new(PanickingSummarizerClub)]);
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    assert!(bg_compact_inflight(&reg));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(
            std::time::Instant::now() < deadline,
            "failed pass never drained"
        );
        try_splice_bg_compact(&reg, &mut h, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
    // The failing summarizer used to respawn at the very next boundary and
    // spam a fresh "compacting context" notice each time. Now it backs off.
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    assert!(
        !bg_compact_inflight(&reg),
        "a failed pass must not respawn inside the retry cooldown"
    );
    // Once the cooldown expires the route gets another chance.
    crate::agent::harness::compact::expire_bg_cooldown_for_test(&reg);
    maybe_start_bg_compact(&mut h, 30, 5, 0, &[], &reg, &tx);
    assert!(
        bg_compact_inflight(&reg),
        "an expired cooldown re-admits background compaction"
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while bg_compact_inflight(&reg) {
        assert!(std::time::Instant::now() < deadline, "drain after retry");
        try_splice_bg_compact(&reg, &mut h, &tx);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn stale_bg_compact_keeps_slot_until_worker_really_exits() {
    let _guard = crate::tests::env_lock();
    struct BlockingSummarizer {
        calls: Arc<AtomicUsize>,
        release: Arc<AtomicBool>,
    }
    impl Club for BlockingSummarizer {
        fn respond(&self, _p: &str) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            while !self.release.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok("late notes".to_string())
        }
        fn label(&self) -> &str {
            "blocking-compactor"
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let mut history = long_history();
    let (tx, _rx) = mpsc::channel();
    let mut reg = registry_with_store(Arc::new(crate::knowledge::memory::store::NullStore));
    reg.set_aux_clubs(vec![Arc::new(BlockingSummarizer {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    })]);
    maybe_start_bg_compact(&mut history, 30, 5, 0, &[], &reg, &tx);
    let deadline = Instant::now() + Duration::from_secs(2);
    while calls.load(Ordering::Acquire) == 0 {
        assert!(Instant::now() < deadline, "compaction worker never started");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(bg_compact_workers_inflight() > 0);

    age_bg_compact_for_test(&reg);
    try_splice_bg_compact(&reg, &mut history, &tx);
    assert!(
        bg_compact_inflight(&reg),
        "stale slot must remain while its worker is actually blocked"
    );
    maybe_start_bg_compact(&mut history, 30, 5, 0, &[], &reg, &tx);
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        calls.load(Ordering::Acquire),
        1,
        "stale pass must not spawn a replacement worker"
    );

    release.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(2);
    while bg_compact_inflight(&reg) {
        assert!(
            Instant::now() < deadline,
            "completed stale slot never cleared"
        );
        try_splice_bg_compact(&reg, &mut history, &tx);
        std::thread::sleep(Duration::from_millis(2));
    }
}
