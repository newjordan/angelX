use super::*;

fn flatten(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn modal_area_is_centered_and_bounded() {
    let full = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 40,
    };
    let area = modal_area(full, "approve this action?", None);
    assert_eq!(area.width, 72);
    assert!(area.height >= 5);
    assert_eq!(area.x, 24);
    assert!(area.y > 0);
}

#[test]
fn text_width_uses_terminal_display_cells() {
    assert_eq!(display_cell_width("approve this action?"), 20);
    assert_eq!(display_cell_width("run 日本語?"), 11);
    assert_eq!(display_cell_width("approve 🧪?"), 11);
}

#[test]
fn tiny_modal_never_exceeds_its_terminal_rect() {
    for full in [
        Rect::new(0, 0, 0, 0),
        Rect::new(4, 3, 1, 1),
        Rect::new(4, 3, 2, 2),
        Rect::new(4, 3, 8, 4),
        Rect::new(4, 3, 16, 6),
        Rect::new(4, 3, 19, 8),
    ] {
        let area = modal_area(full, "approve 日本語 action?", Some("remote host · spark"));
        assert!(area.x >= full.x && area.y >= full.y, "{full:?} -> {area:?}");
        assert!(area.width <= full.width, "{full:?} -> {area:?}");
        assert!(area.height <= full.height, "{full:?} -> {area:?}");
        assert!(
            area.x.saturating_add(area.width) <= full.x.saturating_add(full.width),
            "{full:?} -> {area:?}"
        );
        assert!(
            area.y.saturating_add(area.height) <= full.y.saturating_add(full.height),
            "{full:?} -> {area:?}"
        );
    }
}

#[test]
fn multiline_body_preserves_explicit_rows_and_scope_controls() {
    let prompt = "Action capsule:\n• write result.json\n\nApprove?\n";
    let scope = "action batch · action-capsule:cf75851bcf4d1766";
    let actual = body(prompt, Some(scope));
    let expected: Vec<_> = prompt.split('\n').collect();
    assert_eq!(actual.len(), expected.len() + 4);
    for (line, expected) in actual.iter().zip(expected.iter()) {
        assert_eq!(flatten(std::slice::from_ref(line)), *expected);
        assert!(line.spans.iter().all(|span| span.style == PROMPT_STYLE));
    }
    assert_eq!(
        flatten(&actual[expected.len() + 1..expected.len() + 2]),
        format!("scope: {scope}")
    );
    assert_eq!(flatten(&actual[actual.len() - 1..]), APPROVAL_CONTROLS);
}

#[test]
fn multiline_modal_renders_separate_prompt_rows_wide_and_narrow() {
    use ratatui::{Terminal, backend::TestBackend, widgets::Clear};

    let prompt = "Action capsule — 1 operation(s):\n• run shell command: cat source.json\n\nApprove this bounded batch? No verifier or extra command will run automatically.\n";
    let scope = Some("action batch · action-capsule:cf75851bcf4d1766");
    for (width, height) in [(132, 44), (36, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut area = Rect::default();
        terminal
            .draw(|frame| {
                area = modal_area(frame.area(), prompt, scope);
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(body(prompt, scope))
                        .wrap(Wrap { trim: false })
                        .block(block()),
                    area,
                );
            })
            .unwrap();
        let inner = block().inner(area);
        assert!(approval_body_rows(prompt, scope, inner.width) <= inner.height);
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (inner.y..inner.bottom())
            .map(|y| {
                (inner.x..inner.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect();
        let heading = rows
            .iter()
            .position(|row| row.contains("Action capsule"))
            .unwrap();
        let command = rows
            .iter()
            .position(|row| row.contains("run shell command:"))
            .unwrap();
        let file = rows
            .iter()
            .position(|row| row.contains("source.json"))
            .unwrap();
        let question = rows
            .iter()
            .position(|row| row.contains("Approve this bounded"))
            .unwrap();
        assert!(
            command > heading,
            "prompt rows collapsed at width {width}: {rows:?}"
        );
        assert!(
            question > file + 1,
            "explicit blank row lost at width {width}: {rows:?}"
        );
        let compact: String = rows
            .concat()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect();
        assert!(compact.contains("scope:actionbatch·action-capsule:cf75851bcf4d1766"));
        for control in ["[y]approve", "[a]approveall(thisturn)", "[n]deny"] {
            assert!(
                compact.contains(control),
                "clipped control at width {width}: {rows:?}"
            );
        }
    }
}

#[test]
fn compact_modal_reserves_wrapped_rows_for_every_control() {
    use ratatui::{Terminal, backend::TestBackend, widgets::Clear};

    let prompt = "approve this action?";
    let scope = Some("remote host · spark");
    let backend = TestBackend::new(24, 14);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut area = Rect::default();
    terminal
        .draw(|frame| {
            area = modal_area(frame.area(), prompt, scope);
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(body(prompt, scope))
                    .wrap(Wrap { trim: false })
                    .block(block()),
                area,
            );
        })
        .unwrap();

    let inner = block().inner(area);
    assert!(approval_body_rows(prompt, scope, inner.width) <= inner.height);
    let screen = crate::draw::test_backend_text(terminal.backend());
    assert!(screen.contains("[y] approve"), "{screen}");
    assert!(
        screen.contains("[a]") && screen.contains("approve all"),
        "{screen}"
    );
    assert!(screen.contains("[n] deny"), "{screen}");
}

#[test]
fn approval_styles_use_the_semantic_hud_palette() {
    assert_eq!(PROMPT_STYLE.fg, Some(HUD_TEXT));
    assert_eq!(SCOPE_STYLE.fg, Some(HUD_BLUE));
    assert_eq!(CONTROLS_STYLE.fg, Some(HUD_GOLD));
}

#[test]
fn body_includes_prompt_and_controls() {
    let prompt = String::from("run external command?");
    let body = body(&prompt, None);
    assert!(matches!(
        body[0].spans[0].content,
        std::borrow::Cow::Borrowed(_)
    ));
    let text = flatten(&body);
    assert!(text.contains("run external command?"));
    assert!(text.contains("[y] approve"));
    assert!(text.contains("[n] deny"));
}

#[test]
fn scoped_body_and_geometry_expose_trusted_identity() {
    let full = Rect::new(0, 0, 120, 40);
    let unscoped = modal_area(full, "approve this action?", None);
    let scoped = modal_area(full, "approve this action?", Some("remote host · spark"));
    assert!(scoped.height > unscoped.height);
    let text = flatten(&body("approve this action?", Some("remote host · spark")));
    assert!(text.contains("scope: remote host · spark"));

    let wrapping_scope = "x".repeat(65);
    assert_eq!(
        modal_area(full, "ok", Some(&wrapping_scope)).height,
        8,
        "the trusted scope prefix participates in wrapping geometry"
    );
}

#[test]
fn approval_controls_line_is_cached() {
    let controls = approval_controls_line();
    assert!(controls.spans[0].content.contains("[a] approve all"));
    assert_eq!(
        controls.spans[0].content.as_ptr(),
        approval_controls_line().spans[0].content.as_ptr()
    );
}

#[test]
fn approval_blank_line_is_cached() {
    let blank = approval_blank_line();
    assert!(blank.spans.is_empty() || blank.spans.iter().all(|span| span.content.is_empty()));
    assert_eq!(blank, approval_blank_line());
}

#[test]
fn approval_keys_map_to_decisions() {
    assert_eq!(
        decision_for_key(KeyCode::Char('y')),
        Some(Decision::Approve)
    );
    assert_eq!(
        decision_for_key(KeyCode::Char('a')),
        Some(Decision::ApproveAll)
    );
    assert_eq!(decision_for_key(KeyCode::Esc), Some(Decision::Deny));
    assert_eq!(decision_for_key(KeyCode::Char('x')), None);
}

#[test]
fn decision_text_matches_activity_copy() {
    assert_eq!(decision_text(Decision::Approve), "approved");
    assert_eq!(
        decision_text(Decision::ApproveAll),
        "approved (all this turn)"
    );
    assert_eq!(decision_text(Decision::Deny), "denied");
}

#[test]
fn approval_title_is_cached() {
    let title = approval_title();
    assert!(title.contains("approval required"));
    assert_eq!(title.as_ptr(), approval_title().as_ptr());
}
