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
        session_start: vec![],
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
        session_start: vec![],
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
        session_start: vec![],
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
        session_start: vec![],
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
            "PostToolUse":[{"command":"log"}],
            "SessionStart":[{"matcher":"resume|fork","command":"record"}]
        }}"#;
    let h = Hooks::parse(cfg);
    assert_eq!(h.pre.len(), 1);
    assert_eq!(h.pre[0].matcher, "shell");
    assert_eq!(h.pre[0].command, "echo hi");
    assert_eq!(h.post.len(), 1);
    assert_eq!(h.post[0].matcher, "*"); // default when omitted
    assert_eq!(h.session_start.len(), 1);
    assert_eq!(h.session_start[0].matcher, "resume|fork");
    assert_eq!(h.session_start[0].command, "record");
    assert!(!h.is_empty());
    let only_session = Hooks::parse(r#"{"hooks":{"SessionStart":[{"command":"record"}]}}"#);
    assert!(
        only_session.is_empty(),
        "SessionStart must not force tool batches onto the serial path"
    );
    assert_eq!(only_session.session_start.len(), 1);
    assert_eq!(only_session.session_start[0].matcher, "*");
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
        session_start: vec![],
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
        session_start: vec![],
    };
    assert!(ok.pre_tool_use("shell", &serde_json::json!({})).is_none());
    // A non-matching tool is never gated.
    let scoped = Hooks {
        pre: vec![HookRule {
            matcher: "cargo".into(),
            command: "exit 1".into(),
        }],
        post: vec![],
        session_start: vec![],
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
        session_start: vec![],
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
        session_start: vec![],
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
    // Task mode: an unverified edit would raise the stop checkpoint.
    let _task = EnvGuard::set("ANGEL_TASK_ACTIVE", "1");
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
                            .any(|message| book::ledger::is_warpath_message(&message.content)),
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
        .map(|message| book::ledger::without_warpaths(&message.content).into_owned())
        .collect::<Vec<_>>();
    assert!(
        results.iter().any(|r| r == "eno") && results.iter().any(|r| r == "owt"),
        "{results:?}"
    );
    assert!(results.iter().all(|result| !is_hook_blocked_result(result)));
    let _ = std::fs::remove_dir_all(root);
}

fn session_start_record(dir: &std::path::Path) -> (Hooks, std::path::PathBuf) {
    let log = dir.join("session-start.log");
    let command = format!(
        "printf '%s\\n' \"$ANGEL_SESSION_ID\" >> '{}'; printf '%s\\n' \"$ANGEL_WORKSPACE\" >> '{}'; cat >> '{}'",
        log.display(),
        log.display(),
        log.display()
    );
    let hooks = Hooks {
        pre: vec![],
        post: vec![],
        session_start: vec![HookRule {
            matcher: "*".into(),
            command,
        }],
    };
    (hooks, log)
}

fn session_start_events(log: &std::path::Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let mut lines = text.lines();
    let mut events = Vec::new();
    while let Some(session_id) = lines.next() {
        let workspace = lines.next().unwrap_or("");
        let json = lines.next().expect("session start payload");
        let value: serde_json::Value = serde_json::from_str(json).expect(json);
        let mut keys: Vec<_> = value.as_object().expect("object").keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["cwd", "hook_event_name", "session_id", "source"],
            "payload must stay identity-only: {value}"
        );
        assert_eq!(value["hook_event_name"], "SessionStart");
        assert_eq!(value["session_id"], session_id);
        assert_eq!(value["cwd"], workspace);
        events.push(value);
    }
    events
}

#[test]
fn session_start_payload_is_identity_only_and_follows_the_source() {
    let _lock = crate::tests::env_lock();
    let _timeout = EnvGuard::set("ANGEL_HOOK_TIMEOUT", "10");
    let root = scratch("session-start-payload");
    let real = root.join("real");
    let link = root.join("link");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let canonical = std::fs::canonicalize(&real).unwrap();
    let (hooks, log) = session_start_record(&root);
    hooks.session_start("", &link, SessionStartSource::Startup);
    assert!(
        !log.exists(),
        "an empty session id is not a session to announce"
    );

    let scoped = Hooks {
        pre: vec![],
        post: vec![],
        session_start: vec![HookRule {
            matcher: "resume|workspace".into(),
            command: hooks.session_start[0].command.clone(),
        }],
    };
    scoped.session_start("skip-me", &link, SessionStartSource::Fork);
    assert!(!log.exists(), "fork must not match resume|workspace");

    scoped.session_start("sess-1", &link, SessionStartSource::Workspace);
    scoped.session_start("sess-2", &real, SessionStartSource::Resume);
    let events = session_start_events(&log);
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[0]["source"], "workspace");
    assert_eq!(events[0]["session_id"], "sess-1");
    assert_eq!(events[0]["cwd"], canonical.to_string_lossy().as_ref());
    assert_eq!(events[1]["source"], "resume");
    assert_eq!(events[1]["session_id"], "sess-2");
    assert_eq!(events[1]["cwd"], canonical.to_string_lossy().as_ref());
    let raw = std::fs::read_to_string(&log).unwrap();
    assert!(!raw.contains("transcript"));
    assert!(!raw.contains("sess-1.json"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn session_start_failure_and_timeout_do_not_block() {
    let _lock = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    let root = scratch("session-start-timeout");
    let payload =
        br#"{"hook_event_name":"SessionStart","session_id":"s","cwd":"/tmp","source":"startup"}"#;
    {
        let _timeout = EnvGuard::set("ANGEL_HOOK_TIMEOUT", "0");
        let started = Instant::now();
        let timed = run_session_start_hook("sleep 30", payload, &root, "s");
        assert!(
            matches!(timed, HookRun::TimedOut),
            "YOLO must not drop the SessionStart deadline: {timed:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "a slow SessionStart hook blocked the caller for {:?}",
            started.elapsed()
        );

        let hooks = Hooks {
            pre: vec![],
            post: vec![],
            session_start: vec![
                HookRule {
                    matcher: "*".into(),
                    command: "sleep 30".into(),
                },
                HookRule {
                    matcher: "*".into(),
                    command: "exit 9".into(),
                },
            ],
        };
        let started = Instant::now();
        hooks.session_start("still-starts", &root, SessionStartSource::Startup);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "failing SessionStart hooks blocked session_start for {:?}",
            started.elapsed()
        );
    }

    let _timeout = EnvGuard::set("ANGEL_HOOK_TIMEOUT", "10");
    let failed = run_session_start_hook("echo no >&2; exit 4", payload, &root, "s");
    assert!(
        matches!(failed, HookRun::Rejected { code: 4, .. }),
        "{failed:?}"
    );
    let missing = run_session_start_hook(&"x".repeat(70 * 1024), payload, &root, "s");
    assert!(matches!(missing, HookRun::Unavailable(_)), "{missing:?}");
    let _ = std::fs::remove_dir_all(root);
}
