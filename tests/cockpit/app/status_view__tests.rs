use super::*;

fn flatten(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

#[test]
fn fmt_clock_handles_minutes_and_hours() {
    assert_eq!(fmt_clock(0.0), "00:00");
    assert_eq!(fmt_clock(65.0), "01:05");
    assert_eq!(fmt_clock(3661.0), "1:01:01");
}

#[test]
fn header_line_uses_cached_idle_lines_for_known_agents() {
    let line = header_line("practice", Glyph::Idle, 0.0, 0.0);
    let text = flatten(std::slice::from_ref(&line));
    assert!(text.contains("angel0"));
    assert!(text.contains("Practice"));
    assert!(text.contains("idle"));
    assert!(text.contains("00%"));
    assert!(matches!(
        header_line("spark", Glyph::Idle, 0.0, 0.0).spans[1].content,
        Cow::Borrowed(_)
    ));
    assert!(matches!(
        header_line("spark-r1", Glyph::Idle, 0.0, 0.0).spans[1].content,
        Cow::Borrowed(_)
    ));
    assert!(matches!(
        header_line("custom", Glyph::Idle, 0.0, 0.0).spans[1].content,
        Cow::Borrowed(_)
    ));
    assert!(matches!(
        header_line("practice", Glyph::Thinking, 0.0, 0.0).spans[1].content,
        Cow::Borrowed(_)
    ));
}

#[test]
fn working_lines_include_agent_clock_and_load_metrics() {
    let snapshot = OverwatchSnapshot {
        cpu_pct: 12.3,
        gpu_pct: 45.6,
        ..OverwatchSnapshot::default()
    };
    let title = working_title("moa", 65.0, snapshot.load_pct());
    assert!(matches!(title, Cow::Owned(_)));
    let text = flatten(&working_lines("moa", 65.0, snapshot));
    assert!(title.contains("moa"));
    assert!(title.contains("01:05"));
    assert!(text.contains("12.3%"));
    assert!(text.contains("45.6%"));
}

#[test]
fn working_helpers_can_share_preformatted_clock() {
    let snapshot = OverwatchSnapshot {
        cpu_pct: 12.3,
        gpu_pct: 45.6,
        ..OverwatchSnapshot::default()
    };
    let clock = fmt_clock(65.0);
    let title = working_title_with_clock("moa", &clock, snapshot.load_pct());
    let lines = working_lines_with_clock("moa", &clock, snapshot);
    assert!(title.contains("01:05"));
    assert!(matches!(lines[0].spans[4].content, Cow::Borrowed(_)));
    assert_eq!(lines[0].spans[4].content.as_ptr(), clock.as_ptr());
}

#[test]
fn working_title_borrows_zero_state_for_known_agents() {
    let title = working_title("practice", 0.0, 0.0);
    assert!(matches!(title, Cow::Borrowed(_)));
    assert!(title.contains("00:00"));
    assert!(matches!(working_title("spark", 0.2, 0.0), Cow::Borrowed(_)));
    assert!(matches!(
        working_title("spark-r1", 0.0, 0.0),
        Cow::Borrowed(_)
    ));
    assert!(matches!(working_title("custom", 0.0, 0.0), Cow::Owned(_)));
    assert!(matches!(working_title("practice", 1.0, 0.0), Cow::Owned(_)));
    assert!(matches!(
        working_title("practice", 0.0, 12.0),
        Cow::Owned(_)
    ));
}

#[test]
fn working_lines_borrow_zero_state_for_known_agents() {
    let lines = working_lines("practice", 0.0, OverwatchSnapshot::default());
    let text = flatten(&lines);
    assert!(text.contains("practice"));
    assert!(text.contains("00.0%"));
    assert!(matches!(lines[0].spans[2].content, Cow::Borrowed(_)));
    let dynamic = working_lines(
        "practice",
        0.0,
        OverwatchSnapshot {
            cpu_pct: 1.0,
            ..OverwatchSnapshot::default()
        },
    );
    assert!(matches!(dynamic[0].spans[8].content, Cow::Owned(_)));
}

#[test]
fn compact_load_suffix_borrows_default_zero_metrics() {
    let suffix = compact_load_suffix(0.0, 0.0);
    assert!(matches!(suffix, Cow::Borrowed(_)));
    assert!(suffix.contains("00%"));
    assert!(matches!(compact_load_suffix(12.0, 0.0), Cow::Owned(_)));
}

#[test]
fn agent_bay_title_borrows_default_idle_agent() {
    let title = agent_bay_title("A>", "Agent", Glyph::Idle, 0.0, 0.0);
    assert!(matches!(title, Cow::Borrowed(_)));
    assert!(title.contains("Agent"));
    let sparky = agent_bay_title("Sp", "Sparky", Glyph::Idle, 0.0, 0.0);
    assert!(matches!(sparky, Cow::Borrowed(_)));
    assert!(sparky.contains("Sparky"));
    assert!(matches!(
        agent_bay_title("Sp", "Agent", Glyph::Idle, 0.0, 0.0),
        Cow::Owned(_)
    ));
}

#[test]
fn agent_metrics_line_borrows_default_idle_metrics() {
    let line = agent_metrics_line(0.0, 0.0, 0.0, false);
    assert!(matches!(line, Cow::Borrowed(_)));
    assert_eq!(line, "C>00% M>00% G>00% .. idle");
    let loaded = agent_metrics_line(1.0, 42.0, 0.0, false);
    assert!(matches!(loaded, Cow::Owned(_)));
    assert!(loaded.contains("M>42%"), "{loaded}");
    let active = agent_metrics_line(0.0, 0.0, 0.0, true);
    assert!(matches!(active, Cow::Borrowed(_)));
    assert_eq!(active, "C>00% M>00% G>00% *> active");
}

#[test]
fn agent_route_title_uses_active_box_mode_and_busy_metrics() {
    use crate::club::ClubTab;
    let tabs = [
        ClubTab {
            label: "openai".into(),
            mode: None,
            in_hand: false,
            available: true,
        },
        ClubTab {
            label: "sota".into(),
            mode: Some("longcat".into()),
            in_hand: true,
            available: true,
        },
    ];
    let title = agent_route_title(&tabs, Some("08:28"), Some(6.0), 40).unwrap();
    assert!(title.contains("longcat"), "{title}");
    assert!(
        !title.contains("sota·"),
        "bay title must not prefix the machine slug: {title}"
    );
    assert!(title.contains("08:28"), "{title}");
    assert!(title.contains("load 06%"), "{title}");
    assert!(
        !title.contains("openai"),
        "agent chrome shows current route, not the whole tab strip: {title}"
    );
}

#[test]
fn token_usage_meter_formats_last_and_session_counts() {
    let meter = token_usage_meter(Some(TokenUsage {
        turns: 2,
        last_input: 20,
        last_output: 6,
        last_reasoning: 3,
        total_input: 30,
        total_output: 10,
        total_reasoning: 3,
    }));
    assert!(meter.contains("tok in 20/30"));
    assert!(meter.contains("out 6/10"));
    assert!(meter.contains("rsn 3/3"));
    assert_eq!(token_usage_meter(None), "tok in --/-- · out --/--");
}

#[test]
fn moa_token_report_lines_show_per_model_counts_and_bars() {
    let report = crate::swarm::ledger::MoaTokenReport {
        turns: 3,
        latest_route: "deliberate".into(),
        latest_ok: true,
        latest_ms: 1200,
        rows: vec![
            crate::swarm::ledger::MoaModelTokenRow {
                label: "openai".into(),
                last_input: 1200,
                last_output: 300,
                total_input: 2400,
                total_output: 600,
            },
            crate::swarm::ledger::MoaModelTokenRow {
                label: "deepseek".into(),
                last_input: 600,
                last_output: 100,
                total_input: 900,
                total_output: 150,
            },
        ],
    };
    let lines = moa_token_report_lines(&report, 48, 3);
    let text = flatten(&lines);
    assert!(text.contains("moa tok 2200"), "{text}");
    assert!(text.contains("openai"), "{text}");
    assert!(text.contains("1200/300"), "{text}");
    assert!(text.contains("deepseek"), "{text}");
    assert!(
        text.chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)),
        "dotmax braille bar missing: {text}"
    );
}

#[test]
fn message_title_includes_active_agent_label() {
    let title = message_title("spark");
    assert!(matches!(title, Cow::Borrowed(_)));
    assert!(title.contains("[spark]"));
    assert_eq!(title, message_title("spark"));
    assert!(matches!(message_title("spark-r1"), Cow::Borrowed(_)));
    assert!(matches!(message_title("turbo"), Cow::Borrowed(_)));
    assert!(matches!(message_title("atlas"), Cow::Borrowed(_)));
    assert!(matches!(message_title("gemma"), Cow::Borrowed(_)));
    assert!(matches!(message_title("moa"), Cow::Borrowed(_)));
    assert!(matches!(message_title("deli"), Cow::Borrowed(_)));
    assert!(matches!(message_title("custom"), Cow::Owned(_)));
}

#[test]
fn practice_message_title_uses_static_fast_path() {
    let title = message_title("practice");
    assert!(matches!(title, Cow::Borrowed(_)));
    assert!(title.contains("[practice]"));
}

#[test]
fn message_title_tabs_lists_boxes_with_active_mode() {
    use crate::club::ClubTab;
    let tab = |label: &str, mode: Option<&str>, in_hand: bool, available: bool| ClubTab {
        label: label.to_string(),
        mode: mode.map(str::to_string),
        in_hand,
        available,
    };
    // In-hand spark box on its moa mode, plus reachable atlas box. The offline
    // turbo box is already absent from the list the caller built.
    let tabs = [
        tab("spark", Some("moa"), true, true),
        tab("atlas", None, false, true),
    ];
    let title = message_title_tabs(&tabs);
    assert!(
        title.contains("[spark·moa]"),
        "in-hand box shows its mode: {title}"
    );
    assert!(
        title.contains("atlas"),
        "the other reachable box is listed: {title}"
    );
    assert!(
        !title.contains("turbo"),
        "offline boxes are absent: {title}"
    );
    assert!(
        title.contains("←→ mode"),
        "multi-mode box shows the subcontrol hint: {title}"
    );

    // A lone reachable single-mode box collapses to the cached label title.
    let one = [tab("atlas", None, true, true)];
    assert_eq!(message_title_tabs(&one), message_title("atlas"));

    // An in-hand box that just went offline is still shown, flagged down.
    let down = [
        tab("spark", Some("moa"), true, false),
        tab("atlas", None, false, true),
    ];
    let title = message_title_tabs(&down);
    assert!(
        title.contains(Glyph::Offline.token()),
        "down in-hand box carries the offline mark: {title}"
    );
}

#[test]
fn reasoning_title_uses_static_fast_path_for_known_agents() {
    let title = reasoning_title("practice");
    assert!(matches!(title, Cow::Borrowed(_)));
    assert!(title.contains("thinking"));
    assert!(title.contains("Practice"));
    assert!(matches!(reasoning_title("spark"), Cow::Borrowed(_)));
    assert!(matches!(reasoning_title("spark-r1"), Cow::Borrowed(_)));
    assert!(matches!(reasoning_title("turbo"), Cow::Borrowed(_)));
    assert!(matches!(reasoning_title("atlas"), Cow::Borrowed(_)));
    assert!(matches!(reasoning_title("gemma"), Cow::Borrowed(_)));
    assert!(matches!(reasoning_title("moa"), Cow::Borrowed(_)));
    assert!(matches!(reasoning_title("deli"), Cow::Borrowed(_)));
    assert!(matches!(reasoning_title("custom"), Cow::Owned(_)));
}

#[test]
fn reasoning_title_caches_custom_labels() {
    let first = reasoning_title("operator-x");
    let second = reasoning_title("operator-x");
    assert!(matches!(first, Cow::Owned(_)));
    assert_eq!(first, second);
    assert!(second.contains("operator-x"));
}

#[test]
fn empty_artifacts_line_is_cached_and_readable() {
    let line = empty_artifacts_line();
    let text = flatten(std::slice::from_ref(&line));
    assert!(text.contains("no delivered artifacts"));
    assert_eq!(
        line.spans[0].content.as_ptr(),
        empty_artifacts_line().spans[0].content.as_ptr()
    );
}

#[test]
fn agent_shell_title_is_cached_and_readable() {
    let title = agent_shell_title();
    assert!(title.contains("agent shell"));
    assert_eq!(title, agent_shell_title());
}

#[test]
fn shell_focus_title_is_cached_and_readable() {
    let title = shell_focus_title();
    assert!(title.contains("shell"));
    assert!(title.contains("^G"));
    assert_eq!(title.as_ptr(), shell_focus_title().as_ptr());
}

#[test]
fn image_viewer_title_is_cached_and_readable() {
    let title = image_viewer_title();
    assert!(title.contains("/hide to return"));
    assert_eq!(title.as_ptr(), image_viewer_title().as_ptr());
}

#[test]
fn shell_hint_is_cached_static_text() {
    let hint = shell_hint();
    assert!(hint.contains("shell focused"));
    assert_eq!(hint.as_ptr(), shell_hint().as_ptr());
}

#[test]
fn composer_placeholder_pads_full_width_to_kill_ghosts() {
    let width = 80;
    for busy in [false, true] {
        let line = composer_placeholder(busy, width);
        let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(
            composer_display_width(&flat),
            width,
            "busy={busy}: placeholder must fill every display cell (got {flat:?})"
        );
        assert!(
            line.spans
                .iter()
                .all(|s| s.style.fg == Some(crate::hud::HUD_DIM)),
            "busy={busy}: placeholder stays dim"
        );
    }
}

/// A3: a double-width glyph in a width-1 pane must still pad/clip to one cell
/// after the draw-path pad pass (otherwise a neighbor cell ghosts).
#[test]
fn pad_composer_lines_clips_wide_glyph_in_narrow_pane() {
    let width = 1;
    let mut view = composer_view("🙂", width, 1, 1);
    pad_composer_lines(&mut view.lines, width);
    assert!(!view.lines.is_empty());
    let flat: String = view.lines[0]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(
        composer_display_width(&flat),
        width,
        "narrow pane must not leave a 2-cell emoji row: {flat:?}"
    );
}

/// A3: multi-span rows (styled tokens + wide glyphs) truncate to exact width.
#[test]
fn pad_composer_lines_multi_span_truncates_to_exact_display_width() {
    let width = 8;
    let mut lines = vec![Line::from(vec![
        Span::styled("hi", HUD_TEXT_STYLE),
        Span::styled("🙂🙂🙂", HUD_GOLD_BOLD_STYLE), // typically 6 cells
    ])];
    pad_composer_lines(&mut lines, width);
    let flat: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(composer_display_width(&flat), width, "exact={flat:?}");

    let mut long = vec![Line::from(vec![
        Span::styled("hello", HUD_TEXT_STYLE),    // 5
        Span::styled("世界世界", HUD_TEXT_STYLE), // 8
    ])];
    pad_composer_lines(&mut long, width);
    let flat: String = long[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(
        composer_display_width(&flat),
        width,
        "multi-span overflow must clip to {width}: {flat:?}"
    );
}

#[test]
fn pad_composer_lines_rewrites_short_rows() {
    let width = 40;
    let mut lines = vec![Line::from(Span::styled("short", HUD_TEXT_STYLE))];
    pad_composer_lines(&mut lines, width);
    let flat: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(flat.starts_with("short"));
    assert!(flat.ends_with(' '));
    assert_eq!(composer_display_width(&flat), width);
}

/// A3: wide glyphs are 2 terminal cells — char-count pad left ghost cells.
#[test]
fn pad_composer_row_uses_display_width_for_wide_glyphs() {
    let width = 10;
    // "🙂" is typically width 2; "hi" is 2 → content display 4 → pad 6 spaces.
    let row = pad_composer_row("hi🙂", width);
    assert_eq!(
        composer_display_width(&row),
        width,
        "row={row:?} must fill {width} cells"
    );
    // Truncate: a string wider than the pane must still land on exact width.
    let wide = pad_composer_row("日本語テスト長め", 6);
    assert_eq!(composer_display_width(&wide), 6, "wide={wide:?}");
}

#[test]
fn pad_composer_lines_clears_wide_glyph_residue_on_shorten() {
    let width = 12;
    // Simulate a long draft row that then shrinks to a short one.
    let mut lines = vec![Line::from(Span::styled(
        pad_composer_row("旧い長い下書き🙂残渣", width),
        HUD_TEXT_STYLE,
    ))];
    assert_eq!(composer_display_width(&flatten_line(&lines[0])), width);
    // User cleared to a short draft.
    lines[0] = Line::from(Span::styled("ok", HUD_TEXT_STYLE));
    pad_composer_lines(&mut lines, width);
    assert_eq!(
        composer_display_width(&flatten_line(&lines[0])),
        width,
        "shortened draft must rewrite every cell"
    );
    let flat = flatten_line(&lines[0]);
    assert!(flat.starts_with("ok"));
    assert!(!flat.contains('旧'), "no residual CJK from prior draft");
}

/// A3: pasted tabs (and other controls) must not under-pad — unicode-width
/// treats TAB as 0 while the terminal expands it, leaving residual cells.
#[test]
fn pad_composer_row_sanitizes_tabs_to_exact_display_width() {
    let width = 10;
    let row = pad_composer_row("a\tb\tc", width);
    assert!(
        !row.contains('\t'),
        "TAB must not survive into painted cells: {row:?}"
    );
    assert_eq!(
        composer_display_width(&row),
        width,
        "tabbed draft must still fill every cell: {row:?}"
    );
    // Multi-span pad path also rewrites controls.
    let mut lines = vec![Line::from(vec![
        Span::styled("id\t", HUD_TEXT_STYLE),
        Span::styled("name\tval", HUD_GOLD_BOLD_STYLE),
    ])];
    pad_composer_lines(&mut lines, width);
    let flat = flatten_line(&lines[0]);
    assert!(!flat.contains('\t'), "multi-span: {flat:?}");
    assert_eq!(composer_display_width(&flat), width, "multi-span: {flat:?}");
}

/// A3: soft-hyphen / ZWSP / BOM report width 0 but terminals may still reserve
/// a cell — counting them as 0 under-pads and leaves residual neighbor cells.
/// Format ignorables become spaces (1 cell); emoji ZWJ stays for clusters.
#[test]
fn pad_composer_row_sanitizes_format_zwsp_to_exact_display_width() {
    let width = 8;
    let dirty = "ab\u{200b}\u{00ad}cd\u{feff}"; // 4 letters + ZWSP + SHY + BOM
    let row = pad_composer_row(dirty, width);
    assert!(
        !row.chars().any(|c| matches!(
            c,
            '\u{200B}' | '\u{00AD}' | '\u{FEFF}' | '\u{2060}' | '\u{180E}'
        )),
        "format ZW must not survive into painted cells: {row:?}"
    );
    assert_eq!(
        composer_display_width(&row),
        width,
        "format-ZW draft must fill every cell: {row:?}"
    );
    for ch in row.chars() {
        assert!(
            composer_char_display_width(ch) >= 1,
            "painted cell must be ≥1 wide after sanitize: {ch:?} in {row:?}"
        );
    }
    // Multi-span pad path rewrites format ZW the same way.
    let mut lines = vec![Line::from(vec![
        Span::styled("a\u{200b}", HUD_TEXT_STYLE),
        Span::styled("b\u{00ad}c", HUD_GOLD_BOLD_STYLE),
    ])];
    pad_composer_lines(&mut lines, width);
    let flat = flatten_line(&lines[0]);
    assert!(
        !flat.contains('\u{200b}') && !flat.contains('\u{00ad}'),
        "multi-span format ZW: {flat:?}"
    );
    assert_eq!(composer_display_width(&flat), width, "multi-span: {flat:?}");

    // Wrap path must match pad budget (no TAB-style measure skew).
    let wrapped = wrap_composer_rows(&"x\u{200b}\u{00ad}y".chars().collect::<Vec<_>>(), 4, 2);
    assert!(!wrapped.rows.is_empty());
    let painted = &wrapped.rows[0].text;
    assert!(
        !painted.contains('\u{200b}') && !painted.contains('\u{00ad}'),
        "wrap must sanitize format ZW: {painted:?}"
    );
    assert_eq!(
        composer_display_width(painted),
        painted.chars().count(),
        "after sanitize every char is 1-cell space or glyph: {painted:?}"
    );

    // Emoji ZWJ sequences stay joinable (not forced to spaces).
    let family = "👨\u{200d}👩";
    let kept = sanitize_composer_display(family);
    assert!(
        kept.contains('\u{200d}'),
        "ZWJ must remain for emoji clusters: {kept:?}"
    );
}

/// A3 adversarial: ZWNJ and bidi marks/embeddings report unicode-width 0
/// (same class as ZWSP) but were not sanitized — pad under-filled and left
/// residual cells after a cleared draft. ZWJ / VS16 stay for emoji.
#[test]
fn pad_composer_row_sanitizes_zwnj_and_bidi_marks() {
    let width = 10;
    let dirty = "ab\u{200c}\u{200e}\u{200f}cd\u{202a}\u{202c}\u{2066}\u{2069}";
    let row = pad_composer_row(dirty, width);
    assert!(
        !row.chars().any(|c| matches!(
            c,
            '\u{200C}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{202A}'
                | '\u{202B}'
                | '\u{202C}'
                | '\u{202D}'
                | '\u{202E}'
                | '\u{2066}'
                | '\u{2067}'
                | '\u{2068}'
                | '\u{2069}'
        )),
        "bidi/ZWNJ format marks must not survive paint: {row:?}"
    );
    assert_eq!(
        composer_display_width(&row),
        width,
        "bidi/ZWNJ draft must fill every cell: {row:?}"
    );
    for ch in row.chars() {
        assert!(
            composer_char_display_width(ch) >= 1,
            "painted cell ≥1 after sanitize: {ch:?} in {row:?}"
        );
    }
    // Multi-span + wrap paths share sanitize.
    let mut lines = vec![Line::from(vec![
        Span::styled("x\u{200c}", HUD_TEXT_STYLE),
        Span::styled("y\u{200e}z", HUD_GOLD_BOLD_STYLE),
    ])];
    pad_composer_lines(&mut lines, width);
    let flat = flatten_line(&lines[0]);
    assert!(
        !flat.contains('\u{200c}') && !flat.contains('\u{200e}'),
        "multi-span bidi/ZWNJ: {flat:?}"
    );
    assert_eq!(composer_display_width(&flat), width, "multi-span: {flat:?}");

    let wrapped = wrap_composer_rows(&"p\u{200c}\u{202a}q".chars().collect::<Vec<_>>(), 4, 2);
    assert!(!wrapped.rows.is_empty());
    let painted = &wrapped.rows[0].text;
    assert!(
        !painted.contains('\u{200c}') && !painted.contains('\u{202a}'),
        "wrap must sanitize bidi/ZWNJ: {painted:?}"
    );

    // ZWJ + variation selector still kept for emoji clusters.
    let emoji = "❤\u{fe0f}\u{200d}🔥";
    let kept = sanitize_composer_display(emoji);
    assert!(
        kept.contains('\u{200d}') && kept.contains('\u{fe0f}'),
        "ZWJ/VS must remain: {kept:?}"
    );
}

/// A3: Unicode LINE/PARAGRAPH SEPARATOR must hard-break like LF so the
/// terminal cannot paint an extra physical row under a short pad (ghosts).
/// Compact/pad paths also sanitize residual LS/PS to spaces.
#[test]
fn wrap_composer_rows_hard_breaks_on_unicode_line_separators() {
    let width = 40;
    for sep in ['\u{2028}', '\u{2029}'] {
        let chars: Vec<char> = format!("hello{sep}world").chars().collect();
        let wrapped = wrap_composer_rows(&chars, width, 4);
        assert!(
            wrapped.rows.len() >= 2,
            "U+{:04X} must split into ≥2 visual rows, got {}: {:?}",
            sep as u32,
            wrapped.rows.len(),
            wrapped.rows.iter().map(|r| &r.text).collect::<Vec<_>>()
        );
        assert_eq!(wrapped.rows[0].text, "hello", "sep U+{:04X}", sep as u32);
        assert_eq!(wrapped.rows[1].text, "world", "sep U+{:04X}", sep as u32);
        for row in &wrapped.rows {
            assert!(
                !row.text.contains(sep),
                "hard break must not paint into row text: {:?}",
                row.text
            );
        }
        // Full composer_view + pad: every painted row fills the pane.
        let mut view = composer_view(&format!("hello{sep}world"), width, 3, 0);
        pad_composer_lines(&mut view.lines, width);
        assert!(view.lines.len() >= 2);
        for line in &view.lines {
            let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                !flat.contains('\u{2028}') && !flat.contains('\u{2029}'),
                "LS/PS must not survive paint: {flat:?}"
            );
            assert_eq!(
                composer_display_width(&flat),
                width,
                "padded row must fill pane: {flat:?}"
            );
        }
    }
    // LF still behaves the same.
    let lf = wrap_composer_rows(&"hello\nworld".chars().collect::<Vec<_>>(), width, 4);
    assert_eq!(lf.rows[0].text, "hello");
    assert_eq!(lf.rows[1].text, "world");
    // Pad-only path: residual LS becomes a space (compact drafts).
    let row = pad_composer_row("ab\u{2028}cd", 8);
    assert!(!row.contains('\u{2028}'));
    assert_eq!(composer_display_width(&row), 8);
}

fn flatten_line(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn composer_steer_hint_tracks_busy_and_queue_depth() {
    assert_eq!(composer_steer_hint(false, 0), None, "idle → default title");
    assert_eq!(
        composer_steer_hint(true, 0).as_deref(),
        Some("· busy — Enter queues a steer ")
    );
    assert_eq!(
        composer_steer_hint(true, 1).as_deref(),
        Some("· 1 steer queued ")
    );
    assert_eq!(
        composer_steer_hint(true, 3).as_deref(),
        Some("· 3 steers queued ")
    );
}

#[test]
fn composer_intent_distinguishes_drafts_commands_and_high_impact_commands() {
    assert_eq!(composer_intent(""), ComposerIntent::Empty);
    assert_eq!(
        composer_intent(&format!("/self {}discard", " ".repeat(200))),
        ComposerIntent::CriticalCommand
    );
    assert_eq!(
        composer_intent("/self discardxxxxxxxxxxxxxxxx"),
        ComposerIntent::Command
    );
    assert_eq!(
        composer_intent("please delete the stale note"),
        ComposerIntent::Message
    );
    assert_eq!(composer_intent("/status"), ComposerIntent::Command);
    assert_eq!(composer_intent("/clear"), ComposerIntent::CriticalCommand);
    assert_eq!(
        composer_intent("  /self discard  "),
        ComposerIntent::CriticalCommand
    );
    assert_eq!(composer_intent("EXIT"), ComposerIntent::CriticalCommand);
    assert_eq!(composer_intent("/Clear"), ComposerIntent::CriticalCommand);
    assert_eq!(
        composer_intent("/LOOP STOP extra"),
        ComposerIntent::CriticalCommand
    );
    assert_eq!(
        composer_intent(&format!("{} /clear", "word ".repeat(200))),
        ComposerIntent::Message,
        "critical tokens in the body must not paint the review rail"
    );
    assert_eq!(ComposerIntent::Empty.title(46), "");
    assert_eq!(ComposerIntent::Empty.title(70), "");
    assert!(!ComposerIntent::Empty.title(70).contains("Enter sends"));
    assert!(!ComposerIntent::Empty.title(70).contains("Shift+Enter"));
    let src = include_str!("../../../cockpit/src/views/status_view.rs");
    let start = src
        .find("pub fn composer_intent(")
        .expect("composer_intent");
    let body = src[start..]
        .split("pub fn composer_frame_title(")
        .next()
        .expect("intent body");
    assert!(
        !body.contains("to_ascii_lowercase"),
        "composer_intent must not lowercase the whole draft: {body}"
    );
}

#[test]
fn composer_frame_title_borrows_idle_chrome() {
    let idle = composer_frame_title(
        ComposerIntent::Empty,
        80,
        ComposerTitleState {
            steerable: true,
            ..Default::default()
        },
    );
    assert!(
        matches!(idle, Cow::Borrowed(_)),
        "idle title must stay a static borrow, got {idle:?}"
    );
    assert_eq!(idle.as_ref(), ComposerIntent::Empty.title(80));

    let draft = composer_frame_title(
        ComposerIntent::Message,
        80,
        ComposerTitleState {
            steerable: true,
            ..Default::default()
        },
    );
    assert!(matches!(draft, Cow::Borrowed(_)));

    let busy = composer_frame_title(
        ComposerIntent::Empty,
        80,
        ComposerTitleState {
            busy: true,
            steerable: true,
            ..Default::default()
        },
    );
    assert!(
        matches!(busy, Cow::Owned(_)),
        "busy steer hint extends the title"
    );
    assert!(busy.contains("guidance"));
    assert!(busy.contains("busy"));
}

#[test]
fn composer_view_wraps_prompt_until_capacity() {
    let view = composer_view("abcdef", 3, 2, 6);
    assert_eq!(view.lines.len(), 2);
    assert_eq!(line_text(&view.lines[0]), "def");
    assert_eq!(line_text(&view.lines[1]), "");
    assert_eq!(view.cursor_row, 0);
    assert_eq!(view.cursor_col, 3);
}

#[test]
fn composer_view_wraps_at_words_and_respects_pasted_newlines() {
    let view = composer_view("hello gentle\nworld", 8, 3, 18);
    assert_eq!(line_text(&view.lines[0]), "hello ");
    assert_eq!(line_text(&view.lines[1]), "gentle");
    assert_eq!(line_text(&view.lines[2]), "world");
    assert_eq!(view.cursor_row, 2);
    assert_eq!(view.cursor_col, 5);
}

#[test]
fn composer_view_scrolls_prompt_vertically_without_stats_or_horizontal_tail() {
    let input = "word ".repeat(20);
    let view = composer_view(&input, 48, 2, input.chars().count());
    assert_eq!(view.lines.len(), 2);
    assert_eq!(line_text(&view.lines[0]), "word ".repeat(9));
    assert_eq!(line_text(&view.lines[1]), "word ".repeat(2));
    assert_eq!((view.cursor_row, view.cursor_col), (1, 10));
    let home = composer_view(&input, 48, 2, 0);
    assert_eq!(line_text(&home.lines[1]), "word ".repeat(9));
    assert_eq!((home.cursor_row, home.cursor_col), (0, 0));
}

#[test]
fn composer_view_handles_multibyte_prompt_safely() {
    let input = "ធ្វើការ inspect workspace quickly for HTML5 game assets/code.";
    let view = composer_view(input, 24, 1, input.chars().count());
    assert_eq!(view.lines.len(), 1);
    assert!(line_text(&view.lines[0]).width() <= 24);
}

#[test]
fn composer_window_uses_terminal_cells_for_wide_unicode() {
    let input = "界".repeat(20);
    let view = composer_view(&input, 10, 1, 10);
    assert_eq!(line_text(&view.lines[0]), "界".repeat(5));
    assert_eq!(view.cursor_col, 0, "soft boundary belongs to the next row");
}

#[test]
fn composer_unicode_wrap_and_caret_are_cell_bounded_at_every_cursor_and_width() {
    for input in [
        "界🙂abc界🙂".repeat(6),
        "e\u{301}界🙂z\u{301}".repeat(6),
        "👩‍💻界\n\nnext\u{2028}last\u{2029}".repeat(6),
    ] {
        for width in 1..=16 {
            let chars: Vec<_> = input.chars().collect();
            let wrapped = wrap_composer_rows(&chars, width, usize::MAX);
            for row in &wrapped.rows {
                assert_eq!(
                    row.text,
                    chars[row.start..row.end].iter().collect::<String>()
                );
                // Never split a combining/ZWJ cluster at a soft boundary.
                let byte = input
                    .char_indices()
                    .nth(row.start)
                    .map_or(input.len(), |(i, _)| i);
                assert!(
                    byte == input.len() || input.grapheme_indices(true).any(|(i, _)| i == byte)
                );
            }
            for cursor in 0..=chars.len() {
                let mut view = composer_view(&input, width, 3, cursor);
                assert!(usize::from(view.cursor_col) <= width);
                assert!(view.cursor_row < 3);
                if usize::from(view.cursor_col) == width {
                    assert!(view.cursor_row + 1 < 3);
                }
                pad_composer_lines(&mut view.lines, width);
                for line in &view.lines {
                    assert_eq!(line.width(), width, "{width}, {cursor}: {line:?}");
                }
            }
        }
    }
    let view = composer_view("👩‍💻!", 3, 2, 3);
    assert_eq!((view.cursor_row, view.cursor_col), (0, 2));
}

#[test]
fn composer_multiline_unicode_selection_tracks_scrolled_source_offsets() {
    let input = "前\n\n界e\u{301}👩‍💻\u{2028}終\n";
    let chars: Vec<_> = input.chars().collect();
    let wrapped = wrap_composer_rows(&chars, 4, usize::MAX);
    assert_eq!(
        wrapped
            .rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
        ["前", "", "界e\u{301}", "👩‍💻", "終", ""]
    );
    for (cursor, expected) in [
        (0, (0, 0)),
        (1, (0, 2)),
        (2, (1, 0)),
        (3, (2, 0)),
        (4, (2, 2)),
        (6, (3, 0)),
        (9, (3, 2)),
        (10, (4, 0)),
        (12, (5, 0)),
    ] {
        assert_eq!(
            wrapped_cursor(&wrapped.rows, cursor),
            expected,
            "cursor {cursor}"
        );
    }
    let view = composer_view_with_selection(input, 4, 2, 9, Some((3, 9)));
    assert_eq!((view.cursor_row, view.cursor_col), (1, 2));
    let selected: String = view
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .filter(|span| span.style.bg == Some(HUD_BLUE))
        .map(|span| span.content.as_ref())
        .collect();
    assert_eq!(selected, "界e\u{301}👩‍💻");
    // Scalar-index selections inside a cluster paint the whole glyph, not
    // split spans that would change its terminal width or caret position.
    let mut view = composer_view_with_selection("👩‍💻!", 4, 2, 3, Some((1, 2)));
    pad_composer_lines(&mut view.lines, 4);
    assert_eq!(view.lines[0].width(), 4);
    assert!(
        view.lines[0]
            .spans
            .iter()
            .any(|s| s.content == "👩‍💻" && s.style.bg == Some(HUD_BLUE))
    );
}

#[test]
fn composer_height_grows_then_caps_and_reflows_on_resize_and_clear() {
    assert_eq!(composer_height("", 12, 30), 4);
    assert_eq!(composer_height("one\ntwo", 12, 30), 4);
    assert_eq!(composer_height("one\ntwo\n三", 12, 30), 5);
    assert_eq!(composer_height("界".repeat(11).as_str(), 12, 30), 5);
    assert_eq!(composer_height("界".repeat(11).as_str(), 24, 30), 4);
    assert_eq!(composer_height("a\n".repeat(20).as_str(), 12, 30), 7);
    assert_eq!(composer_height("", 12, 30), 4);
    for available in 0..=9 {
        assert!(
            composer_height("界\n".repeat(10).as_str(), 3, available)
                <= available.saturating_sub(1)
        );
    }
    // Match draw.rs's bottom-anchored layout: growth reduces only the
    // transcript above, never shifts the composer bottom down.
    use ratatui::layout::{Constraint, Layout, Rect};
    let frame = Rect::new(2, 3, 40, 30);
    let sections = |input| {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(composer_height(input, frame.width, frame.height - 3)),
        ])
        .spacing(-1)
        .split(frame)
    };
    let small = sections("");
    let large = sections("1\n2\n3\n4\n5\n6");
    assert_eq!(small[2].bottom(), large[2].bottom());
    assert_eq!(small[2].y - large[2].y, 3);
    assert_eq!(small[1].height - large[1].height, 3);
}

#[test]
fn composer_large_unicode_paste_keeps_vertical_rows_and_selection_without_a_tail_window() {
    let input = format!("{}界e\u{301}👩‍💻\nlast", "先頭\n".repeat(1500));
    let cursor = input.chars().count();
    let before = input.clone();
    assert_eq!(composer_height(&input, 14, 30), 7);
    let mut view = composer_view_with_selection(&input, 12, 5, cursor, Some((cursor - 4, cursor)));
    assert!(line_text(&view.lines[0]).contains("long draft"));
    let visible: Vec<_> = view.lines[1..].iter().map(line_text).collect();
    assert_eq!(visible, ["先頭", "先頭", "界e\u{301}👩‍💻", "last"]);
    assert_eq!((view.cursor_row, view.cursor_col), (4, 4));
    assert!(
        view.lines[4]
            .spans
            .iter()
            .any(|s| s.content == "last" && s.style.bg == Some(HUD_BLUE))
    );
    pad_composer_lines(&mut view.lines, 12);
    assert!(view.lines.iter().all(|line| line.width() == 12));
    let home = composer_view(&input, 12, 5, 0);
    assert_eq!((home.cursor_row, home.cursor_col), (1, 0));
    assert!(home.lines[1..].iter().all(|line| line_text(line) == "先頭"));
    assert_eq!(
        input, before,
        "rendering never alters submitted source bytes"
    );
    for (width, height) in [(0, 5), (12, 0), (0, 0)] {
        let view = composer_view(&input, width, height, cursor);
        assert!(view.lines.is_empty());
        assert_eq!((view.cursor_row, view.cursor_col), (0, 0));
    }
}

#[test]
fn corrective_large_ascii_window_matches_full_cell_rows_across_hard_breaks() {
    let input = format!("xxxx\nx\n{}\n", "x".repeat(5000));
    let chars: Vec<char> = input.chars().collect();
    for width in [1, 4, 80] {
        let full = wrap_composer_rows(&chars, width, usize::MAX);
        for cursor in (0..24).chain(input.len() - 24..=input.len()) {
            let window = ascii_composer_window(&input, width, cursor);
            let (row, col) = wrapped_cursor(&full.rows, cursor);
            let (local, local_col) = wrapped_cursor(&window.rows, cursor);
            assert_eq!(col, local_col, "cursor {cursor}, width {width}");
            assert_eq!(full.rows[row].start, window.rows[local].start);
            assert_eq!(full.rows[row].text, window.rows[local].text);
            if row > 0 {
                assert!(local > 0);
                assert_eq!(full.rows[row - 1].start, window.rows[local - 1].start);
            }
            if row + 1 < full.rows.len() {
                assert_eq!(full.rows[row + 1].start, window.rows[local + 1].start);
            }
        }
    }
}

#[test]
fn composer_view_large_ascii_wrap_has_bounded_unchanged_frame_cost() {
    let input = "x".repeat(1_000_001);
    let started = std::time::Instant::now();
    let mut last = None;
    for _ in 0..100 {
        last = Some(composer_view(&input, 80, 2, input.len()));
    }
    let elapsed = started.elapsed();
    let view = last.expect("rendered");
    let text = flatten(&view.lines);
    assert!(text.contains("long draft") && !text.contains("…"), "{text}");
    assert!(
        view.lines
            .iter()
            .all(|line| line_text(line).chars().count() <= 80)
    );
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "100 large-draft frames took {elapsed:?}"
    );
}

#[test]
fn composer_view_large_ascii_selection_keeps_bounded_unchanged_frame_cost() {
    let input = "x".repeat(1_000_001);
    let selection = Some((input.len() - 12, input.len()));
    let started = std::time::Instant::now();
    let mut last = None;
    for _ in 0..100 {
        last = Some(composer_view_with_selection(
            &input,
            80,
            3,
            input.len(),
            selection,
        ));
    }
    let elapsed = started.elapsed();
    let view = last.expect("rendered");
    assert_eq!(
        view.lines
            .iter()
            .flat_map(|line| &line.spans)
            .filter(|span| span.style.bg == Some(HUD_BLUE))
            .map(|span| span.content.chars().count())
            .sum::<usize>(),
        12
    );
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "100 selected large-draft frames took {elapsed:?}"
    );

    let off_window = composer_view_with_selection(&input, 80, 2, input.len(), Some((0, 12)));
    assert!(
        off_window
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .all(|span| span.style.bg != Some(HUD_BLUE)),
        "an off-window selection is counted by the title but not painted onto unrelated tail text"
    );
}

#[test]
fn fixed_zero_working_lines_resolves_known_agents_only() {
    for label in [
        "practice", "spark", "spark-r1", "turbo", "atlas", "gemma", "moa", "deli",
    ] {
        let lines = fixed_zero_working_lines(label)
            .unwrap_or_else(|| panic!("known agent {label} has a zero-working portrait"));
        // Two cached lines: a header naming the agent + a "pending" subline.
        assert_eq!(lines.len(), 2);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header.contains(label), "header names {label}: {header}");
    }
    // An unknown label has no fixed portrait.
    assert!(fixed_zero_working_lines("nonesuch").is_none());
}
