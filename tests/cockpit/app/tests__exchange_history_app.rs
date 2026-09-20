//! App-level retry/undo/redo command tests (module-breakup: completing the
//! exchange-history family — the fixture-free rewriter pins live above; these
//! drive the commands through a preview `App` and the flight-slot contract).

use super::seed_preview_app;
use crate::App;
use crate::app_control;
use crate::club::{ChatMsg, ChatRole};

#[test]
fn retry_command_rebuilds_transcript_and_starts_a_replacement_turn() {
    let mut app = seed_preview_app();
    app.history.push(ChatMsg::user("repair the parser"));
    app.history
        .push(ChatMsg::harness("<selected_skill>debug</selected_skill>"));
    app.history.push(ChatMsg::assistant("stale failed answer"));
    app.rebuild_display();

    app.input = "/retry".to_string();
    app.submit();

    assert!(
        app.thinking.is_some(),
        "retry must start a replacement turn"
    );
    assert!(
        app.history
            .iter()
            .all(|message| message.content.as_ref() != "stale failed answer"),
        "stale answer remained in model history"
    );
    assert!(
        app.messages
            .iter()
            .all(|message| message.text.as_ref() != "stale failed answer"),
        "stale answer remained in the visible transcript"
    );
    assert!(
        app.messages.iter().any(|message| message
            .text
            .contains("workspace changes were not rolled back")),
        "operator receives a truthful conversation-only replacement receipt"
    );
    assert!(
        app.history
            .iter()
            .any(|message| message.content.contains("<selected_skill>debug")),
        "request-side Harness evidence must be replayed"
    );
    app.interrupt();
}

#[test]
fn retry_waits_for_the_flight_slot_and_keeps_the_command_draft() {
    let mut app = seed_preview_app();
    app.history.push(ChatMsg::user("repair the parser"));
    app.history.push(ChatMsg::assistant("prior answer"));
    let before = app.history.len();
    let (_tx, job) = app_control::BackgroundJob::channel("test background job", "Retry the test");
    app.bg_job = Some(job);
    app.input = "/retry".to_string();

    app.submit();

    assert_eq!(app.input, "/retry");
    assert_eq!(app.history.len(), before);
    assert!(app.messages.last().unwrap().text.contains("draft kept"));
    app.interrupt();
}

#[test]
fn undo_command_rebuilds_transcript_clears_turn_canvas_and_stays_local() {
    let mut app = seed_preview_app();
    app.history.push(ChatMsg::user("earlier task"));
    app.history.push(ChatMsg::assistant("earlier answer"));
    app.history.push(ChatMsg::user("mistaken task"));
    app.history.push(ChatMsg::harness(
        "<selected_skill>mistaken</selected_skill>",
    ));
    app.history.push(ChatMsg::assistant("mistaken answer"));
    app.reasoning = "stale private reasoning".to_string();
    app.rebuild_display();
    app.input = "/undo".to_string();

    app.submit();

    assert!(app.thinking.is_none(), "undo is local history surgery");
    assert!(app.reasoning.is_empty(), "removed turn canvas must clear");
    assert_eq!(
        app.history
            .iter()
            .filter(|message| message.role == ChatRole::User)
            .count(),
        1
    );
    assert!(
        app.history
            .iter()
            .all(|message| !message.content.contains("mistaken")),
        "removed exchange remained in model history: {:?}",
        app.history
    );
    assert!(
        app.messages
            .iter()
            .all(|message| !message.text.contains("mistaken answer")),
        "removed answer remained in visible transcript"
    );
    assert!(
        app.messages
            .last()
            .unwrap()
            .text
            .contains("side effects were not rolled back"),
        "receipt must distinguish conversation undo from rollback"
    );
}

#[test]
fn undo_waits_for_the_flight_slot_and_keeps_the_command_draft() {
    let mut app = seed_preview_app();
    app.history.push(ChatMsg::user("mistaken task"));
    app.history.push(ChatMsg::assistant("mistaken answer"));
    let before = app.history.len();
    let (_tx, job) = app_control::BackgroundJob::channel("test background job", "Retry the test");
    app.bg_job = Some(job);
    app.input = "/undo".to_string();

    app.submit();

    assert_eq!(app.input, "/undo");
    assert_eq!(app.history.len(), before);
    assert!(app.messages.last().unwrap().text.contains("draft kept"));
    app.interrupt();
}

#[test]
fn redo_restores_the_exact_undone_exchange_without_starting_a_turn() {
    let mut app = seed_preview_app();
    app.history.push(ChatMsg::user("earlier task"));
    app.history.push(ChatMsg::assistant("earlier answer"));
    app.history.push(ChatMsg::user("mistaken task"));
    app.history.push(ChatMsg::harness(
        "<selected_skill>mistaken</selected_skill>",
    ));
    app.history.push(ChatMsg::assistant("mistaken answer"));
    app.history
        .push(ChatMsg::tool("call-1", "mistaken tool output"));
    let before = app
        .history
        .iter()
        .map(|message| (message.role.clone(), message.content.clone()))
        .collect::<Vec<_>>();
    app.rebuild_display();

    app.input = "/undo".to_string();
    app.submit();
    app.input = "/redo".to_string();
    app.submit();

    assert!(app.thinking.is_none(), "redo is local history restoration");
    assert_eq!(
        app.history
            .iter()
            .map(|message| (message.role.clone(), message.content.clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.as_ref() == "mistaken answer"),
        "restored answer must return to the visible transcript"
    );
    assert!(
        app.messages
            .last()
            .unwrap()
            .text
            .contains("were never rolled back or replayed"),
        "receipt must preserve the conversation-only boundary"
    );
    assert!(
        app.undone_exchange.is_none(),
        "one redo consumes the restorable exchange"
    );
}

#[test]
fn redo_fails_closed_after_the_conversation_changes() {
    let mut app = seed_preview_app();
    app.history.push(ChatMsg::user("mistaken task"));
    app.history.push(ChatMsg::assistant("mistaken answer"));
    app.input = "/undo".to_string();
    app.submit();
    app.history.push(ChatMsg::user("replacement task"));
    app.history.push(ChatMsg::assistant("replacement answer"));
    let before = app.history.len();

    app.input = "/redo".to_string();
    app.submit();

    assert_eq!(app.history.len(), before);
    assert!(
        app.history
            .iter()
            .all(|message| message.content.as_ref() != "mistaken answer")
    );
    assert!(
        app.messages
            .last()
            .unwrap()
            .text
            .contains("conversation changed after /undo")
    );
    assert!(app.undone_exchange.is_none());
}

#[test]
fn redo_waits_for_the_flight_slot_and_keeps_the_command_draft() {
    let mut app = seed_preview_app();
    app.history.push(ChatMsg::user("mistaken task"));
    app.history.push(ChatMsg::assistant("mistaken answer"));
    app.input = "/undo".to_string();
    app.submit();
    let (_tx, job) = app_control::BackgroundJob::channel("test background job", "Retry the test");
    app.bg_job = Some(job);
    app.input = "/redo".to_string();

    app.submit();

    assert_eq!(app.input, "/redo");
    assert!(app.undone_exchange.is_some());
    assert!(app.messages.last().unwrap().text.contains("draft kept"));
    app.interrupt();
}

#[test]
fn redo_payload_is_dropped_by_a_new_turn_or_new_chat() {
    let seed_undo = |app: &mut App| {
        app.history.push(ChatMsg::user("mistaken task"));
        app.history.push(ChatMsg::assistant("mistaken answer"));
        app.input = "/undo".to_string();
        app.submit();
        assert!(app.undone_exchange.is_some());
    };

    let mut turn_app = seed_preview_app();
    seed_undo(&mut turn_app);
    turn_app.input = "replacement branch".to_string();
    turn_app.submit();
    assert!(turn_app.thinking.is_some());
    assert!(
        turn_app.undone_exchange.is_none(),
        "a new model turn selects a different branch"
    );
    turn_app.interrupt();

    let mut new_app = seed_preview_app();
    seed_undo(&mut new_app);
    new_app.input = "/new".to_string();
    new_app.submit();
    assert!(
        new_app.undone_exchange.is_none(),
        "a fresh session cannot inherit a prior redo payload"
    );
}
