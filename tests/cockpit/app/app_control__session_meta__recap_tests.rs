use super::*;

fn calls(calls: &[(&str, serde_json::Value)]) -> ChatMsg {
    ChatMsg::assistant_calls(
        calls
            .iter()
            .enumerate()
            .map(|(index, (name, args))| crate::club::ToolCall {
                id: format!("call-{index}"),
                name: (*name).to_string(),
                args: args.clone(),
            })
            .collect(),
    )
}

#[test]
fn recap_counts_and_orders_tools_deterministically() {
    let history = vec![
        ChatMsg::user("repair\n the parser"),
        calls(&[
            ("shell", serde_json::json!({"command": "cargo test"})),
            ("read_file", serde_json::json!({"path": "src/lib.rs"})),
        ]),
        ChatMsg::tool("call-0", "ok"),
        ChatMsg::tool("call-1", "source"),
        calls(&[
            ("read_file", serde_json::json!({"path": "src/lib.rs"})),
            ("shell", serde_json::json!({"command": "cargo fmt"})),
            (
                "str_replace",
                serde_json::json!({"path": "src/lib.rs", "old": "a", "new": "b"}),
            ),
        ]),
        ChatMsg::tool("call-2", "source"),
        ChatMsg::assistant("fixed\nand verified"),
    ];

    let recap = session_recap_text(&history);

    assert!(
        recap.contains("1 user · 1 replies · 3 tool results"),
        "{recap}"
    );
    assert!(
        recap.contains("top tools read_file×2 · shell×2 · str_replace×1"),
        "{recap}"
    );
    assert!(recap.contains("touched  src/lib.rs"), "{recap}");
    assert!(recap.contains("latest › repair the parser"), "{recap}");
    assert!(recap.contains("angel  › fixed and verified"), "{recap}");
}

#[test]
fn recap_lists_latest_unique_mutation_paths_and_stays_bounded() {
    let long_path = format!("src/{}/latest.rs", "deep/".repeat(20));
    let history = vec![
        calls(&[(
            "write_file",
            serde_json::json!({"path": "src/old.rs", "content": "old"}),
        )]),
        calls(&[(
            "str_replace",
            serde_json::json!({"path": "src/old.rs", "old": "a", "new": "b"}),
        )]),
        calls(&[(
            "multi_edit",
            serde_json::json!({
                "path": long_path,
                "edits": [{"old": "before", "new": "after"}]
            }),
        )]),
        calls(&[(
            "apply_patch",
            serde_json::json!({
                "diff": "*** Begin Patch\n*** Update File: src/patch-a.rs\n@@\n-old\n+new\n*** Update File: src/patch-b.rs\n@@\n-old\n+new\n*** End Patch\n"
            }),
        )]),
        ChatMsg::user("word ".repeat(200)),
        ChatMsg::assistant("answer ".repeat(200)),
    ];

    let recap = session_recap_text(&history);
    let touched = recap.lines().find(|line| line.contains("touched")).unwrap();

    assert!(touched.contains("src/patch-b.rs"), "{touched}");
    assert!(touched.contains("src/patch-a.rs"), "{touched}");
    assert!(touched.contains("latest.rs"), "{touched}");
    assert!(!touched.contains("src/old.rs"), "{touched}");
    assert_eq!(recap.lines().count(), 6, "{recap}");
    assert!(
        recap.lines().all(|line| line.chars().count() <= 110),
        "{recap}"
    );
}

#[test]
fn empty_recap_is_explicit() {
    let recap = session_recap_text(&[]);
    assert!(recap.contains("0 user · 0 replies · 0 tool results"));
    assert!(recap.contains("top tools none"));
    assert!(recap.contains("touched  none"));
    assert!(recap.contains("latest › none"));
    assert!(recap.contains("angel  › none"));
}
