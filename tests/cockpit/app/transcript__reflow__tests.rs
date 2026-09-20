use super::*;
#[test]
fn budgets_preserve_progress_and_appended_tail() {
    let mut messages: Vec<_> = (0..7)
        .map(|_| Message::new(super::super::Role::User, "row"))
        .collect();
    let mut work = PendingReflow::new(40);
    work.advance(&messages, 0, || false, |_, _| panic!("zero count"));
    work.advance(&messages, 4, || true, |_, _| panic!("zero time"));
    assert!(work.heights.is_empty());
    work.advance(
        &messages,
        3,
        || false,
        |_, width| {
            assert_eq!(width, 40);
            Some(2)
        },
    );
    assert_eq!(work.prefix, [0, 2, 4, 6]);
    messages.push(Message::new(super::super::Role::User, "tail"));
    let mut calls = 0;
    work.advance(
        &messages,
        10,
        || {
            calls += 1;
            calls > 2
        },
        |_, _| Some(3),
    );
    assert_eq!(work.prefix, [0, 2, 4, 6, 9, 12]);
    assert!(!work.ready(messages.len()));
    work.advance(&messages, 10, || false, |_, _| Some(4));
    assert!(work.ready(messages.len()));
    assert_eq!(work.prefix, [0, 2, 4, 6, 9, 12, 16, 20, 24]);
}
