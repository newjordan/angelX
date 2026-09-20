// Included in caddy::tests so these cases use the same isolated store fixture.

#[test]
fn caddy_unicode_boundary_preserves_headless_terminal_receipt() {
    use crate::agent::harness::{TaskJsonContext, TaskJsonEnvelope, TurnOutcome, TurnStopReason};
    let _guard = crate::tests::env_lock();
    let (dir, _, workspace, _env) = fixture("headless-unicode");
    let history = [
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "unicode".into(),
            name: "shell".into(),
            args: serde_json::json!({"command": "cat benchmark.log"}),
        }]),
        ChatMsg::tool("unicode", format!("{}é", "a".repeat(8191))),
    ];
    let envelope = TaskJsonEnvelope::from_outcome(
        TaskJsonContext {
            task_id: Some("unicode-limit".into()),
            run_id: None,
            workspace,
            club: None,
            model: None,
            reasoning_effort: None,
            output_budget: None,
            elapsed_ms: 1,
            tools: Vec::new(),
            timing: None,
            usage: None,
            runtime: None,
            session_id: None,
            artifacts: Vec::new(),
            memory_health: crate::knowledge::caddy::StoreHealthSummary::default(),
        },
        TurnOutcome {
            stop_notice: None,
            reward_binding: None,
            answer: "done".into(),
            stop_reason: TurnStopReason::Answer,
            hops: 1,
            interrupted: false,
            deadline_reached: false,
            max_hops_reached: false,
            acceptance: None,
            rollout_id: None,
            tools: Vec::new(),
            timing: None,
        },
        &history,
    );
    let receipt = serde_json::to_value(envelope).unwrap();
    assert_eq!(receipt["answer"], "done");
    assert_eq!(receipt["task_id"], "unicode-limit");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn caddy_truncation_handles_ascii_unicode_and_zero_limits() {
    for text in ["a".repeat(10_000), "é🦀界".repeat(4_000)] {
        for limit in [0, 1, 2, 160, 200, 8192] {
            let bounded = bound_chars(&text, limit);
            assert!(bounded.chars().count() <= limit);
            assert!(bounded.len() <= limit * 4);
            if text.chars().count() > limit && limit > 0 {
                assert!(bounded.ends_with('…'));
            }
            let prefix = bound_bytes(&text, limit);
            assert!(prefix.len() <= limit);
            assert!(text.starts_with(prefix));
        }
    }
    assert_eq!(bound_chars("é🦀", 2), "é🦀");
    assert_eq!(
        bound_bytes(&format!("{}é", "a".repeat(8191)), 8192).len(),
        8191
    );
    assert_eq!(
        env_prefixes(&format!("KEY={} cargo test", "x".repeat(10_000))),
        Vec::<String>::new()
    );
    let prefixes = "K=V ".repeat(100);
    assert_eq!(env_prefixes(&prefixes).len(), MAX_ENV_PREFIXES);
}

#[test]
fn caddy_rejects_untyped_success_failed_receipts_and_reused_call_ids() {
    use crate::agent::harness::{ExecutionOutcome as E, ToolOutcome, VerificationOutcome as V};
    let _guard = crate::tests::env_lock();
    let (dir, _, workspace, _env) = fixture("truth");
    for (name, command, output) in [
        ("shell", "cat benchmark.log", "geomean 0.86"),
        (
            "shell",
            "./benchmark.sh",
            "exit 7\nverification unavailable",
        ),
        ("shell", "./benchmark.sh", "[exit 0] verified: passed"),
        ("proc_run", "./benchmark.sh", "started pid 123"),
        ("run_tests", "", "1 passed; 0 failed"),
    ] {
        let call = ToolCall {
            id: "untyped".into(),
            name: name.into(),
            args: serde_json::json!({"command": command}),
        };
        let history = [
            ChatMsg::assistant_calls(vec![call]),
            ChatMsg::tool("untyped", output),
        ];
        assert_eq!(
            write_back_from_history(&workspace, &history).0,
            0,
            "{name}: {command}"
        );
    }
    let call = ToolCall {
        id: "typed".into(),
        name: "run_tests".into(),
        args: serde_json::json!({"args": "--release"}),
    };
    for execution in [
        E::Failed,
        E::NotStarted,
        E::Denied,
        E::Cancelled,
        E::Panicked,
    ] {
        let result = ChatMsg::tool("typed", "1 passed; 0 failed").with_tool_receipt(
            &call,
            ToolOutcome {
                execution,
                verification: V::Passed,
            },
        );
        assert!(!verified_recipe_call(&call, &result));
    }
    for verification in [V::Failed, V::Inconclusive, V::NotApplicable] {
        let result = ChatMsg::tool("typed", "1 passed; 0 failed").with_tool_receipt(
            &call,
            ToolOutcome {
                execution: E::Succeeded,
                verification,
            },
        );
        assert!(!verified_recipe_call(&call, &result));
    }
    let result = ChatMsg::tool("typed", "1 passed; 0 failed").with_tool_receipt(
        &call,
        ToolOutcome {
            execution: E::Succeeded,
            verification: V::Passed,
        },
    );
    assert!(verified_recipe_call(&call, &result));
    let mut spoofed = call.clone();
    spoofed.args["command"] = "arbitrary-unexecuted-command".into();
    let (description, _) = command_of(&spoofed).unwrap();
    assert_eq!(description, command_of(&call).unwrap().0);
    assert!(!description.contains("arbitrary-unexecuted-command"));
    let mut native = call.clone();
    native.name = "run_tests".into();
    native.args["runtime"] = "node".into();
    let node_description = command_of(&native).unwrap().0;
    native.args["runtime"] = "python".into();
    assert_ne!(node_description, command_of(&native).unwrap().0);
    native.name = "cargo".into();
    let cargo_description = command_of(&native).unwrap().0;
    native.args["runtime"] = "arbitrary-unused-runtime".into();
    assert_eq!(cargo_description, command_of(&native).unwrap().0);
    let irrelevant = ToolCall {
        id: "large".into(),
        name: "write_file".into(),
        args: serde_json::json!({"content": "x".repeat(100_000)}),
    };
    let unrelated = ChatMsg::tool("large", "ok").with_tool_receipt(
        &irrelevant,
        ToolOutcome {
            execution: E::Succeeded,
            verification: V::Passed,
        },
    );
    assert!(unrelated.tool_receipt.is_none());
    let mut changed = call.clone();
    changed.args = serde_json::json!({"args": "--different"});
    assert!(!verified_recipe_call(&changed, &result));
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains("tool_receipt"));
    let restored: ChatMsg = serde_json::from_str(&serialized).unwrap();
    assert!(
        !verified_recipe_call(&call, &restored),
        "imports cannot recreate execution truth"
    );
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn caddy_legacy_recipes_need_reverification_and_unicode_writeback_stays_bounded() {
    use crate::agent::harness::{ExecutionOutcome, ToolOutcome, VerificationOutcome};
    let _guard = crate::tests::env_lock();
    let (dir, _, workspace, _env) = fixture("migration");
    let call = ToolCall {
        id: "bound".into(),
        name: "run_tests".into(),
        args: serde_json::json!({"args": "🦀".repeat(10_000)}),
    };
    let command = bound_chars(&command_of(&call).unwrap().0, MAX_STORED_COMMAND_CHARS);
    let repo = seed_store(
        &dir,
        &workspace,
        &[Recipe {
            ts_ms: now_ms(),
            command: command.clone(),
            env: vec![],
            duration_ms: None,
            tool: "run_tests".into(),
            note: "verified: ok".into(),
            verification: None,
            verified_head: None,
            stale_since_changes: 0,
            workspace_state: None,
            observed_workspace_state: None,
        }],
        &[],
    );
    assert!(
        render_card(&workspace, card_cap()).is_empty(),
        "legacy records are not verified"
    );
    let result = ChatMsg::tool("bound", format!("{}é", "a".repeat(8191))).with_tool_receipt(
        &call,
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::Passed,
        },
    );
    let history = [ChatMsg::assistant_calls(vec![call]), result];
    assert_eq!(write_back_from_history(&workspace, &history), (1, 0));
    let recipes: Vec<Recipe> = load_jsonl(&repo.join("recipes.jsonl"));
    assert_eq!(
        recipes.len(),
        2,
        "fresh verification must not dedup against legacy text"
    );
    let fresh = &recipes[1];
    assert_eq!(fresh.command.chars().count(), MAX_STORED_COMMAND_CHARS);
    assert!(fresh.command.len() <= 4 * MAX_STORED_COMMAND_CHARS);
    let card = render_card(&workspace, card_cap());
    assert!(!card.is_empty());
    assert!(card.len() <= card_cap());
    assert!(
        card.lines()
            .all(|line| line.chars().count() <= MAX_LINE_CHARS)
    );
    let hazard = ToolCall {
        id: "hazard".into(),
        name: "shell".into(),
        args: serde_json::json!({"command": "a".repeat(10_000)}),
    };
    let history = [
        ChatMsg::assistant_calls(vec![hazard]),
        ChatMsg::tool("hazard", format!("tool error: {}", "界".repeat(10_000))),
    ];
    assert_eq!(write_back_from_history(&workspace, &history), (0, 1));
    let hazards: Vec<Hazard> = load_jsonl(&repo.join("hazards.jsonl"));
    assert!(hazards[0].command.chars().count() <= MAX_STORED_COMMAND_CHARS);
    assert!(hazards[0].diagnostic.chars().count() <= MAX_DIAGNOSTIC_CHARS);
    assert!(hazards[0].diagnostic.len() <= 4 * MAX_DIAGNOSTIC_CHARS);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
