use super::failure_detail;

#[test]
fn failure_detail_keeps_cargo_summary_before_late_stderr_chatter() {
    let diagnostic = "---- a_roll_is_err_if_the_game_is_done stdout ----\n\nthread 'a_roll_is_err_if_the_game_is_done' panicked at tests/bowling.rs:56:5:\nassertion `left == right` failed\n  left: Ok(())\n right: Err(GameComplete)";
    let summary = "test result: FAILED. 11 passed; 20 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s";
    let output = format!(
        "running 31 tests\n{diagnostic}\n{}\n{summary}\n\n{}\nerror: test failed, to rerun pass `--test bowling`",
        "another failure and its diagnostic\n".repeat(120),
        "   Compiling bowling v1.2.0\n     Running tests/bowling.rs (target/debug/deps/bowling)\n"
            .repeat(12),
    );
    let detail = failure_detail(&output, 1500);
    assert!(detail.contains(diagnostic), "{detail}");
    assert!(detail.ends_with(summary), "{detail}");
    assert!(!detail.contains("Compiling bowling"), "{detail}");
    assert!(detail.len() <= 1500);
}

#[test]
fn failure_detail_keeps_first_actionable_failure_and_final_summary() {
    for (name, failure) in [
        (
            "cargo",
            "---- scoring stdout ----\nthread 'scoring' panicked at tests/bowling.rs:25:5:\nassertion failed: game.score().is_some()\n",
        ),
        (
            "pytest",
            "____________________ test_scoring ____________________\n> assert game.score() == 30\nE AssertionError: None != 30\n",
        ),
        (
            "unittest",
            "FAIL: test_scoring (test_bowling.BowlingTest)\nTraceback (most recent call last):\nAssertionError: None != 30\n",
        ),
        (
            "jest",
            "  ● Bowling › scoring\n    expect(received).toEqual(expected)\n    Expected: 30\n    Received: null\n",
        ),
        (
            "tap",
            "not ok 3 - scoring\n  error: expected 30, received null\n  code: ERR_ASSERTION\n",
        ),
        (
            "vitest",
            " FAIL  bowling.test.js > scoring\nAssertionError: expected null to deeply equal 30\n",
        ),
        (
            "direct-panic",
            "thread 'scoring' panicked at tests/bowling.rs:25:5:\nassertion failed: game.score().is_some()\n",
        ),
        (
            "gtest",
            "bowling_test.cpp:25: Failure\nExpected equality of these values:\n  game.score()\n  30\n",
        ),
        (
            "catch2",
            "bowling_test.cpp:25: FAILED:\n  REQUIRE( game.score() == 30 )\nwith expansion:\n  0 == 30\n",
        ),
        (
            "compiler",
            "error[E0308]: mismatched types\n --> src/lib.rs:25:5\nexpected u32, found String\n",
        ),
        (
            "syntax-error",
            "error: expected expression, found `}`\n --> src/lib.rs:25:5\n25 | }\n   | ^ expected expression\n",
        ),
    ] {
        let summary = "FINAL SUMMARY: 1 passed; 30 failed; exit 1";
        let output = format!(
            "{}{}{}\n{summary}\n",
            "Compiling dependency: ordinary build progress\n".repeat(120),
            failure,
            "later failing test; details omitted from fixture\n".repeat(120)
        );
        let detail = failure_detail(&output, 1500);
        assert!(
            detail.contains(failure.trim()),
            "{name}: first diagnostic was lost: {detail}"
        );
        assert!(
            detail.ends_with(summary),
            "{name}: final summary was lost: {detail}"
        );
        assert!(
            !detail.contains("Compiling dependency"),
            "{name}: build chatter displaced evidence"
        );
        assert!(
            detail.len() <= 1500,
            "{name}: byte budget exceeded: {}",
            detail.len()
        );
    }
}

#[test]
fn failure_detail_is_utf8_safe_and_honors_tiny_budgets() {
    let output = format!(
        "{}\n---- unicode stdout ----\nassertion failed: 値が違います\n{}\nFINAL 結果",
        "進捗".repeat(1000),
        "λ".repeat(2000)
    );
    let detail = failure_detail(&output, 1500);
    assert!(detail.contains("assertion failed: 値が違います"));
    assert!(detail.ends_with("FINAL 結果"));
    for budget in 0..90 {
        let detail = failure_detail(&output, budget);
        assert!(detail.len() <= budget, "budget {budget}: {detail:?}");
    }
}

#[test]
fn failure_detail_unknown_format_uses_bounded_tail_and_short_output_stays_intact() {
    let output = format!(
        "{}\ncustom runner diagnostic at end",
        "unrecognized output\n".repeat(200)
    );
    let detail = failure_detail(&output, 1500);
    assert!(detail.ends_with("custom runner diagnostic at end"));
    assert!(detail.contains("earlier output omitted"));
    assert!(detail.len() <= 1500);
    assert_eq!(
        failure_detail(" \nshort failure λ\n ", 1500),
        "short failure λ"
    );
}
