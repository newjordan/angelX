//! Cap/prune/context-fit/compaction and schema-budget coverage.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- context / compact suite ------------------------------------------------

#[test]
fn cap_text_passes_small_output_through() {
    let s = "short\noutput";
    assert_eq!(cap_text(s, 1024, 100), s);
    // Both dimensions disabled is always a no-op, even for huge input.
    let big = "x".repeat(5000);
    assert_eq!(cap_text(&big, 0, 0), big);
}

#[test]
fn cap_text_caps_by_lines_keeping_head_and_tail() {
    let input = (0..100)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let out = cap_text(&input, 0, 10); // bytes off, 10-line cap
    assert!(out.contains("line0"), "head kept: {out}");
    assert!(out.contains("line99"), "tail kept: {out}");
    assert!(
        out.contains("middle line(s) elided"),
        "marker present: {out}"
    );
    assert!(
        out.lines().count() <= 12,
        "expected ~11 lines, got {}",
        out.lines().count()
    );
}

#[test]
fn cap_text_caps_by_bytes_on_char_boundary() {
    // Multibyte content so a naive byte split would panic mid-codepoint.
    let input = "é".repeat(10_000); // 2 bytes each = 20_000 bytes, 1 line
    let out = cap_text(&input, 1024, 0); // 1 KiB byte cap, lines off
    assert!(out.len() < input.len(), "did not shrink: {}", out.len());
    assert!(out.contains("middle byte(s) elided"), "marker present");
    assert!(out.starts_with('é'), "head kept on a char boundary");
    assert!(out.ends_with('é'), "tail kept on a char boundary");
}

fn pruning_history() -> Vec<ChatMsg> {
    vec![
        ChatMsg::system("sys"),
        ChatMsg::user("u1"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "a".into(),
            name: "read_file".into(),
            args: serde_json::json!({}),
        }]),
        ChatMsg::tool("a", "ra"),
        ChatMsg::user("u2"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "b".into(),
            name: "read_file".into(),
            args: serde_json::json!({}),
        }]),
        ChatMsg::tool("b", "rb"),
        ChatMsg::assistant("done"),
    ]
}

#[test]
fn prune_history_disabled_and_under_cap_are_noops() {
    let mut h = pruning_history();
    let orig = h.len();
    prune_history(&mut h, 0); // disabled
    assert_eq!(h.len(), orig);
    prune_history(&mut h, 100); // under cap
    assert_eq!(h.len(), orig);
}

#[test]
fn prune_history_keeps_system_and_drops_oldest() {
    let mut h = pruning_history();
    prune_history(&mut h, 5);
    assert!(h.len() <= 5, "len {}", h.len());
    assert_eq!(h[0].role, ChatRole::System, "system preamble kept");
    assert_eq!(&*h.last().unwrap().content, "done", "newest turn kept");
}

#[test]
fn prune_history_never_orphans_a_tool_result() {
    // Exercise both the clean-boundary and tool-advance paths.
    for cap in [3usize, 4, 5, 6] {
        let mut h = pruning_history();
        prune_history(&mut h, cap);
        // The kept suffix must not begin on a tool result.
        if let Some(m) = h.iter().find(|m| m.role != ChatRole::System) {
            assert_ne!(
                m.role,
                ChatRole::Tool,
                "cap {cap}: kept suffix starts on an orphan tool result"
            );
        }
        // Every surviving tool result must pair with a preceding call.
        for (i, m) in h.iter().enumerate() {
            if m.role == ChatRole::Tool {
                let id = m.tool_call_id.clone().unwrap();
                let paired = h[..i]
                    .iter()
                    .any(|p| p.tool_calls.iter().any(|c| c.id == id));
                assert!(paired, "cap {cap}: tool {id} lost its assistant call");
            }
        }
    }
}

#[test]
fn prune_history_refuses_to_gut_system_preamble() {
    let mut h = vec![
        ChatMsg::system("s1"),
        ChatMsg::system("s2"),
        ChatMsg::system("s3"),
        ChatMsg::user("u"),
        ChatMsg::assistant("a"),
    ];
    let orig = h.len();
    // Cap (2) is smaller than the 3-message system preamble — must no-op
    // rather than evict the prompt.
    prune_history(&mut h, 2);
    assert_eq!(h.len(), orig, "must not evict the system preamble");
}

#[test]
fn prune_history_preserves_one_exact_harness_turn_context_within_the_cap() {
    let context = test_turn_context("keep the active controls");
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user("operator task"),
        ChatMsg::harness(context.as_str()),
    ];
    for i in 0..12 {
        history.push(ChatMsg::assistant(format!("work {i}")));
    }

    prune_history(&mut history, 5);

    assert!(history.len() <= 5, "hard cap exceeded: {}", history.len());
    assert_eq!(history[0].role, ChatRole::System);
    let contexts = history
        .iter()
        .filter(|message| crate::app_control::is_turn_context_message(message))
        .collect::<Vec<_>>();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].role, ChatRole::Harness);
    assert_eq!(&*contexts[0].content, context);
    assert_eq!(&*history.last().unwrap().content, "work 11");
}

#[test]
fn estimate_tokens_is_chars_over_four_ish() {
    let h = vec![ChatMsg::user("a".repeat(40))]; // 40 content + 8 overhead = 48 -> 12
    let t = estimate_tokens(&h);
    assert!((10..=14).contains(&t), "got {t}");
}

#[test]
fn history_token_roll_reuses_on_append_and_recomputes_on_rewrite() {
    let mut roll = HistoryTokenRoll::default();
    let mut h = vec![ChatMsg::user("hello"), ChatMsg::assistant("world")];
    let base = roll.recompute(&h);
    assert_eq!(base, estimate_tokens(&h));
    // Pure append: only the new tail is counted.
    h.push(ChatMsg::user("more"));
    let grown = roll.observe(&h);
    assert_eq!(grown, estimate_tokens(&h));
    // Same snapshot: no rewalk.
    assert_eq!(roll.observe(&h), grown);
    // In-place rewrite of an earlier message: full recompute (content hash
    // changes even when the char length is similar).
    h[0] = ChatMsg::user("HELLO — a much longer rewritten user turn");
    let rewritten = roll.observe(&h);
    assert_eq!(rewritten, estimate_tokens(&h));
    assert_ne!(
        rewritten, grown,
        "rewrite must change the estimate, not reuse the stale append count"
    );
}

#[test]
fn json_length_counter_matches_compact_serialization_without_payload_materialization() {
    let values = [
        serde_json::Value::Null,
        serde_json::json!(true),
        serde_json::json!(-1234.5),
        serde_json::json!("quote \" slash \\ controls \n\u{0001} and 🦀"),
        serde_json::json!({
            "nested": [null, false, {"encoded": "A".repeat(256 * 1024)}],
            "unicode-key-λ": "value"
        }),
    ];
    for value in values {
        assert_eq!(
            json_serialized_len(&value),
            serde_json::to_string(&value).unwrap().len(),
            "streaming byte count drifted for {value:?}"
        );
    }
}

#[test]
fn estimate_tokens_counts_large_tool_arguments_by_exact_serialized_bytes() {
    let args = serde_json::json!({
        "path": "artifact.bin",
        "payload": "A".repeat(512 * 1024),
        "flags": [true, false, null]
    });
    let serialized_len = serde_json::to_string(&args).unwrap().len();
    let history = vec![ChatMsg::assistant_calls(vec![ToolCall {
        id: "call-1".to_string(),
        name: "write_blob".to_string(),
        args,
    }])];
    assert_eq!(
        estimate_tokens(&history),
        ("write_blob".len() + serialized_len + 8) / 4
    );
}

#[test]
fn estimate_tool_tokens_counts_schema_bytes() {
    assert_eq!(estimate_tool_tokens(&[]), 0, "no tools → no cost");
    let tools = vec![ToolDef {
        name: "read_file".to_string(),                 // 9
        description: "Read a file".to_string(),        // 11
        params: serde_json::json!({"type": "object"}), // 17 chars serialized
    }];
    let t = estimate_tool_tokens(&tools);
    // (9 + 11 + 17 + 8) / 4 ≈ 11; just assert it's a non-trivial positive cost.
    assert!(t >= 8, "tool schema should carry real token cost, got {t}");
}

#[test]
fn defs_for_run_trims_to_essentials_on_tiny_window() {
    // Serialize with the sibling profile test below: defs_for_run reads
    // ANGEL_TOOL_SCHEMA_PROFILE, so an unguarded run can observe the
    // essential profile it sets under env_lock and fail the full-set asserts.
    let _guard = crate::tests::env_lock();
    let reg = ToolRegistry::with_defaults();
    let full = reg.defs();
    // Unknown or roomy window → the full advertised set, unchanged.
    assert_eq!(reg.defs_for_run(None, false).len(), full.len());
    assert_eq!(reg.defs_for_run(Some(1_000_000), false).len(), full.len());
    // Tiny window → trimmed to the essential loop, strictly smaller, and the
    // core read tool must survive so the agent can still function + bloat.
    let lean = reg.defs_for_run(Some(2048), false);
    assert!(
        lean.len() < full.len(),
        "tiny window should trim schemas: {} vs {}",
        lean.len(),
        full.len()
    );
    assert!(
        lean.iter().all(|d| is_essential_tool(&d.name)),
        "lean set must be essentials only"
    );
    assert!(lean.iter().any(|d| d.name == "read_file"));
}

#[test]
fn essential_schema_profile_is_lean_but_keeps_discovery() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "essential");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs();
    let lean = reg.defs_for_run(Some(1_000_000), false);
    assert!(lean.len() < full.len(), "{} vs {}", lean.len(), full.len());
    assert!(lean.iter().all(|tool| is_essential_tool(&tool.name)));
    assert!(lean.iter().any(|tool| tool.name == "tool_search"));
    assert!(lean.iter().any(|tool| tool.name == "code_mode"));
    let code_mode = lean.iter().find(|tool| tool.name == "code_mode").unwrap();
    let code_mode_tokens = estimate_tool_tokens(std::slice::from_ref(code_mode));
    assert!(
        code_mode_tokens <= 700,
        "code_mode's per-request schema cost regressed: {code_mode_tokens} tokens"
    );
    assert!(code_mode.description.contains("read_file"));
    assert!(!code_mode.description.contains("web_search"));
    assert!(!code_mode.description.contains("shell, "));

    let result = reg
        .dispatch(
            "tool_search",
            &serde_json::json!({"query": "run cargo tests"}),
        )
        .unwrap();
    assert!(
        result.contains("run_tests") || result.contains("cargo"),
        "hidden build tools must remain discoverable: {result}"
    );
    let activated = reg.defs_for_run(Some(1_000_000), true);
    assert!(
        activated
            .iter()
            .any(|tool| matches!(tool.name.as_str(), "run_tests" | "cargo")),
        "a search hit must become a real provider schema on the next request"
    );
}

#[test]
fn tool_search_keeps_discovering_beyond_the_former_default_and_hard_caps() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "essential");
    let _cap = EnvGuard::unset("ANGEL_TOOL_SEARCH_ACTIVE_MAX");

    struct ResearchCapability(String);
    impl Tool for ResearchCapability {
        fn name(&self) -> &str {
            &self.0
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.0.clone(),
                description: format!("{} research capability", self.0),
                params: serde_json::json!({"type":"object","properties":{}}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            Ok(self.0.clone())
        }
    }

    let mut reg = ToolRegistry::new();
    for index in 0..40 {
        reg.register_deferred(Box::new(ResearchCapability(format!("research{index}"))));
    }
    reg.enable_tool_search();
    for index in 0..40 {
        let name = format!("research{index}");
        let result = reg
            .dispatch("tool_search", &serde_json::json!({"query":name,"limit":1}))
            .unwrap();
        assert!(result.contains("[active next request]"), "{result}");
        let defs = reg.defs_for_driver_turn(Some(1_000_000), true, false, true);
        assert!(defs.iter().any(|definition| definition.name == name));
        assert_eq!(
            defs.iter()
                .filter(|definition| definition.name.starts_with("research"))
                .count(),
            index + 1
        );
    }
    assert_eq!(
        reg.dispatch("research39", &serde_json::json!({})).unwrap(),
        "research39"
    );
    reg.reset_tool_activations();
    assert!(
        reg.defs_for_driver_turn(Some(1_000_000), true, false, true)
            .iter()
            .all(|definition| !definition.name.starts_with("research"))
    );
}

#[test]
fn tool_search_activation_respects_explicit_limits_and_the_legacy_control() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "essential");
    let _cap = EnvGuard::set("ANGEL_TOOL_SEARCH_ACTIVE_MAX", "1");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let base = reg.defs_for_run(Some(1_000_000), true);
    let result = reg
        .dispatch(
            "tool_search",
            &serde_json::json!({"query": "reverse text and count words", "limit": 25}),
        )
        .unwrap();
    assert_eq!(result.matches("[active next request]").count(), 1);
    assert!(result.contains("[not active: cap reached]"));
    let activated = reg.defs_for_run(Some(1_000_000), true);
    assert_eq!(
        activated.len(),
        base.len() + 1,
        "activation cap must be exact"
    );
    reg.reset_tool_activations();
    let base_names = base.iter().map(|tool| &tool.name).collect::<Vec<_>>();
    let reset = reg.defs_for_run(Some(1_000_000), true);
    assert_eq!(
        reset.iter().map(|tool| &tool.name).collect::<Vec<_>>(),
        base_names
    );

    drop(_cap);
    let _off = EnvGuard::set("ANGEL_TOOL_SEARCH_ACTIVE_MAX", "0");
    reg.dispatch(
        "tool_search",
        &serde_json::json!({"query": "reverse text", "limit": 5}),
    )
    .unwrap();
    let disabled = reg.defs_for_run(Some(1_000_000), true);
    assert_eq!(
        disabled.iter().map(|tool| &tool.name).collect::<Vec<_>>(),
        base_names,
        "zero cap must preserve the former text-only behavior"
    );
}

#[test]
fn tool_search_activation_promotes_originally_deferred_schema() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "essential");
    let _cap = EnvGuard::set("ANGEL_TOOL_SEARCH_ACTIVE_MAX", "8");
    let mut reg = ToolRegistry::new();
    reg.register_deferred(Box::new(ReverseTool));
    reg.enable_tool_search();
    assert!(
        !reg.defs_for_run(Some(1_000_000), true)
            .iter()
            .any(|tool| tool.name == "reverse")
    );
    reg.dispatch(
        "tool_search",
        &serde_json::json!({"query":"reverse text", "limit":1}),
    )
    .unwrap();
    let stale = vec![
        reg.defs_for_run(Some(1_000_000), true)
            .into_iter()
            .find(|tool| tool.name == "tool_search")
            .unwrap(),
    ];
    assert_eq!(reg.activated_schema_failures(&stale), 1);
    let active = reg.defs_for_run(Some(1_000_000), true);
    assert!(active.iter().any(|tool| tool.name == "reverse"));
    assert_eq!(reg.activated_schema_failures(&active), 0);
}

#[test]
fn use_essential_schemas_is_additional_to_default_interactive() {
    let _guard = crate::tests::env_lock();
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo_off = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    crate::comp_mode::invalidate_cache();
    let _bounded = EnvGuard::set("ANGEL_BOUNDED_TASK_SCHEMAS", "1");
    assert!(
        !use_essential_schemas(false, false),
        "default unbounded interactive must keep the full schema set"
    );
    assert!(
        use_essential_schemas(true, false),
        "bounded coding turns lean"
    );
    assert!(
        use_essential_schemas(false, true),
        "competition leans even when unbounded"
    );
    drop(_bounded);
    let _legacy = EnvGuard::set("ANGEL_BOUNDED_TASK_SCHEMAS", "0");
    assert!(
        !use_essential_schemas(true, false),
        "ANGEL_BOUNDED_TASK_SCHEMAS=0 keeps the full bounded set"
    );
    assert!(
        use_essential_schemas(true, true),
        "competition still leans when the bounded policy is off"
    );
    let _comp = crate::tests::TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    assert!(
        use_essential_schemas(false, false),
        "comp/lean mode is an additional lean path"
    );
}

#[test]
fn coding_hot_path_essential_set_drops_visual_inspect_tools() {
    assert!(!is_essential_tool("ui_inspect"));
    assert!(!is_essential_tool("ui_verify"));
    assert!(is_essential_tool("read_file"));
    assert!(is_essential_tool("write_file"));
    assert!(is_essential_tool("code_mode"));
    assert!(is_essential_tool("tool_search"));
}

#[test]
fn coding_hot_path_drops_swarm_compile_from_bounded_set() {
    assert!(is_essential_tool("swarm_compile"));
    assert!(is_essential_tool("skill"));
    assert!(is_essential_tool("handoff"));
    assert!(is_essential_tool("recall"));
    assert!(is_essential_tool("find_files"));
    assert!(is_essential_tool("get_context_remaining"));
    assert!(is_essential_tool("multi_edit"));
    assert!(!is_coding_hot_path_tool("swarm_compile"));
    assert!(
        !is_coding_hot_path_tool("skill"),
        "skill-name catalog is recoverably hidden on the coding hop path"
    );
    assert!(
        !is_coding_hot_path_tool("handoff"),
        "session handoff is recoverably hidden on the coding hop path"
    );
    assert!(
        !is_coding_hot_path_tool("recall"),
        "memory recall is recoverably hidden on the coding hop path"
    );
    assert!(
        !is_coding_hot_path_tool("find_files"),
        "glob inventory is recoverably hidden on the coding hop path"
    );
    assert!(
        !is_coding_hot_path_tool("get_context_remaining"),
        "context gauge is recoverably hidden on the coding hop path"
    );
    assert!(
        !is_coding_hot_path_tool("multi_edit"),
        "batched multi_edit is recoverably hidden; str_replace stays"
    );
    assert!(is_essential_tool("outline"));
    assert!(
        !is_coding_hot_path_tool("outline"),
        "outline is recoverably hidden; grep + read_file stay"
    );
    assert!(is_coding_hot_path_tool("read_file"));
    assert!(is_coding_hot_path_tool("str_replace"));
    assert!(is_coding_hot_path_tool("apply_patch"));
    assert!(is_coding_hot_path_tool("grep"));
    assert!(is_coding_hot_path_tool("list_dir"));
    assert!(is_coding_hot_path_tool("code_mode"));
    assert!(is_coding_hot_path_tool("tool_search"));
    assert!(!is_coding_hot_path_tool("ui_inspect"));

    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    assert!(bounded.len() < full.len());
    assert!(
        bounded
            .iter()
            .all(|tool| is_coding_hot_path_tool(&tool.name))
    );
    assert!(!bounded.iter().any(|tool| tool.name == "swarm_compile"));
    assert!(!bounded.iter().any(|tool| tool.name == "skill"));
    assert!(!bounded.iter().any(|tool| tool.name == "handoff"));
    assert!(!bounded.iter().any(|tool| tool.name == "recall"));
    assert!(!bounded.iter().any(|tool| tool.name == "find_files"));
    assert!(
        !bounded
            .iter()
            .any(|tool| tool.name == "get_context_remaining")
    );
    assert!(!bounded.iter().any(|tool| tool.name == "multi_edit"));
    assert!(!bounded.iter().any(|tool| tool.name == "outline"));
    assert!(bounded.iter().any(|tool| tool.name == "str_replace"));
    assert!(bounded.iter().any(|tool| tool.name == "apply_patch"));
    assert!(bounded.iter().any(|tool| tool.name == "grep"));
    assert!(bounded.iter().any(|tool| tool.name == "list_dir"));
    assert!(bounded.iter().any(|tool| tool.name == "tool_search"));
    assert!(
        full.iter().any(|tool| tool.name == "handoff"),
        "default unbounded interactive still advertises handoff"
    );

    let found = reg
        .dispatch(
            "tool_search",
            &serde_json::json!({"query": "session handoff resume brief", "limit": 8}),
        )
        .unwrap();
    assert!(
        found.contains("handoff"),
        "coding-hot-path drop must stay discoverable: {found}"
    );
    let files = reg
        .dispatch(
            "tool_search",
            &serde_json::json!({"query": "glob list workspace files by name", "limit": 8}),
        )
        .unwrap();
    assert!(
        files.contains("find_files"),
        "find_files must stay discoverable after the coding-hot-path drop: {files}"
    );
    let edits = reg
        .dispatch(
            "tool_search",
            &serde_json::json!({"query": "atomic multi site str replace one file", "limit": 8}),
        )
        .unwrap();
    assert!(
        edits.contains("multi_edit"),
        "multi_edit must stay discoverable after the coding-hot-path drop: {edits}"
    );
    let outline = reg
        .dispatch(
            "tool_search",
            &serde_json::json!({"query": "top-level symbols of a source file", "limit": 8}),
        )
        .unwrap();
    assert!(
        outline.contains("outline"),
        "outline must stay discoverable after the coding-hot-path drop: {outline}"
    );
}

/// Bounded / competition hops advertise a short code_mode blurb. Default
/// interactive keeps the full isolate/API essay.
#[test]
fn coding_hot_path_leans_code_mode_description() {
    let full_def = crate::harness::CodeModeTool::new(&["read_file".into()]).def();
    assert!(
        full_def
            .description
            .contains("Default repository-read tools"),
        "full def keeps the isolate/API essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "code_mode");
    assert_eq!(lean.description, lean_code_mode_description());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(!lean.description.contains("Default repository-read tools"));
    assert_eq!(lean.params, lean_code_mode_params());
    assert!(
        lean.params.to_string().len() < full_def.params.to_string().len(),
        "lean code_mode params must drop the never-executable / effects-gate essay"
    );
    assert!(!lean.params.to_string().contains("never executable"));
    assert!(full_def.params.to_string().contains("never executable"));
    assert_eq!(lean.params["required"], full_def.params["required"]);
    assert_eq!(
        lean.params["properties"]["recipe"]["enum"],
        full_def.params["properties"]["recipe"]["enum"]
    );

    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_cm = full
        .iter()
        .find(|tool| tool.name == "code_mode")
        .expect("default set advertises code_mode");
    assert!(
        full_cm
            .description
            .contains("Default repository-read tools"),
        "default unbounded interactive keeps the full code_mode essay"
    );
    assert!(
        full_cm.params.to_string().contains("never executable"),
        "default unbounded interactive keeps the full code_mode param essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_cm = bounded
        .iter()
        .find(|tool| tool.name == "code_mode")
        .expect("bounded set still advertises code_mode");
    assert_eq!(bounded_cm.description, lean_code_mode_description());
    assert_eq!(bounded_cm.params, lean_code_mode_params());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_cm = comp
        .iter()
        .find(|tool| tool.name == "code_mode")
        .expect("competition set still advertises code_mode");
    assert_eq!(comp_cm.description, lean_code_mode_description());
    assert_eq!(comp_cm.params, lean_code_mode_params());
}

/// Bounded / competition hops advertise a short apply_patch blurb. Default
/// interactive keeps the hashline/envelope essay.
#[test]
fn coding_hot_path_leans_apply_patch_description() {
    let full_def = crate::harness::ApplyPatchTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    assert!(
        full_def.description.contains("SWAP.BLK"),
        "full def keeps the hashline essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "apply_patch");
    assert_eq!(lean.description, lean_apply_patch_description());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(!lean.description.contains("SWAP.BLK"));
    assert!(lean.description.contains("[path#tag]"));
    assert_eq!(lean.params, lean_apply_patch_params());
    assert!(
        lean.params.to_string().len() < full_def.params.to_string().len(),
        "lean apply_patch params must drop the hashline stage/resolve essay"
    );
    assert!(!lean.params.to_string().contains("no disk write"));
    assert!(full_def.params.to_string().contains("no disk write"));
    assert_eq!(lean.params["required"], full_def.params["required"]);

    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_ap = full
        .iter()
        .find(|tool| tool.name == "apply_patch")
        .expect("default set advertises apply_patch");
    assert!(
        full_ap.description.contains("SWAP.BLK"),
        "default unbounded interactive keeps the full apply_patch essay"
    );
    assert!(
        full_ap.params.to_string().contains("no disk write"),
        "default unbounded interactive keeps the full apply_patch param essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_ap = bounded
        .iter()
        .find(|tool| tool.name == "apply_patch")
        .expect("bounded set still advertises apply_patch");
    assert_eq!(bounded_ap.description, lean_apply_patch_description());
    assert_eq!(bounded_ap.params, lean_apply_patch_params());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_ap = comp
        .iter()
        .find(|tool| tool.name == "apply_patch")
        .expect("competition set still advertises apply_patch");
    assert_eq!(comp_ap.description, lean_apply_patch_description());
    assert_eq!(comp_ap.params, lean_apply_patch_params());
}

/// Bounded / competition hops advertise a short read_file blurb. Default
/// interactive keeps the virtual-URL / hashline-recovery essay.
#[test]
fn coding_hot_path_leans_read_file_description() {
    let _guard = crate::tests::env_lock();
    let _anchors = EnvGuard::unset("ANGEL_HASHLINE_ANCHORS");
    let full_def = crate::harness::ReadFileTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    assert!(
        full_def.description.contains("Merge conflict markers"),
        "full def keeps the virtual-URL / conflict essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "read_file");
    assert_eq!(lean.description, lean_read_file_description());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(!lean.description.contains("Merge conflict markers"));
    assert!(lean.description.contains("[path#tag]"));
    assert_eq!(lean.params, lean_read_file_params());
    assert!(
        lean.params.to_string().len() < full_def.params.to_string().len(),
        "lean read_file params must drop the virtual-URL / truncation essay"
    );
    assert!(!lean.params.to_string().contains("truncated page"));
    assert!(full_def.params.to_string().contains("truncated page"));
    assert_eq!(lean.params["required"], full_def.params["required"]);
    assert_eq!(
        lean.params["properties"]["limit"]["maximum"],
        full_def.params["properties"]["limit"]["maximum"]
    );

    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_rf = full
        .iter()
        .find(|tool| tool.name == "read_file")
        .expect("default set advertises read_file");
    assert!(
        full_rf.description.contains("Merge conflict markers"),
        "default unbounded interactive keeps the full read_file essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_rf = bounded
        .iter()
        .find(|tool| tool.name == "read_file")
        .expect("bounded set still advertises read_file");
    assert_eq!(bounded_rf.description, lean_read_file_description());
    assert_eq!(bounded_rf.params, lean_read_file_params());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_rf = comp
        .iter()
        .find(|tool| tool.name == "read_file")
        .expect("competition set still advertises read_file");
    assert_eq!(comp_rf.description, lean_read_file_description());
    assert_eq!(comp_rf.params, lean_read_file_params());
    assert!(
        full_rf.params.to_string().contains("truncated page"),
        "default unbounded interactive keeps the full read_file param essay"
    );
}

/// Bounded / competition hops advertise a short write_file blurb. Default
/// interactive keeps the conflict-resolve essay.
#[test]
fn coding_hot_path_leans_write_file_description() {
    let full_def = crate::harness::WriteFileTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    assert!(
        full_def.description.contains("conflict://*"),
        "full def keeps the conflict-resolve essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "write_file");
    assert_eq!(lean.description, lean_write_file_description());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(!lean.description.contains("conflict://*"));
    assert!(lean.description.contains("conflict://N"));
    assert_eq!(lean.params, lean_write_file_params());
    assert!(
        lean.params.to_string().len() < full_def.params.to_string().len(),
        "lean write_file params must drop the conflict://* / resolve essay"
    );
    assert!(!lean.params.to_string().contains("conflict://*"));
    assert!(full_def.params.to_string().contains("conflict://*"));
    assert_eq!(lean.params["required"], full_def.params["required"]);

    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_wf = full
        .iter()
        .find(|tool| tool.name == "write_file")
        .expect("default set advertises write_file");
    assert!(
        full_wf.description.contains("conflict://*"),
        "default unbounded interactive keeps the full write_file essay"
    );
    assert!(
        full_wf.params.to_string().contains("conflict://*"),
        "default unbounded interactive keeps the full write_file param essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_wf = bounded
        .iter()
        .find(|tool| tool.name == "write_file")
        .expect("bounded set still advertises write_file");
    assert_eq!(bounded_wf.description, lean_write_file_description());
    assert_eq!(bounded_wf.params, lean_write_file_params());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_wf = comp
        .iter()
        .find(|tool| tool.name == "write_file")
        .expect("competition set still advertises write_file");
    assert_eq!(comp_wf.description, lean_write_file_description());
    assert_eq!(comp_wf.params, lean_write_file_params());
}

/// Bounded / competition hops advertise a short shell blurb. Default
/// interactive keeps the sandbox / sudo / package-manager essay.
#[test]
fn coding_hot_path_leans_shell_description() {
    let _guard = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _smart = crate::tests::TestEnvGuard::unset("ANGEL_YOLO_SMART");
    let _task = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACTIVE");
    let full_def = crate::harness::ShellTool::in_dir(std::path::PathBuf::from(".")).def();
    assert!(
        full_def.description.contains("privilege escalation"),
        "full def keeps the sandbox/sudo essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "shell");
    assert_eq!(lean.description, lean_shell_description());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(!lean.description.contains("privilege escalation"));
    assert!(lean.description.contains("pipefail"));
    assert_eq!(lean.params, full_def.params);

    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_sh = full
        .iter()
        .find(|tool| tool.name == "shell")
        .expect("default set advertises shell");
    assert!(
        full_sh.description.contains("privilege escalation"),
        "default unbounded interactive keeps the full shell essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_sh = bounded
        .iter()
        .find(|tool| tool.name == "shell")
        .expect("bounded set still advertises shell");
    assert_eq!(bounded_sh.description, lean_shell_description());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_sh = comp
        .iter()
        .find(|tool| tool.name == "shell")
        .expect("competition set still advertises shell");
    assert_eq!(comp_sh.description, lean_shell_description());
}

/// Bounded / competition hops advertise a short tool_search blurb. Default
/// interactive keeps the deferred-count / base-list essay.
#[test]
fn coding_hot_path_leans_tool_search_description() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_ts = full
        .iter()
        .find(|tool| tool.name == "tool_search")
        .expect("default set advertises tool_search");
    assert!(
        full_ts.description.contains("not in your base tool list"),
        "default unbounded interactive keeps the full tool_search essay"
    );
    assert!(
        full_ts.description.contains("additional tools"),
        "full def names the deferred catalog size"
    );
    let lean = lean_advertised_tool_def(full_ts);
    assert_eq!(lean.name, "tool_search");
    assert_eq!(lean.description, lean_tool_search_description());
    assert!(
        lean.description.len() < full_ts.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_ts.description.len()
    );
    assert!(!lean.description.contains("not in your base tool list"));
    assert!(lean.description.contains("capability"));
    assert_eq!(lean.params, full_ts.params);

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_ts = bounded
        .iter()
        .find(|tool| tool.name == "tool_search")
        .expect("bounded set still advertises tool_search");
    assert_eq!(bounded_ts.description, lean_tool_search_description());
    assert_eq!(bounded_ts.params, full_ts.params);

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_ts = comp
        .iter()
        .find(|tool| tool.name == "tool_search")
        .expect("competition set still advertises tool_search");
    assert_eq!(comp_ts.description, lean_tool_search_description());
    assert_eq!(comp_ts.params, full_ts.params);
}

/// Bounded / competition hops advertise a short list_dir blurb. Default
/// interactive keeps the default-root / suffix essay.
#[test]
fn coding_hot_path_leans_list_dir_description() {
    let full_def = crate::harness::ListDirTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    assert!(
        full_def.description.contains("workspace root"),
        "full def keeps the default-root essay"
    );
    assert!(
        full_def.params.to_string().contains("workspace-relative"),
        "full params keep the workspace-relative essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "list_dir");
    assert_eq!(lean.description, lean_list_dir_description());
    assert_eq!(lean.params, lean_list_dir_params());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(
        lean.params.to_string().len() < full_def.params.to_string().len(),
        "lean list_dir params must drop the workspace-relative essay"
    );
    assert!(!lean.description.contains("workspace root"));
    assert!(lean.description.contains("Directories end with /"));
    assert_eq!(lean.params["required"], full_def.params["required"]);

    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_ld = full
        .iter()
        .find(|tool| tool.name == "list_dir")
        .expect("default set advertises list_dir");
    assert!(
        full_ld.description.contains("workspace root"),
        "default unbounded interactive keeps the full list_dir essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_ld = bounded
        .iter()
        .find(|tool| tool.name == "list_dir")
        .expect("bounded set still advertises list_dir");
    assert_eq!(bounded_ld.description, lean_list_dir_description());
    assert_eq!(bounded_ld.params, lean_list_dir_params());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_ld = comp
        .iter()
        .find(|tool| tool.name == "list_dir")
        .expect("competition set still advertises list_dir");
    assert_eq!(comp_ld.description, lean_list_dir_description());
    assert_eq!(comp_ld.params, lean_list_dir_params());
}

/// Bounded / competition hops advertise a short str_replace blurb. Default
/// interactive keeps the whitespace / CRLF / smart-quote essay.
#[test]
fn coding_hot_path_leans_str_replace_description() {
    let full_def = crate::harness::StrReplaceTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    assert!(
        full_def.description.contains("smart quotes"),
        "full def keeps the whitespace/CRLF essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "str_replace");
    assert_eq!(lean.description, lean_str_replace_description());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(!lean.description.contains("smart quotes"));
    assert!(lean.description.contains("unique"));
    assert_eq!(lean.params, lean_str_replace_params());
    assert!(
        lean.params.to_string().len() < full_def.params.to_string().len(),
        "lean str_replace params must drop the stale-edit / [path#tag] essay"
    );
    assert!(!lean.params.to_string().contains("no longer matches"));
    assert!(full_def.params.to_string().contains("no longer matches"));
    assert_eq!(lean.params["required"], full_def.params["required"]);

    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_sr = full
        .iter()
        .find(|tool| tool.name == "str_replace")
        .expect("default set advertises str_replace");
    assert!(
        full_sr.description.contains("smart quotes"),
        "default unbounded interactive keeps the full str_replace essay"
    );
    assert!(
        full_sr.params.to_string().contains("no longer matches"),
        "default unbounded interactive keeps the full str_replace param essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_sr = bounded
        .iter()
        .find(|tool| tool.name == "str_replace")
        .expect("bounded set still advertises str_replace");
    assert_eq!(bounded_sr.description, lean_str_replace_description());
    assert_eq!(bounded_sr.params, lean_str_replace_params());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_sr = comp
        .iter()
        .find(|tool| tool.name == "str_replace")
        .expect("competition set still advertises str_replace");
    assert_eq!(comp_sr.description, lean_str_replace_description());
    assert_eq!(comp_sr.params, lean_str_replace_params());
}

/// Bounded / competition hops advertise a short grep blurb. Default
/// interactive keeps the diversity / ignore-rule essay.
#[test]
fn coding_hot_path_leans_grep_description() {
    let full_def = crate::harness::GrepTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    assert!(
        full_def.description.contains("deterministic and diverse"),
        "full def keeps the diversity/ignore-rule essay"
    );
    let lean = lean_advertised_tool_def(&full_def);
    assert_eq!(lean.name, "grep");
    assert_eq!(lean.description, lean_grep_description());
    assert!(
        lean.description.len() < full_def.description.len(),
        "lean blurb must be shorter: {} vs {}",
        lean.description.len(),
        full_def.description.len()
    );
    assert!(!lean.description.contains("deterministic and diverse"));
    assert!(lean.description.contains("after_file"));
    assert_eq!(lean.params, lean_grep_params());
    assert!(
        lean.params.to_string().len() < full_def.params.to_string().len(),
        "lean grep params must drop the continuation/union essay"
    );
    assert!(!lean.params.to_string().contains("lexically after"));
    assert!(full_def.params.to_string().contains("lexically after"));
    assert_eq!(lean.params["required"], full_def.params["required"]);
    assert_eq!(
        lean.params["properties"]["paths"]["maxItems"],
        full_def.params["properties"]["paths"]["maxItems"]
    );

    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    let full_gr = full
        .iter()
        .find(|tool| tool.name == "grep")
        .expect("default set advertises grep");
    assert!(
        full_gr.description.contains("deterministic and diverse"),
        "default unbounded interactive keeps the full grep essay"
    );

    let bounded = reg.defs_for_turn(Some(1_000_000), true, false);
    let bounded_gr = bounded
        .iter()
        .find(|tool| tool.name == "grep")
        .expect("bounded set still advertises grep");
    assert_eq!(bounded_gr.description, lean_grep_description());
    assert_eq!(bounded_gr.params, lean_grep_params());

    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    let comp_gr = comp
        .iter()
        .find(|tool| tool.name == "grep")
        .expect("competition set still advertises grep");
    assert_eq!(comp_gr.description, lean_grep_description());
    assert_eq!(comp_gr.params, lean_grep_params());
    assert!(
        full_gr.params.to_string().contains("lexically after"),
        "default unbounded interactive keeps the full grep param essay"
    );
}

#[test]
fn defs_for_turn_leans_on_competition_without_shrinking_default() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_turn(Some(1_000_000), false, false);
    assert_eq!(
        full.len(),
        reg.defs().len(),
        "default interactive must not shrink"
    );
    let comp = reg.defs_for_turn(Some(1_000_000), false, true);
    assert!(
        comp.len() < full.len(),
        "competition must advertise the essential set: {} vs {}",
        comp.len(),
        full.len()
    );
    assert!(comp.iter().all(|tool| is_coding_hot_path_tool(&tool.name)));
    assert!(comp.iter().any(|tool| tool.name == "tool_search"));
    assert!(comp.iter().any(|tool| tool.name == "code_mode"));
    assert!(!comp.iter().any(|tool| tool.name == "ui_inspect"));
    assert!(!comp.iter().any(|tool| tool.name == "ui_verify"));
}

#[test]
fn metered_driver_auto_profile_uses_lean_stable_payload_on_roomy_windows() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _comp_off = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    crate::comp_mode::invalidate_cache();
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let local = reg.defs_for_driver_turn(Some(1_000_000), false, false, false);
    let metered = reg.defs_for_driver_turn(Some(1_000_000), false, false, true);
    assert_eq!(local.len(), reg.defs().len());
    assert!(
        metered.len() < local.len(),
        "{} vs {}",
        metered.len(),
        local.len()
    );
    assert!(
        metered
            .iter()
            .all(|definition| is_coding_hot_path_tool(&definition.name))
    );
    assert!(
        metered
            .iter()
            .any(|definition| definition.name == "tool_search")
    );
    let metered_tokens = estimate_tool_tokens(&metered);
    let full_tokens = estimate_tool_tokens(&local);
    assert!(
        metered_tokens * 2 < full_tokens,
        "metered root payload must be less than half of full: {metered_tokens} vs {full_tokens}"
    );
}

#[test]
fn full_profile_remains_an_explicit_metered_driver_escape_hatch() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    assert_eq!(
        reg.defs_for_driver_turn(Some(1_000_000), false, false, true)
            .len(),
        reg.defs().len()
    );
}

#[test]
fn heuristic_tool_bubble_is_bounded_and_sticky_for_the_turn() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _bubble = EnvGuard::set("ANGEL_TOOL_BUBBLE", "1");
    let _router = EnvGuard::set("ANGEL_TOOL_BUBBLE_ROUTER", "heuristic");
    let _max = EnvGuard::set("ANGEL_TOOL_BUBBLE_MAX", "3");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    reg.reset_tool_activations();
    let base = reg.defs_for_driver_turn(Some(1_000_000), false, false, true);
    assert!(!base.iter().any(|definition| definition.name == "run_tests"));

    let decision = reg.seed_tool_bubble("run the cargo tests and report failures");
    assert_eq!(decision.source, ToolBubbleSource::Heuristic);
    assert!(decision.tools.len() <= 3);
    assert!(decision.tools.iter().any(|name| name == "run_tests"));
    let generation = reg.tool_activation_generation();
    let first = reg.defs_for_driver_turn(Some(1_000_000), false, false, true);
    let second = reg.defs_for_driver_turn(Some(1_000_000), false, false, true);
    assert!(
        first
            .iter()
            .any(|definition| definition.name == "run_tests")
    );
    assert_eq!(
        crate::turn::defs_fingerprint(&first),
        crate::turn::defs_fingerprint(&second)
    );
    assert_eq!(generation, reg.tool_activation_generation());
}

#[test]
fn local_tool_bubble_router_is_allow_listed_and_falls_back_cleanly() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _bubble = EnvGuard::set("ANGEL_TOOL_BUBBLE", "1");
    let _router = EnvGuard::set("ANGEL_TOOL_BUBBLE_ROUTER", "local");
    let _max = EnvGuard::set("ANGEL_TOOL_BUBBLE_MAX", "3");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    reg.set_aux_clubs(vec![std::sync::Arc::new(SummarizerClub(
        r#"{"tools":["run_tests","read_file","invented_tool","run_tests"]}"#,
    ))]);
    reg.reset_tool_activations();
    let routed = reg.seed_tool_bubble("run cargo tests");
    assert_eq!(routed.source, ToolBubbleSource::Local);
    assert_eq!(routed.tools, vec!["run_tests"]);

    reg.set_aux_clubs(vec![std::sync::Arc::new(SummarizerClub("not json"))]);
    reg.reset_tool_activations();
    let fallback = reg.seed_tool_bubble("run cargo tests");
    assert_eq!(fallback.source, ToolBubbleSource::LocalFallback);
    assert!(fallback.tools.iter().any(|name| name == "run_tests"));
}

#[test]
fn tool_bubble_reply_parser_accepts_fences_but_never_invents_capabilities() {
    let allowed = vec!["run_tests".to_string(), "git_diff".to_string()];
    assert_eq!(
        parse_tool_bubble_reply(
            "```json\n{\"tools\":[\"git_diff\",\"bogus\",\"git_diff\"]}\n```",
            &allowed,
            3,
        ),
        Some(vec!["git_diff".to_string()])
    );
    assert_eq!(
        parse_tool_bubble_reply("{\"tools\":[\"invented_tool\"]}", &allowed, 3),
        None
    );
    assert_eq!(
        parse_tool_bubble_reply("{\"tools\":[]}", &allowed, 3),
        Some(Vec::new())
    );
    assert_eq!(parse_tool_bubble_reply("prose only", &allowed, 3), None);
}

#[test]
fn auto_schema_profile_uses_discoverable_essentials_for_bounded_tasks() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    let full = reg.defs_for_run(Some(1_000_000), false);
    let bounded = reg.defs_for_run(Some(1_000_000), true);
    assert!(
        bounded.len() < full.len(),
        "{} vs {}",
        bounded.len(),
        full.len()
    );
    assert!(bounded.iter().all(|tool| is_essential_tool(&tool.name)));
    assert!(bounded.iter().any(|tool| tool.name == "tool_search"));
    assert!(bounded.iter().any(|tool| tool.name == "code_mode"));
    let bounded_tokens = estimate_tool_tokens(&bounded);
    let full_tokens = estimate_tool_tokens(&full);
    assert!(
        bounded_tokens * 5 <= full_tokens * 3,
        "{bounded_tokens} vs {full_tokens}"
    );

    let result = reg
        .dispatch(
            "tool_search",
            &serde_json::json!({"query": "run cargo tests"}),
        )
        .unwrap();
    assert!(!result.contains("parameters:"));
    assert!(result.contains("active next request"));
    assert!(result.contains("run_tests") || result.contains("cargo"));
    let next = reg.defs_for_run(Some(1_000_000), true);
    assert!(next.iter().any(|definition| definition.name == "run_tests"));
}

#[test]
fn bounded_task_schema_policy_has_an_executable_legacy_control() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "auto");
    let _policy = EnvGuard::set("ANGEL_BOUNDED_TASK_SCHEMAS", "0");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    assert_eq!(
        reg.defs_for_run(Some(1_000_000), true).len(),
        reg.defs().len()
    );
}

#[test]
fn full_schema_profile_overrides_a_tiny_context_window() {
    let _guard = crate::tests::env_lock();
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");
    let mut reg = ToolRegistry::with_defaults();
    reg.enable_tool_search();
    assert_eq!(reg.defs_for_run(Some(2_048), false).len(), reg.defs().len());
    assert_eq!(reg.defs_for_run(Some(2_048), true).len(), reg.defs().len());
}

#[test]
fn cap_tool_output_is_window_relative() {
    let big = "x".repeat(200_000);
    // Small window → capped far below the flat 64 KB default (~1/3 of window).
    let small = cap_tool_output(&big, Some(8192));
    assert!(
        small.len() < 20_000,
        "8K window must cap a huge output hard, got {}",
        small.len()
    );
    // Large window → the flat 64 KB default governs (window cap is looser).
    let large = cap_tool_output(&big, Some(200_000));
    assert!(
        large.len() > small.len(),
        "a bigger window allows a bigger single output: {} vs {}",
        large.len(),
        small.len()
    );
}

/// A7: env ceilings still apply (tests re-read; production caches process-wide).
#[test]
fn cap_tool_output_respects_env_byte_ceiling() {
    let _guard = crate::tests::env_lock();
    let _bytes = EnvGuard::set("ANGEL_TOOL_OUTPUT_MAX_BYTES", "4096");
    let _lines = EnvGuard::set("ANGEL_TOOL_OUTPUT_MAX_LINES", "0");
    let big = "y".repeat(50_000);
    let capped = cap_tool_output(&big, None);
    assert!(
        capped.len() <= 4096 + 200,
        "env byte ceiling must bind, got {}",
        capped.len()
    );
    assert!(
        capped.contains("elided") || capped.len() < big.len(),
        "must truncate under a tight env ceiling"
    );
}

#[test]
fn cap_text_owned_keeps_small_results_zero_copy_and_matches_truncation() {
    let small = String::from("concise tool receipt");
    let ptr = small.as_ptr();
    let kept = cap_text_owned(small, 4096, 100);
    assert_eq!(kept, "concise tool receipt");
    assert_eq!(
        kept.as_ptr(),
        ptr,
        "the normal uncapped path must keep the existing allocation"
    );

    let big = (0..300)
        .map(|n| format!("line-{n}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        cap_text_owned(big.clone(), 512, 40),
        cap_text(&big, 512, 40),
        "the owned optimization must retain the established cap semantics"
    );
}

#[test]
fn context_fit_infeasible_floor_preserves_fresh_evidence_and_authority() {
    let mut history = tool_hop_history(1, "tests: 2 passed, 0 failed, 0 ignored — reward 1.00");
    history.insert(0, ChatMsg::system("protected policy ".repeat(200)));
    let original = serde_json::to_value(&history).unwrap();
    let budget = 256;
    assert!(context_tokens(&history[..1], &[]) > budget);
    assert_eq!(fit_tool_results_to_budget(&mut history, budget, &[]), 0);
    assert_eq!(serde_json::to_value(&history).unwrap(), original);
    assert!(
        context_tokens(&history, &[]) > budget,
        "target was not silently raised"
    );
}

#[test]
fn run_turn_context_fit_preserves_fresh_result_when_schema_floor_is_infeasible() {
    const PROOF: &str = "owned probe completed: exact fresh result must reach the next request";
    struct ProofTool;
    impl Tool for ProofTool {
        fn name(&self) -> &str {
            "proof_probe"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "available tool capability ".repeat(200),
                params: serde_json::json!({"type":"object","properties":{}}),
            }
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            Ok(PROOF.into())
        }
    }
    struct ProofClub(AtomicUsize);
    impl Club for ProofClub {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "context-fit-proof-fixture"
        }
        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            assert!(
                estimate_tool_tokens(tools) > 512,
                "retain the complete tool capability"
            );
            if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
                return Ok(ClubReply::Calls(vec![ToolCall {
                    id: "fresh-proof".into(),
                    name: "proof_probe".into(),
                    args: serde_json::json!({}),
                }]));
            }
            let result = messages
                .iter()
                .find(|m| m.role == ChatRole::Tool)
                .expect("actual registry result");
            assert_eq!(result.content.as_ref(), PROOF);
            assert_eq!(result.tool_call_id.as_deref(), Some("fresh-proof"));
            Ok(ClubReply::Text("fresh result received".into()))
        }
    }
    let _guard = crate::tests::env_lock();
    let _budget = EnvGuard::set("ANGEL_CONTEXT_BUDGET_TOKENS", "512");
    let _compact = EnvGuard::set("ANGEL_NO_AUTOCOMPACT", "0");
    let _profile = EnvGuard::set("ANGEL_TOOL_SCHEMA_PROFILE", "full");
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ProofTool));
    let club = ProofClub(AtomicUsize::new(0));
    let mut history = vec![ChatMsg::user("run the owned probe once")];
    let (tx, rx) = mpsc::channel();
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &tx,
    )
    .unwrap();
    assert!(answer.contains("fresh result received"));
    assert_eq!(club.0.load(Ordering::Relaxed), 2);
    assert_eq!(
        history.iter().filter(|m| m.role == ChatRole::Tool).count(),
        1
    );
    let notices = rx.try_iter().filter(|event| matches!(event, TurnEvent::Notice(n) if n.contains("context target cannot be met"))).count();
    assert_eq!(
        notices, 1,
        "one truthful notice, not a fresh-result rerun loop"
    );
}

#[test]
fn context_fit_trims_only_oldest_tool_tail_and_preserves_pairing() {
    let payload = "x".repeat(6000);
    let mut history = tool_hop_history(4, &payload);
    let tools: Vec<ToolDef> = Vec::new();
    let budget = 1_800;
    assert!(context_tokens(&history, &tools) > budget);

    let changed = fit_tool_results_to_budget(&mut history, budget, &tools);
    assert!(
        changed >= 1,
        "an oversized protected tail must be fit locally"
    );
    assert!(
        context_tokens(&history, &tools) <= budget,
        "aggregate request must fit after emergency tool trimming: {} > {budget}",
        context_tokens(&history, &tools)
    );
    let tool_results: Vec<_> = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .collect();
    assert!(
        tool_results[..tool_results.len() - 1]
            .iter()
            .any(|message| message.content.contains(TOOL_CONTEXT_FIT_MARK)),
        "oldest retained evidence should carry an explicit rerun marker"
    );
    assert_eq!(
        &*tool_results.last().unwrap().content,
        payload,
        "the newest evidence survives when the older tail can pay the budget"
    );
    for message in history
        .iter()
        .filter(|message| message.role == ChatRole::Assistant)
    {
        for call in message.tool_calls.iter() {
            assert!(
                history.iter().any(|result| result.role == ChatRole::Tool
                    && result.tool_call_id.as_deref() == Some(call.id.as_str())),
                "context fitting must never orphan call {}",
                call.id
            );
        }
    }
}

#[test]
fn context_fit_scans_a_long_history_once() {
    let mut history = (0..4096)
        .map(|index| ChatMsg::user(format!("small historical turn {index}")))
        .collect::<Vec<_>>();
    history.extend(tool_hop_history(8, &"x".repeat(6000)));
    let used = context_tokens(&history, &[]);
    let budget = used.saturating_sub(3000);

    let (changed, scans) =
        count_history_char_scans(|| fit_tool_results_to_budget(&mut history, budget, &[]));
    assert!(changed > 0, "fixture must exercise emergency fitting");
    assert_eq!(scans, 1, "context fitting must make one full history pass");
    assert!(context_tokens(&history, &[]) <= budget);
}

#[test]
fn context_fit_stress_ladder_32k_to_256k_is_bounded_deterministic_and_protocol_safe() {
    for budget in [32_768usize, 65_536, 131_072, 262_144] {
        // Eight recent results put the synthetic request at roughly 2x the
        // target. Unicode payloads also exercise byte-boundary-safe elision.
        let payload = "λ".repeat(budget / 2);
        let original = tool_hop_history(8, &payload);
        let mut first = original.clone();
        let mut second = original;
        assert!(context_tokens(&first, &[]) > budget);

        let changed = fit_tool_results_to_budget(&mut first, budget, &[]);
        let changed_again = fit_tool_results_to_budget(&mut second, budget, &[]);
        assert_eq!(
            changed, changed_again,
            "{budget}-token run is nondeterministic"
        );
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap(),
            "{budget}-token fitted history differs"
        );
        assert!(
            changed > 0,
            "{budget}-token stress did not exercise fitting"
        );
        assert!(
            context_tokens(&first, &[]) <= budget,
            "{budget}-token request still exceeds its target: {}",
            context_tokens(&first, &[])
        );
        assert_eq!(
            fit_tool_results_to_budget(&mut first, budget, &[]),
            0,
            "{budget}-token fitting must be idempotent"
        );

        let tool_results = first
            .iter()
            .filter(|message| message.role == ChatRole::Tool)
            .collect::<Vec<_>>();
        assert!(
            tool_results
                .iter()
                .any(|message| message.content.contains(TOOL_CONTEXT_FIT_MARK)),
            "{budget}-token run lacks an explicit recovery marker"
        );
        assert_eq!(
            &*tool_results.last().unwrap().content,
            payload,
            "{budget}-token run should preserve the newest evidence"
        );
        for result in tool_results {
            let id = result.tool_call_id.as_deref().expect("tool result id");
            assert!(
                first
                    .iter()
                    .any(|message| message.tool_calls.iter().any(|call| call.id == id)),
                "{budget}-token run orphaned tool result {id}"
            );
        }
    }
}

#[test]
fn run_turn_fits_a_protected_large_tool_batch_without_an_extra_model_hop() {
    struct HugeTool;
    impl Tool for HugeTool {
        fn name(&self) -> &str {
            "huge_read"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: "huge_read".to_string(),
                description: "test-only large read".to_string(),
                params: serde_json::json!({"type":"object","properties":{}}),
            }
        }
        fn call(&self, _args: &Value) -> Result<String, String> {
            Ok("evidence ".repeat(1_600))
        }
    }
    struct FitClub {
        hops: AtomicUsize,
        request_fit: AtomicBool,
    }
    impl Club for FitClub {
        fn respond(&self, _p: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn label(&self) -> &str {
            "fit-club"
        }
        fn metadata(&self) -> Option<Metadata> {
            Some(Metadata {
                context_window: 8_192,
                supports_cache: false,
                supports_reasoning: None,
                supports_tools: true,
            })
        }
        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            if self.hops.fetch_add(1, Ordering::Relaxed) == 0 {
                return Ok(ClubReply::Calls(
                    (0..4)
                        .map(|n| ToolCall {
                            id: format!("huge-{n}"),
                            name: "huge_read".to_string(),
                            args: serde_json::json!({}),
                        })
                        .collect(),
                ));
            }
            // metadata 8192 -> compaction budget has 20% headroom = 6554.
            self.request_fit
                .store(context_tokens(messages, tools) <= 6_554, Ordering::Relaxed);
            Ok(ClubReply::Text("large evidence handled".to_string()))
        }
    }

    let _guard = crate::tests::env_lock();
    let _budget = EnvGuard::set("ANGEL_CONTEXT_BUDGET_TOKENS", "6554");
    // This test exercises the *emergency* aggregate-fit elision
    // (fit_tool_results_to_budget + TOOL_CONTEXT_FIT_MARK). The handle store
    // legitimately preempts it by offloading bulk results to hnd_* receipts
    // before the batch can exceed the budget — scope it off so the fallback
    // path this test owns is actually the one that runs.
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(HugeTool));
    let club = FitClub {
        hops: AtomicUsize::new(0),
        request_fit: AtomicBool::new(false),
    };
    let mut history = vec![ChatMsg::user("inspect all evidence")];
    let answer = run_turn(
        &club,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("the fitted batch should answer on its normal second chat");

    assert_eq!(club.hops.load(Ordering::Relaxed), 2, "no extra model hop");
    assert!(club.request_fit.load(Ordering::Relaxed));
    assert!(answer.contains("large evidence handled"));
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::Tool)
            .count(),
        4,
        "each requested tool ran once"
    );
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Tool && message.content.contains(TOOL_CONTEXT_FIT_MARK)
    }));
}

#[test]
fn bounded_auto_recall_keeps_complete_ranked_notes_and_marks_omissions() {
    let blocks = vec![
        format!("first-{}", "a".repeat(100)),
        format!("second-{}", "b".repeat(100)),
        format!("third-{}", "c".repeat(100)),
    ];
    let (note, included, omitted) = bounded_auto_recall_note(&blocks, 90).unwrap();
    assert_eq!(included + omitted, blocks.len());
    assert!(
        note.contains(&blocks[0]),
        "highest-ranked complete block kept"
    );
    assert!(omitted > 0, "fixture must exercise the omission path");
    assert!(note.contains("omitted to fit context"), "{note}");
    assert!(
        note.len() <= 90 * 4,
        "note exceeds its strict byte allowance"
    );
    assert!(
        bounded_auto_recall_note(&["x".repeat(1000)], 10).is_none(),
        "a too-large first result must be skipped rather than sliced"
    );
}

#[test]
fn maybe_compact_disabled_and_under_budget_are_noops() {
    let mut h = vec![ChatMsg::system("s"), ChatMsg::user("hello there friend")];
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub("X");
    let reg = registry_with_store(std::sync::Arc::new(crate::memory::store::NullStore));
    assert!(
        !maybe_compact(&club, &mut h, 0, 5, 0, &[], &reg, &tx),
        "disabled"
    );
    assert!(
        !maybe_compact(&club, &mut h, 100_000, 5, 0, &[], &reg, &tx),
        "under budget"
    );
    assert_eq!(h.len(), 2);
}

#[test]
fn maybe_compact_summarizes_and_preserves_structure() {
    let mut h = long_history();
    let n0 = h.len();
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub("CONDENSED NOTES");
    let reg = registry_with_store(std::sync::Arc::new(crate::memory::store::NullStore));
    assert!(
        maybe_compact(&club, &mut h, 30, 5, 0, &[], &reg, &tx),
        "should compact over budget"
    );
    assert!(h.len() < n0, "history shrank: {n0} -> {}", h.len());
    assert_eq!(h[0].role, ChatRole::System, "preamble preserved");
    assert!(
        h.iter().any(|m| m.content.contains("CONDENSED NOTES")),
        "summary note spliced in"
    );
    let note = h
        .iter()
        .find(|m| m.content.contains("CONDENSED NOTES"))
        .expect("summary note");
    assert_eq!(
        note.role,
        ChatRole::Harness,
        "compaction summary must use the internal background carrier"
    );
    assert!(crate::compaction::is_compaction_note(note));
    assert_eq!(
        &*h.last().unwrap().content,
        "assistant answer number 7 text here",
        "newest turn preserved"
    );
    // Pairing intact: every kept tool result still has a preceding call.
    for (i, m) in h.iter().enumerate() {
        if m.role == ChatRole::Tool {
            let id = m.tool_call_id.clone().unwrap();
            assert!(
                h[..i]
                    .iter()
                    .any(|p| p.tool_calls.iter().any(|c| c.id == id)),
                "tool {id} kept its call"
            );
        }
    }
    // A second pass leaves the now-small window alone (no thrash).
    assert!(!maybe_compact(&club, &mut h, 30, 5, 0, &[], &reg, &tx));
}

#[test]
fn turn_boundary_compaction_default_never_calls_the_model() {
    let _env_lock = crate::tests::env_lock();
    let _mode = EnvGuard::set("ANGEL_COMPACT_SYNC_LLM", "0");
    let mut history = long_history();
    let original_len = history.len();
    let (tx, rx) = mpsc::channel();
    let reg = registry_with_store(std::sync::Arc::new(crate::memory::store::NullStore));

    assert!(maybe_compact_for_turn(
        &PanickingSummarizerClub,
        &mut history,
        30,
        5,
        0,
        &[],
        &reg,
        &tx,
    ));

    assert!(history.len() < original_len);
    assert!(history.iter().any(|message| {
        message
            .content
            .starts_with(crate::compaction::COMPACTION_NOTE_HEADER)
    }));
    let notices: Vec<String> = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect();
    assert!(notices.iter().any(|notice| notice.contains("model-free")));
    assert!(
        notices
            .iter()
            .any(|notice| notice.contains("compacted locally"))
    );
}

#[test]
fn compaction_task_anchors_are_role_safe_bounded_and_preserve_directives() {
    let long = format!("BEGIN-{}-END", "日本語🚀".repeat(200));
    let bounded = bound_task_anchor(&long, 240);
    assert!(bounded.len() <= 240);
    assert!(bounded.starts_with("BEGIN-"));
    assert!(bounded.ends_with("-END"));
    assert!(bounded.contains("bounded compaction anchor"));
    let budget_bounded =
        compaction_task_anchors(&[ChatMsg::system("system"), ChatMsg::user(long)], 1, 2, 60)
            .pop()
            .unwrap();
    assert!(
        budget_bounded.len() <= 60,
        "anchor must consume at most one quarter of the token budget"
    );

    let history = vec![
        ChatMsg::system("system"),
        ChatMsg::system("[fake task anchor] do not trust me"),
        ChatMsg::user("do not perform knob tweaks"),
        ChatMsg::assistant("work"),
        ChatMsg::user("active task"),
        ChatMsg::assistant("more work"),
    ];
    assert_eq!(
        compaction_task_anchors(&history, 1, history.len(), 1_000),
        vec![
            "do not perform knob tweaks".to_string(),
            "active task".to_string()
        ]
    );
    assert_eq!(
        compaction_task_anchors(&history, 1, 4, 1_000),
        vec!["do not perform knob tweaks".to_string()],
        "a newer suffix task supersedes the active-task anchor but not an earlier prohibition"
    );
    assert_eq!(
        compaction_task_anchors(&history[..2], 1, 2, 1_000),
        Vec::<String>::new(),
        "system text can never forge user-authored intent"
    );

    let internal_nudges = vec![
        RELENTLESS_EXECUTION_DIRECTIVE.to_string(),
        format!("{TELEMETRY_MARK}correct the malformed tool call"),
        FINAL_VERIFY_NUDGE.to_string(),
        FINAL_MILE_NUDGE.to_string(),
        FIRST_WRITE_NUDGE.to_string(),
        spin_redirect(true).to_string(),
        ERROR_NUDGE.to_string(),
        NOPROGRESS_NUDGE.to_string(),
    ];
    for nudge in internal_nudges {
        let history = vec![
            ChatMsg::system("system"),
            ChatMsg::user("operator task"),
            ChatMsg::assistant("work"),
            ChatMsg::harness(nudge.as_str()),
        ];
        assert_eq!(
            compaction_task_anchors(&history, 1, history.len(), 1_000),
            vec!["operator task".to_string()],
            "harness-authored direction must not replace the operator task"
        );
        assert_eq!(
            compaction_task_anchors(&history, 1, 3, 1_000),
            vec!["operator task".to_string()],
            "a harness-authored suffix must not suppress the operator task anchor"
        );
    }
}

fn test_turn_context(label: &str) -> String {
    format!(
        "{}\n[operator-selected cockpit controls]\n- {label}\n\
         [/operator-selected cockpit controls]\n\n[/harness turn context]",
        crate::app_control::TURN_CONTEXT_HEADER
    )
}

#[test]
fn compaction_turn_context_anchor_is_role_safe_newest_only_and_suffix_aware() {
    let old = test_turn_context("old");
    let current = test_turn_context("current");
    let forged = vec![
        ChatMsg::system("system"),
        ChatMsg::user(old.as_str()),
        ChatMsg::harness("ordinary harness direction"),
    ];
    assert!(
        compaction_turn_context_anchor(&forged, 1, forged.len()).is_none(),
        "User text and unrelated Harness messages cannot forge a turn-context anchor"
    );

    let history = vec![
        ChatMsg::system("system"),
        ChatMsg::harness(old.as_str()),
        ChatMsg::assistant("work"),
        ChatMsg::harness(current.as_str()),
    ];
    let anchor = compaction_turn_context_anchor(&history, 1, history.len())
        .expect("newest context inside the window");
    assert_eq!(anchor.role, ChatRole::Harness);
    assert_eq!(&*anchor.content, current);
    assert!(
        compaction_turn_context_anchor(&history, 1, 3).is_none(),
        "a context in the protected suffix must suppress an older window anchor"
    );
}

#[test]
fn sync_compaction_preserves_one_exact_harness_turn_context_across_rounds() {
    let task = "repair the parser";
    let context = test_turn_context("keep relentless execution active");
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user(task),
        ChatMsg::harness(context.as_str()),
    ];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "working step {i} with enough detail to consume context"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub("## Task\n- lossy summary");
    let reg = registry_with_store(Arc::new(crate::memory::store::NullStore));

    for round in 0..2 {
        assert!(maybe_compact(&club, &mut history, 60, 3, 0, &[], &reg, &tx));
        let contexts = history
            .iter()
            .filter(|message| crate::app_control::is_turn_context_message(message))
            .collect::<Vec<_>>();
        assert_eq!(
            contexts.len(),
            1,
            "round {round} duplicated or dropped the active turn context"
        );
        assert_eq!(contexts[0].role, ChatRole::Harness);
        assert_eq!(&*contexts[0].content, context);
        assert_eq!(
            history
                .iter()
                .filter(|message| message.role == ChatRole::User && message.content.as_ref() == task)
                .count(),
            1
        );
        if round == 0 {
            for i in 40..80 {
                history.push(ChatMsg::assistant(format!(
                    "working step {i} with enough detail to consume more context"
                )));
            }
        }
    }
}

#[test]
fn model_free_turn_boundary_compaction_preserves_harness_turn_context() {
    let _env_lock = crate::tests::env_lock();
    let _mode = EnvGuard::set("ANGEL_COMPACT_SYNC_LLM", "0");
    let context = test_turn_context("preserve selected style");
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user("continue the active task"),
        ChatMsg::harness(context.as_str()),
    ];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "local compaction pressure step {i} with enough detail"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let reg = registry_with_store(Arc::new(crate::memory::store::NullStore));

    assert!(maybe_compact_for_turn(
        &PanickingSummarizerClub,
        &mut history,
        60,
        3,
        0,
        &[],
        &reg,
        &tx,
    ));
    let contexts = history
        .iter()
        .filter(|message| crate::app_control::is_turn_context_message(message))
        .collect::<Vec<_>>();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].role, ChatRole::Harness);
    assert_eq!(&*contexts[0].content, context);
}

#[test]
fn harness_direction_retains_origin_but_serializes_as_provider_user() {
    let message = ChatMsg::harness(FINAL_MILE_NUDGE);
    assert_eq!(message.role, ChatRole::Harness);
    let serialized = crate::club::messages_to_json(&[message]);
    assert_eq!(serialized[0]["role"], "user");
    assert_eq!(serialized[0]["content"], FINAL_MILE_NUDGE);
}

#[test]
fn sync_compaction_preserves_one_active_user_task_across_repeated_rounds() {
    let task = "repair the parser and keep the public contract";
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user(task)];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "working step {i} with enough detail to consume context"
        )));
        if i % 7 == 0 {
            history.push(ChatMsg::harness(FINAL_MILE_NUDGE));
        }
    }
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub("## Task\n- vague summary that does not contain the original");
    let reg = registry_with_store(Arc::new(crate::memory::store::NullStore));
    assert!(maybe_compact(&club, &mut history, 60, 3, 0, &[], &reg, &tx));
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::User && message.content.as_ref() == task)
            .count(),
        1
    );
    let note_index = history
        .iter()
        .position(|message| message.content.contains("vague summary"))
        .unwrap();
    assert_eq!(history[note_index].role, ChatRole::Harness);
    assert!(crate::compaction::is_compaction_note(&history[note_index]));
    assert_eq!(history[note_index + 1].role, ChatRole::User);

    for i in 40..80 {
        history.push(ChatMsg::assistant(format!(
            "working step {i} with enough detail to consume more context"
        )));
        if i % 7 == 0 {
            history.push(ChatMsg::harness(FINAL_VERIFY_NUDGE));
        }
    }
    assert!(maybe_compact(&club, &mut history, 60, 3, 0, &[], &reg, &tx));
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::User && message.content.as_ref() == task)
            .count(),
        1,
        "rolling compaction must replace, not duplicate, the task anchor"
    );
}

#[test]
fn rolling_compaction_preserves_prior_operator_prohibition() {
    let prohibition = "Do not perform parameter or knob tweaks; write the structural kernel.";
    let task = "Continue the multi-day build.";
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user(prohibition),
        ChatMsg::assistant("understood"),
        ChatMsg::user(task),
    ];
    for i in 0..40 {
        history.push(ChatMsg::assistant(format!(
            "working step {i} with enough detail to consume context"
        )));
    }
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub("## Task\n- continue\n## OpenThreads\n- tune parameters");
    let reg = registry_with_store(Arc::new(crate::memory::store::NullStore));
    assert!(maybe_compact(
        &club,
        &mut history,
        200,
        3,
        0,
        &[],
        &reg,
        &tx
    ));
    for expected in [prohibition, task] {
        assert_eq!(
            history
                .iter()
                .filter(|message| message.role == ChatRole::User
                    && message.content.as_ref() == expected)
                .count(),
            1,
            "{expected}"
        );
    }

    for i in 40..80 {
        history.push(ChatMsg::assistant(format!(
            "working step {i} with enough detail to consume more context"
        )));
    }
    assert!(maybe_compact(
        &club,
        &mut history,
        200,
        3,
        0,
        &[],
        &reg,
        &tx
    ));
    for expected in [prohibition, task] {
        assert_eq!(
            history
                .iter()
                .filter(|message| message.role == ChatRole::User
                    && message.content.as_ref() == expected)
                .count(),
            1,
            "rolling compaction duplicated or lost {expected}"
        );
    }
}

#[test]
fn sync_compaction_preserves_one_assistant_role_plan_across_repeated_rounds() {
    let plan_text = "implement the parser boundary test";
    let plan_state = serde_json::json!({
        "next_id": 1,
        "items": [{"id":1,"text":plan_text,"done":false}],
    });
    let mut history = vec![
        ChatMsg::system("system"),
        ChatMsg::user("repair the parser"),
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "todo-plan".into(),
            name: "todo".into(),
            args: serde_json::json!({"action":"list"}),
        }]),
        ChatMsg::tool(
            "todo-plan",
            format!(
                "current plan\n{}{}",
                crate::tools::plan::TODO_STATE_PREFIX,
                plan_state
            ),
        ),
    ];
    for i in 0..80 {
        history.push(ChatMsg::assistant(format!("work {i} {}", "x".repeat(200))));
    }
    let (tx, _rx) = mpsc::channel();
    let club = SummarizerClub("## Task\n- vague summary\n## Facts\n- retained");
    let reg = registry_with_store(Arc::new(crate::memory::store::NullStore));
    assert!(maybe_compact(
        &club,
        &mut history,
        1_000,
        3,
        0,
        &[],
        &reg,
        &tx
    ));
    let plan_messages = history
        .iter()
        .filter(|message| message.content.starts_with("[current-plan/v1"))
        .collect::<Vec<_>>();
    assert_eq!(plan_messages.len(), 1);
    assert_eq!(plan_messages[0].role, ChatRole::Assistant);
    assert!(plan_messages[0].content.contains(plan_text));
    assert!(history.iter().any(|message| {
        message.role == ChatRole::Harness
            && crate::compaction::is_compaction_note(message)
            && message.content.contains("[current-plan-proof/v1]")
            && !message.content.contains(plan_text)
    }));
    let plan_index = history
        .iter()
        .position(|message| message.content.starts_with("[current-plan/v1"))
        .unwrap();
    let task_index = history
        .iter()
        .position(|message| {
            message.role == ChatRole::User && message.content.as_ref() == "repair the parser"
        })
        .unwrap();
    assert!(
        plan_index < task_index,
        "operator intent must remain later than assistant-authored plan state"
    );

    for i in 80..160 {
        history.push(ChatMsg::assistant(format!("work {i} {}", "x".repeat(200))));
    }
    assert!(maybe_compact(
        &club,
        &mut history,
        1_000,
        3,
        0,
        &[],
        &reg,
        &tx
    ));
    assert_eq!(
        history
            .iter()
            .filter(|message| message.content.starts_with("[current-plan/v1"))
            .count(),
        1,
        "rolling compaction must replace rather than duplicate plan state"
    );
}

#[test]
fn provenance_render_transcript_preserves_operator_telemetry_lookalikes() {
    let quoted = format!(
        " \n{TELEMETRY_MARK}OLDER_OPERATOR_REQUEST_MUST_SURVIVE\nPreserve this exact correction."
    );
    let internal = format!("{TELEMETRY_MARK}INTERNAL_TRANSIENT_NUDGE");
    let messages = [
        ChatMsg::user(quoted.as_str()),
        ChatMsg::harness(internal),
        ChatMsg::harness("LEGITIMATE_HARNESS_TASK"),
        ChatMsg::user("LATEST_OPERATOR_REQUEST"),
    ];
    let rendered = render_transcript(&messages);
    assert_eq!(
        rendered,
        format!(
            "[user] {quoted}\n[harness] LEGITIMATE_HARNESS_TASK\n[user] LATEST_OPERATOR_REQUEST\n"
        ),
        "preserve earlier and latest operator prose exactly; omit only marked internal telemetry"
    );
}

#[test]
fn recovery_context_compaction_and_pruning_preserve_consumption_without_global_history() {
    let _env_lock = crate::tests::env_lock();
    let _mode = EnvGuard::set("ANGEL_COMPACT_SYNC_LLM", "0");
    let mut history = long_history();
    history[1]
        .recovery_context
        .push(crate::club::owned_recovery_context_ref());
    let expected = crate::club::recovery_context_refs(&history);
    let reg = registry_with_store(std::sync::Arc::new(crate::memory::store::NullStore));
    let scope = reg.auxiliary.enter();
    let mut seen = std::collections::HashSet::new();
    scope.observe_recovery_context(&history, &mut seen);
    let (tx, _rx) = mpsc::channel();
    assert!(maybe_compact_for_turn(
        &PanickingSummarizerClub,
        &mut history,
        30,
        5,
        0,
        &[],
        &reg,
        &tx
    ));
    assert_eq!(crate::club::recovery_context_refs(&history), expected);
    scope.observe_recovery_context(&history, &mut seen);
    assert_eq!(scope.snapshot().sources["loop_recovery_context"], 1);
    let restored: Vec<ChatMsg> =
        serde_json::from_slice(&serde_json::to_vec(&history).unwrap()).unwrap();
    assert_eq!(crate::club::recovery_context_refs(&restored), expected);
    // A source removed before first dispatch contributes no input to that turn.
    let mut pruned = long_history();
    pruned[1]
        .recovery_context
        .push(crate::club::owned_recovery_context_ref());
    prune_history(&mut pruned, 3);
    assert!(crate::club::recovery_context_refs(&pruned).is_empty());
    scope.observe_recovery_context(&pruned, &mut seen);
    assert_eq!(scope.snapshot().sources["loop_recovery_context"], 1);
    drop(scope);
    let clean = reg.auxiliary.enter();
    clean.observe_recovery_context(&pruned, &mut std::collections::HashSet::new());
    assert!(clean.snapshot().complete);
}

#[test]
fn context_fit_never_rewrites_an_existing_emergency_receipt() {
    let mut history = tool_hop_history(4, &"λ".repeat(3000));
    assert!(fit_tool_results_to_budget(&mut history, 1800, &[]) > 0);
    let once: Vec<_> = history
        .iter()
        .enumerate()
        .filter(|(_, m)| m.content.contains(TOOL_CONTEXT_FIT_MARK))
        .map(|(i, m)| (i, m.content.clone()))
        .collect();
    assert!(!once.is_empty());
    for budget in [1700, 1500, 1000, 500] {
        fit_tool_results_to_budget(&mut history, budget, &[]);
        for (i, content) in &once {
            assert_eq!(
                &history[*i].content, content,
                "emergency elision is one-time"
            );
        }
    }
}

#[test]
fn context_compact_explicit_contract_survives_24_boundaries_verbatim() {
    let _guard = crate::tests::env_lock();
    let _budget = EnvGuard::set("ANGEL_CONTEXT_BUDGET_TOKENS", "2500");
    let _sync = EnvGuard::set("ANGEL_COMPACT_SYNC_LLM", "0");
    let _local = EnvGuard::set("ANGEL_COMPACT_LOCAL", "0");
    let task = format!(
        "  Task context {}\nConstraints:\nNamed identifiers: ALPHA-KITE-9182 BETA-ORCA-4410\nForbidden path: never read /etc/shadow\nNumeric limit: max_hops_soft=17\nRequired test command: python3 -m unittest scripts.test_compaction_retention\nAnswer format: RETENTION-ANSWER: {{status}}\n{}  ",
        "padding ".repeat(400),
        "tail ".repeat(400)
    );
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user(task.as_str())];
    let reg = registry_with_store(Arc::new(crate::memory::store::NullStore));
    let (tx, _rx) = mpsc::channel();
    for boundary in 0..24 {
        for hop in 0..8 {
            history.push(ChatMsg::assistant(format!(
                "boundary {boundary} hop {hop} {}",
                "work ".repeat(500)
            )));
        }
        assert!(maybe_compact_for_turn(
            &SummarizerClub("## Facts\n- Work continued."),
            &mut history,
            2500,
            2,
            400,
            &[],
            &reg,
            &tx
        ));
        assert_eq!(
            history
                .iter()
                .filter(|m| m.role == ChatRole::User && m.content.as_ref() == task)
                .count(),
            1,
            "boundary {boundary}"
        );
        let note = history
            .iter()
            .find(|m| crate::compaction::is_compaction_note(m))
            .unwrap();
        assert!(
            note.content
                .contains("\"constraints_retained\":\"verbatim\"")
        );
        assert!(
            !note.content.contains("ALPHA-KITE-9182"),
            "protected text must not be folded into summary"
        );
    }
}

#[test]
fn context_compact_preserves_first_task_and_explicit_block_with_newer_user_tail() {
    let task = "Repair the parser.";
    let contract = "  Answer format:\nRETENTION-ANSWER: {status}\nConstraints:\nALPHA-KITE-9182 BETA-ORCA-4410  ";
    let history = vec![
        ChatMsg::system("system"),
        ChatMsg::user(task),
        ChatMsg::user(contract),
        ChatMsg::assistant("work"),
        ChatMsg::user("Also handle whitespace."),
    ];
    assert_eq!(
        compaction_task_anchors(&history, 1, 4, 2500),
        vec![task.to_string(), contract.to_string()]
    );
}

#[test]
fn context_compact_summary_input_excludes_operator_contract() {
    let contract = "Constraints: retain ALPHA-KITE-9182; answer format RETENTION-ANSWER: {status}";
    let source = vec![
        ChatMsg::user(contract),
        ChatMsg::assistant("A durable observation."),
    ];
    let summary = compaction_summary_window(&source);
    assert_eq!(summary[0].role, ChatRole::Harness);
    assert!(!summary[0].content.contains("ALPHA-KITE-9182"));
    assert!(
        summary[0]
            .content
            .contains(&crate::cut::sha256_hex(contract.as_bytes()))
    );
    assert_eq!(summary[1].content, source[1].content);
}

#[test]
fn context_compact_history_pruning_keeps_operator_constraints() {
    let contract = "Constraints: preserve ALPHA-KITE-9182; answer format RETENTION-ANSWER: OK";
    let mut history = vec![ChatMsg::system("policy"), ChatMsg::user(contract)];
    for i in 0..20 {
        history.push(ChatMsg::assistant(format!("work {i}")));
    }
    prune_history(&mut history, 5);
    assert!(history.len() <= 5);
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::User && m.content.as_ref() == contract)
    );
    assert_eq!(history.last().unwrap().content.as_ref(), "work 19");
}

#[test]
fn context_compact_constraint_ledger_reports_excerpt_and_drop() {
    let task = "Long operator context. ".repeat(100);
    let history = vec![
        ChatMsg::user(task.as_str()),
        ChatMsg::user("Unretained conversation."),
    ];
    let anchors = vec![bound_task_anchor(&task, 500)];
    let note = constraint_retention_note(
        "[Earlier conversation compacted]".to_string(),
        &history,
        &anchors,
        &[],
    );
    let ledger: serde_json::Value = serde_json::from_str(
        note.lines()
            .find_map(|line| line.strip_prefix("[constraints-ledger/v1] "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["restated"], 1);
    assert_eq!(ledger["dropped"], 1);
    assert_eq!(ledger["constraints_retained"], "dropped");
    assert_eq!(ledger["boundary"], 1);
    assert!(note.contains(&crate::cut::sha256_hex(task.as_bytes())));
    let next = constraint_retention_note(
        "[Earlier conversation compacted]".to_string(),
        &[ChatMsg::harness(note), ChatMsg::user(anchors[0].as_str())],
        &anchors,
        &[],
    );
    let ledger: serde_json::Value = serde_json::from_str(
        next.lines()
            .find_map(|line| line.strip_prefix("[constraints-ledger/v1] "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["boundary"], 2);
}
