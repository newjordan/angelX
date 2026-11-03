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
    assert_eq!(guard.observe(None, &cargo_test()), None);
    assert_eq!(guard.observe(Some("  "), &cargo_test()), None);
    let note = guard.observe(None, &cargo_test()).expect("third silent repeat");
    assert!(note.contains("`shell "), "{note}");
    assert!(note.contains("cargo test"), "{note}");
    assert!(note.contains("3 times in a row"), "{note}");
    assert!(note.contains("one or two lines"), "{note}");
    // The note wakes thinking for exactly the next hop.
    assert!(guard.take_wake());
    assert!(!guard.take_wake());
}

#[test]
fn text_or_a_changed_call_restarts_the_repeat_count() {
    let mut guard = SilenceGuard::default();
    guard.observe(None, &cargo_test());
    guard.observe(None, &cargo_test());
    assert_eq!(
        guard.observe(Some("the frame index is off by one"), &cargo_test()),
        None
    );
    assert_eq!(guard.observe(None, &cargo_test()), None);
    let other = call("shell", serde_json::json!({"command": "cargo test strike"}));
    assert_eq!(guard.observe(None, &other), None);
    assert_eq!(guard.observe(None, &cargo_test()), None);
    assert!(!guard.take_wake());
}

#[test]
fn a_pasted_back_reply_is_no_new_text() {
    // rust-robot-name, v1 s1: `echo hi` sixty times, each under the same
    // pasted-back system prompt.
    let paste = "⠽⠃⠍⠁⠽⠑\n⠍⠓\n`/ws/robot-name`";
    let echo = call("shell", serde_json::json!({"command": "echo hi"}));
    let mut guard = SilenceGuard::default();
    assert_eq!(guard.observe(Some(paste), &echo), None);
    assert_eq!(guard.observe(Some(paste), &echo), None);
    let note = guard.observe(Some(paste), &echo).expect("pasted repeat");
    assert!(note.contains("echo hi"), "{note}");
}

#[test]
fn eight_textless_hops_earn_a_note_even_when_the_calls_differ() {
    let mut guard = SilenceGuard::default();
    for n in 1..SILENT_HOP_LIMIT {
        let probe = call("shell", serde_json::json!({"command": format!("cargo test case_{n}")}));
        assert_eq!(guard.observe(None, &probe), None, "hop {n}");
    }
    let last = call("shell", serde_json::json!({"command": "cargo test last"}));
    let note = guard.observe(None, &last).expect("eighth silent hop");
    assert!(note.contains("last 8 tool calls came with no text"), "{note}");
    assert!(guard.take_wake());
}

#[test]
fn the_rust_bowling_loop_is_interrupted_and_the_guard_stays_bounded() {
    // v3 s0 rust-bowling: one silent rewrite, then 52 silent shell calls, 37
    // identical in a row, to the 60-hop cap.
    let mut guard = SilenceGuard::default();
    let mut notes = Vec::new();
    let mut wakes = 0;
    for hop in 0..60 {
        let calls = if hop < 15 {
            call("shell", serde_json::json!({"command": format!("cargo test t{hop}")}))
        } else {
            cargo_test()
        };
        if let Some(note) = guard.observe(None, &calls) {
            notes.push(hop);
            assert!(!note.is_empty());
        }
        wakes += usize::from(guard.take_wake());
    }
    assert_eq!(notes.len(), NOTES_PER_TURN, "{notes:?}");
    assert_eq!(wakes, NOTES_PER_TURN);
    assert_eq!(notes[0], SILENT_HOP_LIMIT - 1, "{notes:?}");
    // Each later note needs a fresh run of its own.
    assert!(notes.windows(2).all(|pair| pair[1] - pair[0] >= SAME_CALL_LIMIT));
}
