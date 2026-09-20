use super::*;

#[test]
fn styled_caption_clips_to_terminal_cells_for_wide_unicode() {
    let clipped = clip_caption(
        Line::from(vec![
            Span::styled("資料/設計🧪", Style::new().fg(crate::hud::HUD_TEXT)),
            Span::styled(" → Scriptorium", Style::new().fg(crate::hud::HUD_BLUE)),
        ]),
        12,
    );
    let text = clipped
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    assert!(
        unicode_width::UnicodeWidthStr::width(text.as_str()) <= 12,
        "clipped caption overflowed 12 cells: {text:?}"
    );
    assert!(text.ends_with('…'), "clipped caption needs an ellipsis");
}
