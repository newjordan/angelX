//! Verify-before-done, final-mile, and completion-evidence gates.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

/// The stop checkpoint raised for edits no verifier has covered.
fn is_unverified_checkpoint(content: &str) -> bool {
    book::ledger::is_warpath_message(content) && has_route(first_line(content), "⠧⠋")
}

/// A stop checkpoint: a warpath ending in `⠟⠁`, facts beside it on later lines.
fn is_stop_checkpoint(content: &str) -> bool {
    book::ledger::is_warpath_message(content) && first_line(content).ends_with("⠟⠁")
}

// --- verification / final-mile suite ---

#[test]
fn remote_verifiers_require_an_executed_check_not_benchmark_vocabulary() {
    for (command, expected) in [
        ("ssh worker 'cargo test'", true),
        ("ssh -p 2222 worker 'python3 -m pytest'", true),
        ("ssh worker 'cargo test || true'", false),
        ("ssh worker 'echo cargo test'", false),
        ("cat results.golden.json", false),
        ("echo benchmark", false),
        ("rg benchmark README.md", false),
        ("ssh worker 'cat results.golden.json'", false),
    ] {
        let call = ToolCall {
            id: "remote".into(),
            name: "shell".into(),
            args: serde_json::json!({"command":command}),
        };
        assert_eq!(is_verification_call(&call), expected, "{command}");
    }
}

#[test]
fn verify_before_done_classifies_mutations_and_real_verifiers() {
    let call = |name: &str, args: Value| ToolCall {
        id: "call".into(),
        name: name.into(),
        args,
    };

    assert!(is_mutation_call(&call(
        "write_file",
        serde_json::json!({"path":"src/lib.rs","content":""})
    )));
    assert!(is_mutation_call(&call(
        "fmt",
        serde_json::json!({"check":false})
    )));
    assert!(!is_mutation_call(&call(
        "fmt",
        serde_json::json!({"check":true})
    )));
    assert!(!is_mutation_call(&call(
        "code_mode",
        serde_json::json!({"script":"return grep({pattern:'x'});"})
    )));
    assert!(is_mutation_call(&call(
        "code_mode",
        serde_json::json!({"script":"return shell({command:'true'});","allow_effects":true})
    )));

    let prose = call(
        "write_file",
        serde_json::json!({"path":"README.md","content":"guide"}),
    );
    let source = call(
        "write_file",
        serde_json::json!({"path":"src/lib.rs","content":"pub fn x() {}"}),
    );
    let unknown = call("integrate", serde_json::json!({}));
    assert!(is_prose_only_path("docs/guide.MDX"));
    assert!(is_prose_only_path("LICENSE"));
    assert!(
        is_prose_only_path("ReadMe"),
        "mixed-case extensionless prose still matches without a lowercase copy"
    );
    assert!(is_prose_only_path("CHANGELOG"));
    assert!(!is_prose_only_path("Cargo.toml"));
    assert!(!mutation_requires_verification(&prose));
    assert!(mutation_requires_verification(&source));
    assert!(
        mutation_requires_verification(&unknown),
        "opaque/directly untyped mutations stay conservative"
    );

    for verifier in [
        call("run_tests", serde_json::json!({})),
        call("check", serde_json::json!({})),
        call("lint", serde_json::json!({})),
        call("cargo", serde_json::json!({"args":"test --all-targets"})),
        call("shell", serde_json::json!({"command":"npm test"})),
        call("shell", serde_json::json!({"command":"uv run pytest -q"})),
        call(
            "shell",
            serde_json::json!({"command":"cd cockpit && cargo   test 2>&1 | tail -20"}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"GOTOOLCHAIN=auto go build ./..."}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"dotnet test src/App.Tests/App.Tests.csproj"}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"./gradlew :module:test"}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"printf 'prep\\n'; cargo test"}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"cargo test && printf 'done\\n'"}),
        ),
        call("shell", serde_json::json!({"command":"cargo test;   "})),
        call("shell", serde_json::json!({"command":"CARGO TEST"})),
        call("shell", serde_json::json!({"command":"Npm Test"})),
        call("shell", serde_json::json!({"command":"UV RUN PYTEST -q"})),
    ] {
        assert!(is_verification_call(&verifier), "{}", verifier.name);
    }
    assert!(!is_verification_call(&call(
        "shell",
        serde_json::json!({"command":"sed -n '1,40p' src/lib.rs"})
    )));
    assert!(!is_verification_call(&call(
        "shell",
        serde_json::json!({"command":"echo cargo test"})
    )));
    assert!(
        !is_verification_call(&call(
            "shell",
            serde_json::json!({"command":"echo CARGO TEST"})
        )),
        "quoted/echoed mixed-case argv is still not a verifier"
    );
    assert!(!is_verification_call(&call(
        "shell",
        serde_json::json!({"command":"true || cargo test"})
    )));
    for masked in [
        "cargo test; printf 'done\\n'",
        "cargo test\nprintf 'done\\n'",
        "cargo test & printf 'launched\\n'",
        "cargo test || true",
        "printf '; cargo test'",
        "printf \"\\n cargo test\"",
        "printf 'cargo test | tail -1'",
        "printf 'harmless | cargo test'",
        "printf 'harmless & cargo test'",
        "cargo test '",
        "true $(false; cargo test )",
        "true \"$(false; cargo test)\"",
        "true `false; cargo test `",
        "true > >(false; cargo test )",
        "cargo(){ true; }; cargo test",
        "function cargo { true; }; cargo test",
        "cat <<'EOF'\n; cargo test\nEOF",
        "true # ignored; cargo test",
    ] {
        assert!(
            !is_verification_call(&call("shell", serde_json::json!({"command": masked}))),
            "status-masked verifier must not count: {masked}"
        );
    }

    let run_tests = call(
        "run_tests",
        serde_json::json!({"args":"--all-targets   -- --test-threads=1"}),
    );
    for sufficient in [run_tests, call("check", serde_json::json!({}))] {
        assert!(
            verification_is_completion_sufficient(&sufficient),
            "{}",
            sufficient.name
        );
    }
    for supplemental in [
        call("lint", serde_json::json!({})),
        call("fmt", serde_json::json!({"check":true})),
        call(
            "shell",
            serde_json::json!({"command":"GOTOOLCHAIN=auto go test ./execution/rerun/..."}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"JAVA_HOME=/jdk mvn -DskipTests package"}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"dotnet build src/App/App.csproj"}),
        ),
        call(
            "shell",
            serde_json::json!({"command":"GOTOOLCHAIN=auto go vet ./execution/rerun/..."}),
        ),
    ] {
        assert!(!verification_is_completion_sufficient(&supplemental));
    }
}

#[test]
fn verifier_outcomes_do_not_confuse_dispatch_with_green_evidence() {
    let call = |name: &str, args: Value| ToolCall {
        id: "verify".into(),
        name: name.into(),
        args,
    };
    let tests = call("run_tests", serde_json::json!({}));
    assert_eq!(
        verification_outcome(
            &tests,
            "tests: 12 passed, 0 failed, 1 ignored — reward 1.00"
        ),
        Some(VerificationOutcome::Passed)
    );
    assert_eq!(
        verification_outcome(
            &tests,
            "tests: 11 passed, 1 failed, 0 ignored — reward 0.92"
        ),
        Some(VerificationOutcome::Failed)
    );
    assert_eq!(
        verification_outcome(&tests, "test runner returned an unfamiliar report"),
        Some(VerificationOutcome::Inconclusive)
    );

    let check = call("check", serde_json::json!({}));
    assert_eq!(
        verification_outcome(&check, "check: 2 warnings, 0 errors — reward 0.98"),
        Some(VerificationOutcome::Passed)
    );
    assert_eq!(
        verification_outcome(&check, "check: 0 warnings, 3 errors — reward 0.00"),
        Some(VerificationOutcome::Failed)
    );
    assert_eq!(
        verification_outcome(&check, "tool error: dependency unavailable"),
        Some(VerificationOutcome::Failed)
    );
    assert_eq!(
        verification_outcome(&check, "action capsule denied by policy"),
        None
    );

    let cargo_check = call("cargo", serde_json::json!({"args":"check --quiet"}));
    assert_eq!(
        verification_outcome(&cargo_check, "[cargo verdict: pass]"),
        Some(VerificationOutcome::Passed)
    );
    assert_eq!(
        verification_outcome(&cargo_check, "Error: Could Not Compile foo"),
        Some(VerificationOutcome::Failed),
        "mixed-case compile fail still classifies without a lowercase copy"
    );
    assert_eq!(
        verification_outcome(&cargo_check, "Finished `dev` profile [unoptimized]"),
        Some(VerificationOutcome::Passed)
    );
    let fmt = call("fmt", serde_json::json!({"check": true}));
    assert_eq!(
        verification_outcome(&fmt, "Already rustfmt-clean"),
        Some(VerificationOutcome::Passed)
    );
    assert_eq!(
        verification_outcome(&fmt, "Needs Formatting"),
        Some(VerificationOutcome::Failed)
    );
    let cargo_test = call("cargo", serde_json::json!({"args":"test --quiet"}));
    assert_eq!(
        verification_outcome(&cargo_test, "[cargo verdict: pass]"),
        Some(VerificationOutcome::Inconclusive),
        "a successful runner process with zero observed tests is not green test evidence"
    );

    for command in [
        "cargo test",
        "npm test",
        "pytest -q",
        "PATH=./test-shim:$PATH cargo test",
        "BASH_ENV=./test-env cargo test",
        "/tmp/test-shim/cargo test",
    ] {
        let shell = call("shell", serde_json::json!({"command": command}));
        assert!(
            is_verification_call(&shell),
            "raw {command} remains a verification attempt"
        );
        assert_eq!(
            verification_outcome(&shell, "all checks passed"),
            Some(VerificationOutcome::Inconclusive),
            "an untyped shell success cannot prove which executable ran: {command}"
        );
        assert!(
            !verification_is_completion_sufficient(&shell),
            "raw shell cannot arm durable green: {command}"
        );
    }
    let failing_shell = call("shell", serde_json::json!({"command": "npm test"}));
    assert_eq!(
        verification_outcome(&failing_shell, "tool error: shell command failed (exit 1)"),
        Some(VerificationOutcome::Failed),
        "a definite raw-shell failure remains useful red evidence"
    );
}

/// A JavaScript test runner called directly is a test run, as `npm test` is.
/// polyglot-v1 js-forth: DeepSeek ran `npx jest` about 30 times, and none of
/// its red runs reached the verifier state.
#[test]
fn direct_js_test_runners_are_verification_calls() {
    let shell = |command: &str| ToolCall {
        id: String::new(),
        name: "shell".into(),
        args: serde_json::json!({"command": command}),
    };
    for command in [
        "npx jest forth.spec.js 2>&1 | tail -30",
        "cd ws && npx jest forth.spec.js 2>&1 | sed -n '1,80p'",
        "npx --yes vitest run",
        "bunx vitest run src",
        "pnpm exec mocha test/",
        "pnpm vitest run",
        "yarn jest --ci",
        "npm exec -- jest",
        "./node_modules/.bin/jest -i",
        "CI=true jest",
        "mocha",
    ] {
        assert!(is_verification_call(&shell(command)), "{command}");
        assert_eq!(
            verification_outcome(&shell(command), "tool error: shell command failed (exit 1)"),
            Some(VerificationOutcome::Failed),
            "{command}"
        );
        assert!(
            !verification_is_completion_sufficient(&shell(command)),
            "a raw shell run is never durable green: {command}"
        );
    }
    for command in [
        "npx prettier --check .",
        "npx tsc --noEmit",
        "cat jest.config.js",
        "grep -n jest package.json",
        "echo jest",
        "npm install jest",
        "yarn add -D vitest",
    ] {
        assert!(!is_verification_call(&shell(command)), "{command}");
    }
}

#[test]
fn execution_events_do_not_classify_successful_output_as_cancellation_or_panic() {
    let call = ToolCall {
        id: "evaluator".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "angel-evaluate full"}),
    };
    for output in [
        "{\"status\":\"completed\",\"score\":0.843792}\n[stderr] Evaluator request id; if interrupted, inspect its status",
        "test cancelled_worker_is_recovered ... ok\ntest result: ok. 1 passed; 0 failed;",
        "0 canceled, 0 interrupted, no worker panicked",
        "example output: tool panicked, then recovered",
    ] {
        let outcome = turn_event_outcome(&call, output, false);
        assert_eq!(outcome.execution, ExecutionOutcome::Succeeded, "{output}");
        assert_ne!(outcome.verification, VerificationOutcome::Passed);
    }
    assert_eq!(
        turn_event_outcome(
            &call,
            "tool error: shell command failed (exit 1)\n0 cancelled",
            false
        )
        .execution,
        ExecutionOutcome::Failed
    );
    for error in [
        "tool error: shell command cancelled by operator\npartial output",
        "tool error: execution cancelled before spawn",
        "Cancelled by the operator",
    ] {
        assert_eq!(
            turn_event_outcome(&call, error, false).execution,
            ExecutionOutcome::Cancelled
        );
    }
    assert_eq!(
        turn_event_outcome(&call, "tool error: shell panicked: payload", false).execution,
        ExecutionOutcome::Panicked
    );
    assert_eq!(
        turn_event_outcome(&call, "Worker Panicked while applying", false).execution,
        ExecutionOutcome::Panicked
    );
}

#[test]
fn structured_kill_execution_events_distinguish_cancellation_from_failure() {
    let call = ToolCall {
        id: "killed-shell".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "sleep 8"}),
    };
    for reason in ["cancelled", "tool_idle", "deadline", "signal_death"] {
        let kill =
            crate::agent::sandbox::process_owner::KillReceipt::new(Some(15), reason, "harness");
        let error = format!("tool error: {}", kill.error("shell stopped"));
        let outcome = turn_event_outcome(&call, &error, false);
        let expected = if reason == "cancelled" {
            ExecutionOutcome::Cancelled
        } else {
            ExecutionOutcome::Failed
        };
        assert_eq!(outcome.execution, expected, "{reason}");
        assert_eq!(
            outcome.verification,
            VerificationOutcome::Inconclusive,
            "{reason}"
        );
        // Recoverable tool-idle escalation is a failed tool, not a turn cancel.
        let recoverable = format!("tool error: tool_idle: retry with explicit input\n{error}");
        assert_eq!(
            turn_event_outcome(&call, &recoverable, false).execution,
            ExecutionOutcome::Failed
        );
        // Successful child output that quotes a receipt cannot claim a kill.
        assert_eq!(
            turn_event_outcome(&call, &kill.error("quoted output"), false).execution,
            ExecutionOutcome::Succeeded
        );
    }
}

#[test]
fn execution_outcomes_have_stable_snake_case_projections() {
    assert_eq!(ExecutionOutcome::Succeeded.as_str(), "succeeded");
    assert_eq!(ExecutionOutcome::NotStarted.as_str(), "not_started");
    assert_eq!(ExecutionOutcome::Failed.as_str(), "failed");
    assert_eq!(ExecutionOutcome::Denied.as_str(), "denied");
    assert_eq!(ExecutionOutcome::Cancelled.as_str(), "cancelled");
    assert_eq!(ExecutionOutcome::Panicked.as_str(), "panicked");
}

#[test]
fn raw_shell_verifier_is_not_reused_or_completion_skipped() {
    let _env_guard = crate::tests::env_lock();
    // Counts every verifier execution; completion confirmation would add runs.
    let _confirm = EnvGuard::set("ANGEL_CONFIRM_GREEN_RUNS", "0");
    let root = scratch("raw_shell_verifier_not_reused");
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    std::fs::write(root.join("package.json"), r#"{"scripts":{"test":"true"}}"#).unwrap();
    std::fs::create_dir_all(root.join("shim")).unwrap();
    let shim = root.join("shim/npm");
    std::fs::write(&shim, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&shim).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&shim, permissions).unwrap();
    }
    assert!(
        Command::new("git")
            .args(["add", ".gitignore", "package.json"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args([
                "-c",
                "user.name=Angel Test",
                "-c",
                "user.email=angel@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );

    struct VerifyTwiceThenEdit {
        step: AtomicUsize,
    }
    impl Club for VerifyTwiceThenEdit {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "raw-shell-verifier-not-reused"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            let step = self.step.fetch_add(1, Ordering::Relaxed);
            Ok(match step {
                0 | 1 | 4 => ClubReply::Calls(vec![ToolCall {
                    id: format!("verify-{step}"),
                    name: "shell".into(),
                    args: serde_json::json!({
                        "command":"PATH=./shim:$PATH mkdir -p target; printf x >> target/verify-count; PATH=./shim:$PATH npm test"
                    }),
                }]),
                2 => ClubReply::Calls(vec![ToolCall {
                    id: "overlapping-verifier".into(),
                    name: "shell".into(),
                    args: serde_json::json!({
                        "command":"PATH=./shim:$PATH mkdir -p target; printf y >> target/verify-count; PATH=./shim:$PATH npm run test"
                    }),
                }]),
                3 => ClubReply::Calls(vec![ToolCall {
                    id: "edit".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({"path":"src.txt","content":"changed\n"}),
                }]),
                _ => ClubReply::Text("verified changed workspace".into()),
            })
        }
    }

    let club = VerifyTwiceThenEdit {
        step: AtomicUsize::new(0),
    };
    let registry = ToolRegistry::with_team(root.clone(), Vec::new());
    let mut history = vec![ChatMsg::user("verify, edit, and verify again")];
    let (event_tx, event_rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &event_tx,
    )
    .unwrap();

    assert_eq!(answer, "verified changed workspace");
    assert_eq!(
        std::fs::read_to_string(root.join("target/verify-count")).unwrap(),
        "xxyx",
        "every raw-shell verifier must execute because its success is not cacheable green evidence"
    );
    assert!(
        !history
            .iter()
            .any(|message| message.content.contains("[reused verifier result:"))
    );
    assert!(
        !history
            .iter()
            .any(|message| message.content.contains("[skipped redundant verifier:"))
    );
    let events = event_rx.try_iter().collect::<Vec<_>>();
    for dispatched_id in ["verify-0", "verify-1", "overlapping-verifier", "verify-4"] {
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    TurnEvent::ToolCall { id, .. } if id.0 == dispatched_id
                ))
                .count(),
            1,
            "the authored {dispatched_id} call must remain paired"
        );
        let outcome = events.iter().find_map(|event| match event {
            TurnEvent::ToolResult { id, outcome, .. } if id.0 == dispatched_id => Some(outcome),
            _ => None,
        });
        assert_eq!(
            outcome,
            Some(&ToolOutcome {
                execution: ExecutionOutcome::Succeeded,
                verification: VerificationOutcome::Inconclusive,
            })
        );
    }
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn turn_deadline_cancel_is_local_and_does_not_stall_during_unwind() {
    let operator_cancel = AtomicBool::new(false);
    let observed =
        with_turn_deadline_cancel(&operator_cancel, Some(Instant::now()), |effective_cancel| {
            effective_cancel.load(Ordering::Acquire)
        });
    assert!(observed);
    assert!(!operator_cancel.load(Ordering::Acquire));

    let started = Instant::now();
    let panic = std::panic::catch_unwind(|| {
        with_turn_deadline_cancel(
            &operator_cancel,
            Instant::now().checked_add(Duration::from_secs(60)),
            |_effective_cancel| panic!("deadline operation fixture"),
        )
    });
    assert!(panic.is_err());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "watchdog must be unparked while the operation unwinds"
    );
    assert!(!operator_cancel.load(Ordering::Acquire));
}

#[test]
fn edited_task_turn_gets_one_stop_checkpoint() {
    let _env_guard = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");

    struct IntegrateStub;
    impl Tool for IntegrateStub {
        fn name(&self) -> &str {
            "integrate"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "test mutation".into(),
                params: serde_json::json!({"type":"object","properties":{}}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            Ok("integrated test branch".into())
        }
    }

    struct AnswersAfterEdit {
        step: AtomicUsize,
    }
    impl Club for AnswersAfterEdit {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "verify-nudge-test"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(match self.step.fetch_add(1, Ordering::Relaxed) {
                0 => ClubReply::Calls(vec![ToolCall {
                    id: "edit".into(),
                    name: "integrate".into(),
                    args: serde_json::json!({}),
                }]),
                1 => ClubReply::Text("done without proof".into()),
                _ => ClubReply::Text("done with blocker disclosed".into()),
            })
        }
    }

    let club = AnswersAfterEdit {
        step: AtomicUsize::new(0),
    };
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(IntegrateStub));
    let mut history = vec![ChatMsg::user("change it")];
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(5),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();

    // One checkpoint carrying the fact and the deli route; the next answer
    // stands.
    assert_eq!(answer, "done with blocker disclosed");
    assert_eq!(club.step.load(Ordering::Relaxed), 3);
    let checkpoints = history
        .iter()
        .filter(|message| is_unverified_checkpoint(&message.content))
        .collect::<Vec<_>>();
    assert_eq!(checkpoints.len(), 1, "{history:?}");
    assert!(first_line(&checkpoints[0].content).ends_with("⠟⠁"));
}

#[test]
fn direct_prose_only_edit_does_not_spend_a_verification_hop() {
    let _env_guard = crate::tests::env_lock();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let root = scratch("prose_only_completion");
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );

    struct WriteReadmeThenAnswer {
        step: AtomicUsize,
    }
    impl Club for WriteReadmeThenAnswer {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "prose-only-completion"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(if self.step.fetch_add(1, Ordering::Relaxed) == 0 {
                ClubReply::Calls(vec![ToolCall {
                    id: "docs".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({
                        "path": "README.md",
                        "content": "# Operator guide\n"
                    }),
                }])
            } else {
                ClubReply::Text("documentation updated".into())
            })
        }
    }

    let club = WriteReadmeThenAnswer {
        step: AtomicUsize::new(0),
    };
    let registry = ToolRegistry::with_team(root.clone(), Vec::new());
    let mut history = vec![ChatMsg::user("update the readme")];
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();

    assert_eq!(answer, "documentation updated");
    assert_eq!(club.step.load(Ordering::Relaxed), 2);
    assert!(root.join("README.md").exists());
    assert!(
        history
            .iter()
            .all(|message| !is_stop_checkpoint(&message.content))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn red_verifier_allows_blocker_report_without_an_extra_model_hop() {
    let _env_guard = crate::tests::env_lock();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");

    struct StubTool {
        name: &'static str,
        result: Result<&'static str, &'static str>,
    }
    impl Tool for StubTool {
        fn name(&self) -> &str {
            self.name
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name.into(),
                description: "verification-state fixture".into(),
                params: serde_json::json!({"type":"object","properties":{}}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            self.result.map(str::to_string).map_err(str::to_string)
        }
    }

    struct EditCheckBlocker {
        step: AtomicUsize,
    }
    impl Club for EditCheckBlocker {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "red-verifier-completion"
        }
        fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(match self.step.fetch_add(1, Ordering::Relaxed) {
                0 => ClubReply::Calls(vec![ToolCall {
                    id: "edit".into(),
                    name: "integrate".into(),
                    args: serde_json::json!({}),
                }]),
                1 => ClubReply::Calls(vec![ToolCall {
                    id: "red-check".into(),
                    name: "check".into(),
                    args: serde_json::json!({}),
                }]),
                _ => ClubReply::Text(
                    "check failed: dependency unavailable; change is unverified".into(),
                ),
            })
        }
    }

    let club = EditCheckBlocker {
        step: AtomicUsize::new(0),
    };
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(StubTool {
        name: "integrate",
        result: Ok("integrated"),
    }));
    registry.register(Box::new(StubTool {
        name: "check",
        result: Err("dependency unavailable"),
    }));
    let mut history = vec![ChatMsg::user("make the change and verify it")];
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();

    assert!(answer.contains("unverified"));
    assert_eq!(club.step.load(Ordering::Relaxed), 3);
    assert!(
        history
            .iter()
            .all(|message| !is_stop_checkpoint(&message.content))
    );
}

#[test]
fn opaque_shell_edit_requires_fresh_workspace_verification() {
    let _env_guard = crate::tests::env_lock();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let root = scratch("opaque_shell_verify");
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );

    struct ShellThenVerify {
        step: AtomicUsize,
        saw_nudge: AtomicBool,
    }
    impl Club for ShellThenVerify {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "opaque-shell-verify"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(match self.step.fetch_add(1, Ordering::Relaxed) {
                0 => ClubReply::Calls(vec![ToolCall {
                    id: "opaque-edit".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"command":"printf opaque > opaque.txt"}),
                }]),
                1 => ClubReply::Text("done without proof".into()),
                2 => {
                    self.saw_nudge.store(
                        messages
                            .iter()
                            .any(|message| is_unverified_checkpoint(&message.content)),
                        Ordering::Relaxed,
                    );
                    ClubReply::Calls(vec![ToolCall {
                        id: "verify".into(),
                        name: "check".into(),
                        args: serde_json::json!({}),
                    }])
                }
                _ => ClubReply::Text("done after verification attempt".into()),
            })
        }
    }

    let club = ShellThenVerify {
        step: AtomicUsize::new(0),
        saw_nudge: AtomicBool::new(false),
    };
    let registry = ToolRegistry::with_team(root.clone(), Vec::new());
    let mut history = vec![ChatMsg::user("make an opaque shell edit")];
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &mpsc::channel::<TurnEvent>().0,
    )
    .unwrap();

    assert_eq!(answer, "done after verification attempt");
    assert!(club.saw_nudge.load(Ordering::Relaxed));
    assert_eq!(club.step.load(Ordering::Relaxed), 4);
    assert_eq!(
        history
            .iter()
            .filter(|message| is_unverified_checkpoint(&message.content))
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A PATH-shimmed shell green is a test attempt as far as the model's facts
/// go (no `⠧⠋`), but it never earns a typed pass: that stays with the
/// curated check.
#[test]
fn path_shim_shell_green_is_an_attempt_but_not_a_typed_pass() {
    use std::os::unix::fs::PermissionsExt;

    let _env_guard = crate::tests::env_lock();
    let _experience = EnvGuard::set("ANGEL_EXPERIENCE", "0");
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let root = scratch("path_shim_shell_green");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("shim")).unwrap();
    std::fs::write(root.join("src/lib.py"), "def value():\n    return 1\n").unwrap();
    let runtimes = crate::agent::tools::build::PinnedNativeRuntimes::capture(&root);
    if !runtimes.python_available_for_test() {
        eprintln!(
            "UNSUPPORTED: PATH-shim typed-verification fixture requires a trusted Python runtime"
        );
        return;
    }
    let shim = root.join("shim/npm");
    std::fs::write(&shim, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&shim).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&shim, permissions).unwrap();

    struct EditShimDoneThenCheck {
        step: AtomicUsize,
        saw_nudge: AtomicBool,
    }
    impl Club for EditShimDoneThenCheck {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "path-shim-shell-green"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(match self.step.fetch_add(1, Ordering::Relaxed) {
                0 => ClubReply::Calls(vec![ToolCall {
                    id: "edit".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({
                        "path": "src/lib.py",
                        "content": "def value():\n    return 2\n"
                    }),
                }]),
                1 => ClubReply::Calls(vec![ToolCall {
                    id: "shim-green".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"command": "PATH=./shim:$PATH npm test"}),
                }]),
                2 => {
                    self.saw_nudge.store(
                        messages
                            .iter()
                            .filter(|message| message.role != ChatRole::System)
                            .any(|message| {
                                has_route(first_line(&message.content), "⠧⠋")
                                    || has_route(first_line(&message.content), "⠧⠁")
                            }),
                        Ordering::Relaxed,
                    );
                    ClubReply::Calls(vec![ToolCall {
                        id: "curated-check".into(),
                        name: "check".into(),
                        args: serde_json::json!({"runtime":"python", "args":"src/lib.py"}),
                    }])
                }
                _ => ClubReply::Text("done after curated verification".into()),
            })
        }
    }

    let club = EditShimDoneThenCheck {
        step: AtomicUsize::new(0),
        saw_nudge: AtomicBool::new(false),
    };
    let registry = ToolRegistry::with_team(root.clone(), Vec::new());
    let mut history = vec![ChatMsg::user("edit the Python file and verify it")];
    let (event_tx, event_rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &event_tx,
    )
    .unwrap();

    assert_eq!(answer, "done after curated verification");
    assert!(!club.saw_nudge.load(Ordering::Relaxed));
    assert_eq!(club.step.load(Ordering::Relaxed), 4);
    // No untested or red route ever showed; the clean run after the edit
    // showed the finish cue instead.
    assert!(
        !history
            .iter()
            .filter(|message| message.role != ChatRole::System)
            .any(|message| has_route(first_line(&message.content), "⠧⠋")
                || has_route(first_line(&message.content), "⠧⠁"))
    );
    assert!(
        history
            .iter()
            .any(|message| message.role == ChatRole::Tool && has_route(&message.content, "⠺⠓"))
    );
    let events = event_rx.try_iter().collect::<Vec<_>>();
    let verification = |wanted: &str| {
        events.iter().find_map(|event| match event {
            TurnEvent::ToolResult { id, outcome, .. } if id.0 == wanted => {
                Some(outcome.verification)
            }
            _ => None,
        })
    };
    assert_eq!(
        verification("shim-green"),
        Some(VerificationOutcome::Inconclusive)
    );
    assert_eq!(
        verification("curated-check"),
        Some(VerificationOutcome::Passed),
        "curated check result: {:?}",
        history
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some("curated-check"))
    );
    let dispatched_receipt = history
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("curated-check"))
        .and_then(|message| message.tool_receipt.as_ref())
        .expect("the real typed dispatch must retain its verification receipt");
    assert_eq!(dispatched_receipt.call.name, "check");
    assert_eq!(
        dispatched_receipt.outcome.execution,
        ExecutionOutcome::Succeeded
    );
    assert_eq!(
        dispatched_receipt.outcome.verification,
        VerificationOutcome::Passed
    );
    let shell_receipt = history
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("shim-green"))
        .and_then(|message| message.tool_receipt.as_ref())
        .expect("shell dispatch retains execution evidence");
    assert_eq!(shell_receipt.call.name, "shell");
    assert_eq!(shell_receipt.outcome.execution, ExecutionOutcome::Succeeded);
    assert_eq!(
        shell_receipt.outcome.verification,
        VerificationOutcome::Inconclusive
    );
    let routing = shell_receipt
        .routing
        .as_ref()
        .expect("shell dispatch records its routing decision");
    assert!(routing.routed_call.is_none(), "PATH shim must not route");
    assert!(!routing.reason.is_empty(), "unroutable shell explains why");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn self_authored_verification_detects_named_test_files() {
    let mut authored = std::collections::HashSet::new();
    authored.insert("model_test.go".into());
    let weak = ToolCall {
        id: "t".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "go test -run TestBLP ./model_test.go"}),
    };
    let strong = ToolCall {
        id: "t2".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "go test ./..."}),
    };
    assert!(verification_targets_self_authored(&weak, &authored));
    assert!(!verification_targets_self_authored(&strong, &authored));
    let mixed = ToolCall {
        id: "t3".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "GO TEST -run TestBLP ./model_test.go"}),
    };
    assert!(
        verification_targets_self_authored(&mixed, &authored),
        "mixed-case argv still matches the authored basename without a lowercase copy"
    );
}

#[test]
fn dependency_mutation_shells_count_as_first_write_progress() {
    assert!(dependency_mutation_command(
        "go get github.com/tdewolff/minify/v2@v2.24.11"
    ));
    assert!(dependency_mutation_command("go mod tidy"));
    assert!(dependency_mutation_command("npm install lodash@4.17.21"));
    assert!(dependency_mutation_command("cargo add serde"));
    assert!(
        dependency_mutation_command("NPM INSTALL lodash@4.17.21"),
        "mixed-case argv still counts as a lockfile mutation"
    );
    assert!(dependency_mutation_command("Cargo Add serde"));
    assert!(dependency_mutation_command("Go Mod Tidy"));
    assert!(!dependency_mutation_command("go test ./..."));
    assert!(!dependency_mutation_command("ls -la"));
    assert!(!dependency_mutation_command("echo npm install"));
    let call = ToolCall {
        id: "s".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "GO GET example.com/lib@v1.2.3"}),
    };
    assert!(is_dependency_mutation_call(&call));
    assert!(is_product_mutation_call(&call));
    assert!(!is_mutation_call(&call));
}

// --- B5: banned self-repair priming nouns never return to live prompts ---

#[test]
fn live_prompts_carry_no_self_repair_priming() {
    // Regression pin. The two strings that reach the model every session — the
    // orchestrator posture paragraph and the self-model block — must never re-
    // introduce the self-repair priming nouns. Naming the forbidden artifact is
    // exactly what hands a degraded model a searchable token to chase (the
    // 2026-07-04 MOA incident). build_system_prompt just concatenates these two
    // with skills/project/work blocks (which never carried the nouns), and it
    // needs a Bag + filesystem I/O, so pinning the two sources is the live cover.
    // The orchestrator's words are its personality and workflow chapters.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let orch = ["⠽⠃", "⠽⠙", "⠽⠑"]
        .map(|cells| crate::agent::harness::book::ledger::read(root, cells).unwrap())
        .join("\n");
    let selfmodel =
        crate::agent::tools::self_model::self_context(Path::new(env!("CARGO_MANIFEST_DIR")));
    let conductor_notice = crate::drive::conductor::startup_notice_text(2);
    let conductor_root = crate::agent::tools::self_model::source_root().unwrap();
    let conductor_usage = crate::drive::conductor::run(Some("bogus"), &conductor_root);
    let still_usage = crate::knowledge::barrel::run(Some("bogus"));
    let banned = ["gauntlet", "health checkup", "repair mode", "fix yourself"];
    for (name, text) in [
        ("orchestrator", orch.as_str()),
        ("self-model", selfmodel.as_str()),
        ("conductor notice", conductor_notice.as_str()),
        ("conductor usage", conductor_usage.as_str()),
        ("still usage", still_usage.as_str()),
    ] {
        let lower = text.to_ascii_lowercase();
        for b in banned {
            assert!(
                !lower.contains(b),
                "{name} prompt must not contain the priming noun '{b}':\n{text}"
            );
        }
    }
    // NOTE: `tool_repair` is a legitimate registered tool name and is intentionally
    // NOT asserted against here; the swarm incident fixture is left untouched.
}

#[test]
fn filtered_test_runs_are_not_completion_sufficient() {
    let call = |name: &str, args: serde_json::Value| ToolCall {
        id: "c1".into(),
        name: name.into(),
        args,
    };
    // Full test runs qualify.
    for args in ["test", "test --quiet"] {
        assert!(
            verification_is_completion_sufficient(&call(
                "cargo",
                serde_json::json!({"args": args})
            )),
            "cargo {args} must be completion-sufficient"
        );
    }
    for args in ["", "--quiet"] {
        assert!(
            verification_is_completion_sufficient(&call(
                "run_tests",
                serde_json::json!({"args": args})
            )),
            "run_tests {args:?} must be completion-sufficient"
        );
    }
    // A raw compile probe proves the tree builds, not that the task is done —
    // a check-green arming the wrap-up guard cut run11 off mid-ladder. (The
    // dedicated `check` TOOL keeps its curated-gate contract; raw cargo does
    // not.)
    for args in ["check", "check --quiet"] {
        assert!(
            !verification_is_completion_sufficient(&call(
                "cargo",
                serde_json::json!({"args": args})
            )),
            "cargo {args} must NOT be completion-sufficient"
        );
    }
    // Shell parsing still recognizes these as verification attempts, but a
    // zero shell status cannot identify the executable behind `cargo`.
    assert!(!verification_is_completion_sufficient(&call(
        "shell",
        serde_json::json!({"command": "cargo test"})
    )));
    assert!(!verification_is_completion_sufficient(&call(
        "shell",
        serde_json::json!({"command": "printf prep; cargo test && printf done"})
    )));
    assert!(!verification_is_completion_sufficient(&call(
        "shell",
        serde_json::json!({"command": "cd cockpit && cargo check"})
    )));
    for masked in [
        "cargo test; printf done",
        "cargo test\nprintf done",
        "cargo test & printf launched",
        "cargo test || true",
    ] {
        assert!(
            !verification_is_completion_sufficient(&call(
                "shell",
                serde_json::json!({"command": masked})
            )),
            "status-masked full suite must not arm completion: {masked}"
        );
    }
    // Filtered slices, libtest filters, and --ignored runs are targeted
    // evidence — they must neither arm nor be skipped by the post-green guard
    // (run3-sol lost its fixture gate, full suite, and commit to a filtered
    // green arming the guillotine).
    for args in [
        "test raycast",
        "test --quiet raycast",
        "test -- moonlit",
        "test dump_ride_frames_for_review -- --ignored",
        "test -- --ignored",
    ] {
        assert!(
            !verification_is_completion_sufficient(&call(
                "cargo",
                serde_json::json!({"args": args})
            )),
            "cargo {args} must NOT be completion-sufficient"
        );
    }
    for args in [
        "raycast",
        "--test integration",
        "-p one-package",
        "--package one-package",
        "--lib",
        "--bins",
        "--doc",
        "--no-run",
        "-- --ignored",
    ] {
        assert!(
            !verification_is_completion_sufficient(&call(
                "run_tests",
                serde_json::json!({"args": args})
            )),
            "run_tests {args} must NOT be completion-sufficient"
        );
    }
    assert!(!verification_is_completion_sufficient(&call(
        "shell",
        serde_json::json!({"command": "cargo test raycast"})
    )));
    assert!(!verification_is_completion_sufficient(&call(
        "shell",
        serde_json::json!({"command": "cd cockpit && cargo test wide_view -- --nocapture"})
    )));
}

#[test]
fn baseline_green_verifier_is_not_completion_evidence_without_a_mutation() {
    use super::{VerificationOutcome, turn::is_completion_green};

    assert!(!is_completion_green(VerificationOutcome::Passed, false));
    assert!(is_completion_green(VerificationOutcome::Passed, true));
    assert!(!is_completion_green(VerificationOutcome::Failed, true));
    assert!(!is_completion_green(
        VerificationOutcome::Inconclusive,
        true
    ));
}

/// Every verifier call the model makes executes: the harness never answers a
/// verifier from cache or skips it as redundant.
fn native_verifier_dispatch_scenario(
    calls: Vec<(&'static str, Value)>,
    expected: usize,
    behavior_red: bool,
) {
    let _guard = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-mcp.json");
    let root = scratch("native_verifier_dispatch");
    let output = scratch("native_verifier_output");
    // Resolve the shared test helper before isolating the fixture Cargo target.
    // Otherwise helper bootstrap would rebuild the cockpit inside this target.
    crate::agent::sandbox::prime_helper();
    let _target = EnvGuard::set("CARGO_TARGET_DIR", output.to_str().unwrap());
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    // `extra.rs` is a real bin target so a test that rewrites it exercises a
    // compiled path: a check that never compiled the changed file is
    // inconclusive by design (tools/build/targets.rs), not green.
    let manifest = "[package]\nname = \"owned_verifier\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\npath = \"subject.rs\"\n[[bin]]\nname = \"extra\"\npath = \"extra.rs\"\n";
    std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
    std::fs::write(root.join("extra.rs"), "fn main() {}\n").unwrap();
    std::fs::write(root.join("Cargo.lock"), "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"owned_verifier\"\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    let source = format!(
        "pub fn answer() -> u8 {{ 1 }}\n#[cfg(test)] mod tests {{ #[test] fn required_behavior() {{ assert_eq!(super::answer(), {}); }} }}\n",
        if behavior_red { 2 } else { 1 }
    );
    std::fs::write(root.join("subject.rs"), &source).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "add",
            "Cargo.toml",
            "Cargo.lock",
            ".gitignore",
            "subject.rs",
            "extra.rs",
        ],
        vec![
            "-c",
            "user.name=owned-test",
            "-c",
            "user.email=owned@test",
            "commit",
            "-qm",
            "fixture",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
    }
    struct ChangeConfig {
        root: PathBuf,
    }
    impl Tool for ChangeConfig {
        fn name(&self) -> &str {
            "change_config"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "owned tracked configuration mutation".into(),
                params: serde_json::json!({"type":"object","properties":{}}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            let path = self.root.join("Cargo.toml");
            let mut text = std::fs::read_to_string(&path).unwrap();
            text.push_str("\n[features]\nchanged_verifier_configuration = []\n");
            std::fs::write(path, text).unwrap();
            Ok("configuration changed".into())
        }
    }
    struct Sequence {
        calls: Vec<(&'static str, Value)>,
        step: AtomicUsize,
    }
    impl Club for Sequence {
        fn label(&self) -> &str {
            "owned-native-verifier-sequence"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let index = self.step.fetch_add(1, Ordering::SeqCst);
            Ok(match self.calls.get(index) {
                Some((name, args)) => ClubReply::Calls(vec![ToolCall {
                    id: format!("gate-{index}"),
                    name: (*name).into(),
                    args: args.clone(),
                }]),
                None => ClubReply::Text("Observed requested verifier results.".into()),
            })
        }
    }
    let mut registry = ToolRegistry::with_team(root.clone(), Vec::new());
    registry.register(Box::new(ChangeConfig { root: root.clone() }));
    let expected_source = calls
        .iter()
        .rev()
        .find_map(|(name, args)| {
            (*name == "write_file"
                && args.get("path").and_then(Value::as_str) == Some("subject.rs"))
            .then(|| {
                args.get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .flatten()
        })
        .unwrap_or_else(|| source.clone());
    let max_hops = calls.len() + 2;
    let club = Sequence {
        calls,
        step: AtomicUsize::new(0),
    };
    let mut history = vec![ChatMsg::user(
        "Execute every distinct requested verifier, including behavior tests after compilation, and report the evidence.",
    )];
    let (tx, rx) = mpsc::channel::<TurnEvent>();
    run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(max_hops),
        &tx,
    )
    .unwrap();
    drop(tx);
    let results = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::ToolResult {
                name,
                summary,
                outcome,
                ..
            } if matches!(name.as_str(), "check" | "run_tests" | "cargo") => {
                Some((name, summary, outcome))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let started = results
        .iter()
        .filter(|(_, _, outcome)| {
            matches!(
                outcome.execution,
                ExecutionOutcome::Succeeded | ExecutionOutcome::Failed
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        started.len(),
        expected,
        "actual native verifier starts, with paired results: {results:?}"
    );
    assert!(
        started
            .iter()
            .any(|(_, _, outcome)| outcome.verification == VerificationOutcome::Passed),
        "fixture must produce real green native verification: {results:?}"
    );
    if behavior_red {
        assert!(
            started.iter().any(|(name, _, outcome)| name == "check"
                && outcome.verification == VerificationOutcome::Passed),
            "compile must pass: {results:?}"
        );
        assert!(
            started.iter().any(|(name, _, outcome)| name == "run_tests"
                && outcome.verification == VerificationOutcome::Failed),
            "real behavior test must execute and fail: {results:?}"
        );
        assert!(
            history
                .iter()
                .any(|m| m.role == ChatRole::Tool && m.content.contains("1 failed")),
            "actual Cargo failure output must be visible: {history:?}"
        );
    } else {
        assert!(
            started
                .iter()
                .all(|(_, _, outcome)| outcome.verification == VerificationOutcome::Passed),
            "all real native verifier fixtures must pass: {results:?}"
        );
    }
    if expected_source != source {
        let tests = started
            .iter()
            .filter(|(name, _, _)| name == "run_tests")
            .collect::<Vec<_>>();
        assert_eq!(
            tests.len(),
            2,
            "both pre-fix and post-fix tests must execute"
        );
        assert_eq!(tests[0].2.verification, VerificationOutcome::Failed);
        assert_eq!(tests[1].2.verification, VerificationOutcome::Passed);
    }
    assert_eq!(
        std::fs::read_to_string(root.join("subject.rs")).unwrap(),
        expected_source
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(output);
}
#[test]
fn explicitly_required_tests_execute_after_green_compile_on_same_source() {
    native_verifier_dispatch_scenario(
        vec![
            ("check", serde_json::json!({})),
            ("run_tests", serde_json::json!({})),
        ],
        2,
        true,
    );
}
#[test]
fn exact_green_repeat_executes_again() {
    native_verifier_dispatch_scenario(
        vec![
            ("check", serde_json::json!({})),
            ("check", serde_json::json!({})),
        ],
        2,
        false,
    );
}
#[test]
fn distinct_test_arguments_execute_after_green_suite() {
    native_verifier_dispatch_scenario(
        vec![
            ("run_tests", serde_json::json!({})),
            ("run_tests", serde_json::json!({"args":"--all-targets"})),
        ],
        2,
        false,
    );
}
#[test]
fn distinct_verifier_tools_both_execute() {
    native_verifier_dispatch_scenario(
        vec![
            ("check", serde_json::json!({})),
            ("cargo", serde_json::json!({"args":"check"})),
        ],
        2,
        false,
    );
}
#[test]
fn tracked_verifier_configuration_change_requires_fresh_check() {
    native_verifier_dispatch_scenario(
        vec![
            ("check", serde_json::json!({})),
            ("change_config", serde_json::json!({})),
            ("check", serde_json::json!({})),
        ],
        2,
        false,
    );
}

#[test]
fn green_compile_does_not_discard_distinct_behavior_verification() {
    native_verifier_dispatch_scenario(
        vec![
            (
                "write_file",
                serde_json::json!({"path":"extra.rs", "content":"fn main() {}\npub const EXTRA: bool = true;\n"}),
            ),
            ("check", serde_json::json!({})),
            ("read_file", serde_json::json!({"path":"subject.rs"})),
            ("run_tests", serde_json::json!({})),
        ],
        2,
        true,
    );
}

#[test]
fn distinct_red_verifier_keeps_source_repair_and_green_recheck_available() {
    native_verifier_dispatch_scenario(
        vec![
            (
                "write_file",
                serde_json::json!({"path":"extra.rs", "content":"fn main() {}\npub const EXTRA: bool = true;\n"}),
            ),
            ("check", serde_json::json!({})),
            ("read_file", serde_json::json!({"path":"subject.rs"})),
            ("run_tests", serde_json::json!({})),
            (
                "write_file",
                serde_json::json!({"path":"subject.rs", "content":"pub fn answer() -> u8 { 2 }\n#[cfg(test)] mod tests { #[test] fn required_behavior() { assert_eq!(super::answer(), 2); } }\n"}),
            ),
            ("run_tests", serde_json::json!({})),
        ],
        3,
        true,
    );
}

/// A foreground shell process is bounded even when a here-doc prevents it from
/// being classified as verification. A later repair and green recheck must work.
#[test]
fn task_call_budget_contains_unclassified_shell_and_allows_repair() {
    let _env_guard = crate::tests::env_lock();
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
    let _budget = EnvGuard::set("ANGEL_TASK_CALL_TIMEOUT_SECS", "2");
    let _hard = EnvGuard::unset("ANGEL_TOOL_HARD_TIMEOUT");
    let _timeout = EnvGuard::unset("ANGEL_TOOL_TIMEOUT");
    let spin = "import unittest\n\nclass Spin(unittest.TestCase):\n    def test_spin(self):\n        while True:\n            pass\n";
    let here_doc = format!(
        "cat > test_spin.py <<'EOF'\n{spin}EOF\npython3 -m unittest test_spin 2>&1 | head -20"
    );
    for (command, classified) in [
        ("python3 -m unittest test_spin", true),
        (here_doc.as_str(), false),
    ] {
        let root = scratch("task_call_budget");
        std::fs::write(root.join("test_spin.py"), spin).unwrap();
        let args = serde_json::json!({ "command": command });
        assert_eq!(
            is_verification_call(&ToolCall {
                id: String::new(),
                name: "shell".into(),
                args: args.clone(),
            }),
            classified,
            "the regression must exercise the classifier bypass"
        );
        let registry = ToolRegistry::with_team(root.clone(), Vec::new());
        let started = std::time::Instant::now();
        let result = registry.dispatch_with_cancel("shell", &args, None);
        let elapsed = started.elapsed();
        let text = match result {
            Ok(text) | Err(text) => text,
        };
        assert!(
            elapsed < std::time::Duration::from_secs(20),
            "{command}: must stop near the 2 s budget, took {elapsed:?}: {text}"
        );
        assert!(text.contains("timed out after 2s"), "{command}: {text}");
        let repaired = registry.dispatch_with_cancel("write_file", &serde_json::json!({
            "path": "test_spin.py",
            "content": "import unittest\nclass Spin(unittest.TestCase):\n    def test_spin(self):\n        self.assertEqual(2 + 2, 4)\n",
        }), None).expect("timeout must leave source repair available");
        assert!(!repaired.is_empty());
        let green = registry
            .dispatch_with_cancel(
                "shell",
                &serde_json::json!({
                    "command": "python3 -m unittest test_spin",
                }),
                None,
            )
            .expect("the repaired test must run successfully");
        assert!(green.contains("OK"), "{green}");
        let _ = std::fs::remove_dir_all(root);
    }
}
