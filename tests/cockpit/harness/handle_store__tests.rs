use super::*;

fn limits() -> HandleStoreLimits {
    HandleStoreLimits {
        max_entries: 4,
        max_total_bytes: 10_000,
        max_body_bytes: 4_000,
        max_disclose_bytes: 64,
        max_paths: 2,
        max_preview_chars: 40,
    }
}

#[test]
fn put_returns_bounded_receipt_without_body() {
    let mut store = HandleStore::new();
    let body = "SECRET_BULK_MARKER\nfn main() {}\n".repeat(50);
    let receipt = store
        .put(
            &body,
            PutMeta {
                kind: HandleKind::ToolResult,
                producer: "read_file",
                identity: Some("read_file|src/main.rs"),
                paths: &["src/main.rs".into()],
                include_preview: false,
            },
            limits(),
            1,
        )
        .unwrap();
    let rendered = receipt.render();
    assert!(rendered.starts_with(HANDLE_RECEIPT_MARK));
    assert!(rendered.contains("hnd_"));
    assert!(rendered.contains("kind=tool_result"));
    assert!(rendered.contains("producer=read_file"));
    assert!(rendered.contains("identity=read_file|src/main.rs"));
    assert!(
        !rendered.contains("SECRET_BULK_MARKER"),
        "body must not leak into receipt: {rendered}"
    );
    assert_eq!(store.stats().entries, 1);
    assert_eq!(store.stats().total_bytes, body.len());
    assert!(store.get_receipt(receipt.handle.as_str()).is_some());
    assert!(store.content_sha256(receipt.handle.as_str()).is_some());
    assert!(store.contains(receipt.handle.as_str()));
    let _ = store
        .entries
        .get(receipt.handle.as_str())
        .map(|e| e.created_ms);
}

#[test]
fn disclose_is_capped_and_offsetable() {
    let mut store = HandleStore::new();
    let body = "abcdefghijklmnopqrstuvwxyz0123456789".repeat(4); // 144 bytes
    let receipt = store
        .put(
            &body,
            PutMeta {
                kind: HandleKind::Manual,
                producer: "test",
                identity: None,
                paths: &[],
                include_preview: false,
            },
            limits(),
            1,
        )
        .unwrap();
    let slice = store
        .disclose(receipt.handle.as_str(), 0, 10_000, limits(), 2)
        .unwrap();
    assert!(slice.truncated);
    assert_eq!(slice.bytes, 64);
    assert_eq!(slice.content, &body[..64]);
    let mid = store
        .disclose(receipt.handle.as_str(), 10, 5, limits(), 3)
        .unwrap();
    assert_eq!(mid.content, "klmno");
    assert_eq!(mid.offset, 10);
}

#[test]
fn eviction_respects_entry_and_byte_caps() {
    let mut store = HandleStore::new();
    let lim = HandleStoreLimits {
        max_entries: 2,
        max_total_bytes: 100,
        max_body_bytes: 80,
        max_disclose_bytes: 32,
        max_paths: 1,
        max_preview_chars: 20,
    };
    let r1 = store
        .put(
            &"a".repeat(40),
            PutMeta {
                kind: HandleKind::Manual,
                producer: "a",
                identity: None,
                paths: &[],
                include_preview: false,
            },
            lim,
            1,
        )
        .unwrap();
    let _r2 = store
        .put(
            &"b".repeat(40),
            PutMeta {
                kind: HandleKind::Manual,
                producer: "b",
                identity: None,
                paths: &[],
                include_preview: false,
            },
            lim,
            2,
        )
        .unwrap();
    assert_eq!(store.stats().entries, 2);
    let _r3 = store
        .put(
            &"c".repeat(40),
            PutMeta {
                kind: HandleKind::Manual,
                producer: "c",
                identity: None,
                paths: &[],
                include_preview: false,
            },
            lim,
            3,
        )
        .unwrap();
    assert_eq!(store.stats().entries, 2);
    assert!(!store.contains(r1.handle.as_str()), "oldest evicted");
    assert!(store.stats().evictions >= 1);
}

#[test]
fn root_trajectory_collapses_bulk_and_preserves_strategy() {
    let mut history = vec![
        ChatMsg::system("sys preamble that should be ignored"),
        ChatMsg::user("fix the bug"),
    ];
    history.push(ChatMsg::assistant_calls(vec![ToolCall {
        id: "1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path":"a.rs"}),
    }]));
    history.push(ChatMsg::tool("1", "x".repeat(5000)));
    history.push(ChatMsg::assistant_calls(vec![ToolCall {
        id: "2".into(),
        name: "str_replace".into(),
        args: serde_json::json!({"path":"a.rs"}),
    }]));
    history.push(ChatMsg::tool("2", "ok"));
    history.push(ChatMsg::assistant("done"));

    let short = root_trajectory(&history);

    // Longer bulk, same tool order → same strategy tokens for calls.
    history[3].content = "y".repeat(50_000).into();
    let long = root_trajectory(&history);

    assert_eq!(
        short
            .tokens
            .iter()
            .filter(|t| t.starts_with("call:"))
            .collect::<Vec<_>>(),
        long.tokens
            .iter()
            .filter(|t| t.starts_with("call:"))
            .collect::<Vec<_>>(),
    );
    assert_eq!(short.tool_hops, long.tool_hops);
    assert!(short.bulk_tool_results >= 1);

    // Handle-backed receipts classify as handle, not bulk.
    history[3].content = format!(
        "{TOOL_AGED_MARK}: read_file|a.rs (5000 bytes) handle=hnd_1 — re-run the tool if needed]"
    )
    .into();
    let handled = root_trajectory(&history);
    assert_eq!(handled.handle_receipts, 1);
    // The small "ok" write result remains a bulk token; the large read does not.
    assert_eq!(handled.bulk_tool_results, 1);
    assert!(handled.tokens.iter().any(|t| t == "tool:handle"));
    assert!(
        !handled
            .tokens
            .iter()
            .any(|t| t.starts_with("tool:bulk:xl") || t.starts_with("tool:bulk:xxl"))
    );
}

#[test]
fn trajectory_similarity_is_high_for_isomorphic_strategies() {
    let a = RootTrajectory {
        tokens: vec![
            "user:s".into(),
            "call:read_file".into(),
            "tool:handle".into(),
            "call:str_replace".into(),
            "tool:bulk:xs".into(),
            "asst:s".into(),
        ],
        root_chars: 200,
        tool_hops: 2,
        handle_receipts: 1,
        aged_receipts: 0,
        bulk_tool_results: 1,
    };
    let b = a.clone();
    let sim = trajectory_similarity(&a, &b);
    assert!((sim.jaccard_3gram - 1.0).abs() < 1e-9);
    assert!((sim.weighted_jaccard_3gram - 1.0).abs() < 1e-9);
    assert!((sim.token_levenshtein_sim - 1.0).abs() < 1e-9);

    let mut c = a.clone();
    c.tokens[2] = "tool:bulk:xxl".into(); // bulk instead of handle
    let sim2 = trajectory_similarity(&a, &c);
    assert!(sim2.token_levenshtein_sim < 1.0);
    assert!(sim2.jaccard_3gram < 1.0);
    assert!(sim2.weighted_jaccard_3gram < 1.0);
}

#[test]
fn handle_kind_round_trips() {
    for kind in [
        HandleKind::ToolResult,
        HandleKind::CodeMode,
        HandleKind::Subcall,
        HandleKind::Manual,
    ] {
        assert_eq!(HandleKind::parse(kind.as_str()), Some(kind));
    }
}

#[test]
fn session_put_and_disclose_round_trip_when_enabled() {
    let _lock = crate::tests::env_lock();
    let _on = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    session_clear();
    let body = "payload-line-one\npayload-line-two\n";
    let receipt = session_put(
        body,
        PutMeta {
            kind: HandleKind::ToolResult,
            producer: "read_file",
            identity: Some("read_file|x.rs"),
            paths: &["x.rs".into()],
            include_preview: false,
        },
    )
    .expect("store enabled");
    let slice = session_disclose(receipt.handle.as_str(), 0, 1024).unwrap();
    assert_eq!(slice.content, body);
    assert!(!slice.truncated);
    session_clear();
}

#[test]
fn session_put_disabled_returns_none() {
    let _lock = crate::tests::env_lock();
    let _off = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    session_clear();
    assert!(
        session_put(
            "body",
            PutMeta {
                kind: HandleKind::Manual,
                producer: "t",
                identity: None,
                paths: &[],
                include_preview: false,
            },
        )
        .is_none()
    );
}

#[test]
fn maybe_offload_root_body_respects_floor_and_store() {
    let _lock = crate::tests::env_lock();
    let _on = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    session_clear();
    let small = "tiny";
    assert!(
        maybe_offload_root_body(small, HandleKind::Subcall, "spawn", "spawn|panel", 100).is_none()
    );
    let large = "BULK_SUBCALL_".to_string() + &"z".repeat(200);
    let receipt = maybe_offload_root_body(&large, HandleKind::Subcall, "spawn", "spawn|panel", 64)
        .expect("large body offloads");
    assert!(receipt.contains(HANDLE_RECEIPT_MARK) || receipt.contains("hnd_"));
    assert!(!receipt.contains("BULK_SUBCALL_"));
    let handle = receipt
        .split("hnd_")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_alphanumeric()).next())
        .map(|s| format!("hnd_{s}"))
        .unwrap();
    let slice = session_disclose(&handle, 0, 512).unwrap();
    assert!(slice.content.contains("BULK_SUBCALL_"));
    session_clear();
}

#[test]
fn eager_offload_parks_large_inspection_keeps_mutations() {
    let _lock = crate::tests::env_lock();
    let _store = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", "1");
    let _min = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", "64");
    session_clear();

    let bulk = "READ_BODY_".to_string() + &"x".repeat(200);
    let off = eager_offload_tool_result("read_file", bulk.clone(), Some("read_file|a.rs"));
    assert!(off.offloaded, "large inspection should offload");
    assert!(off.content.contains(HANDLE_RECEIPT_MARK) || off.content.contains("hnd_"));
    assert!(!off.content.contains("READ_BODY_"));
    assert!(off.bytes_saved > 0);

    let write = eager_offload_tool_result("write_file", bulk, Some("write_file|a.rs"));
    assert!(!write.offloaded, "mutations stay complete");
    assert!(write.content.contains("READ_BODY_"));

    let err = eager_offload_tool_result(
        "read_file",
        "tool error: missing".into(),
        Some("read_file|x"),
    );
    assert!(!err.offloaded);
    session_clear();
}

#[test]
fn eager_offload_keeps_body_when_disclosure_tool_is_disabled() {
    let _lock = crate::tests::env_lock();
    let _store = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", "0");
    let _min = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", "64");
    session_clear();

    let body = "READ_BODY_".to_string() + &"x".repeat(200);
    let off = eager_offload_tool_result("read_file", body.clone(), Some("read_file|a.rs"));
    assert!(!off.offloaded);
    assert_eq!(off.content, body);
    session_clear();
}

#[test]
fn inspection_identity_for_offload_skips_when_cannot_park() {
    use crate::agent::club::ToolCall;
    let _lock = crate::tests::env_lock();
    let _store = EnvGuard::set("ANGEL_HANDLE_STORE", "1");
    let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", "1");
    let _min = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", "64");

    let read = ToolCall {
        id: "c1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/lib.rs"}),
    };
    let write = ToolCall {
        id: "c2".into(),
        name: "write_file".into(),
        args: serde_json::json!({"path": "src/lib.rs", "content": "x"}),
    };
    let grep = ToolCall {
        id: "c3".into(),
        name: "grep".into(),
        args: serde_json::json!({"pattern": "needle".repeat(20_000)}),
    };

    assert!(
        !eager_offload_can_park("write_file", &"x".repeat(200)),
        "mutations never park"
    );
    assert!(
        !eager_offload_can_park("read_file", "tiny"),
        "tiny inspections skip identity"
    );
    assert!(
        !eager_offload_can_park("read_file", "tool error: missing"),
        "errors skip identity"
    );
    assert!(eager_offload_can_park(
        "read_file",
        &("READ_BODY_".to_string() + &"x".repeat(200))
    ));

    assert!(inspection_identity_for_offload(&write, &"x".repeat(200)).is_none());
    assert!(inspection_identity_for_offload(&read, "tiny").is_none());
    assert!(inspection_identity_for_offload(&grep, "tiny").is_none());
    let parked =
        inspection_identity_for_offload(&read, &("READ_BODY_".to_string() + &"x".repeat(200)))
            .expect("large eligible read still identities");
    assert_eq!(parked, "read_file|src/lib.rs");
    assert_eq!(
        inspection_identity_for_offload(&read, &("READ_BODY_".to_string() + &"x".repeat(200))),
        crate::agent::harness::inspection_identity(&read)
    );
}

/// Local env guard so this module's tests do not depend on harness::tests
/// private helpers beyond `env_lock`.
struct EnvGuard {
    key: &'static str,
    old: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        // SAFETY: tests hold env_lock when mutating process env.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(old) = &self.old {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(self.key, old) };
        } else {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}
