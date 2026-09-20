//! Hop classification, inspection identity, and churn/error detection.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- hop progress suite ---

#[test]
fn classify_hop_distinguishes_advance_reread_neutral() {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert("read_file|a.rs".into());

    // a NEW single-file read advances
    let new_read = vec![tc("read_file", serde_json::json!({"path": "b.rs"}))];
    assert_eq!(classify_hop(&new_read, &seen), HopProgress::Advanced);

    // re-reading a known file (only) is a re-read
    let reread = vec![tc("read_file", serde_json::json!({"path": "a.rs"}))];
    assert_eq!(classify_hop(&reread, &seen), HopProgress::Reread);

    // a write/effect always advances, even alongside a re-read
    let write = vec![
        tc("read_file", serde_json::json!({"path": "a.rs"})),
        tc(
            "write_file",
            serde_json::json!({"path": "a.rs", "content": "x"}),
        ),
    ];
    assert_eq!(classify_hop(&write, &seen), HopProgress::Advanced);
    let exec = vec![tc("shell", serde_json::json!({"cmd": "cargo test"}))];
    assert_eq!(classify_hop(&exec, &seen), HopProgress::Advanced);

    // Read-only shell fallbacks participate in churn detection. Shell remains
    // effectful everywhere else; only a conservative inspection allowlist is
    // classified here.
    let shell_read = vec![tc(
        "shell",
        serde_json::json!({"cmd": "sed -n '1,200p' src/lib.rs"}),
    )];
    assert_eq!(classify_hop(&shell_read, &seen), HopProgress::Advanced);
    seen.insert(inspection_identities(&shell_read)[0].clone());
    assert_eq!(classify_hop(&shell_read, &seen), HopProgress::Reread);
    let shell_search = vec![tc(
        "shell",
        serde_json::json!({"cmd": "rg TODO src | head -20"}),
    )];
    assert_eq!(classify_hop(&shell_search, &seen), HopProgress::Advanced);

    for effect in [
        "sed -i 's/a/b/' src/lib.rs",
        "find src -delete",
        "rg TODO src > findings.txt",
        "git status && rm -rf build",
    ] {
        let calls = vec![tc("shell", serde_json::json!({"cmd": effect}))];
        assert!(inspection_identities(&calls).is_empty(), "{effect}");
        assert_eq!(classify_hop(&calls, &seen), HopProgress::Advanced);
    }

    // A new search advances; repeating it (even with a new call id) is a re-read.
    let grep = vec![tc("grep", serde_json::json!({"pattern": "x"}))];
    assert_eq!(classify_hop(&grep, &seen), HopProgress::Advanced);
    seen.insert(inspection_identities(&grep)[0].clone());
    assert_eq!(classify_hop(&grep, &seen), HopProgress::Reread);

    // Polling a background handle observes state; changing uptime/log prose in
    // the result must not make the same proc_status call count as new work.
    let poll = vec![tc("proc_status", serde_json::json!({"id": 10}))];
    assert_eq!(classify_hop(&poll, &seen), HopProgress::Advanced);
    seen.insert(inspection_identities(&poll)[0].clone());
    assert_eq!(classify_hop(&poll, &seen), HopProgress::Reread);

    // a known re-read PLUS a new read still advances (new read wins)
    let mixed = vec![
        tc("read_file", serde_json::json!({"path": "a.rs"})),
        tc("read_file", serde_json::json!({"path": "c.rs"})),
    ];
    assert_eq!(classify_hop(&mixed, &seen), HopProgress::Advanced);
}

#[test]
fn classify_hop_with_identities_matches_and_skips_rebuild() {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert("read_file|a.rs".into());

    let reread = vec![tc("read_file", serde_json::json!({"path": "a.rs"}))];
    let reread_ids = inspection_identities(&reread);
    assert_eq!(reread_ids, vec!["read_file|a.rs".to_string()]);
    assert_eq!(
        classify_hop_with_identities(&reread, &seen, &reread_ids),
        classify_hop(&reread, &seen)
    );
    assert_eq!(
        classify_hop_with_identities(&reread, &seen, &reread_ids),
        HopProgress::Reread
    );

    let write = vec![tc(
        "write_file",
        serde_json::json!({"path": "a.rs", "content": "x"}),
    )];
    let write_ids = inspection_identities(&write);
    assert!(
        write_ids.len() < write.len(),
        "writes have no inspection identity — that is the effect signal"
    );
    assert_eq!(
        classify_hop_with_identities(&write, &seen, &write_ids),
        HopProgress::Advanced
    );
    assert_eq!(
        classify_hop_with_identities(&write, &seen, &write_ids),
        classify_hop(&write, &seen)
    );

    let mixed = vec![
        tc("read_file", serde_json::json!({"path": "a.rs"})),
        tc(
            "write_file",
            serde_json::json!({"path": "b.rs", "content": "y"}),
        ),
    ];
    let mixed_ids = inspection_identities(&mixed);
    assert_eq!(mixed_ids.len(), 1);
    assert_eq!(
        classify_hop_with_identities(&mixed, &seen, &mixed_ids),
        HopProgress::Advanced
    );

    let empty: Vec<ToolCall> = Vec::new();
    assert_eq!(
        classify_hop_with_identities(&empty, &seen, &[]),
        HopProgress::Neutral
    );
}

#[test]
fn hop_progress_for_loop_skips_classify_when_not_applied() {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert("read_file|a.rs".into());
    let reread = vec![tc("read_file", serde_json::json!({"path": "a.rs"}))];
    let ids = inspection_identities(&reread);

    assert!(
        !churn_classify_applied(6),
        "2026-08-17 +++ does not apply synthetic reread stops"
    );
    assert!(!churn_classify_applied(0));
    assert_eq!(
        hop_progress_for_loop(false, &reread, &seen, &ids),
        HopProgress::Neutral,
        "unused classify must not walk the hop"
    );
    assert_eq!(
        hop_progress_for_loop(true, &reread, &seen, &ids),
        HopProgress::Reread,
        "when re-enabled, the shipped classifier still sees rereads"
    );
}

#[test]
fn inspection_identities_for_loop_skips_when_not_applied() {
    let grep = tc("grep", serde_json::json!({"pattern": "x"}));
    let calls = vec![
        tc("read_file", serde_json::json!({"path": "a.rs"})),
        grep.clone(),
    ];
    assert!(
        !churn_classify_applied(6),
        "default hops must not classify no-progress"
    );
    let skipped = inspection_identities_for_loop(false, &calls);
    assert!(
        skipped.is_empty(),
        "ordinary hops must not allocate inspection identities"
    );
    let applied = inspection_identities_for_loop(true, &calls);
    assert_eq!(applied, inspection_identities(&calls));
    assert_eq!(applied[0], "read_file|a.rs");
    assert_eq!(
        applied[1],
        format!("grep|{}", payload_fingerprint(&grep.args))
    );
}

#[test]
fn inspection_identities_cover_scoped_reads_and_broad_searches() {
    let grep = tc("grep", serde_json::json!({"pattern": "x"}));
    let other = tc("grep", serde_json::json!({"pattern": "y"}));
    let calls = vec![
        tc("read_file", serde_json::json!({"path": "a.rs"})),
        grep.clone(),
        tc("shell", serde_json::json!({"cmd": "ls"})),
    ];
    let ids = inspection_identities(&calls);
    assert_eq!(ids[0], "read_file|a.rs");
    assert_eq!(ids[2], "shell|ls");
    assert_eq!(
        ids[1],
        format!("grep|{}", payload_fingerprint(&grep.args)),
        "broad-search identity must hash args, not Display-serialize them"
    );
    assert!(
        !ids[1].contains("pattern"),
        "raw grep args must stay out of the identity"
    );
    assert_ne!(
        inspection_identity(&grep),
        inspection_identity(&other),
        "distinct patterns must not collide"
    );

    let huge = "needle".repeat(20_000);
    assert!(huge.len() > 100_000);
    let large = tc("grep", serde_json::json!({"pattern": huge}));
    let large_id = inspection_identity(&large).expect("pathless grep has an identity");
    assert!(
        large_id.len() < 64,
        "large grep patterns must not inflate the identity, got {}",
        large_id.len()
    );
    assert!(
        !large_id.contains("needle"),
        "raw pattern must not enter the identity"
    );
}

/// The streak logic the guard runs: re-reads accumulate, an advance resets,
/// neutral hops are ignored, and the nudge fires exactly at the limit.
#[test]
fn churn_streak_accumulates_and_resets() {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert("read_file|a.rs".into());
    let limit = 3usize;
    let mut streak = 0usize;
    let mut nudges = 0usize;
    let step = |streak: &mut usize, nudges: &mut usize, prog: HopProgress| {
        match prog {
            HopProgress::Advanced => *streak = 0,
            HopProgress::Reread => *streak += 1,
            HopProgress::Neutral => {}
        }
        if *streak == limit {
            *nudges += 1;
        }
    };
    // reread, reread, neutral (ignored), reread -> hits 3 -> one nudge
    step(&mut streak, &mut nudges, HopProgress::Reread);
    step(&mut streak, &mut nudges, HopProgress::Reread);
    step(&mut streak, &mut nudges, HopProgress::Neutral);
    step(&mut streak, &mut nudges, HopProgress::Reread);
    assert_eq!((streak, nudges), (3, 1));
    // an advancing hop resets; further re-reads can nudge again later
    step(&mut streak, &mut nudges, HopProgress::Advanced);
    assert_eq!(streak, 0);
}

#[test]
fn is_error_result_only_flags_dispatch_failures() {
    assert!(is_error_result("tool error: unknown tool: foo"));
    assert!(is_error_result("tool error: worker panicked"));
    // Successful runs that *report* failure are not dispatch errors.
    assert!(!is_error_result("3 tests failed, 10 passed"));
    assert!(!is_error_result("no files match \"*.zzz\""));
    assert!(!is_error_result(""));
}

#[test]
fn context_overflow_error_detection_is_specific() {
    for error in [
        "HTTP 413: payload too large",
        "context_length_exceeded",
        "maximum context length is 32768 tokens",
        "input token count exceeds the model limit",
        "request exceeds the context window",
        "HTTP 400: request (104239 tokens) exceeds the available context size (102400 tokens); type=exceed_context_size_error",
    ] {
        assert!(is_context_overflow_error(error), "{error}");
    }
    for error in [
        "HTTP 429: rate limited",
        "provider returned empty stdout",
        "context cache temporarily unavailable",
        "connection reset",
    ] {
        assert!(!is_context_overflow_error(error), "{error}");
    }
}

fn tc_id(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        args: serde_json::json!({}),
    }
}

#[test]
fn unique_tool_call_ids_skip_reserved_set_repair() {
    let mut calls = vec![tc_id("c1", "read_file"), tc_id("c2", "grep")];
    let history = vec![
        ChatMsg::user("go"),
        ChatMsg::assistant_calls(vec![tc_id("prior", "read_file")]),
        ChatMsg::tool("prior", "ok"),
    ];
    assert!(
        !tool_call_ids_need_repair(&calls, &history),
        "fresh unique provider IDs must not pay reserved-set repair"
    );
    let original = calls.iter().map(|call| call.id.clone()).collect::<Vec<_>>();
    assert_eq!(normalize_tool_call_ids(&mut calls, &history, 3), 0);
    assert_eq!(
        calls
            .iter()
            .map(|call| call.id.as_str())
            .collect::<Vec<_>>(),
        original.iter().map(String::as_str).collect::<Vec<_>>(),
        "valid unused IDs stay byte-for-byte"
    );
}

#[test]
fn blank_duplicate_or_reused_tool_call_ids_still_repair() {
    let history = vec![
        ChatMsg::assistant_calls(vec![tc_id("used", "read_file")]),
        ChatMsg::tool("used", "ok"),
    ];
    assert!(tool_call_ids_need_repair(
        &[tc_id("", "read_file")],
        &history
    ));
    assert!(tool_call_ids_need_repair(
        &[tc_id(" \t", "read_file")],
        &history
    ));
    assert!(tool_call_ids_need_repair(
        &[tc_id("dup", "read_file"), tc_id("dup", "grep")],
        &history
    ));
    assert!(tool_call_ids_need_repair(
        &[tc_id("used", "read_file")],
        &history
    ));

    let mut reused = vec![tc_id("used", "read_file")];
    assert_eq!(normalize_tool_call_ids(&mut reused, &history, 2), 1);
    assert_eq!(reused[0].id, "angel_h2_call_0");

    let mut blanks = vec![tc_id("", "read_file"), tc_id("keep", "grep")];
    assert_eq!(normalize_tool_call_ids(&mut blanks, &[], 1), 1);
    assert_eq!(blanks[0].id, "angel_h1_call_0");
    assert_eq!(blanks[1].id, "keep");
}

/// One unique provider ID is the default hop. It must not allocate a batch
/// set, still skip reserved-set repair, and still detect a reused spelling.
#[test]
fn single_call_hops_skip_batch_id_set() {
    let history = vec![
        ChatMsg::user("go"),
        ChatMsg::assistant_calls(vec![tc_id("prior", "read_file"), tc_id("other", "grep")]),
        ChatMsg::tool("prior", "ok"),
        ChatMsg::tool("other", "ok"),
    ];
    let mut fresh = vec![tc_id("c1", "read_file")];
    assert!(
        !tool_call_ids_need_repair(&fresh, &history),
        "a single unused provider ID must skip repair"
    );
    assert_eq!(normalize_tool_call_ids(&mut fresh, &history, 4), 0);
    assert_eq!(fresh[0].id, "c1");

    assert!(
        !tool_call_ids_need_repair(&[], &history),
        "an empty batch has nothing to repair"
    );

    assert!(
        tool_call_ids_need_repair(&[tc_id("prior", "read_file")], &history),
        "a single reused assistant-call ID still repairs"
    );
    assert!(
        tool_call_ids_need_repair(&[tc_id("other", "grep")], &history),
        "a single reused tool-result ID still repairs"
    );
    assert!(tool_call_ids_need_repair(
        &[tc_id("", "read_file")],
        &history
    ));

    let mut reused = vec![tc_id("prior", "read_file")];
    assert_eq!(normalize_tool_call_ids(&mut reused, &history, 5), 1);
    assert_eq!(reused[0].id, "angel_h5_call_0");
}

/// Default hops summarize `read_file {path}` without a parts Vec + join.
/// Multi-key and long values keep the same truncated `k=v` spelling.
#[test]
fn summarize_args_writes_tiny_objects_in_place() {
    assert_eq!(summarize_args(&serde_json::json!({})), "");
    assert_eq!(
        summarize_args(&serde_json::json!({"path": "src/main.rs"})),
        "path=src/main.rs"
    );
    assert_eq!(summarize_args(&serde_json::json!({"path": ""})), "path=");
    let long = "x".repeat(80);
    assert_eq!(
        summarize_args(&serde_json::json!({"path": long})),
        format!("path={}", "x".repeat(60))
    );
    let two = summarize_args(&serde_json::json!({"path": "a.rs", "offset": 1}));
    assert!(two.contains("path=a.rs"), "{two}");
    assert!(two.contains("offset=1"), "{two}");
    assert!(two.contains(", "), "{two}");
    assert_eq!(summarize_args(&serde_json::json!("hello")), "\"hello\"");
}
