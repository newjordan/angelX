use super::*;

#[test]
fn height_matches_paragraph_for_contextual_width_and_split_spans() {
    for text in [
        "",
        " ",
        "\t",
        "a\tb",
        "word      next",
        "a\r\nb",
        "\u{00ad}",
        "لا",
        "لالا",
        "لَا",
        "שלום",
        "क्‍ष",
        "नमस्ते",
        "界界界",
        "e\u{301}",
        "👩‍🔬",
        "❤️",
        "♥︎",
        "🇯🇵",
        "👨‍👩‍👧‍👦",
        "\u{200b}",
    ] {
        for split in [false, true] {
            let line = if split {
                Line::from(
                    text.chars()
                        .map(|ch| Span::raw(ch.to_string()))
                        .collect::<Vec<_>>(),
                )
            } else {
                Line::raw(text)
            };
            for width in (0..120).chain([65_535, 65_536]) {
                let lines = vec![line.clone(), Line::raw(""), Line::raw("tail")];
                let expected = Paragraph::new(lines.clone())
                    .wrap(Wrap { trim: false })
                    .line_count(width.max(1) as u16) as u32;
                assert_eq!(
                    wrapped_height_u32(lines, width),
                    expected,
                    "{text:?} split={split} width={width}"
                );
            }
        }
    }
}

#[test]
#[ignore = "explicit repeated resize height measurement"]
fn resize_height_rebuild_measurement() {
    let messages: Vec<_> = (0..2_000).map(|i| Message::new(
        if i % 2 == 0 { Role::User } else { Role::Angel },
        format!("History {i}: read **bounded pages**, keep `symbols` and verifier evidence. 日本語 e\u{301} 👩‍🔬\nSecond row of stable context.")
    )).collect();
    for repeat in 1..=5 {
        for width in [60, 80, 103] {
            let started = std::time::Instant::now();
            let heights: Vec<_> = messages.iter().map(|m| message_height(m, width)).collect();
            let elapsed_us = started.elapsed().as_micros();
            eprintln!(
                "RESIZE_HEIGHT_WORK {}",
                serde_json::json!({
                    "repeat": repeat, "width": width, "messages": messages.len(),
                    "elapsed_us": elapsed_us, "heights": heights,
                })
            );
        }
    }
}

#[test]
fn chat_roles_align_and_color_wrapped_rows_without_restyling_structured_output() {
    use ratatui::{
        buffer::Buffer,
        layout::{Alignment, Rect},
        widgets::Widget,
    };
    let messages = [
        Message::new(Role::User, "second"),
        Message::new(Role::Angel, "reply 界\n  aligned columns"),
    ];
    for width in [12, 30, 179] {
        let mut renders = Vec::new();
        let rendered = lines(&messages, "", false, width, &mut renders);
        for line in &rendered[..1] {
            assert_eq!(line.alignment, Some(Alignment::Right));
            assert_eq!(line.spans.last().unwrap().style, USER_TEXT_STYLE);
            assert!(
                !line
                    .spans
                    .last()
                    .unwrap()
                    .style
                    .add_modifier
                    .contains(Modifier::BOLD)
            );
        }
        for line in &rendered[2..] {
            assert_ne!(line.alignment, Some(Alignment::Right));
        }
        let height = messages
            .iter()
            .map(|m| message_height(m, width))
            .sum::<u16>();
        let area = Rect::new(3, 2, width as u16, height);
        let mut buf = Buffer::empty(area);
        Paragraph::new(rendered)
            .wrap(Wrap { trim: false })
            .render(area, &mut buf);
        // Last user row hugs the right edge; reply begins at the left. Body
        // phosphor differs from default prose; gold remains on the agent tag.
        let user_end = area.y + message_height(&messages[0], width) - 2;
        assert_eq!(buf[(area.right() - 1, user_end)].symbol(), "d");
        assert_eq!(
            buf[(area.right() - 1, user_end)].fg,
            crate::ui::hud::HUD_PHOSPHOR
        );
        let agent_y = area.y + message_height(&messages[0], width);
        assert_eq!(buf[(area.x, agent_y)].symbol(), "a");
        assert_eq!(buf[(area.x, agent_y)].fg, crate::ui::hud::HUD_GOLD);
        assert_ne!(buf[(area.x + 6, agent_y)].fg, crate::ui::hud::HUD_PHOSPHOR);
    }

    // Do not tint syntax spans, indent/align tables, or recolor tool/status rows.
    for (role, text) in [
        (
            Role::Angel,
            "```rust\nlet x = 1;\n```\n\n| key | value |\n| --- | --- |\n| 界 | 2 |",
        ),
        (Role::Activity, "shell: cargo test\n  test result: ok"),
        (Role::System, "  key    value"),
        (Role::Council, "delegate result"),
    ] {
        let message = Message::new(role, text);
        let md = message_render(&message, 60);
        let mut rendered = Vec::new();
        append_message_lines(&mut rendered, &message, borrowed(&md), 60);
        assert!(
            rendered
                .iter()
                .all(|line| line.alignment != Some(Alignment::Right))
        );
        if let Some(md) = &md {
            for (actual, original) in rendered.iter().zip(md.iter()) {
                let mut actual = actual.clone();
                if actual.spans.first().is_some_and(|s| s.content == "angel ") {
                    actual.spans.remove(0);
                }
                assert_eq!(
                    &actual, original,
                    "structured agent content remains verbatim"
                );
            }
        }
    }
}

#[test]
fn multiline_unicode_copy_uses_unpadded_cells_after_wrap_and_scroll() {
    use crate::ui::mouse::{self, PaneId, PaneRegistry, Selection};
    use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
    let message = Message::new(Role::User, "prefix words wrap here\n界e\u{301}👩‍💻");
    for width in [12, 30, 179] {
        let height = message_height(&message, width);
        for scroll in [0, height - 3] {
            let area = Rect::new(4, 3, width as u16, height - scroll);
            let mut buf = Buffer::empty(Rect::new(0, 0, area.right() + 1, area.bottom()));
            let messages = [Message::new(Role::User, message.text.clone())];
            let mut renders = Vec::new();
            Paragraph::new(lines(&messages, "", false, width, &mut renders))
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .render(area, &mut buf);
            let y = area.y + height - 2 - scroll;
            let x = area.x;
            assert_eq!(buf[(x, y)].symbol(), "界");
            assert_eq!(buf[(x + 2, y)].symbol(), "e\u{301}");
            assert_eq!(buf[(x + 3, y)].symbol(), "👩‍💻");
            let mut panes = PaneRegistry::default();
            panes.push(PaneId::Transcript, area);
            assert_eq!(panes.pane_at(x + 1, y), Some((PaneId::Transcript, area)));
            assert_eq!(panes.pane_at(area.right(), y), None);
            let mut selection = Selection::new(PaneId::Transcript, area, x, y);
            selection.extend(x + 4, y);
            assert_eq!(
                mouse::extract_text_in(&buf, &selection, area),
                "界e\u{301}👩‍💻"
            );
        }
    }
}

#[test]
fn corrective_user_paste_keeps_code_columns_and_multiline_mouse_copy_unpadded() {
    use crate::ui::mouse::{self, PaneId, Selection};
    use ratatui::{
        buffer::Buffer,
        layout::{Alignment, Rect},
        widgets::Widget,
    };
    for text in [
        "if x:\n    pass\n    longer()",
        "```python\nif x:\n    pass\n    longer()\n```",
        "name    count\nx       3\nlong    20",
        "界   e\u{301}\n👩‍💻   ok",
    ] {
        for width in [20, 80, 179] {
            let messages = [Message::new(Role::User, text)];
            let mut renders = Vec::new();
            let rendered = lines(&messages, "", false, width, &mut renders);
            assert_eq!(rendered[0].alignment, Some(Alignment::Right));
            assert_eq!(
                rendered[0].spans[0].style.fg,
                Some(crate::ui::hud::HUD_PURPLE)
            );
            assert!(
                rendered[1..]
                    .iter()
                    .all(|line| line.alignment != Some(Alignment::Right))
            );
            let height = message_height(&messages[0], width);
            assert_eq!(height, text.lines().count() as u16 + 2);
            let area = Rect::new(3, 2, width as u16, height);
            let mut buf = Buffer::empty(area);
            Paragraph::new(rendered)
                .wrap(Wrap { trim: false })
                .render(area, &mut buf);
            let mut selection = Selection::new(PaneId::Transcript, area, area.x, area.y + 1);
            selection.extend(area.right() - 1, area.bottom() - 2);
            assert_eq!(mouse::extract_text_in(&buf, &selection, area), text);
        }
    }
    // A long single-line paste also must not generate leading alignment blanks
    // on its soft-wrapped continuation rows.
    let messages = [Message::new(Role::User, "word word word word word word")];
    let mut renders = Vec::new();
    let rendered = lines(&messages, "", false, 12, &mut renders);
    assert!(
        rendered[1..]
            .iter()
            .all(|line| line.alignment != Some(Alignment::Right))
    );
}
