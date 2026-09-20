//! Pure exchange-history surgery tests (module-breakup: extracted from the
//! `tests.rs` monolith). The retry/undo rewriters operate on the raw
//! `ChatMsg` history — no `App`, no flight slot — so these pins live here
//! beside nothing but the club types; the command-level tests stay in the
//! parent with the other `seed_preview_app` suites.

use crate::agent::club::{ChatMsg, ChatRole};
use crate::app::control;

#[test]
fn retry_last_request_replaces_exchange_and_preserves_exact_request_prefix() {
    let attachment = crate::agent::club::Media::Image {
        mime: "image/png".to_string(),
        b64: "cGl4ZWw=".to_string(),
    };
    let mut history = vec![
        ChatMsg::system("bootstrap"),
        ChatMsg::user("earlier task"),
        ChatMsg::assistant("earlier answer"),
        ChatMsg::user_with_media("last task", vec![attachment]),
        ChatMsg::harness("<selected_skill>exact playbook</selected_skill>"),
        ChatMsg::harness("<turn_context>exact controls</turn_context>"),
        ChatMsg::assistant("stale answer"),
        ChatMsg::tool("call-1", "stale tool output"),
        ChatMsg::assistant("stale final"),
    ];

    let replayed = control::retry_last_request(&mut history);

    assert_eq!(replayed.as_deref(), Some("last task"));
    assert_eq!(history.len(), 6);
    assert_eq!(history[3].role, ChatRole::User);
    assert_eq!(history[3].attachments.len(), 1);
    assert_eq!(
        &*history[4].content,
        "<selected_skill>exact playbook</selected_skill>"
    );
    assert_eq!(
        &*history[5].content,
        "<turn_context>exact controls</turn_context>"
    );
    assert!(
        history[3..]
            .iter()
            .all(|message| !matches!(message.role, ChatRole::Assistant | ChatRole::Tool)),
        "the superseded assistant/tool exchange must not survive: {history:?}"
    );
}

#[test]
fn retry_without_a_surviving_operator_turn_fails_closed() {
    let mut history = vec![
        ChatMsg::system("bootstrap"),
        ChatMsg::harness("compacted task anchor"),
        ChatMsg::assistant("answer"),
    ];
    let original = history
        .iter()
        .map(|message| (message.role.clone(), message.content.clone()))
        .collect::<Vec<_>>();

    assert!(control::retry_last_request(&mut history).is_none());
    assert_eq!(
        history
            .iter()
            .map(|message| (message.role.clone(), message.content.clone()))
            .collect::<Vec<_>>(),
        original,
        "retry must not guess operator intent from Harness-role text"
    );
}

#[test]
fn undo_last_exchange_removes_user_harness_and_model_suffix_only() {
    let mut history = vec![
        ChatMsg::system("bootstrap"),
        ChatMsg::user("earlier task"),
        ChatMsg::assistant("earlier answer"),
        ChatMsg::user("mistaken task"),
        ChatMsg::harness("<selected_skill>mistaken playbook</selected_skill>"),
        ChatMsg::assistant("mistaken answer"),
        ChatMsg::tool("call-1", "mistaken tool output"),
    ];

    let removed = control::undo_last_exchange(&mut history).unwrap();

    assert_eq!(removed.len(), 4);
    assert_eq!(&*removed[0].content, "mistaken task");
    assert_eq!(&*removed[3].content, "mistaken tool output");
    assert_eq!(history.len(), 3);
    assert_eq!(&*history[1].content, "earlier task");
    assert_eq!(&*history[2].content, "earlier answer");
}

#[test]
fn undo_without_a_surviving_operator_turn_fails_closed() {
    let mut history = vec![
        ChatMsg::system("bootstrap"),
        ChatMsg::harness("compacted task anchor"),
        ChatMsg::assistant("answer"),
    ];
    let before = history.len();

    assert!(control::undo_last_exchange(&mut history).is_none());
    assert_eq!(history.len(), before);
}
