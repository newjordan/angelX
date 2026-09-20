use super::*;
use crate::harness::{Tool, VerificationOutcome};

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
                .present("tests: 0 passed, 1 failed\nraw\nend".into())
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
    fn def(&self) -> crate::club::ToolDef {
        crate::club::ToolDef {
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
