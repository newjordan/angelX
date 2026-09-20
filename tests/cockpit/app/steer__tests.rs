use super::*;
use crate::club::ChatRole;

#[test]
fn queue_is_fifo_and_drains_empty() {
    let q = SteerQueue::default();
    assert!(q.is_empty());
    q.push(ChatMsg::user("first"));
    q.push(ChatMsg::user("second"));
    assert_eq!(q.len(), 2);
    let drained = q.drain();
    assert_eq!(drained.len(), 2);
    assert_eq!(&*drained[0].content, "first");
    assert_eq!(&*drained[1].content, "second");
    assert!(q.is_empty());
    assert!(q.drain().is_empty());
}

#[test]
fn context_message_is_harness_role_and_does_not_rewrite_the_note() {
    let context = context_message();
    let note = ChatMsg::user("check the tests too");
    assert_eq!(context.role, ChatRole::Harness);
    assert_eq!(&*context.content, STEER_CONTEXT);
    assert_eq!(note.role, ChatRole::User);
    assert_eq!(&*note.content, "check the tests too");
}

#[test]
fn snippet_caps_to_the_first_line() {
    assert_eq!(snippet("short"), "short");
    assert_eq!(snippet("line one\nline two"), "line one");
    let long = "x".repeat(80);
    let s = snippet(&long);
    assert!(s.chars().count() <= 61);
    assert!(s.ends_with('…'));
}
