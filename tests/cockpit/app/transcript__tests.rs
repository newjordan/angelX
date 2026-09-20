use super::*;

fn plain_text(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn user_and_angel_lines_get_role_tags() {
    let messages = [
        Message {
            role: Role::User,
            text: "hello".into(),
        },
        Message {
            role: Role::Angel,
            text: "world".into(),
        },
    ];
    let mut renders = Vec::new();
    let rendered = lines(&messages, "", false, 80, &mut renders);
    let text = plain_text(&rendered);
    assert!(text.contains("you "));
    assert!(text.contains("hello"));
    assert!(text.contains("angel "));
    assert!(text.contains("world"));
}

#[test]
fn partial_response_renders_with_cursor_while_thinking() {
    let mut renders = Vec::new();
    let rendered = lines(&[], "streaming", true, 80, &mut renders);
    assert_eq!(rendered.capacity(), 1);
    let text = plain_text(&rendered);
    assert!(text.contains("angel "));
    assert!(text.contains("streaming"));
    assert!(text.contains("▌"));
    assert!(matches!(
        rendered[0]
            .spans
            .iter()
            .find(|span| span.content == "streaming")
            .unwrap()
            .content,
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn markdown_partial_response_renders_with_cursor() {
    let mut renders = Vec::new();
    let rendered = lines(&[], "**streaming**", true, 80, &mut renders);
    let text = plain_text(&rendered);
    assert!(text.contains("angel "));
    assert!(text.contains("streaming"));
    assert!(text.contains("▌"));
}

#[test]
fn oversized_markdown_partial_defers_formatting_until_commit() {
    let partial = format!("# streamed\n\n{}", "**live markdown**\n".repeat(4_000));
    assert!(partial.len() > MAX_LIVE_MARKDOWN_BYTES);
    assert!(
        partial_render(&partial, 80).is_none(),
        "a growing oversized partial must not repeatedly parse Markdown"
    );

    let mut renders = Vec::new();
    let rendered = lines(&[], &partial, true, 80, &mut renders);
    let text = plain_text(&rendered);
    assert!(text.contains("# streamed"));
    assert!(text.contains("**live markdown**"));
    assert!(text.contains('▌'));

    let committed = Message {
        role: Role::Angel,
        text: partial.into(),
    };
    assert!(
        message_render(&committed, 80).is_some(),
        "the settled answer must receive normal cached Markdown rendering"
    );
}

#[test]
fn growing_oversized_markdown_keeps_one_hundred_height_frames_interactive() {
    let chunk = format!(
        "## streamed section\n\n```rust\n{}\n```\n\n",
        "let value = **not actually emphasis**;\n".repeat(64)
    );
    let mut partial = String::new();
    let mut cache = PartialHeightCache::default();
    let started = std::time::Instant::now();
    for _ in 0..100 {
        partial.push_str(&chunk);
        std::hint::black_box(partial_height_cached(&mut cache, &partial, true, 96));
    }
    let elapsed = started.elapsed();
    assert!(partial.len() > 200_000, "fixture must cross the live cap");
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "100 growing Markdown frames took {elapsed:?}"
    );
}

#[test]
fn incremental_plain_height_matches_full_rewrap_across_appends_and_reset() {
    let mut cache = PartialHeightCache::default();
    let mut partial = "# start\n".repeat(5_000);
    for suffix in [
        "unicode 界界\n",
        "trailing newline\n",
        "\n",
        "one very long final line ".repeat(40).as_str(),
    ] {
        partial.push_str(suffix);
        let cached = partial_height_cached(&mut cache, &partial, true, 73);
        let full = plain_segment_height(&partial, true, true, 73).min(u16::MAX as u32) as u16;
        assert_eq!(cached, full, "cache drift after suffix {suffix:?}");
    }

    let replacement = format!("replacement\n{}", "界 mixed width words\n".repeat(4_000));
    let cached = partial_height_cached(&mut cache, &replacement, true, 73);
    let full = plain_segment_height(&replacement, true, true, 73).min(u16::MAX as u32) as u16;
    assert_eq!(cached, full, "non-append replacement must rebuild");

    let resized = partial_height_cached(&mut cache, &replacement, true, 41);
    let resized_full =
        plain_segment_height(&replacement, true, true, 41).min(u16::MAX as u32) as u16;
    assert_eq!(resized, resized_full, "width change must rebuild");
}

#[test]
fn plain_agent_text_uses_borrowed_fast_path() {
    let messages = [Message {
        role: Role::Angel,
        text: "plain response".into(),
    }];
    let mut renders = Vec::new();
    let rendered = lines(&messages, "", false, 80, &mut renders);
    assert_eq!(plain_text(&rendered), "angel \nplain response");
    assert!(matches!(
        rendered[0].spans.last().unwrap().content,
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn plain_message_lines_reserve_expected_capacity() {
    let (tag, style) = message_tag(Role::Angel);
    let single = plain_message_lines("plain response", tag, style, None);
    assert_eq!(single.capacity(), 1);
    let multi = plain_message_lines("one\ntwo\nthree", tag, style, None);
    assert_eq!(multi.capacity(), 3);
    assert_eq!(multi.len(), 3);
}

#[test]
fn system_message_uses_dim_tag_and_text() {
    let messages = [Message {
        role: Role::System,
        text: "compacted earlier".into(),
    }];
    let mut renders = Vec::new();
    let rendered = lines(&messages, "", false, 80, &mut renders);
    let text = plain_text(&rendered);
    assert!(text.contains("· "), "system tag present:\n{text}");
    assert!(text.contains("compacted earlier"));
    // The system body span carries the dim text style.
    let body = rendered[0]
        .spans
        .iter()
        .find(|s| s.content == "compacted earlier")
        .expect("system body span");
    assert_eq!(body.style, SYSTEM_TEXT_STYLE);
}

#[test]
fn activity_lines_are_indented_and_yellow_per_line() {
    let messages = [Message {
        role: Role::Activity,
        text: "T: run tests\nR: 5 passed".into(),
    }];
    let mut renders = Vec::new();
    let rendered = lines(&messages, "", false, 80, &mut renders);
    // One rendered line per source line, each indented and styled as activity.
    assert_eq!(rendered.len(), 2);
    for line in &rendered {
        assert_eq!(line.spans[0].content.as_ref(), "  ", "two-space indent");
        assert_eq!(line.spans[1].style, ACTIVITY_STYLE);
    }
    assert_eq!(rendered[0].spans[1].content.as_ref(), "T: run tests");
    assert_eq!(rendered[1].spans[1].content.as_ref(), "R: 5 passed");
}

#[test]
fn receipt_gauge_testbackend_matches_guard_fill() {
    use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};
    let receipt = crate::turn_event_view::receipt_gauge_text(
        ".. action receipt · shell dispatch error ×29 · last 2 ms · 1–29 ms",
        "action receipt · shell dispatch error · 30 ms",
    )
    .unwrap();
    let guard = crate::turn_event_view::notice_gauge_text("passive wait blocked", 30);
    let mut terminal = Terminal::new(TestBackend::new(140, 2)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(
                Paragraph::new(vec![activity_line(&receipt), activity_line(&guard)]),
                frame.area(),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(2, 0)].bg, buffer[(2, 1)].bg);
    assert_eq!(buffer[(2, 0)].bg, GAUGE_HOT_FILL.bg.unwrap());
    assert_ne!(buffer[(2, 0)].bg, Color::Reset);
    println!(
        "receipt TestBackend: saturated receipt and guard background match {:?}",
        buffer[(2, 0)].bg
    );
}

#[test]
fn gauge_rows_fill_progressively_and_keep_height_stable() {
    let note = "passive wait blocked: status/sleep calls were not started";
    let low = crate::turn_event_view::notice_gauge_text(note, 3);
    let hot = crate::turn_event_view::notice_gauge_text(note, 17);
    for (text, fill) in [(&low, GAUGE_LOW_FILL), (&hot, GAUGE_HOT_FILL)] {
        let message = Message {
            role: Role::Activity,
            text: text.as_str().into(),
        };
        let mut renders = Vec::new();
        let rendered = lines(std::slice::from_ref(&message), "", false, 120, &mut renders);
        assert_eq!(rendered.len(), 1);
        let filled = &rendered[0].spans[1];
        assert_eq!(filled.style, fill, "leading span carries the fill: {text}");
        // The chip is the styled count at the end of the row.
        let chip = rendered[0].spans.last().unwrap();
        assert!(chip.style.add_modifier.contains(Modifier::BOLD));
        // Same text bytes in, same text bytes out — wrap height can't drift.
        let joined: String = rendered[0]
            .spans
            .iter()
            .skip(1)
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(&joined, text);
    }
    // Low repeat fills part of the row; saturation covers the whole body.
    let low_line = activity_line(&low);
    assert!(
        low_line
            .spans
            .iter()
            .any(|span| span.style == ACTIVITY_STYLE && !span.content.trim().is_empty()),
        "a low count leaves unfilled body text"
    );
    let hot_line = activity_line(&hot);
    assert!(
        !hot_line
            .spans
            .iter()
            .any(|span| span.style == ACTIVITY_STYLE && span.content.len() > 3),
        "a saturated gauge fills the entire body"
    );
    // Plain notices render exactly as before.
    let plain = activity_line(".. context note");
    assert_eq!(plain.spans[1].style, ACTIVITY_STYLE);
}

#[test]
fn empty_message_still_emits_a_tag_line() {
    // Empty text with a tag → a single line carrying just the tag (the
    // start_len == len fallback in append_plain_message_lines).
    let (tag, style) = message_tag(Role::User);
    let rendered = plain_message_lines("", tag, style, None);
    assert_eq!(rendered.len(), 1);
    assert_eq!(rendered[0].spans.len(), 1);
    assert_eq!(rendered[0].spans[0].content.as_ref(), "you ");
    // plain_line_capacity reserves 1 for empty text.
    assert_eq!(plain_line_capacity(""), 1);
}

#[test]
fn empty_message_with_no_tag_emits_a_blank_line() {
    // The Activity tag is empty: empty text → a bare blank line, no tag span.
    let (tag, style) = message_tag(Role::Activity);
    assert_eq!(tag, "");
    let rendered = plain_message_lines("", tag, style, None);
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].spans.is_empty(), "blank line has no spans");
}

#[test]
fn markdown_agent_text_still_renders_markdown() {
    let messages = [Message {
        role: Role::Angel,
        text: "**bold**".into(),
    }];
    let mut renders = Vec::new();
    let rendered = lines(&messages, "", false, 80, &mut renders);
    let text = plain_text(&rendered);
    assert!(text.contains("angel "));
    assert!(text.contains("bold"));
    // The markdown body borrows from the cached render — a deep clone per
    // visible message per frame is the regression this guards against.
    assert!(matches!(
        rendered[0]
            .spans
            .iter()
            .find(|span| span.content == "bold")
            .unwrap()
            .content,
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn latex_only_agent_text_uses_the_markdown_math_renderer() {
    let messages = [Message {
        role: Role::Angel,
        text: r"$E = mc^2$".into(),
    }];
    let mut renders = Vec::new();
    let rendered = lines(&messages, "", false, 80, &mut renders);
    let text = plain_text(&rendered);
    assert!(text.contains("E = mc²"), "{text}");
    assert!(!text.contains('$'), "{text}");
}
