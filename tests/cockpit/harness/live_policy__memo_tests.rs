use super::*;
use std::cell::Cell;

const LEG: &str = "result-classify";

/// A question no other test asks: the leg's own input shape, with a unique exit code.
fn question() -> serde_json::Value {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1_000_000);
    serde_json::json!({
        "error": true, "verifier": true, "has_exit": true,
        "exit": SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        "empty_marker": false, "club_stream": false,
    })
}

fn ask(input: &serde_json::Value) -> serde_json::Value {
    decide_shipped::<serde_json::Value>(LEG, input, |_| Ok(())).0
}

fn ask_every_time(input: &serde_json::Value) -> serde_json::Value {
    decide_shipped_unmemoized::<serde_json::Value>(LEG, input, |_| Ok(())).0
}

#[test]
fn the_same_question_to_the_same_generation_is_answered_once() {
    let input = question();
    let (_, misses_before) = memo::counts();
    let first = ask(&input);
    let (hits_mid, misses_mid) = memo::counts();
    assert!(
        misses_mid > misses_before,
        "the first ask reaches the module"
    );
    let second = ask(&input);
    let (hits_after, _) = memo::counts();
    assert_eq!(
        first, second,
        "the answer cannot depend on being asked twice"
    );
    assert!(
        hits_after > hits_mid,
        "the second ask is served from the memo"
    );
}

#[test]
fn a_memoized_decision_is_identical_to_asking_the_module() {
    for _ in 0..5 {
        let input = question();
        let direct = ask_every_time(&input);
        assert_eq!(ask(&input), direct);
        assert_eq!(ask(&input), direct);
    }
}

#[test]
fn the_game_rules_path_keeps_nothing() {
    let input = question();
    let digest = angel_live::shipped::program(LEG)
        .unwrap()
        .digest()
        .to_string();
    let key = memo::key(&digest, &input).unwrap();
    ask_every_time(&input);
    ask_every_time(&input);
    assert!(
        !memo::holds(&key),
        "the unmemoized path must not record an answer"
    );
    ask(&input);
    assert!(memo::holds(&key), "the memoized path does");
}

#[test]
fn a_different_input_or_a_different_generation_is_a_different_question() {
    let (a, b) = (question(), question());
    assert_ne!(
        memo::key("one", &a),
        memo::key("one", &b),
        "the input is part of the question"
    );
    let shipped = angel_live::shipped::program(LEG).unwrap();
    let other = angel_live::shipped::program("verify-policy").unwrap();
    assert_ne!(shipped.digest(), other.digest());
    assert_ne!(
        memo::key(shipped.digest(), &a),
        memo::key(other.digest(), &a),
        "the generation is part of the question"
    );
}

#[test]
fn a_rejected_answer_is_retried_against_the_module_not_the_memo() {
    let input = question();
    let expected = ask_every_time(&input);
    // Prime the memo, then reject the first answer the caller sees.
    ask(&input);
    let rejections = Cell::new(0);
    let (answer, _) = decide_shipped::<serde_json::Value>(LEG, &input, |_| {
        if rejections.get() == 0 {
            rejections.set(1);
            Err("first answer rejected".to_string())
        } else {
            Ok(())
        }
    });
    assert_eq!(rejections.get(), 1, "the check ran again on the retry");
    assert_eq!(answer, expected);
}

#[test]
fn the_memo_is_bounded_and_oversized_questions_are_not_kept() {
    for round in 0..(memo::CAPACITY + 40) {
        memo::keep(
            ("bound-test".to_string(), format!("{{\"n\":{round}}}")),
            &serde_json::json!({"n": round}),
        );
    }
    assert!(memo::len() <= memo::CAPACITY);

    let big = serde_json::json!({"text": "x".repeat(32 * 1024)});
    assert_eq!(memo::key("digest", &big), None);
    assert!(memo::key("digest", &serde_json::json!({"text": "x"})).is_some());
    let before = memo::len();
    memo::keep(
        ("bound-test".to_string(), "oversized-answer".to_string()),
        &serde_json::json!({"text": "y".repeat(32 * 1024)}),
    );
    assert_eq!(memo::len(), before, "an oversized answer is not kept");
}
