use super::*;
use crate::agent::harness::{Tool, VerificationOutcome};

#[test]
fn t04b_routing_table_and_safe_presentation() {
    let root = std::env::temp_dir();
    for (command, tool, runtime) in [
        ("npm test", "run_tests", "node"),
        ("pnpm test -- -k", "run_tests", "node"),
        ("yarn test", "run_tests", "node"),
        ("node --test test.mjs", "run_tests", "node"),
        ("pytest test_a.py -k works", "run_tests", "python"),
        ("python -m pytest tests", "run_tests", "python"),
        (
            "python3 -m unittest discover -s tests -p test_a.py -t .",
            "run_tests",
            "python",
        ),
        ("cargo test 2>&1 | tail -30", "run_tests", "rust"),
        ("cargo check", "check", "rust"),
        ("cargo clippy", "lint", "rust"),
        ("go test ./...", "run_tests", "go"),
    ] {
        for name in ["shell", "proc_run"] {
            let route = plan(name, &json!({"command":command}), &root).unwrap();
            assert_eq!(route.call.name, tool, "{command}");
            assert_eq!(route.call.args["runtime"], runtime, "{command}");
        }
    }
    for command in [
        "echo npm test",
        "./node --test x",
        "npm install",
        "cargo build",
        "cargo test | tee log",
        "cargo test || echo nope",
        "cargo test; touch x",
        "cargo test > log",
        "npm test $(echo x)",
        "npm test && echo yes",
        "npm test\ntrue",
    ] {
        assert!(
            plan("shell", &json!({"command":command}), &root).is_err(),
            "{command}"
        );
    }
    for command in [
        "npm test | tail -1",
        "npm test || true",
        "npm test; echo done",
    ] {
        let route = plan("shell", &json!({"command":command}), &root).unwrap();
        assert!(
            route
                .present("tests: 0 passed, 1 failed\nraw\nend".into(), false)
                .starts_with("tests: 0 passed, 1 failed")
        );
    }
    assert!(
        plan(
            "shell",
            &json!({"command":"npm test","cwd":"elsewhere"}),
            &root
        )
        .is_err()
    );
}

struct Fake {
    name: &'static str,
    text: &'static str,
}
impl Tool for Fake {
    fn name(&self) -> &str {
        self.name
    }
    fn def(&self) -> crate::agent::club::ToolDef {
        crate::agent::club::ToolDef {
            name: self.name.into(),
            description: String::new(),
            params: json!({}),
        }
    }
    fn call(&self, _: &Value) -> Result<String, String> {
        Ok(self.text.into())
    }
}

#[test]
fn t04b_dispatch_receipts_bind_call_and_output_and_preserve_failures() {
    let _guard = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Fake {
        name: "run_tests",
        text: "tests: 1 passed, 0 failed — reward 1.00\nraw",
    }));
    registry.register(Box::new(Fake {
        name: "shell",
        text: "tests: 9 passed, 0 failed — reward 1.00",
    }));
    let call = ToolCall {
        id: "t04b".into(),
        name: "shell".into(),
        args: json!({"command":"npm test | tail -1"}),
    };
    let text = registry.dispatch(&call.name, &call.args).unwrap();
    let entry = registry.routed_execution(&call, &text).unwrap();
    assert_eq!(entry.outcome.verification, VerificationOutcome::Passed);
    assert_eq!(entry.receipt.routed_call.unwrap().name, "run_tests");
    assert!(
        registry
            .routed_execution(&call, "tests: 999 passed, 0 failed")
            .is_none()
    );
    let fallback = ToolCall {
        args: json!({"command":"npm test | tee result"}),
        ..call.clone()
    };
    let text = registry.dispatch(&fallback.name, &fallback.args).unwrap();
    let entry = registry.routed_execution(&fallback, &text).unwrap();
    assert_eq!(
        entry.outcome.verification,
        VerificationOutcome::Inconclusive
    );
    assert!(entry.receipt.routed_call.is_none());
    assert!(entry.receipt.reason.contains("pipeline"));
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Fake {
        name: "run_tests",
        text: "tests: 0 passed, 1 failed — reward 0.00\nraw",
    }));
    for command in [
        "npm test || true",
        "npm test; echo ok",
        "npm test | tail -1",
    ] {
        let call = ToolCall {
            args: json!({"command":command}),
            ..call.clone()
        };
        let text = registry.dispatch(&call.name, &call.args).unwrap();
        assert_eq!(
            registry.executed_outcome(&call, &text, false).verification,
            VerificationOutcome::Failed
        );
    }
}

#[test]
fn t04b_make_single_recipe_only() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("t04b-make-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for (text, accepted) in [
        ("test:\n\tnpm test\n", true),
        ("test: build\n\tnpm test\n", false),
        ("test:\n\techo hi\n\tnpm test\n", false),
        ("test:\n\t$(RUNNER) test\n", false),
        ("test: build\ntest:\n\tnpm test\n", false),
    ] {
        std::fs::write(root.join("Makefile"), text).unwrap();
        assert_eq!(
            plan("shell", &json!({"command":"make test"}), &root).is_ok(),
            accepted
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn routed_receipt_survives_typed_reads_then_is_consumed_attribution() {
    let _guard = crate::tests::env_lock();
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Fake {
        name: "run_tests",
        text: "tests: 1 passed, 0 failed\nraw",
    }));
    registry.register(Box::new(Fake {
        name: "shell",
        text: "tests: 1 passed, 0 failed\nraw",
    }));
    let call = ToolCall {
        id: "receipt-lifecycle".into(),
        name: "shell".into(),
        args: json!({"command":"npm test | tail -1"}),
    };
    let text = registry.dispatch(&call.name, &call.args).unwrap();

    assert!(registry.take_routed_execution(&call, "different output").is_none());
    assert!(registry.routed_execution(&call, &text).is_some(), "typed verification read must not consume receipt");
    let attributed = registry.take_routed_execution(&call, &text).expect("final attribution consumes receipt");
    assert_eq!(attributed.receipt.routed_call.unwrap().name, "run_tests");
    assert!(registry.routed_execution(&call, &text).is_none(), "completed call must release retained output");
    assert!(registry.take_routed_execution(&call, &text).is_none());
}

const UNITTEST_RED: &str = "tool error: python3 -m unittest discover -v failed (exit 1) — chosen because of 1 python test file(s)
⠥⠋
test_a (t.T.test_a) ... FAIL
test_b (t.T.test_b) ... ok

======================================================================
FAIL: test_a (t.T.test_a)
----------------------------------------------------------------------
Traceback (most recent call last):
  File \"t.py\", line 5, in test_a
    self.assertEqual(score(), 300)
AssertionError: 0 != 300

----------------------------------------------------------------------
Ran 2 tests in 0.001s

FAILED (failures=1)";

#[test]
fn a_red_routed_run_keeps_its_first_failure_when_the_tail_would_hide_it() {
    let root = std::env::temp_dir();
    let route = plan(
        "shell",
        &json!({"command": "python3 -m pytest -q 2>&1 | tail -3"}),
        &root,
    )
    .unwrap();
    let red = route.present(UNITTEST_RED.into(), true);
    assert!(
        red.starts_with("tool error: python3 -m unittest discover -v failed"),
        "{red}"
    );
    assert!(red.contains("FAIL: test_a (t.T.test_a)"), "{red}");
    assert!(red.contains("AssertionError: 0 != 300"), "{red}");
    assert!(
        red.ends_with("Ran 2 tests in 0.001s\n\nFAILED (failures=1)"),
        "{red}"
    );
    // Green: the tail exactly as asked, as before.
    let green = route.present(UNITTEST_RED.into(), false);
    assert_eq!(
        green,
        "tool error: python3 -m unittest discover -v failed (exit 1) — chosen because of 1 python test file(s)\n[typed output tail]\nRan 2 tests in 0.001s\n\nFAILED (failures=1)"
    );
}

#[test]
fn a_long_red_run_shows_its_first_block_then_the_tail_it_asked_for() {
    let root = std::env::temp_dir();
    let route = plan(
        "shell",
        &json!({"command": "cargo test 2>&1 | tail -2"}),
        &root,
    )
    .unwrap();
    let mut text =
        String::from("tool error: cargo test failed (exit 101)\n---- camelcase stdout ----\n");
    text.push_str("thread 'camelcase' panicked at tests/acronym.rs:80:5:\nassertion `left == right` failed\n  left: \"HML\"\n right: \"HTML\"\n");
    for i in 0..60 {
        text.push_str(&format!("noise {i}\n"));
    }
    text.push_str("failures:\n    camelcase\ntest result: FAILED. 9 passed; 1 failed");
    let red = route.present(text, true);
    let block_end = red.find("…[intervening output omitted]").expect(&red);
    assert!(
        red[..block_end].contains("---- camelcase stdout ----"),
        "{red}"
    );
    assert!(red[..block_end].contains("right: \"HTML\""), "{red}");
    assert!(
        red.ends_with(
            "[typed output tail]\n    camelcase\ntest result: FAILED. 9 passed; 1 failed"
        ),
        "{red}"
    );
    assert!(
        red.lines().count() <= 1 + super::FIRST_FAILURE_LINES + 2 + 2,
        "{red}"
    );
}

#[test]
fn a_tail_that_already_shows_a_failure_is_left_as_asked() {
    let root = std::env::temp_dir();
    let route = plan(
        "shell",
        &json!({"command": "pytest -q 2>&1 | tail -2"}),
        &root,
    )
    .unwrap();
    let text = "tool error: python3 -m pytest failed (exit 1)\n___ T.test_a ___\n    assert 0 == 300\nE   AssertionError\nFAILED t.py::T::test_a - AssertionError: 0 != 300\n1 failed, 1 passed in 0.01s";
    assert_eq!(
        route.present(text.into(), true),
        "tool error: python3 -m pytest failed (exit 1)\n[typed output tail]\nFAILED t.py::T::test_a - AssertionError: 0 != 300\n1 failed, 1 passed in 0.01s"
    );
    // No tail asked: the whole result, as before.
    let whole = plan("shell", &json!({"command": "pytest -q"}), &root).unwrap();
    assert_eq!(whole.present(text.into(), true), text);
}
