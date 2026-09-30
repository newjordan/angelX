//! The book's introduction: each stamp is introduced once, in English, where
//! the model first sees it; after that it rides alone.
use super::{LEAD, apply, introductions};
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
fn a_stamp_is_introduced_at_first_sight_then_rides_alone() {
    let intros = introductions(&turn(), None);
    let indices: Vec<usize> = intros.iter().map(|(index, _)| *index).collect();
    // The opening system message and the first red run; never the second.
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
    // The standing direction is said once, with the first introduction.
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
    // The workspace map is its signal and its short pages.
    assert!(core.contains("\n⠍⠓ the task workspace map"), "{core}");
    // A route the core already spelled out rides alone when it comes back.
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
    // The values ride beside the stamps; a slot reads as `…`, never as
    // template syntax.
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
fn braille_quoted_mid_line_is_text_not_a_stamp() {
    let messages = vec![tool(
        "a",
        "   12 | //! | ⠧ v | `v_verification.rs` | what the tests say ⠧⠉ |",
    )];
    assert!(introductions(&messages, None).is_empty());
    // A mark opens a bracket; a line inside a JSON string opens after `\n`.
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
    // Compaction dropped the first red run: the second one is introduced.
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
        .build_body(&messages, &[read_file.clone()], false)
        .unwrap();
    assert!(text(&body).contains("fix the first diagnostic before the next edit"));
    // The history itself is untouched.
    assert!(!messages[3].content.contains("diagnostic"));
    // No reader, no introduction; and the switch turns it off.
    let bare = club.build_body(&messages, &[], false).unwrap();
    assert!(!text(&bare).contains("fix the first diagnostic before the next edit"));
    let _off = crate::tests::TestEnvGuard::set("ANGEL_BOOK_INTRO", "0");
    let off = club.build_body(&messages, &[read_file], false).unwrap();
    assert!(!text(&off).contains("fix the first diagnostic before the next edit"));
}

/// The transport-neutral legend: an outbound copy with each stamp's English at
/// first sight, the stored history untouched; nothing without the ledger
/// reader or with the switch off. On by default.
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
        let copy = super::introduced(&history, &[read_file.clone()]);
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

/// The bench arm `ANGEL_BOOK_INTRO=every` introduces a stamp at every
/// sighting; the lead is still said once.
#[test]
fn the_every_sighting_arm_repeats_the_english() {
    let intros = super::introductions_at(&turn(), None, true);
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

/// A loop route rides its own turn after the warning sign; the stamp after the
/// sign still gets its English at first sight, and rides alone after that.
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
