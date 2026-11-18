//! The book's introduction: each stamp is introduced once, in English, where
//! the model first sees it; after that it rides alone.
use super::{LEAD, apply, introductions, unslotted};
use crate::agent::club::{ChatMsg, ToolCall};

fn call(id: &str, name: &str, args: serde_json::Value) -> ChatMsg {
    ChatMsg::assistant_calls_with_reasoning(
        vec![ToolCall {
            id: id.into(),
            name: name.into(),
            args,
        }],
        None,
    )
}

fn tool(id: &str, content: &str) -> ChatMsg {
    ChatMsg::tool(id, content.to_string())
}

fn turn() -> Vec<ChatMsg> {
    vec![
        ChatMsg::system("⠽⠙\n⠍⠓\n`/work/demo`".to_string()),
        ChatMsg::user("Fix the failing test.".to_string()),
        call("a", "run_tests", serde_json::json!({})),
        tool("a", "tests: 0 passed, 1 failed\n⠧⠉"),
        call("b", "run_tests", serde_json::json!({})),
        tool("b", "tests: 0 passed, 1 failed\n⠧⠉"),
    ]
}

#[test]
fn untested_introduction_uses_a_tool_loaded_in_bounded_task_mode() {
    let messages = vec![tool("edit", "changed\n⠧⠋")];
    let intros = introductions(&messages, None);
    let text = &intros[0].1;
    assert!(crate::agent::harness::is_essential_tool("shell"));
    assert!(
        text.contains("use `shell` to run the smallest relevant repository"),
        "{text}"
    );
    assert!(!text.contains("`run_tests`"), "{text}");
}

#[test]
fn a_stamp_is_introduced_at_first_sight_then_rides_alone() {
    let intros = introductions(&turn(), None);
    let indices: Vec<usize> = intros.iter().map(|(index, _)| *index).collect();
    assert_eq!(indices, vec![0, 3], "{intros:?}");
    let red = &intros[1].1;
    assert!(
        red.starts_with("⠧⠉ the project check after this edit failed"),
        "{red}"
    );
    assert!(
        red.contains("→ fix the first diagnostic before the next edit"),
        "{red}"
    );
    assert!(intros[0].1.starts_with(LEAD), "{}", intros[0].1);
    assert!(!red.contains(LEAD), "{red}");
}

#[test]
fn the_core_type_is_introduced_as_its_pages_and_covers_its_routes() {
    let intros = introductions(&turn(), None);
    let core = &intros[0].1;
    assert!(core.contains("\n⠽⠙\n"), "{core}");
    assert!(
        core.contains("You are Angel — the Driver for this workspace."),
        "{core}"
    );
    assert!(core.contains("\n⠍⠓ the task workspace map"), "{core}");
    let mut messages = turn();
    messages.push(tool("c", "done\n⠺⠃"));
    let later = introductions(&messages, None);
    assert_eq!(later.len(), 2, "{later:?}");
}

#[test]
fn a_page_is_introduced_as_its_sentence() {
    let messages = vec![
        ChatMsg::system("⠽⠙".to_string()),
        ChatMsg::harness("⠗⠃\n⠗⠃⠃ \"ship it\"\n⠗⠃⠙ \"cargo test\"\n⠗⠃".to_string()),
    ];
    let intros = introductions(&messages, None);
    let goal = &intros[1].1;
    assert!(goal.contains("⠗⠃⠃ objective: …"), "{goal}");
    assert!(
        goal.contains("⠗⠃⠙ verifiable check (must pass): …"),
        "{goal}"
    );
    assert!(!goal.contains('{'), "{goal}");
}

#[test]
fn an_introduction_never_changes_on_a_later_request() {
    let mut messages = turn();
    let first = introductions(&messages, None);
    messages.push(call("c", "read_file", serde_json::json!({"path": "x"})));
    messages.push(tool("c", "contents\n⠺⠓"));
    let later = introductions(&messages, None);
    assert_eq!(&later[..first.len()], &first[..]);
    assert_eq!(later.len(), first.len() + 1, "{later:?}");
}

#[test]
fn a_stamp_the_model_decoded_needs_no_introduction() {
    let messages = vec![
        ChatMsg::system("⠽⠙".to_string()),
        ChatMsg::user("task".to_string()),
        call("r", "read_file", serde_json::json!({"path": "ledger://⠧⠉"})),
        tool("r", "⠧⠉  the project check failed\n⠧⠉⠁ …"),
        call("a", "run_tests", serde_json::json!({})),
        tool("a", "tests: 0 passed, 1 failed\n⠧⠉"),
    ];
    let intros = introductions(&messages, None);
    assert_eq!(intros.len(), 1, "only the opening: {intros:?}");
}

#[test]
fn a_failed_ledger_read_never_counts_as_learning_the_legend() {
    let messages = vec![
        call("r", "read_file", serde_json::json!({"path":"ledger://⡫⠃"})),
        tool("r", "error: interrupted before reading"),
        ChatMsg::harness("⚠⡫⠃"),
    ];
    let intros = introductions(&messages, None);
    assert_eq!(intros.last().unwrap().0, 2);
    assert!(intros.last().unwrap().1.contains("intentional replication"));
}

#[test]
fn reading_one_page_does_not_teach_the_whole_route() {
    let messages = vec![
        call("r", "read_file", serde_json::json!({"path":"ledger://⠵⠋⠁"})),
        tool("r", "⠵⠋⠁  Reason the same way."),
        ChatMsg::harness("⠵⠋⠉"),
    ];
    let intros = introductions(&messages, None);
    assert_eq!(intros.last().unwrap().0, 2);
    assert!(intros.last().unwrap().1.contains("Keep code blocks"));
}

#[test]
fn a_stamp_whose_reading_was_elided_is_introduced_again() {
    let aged = format!(
        "{}: read_file|ledger://⠧⠉ (812 bytes){}",
        crate::agent::harness::compact::TOOL_AGED_MARK,
        crate::agent::harness::compact::AGED_TAIL
    );
    let messages = vec![
        ChatMsg::system("⠽⠙".to_string()),
        ChatMsg::user("task".to_string()),
        call("r", "read_file", serde_json::json!({"path": "ledger://⠧⠉"})),
        tool("r", &aged),
        call("a", "run_tests", serde_json::json!({})),
        tool("a", "tests: 0 passed, 1 failed\n⠧⠉"),
    ];
    let intros = introductions(&messages, None);
    assert_eq!(
        intros.last().map(|(index, _)| *index),
        Some(5),
        "{intros:?}"
    );
    assert!(intros.last().unwrap().1.contains("⠧⠉ "), "{intros:?}");
}

#[test]
fn braille_quoted_mid_line_is_text_not_a_stamp() {
    let messages = vec![tool(
        "a",
        "   12 | //! | ⠧ v | `v_verification.rs` | what the tests say ⠧⠉ |",
    )];
    assert!(introductions(&messages, None).is_empty());
    let marks = vec![
        tool("b", "[⡨⠃⠚ aged=2 dropped=0 saved=100]"),
        ChatMsg::harness(r#"{"skills_catalog":"\n\n⠥⠃\n16\n"}"#.to_string()),
    ];
    let intros = introductions(&marks, None);
    assert_eq!(intros.len(), 2, "{intros:?}");
    assert!(
        intros[0].1.contains("⡨⠃⠚ [tool-aging boundary"),
        "{intros:?}"
    );
    assert!(intros[1].1.contains("⠥⠃ "), "{intros:?}");
}

#[test]
fn a_compacted_first_sight_is_introduced_again() {
    let messages = turn();
    let compacted = vec![
        messages[0].clone(),
        messages[1].clone(),
        messages[4].clone(),
        messages[5].clone(),
    ];
    let intros = introductions(&compacted, None);
    assert_eq!(
        intros.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        vec![0, 3]
    );
}

#[test]
fn standing_braille_is_introduced_at_its_own_index() {
    let messages = vec![
        ChatMsg::system("plain".to_string()),
        ChatMsg::user("task".to_string()),
    ];
    let intros = introductions(&messages, Some((1, "⠵⠁")));
    assert_eq!(intros.len(), 1, "{intros:?}");
    assert_eq!(intros[0].0, 1);
    assert!(intros[0].1.contains("⠵⠁ "), "{intros:?}");
}

#[test]
fn apply_appends_to_text_and_to_a_multimodal_texts_first_part() {
    let mut outbound = vec![
        serde_json::json!({"role": "system", "content": "⠧⠉"}),
        serde_json::json!({"role": "user", "content": [
            {"type": "text", "text": "look"},
            {"type": "image_url", "image_url": {"url": "data:"}},
        ]}),
    ];
    apply(
        &mut outbound,
        vec![(0, "intro zero".into()), (1, "intro one".into())],
    );
    assert_eq!(outbound[0]["content"], "⠧⠉\n\nintro zero");
    assert_eq!(outbound[1]["content"][0]["text"], "look\n\nintro one");
    assert_eq!(outbound[1]["content"][1]["type"], "image_url");
}

#[test]
fn the_http_wire_carries_the_introduction_only_where_the_model_can_read() {
    use crate::agent::club::{HttpClub, ToolDef};
    let _guard = crate::tests::env_lock();
    let _caveman = crate::tests::TestEnvGuard::set("ANGEL_SOTA_CAVEMAN", "0");
    let club = HttpClub::new(
        "deepseek",
        "https://api.deepseek.com/v1",
        "deepseek-chat",
        None,
    );
    let read_file = ToolDef {
        name: "read_file".into(),
        description: "read".into(),
        params: serde_json::json!({"type": "object"}),
    };
    let messages = turn();
    let text = |body: &serde_json::Value| body["messages"].to_string();
    let body = club
        .build_body(&messages, std::slice::from_ref(&read_file), false)
        .unwrap();
    assert!(text(&body).contains("fix the first diagnostic before the next edit"));
    assert!(!messages[3].content.contains("diagnostic"));
    let bare = club.build_body(&messages, &[], false).unwrap();
    assert!(!text(&bare).contains("fix the first diagnostic before the next edit"));
    let _off = crate::tests::TestEnvGuard::set("ANGEL_BOOK_INTRO", "0");
    let off = club.build_body(&messages, &[read_file], false).unwrap();
    assert!(!text(&off).contains("fix the first diagnostic before the next edit"));
}

#[test]
fn the_legend_copy_is_introduced_and_leaves_history_alone() {
    let _guard = crate::tests::env_lock();
    let read_file = crate::agent::club::ToolDef {
        name: "read_file".into(),
        description: "read".into(),
        params: serde_json::json!({"type": "object"}),
    };
    let history = turn();
    {
        let _unset = crate::tests::TestEnvGuard::unset("ANGEL_BOOK_INTRO");
        let copy = super::introduced(&history, std::slice::from_ref(&read_file));
        assert!(matches!(copy, std::borrow::Cow::Owned(_)), "on by default");
        assert!(
            copy[3]
                .content
                .contains("fix the first diagnostic before the next edit")
        );
        assert!(
            !copy[5]
                .content
                .contains("fix the first diagnostic before the next edit"),
            "once"
        );
        assert!(
            !history[3].content.contains("diagnostic"),
            "history untouched"
        );
        assert!(matches!(
            super::introduced(&history, &[]),
            std::borrow::Cow::Borrowed(_)
        ));
    }
    let _off = crate::tests::TestEnvGuard::set("ANGEL_BOOK_INTRO", "0");
    assert!(matches!(
        super::introduced(&history, &[read_file]),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn the_every_sighting_arm_repeats_the_english() {
    let intros = super::introductions_at(&turn(), None, true, "");
    let indices: Vec<usize> = intros.iter().map(|(index, _)| *index).collect();
    assert_eq!(indices, vec![0, 3, 5], "{intros:?}");
    assert!(
        intros[2]
            .1
            .starts_with("⠧⠉ the project check after this edit failed"),
        "{intros:?}"
    );
    assert_eq!(
        intros
            .iter()
            .filter(|(_, text)| text.contains(LEAD))
            .count(),
        1
    );
}

#[test]
fn a_loop_turn_after_the_warning_sign_is_introduced_once() {
    let messages = vec![
        ChatMsg::user("go".to_string()),
        ChatMsg::harness("⛔⠇⠁".to_string()),
        ChatMsg::harness("⛔⠇⠁".to_string()),
    ];
    let intros = introductions(&messages, None);
    assert_eq!(intros.len(), 1, "{intros:?}");
    assert_eq!(intros[0].0, 1);
    assert!(
        intros[0]
            .1
            .contains("⠇⠁ You've repeated the same tool call several times with no new result"),
        "{intros:?}"
    );
}

#[test]
fn a_value_slot_reads_as_an_ellipsis_and_a_doubled_brace_is_kept() {
    assert_eq!(
        unslotted("only {n_remaining} tokens remain"),
        "only … tokens remain"
    );
    assert_eq!(unslotted("at `{}` now"), "at `…` now");
    let ported = "the format `[$app-name](app://{{connector_id}})` and `{\"authority\":{\"kind\":\"orchestrator\"}}`";
    assert_eq!(unslotted(ported), ported);
    assert_eq!(unslotted("{{a}} then {b}"), "{{a}} then …");
    assert_eq!(
        unslotted("an open {{ never closes"),
        "an open {{ never closes"
    );
}

#[test]
fn advertised_tool_schema_stamps_are_introduced_once() {
    use crate::agent::club::ToolDef;
    let tools = [ToolDef {
        name: "read_file".into(),
        description: "List a directory. ⠣⠃".into(),
        params: serde_json::json!({
            "type": "object",
            "properties": {
                "hint": { "type": "string", "description": "⠣⠃⠁" },
            },
        }),
    }];
    let messages = vec![
        ChatMsg::system("ready".to_string()),
        ChatMsg::user("list it".to_string()),
        ChatMsg::harness("hint ⠣⠃⠁".to_string()),
    ];
    let out = super::introduced(&messages, &tools);
    assert!(
        out[0].content.contains("filename fragment to rank first"),
        "{}",
        out[0].content
    );
    assert!(
        out[0].content.contains("⠣⠃⠁ filename fragment"),
        "{}",
        out[0].content
    );
    assert_eq!(&*out[2].content, "hint ⠣⠃⠁");
}
