//! Pre/post tool-use hooks coverage (matchers, block, yolo bypass, dispatch gate).
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory. Shared helpers
//! (`EnvGuard`) remain in the parent module.

use super::*;

// --- hooks suite ------------------------------------------------------------

#[test]
fn hook_matches_patterns() {
    assert!(hook_matches("*", "anything"));
    assert!(hook_matches("shell|cargo", "cargo"));
    assert!(!hook_matches("shell|cargo", "read_file"));
    assert!(hook_matches("read", "read_file")); // substring segment
}

#[test]
fn signaled_pre_tool_hook_is_a_failure() {
    let _lock = crate::tests::env_lock();
    let _timeout = EnvGuard::set("ANGEL_HOOK_TIMEOUT", "10");
    let outcome = run_hook(
        "kill -TERM $$",
        "shell",
        &serde_json::json!({"command": "true"}),
        None,
    );
    assert!(
        matches!(outcome, HookRun::Rejected { code, .. } if code != 0),
        "a signal-terminated hook must not allow the tool: {outcome:?}"
    );
}

#[test]
fn configured_pre_hook_fails_closed_on_timeout_and_unavailable_command() {
    let _lock = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let _timeout = EnvGuard::set("ANGEL_HOOK_TIMEOUT", "0");
    let _fail_closed = EnvGuard::set("ANGEL_PRE_HOOK_FAIL_OPEN", "0");
    let timed = Hooks {
        pre: vec![HookRule {
            matcher: "reverse".into(),
            command: "sleep 1".into(),
        }],
        post: vec![],
    };
    let denial = timed
        .pre_tool_use("reverse", &serde_json::json!({"text":"safe"}))
        .expect("timeout must deny a configured pre-hook by default");
    assert!(is_hook_blocked_result(&denial));
    assert!(denial.contains("timed out"), "{denial}");

    let unavailable = Hooks {
        pre: vec![HookRule {
            matcher: "reverse".into(),
            command: "x".repeat(70 * 1024),
        }],
        post: vec![],
    };
    let denial = unavailable
        .pre_tool_use("reverse", &serde_json::json!({"text":"safe"}))
        .expect("an unspawnable pre-hook must not bypass policy");
    assert!(is_hook_blocked_result(&denial));
    assert!(
        denial.contains("unavailable") && denial.contains("limit"),
        "{denial}"
    );
}

#[test]
fn pre_hook_fail_open_requires_an_explicit_switch() {
    let _lock = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let _timeout = EnvGuard::set("ANGEL_HOOK_TIMEOUT", "0");
    let _fail_open = EnvGuard::set("ANGEL_PRE_HOOK_FAIL_OPEN", "1");
    let hooks = Hooks {
        pre: vec![HookRule {
            matcher: "reverse".into(),
            command: "sleep 1".into(),
        }],
        post: vec![],
    };
    assert!(
        hooks
            .pre_tool_use("reverse", &serde_json::json!({"text":"safe"}))
            .is_none()
    );

    let _reject_timeout = EnvGuard::set("ANGEL_HOOK_TIMEOUT", "1");
    let rejecting = Hooks {
        pre: vec![HookRule {
            matcher: "reverse".into(),
            command: "printf 'policy says no'; exit 19".into(),
        }],
        post: vec![],
    };
    let denial = rejecting
        .pre_tool_use("reverse", &serde_json::json!({"text":"safe"}))
        .expect("fail-open must not override a hook's deliberate rejection");
    assert!(is_hook_blocked_result(&denial));
    assert!(
        denial.contains("hook exited 19: policy says no"),
        "{denial}"
    );
}

#[test]
fn oversized_hook_payload_is_replaced_by_a_bounded_digest_receipt() {
    let huge = "z".repeat(2 * 1024 * 1024);
    let outcome = run_hook(
        r#"case "$ANGEL_TOOL_ARGS" in *angel-hook-payload/v1*) ;; *) exit 23;; esac; case "$ANGEL_TOOL_ARGS" in *'"omitted":true'*) exit 0;; *) exit 24;; esac"#,
        "write_file",
        &serde_json::json!({"path":"large.txt","content":huge}),
        None,
    );
    assert!(
        matches!(outcome, HookRun::Passed { .. }),
        "large payload should reach the hook as a bounded receipt, not E2BIG: {outcome:?}"
    );
}

#[test]
fn hooks_parse_reads_events() {
    let cfg = r#"{"hooks":{
            "PreToolUse":[{"matcher":"shell","command":"echo hi"}],
            "PostToolUse":[{"command":"log"}]
        }}"#;
    let h = Hooks::parse(cfg);
    assert_eq!(h.pre.len(), 1);
    assert_eq!(h.pre[0].matcher, "shell");
    assert_eq!(h.pre[0].command, "echo hi");
    assert_eq!(h.post.len(), 1);
    assert_eq!(h.post[0].matcher, "*"); // default when omitted
    assert!(Hooks::parse("garbage").is_empty());
}

#[test]
fn pre_tool_use_hook_can_block() {
    // Serialize against yolo_bypasses_* which sets ANGEL_YOLO=1 process-wide.
    let _lock = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    // Non-zero exit blocks, with the hook's output as the reason.
    let block = Hooks {
        pre: vec![HookRule {
            matcher: "*".into(),
            command: "echo nope >&2; exit 2".into(),
        }],
        post: vec![],
    };
    let r = block.pre_tool_use("shell", &serde_json::json!({}));
    assert!(
        r.as_deref().is_some_and(|s| s.contains("nope")),
        "blocked: {r:?}"
    );
    assert!(is_hook_blocked_result(r.as_deref().unwrap()));
    // exit 0 allows.
    let ok = Hooks {
        pre: vec![HookRule {
            matcher: "*".into(),
            command: "exit 0".into(),
        }],
        post: vec![],
    };
    assert!(ok.pre_tool_use("shell", &serde_json::json!({})).is_none());
    // A non-matching tool is never gated.
    let scoped = Hooks {
        pre: vec![HookRule {
            matcher: "cargo".into(),
            command: "exit 1".into(),
        }],
        post: vec![],
    };
    assert!(
        scoped
            .pre_tool_use("read_file", &serde_json::json!({}))
            .is_none()
    );
}

#[test]
fn yolo_bypasses_pre_tool_hook_denials() {
    let _lock = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let block = Hooks {
        pre: vec![HookRule {
            matcher: "*".into(),
            command: "echo denied; exit 1".into(),
        }],
        post: vec![],
    };
    assert!(
        block
            .pre_tool_use("shell", &serde_json::json!({}))
            .is_none()
    );
}

#[test]
fn dispatch_with_hooks_blocks_then_allows() {
    // Serialize against yolo_bypasses_* which sets ANGEL_YOLO=1 process-wide.
    let _lock = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ReverseTool));
    let args = serde_json::json!({ "text": "ab" });
    // No hooks → identical to a plain dispatch.
    let direct = reg.dispatch("reverse", &args).unwrap();
    let empty = Hooks::default();
    assert_eq!(dispatch_with_hooks(&reg, &empty, "reverse", &args), direct);
    // A blocking PreToolUse hook → the tool never runs.
    let block = Hooks {
        pre: vec![HookRule {
            matcher: "reverse".into(),
            command: "echo denied; exit 1".into(),
        }],
        post: vec![],
    };
    let r = dispatch_with_hooks(&reg, &block, "reverse", &args);
    assert!(r.contains("blocked") && r.contains("denied"), "got: {r}");
    assert_eq!(
        turn_event_outcome(
            &ToolCall {
                id: "blocked".into(),
                name: "reverse".into(),
                args,
            },
            &r,
            false,
        ),
        ToolOutcome {
            execution: ExecutionOutcome::Denied,
            verification: VerificationOutcome::NotApplicable,
        }
    );
}

#[test]
fn blocked_write_is_denied_without_mutation_or_verification_credit() {
    let _lock = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "1");
    let _first_write = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let root = scratch("blocked-write-accounting");
    let target = root.join("guarded.rs");
    std::fs::write(&target, "original\n").unwrap();
    let config = root.join("hooks.json");
    std::fs::write(
        &config,
        r#"{"hooks":{"PreToolUse":[{"matcher":"write_file","command":"echo policy-denied >&2; exit 9"}]}}"#,
    )
    .unwrap();
    let _config = EnvGuard::set("ANGEL_HOOKS_CONFIG", config.to_string_lossy().as_ref());

    struct BlockedWriter {
        step: AtomicUsize,
        saw_verify_nudge: AtomicBool,
    }
    impl Club for BlockedWriter {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "blocked-writer"
        }
        fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(match self.step.fetch_add(1, Ordering::Relaxed) {
                0 => ClubReply::Calls(vec![ToolCall {
                    id: "blocked-write".into(),
                    name: "write_file".into(),
                    args: serde_json::json!({"path": "guarded.rs", "content": "changed\n"}),
                }]),
                _ => {
                    self.saw_verify_nudge.store(
                        messages
                            .iter()
                            .any(|message| message.content.as_ref() == FINAL_VERIFY_NUDGE),
                        Ordering::Relaxed,
                    );
                    ClubReply::Text("policy denial reported".into())
                }
            })
        }
    }

    let club = BlockedWriter {
        step: AtomicUsize::new(0),
        saw_verify_nudge: AtomicBool::new(false),
    };
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    register_file_tools(&mut registry, root.clone());
    let (event_tx, event_rx) = mpsc::channel();
    let mut history = vec![ChatMsg::user("try the guarded write")];
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &event_tx,
    )
    .unwrap();

    assert_eq!(answer, "policy denial reported");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "original\n");
    assert!(!club.saw_verify_nudge.load(Ordering::Relaxed));
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Tool && is_hook_blocked_result(&message.content)
    }));
    assert!(event_rx.try_iter().any(|event| matches!(
        event,
        TurnEvent::ToolResult { id, outcome, .. }
            if id.0 == "blocked-write" && outcome.execution == ExecutionOutcome::Denied
    )));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn configured_hooks_make_the_tool_batch_a_serial_barrier() {
    let _lock = crate::tests::env_lock();
    let _yolo_off = EnvGuard::set("ANGEL_YOLO", "0");
    let root = scratch("hook-serial-barrier");
    let active = root.join("hook-active");
    let overlap = root.join("hook-overlap");
    let command = format!(
        "if mkdir '{}'; then sleep 0.15; rmdir '{}'; else touch '{}'; exit 91; fi",
        active.display(),
        active.display(),
        overlap.display()
    );
    let config = root.join("hooks.json");
    std::fs::write(
        &config,
        serde_json::json!({
            "hooks": {"PreToolUse": [{"matcher": "reverse", "command": command}]}
        })
        .to_string(),
    )
    .unwrap();
    let _config = EnvGuard::set("ANGEL_HOOKS_CONFIG", config.to_string_lossy().as_ref());

    struct TwoReads(AtomicUsize);
    impl Club for TwoReads {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "two-hooked-reads"
        }
        fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
            Ok(if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
                ClubReply::Calls(vec![
                    ToolCall {
                        id: "hook-a".into(),
                        name: "reverse".into(),
                        args: serde_json::json!({"text":"one"}),
                    },
                    ToolCall {
                        id: "hook-b".into(),
                        name: "reverse".into(),
                        args: serde_json::json!({"text":"two"}),
                    },
                ])
            } else {
                ClubReply::Text("done".into())
            })
        }
    }

    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.clone());
    registry.register(Box::new(ReverseTool));
    let mut history = vec![ChatMsg::user("run both reads")];
    let (event_tx, _) = mpsc::channel();
    let started = Instant::now();
    let answer = run_turn(
        &TwoReads(AtomicUsize::new(0)),
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(3),
        &event_tx,
    )
    .unwrap();

    assert_eq!(answer, "done");
    assert!(
        started.elapsed() >= Duration::from_millis(250),
        "two 150ms configured hooks overlapped instead of forming a serial barrier"
    );
    assert!(!overlap.exists(), "configured hooks overlapped");
    let results = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .map(|message| message.content.as_ref())
        .collect::<Vec<_>>();
    assert!(
        results.contains(&"eno") && results.contains(&"owt"),
        "{results:?}"
    );
    assert!(results.iter().all(|result| !is_hook_blocked_result(result)));
    let _ = std::fs::remove_dir_all(root);
}
