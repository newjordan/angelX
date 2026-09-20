//! Tool-result aging, semantic aging, inspection dedup, and argument shrink.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- tool aging / shrink / inspection dedup suite ---------------------------

#[test]
fn completed_tool_argument_shrink_keeps_recent_calls_and_valid_structure() {
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    for index in 0..6 {
        semantic_tool_hop(
            &mut history,
            &format!("write-{index}"),
            "write_file",
            serde_json::json!({
                "path": format!("src/file-{index}.rs"),
                "content": format!("// payload {index}\n{}", "x".repeat(2_000)),
            }),
            &format!("wrote src/file-{index}.rs"),
        );
    }
    let before = estimate_tokens(&history);
    let shrunk = shrink_completed_tool_arguments(&mut history, 2, 1_024);
    assert_eq!(shrunk.calls, 4);
    assert_eq!(shrunk.strings, 4);
    assert!(shrunk.bytes_saved > 7_000);
    let after = estimate_tokens(&history);
    assert!(after < before);
    assert!(
        before.saturating_sub(after) > 1_500,
        "fixture must prove a material request-token reduction"
    );

    let calls = history
        .iter()
        .flat_map(|message| message.tool_calls.iter())
        .collect::<Vec<_>>();
    for (index, call) in calls.iter().enumerate() {
        assert_eq!(
            call.args["path"],
            serde_json::json!(format!("src/file-{index}.rs")),
            "small routing arguments remain exact",
        );
        if index < 4 {
            assert!(
                call.args["content"]
                    .as_str()
                    .unwrap()
                    .starts_with(TOOL_ARGUMENT_SHRINK_MARK)
            );
        } else {
            assert!(call.args["content"].as_str().unwrap().len() > 2_000);
        }
    }
    let second = shrink_completed_tool_arguments(&mut history, 0, 64);
    assert_eq!(second.calls, 2, "only the two recent calls remain eligible");
    assert_eq!(second.strings, 2);
    assert!(second.bytes_saved > 3_000);
    assert_eq!(
        shrink_completed_tool_arguments(&mut history, 0, 64),
        ToolArgumentShrink::default(),
        "receipts are idempotent",
    );
}

#[test]
fn tool_argument_shrink_preserves_failed_denied_and_unpaired_calls() {
    let large = "z".repeat(2_000);
    let mut history = vec![ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "success",
        "write_file",
        serde_json::json!({"path":"ok.rs","content":large}),
        "wrote ok.rs",
    );
    semantic_tool_hop(
        &mut history,
        "failed",
        "write_file",
        serde_json::json!({"path":"failed.rs","content":"f".repeat(2_000)}),
        "tool error: write failed",
    );
    history.push(ChatMsg::tool(
        "failed",
        "late stray success must not relabel a failed call",
    ));
    semantic_tool_hop(
        &mut history,
        "denied",
        "shell",
        serde_json::json!({"command":"d".repeat(2_000)}),
        "action capsule denied by operator",
    );
    semantic_tool_hop(
        &mut history,
        "verified",
        "shell",
        serde_json::json!({"command":"cargo test ".to_string() + &"v".repeat(2_000)}),
        "all tests passed",
    );
    semantic_tool_hop(
        &mut history,
        "patched",
        "apply_patch",
        serde_json::json!({"diff":format!("*** Begin Patch\n*** Update File: src/lib.rs\n{}\n*** End Patch", "p".repeat(2_000))}),
        "patch applied",
    );
    history.push(ChatMsg::assistant_calls(vec![ToolCall {
        id: "unpaired".to_string(),
        name: "apply_patch".to_string(),
        args: serde_json::json!({"patch":"u".repeat(2_000)}),
    }]));

    let original = history.clone();
    assert!(std::sync::Arc::ptr_eq(
        &history[1].tool_calls,
        &original[1].tool_calls
    ));
    let shrunk = shrink_completed_tool_arguments(&mut history, 0, 1_024);
    assert_eq!(shrunk.calls, 1);
    assert!(
        history[1].tool_calls[0].args["content"]
            .as_str()
            .unwrap()
            .starts_with(TOOL_ARGUMENT_SHRINK_MARK)
    );
    let original_content = original[1].tool_calls[0].args["content"].as_str().unwrap();
    assert_eq!(original_content.len(), 2_000);
    assert!(
        !original_content.starts_with(TOOL_ARGUMENT_SHRINK_MARK),
        "worker-local argument shrinking must not mutate the shared UI snapshot"
    );
    assert!(!std::sync::Arc::ptr_eq(
        &history[1].tool_calls,
        &original[1].tool_calls
    ));
    for index in [3usize, 6, 8, 10, 12] {
        let current = &history[index].tool_calls[0];
        let before = &original[index].tool_calls[0];
        assert_eq!(current.id, before.id);
        assert_eq!(current.name, before.name);
        assert_eq!(current.args, before.args);
    }
    let call_ids = history
        .iter()
        .flat_map(|message| message.tool_calls.iter().map(|call| call.id.as_str()))
        .collect::<Vec<_>>();
    let result_ids = history
        .iter()
        .filter_map(|message| message.tool_call_id.as_deref())
        .collect::<Vec<_>>();
    assert!(result_ids.iter().all(|id| call_ids.contains(id)));
}

#[test]
fn tool_argument_shrink_has_an_executable_off_control() {
    let _guard = crate::tests::env_lock();
    let build = || {
        let mut history = vec![ChatMsg::user("task")];
        semantic_tool_hop(
            &mut history,
            "write",
            "write_file",
            serde_json::json!({"path":"src/lib.rs", "content":"p".repeat(2_000)}),
            "wrote src/lib.rs",
        );
        history
    };
    let _off = EnvGuard::set("ANGEL_TOOL_ARGUMENT_SHRINK", "0");
    assert_eq!(
        maybe_shrink_tool_arguments(&mut build()),
        ToolArgumentShrink::default()
    );
    drop(_off);
    let _on = EnvGuard::set("ANGEL_TOOL_ARGUMENT_SHRINK", "1");
    let _min = EnvGuard::set("ANGEL_TOOL_ARGUMENT_MIN_BYTES", "1024");
    let _keep = EnvGuard::set("ANGEL_TOOL_ARGUMENT_KEEP_CALLS", "0");
    assert_eq!(maybe_shrink_tool_arguments(&mut build()).calls, 1);
}

#[test]
fn age_tool_results_elides_old_bulk_and_protects_recent_hops() {
    let big = "x".repeat(5000);
    let mut history = tool_hop_history(6, &big);
    let aged = age_tool_results(&mut history, 4, 0, 2048, false, 12);
    assert_eq!(aged.results, 2, "two hops older than the protected four");
    assert!(aged.bytes_saved > 0);
    let tools: Vec<&ChatMsg> = history
        .iter()
        .filter(|m| m.role == ChatRole::Tool)
        .collect();
    for old in &tools[..2] {
        assert!(old.content.contains(TOOL_AGED_MARK), "{}", old.content);
        assert!(old.content.contains("(5000 bytes)"), "{}", old.content);
        assert!(
            old.content.contains("read_file|src/file-"),
            "{}",
            old.content
        );
        assert!(
            old.content.contains("re-run the tool if needed"),
            "{}",
            old.content
        );
    }
    for recent in &tools[2..] {
        assert_eq!(&*recent.content, big, "protected recent hops untouched");
    }
    // Non-tool messages are never touched.
    assert_eq!(&*history[0].content, "sys");
    assert_eq!(&*history[1].content, "task");
}

#[test]
fn age_tool_results_stores_handle_backed_receipts_when_enabled() {
    let _guard = crate::tests::env_lock();
    let _on = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    session_clear();
    let big = "handle-age-body\n".repeat(400);
    let mut history = tool_hop_history(6, &big);
    let aged = age_tool_results(&mut history, 4, 0, 2048, false, 12);
    assert_eq!(aged.results, 2);
    let tools: Vec<&ChatMsg> = history
        .iter()
        .filter(|m| m.role == ChatRole::Tool)
        .collect();
    let mut handles = Vec::new();
    for old in &tools[..2] {
        assert!(old.content.contains("handle=hnd_"), "{}", old.content);
        assert!(
            !old.content.contains("handle-age-body"),
            "bulk must leave root history: {}",
            old.content
        );
        let handle = old
            .content
            .split("handle=")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .expect("handle id in receipt");
        handles.push(handle.to_string());
    }
    // Explicit disclosure recovers bulk without re-running the tool.
    for handle in &handles {
        let slice = session_disclose(handle, 0, 256).expect("disclose");
        assert!(slice.content.contains("handle-age-body"));
        assert!(slice.total_bytes >= big.len() || slice.total_bytes > 0);
    }
    // Root trajectory classifies handle receipts, not bulk.
    let traj = root_trajectory(&history);
    assert!(traj.handle_receipts >= 2);
    assert_eq!(traj.bulk_tool_results, 4, "recent hops remain bulk");
    session_clear();
}

#[test]
fn age_tool_results_falls_back_without_handles_when_store_disabled() {
    let _guard = crate::tests::env_lock();
    let _off = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    session_clear();
    let big = "x".repeat(5000);
    let mut history = tool_hop_history(6, &big);
    let aged = age_tool_results(&mut history, 4, 0, 2048, false, 12);
    assert_eq!(aged.results, 2);
    let tools: Vec<&ChatMsg> = history
        .iter()
        .filter(|m| m.role == ChatRole::Tool)
        .collect();
    for old in &tools[..2] {
        assert!(old.content.contains(TOOL_AGED_MARK), "{}", old.content);
        assert!(
            !old.content.contains("handle=hnd_"),
            "off control must discard-only: {}",
            old.content
        );
    }
}

fn semantic_tool_hop(history: &mut Vec<ChatMsg>, id: &str, name: &str, args: Value, body: &str) {
    history.push(ChatMsg::assistant_calls(vec![ToolCall {
        id: id.into(),
        name: name.into(),
        args,
    }]));
    history.push(ChatMsg::tool(id, body));
}

#[test]
fn semantic_aging_protects_whole_recent_parallel_hop() {
    let big = "source\n".repeat(900);
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "old",
        "read_file",
        serde_json::json!({"path":"src/old.rs"}),
        &big,
    );
    history.push(ChatMsg::assistant_calls(vec![
        ToolCall {
            id: "recent-a".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path":"src/a.rs"}),
        },
        ToolCall {
            id: "recent-b".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path":"src/b.rs"}),
        },
    ]));
    history.push(ChatMsg::tool("recent-a", big.as_str()));
    history.push(ChatMsg::tool("recent-b", big.as_str()));

    let aged = age_tool_results(&mut history, 1, 0, 512, false, 12);
    assert_eq!(aged.results, 1, "only the truly older hop may age");
    let result = |id: &str| {
        history
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some(id))
            .unwrap()
    };
    assert!(result("old").content.contains(TOOL_AGED_MARK));
    assert_eq!(&*result("recent-a").content, big);
    assert_eq!(&*result("recent-b").content, big);
}

#[test]
fn semantic_aging_protects_exact_token_tail() {
    let body = "x".repeat(1_000); // 250 deterministic estimate tokens.
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    for i in 0..6 {
        semantic_tool_hop(
            &mut history,
            &format!("read-{i}"),
            "read_file",
            serde_json::json!({"path":format!("src/{i}.rs")}),
            &body,
        );
    }

    let aged = age_tool_results(&mut history, 0, 500, 512, false, 12);
    assert_eq!(aged.results, 4);
    let results = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .collect::<Vec<_>>();
    assert!(
        results[..4]
            .iter()
            .all(|message| message.content.contains(TOOL_AGED_MARK))
    );
    assert!(
        results[4..]
            .iter()
            .all(|message| message.content.as_ref() == body)
    );
}

#[test]
fn semantic_aging_fixed_fixture_reduces_request_tokens_without_orphans() {
    let body = "source payload\n".repeat(300);
    let mut history = tool_hop_history(12, &body);
    let before = estimate_tokens(&history);

    let aged = age_tool_results(&mut history, 4, 8_000, 512, false, 12);
    let after = estimate_tokens(&history);
    assert_eq!(aged.results, 4);
    assert!(aged.bytes_saved >= 15_000, "got {}", aged.bytes_saved);
    assert!(
        after * 100 <= before * 75,
        "expected at least 25% request-token reduction: {before} -> {after}"
    );

    let call_ids = history
        .iter()
        .flat_map(|message| message.tool_calls.iter().map(|call| call.id.as_str()))
        .collect::<std::collections::HashSet<_>>();
    assert!(
        history
            .iter()
            .filter(|m| m.role == ChatRole::Tool)
            .all(|m| {
                m.tool_call_id
                    .as_deref()
                    .is_some_and(|id| call_ids.contains(id))
            })
    );
}

#[test]
fn semantic_aging_never_clears_effect_error_or_denial_evidence() {
    let big = "evidence\n".repeat(900);
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "verify",
        "shell",
        serde_json::json!({"command":"cargo test"}),
        &big,
    );
    semantic_tool_hop(
        &mut history,
        "write",
        "write_file",
        serde_json::json!({"path":"src/lib.rs","content":"changed"}),
        &big,
    );
    semantic_tool_hop(
        &mut history,
        "error",
        "read_file",
        serde_json::json!({"path":"missing.rs"}),
        &format!("tool error: {big}"),
    );
    semantic_tool_hop(
        &mut history,
        "denied",
        "read_file",
        serde_json::json!({"path":"secret.rs"}),
        &format!("action capsule denied: {big}"),
    );
    semantic_tool_hop(
        &mut history,
        "eligible",
        "read_file",
        serde_json::json!({"path":"src/eligible.rs"}),
        &big,
    );

    assert_eq!(
        age_tool_results(&mut history, 0, 0, 512, false, 12).results,
        1
    );
    let result = |id: &str| {
        history
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some(id))
            .unwrap()
    };
    assert_eq!(&*result("verify").content, big);
    assert_eq!(&*result("write").content, big);
    assert!(result("error").content.starts_with("tool error:"));
    assert!(
        result("denied")
            .content
            .starts_with("action capsule denied:")
    );
    assert!(result("eligible").content.contains(TOOL_AGED_MARK));
}

#[test]
fn semantic_aging_protects_current_plan_and_latest_skill_bodies() {
    let big = "state\n".repeat(900);
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "skill",
        "skill",
        serde_json::json!({"name":"verify-changes"}),
        &big,
    );
    semantic_tool_hop(
        &mut history,
        "todo",
        "todo",
        serde_json::json!({"action":"set","items":["implement","test"]}),
        &big,
    );
    for i in 0..5 {
        semantic_tool_hop(
            &mut history,
            &format!("shell-{i}"),
            "read_file",
            serde_json::json!({"path":format!("src/state-{i}.rs")}),
            &big,
        );
    }

    assert_eq!(
        age_tool_results(&mut history, 2, 0, 2048, false, 12).results,
        3
    );
    let result = |id: &str| {
        history
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some(id))
            .unwrap()
    };
    assert_eq!(&*result("skill").content, big);
    assert_eq!(&*result("todo").content, big);
    assert!(result("shell-0").content.contains(TOOL_AGED_MARK));
}

#[test]
fn semantic_aging_deduplicates_and_bounds_protected_skill_results() {
    let big = "playbook\n".repeat(700);
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    for i in 0..10 {
        semantic_tool_hop(
            &mut history,
            &format!("skill-{i}"),
            "skill",
            serde_json::json!({"name":format!("skill-{i}")}),
            &big,
        );
    }
    semantic_tool_hop(
        &mut history,
        "skill-9-again",
        "skill",
        serde_json::json!({"name":"skill-9"}),
        &big,
    );

    assert_eq!(
        age_tool_results(&mut history, 0, 0, 2048, false, 12).results,
        3
    );
    let aged_ids: Vec<&str> = history
        .iter()
        .filter(|message| message.content.contains(TOOL_AGED_MARK))
        .filter_map(|message| message.tool_call_id.as_deref())
        .collect();
    assert_eq!(aged_ids, vec!["skill-0", "skill-1", "skill-9"]);
    assert_eq!(
        protected_tool_result_indices(&history).len(),
        PROTECTED_SKILL_RESULTS
    );
}

#[test]
fn semantic_aging_pairs_provider_reused_call_ids_in_protocol_order() {
    let big = "payload\n".repeat(700);
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "reused",
        "skill",
        serde_json::json!({"name":"verify-changes"}),
        &big,
    );
    semantic_tool_hop(
        &mut history,
        "reused",
        "read_file",
        serde_json::json!({"path":"src/status.rs"}),
        &big,
    );
    assert_eq!(
        age_tool_results(&mut history, 0, 0, 2048, false, 12).results,
        1
    );
    let results: Vec<&ChatMsg> = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .collect();
    assert_eq!(&*results[0].content, big, "skill result stays protected");
    assert!(results[1].content.contains(TOOL_AGED_MARK));
}

#[test]
fn emergency_context_fit_can_trim_semantically_protected_results() {
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "skill",
        "skill",
        serde_json::json!({"name":"large-playbook"}),
        &"x".repeat(20_000),
    );
    assert!(protected_tool_result_indices(&history).contains(&(history.len() - 1)));
    assert_eq!(fit_tool_results_to_budget(&mut history, 300, &[]), 1);
    assert!(context_tokens(&history, &[]) <= 300);
    assert!(
        history
            .last()
            .unwrap()
            .content
            .contains(TOOL_CONTEXT_FIT_MARK)
    );
}

#[test]
fn age_tool_results_is_idempotent() {
    let big = "x".repeat(5000);
    let mut history = tool_hop_history(6, &big);
    assert_eq!(
        age_tool_results(&mut history, 4, 0, 2048, false, 12).results,
        2
    );
    let snapshot: Vec<String> = history.iter().map(|m| m.content.to_string()).collect();
    // Even with the bulk floor dropped, an already-aged message is skipped
    // (the marker check, not the byte count, is what makes this idempotent).
    assert_eq!(
        age_tool_results(&mut history, 4, 0, 1, false, 12),
        InspectionAging::default()
    );
    let after: Vec<String> = history.iter().map(|m| m.content.to_string()).collect();
    assert_eq!(snapshot, after, "a second pass changes nothing");
}

#[test]
fn inspection_dedup_keeps_newest_full_copy_and_is_idempotent() {
    let body = "same source line\n".repeat(40);
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "read-old",
        "read_file",
        serde_json::json!({"path":"src/lib.rs"}),
        &body,
    );
    semantic_tool_hop(
        &mut history,
        "read-new",
        "read_file",
        serde_json::json!({"path":"src/./lib.rs"}),
        &body,
    );

    let dedup = dedupe_identical_inspection_results(&mut history, 200);
    assert_eq!(dedup.results, 1);
    assert!(dedup.bytes_saved > 0);
    let results = history
        .iter()
        .filter(|message| message.role == ChatRole::Tool)
        .collect::<Vec<_>>();
    assert!(results[0].content.contains(TOOL_DUPLICATE_MARK));
    assert!(results[0].content.contains("read-new"));
    assert_eq!(&*results[1].content, body, "newest result remains complete");

    assert_eq!(
        dedupe_identical_inspection_results(&mut history, 1),
        InspectionDedup::default(),
        "receipts are stable across repeated hygiene passes"
    );
}

#[test]
fn inspection_dedup_requires_same_identity_content_and_effect_epoch() {
    let body = "same bytes\n".repeat(40);
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    semantic_tool_hop(
        &mut history,
        "path-a",
        "read_file",
        serde_json::json!({"path":"a.rs"}),
        &body,
    );
    semantic_tool_hop(
        &mut history,
        "path-b",
        "read_file",
        serde_json::json!({"path":"b.rs"}),
        &body,
    );
    semantic_tool_hop(
        &mut history,
        "effect",
        "write_file",
        serde_json::json!({"path":"note.txt","content":"changed"}),
        "wrote note.txt",
    );
    semantic_tool_hop(
        &mut history,
        "path-a-after-effect",
        "read_file",
        serde_json::json!({"path":"a.rs"}),
        &body,
    );
    semantic_tool_hop(
        &mut history,
        "path-a-changed",
        "read_file",
        serde_json::json!({"path":"a.rs"}),
        &"different bytes\n".repeat(40),
    );

    assert_eq!(
        dedupe_identical_inspection_results(&mut history, 200),
        InspectionDedup::default(),
        "different paths, post-effect results, and changed content stay complete"
    );
}

#[test]
fn inspection_dedup_env_has_an_executable_off_control() {
    let _guard = crate::tests::env_lock();
    let body = "repeat\n".repeat(60);
    let build = || {
        let mut history = vec![ChatMsg::user("task")];
        semantic_tool_hop(
            &mut history,
            "old",
            "grep",
            serde_json::json!({"pattern":"needle"}),
            &body,
        );
        semantic_tool_hop(
            &mut history,
            "new",
            "grep",
            serde_json::json!({"pattern":"needle"}),
            &body,
        );
        history
    };

    let _off = EnvGuard::set("ANGEL_TOOL_RESULT_DEDUP", "0");
    assert_eq!(
        maybe_dedupe_inspection_results(&mut build()),
        InspectionDedup::default()
    );
    drop(_off);

    let _on = EnvGuard::set("ANGEL_TOOL_RESULT_DEDUP", "1");
    let mut history = build();
    assert_eq!(maybe_dedupe_inspection_results(&mut history).results, 1);

    let mut small = vec![ChatMsg::user("task")];
    semantic_tool_hop(
        &mut small,
        "small-old",
        "grep",
        serde_json::json!({"pattern":"tiny"}),
        "tiny",
    );
    semantic_tool_hop(
        &mut small,
        "small-new",
        "grep",
        serde_json::json!({"pattern":"tiny"}),
        "tiny",
    );
    assert_eq!(
        dedupe_identical_inspection_results(&mut small, 1),
        InspectionDedup::default(),
        "a receipt must never be larger than the result it replaces"
    );
}

#[test]
fn age_tool_results_skips_small_outputs() {
    let mut history = tool_hop_history(8, "short result");
    assert_eq!(
        age_tool_results(&mut history, 4, 0, 2048, false, 12),
        InspectionAging::default(),
        "outputs under the bulk floor are never aged"
    );
    assert!(
        history
            .iter()
            .filter(|m| m.role == ChatRole::Tool)
            .all(|m| m.content.as_ref() == "short result")
    );
}

#[test]
fn tool_aging_env_wiring_and_kill_switch() {
    let _guard = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TOOL_AGE_KEEP_HOPS") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TOOL_AGE_MIN_BYTES") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TOOL_AGE_PROTECT_TOKENS") };
    let big = "y".repeat(4096);
    let mut history = tool_hop_history(12, &big);

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_TOOL_AGING", "0") };
    assert_eq!(
        maybe_age_tool_results(&mut history),
        InspectionAging::default(),
        "kill switch"
    );
    assert!(history.iter().all(|m| !m.content.contains(TOOL_AGED_MARK)));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TOOL_AGING") };
    assert_eq!(
        maybe_age_tool_results(&mut history).results,
        4,
        "defaults: keep 4 true hops plus an 8k-token result tail"
    );
}

// --- opt-in effect aging (ANGEL_TOOL_AGE_EFFECTS) ----------------------------

fn effect_hop_history() -> (Vec<ChatMsg>, Vec<String>) {
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    let mut bodies = Vec::new();
    for i in 0..8 {
        let body = format!("HEAD-{i}\n{}\nTAIL-{i}", "x".repeat(3_000));
        semantic_tool_hop(
            &mut history,
            &format!("shell-{i}"),
            "shell",
            serde_json::json!({"command": format!("cargo test --lib module-{i}")}),
            &body,
        );
        bodies.push(body);
    }
    (history, bodies)
}

#[test]
fn effect_results_are_not_aged_by_default() {
    let (mut history, _) = effect_hop_history();
    let before: Vec<String> = history.iter().map(|m| m.content.to_string()).collect();
    assert_eq!(
        age_tool_results(&mut history, 2, 0, 512, false, 12).results,
        0,
        "default keeps verifier/effect evidence complete"
    );
    let after: Vec<String> = history.iter().map(|m| m.content.to_string()).collect();
    assert_eq!(before, after);
}

#[test]
fn effect_results_age_to_excerpt_receipts_when_enabled() {
    let _guard = crate::tests::env_lock();
    let _off = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    session_clear();
    let (mut history, bodies) = effect_hop_history();
    let aged = age_tool_results(&mut history, 2, 0, 512, true, 12);
    assert_eq!(aged.results, 6, "the oldest six of eight hops age");
    assert!(aged.bytes_saved > 0);
    let result = |id: &str| {
        history
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some(id))
            .unwrap()
    };
    for (i, body) in bodies.iter().enumerate().take(6) {
        let content = &result(&format!("shell-{i}")).content;
        assert!(content.contains(TOOL_AGED_MARK), "{content}");
        assert!(
            content.contains(&format!("shell|cargo test --lib module-{i}")),
            "{content}"
        );
        assert!(content.contains(&format!("head: HEAD-{i} ")), "{content}");
        assert!(content.contains("tail: "), "{content}");
        assert!(
            content.contains(&format!("x TAIL-{i} — re-run the tool if needed]")),
            "tail excerpt must carry the real last bytes: {content}"
        );
        assert!(
            content.len() < body.len(),
            "receipt must be shorter than the original"
        );
    }
    for (i, body) in bodies.iter().enumerate().take(8).skip(6) {
        assert_eq!(
            &*result(&format!("shell-{i}")).content,
            body.as_str(),
            "the two newest hops stay intact"
        );
    }
    session_clear();
}

#[test]
fn failed_effect_results_stay_complete() {
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    let body = format!("tool error: {}", "x".repeat(3_000));
    semantic_tool_hop(
        &mut history,
        "boom",
        "shell",
        serde_json::json!({"command": "cargo test --lib boom"}),
        &body,
    );
    assert_eq!(
        age_tool_results(&mut history, 0, 0, 512, true, 12).results,
        0
    );
    assert_eq!(
        &*history.last().unwrap().content,
        &body,
        "errors are protected even with the knob on"
    );
}

#[test]
fn write_results_are_never_effect_aged() {
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    let body = format!("wrote src/lib.rs\n{}", "x".repeat(3_000));
    semantic_tool_hop(
        &mut history,
        "write",
        "write_file",
        serde_json::json!({"path": "src/lib.rs", "content": "changed"}),
        &body,
    );
    assert_eq!(
        age_tool_results(&mut history, 0, 0, 512, true, 12).results,
        0
    );
    assert_eq!(
        &*history.last().unwrap().content,
        &body,
        "write receipts are mutation evidence and never effect-aged"
    );
}

// --- second-stage excerpt shrink (ANGEL_TOOL_AGE_EXCERPT_HOPS) ----------------

/// Thirty bulky shell hops: enough distance for receipts to travel far past
/// any sane excerpt window while the newest hops stay protected.
fn excerpt_hop_history() -> (Vec<ChatMsg>, Vec<String>) {
    let mut history = vec![ChatMsg::system("sys"), ChatMsg::user("task")];
    let mut bodies = Vec::new();
    for i in 0..30 {
        let body = format!("HEAD-{i}\n{}\nTAIL-{i}", "x".repeat(3_000));
        semantic_tool_hop(
            &mut history,
            &format!("shell-{i}"),
            "shell",
            serde_json::json!({"command": format!("cargo test --lib module-{i}")}),
            &body,
        );
        bodies.push(body);
    }
    (history, bodies)
}

#[test]
fn old_effect_receipts_drop_their_excerpt_after_the_window() {
    let _guard = crate::tests::env_lock();
    let _on = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    session_clear();
    let (mut history, bodies) = excerpt_hop_history();
    let aged = age_tool_results(&mut history, 2, 0, 512, true, 5);
    assert_eq!(aged.results, 28, "the oldest 28 of 30 hops age");
    assert_eq!(
        aged.excerpts_dropped, 24,
        "receipts more than 5 hops behind hop 29 lose their excerpt"
    );
    assert!(aged.bytes_saved > 0);
    let result = |id: &str| {
        history
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some(id))
            .unwrap()
    };
    for (i, body) in bodies.iter().enumerate().take(24) {
        let content = &result(&format!("shell-{i}")).content;
        assert!(content.contains(TOOL_AGED_MARK), "{content}");
        assert!(
            content.contains(&format!("shell|cargo test --lib module-{i}")),
            "identity survives: {content}"
        );
        assert!(
            content.contains(&format!("({} bytes)", body.len())),
            "original size survives: {content}"
        );
        assert!(
            content.contains(" handle=hnd_"),
            "handle text survives without re-parking bulk: {content}"
        );
        assert!(!content.contains(TOOL_EXCERPT_MARK), "{content}");
        assert!(
            content.ends_with(" — re-run the tool if needed]"),
            "{content}"
        );
    }
    for i in 24..28 {
        let content = &result(&format!("shell-{i}")).content;
        assert!(
            content.contains(TOOL_EXCERPT_MARK),
            "receipts within 5 hops of the newest keep their excerpt: {content}"
        );
    }
    for (i, body) in bodies.iter().enumerate().take(30).skip(28) {
        assert_eq!(
            &*result(&format!("shell-{i}")).content,
            body.as_str(),
            "the two newest hops stay intact"
        );
    }
    // A second pass is a no-op: nothing left to drop, history byte-identical.
    let snapshot: Vec<String> = history.iter().map(|m| m.content.to_string()).collect();
    assert_eq!(
        age_tool_results(&mut history, 2, 0, 512, true, 5),
        InspectionAging::default()
    );
    let after: Vec<String> = history.iter().map(|m| m.content.to_string()).collect();
    assert_eq!(snapshot, after, "a second pass changes nothing");
    session_clear();
}

#[test]
fn excerpt_hops_zero_keeps_excerpts() {
    let _guard = crate::tests::env_lock();
    let _on = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    session_clear();
    let (mut history, _) = excerpt_hop_history();
    let aged = age_tool_results(&mut history, 2, 0, 512, true, 0);
    assert_eq!(aged.results, 28);
    assert_eq!(aged.excerpts_dropped, 0, "0 keeps excerpts forever");
    let receipts: Vec<&ChatMsg> = history
        .iter()
        .filter(|m| m.content.contains(TOOL_AGED_MARK))
        .collect();
    assert_eq!(receipts.len(), 28);
    assert!(
        receipts
            .iter()
            .all(|m| m.content.contains(TOOL_EXCERPT_MARK)),
        "no receipt loses its excerpt"
    );
    session_clear();
}

// --- cache-stable mode: aging deferred until compaction ---------------------

/// Prefix retention defaults on even for unknown providers. Explicit per-club
/// and global pins still select the historical token-first ablation.
#[test]
fn cache_stable_mode_defaults_on_and_the_club_pin_beats_the_global() {
    let _guard = crate::tests::env_lock();
    let _global = EnvGuard::unset("ANGEL_CACHE_STABLE");
    let _pin = EnvGuard::unset("ANGEL_CACHE_STABLE_PROBE_CACHE_STABLE");
    let club = crate::club::HttpClub::new("cache-stable-probe", "http://127.0.0.1:9/v1", "m", None);
    let logical = crate::club::PracticeClub::new();
    assert!(
        cache_stable_mode(&club),
        "unknown providers also retain prefixes by default"
    );

    // A backend with a documented byte-exact prefix cache runs cache-first by
    // default; the explicit global still overrides detection either way.
    let capable = crate::club::HttpClub::new(
        "cache-stable-probe",
        "http://127.0.0.1:9/v1",
        "deepseek-v4-flash",
        None,
    );
    assert!(
        cache_stable_mode(&capable),
        "a detected prefix-cache backend is cache-first by default"
    );
    let wrapped = crate::swarm::SwarmClub::from_env(
        "cache-stable-swarm-probe",
        std::sync::Arc::new(crate::club::HttpClub::new(
            "cache-stable-wrapped-probe",
            "http://127.0.0.1:9/v1",
            "glm-5.3-flash",
            None,
        )),
    );
    assert!(
        cache_stable_mode(&wrapped),
        "a cache-capable aggregate must keep its outer swarm history append-only"
    );
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CACHE_STABLE", "0") };
    assert!(
        !cache_stable_mode(&capable),
        "an explicit global off overrides capability detection"
    );
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CACHE_STABLE") };

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CACHE_STABLE", "1") };
    assert!(cache_stable_mode(&club));
    assert!(
        cache_stable_mode(&logical),
        "a club with no env namespace still follows the global"
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CACHE_STABLE_PROBE_CACHE_STABLE", "0") };
    assert!(
        !cache_stable_mode(&club),
        "an explicit per-club off beats the global on"
    );
    assert!(cache_stable_mode(&logical), "the pin is scoped to its club");

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CACHE_STABLE", "0") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_CACHE_STABLE_PROBE_CACHE_STABLE", "1") };
    assert!(
        cache_stable_mode(&club),
        "an explicit per-club on beats the global off"
    );
    assert!(!cache_stable_mode(&logical));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_CACHE_STABLE_PROBE_CACHE_STABLE") };
    assert!(
        !cache_stable_mode(&club),
        "an unset pin falls back to the global"
    );
}

/// Records, per request, whether the history it was handed already carried an
/// aged receipt — the observable signature of a mid-turn in-place rewrite, which
/// is exactly what invalidates a byte-exact provider prefix from there on.
struct AgingProbeClub {
    hops: AtomicUsize,
    requests_with_aged_results: AtomicUsize,
    retained: std::sync::Mutex<Vec<ChatMsg>>,
    check_prefix: bool,
}

impl Club for AgingProbeClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "aging-probe"
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.check_prefix {
            let mut retained = self.retained.lock().unwrap();
            assert_eq!(
                serde_json::to_vec(&messages[..retained.len()]).unwrap(),
                serde_json::to_vec(&*retained).unwrap(),
                "all prior serialized bytes survive every hop"
            );
            *retained = messages.to_vec();
        }
        if messages
            .iter()
            .any(|m| m.role == ChatRole::Tool && m.content.contains(TOOL_AGED_MARK))
        {
            self.requests_with_aged_results
                .fetch_add(1, Ordering::SeqCst);
        }
        let hop = self.hops.fetch_add(1, Ordering::SeqCst);
        if hop >= 10 {
            return Ok(ClubReply::Text("done".into()));
        }
        // A distinct bulky inspection per hop: aging-eligible (`reverse` is a
        // read footprint), never a duplicate, never a repeated identity.
        Ok(ClubReply::Calls(vec![ToolCall {
            id: format!("c{hop}"),
            name: "reverse".into(),
            args: serde_json::json!({ "text": format!("hop {hop} {}", "payload ".repeat(120)) }),
        }]))
    }
}

/// Run ten `reverse` hops with both aging tails tightened so the pass has
/// something to do and crosses the historical eight-hop rewrite boundary. The
/// caller holds the env lock.
fn aging_probe_turn(cache_stable: &str) -> (usize, Vec<ChatMsg>, Vec<String>) {
    aging_probe_turn_with_cadence(cache_stable, "0")
}

fn aging_probe_turn_with_cadence(
    cache_stable: &str,
    cadence: &str,
) -> (usize, Vec<ChatMsg>, Vec<String>) {
    let _cadence = EnvGuard::set("ANGEL_ROLLING_REWRITE_HOPS", cadence);
    let _broker = EnvGuard::set("ANGEL_BACKPLANE", "1");
    let _stable = EnvGuard::set("ANGEL_CACHE_STABLE", cache_stable);
    let _hops = EnvGuard::set("ANGEL_TOOL_AGE_KEEP_HOPS", "1");
    let _tokens = EnvGuard::set("ANGEL_TOOL_AGE_PROTECT_TOKENS", "0");
    let _min = EnvGuard::set("ANGEL_TOOL_AGE_MIN_BYTES", "512");
    // Plain receipts rather than handle receipts: the mark is the same, but the
    // fixture then depends on nothing but the aging pass.
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let club = AgingProbeClub {
        hops: AtomicUsize::new(0),
        requests_with_aged_results: AtomicUsize::new(0),
        retained: std::sync::Mutex::new(Vec::new()),
        check_prefix: cache_stable == "1" && cadence == "0",
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("probe the aging boundary")];
    // Seed real broker-owned evidence so continuation checks do not depend on
    // whether the developer's local memory stores happen to contain anything.
    let atlas = reg.atlas();
    let project = atlas.project_key();
    let selection = crate::backplane::KnowledgeBroker::select(
        project,
        "aging boundary",
        vec![crate::backplane::KnowledgeCandidate::new(
            "memory:aging-fixture",
            project,
            "operator-memory",
            crate::backplane::KnowledgeAuthority::OperatorApproved,
            "An aging boundary fixture fact.",
        )],
        10_000,
    );
    crate::backplane::KnowledgeBroker::replace(&mut history, &selection);
    let (events, rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(16),
        &events,
    )
    .unwrap();
    assert_eq!(answer, "done");
    if cache_stable == "1" && cadence == "0" {
        let retained = serde_json::to_vec(&history).unwrap();
        let retained_len = history.len();
        history.push(ChatMsg::user("continue"));
        let (continuation_events, _rx) = mpsc::channel();
        run_turn(
            &club,
            &reg,
            &mut history,
            &AtomicBool::new(false),
            Some(16),
            &continuation_events,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&history[..retained_len]).unwrap(),
            retained,
            "turn seal and continuation must also preserve every previous byte"
        );
    }

    let notices = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(note) => Some(note),
            _ => None,
        })
        .collect();
    (
        club.requests_with_aged_results.load(Ordering::SeqCst),
        history,
        notices,
    )
}

#[test]
fn cache_stable_rolling_boundary_ages_inspection_bulk_with_an_off_control() {
    let _guard = crate::tests::env_lock();
    let (held, _, _) = aging_probe_turn_with_cadence("1", "0");
    let (flushed, history, _) = aging_probe_turn_with_cadence("1", "4");
    assert_eq!(held, 0);
    assert!(
        flushed > 0,
        "rolling boundary must shrink old inspection results"
    );
    assert!(
        history
            .iter()
            .any(|message| message.content.contains(TOOL_AGED_MARK))
    );
}

#[test]
fn cache_stable_holds_tool_result_aging_until_compaction() {
    let _guard = crate::tests::env_lock();
    let (mid_turn_rewrites, history, notices) = aging_probe_turn("1");
    assert_eq!(
        mid_turn_rewrites, 0,
        "no request may carry a receipt written after an earlier request: the \
         prefix is append-only for the whole turn"
    );
    assert!(
        history
            .iter()
            .filter(|m| m.content.contains(TOOL_AGED_MARK))
            .count()
            == 0,
        "turn completion must retain the prefix until compaction"
    );
    let spoken = notices
        .iter()
        .filter(|note| note.starts_with("cache-stable:"))
        .count();
    assert_eq!(spoken, 1, "one note per turn, not per hop: {notices:?}");
}

/// A boundary that actually rewrote history resets the held-hop count, so the
/// turn-seal notice cannot claim deferral for hops it already flushed.
#[test]
fn cache_stable_held_hop_count_resets_at_a_real_rewrite_boundary() {
    let _guard = crate::tests::env_lock();
    let _stable = EnvGuard::set("ANGEL_CACHE_STABLE", "1");
    let _cap = EnvGuard::set("ANGEL_HISTORY_MAX_MSGS", "6");
    let _broker = EnvGuard::set("ANGEL_BACKPLANE", "1");
    let _keep = EnvGuard::set("ANGEL_TOOL_AGE_KEEP_HOPS", "1");
    let _tokens = EnvGuard::set("ANGEL_TOOL_AGE_PROTECT_TOKENS", "0");
    let _min = EnvGuard::set("ANGEL_TOOL_AGE_MIN_BYTES", "512");
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let club = AgingProbeClub {
        hops: AtomicUsize::new(0),
        requests_with_aged_results: AtomicUsize::new(0),
        retained: std::sync::Mutex::new(Vec::new()),
        check_prefix: false,
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("probe the aging boundary")];
    let (events, rx) = mpsc::channel::<TurnEvent>();
    run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(16),
        &events,
    )
    .unwrap();
    let provider_calls = club.hops.load(Ordering::SeqCst);
    assert!(
        provider_calls > 3,
        "the probe must cross the message cap: {provider_calls}"
    );
    let note = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(note) if note.starts_with("cache-stable:") => Some(note),
            _ => None,
        })
        .next()
        .unwrap_or_else(|| panic!("boundary savings are still reported"));
    let held: usize = note
        .split("for the last ")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|count| count.parse().ok())
        .unwrap_or_else(|| panic!("held-hop count is parseable: {note}"));
    assert!(
        held < provider_calls,
        "a prune boundary resets the held count: {held} held across {provider_calls} provider call(s) — {note}"
    );
}

#[test]
fn tool_result_aging_stays_mid_turn_when_cache_stable_is_off() {
    let _guard = crate::tests::env_lock();
    let (mid_turn_rewrites, history, notices) = aging_probe_turn("0");
    assert!(
        mid_turn_rewrites > 0,
        "explicit off mode retains aging between hops"
    );
    assert!(history.iter().any(|m| m.content.contains(TOOL_AGED_MARK)));
    assert!(
        !notices.iter().any(|note| note.starts_with("cache-stable:")),
        "an inactive mode says nothing: {notices:?}"
    );
}

/// Records, per request, whether the history it was handed already carried a
/// shrunk-argument receipt — the observable signature of a mid-turn in-place
/// rewrite of a completed call, which invalidates a byte-exact provider prefix
/// from that message on exactly the way tool-result aging does.
struct ShrinkProbeClub {
    hops: AtomicUsize,
    requests_with_shrunk_arguments: AtomicUsize,
}

impl Club for ShrinkProbeClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "shrink-probe"
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if messages.iter().any(|m| {
            m.tool_calls
                .iter()
                .any(|call| call.args.to_string().contains(TOOL_ARGUMENT_SHRINK_MARK))
        }) {
            self.requests_with_shrunk_arguments
                .fetch_add(1, Ordering::SeqCst);
        }
        let hop = self.hops.fetch_add(1, Ordering::SeqCst);
        if hop >= 4 {
            return Ok(ClubReply::Text("done".into()));
        }
        Ok(ClubReply::Calls(vec![ToolCall {
            id: format!("w{hop}"),
            name: "write_file".into(),
            args: serde_json::json!({
                "path": format!("note-{hop}.txt"),
                "content": format!("hop {hop}\n{}", "payload ".repeat(200)),
            }),
        }]))
    }
}

/// Run one bounded turn of four completed `write_file` calls with the shrink
/// thresholds tightened so the pass has three calls to receipt. The caller
/// holds the env lock.
fn shrink_probe_turn(cache_stable: &str) -> (usize, Vec<ChatMsg>, Vec<String>) {
    let _stable = EnvGuard::set("ANGEL_CACHE_STABLE", cache_stable);
    let _keep = EnvGuard::set("ANGEL_TOOL_ARGUMENT_KEEP_CALLS", "1");
    let _min = EnvGuard::set("ANGEL_TOOL_ARGUMENT_MIN_BYTES", "64");
    // Isolate the argument rewriter: result aging would otherwise be a second
    // source of the deferral notice, and the verification gate would add an
    // unscripted hop after the writes.
    let _aging = EnvGuard::set("ANGEL_TOOL_AGING", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let club = ShrinkProbeClub {
        hops: AtomicUsize::new(0),
        requests_with_shrunk_arguments: AtomicUsize::new(0),
    };
    let workspace = scratch("cache_stable_shrink");
    let reg = ToolRegistry::with_team(workspace.clone(), Vec::new());
    let mut history = vec![ChatMsg::user("write the notes")];
    let (events, rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &events,
    )
    .unwrap();
    assert_eq!(answer, "done");
    let notices = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(note) => Some(note),
            _ => None,
        })
        .collect();
    let _ = std::fs::remove_dir_all(workspace);
    (
        club.requests_with_shrunk_arguments.load(Ordering::SeqCst),
        history,
        notices,
    )
}

fn history_carries_shrunk_arguments(history: &[ChatMsg]) -> bool {
    history.iter().any(|m| {
        m.tool_calls
            .iter()
            .any(|call| call.args.to_string().contains(TOOL_ARGUMENT_SHRINK_MARK))
    })
}

#[test]
fn cache_stable_holds_tool_argument_shrinking_until_compaction() {
    let _guard = crate::tests::env_lock();
    let (mid_turn_rewrites, history, notices) = shrink_probe_turn("1");
    assert_eq!(
        mid_turn_rewrites, 0,
        "no request may carry an argument receipt written after an earlier \
         request: the prefix is append-only for the whole turn"
    );
    assert!(
        !history_carries_shrunk_arguments(&history),
        "turn completion must retain the prefix until compaction"
    );
    let spoken: Vec<&String> = notices
        .iter()
        .filter(|note| note.starts_with("cache-stable:"))
        .collect();
    assert_eq!(
        spoken.len(),
        1,
        "one note per turn, not per hop: {notices:?}"
    );
    assert!(
        spoken[0].contains("shrank 0 tool-call argument(s)"),
        "the boundary pass must carry its own figures: {spoken:?}"
    );
}

#[test]
fn tool_argument_shrinking_stays_mid_turn_when_cache_stable_is_off() {
    let _guard = crate::tests::env_lock();
    let (mid_turn_rewrites, history, notices) = shrink_probe_turn("0");
    assert!(
        mid_turn_rewrites > 0,
        "default behavior is unchanged: shrinking rewrites history between hops"
    );
    assert!(history_carries_shrunk_arguments(&history));
    assert!(
        !notices.iter().any(|note| note.starts_with("cache-stable:")),
        "an inactive mode says nothing: {notices:?}"
    );
}

/// Records, per request, whether the history it was handed already carried a
/// duplicate-inspection receipt — dedup rewrites an *older* identical result in
/// place exactly like aging does, so a mid-turn receipt is the same
/// prefix-cache invalidation signature.
struct DedupProbeClub {
    hops: AtomicUsize,
    requests_with_dedup_receipts: AtomicUsize,
}

impl Club for DedupProbeClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        "dedup-probe"
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if messages
            .iter()
            .any(|m| m.role == ChatRole::Tool && m.content.contains(TOOL_DUPLICATE_MARK))
        {
            self.requests_with_dedup_receipts
                .fetch_add(1, Ordering::SeqCst);
        }
        let hop = self.hops.fetch_add(1, Ordering::SeqCst);
        if hop >= 4 {
            return Ok(ClubReply::Text("done".into()));
        }
        // The *same* bulky read every hop: identical identity, identical
        // content, no interleaved effect — dedup-eligible from the second
        // result on (`reverse` is a read footprint).
        Ok(ClubReply::Calls(vec![ToolCall {
            id: format!("c{hop}"),
            name: "reverse".into(),
            args: serde_json::json!({ "text": "payload ".repeat(80) }),
        }]))
    }
}

/// Run one bounded turn of four identical `reverse` hops with aging disabled so
/// the dedup pass is the only in-place rewriter left. The caller holds the env
/// lock.
fn dedup_probe_turn(cache_stable: &str) -> (usize, Vec<ChatMsg>, Vec<String>) {
    let _stable = EnvGuard::set("ANGEL_CACHE_STABLE", cache_stable);
    let _aging = EnvGuard::set("ANGEL_TOOL_AGING", "0");
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let club = DedupProbeClub {
        hops: AtomicUsize::new(0),
        requests_with_dedup_receipts: AtomicUsize::new(0),
    };
    let reg = ToolRegistry::with_defaults();
    let mut history = vec![ChatMsg::user("probe the dedup boundary")];
    let (events, rx) = mpsc::channel::<TurnEvent>();
    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(8),
        &events,
    )
    .unwrap();
    assert_eq!(answer, "done");
    let notices = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::Notice(note) => Some(note),
            _ => None,
        })
        .collect();
    (
        club.requests_with_dedup_receipts.load(Ordering::SeqCst),
        history,
        notices,
    )
}

#[test]
fn cache_stable_holds_inspection_dedup_until_compaction() {
    let _guard = crate::tests::env_lock();
    let (mid_turn_rewrites, history, notices) = dedup_probe_turn("1");
    assert_eq!(
        mid_turn_rewrites, 0,
        "no request may carry a dedup receipt written after an earlier request: \
         the prefix is append-only for the whole turn"
    );
    assert!(
        history
            .iter()
            .filter(|m| m.content.contains(TOOL_DUPLICATE_MARK))
            .count()
            == 0,
        "turn completion must retain the prefix until compaction"
    );
    let spoken = notices
        .iter()
        .filter(|note| note.starts_with("cache-stable:") && note.contains("deduped"))
        .count();
    assert_eq!(
        spoken, 1,
        "one note per turn carrying the dedup figures: {notices:?}"
    );
}

#[test]
fn inspection_dedup_stays_mid_turn_when_cache_stable_is_off() {
    let _guard = crate::tests::env_lock();
    let (mid_turn_rewrites, history, notices) = dedup_probe_turn("0");
    assert!(
        mid_turn_rewrites > 0,
        "default behavior is unchanged: dedup rewrites history between hops"
    );
    assert!(
        history
            .iter()
            .any(|m| m.content.contains(TOOL_DUPLICATE_MARK))
    );
    assert!(
        !notices.iter().any(|note| note.starts_with("cache-stable:")),
        "an inactive mode says nothing: {notices:?}"
    );
}

#[test]
fn aging_boundary_appends_marker_measures_savings_and_rewrites_once() {
    let _guard = crate::tests::env_lock();
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let _hops = EnvGuard::set("ANGEL_TOOL_AGE_KEEP_HOPS", "4");
    let _tokens = EnvGuard::set("ANGEL_TOOL_AGE_PROTECT_TOKENS", "0");
    let mut history = tool_hop_history(6, &"bulk ".repeat(1000));
    let len = history.len();
    let before: usize = history.iter().map(|m| m.content.len()).sum();
    let aged = age_tool_results_at_boundary(&mut history);
    assert_eq!(aged.results, 2);
    assert!(
        history
            .last()
            .unwrap()
            .content
            .starts_with("[tool-aging boundary:")
    );
    assert_eq!(history.len(), len + 1);
    let after: usize = history.iter().map(|m| m.content.len()).sum();
    assert_eq!(aged.bytes_saved, (before - after) as u64);
    assert!(aged.bytes_saved > history[len].content.len() as u64);
    let once = serde_json::to_vec(&history).unwrap();
    assert_eq!(age_tool_results_at_boundary(&mut history).bytes_saved, 0);
    assert_eq!(serde_json::to_vec(&history).unwrap(), once);
}

#[test]
fn p06c_retention_measures_real_aging_path_without_recounting() {
    let _guard = crate::tests::env_lock();
    let _off = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let meter = super::super::turn::background::Meter::default();
    let _scope = meter.enter(std::time::Instant::now());
    let mut history = tool_hop_history(6, &"x".repeat(5000));
    for message in history.iter().filter(|m| m.role == ChatRole::Tool) {
        super::super::turn::background::produced(
            message.tool_call_id.as_deref().unwrap(),
            message.content.len() as u64,
            message.content.len() as u64,
        );
    }
    let aged = age_tool_results(&mut history, 4, 0, 2048, false, 12);
    let output = super::super::trajectory::tools_output_snapshot();
    assert_eq!(output.produced_bytes, 30_000);
    assert_eq!(output.aged_bytes, aged.bytes_saved);
    assert_eq!(
        output.retained_bytes,
        history
            .iter()
            .filter(|m| m.role == ChatRole::Tool)
            .map(|m| m.content.len() as u64)
            .sum::<u64>()
    );
    assert_eq!(output.dropped_bytes, 0);
    age_tool_results(&mut history, 4, 0, 2048, false, 12);
    assert_eq!(super::super::trajectory::tools_output_snapshot(), output);
}

/// Drives the production turn loop and file tools with the cohort's generated
/// sources. No provider, network, run deadline, or hop ceiling is involved.
#[test]
fn tool_aging_c03d_long_workload_scripted_smoke() {
    let _lock = crate::tests::env_lock();
    let _stable = EnvGuard::set("ANGEL_CACHE_STABLE", "0");
    let _aging = EnvGuard::set("ANGEL_TOOL_AGING", "1");
    let _dedup = EnvGuard::set("ANGEL_TOOL_RESULT_DEDUP", "0");
    let _keep = EnvGuard::set("ANGEL_TOOL_AGE_KEEP_HOPS", "4");
    let _tokens = EnvGuard::set("ANGEL_TOOL_AGE_PROTECT_TOKENS", "8000");
    let _min = EnvGuard::set("ANGEL_TOOL_AGE_MIN_BYTES", "512");
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let _log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "0");
    let _recon = EnvGuard::set("ANGEL_TASK_RECON", "0");

    struct LongClub {
        extension: &'static str,
        requests: Mutex<Vec<usize>>,
        originals: Mutex<HashMap<String, String>>,
    }
    impl Club for LongClub {
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok("unused".into())
        }
        fn label(&self) -> &str {
            "c03d-long-scripted"
        }
        fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let mut requests = self.requests.lock().unwrap();
            let hop = requests.len();
            requests.push(serde_json::to_vec(messages).unwrap().len());
            for message in messages.iter().filter(|m| m.role == ChatRole::Tool) {
                if !message.content.contains(TOOL_AGED_MARK) {
                    self.originals
                        .lock()
                        .unwrap()
                        .entry(message.tool_call_id.clone().unwrap())
                        .or_insert_with(|| message.content.to_string());
                }
            }
            if hop >= 12 {
                return Ok(ClubReply::Text("Scripted long workload complete.".into()));
            }
            Ok(ClubReply::Calls((0..4).map(|i| ToolCall {
                id: format!("read{hop}-{i}"), name: "read_file".into(),
                args: serde_json::json!({"path":format!("src/shard_{}.{}", hop*4+i, self.extension)}),
            }).collect()))
        }
    }
    for (language, extension) in [("rust", "rs"), ("js", "mjs")] {
        let workspace = scratch(&format!("c03d-long-{language}"));
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../scripts/aging-parity-cohort.py");
        let generated = std::process::Command::new("python3").args(["-c",
            "import importlib.util,sys; from pathlib import Path; s=importlib.util.spec_from_file_location('cohort',sys.argv[1]); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); m.materialize_long(next(t for t in m.long_tasks() if t['language']==sys.argv[2]),Path(sys.argv[3]))"])
            .arg(script).arg(language).arg(&workspace).env("PYTHONDONTWRITEBYTECODE", "1").output().unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let artifacts = workspace.join("trajectory");
        let _dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", artifacts.to_str().unwrap());
        let club = LongClub {
            extension,
            requests: Mutex::new(Vec::new()),
            originals: Mutex::new(HashMap::new()),
        };
        let registry = ToolRegistry::with_team(workspace.clone(), Vec::new());
        let mut history = vec![ChatMsg::user(
            "Read the source shards in batches of four for this scripted aging measurement.",
        )];
        let (events, _rx) = mpsc::channel();
        let answer = run_turn(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            None,
            &events,
        )
        .unwrap();
        assert_eq!(answer, "Scripted long workload complete.");
        let requests = club.requests.lock().unwrap();
        assert_eq!(requests.len(), 13);
        assert!(
            requests[..6].iter().any(|bytes| *bytes >= 300_000),
            "{language}: {requests:?}"
        );
        let journal =
            std::fs::read_to_string(artifacts.join("evidence/parking-events.jsonl")).unwrap();
        let parked: Vec<Value> = journal
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(!parked.is_empty());
        let recorded_events: Vec<Value> = std::fs::read_dir(&artifacts)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
            .flat_map(|path| {
                std::fs::read_to_string(path)
                    .unwrap()
                    .lines()
                    .map(|line| serde_json::from_str::<Value>(line).unwrap())
                    .flat_map(|record| {
                        record["parking_events"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(
            recorded_events, parked,
            "the final ledger must reference every parked body"
        );
        for event in &parked {
            let original = std::fs::read(artifacts.join(event["path"].as_str().unwrap())).unwrap();
            assert_eq!(crate::cut::sha256_hex(&original), event["digest_sha256"]);
            assert_eq!(original.len(), event["original_bytes"]);
            assert_eq!(
                original,
                club.originals.lock().unwrap()[event["tool_call_id"].as_str().unwrap()].as_bytes()
            );
            assert!(event["hop"].is_u64());
        }
        println!(
            "C03d {language}: requests={requests:?}; parked={}; all bodies recovered byte-for-byte; artifacts={}",
            parked.len(),
            artifacts.display()
        );
    }
}

#[test]
fn tool_aging_c03e_threshold_counts_durable_receipt_bytes() {
    let _lock = crate::tests::env_lock();
    let root = scratch("c03e-threshold");
    let _dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", root.to_str().unwrap());
    let _log = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    // The bare receipt is smaller, but its digest makes it exceed 128 bytes.
    let mut history = tool_hop_history(2, &"x".repeat(128));
    let before = serde_json::to_vec(&history).unwrap();
    assert_eq!(
        age_tool_results(&mut history, 1, 0, 128, false, 0).results,
        0
    );
    assert_eq!(serde_json::to_vec(&history).unwrap(), before);
    assert!(!root.join("evidence/parking-events.jsonl").exists());
    // At the configured production floor, parking actually replaces the body.
    let mut history = tool_hop_history(2, &"y".repeat(512));
    let before: usize = history.iter().map(|m| m.content.len()).sum();
    let result = age_tool_results(&mut history, 1, 0, 512, false, 0);
    assert_eq!(result.results, 1);
    let after: usize = history.iter().map(|m| m.content.len()).sum();
    assert_eq!(result.bytes_saved, (before - after) as u64);
    assert!(root.join("evidence/parking-events.jsonl").is_file());
    assert_eq!(
        age_tool_results(&mut history, 1, 0, 512, false, 0).results,
        0
    );
}
