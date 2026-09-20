use super::*;

// save/load roundtrip mutates the process-global ANGEL_GOAL_FILE, so it lives
// in the app-level tests under `env_lock`. Here we cover the pure logic.

#[test]
fn strip_word_matches_subcommands() {
    assert_eq!(
        strip_word("criteria all tests pass", &["criteria"]),
        Some("all tests pass")
    );
    assert_eq!(strip_word("+ship it", &["+"]), Some("ship it"));
    assert_eq!(strip_word("+ ship it", &["+"]), Some("ship it"));
    assert_eq!(strip_word("criterion", &["criteria"]), None); // no trailing space
    assert_eq!(strip_word("note", &["note"]), None); // bare word, no remainder
}

#[test]
fn strip_word_handles_every_alias_and_boundary() {
    // Each refine keyword matches case-insensitively when followed by a space.
    assert_eq!(
        strip_word("ACCEPT it must build", &["accept"]),
        Some("it must build")
    );
    assert_eq!(
        strip_word("verify cargo test", &["verify"]),
        Some("cargo test")
    );
    assert_eq!(strip_word("check ./run.sh", &["check"]), Some("./run.sh"));
    assert_eq!(
        strip_word("note remember this", &["note"]),
        Some("remember this")
    );
    // The first matching word in the list wins.
    assert_eq!(
        strip_word("cmd lint", &["cmd", "verify", "check"]),
        Some("lint")
    );
    // `+` shorthand needs no trailing space, but an empty remainder is rejected.
    assert_eq!(strip_word("+", &["+"]), None);
    assert_eq!(strip_word("+   ", &["+"]), None);
    // A word with no trailing whitespace is not a subcommand (it's the goal text).
    assert_eq!(strip_word("noteworthy idea", &["note"]), None);
    // Trailing whitespace but empty remainder → None.
    assert_eq!(strip_word("note    ", &["note"]), None);
    // No word matches → None.
    assert_eq!(
        strip_word("ship the cockpit", &["criteria", "cmd", "note"]),
        None
    );
}

#[test]
fn strip_word_does_not_panic_on_non_ascii_leading_text() {
    assert_eq!(
        strip_word("ធ្វើការ criteria all tests pass", &["criteria"]),
        None
    );
    assert_eq!(
        strip_word("criteria 日本語 passes", &["criteria"]),
        Some("日本語 passes")
    );
}

#[test]
fn casual_goal_bypass_catches_greetings_not_work() {
    for raw in ["hello", "Hello!", "hi there", "thanks man", "ping"] {
        assert!(casual_goal_bypass(raw), "{raw:?} should bypass goal");
    }
    for raw in [
        "build the game",
        "debug the failing route",
        "please save and launch it",
        "implement the verifier",
    ] {
        assert!(!casual_goal_bypass(raw), "{raw:?} should keep goal");
    }
}

#[test]
fn new_goal_is_active_with_matching_timestamps() {
    let g = Goal::new("win");
    assert_eq!(g.text, "win");
    assert_eq!(g.status, GoalStatus::Active);
    assert!(g.acceptance.is_empty());
    assert!(g.accept_cmd.is_none());
    assert!(g.notes.is_empty());
    // created/updated are stamped together at construction.
    assert_eq!(g.created_ms, g.updated_ms);
    assert!(g.created_ms > 0, "now_ms returned a real epoch time");
}

#[test]
fn goal_status_default_is_active() {
    assert_eq!(GoalStatus::default(), GoalStatus::Active);
}

#[test]
fn goal_limits_reject_oversized_or_overfull_records() {
    let mut goal = Goal::new("x".repeat(MAX_GOAL_TEXT_BYTES));
    assert!(goal_within_limits(&goal));
    goal.text.push('x');
    assert!(!goal_within_limits(&goal));

    let mut goal = Goal::new("bounded objective");
    goal.acceptance = vec!["criterion".to_string(); MAX_GOAL_ACCEPTANCE_ITEMS + 1];
    assert!(!goal_within_limits(&goal));

    let mut goal = Goal::new("bounded objective");
    goal.notes = vec!["note".to_string(); MAX_GOAL_NOTES];
    goal.accept_cmd = Some("x".repeat(MAX_GOAL_COMMAND_BYTES));
    assert!(goal_within_limits(&goal));
    goal.notes.push("one too many".to_string());
    assert!(!goal_within_limits(&goal));
}

#[test]
fn goal_status_serializes_snake_case() {
    // The `#[serde(rename_all = "snake_case")]` contract the store relies on.
    assert_eq!(
        serde_json::to_string(&GoalStatus::Active).unwrap(),
        "\"active\""
    );
    assert_eq!(
        serde_json::to_string(&GoalStatus::Paused).unwrap(),
        "\"paused\""
    );
    let back: GoalStatus = serde_json::from_str("\"done\"").unwrap();
    assert_eq!(back, GoalStatus::Done);
}

#[test]
fn paused_is_the_canonical_write_and_abandoned_still_reads() {
    // Migration step 1 (docs/plans/goal-mission-conformance.md): the goal
    // side now speaks the mission vocabulary. New writes emit `paused`;
    // legacy stores holding `"abandoned"` load as `Paused` via the serde
    // read alias and are re-written as `paused` on their next save.
    let legacy: GoalStatus = serde_json::from_str("\"abandoned\"").unwrap();
    assert_eq!(legacy, GoalStatus::Paused);
    assert_eq!(serde_json::to_string(&legacy).unwrap(), "\"paused\"");

    // A whole legacy record round-trips through the new vocabulary.
    let raw = r#"{"text":"old store","status":"abandoned"}"#;
    let g: Goal = serde_json::from_str(raw).unwrap();
    assert_eq!(g.status, GoalStatus::Paused);
    let json = serde_json::to_string(&g).unwrap();
    assert!(json.contains("\"status\":\"paused\""), "{json}");
    assert!(
        !json.contains("abandoned"),
        "write side is deprecated: {json}"
    );
}

#[test]
fn goal_deserializes_with_defaulted_optional_fields() {
    // A minimal JSON (just `text`) must load, filling defaults — proving the
    // `#[serde(default)]` annotations on the optional fields.
    let g: Goal = serde_json::from_str(r#"{"text":"only text"}"#).unwrap();
    assert_eq!(g.text, "only text");
    assert_eq!(g.status, GoalStatus::Active);
    assert!(g.acceptance.is_empty());
    assert!(g.accept_cmd.is_none());
    assert_eq!(g.created_ms, 0);
}

#[test]
fn context_block_empty_unless_active() {
    let mut g = Goal::new("ship the cockpit");
    assert!(serde_json::to_string(&g).is_ok());
    // Active with text → non-empty, well-formed.
    g.acceptance.push("tests green".to_string());
    g.accept_cmd = Some("cargo test".to_string());
    let json = serde_json::to_string(&g).unwrap();
    let back: Goal = serde_json::from_str(&json).unwrap();
    assert_eq!(back.acceptance, vec!["tests green".to_string()]);
    assert_eq!(back.accept_cmd.as_deref(), Some("cargo test"));
    assert_eq!(back.status, GoalStatus::Active);
}

#[test]
fn goal_status_vocabulary_matches_the_conformance_contract() {
    // docs/plans/goal-mission-conformance.md: the four statuses are the
    // goal side of the mapping table — renaming or adding one without
    // updating the contract + the Node side is a drift bug.
    let mut seen = std::collections::HashSet::new();
    for status in [
        GoalStatus::Active,
        GoalStatus::Done,
        GoalStatus::Paused,
        GoalStatus::Blocked,
    ] {
        let value: &'static str = match status {
            GoalStatus::Active => "active",
            GoalStatus::Done => "done",
            GoalStatus::Paused => "paused",
            GoalStatus::Blocked => "blocked",
        };
        let round: GoalStatus = serde_json::from_str(&format!("\"{value}\""))
            .expect("status serializes its snake_case name");
        assert_eq!(round, status);
        seen.insert(value);
    }
    assert_eq!(seen.len(), 4);
}

#[test]
fn goal_round_and_blocker_semantics_match_the_shared_rules() {
    // Rule 1: BLOCKED is earned — the same reason, 3 consecutive rounds.
    let mut g = Goal::new("x");
    g.tick_blocked("gpu down");
    g.tick_blocked("network down"); // resets the streak
    assert_eq!(g.blocked_streak, 1);
    g.tick_blocked("gpu down");
    g.tick_blocked("gpu down");
    g.tick_blocked("gpu down");
    assert_eq!(g.status, GoalStatus::Blocked);
    // Rule 2: resume keeps rounds and criteria; done is terminal.
    assert!(g.rearm());
    assert_eq!(g.status, GoalStatus::Active);
    g.status = GoalStatus::Done;
    assert!(!g.rearm(), "done is terminal-by-completion");
    // Rule 3: a real round clears blocking candidates.
    let mut h = Goal::new("y");
    h.tick_blocked("gpu down");
    h.record_round();
    assert_eq!(h.blocked_streak, 0);
    assert_eq!(h.blocked_reason, None);
}

#[test]
fn default_goal_rounds_remain_unbounded() {
    let mut goal = Goal::new("keep improving");
    assert_eq!(goal.max_rounds, None);
    goal.rounds = u32::MAX;
    assert!(goal.record_round());
    assert!(goal.can_continue());
    goal.max_rounds = Some(7);
    assert!(!goal.record_round());
}

#[test]
fn round_budget_bounds_continuation() {
    let mut g = Goal::new("x");
    assert!(g.can_continue(), "unbounded goals always continue");
    assert!(g.record_round());
    assert_eq!(g.rounds, 1);

    g.max_rounds = Some(2);
    assert!(g.record_round());
    assert!(!g.can_continue(), "a spent budget stops continuation");
    assert!(!g.record_round(), "spent budget refuses the round");
    assert_eq!(g.rounds, 2, "refused rounds never count");

    g.max_rounds = None;
    assert!(
        g.can_continue(),
        "removing the budget re-opens continuation"
    );
}

#[test]
fn blocked_requires_the_same_reason_for_three_consecutive_rounds() {
    let mut g = Goal::new("x");
    assert!(!g.tick_blocked("gpu down"));
    assert!(!g.tick_blocked("gpu down"));
    assert_eq!(g.status, GoalStatus::Active, "two rounds is not blocked");
    assert!(
        !g.tick_blocked("network down"),
        "a changing reason restarts"
    );
    assert_eq!(g.blocked_streak, 1);
    assert!(!g.tick_blocked("network down"));
    assert!(
        g.tick_blocked("network down"),
        "the third identical round flips the goal blocked"
    );
    assert_eq!(g.status, GoalStatus::Blocked);
    assert_eq!(g.blocked_reason.as_deref(), Some("network down"));
    assert!(!g.can_continue());
}

#[test]
fn real_rounds_and_done_clear_blocking_candidates() {
    let mut g = Goal::new("x");
    g.tick_blocked("gpu down");
    g.tick_blocked("gpu down");
    assert!(g.record_round());
    assert_eq!(g.blocked_streak, 0);
    assert_eq!(g.blocked_reason, None);

    g.tick_blocked("gpu down");
    g.status = GoalStatus::Done;
    // Terminal states absorb blocked ticks without changing anything.
    assert!(!g.tick_blocked("gpu down"));
    assert_eq!(g.status, GoalStatus::Done);
}

#[test]
fn rearm_keeps_state_and_clears_the_blocker() {
    let mut g = Goal::new("x");
    g.max_rounds = Some(9);
    g.acceptance.push("tests green".to_string());
    for _ in 0..GOAL_BLOCKED_MIN_ROUNDS {
        g.tick_blocked("gpu down");
    }
    assert_eq!(g.status, GoalStatus::Blocked);
    assert!(g.rearm());
    assert_eq!(g.status, GoalStatus::Active);
    assert_eq!(g.max_rounds, Some(9));
    assert_eq!(g.acceptance, vec!["tests green".to_string()]);
    assert_eq!(g.blocked_reason, None);
    assert_eq!(g.blocked_streak, 0);
    assert!(g.can_continue());

    // Done is terminal-by-completion: no re-arm.
    g.status = GoalStatus::Done;
    assert!(!g.rearm());
    assert_eq!(g.status, GoalStatus::Done);
}
