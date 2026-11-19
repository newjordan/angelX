use super::*;

fn call(name: &str, args: serde_json::Value) -> Vec<ToolCall> {
    vec![ToolCall {
        id: "c".into(),
        name: name.into(),
        args,
    }]
}

fn cargo_test() -> Vec<ToolCall> {
    call("shell", serde_json::json!({"command": "cargo test 2>&1 | tail -30"}))
}

#[test]
fn a_call_repeated_silently_three_times_earns_one_note_naming_it() {
    let mut guard = SilenceGuard::default();
    assert_eq!(guard.observe(None, &cargo_test(), false), None);
    assert_eq!(guard.observe(Some("  "), &cargo_test(), false), None);
    let note = guard
        .observe(None, &cargo_test(), false)
        .expect("third silent repeat");
    assert!(note.contains("`shell "), "{note}");
    assert!(note.contains("cargo test"), "{note}");
    assert!(note.contains("3 times in a row"), "{note}");
    assert!(note.contains("one or two lines"), "{note}");
    assert!(guard.take_wake());
    assert!(!guard.take_wake());
}

#[test]
fn text_or_a_changed_call_restarts_the_repeat_count() {
    let mut guard = SilenceGuard::default();
    guard.observe(None, &cargo_test(), false);
    guard.observe(None, &cargo_test(), false);
    assert_eq!(
        guard.observe(Some("the frame index is off by one"), &cargo_test(), false),
        None
    );
    assert_eq!(guard.observe(None, &cargo_test(), false), None);
    let other = call("shell", serde_json::json!({"command": "cargo test strike"}));
    assert_eq!(guard.observe(None, &other, false), None);
    assert_eq!(guard.observe(None, &cargo_test(), false), None);
    assert!(!guard.take_wake());
}

#[test]
fn a_pasted_back_reply_is_no_new_text() {
    let paste = "⠽⠃⠍⠁⠽⠑\n⠍⠓\n`/ws/robot-name`";
    let echo = call("shell", serde_json::json!({"command": "echo hi"}));
    let mut guard = SilenceGuard::default();
    assert_eq!(guard.observe(Some(paste), &echo, false), None);
    assert_eq!(guard.observe(Some(paste), &echo, false), None);
    let note = guard
        .observe(Some(paste), &echo, false)
        .expect("pasted repeat");
    assert!(note.contains("echo hi"), "{note}");
}

#[test]
fn eight_textless_hops_earn_a_note_even_when_the_calls_differ() {
    let mut guard = SilenceGuard::default();
    for n in 1..SILENT_HOP_LIMIT {
        let probe = call("shell", serde_json::json!({"command": format!("cargo test case_{n}")}));
        assert_eq!(guard.observe(None, &probe, false), None, "hop {n}");
    }
    let last = call("shell", serde_json::json!({"command": "cargo test last"}));
    let note = guard
        .observe(None, &last, false)
        .expect("eighth silent hop");
    assert!(note.contains("last 8 tool calls came with no text"), "{note}");
    assert!(guard.take_wake());
}

#[test]
fn the_rust_bowling_loop_is_interrupted_and_the_guard_stays_bounded() {
    let mut guard = SilenceGuard::default();
    let mut notes = Vec::new();
    let mut wakes = 0;
    for hop in 0..60 {
        let calls = if hop < 15 {
            call("shell", serde_json::json!({"command": format!("cargo test t{hop}")}))
        } else {
            cargo_test()
        };
        if let Some(note) = guard.observe(None, &calls, false) {
            notes.push(hop);
            assert!(!note.is_empty());
        }
        wakes += usize::from(guard.take_wake());
    }
    assert_eq!(notes.len(), NOTES_PER_TURN, "{notes:?}");
    assert_eq!(wakes, NOTES_PER_TURN);
    assert_eq!(notes[0], SILENT_HOP_LIMIT - 1, "{notes:?}");
    assert!(notes.windows(2).all(|pair| pair[1] - pair[0] >= SAME_CALL_LIMIT));
}

#[test]
fn private_reasoning_is_not_silence() {
    let mut guard = SilenceGuard::default();
    for hop in 0..(SILENT_HOP_LIMIT + 4) {
        let probe = call(
            "shell",
            serde_json::json!({"command": format!("cargo test case_{hop}")}),
        );
        assert_eq!(guard.observe(None, &probe, true), None, "hop {hop}");
    }
    for _ in 0..(SAME_CALL_LIMIT + 2) {
        assert_eq!(guard.observe(None, &cargo_test(), true), None);
    }
    assert!(!guard.take_wake());
}

#[test]
fn a_reasoned_hop_restarts_the_silent_counts() {
    let mut guard = SilenceGuard::default();
    assert_eq!(guard.observe(None, &cargo_test(), false), None);
    assert_eq!(guard.observe(None, &cargo_test(), false), None);
    assert_eq!(guard.observe(None, &cargo_test(), true), None);
    assert_eq!(guard.observe(None, &cargo_test(), false), None);
    assert_eq!(guard.observe(None, &cargo_test(), false), None);
    let note = guard
        .observe(None, &cargo_test(), false)
        .expect("third unreasoned repeat after the reasoned hop");
    assert!(note.contains("3 times in a row"), "{note}");
}
