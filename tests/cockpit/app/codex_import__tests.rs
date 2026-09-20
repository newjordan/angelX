
use super::*;

#[test]
fn parse_maps_roles_and_skips_non_messages() {
    let rollout = r#"{"type":"session_meta","payload":{"id":"x"}}
{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"system prompt"}]}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hi there"}]}}
{"type":"response_item","payload":{"type":"reasoning","summary":[],"encrypted_content":"zzz"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hello"}]}}
{"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{}","call_id":"c1"}}
"#;
    let msgs = parse_rollout(rollout);
    assert_eq!(msgs.len(), 3, "3 messages; reasoning/function_call dropped");
    assert!(matches!(msgs[0].role, ChatRole::System));
    assert_eq!(&*msgs[0].content, "system prompt");
    assert!(matches!(msgs[1].role, ChatRole::User));
    assert_eq!(&*msgs[1].content, "hi there");
    assert!(matches!(msgs[2].role, ChatRole::Assistant));
    assert_eq!(&*msgs[2].content, "hello");
}

#[test]
fn parse_rollout_joins_multipart_text_and_skips_blank_and_unknown() {
    let rollout = concat!(
        // multi-part assistant message → parts joined verbatim.
        r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"foo "},{"text":"bar"}]}}"#,
        "\n",
        // unknown role → dropped.
        r#"{"type":"response_item","payload":{"type":"message","role":"tool","content":[{"text":"ignored"}]}}"#,
        "\n",
        // whitespace-only text → dropped.
        r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"   "}]}}"#,
        "\n",
        // content not an array → empty text → dropped.
        r#"{"type":"response_item","payload":{"type":"message","role":"user","content":"plain"}}"#,
        "\n",
        // missing payload → dropped.
        r#"{"type":"response_item"}"#,
        "\n",
        // not valid JSON → skipped, not an error.
        "this is not json{",
        "\n",
        // a real user line at the end.
        r#"{"type":"response_item","payload":{"type":"message","role":"system","content":[{"text":"sys"}]}}"#,
        "\n"
    );
    let msgs = parse_rollout(rollout);
    assert_eq!(
        msgs.len(),
        2,
        "only the multipart assistant + the system line survive"
    );
    assert!(matches!(msgs[0].role, ChatRole::Assistant));
    assert_eq!(
        &*msgs[0].content, "foo bar",
        "parts are concatenated in order"
    );
    assert!(matches!(msgs[1].role, ChatRole::System));
    assert_eq!(&*msgs[1].content, "sys");
}

#[test]
fn parse_rollout_empty_input_is_empty() {
    assert!(parse_rollout("").is_empty());
    assert!(parse_rollout("\n\n").is_empty());
}

#[test]
fn collect_rollouts_recurses_and_filters_by_name_and_extension() {
    let root = std::env::temp_dir().join(format!("codex_import_collect_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let nested = root.join("2026").join("06").join("22");
    std::fs::create_dir_all(&nested).unwrap();
    // A valid rollout, a non-rollout .jsonl, and a wrong-extension file.
    std::fs::write(nested.join("rollout-abc.jsonl"), "{}").unwrap();
    std::fs::write(nested.join("notes.jsonl"), "{}").unwrap();
    std::fs::write(nested.join("rollout-xyz.txt"), "{}").unwrap();

    let mut found = Vec::new();
    collect_rollouts(&root, &mut found);
    let names: Vec<String> = found
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["rollout-abc.jsonl".to_string()]);

    // A missing directory is a no-op (no panic, leaves the vec unchanged).
    let mut none = Vec::new();
    collect_rollouts(&root.join("does-not-exist"), &mut none);
    assert!(none.is_empty());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn parse_reads_a_real_rollout_file() {
    let dir = std::env::temp_dir().join(format!("codex_import_parse_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rollout-1.jsonl");
    std::fs::write(
            &path,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"hi"}]}}"#,
        )
        .unwrap();
    let msgs = parse(&path).expect("parse a real file");
    assert_eq!(msgs.len(), 1);
    assert_eq!(&*msgs[0].content, "hi");
    // A missing file is an Err, not a panic.
    assert!(parse(&dir.join("nope.jsonl")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
