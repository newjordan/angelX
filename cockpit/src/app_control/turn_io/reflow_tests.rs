use super::*;

#[test]
fn reflow_same_published_height_rewrite_cancels_pending_before_early_return() {
    let mut app = App::preview(crate::Viewer::static_preview());
    app.messages = vec![Message::new(Role::User, "short")];
    let width = 80;
    let height = crate::transcript::message_height(&app.messages[0], width);
    app.transcript_heights_w = width as u16;
    app.transcript_heights = vec![height];
    app.transcript_height_prefix = vec![0, u32::from(height)];
    let mut work = crate::transcript::reflow::PendingReflow::new(10);
    work.advance(
        &app.messages,
        1,
        || false,
        |message, width| Some(crate::transcript::message_height(message, width)),
    );
    let old_target_height = work.heights[0];
    app.pending_transcript_reflow = Some(work);
    app.messages[0] = Message::new(Role::User, "a longer replacement which still fits");
    assert_eq!(
        crate::transcript::message_height(&app.messages[0], width),
        height
    );
    assert_ne!(
        crate::transcript::message_height(&app.messages[0], 10),
        old_target_height
    );
    app.refresh_transcript_row_height(0);
    assert!(app.pending_transcript_reflow.is_none());
    assert_eq!(app.transcript_heights, [height]);
}

#[test]
fn reflow_scrollback_drain_cancels_pending_even_after_length_is_restored() {
    let mut app = App::preview(crate::Viewer::static_preview());
    app.messages = (0..4001)
        .map(|i| Message::new(Role::User, format!("row {i}")))
        .collect();
    let mut work = crate::transcript::reflow::PendingReflow::new(10);
    work.advance(
        &app.messages,
        2,
        || false,
        |message, width| Some(crate::transcript::message_height(message, width)),
    );
    app.pending_transcript_reflow = Some(work);
    app.cap_scrollback();
    assert_eq!(app.messages.len(), 3000);
    assert!(app.pending_transcript_reflow.is_none());
    assert_eq!(app.messages[0].text.as_ref(), "row 1001");
    app.messages
        .extend((0..1001).map(|_| Message::new(Role::User, "tail")));
    assert_eq!(app.messages.len(), 4001);
    assert!(app.pending_transcript_reflow.is_none());
}
