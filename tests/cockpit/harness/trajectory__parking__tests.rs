use super::*;
#[test]
fn trajectory_c03c_parking_survives_trim_compaction_and_turn_reset() {
    let _lock = crate::tests::env_lock();
    use crate::harness::tests::EnvGuard;
    let root = std::env::temp_dir().join(format!("c03c-parking-{}", now_ms()));
    let _dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", root.to_str().unwrap());
    let _enabled = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let club = crate::club::HttpClub::new("fixture", "http://127.0.0.1:9/v1", "fixture", None);
    reset_turn_ledger(&club);
    let original = "original π\n".repeat(200);
    let digest = crate::cut::sha256_hex(original.as_bytes());
    let mut message = ChatMsg::tool("call-a", original.as_str());
    message.content = park_tool_result(&message, "[tool output elided: excerpt]", "aging")
        .unwrap()
        .into();
    message.content = park_tool_result(&message, "[tool output elided]", "excerpt_trim")
        .unwrap()
        .into();
    let summary = park_compaction_window(&[message], "summary").unwrap();
    assert!(summary.contains(&digest));
    let second = park_compaction_window(&[ChatMsg::harness(summary)], "summary again").unwrap();
    assert!(second.contains(&digest));
    reset_turn_ledger(&club);
    assert_eq!(resolve(&root, &digest).unwrap(), original.as_bytes());
    let journal = std::fs::read_to_string(root.join("evidence/parking-events.jsonl")).unwrap();
    let events: Vec<Value> = journal
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 3);
    for event in &events {
        assert_eq!(event["digest_sha256"], digest);
        assert_eq!(event["original_bytes"], original.len());
        assert_eq!(event["path"], format!("evidence/{digest}"));
        assert_eq!(
            std::fs::read(root.join(event["path"].as_str().unwrap())).unwrap(),
            original.as_bytes()
        );
    }
    assert_eq!(events[2]["retained_bytes"], 0);
    std::fs::write(root.join("evidence").join(&digest), "corrupt").unwrap();
    assert!(resolve(&root, &digest).is_err());
    assert!(resolve(&root, "../escape").is_err());
}

#[test]
fn trajectory_c03c_tool_aging_hook_keeps_originals() {
    let _lock = crate::tests::env_lock();
    use crate::harness::tests::EnvGuard;
    let root = std::env::temp_dir().join(format!("c03c-aging-{}", now_ms()));
    let _dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", root.to_str().unwrap());
    let _enabled = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let _handles = EnvGuard::set("ANGEL_HANDLE_STORE", "0");
    let mut history = vec![];
    for i in 0..4 {
        history.push(ChatMsg::assistant_calls(vec![ToolCall {
            id: format!("call-{i}"),
            name: "read_file".into(),
            args: serde_json::json!({"path":format!("file-{i}")}),
        }]));
        history.push(ChatMsg::tool(
            format!("call-{i}"),
            format!("file {i} {}", "π".repeat(1000)),
        ));
    }
    let original = history[1].content.clone();
    let result = age_tool_results(&mut history, 1, 0, 128, false, 0);
    assert!(result.results > 0);
    let digest = receipt_digest(&history[1].content).unwrap();
    assert_eq!(resolve(&root, digest).unwrap(), original.as_bytes());
}

#[test]
fn tool_aging_retains_without_trajectory_logging() {
    let _lock = crate::tests::env_lock();
    use crate::harness::tests::EnvGuard;
    let root = std::env::temp_dir().join(format!("c03f-retention-{}", now_ms()));
    let _dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", root.to_str().unwrap());
    let _logging = EnvGuard::unset("ANGEL_TRAJECTORY_LOG");
    let original = "durable original π\n".repeat(200);
    let receipt = park_tool_result(
        &ChatMsg::tool("c03f", original.as_str()),
        "[tool output elided]",
        "aging",
    )
    .unwrap();
    let digest = receipt_digest(&receipt).unwrap();
    assert_eq!(resolve(&root, digest).unwrap(), original.as_bytes());
    let journal = std::fs::read_to_string(root.join("evidence/parking-events.jsonl")).unwrap();
    let event: Value = serde_json::from_str(journal.lines().next().unwrap()).unwrap();
    assert_eq!(event["digest_sha256"], digest);
    assert_eq!(event["path"], format!("evidence/{digest}"));
    assert_eq!(journal.lines().count(), 1);
}

#[test]
fn trajectory_c03c_parking_refuses_unwritable_store() {
    let _lock = crate::tests::env_lock();
    use crate::harness::tests::EnvGuard;
    let root = std::env::temp_dir().join(format!("c03c-blocked-{}", now_ms()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("evidence"), "block").unwrap();
    let _dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", root.to_str().unwrap());
    let _enabled = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    assert!(park_tool_result(&ChatMsg::tool("a", "original"), "short", "aging").is_err());
}

#[test]
fn parked_originals_are_redacted_like_trajectory_writes() {
    let _lock = crate::tests::env_lock();
    use crate::harness::tests::EnvGuard;
    let root = std::env::temp_dir().join(format!("c03g-redact-{}", now_ms()));
    let _dir = EnvGuard::set("ANGEL_TRAJECTORY_DIR", root.to_str().unwrap());
    let _enabled = EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1");
    let secret = "c03g-park-secret-value";
    let _key = EnvGuard::set("ANGEL_OWNED_TEST_KEY", secret);
    let original = format!("tool dump {secret} trailing payload\n").repeat(40);
    let receipt = park_tool_result(
        &ChatMsg::tool("c03g", original.as_str()),
        "[tool output elided]",
        "aging",
    )
    .unwrap();
    let digest = receipt_digest(&receipt).unwrap();
    let stored = resolve(&root, digest).unwrap();
    let text = String::from_utf8(stored.clone()).unwrap();
    assert!(!text.contains(secret), "parked original leaked secret");
    assert!(
        text.contains("«redacted:ANGEL_OWNED_TEST_KEY»"),
        "expected named redaction marker, got {text}"
    );
    let redacted = crate::secrets::redact_bytes(original.as_bytes());
    assert_eq!(stored, redacted);
    assert_eq!(crate::cut::sha256_hex(&stored), digest);
    let journal = std::fs::read_to_string(root.join("evidence/parking-events.jsonl")).unwrap();
    let event: Value = serde_json::from_str(journal.lines().next().unwrap()).unwrap();
    assert_eq!(event["digest_sha256"], digest);
    assert_eq!(event["original_bytes"], stored.len());
}
