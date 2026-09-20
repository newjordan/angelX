use super::*;
use std::time::{Duration, Instant};

fn wait(layouts: &mut Layouts, messages: &[Message]) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while layouts.busy() {
        layouts.poll(messages);
        assert!(Instant::now() < deadline, "layout worker did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(layouts.failure().is_none(), "{:?}", layouts.failure());
}

#[test]
fn worker_height_and_viewport_match_the_production_unicode_renderer() {
    for role in [Role::User, Role::Angel, Role::Council] {
        let messages = [Message::new(
            role,
            format!(
                "{}\nTAIL-IDENTITY",
                "**evidence** 界 👩‍🔬 e\u{301} لا\n".repeat(320)
            ),
        )];
        let mut layouts = Layouts::default();
        for width in [1, 24, 80] {
            assert!(layouts.height(&messages[0], width).is_none());
            wait(&mut layouts, &messages);
            let expected_height = crate::ui::transcript::message_height(&messages[0], width);
            assert_eq!(layouts.height(&messages[0], width), Some(expected_height));
            for top in [0, expected_height.saturating_sub(9)] {
                let style = Style::default();
                assert!(
                    layouts
                        .view(&messages[0], width as u16, top, 9, style)
                        .is_none()
                );
                wait(&mut layouts, &messages);
                let actual = layouts
                    .view(&messages[0], width as u16, top, 9, style)
                    .unwrap();
                let area = Rect::new(0, 0, width as u16, 9);
                let mut expected = Buffer::empty(area);
                let mut renders = Vec::new();
                let lines = crate::ui::transcript::lines(&messages, "", false, width, &mut renders);
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .style(style)
                    .scroll((top, 0))
                    .render(area, &mut expected);
                assert_eq!(*actual, expected, "width={width} top={top}");
            }
        }
    }
}

#[test]
fn invalidated_and_replaced_messages_cannot_publish_worker_results() {
    let old = Message::new(Role::User, "old identity 界 ".repeat(1_000));
    let mut layouts = Layouts::default();
    assert!(layouts.height(&old, 24).is_none());
    layouts.invalidate();
    let messages = [Message::new(Role::User, "new identity 界 ".repeat(1_000))];
    assert!(layouts.height(&messages[0], 24).is_none());
    wait(&mut layouts, &messages);
    assert!(layouts.entries.is_empty(), "old generation was published");
    assert!(layouts.height(&messages[0], 24).is_none());
    wait(&mut layouts, &messages);
    assert_eq!(layouts.entries.len(), 1);
    assert!(layouts.entries[0].matches(&messages[0], 24));
    let same_text_other_role = Message::new(Role::Angel, Arc::clone(&messages[0].text));
    assert!(!layouts.entries[0].matches(&same_text_other_role, 24));
    assert!(!layouts.entries[0].matches(&messages[0], 40));
}

#[test]
fn viewport_request_does_not_reuse_a_different_scroll_position() {
    let messages = [Message::new(Role::User, "row identity 界\n".repeat(1_000))];
    let mut layouts = Layouts::default();
    assert!(
        layouts
            .view(&messages[0], 24, 0, 8, Style::default())
            .is_none()
    );
    wait(&mut layouts, &messages);
    assert!(
        layouts
            .view(&messages[0], 24, 0, 8, Style::default())
            .is_some()
    );
    assert!(
        layouts
            .view(&messages[0], 24, 3, 8, Style::default())
            .is_none()
    );
    assert!(layouts.busy());
    wait(&mut layouts, &messages);
    assert!(
        layouts
            .view(&messages[0], 24, 3, 8, Style::default())
            .is_some()
    );
}

#[test]
fn streaming_snapshot_height_and_reader_view_are_exact_and_replacements_reject_old_results() {
    let mut text = "streaming 界 👩‍🔬 e\u{301} ".repeat(2_000);
    let mut layouts = Layouts::default();
    assert_eq!(layouts.partial_height(&text, 24, true), 0);
    let retained = Arc::clone(layouts.partial.as_ref().unwrap());
    layouts.partial_height(&text, 24, true);
    assert!(
        Arc::ptr_eq(&retained, layouts.partial.as_ref().unwrap()),
        "unchanged text copied again"
    );
    wait(&mut layouts, &[]);
    let expected_height = crate::ui::transcript::plain_partial_height(&text, 24);
    assert_eq!(layouts.partial_height(&text, 24, true), expected_height);
    assert!(layouts.partial_view(24, 17, 9, Style::default()).is_none());
    wait(&mut layouts, &[]);
    let actual = layouts.partial_view(24, 17, 9, Style::default()).unwrap();
    let area = Rect::new(0, 0, 24, 9);
    let mut expected = Buffer::empty(area);
    Paragraph::new(crate::ui::transcript::plain_partial_lines(&text))
        .wrap(Wrap { trim: false })
        .scroll((17, 0))
        .render(area, &mut expected);
    assert_eq!(*actual, expected);
    text.push_str("APPENDED");
    layouts.partial_height(&text, 24, true);
    let replacement = "replacement 界 ".repeat(3_000);
    layouts.partial_height(&replacement, 24, true);
    wait(&mut layouts, &[]);
    assert!(
        !layouts.entries.iter().any(|entry| entry.partial),
        "replaced stream accepted an old prefix"
    );
    layouts.partial_height(&replacement, 24, true);
    wait(&mut layouts, &[]);
    assert_eq!(
        layouts.partial_height(&replacement, 24, true),
        crate::ui::transcript::plain_partial_height(&replacement, 24)
    );
    assert_eq!(
        layouts.entries.iter().filter(|entry| entry.partial).count(),
        1
    );
}

#[test]
fn clipped_frozen_wide_glyph_cannot_overwrite_the_scroll_rail() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut frozen = Buffer::empty(Rect::new(0, 0, 2, 1));
    frozen[(0, 0)].set_symbol("界");
    let mut terminal = Terminal::new(TestBackend::new(2, 1)).unwrap();
    terminal
        .draw(|frame| {
            paint(&frozen, frame.buffer_mut(), Rect::new(0, 0, 1, 1));
            frame.buffer_mut()[(1, 0)].set_symbol("#");
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), " ");
    assert_eq!(
        terminal.backend().buffer()[(1, 0)].symbol(),
        "#",
        "a wide glyph at the clipped edge must not make the terminal diff skip the rail"
    );
}
