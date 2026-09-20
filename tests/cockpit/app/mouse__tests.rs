use super::*;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

fn rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// ---- hit-testing ----

#[test]
fn pane_hit_test_finds_the_pane_under_the_point_and_topmost_wins() {
    let mut reg = PaneRegistry::default();
    reg.push(PaneId::Transcript, rect(0, 0, 10, 10));
    reg.push(PaneId::Artifacts, rect(20, 0, 10, 10));
    // Overlapping pane registered later is "on top".
    reg.push(PaneId::AgentBay, rect(5, 5, 10, 10));

    assert_eq!(reg.pane_at(2, 2).map(|(p, _)| p), Some(PaneId::Transcript));
    assert_eq!(reg.pane_at(22, 2).map(|(p, _)| p), Some(PaneId::Artifacts));
    // (6,6) is in both Transcript and AgentBay → topmost (AgentBay) wins.
    assert_eq!(reg.pane_at(6, 6).map(|(p, _)| p), Some(PaneId::AgentBay));
    // Gap between panes → nothing.
    assert_eq!(reg.pane_at(17, 2), None);
    // Empty rects are never registered.
    reg.push(PaneId::Input, rect(0, 0, 0, 0));
    assert_eq!(reg.pane_at(0, 0).map(|(p, _)| p), Some(PaneId::Transcript));
}

// ---- clamping / "never crosses a pane boundary" ----

#[test]
fn drag_clamps_to_the_pane_rect() {
    let r = rect(10, 5, 8, 4); // cols 10..=17, rows 5..=8
    assert_eq!(clamp_point(r, 100, 100), (17, 8));
    assert_eq!(clamp_point(r, 0, 0), (10, 5));
    assert_eq!(clamp_point(r, 12, 6), (12, 6));
}

#[test]
fn selection_never_escapes_its_pane() {
    let r = rect(10, 5, 8, 4); // cols 10..=17, rows 5..=8
    let mut sel = Selection::new(PaneId::Transcript, r, 12, 6);
    // Drag far past the bottom-right of the pane (and into where another pane
    // would be) — every produced cell must stay inside the pane.
    sel.extend(99, 99);
    for (y, x0, x1) in selection_spans(&sel) {
        assert!((r.y..r.y + r.height).contains(&y), "row {y} left the pane");
        assert!(
            x0 >= r.x && x1 < r.x + r.width,
            "cols {x0}..={x1} left pane"
        );
    }
    // ...and dragging up-left past the top-left corner, too.
    sel.extend(0, 0);
    for (y, x0, x1) in selection_spans(&sel) {
        assert!((r.y..r.y + r.height).contains(&y));
        assert!(x0 >= r.x && x1 < r.x + r.width);
    }
}

#[test]
fn rebind_shrinks_a_stale_full_page_selection_into_the_prose_rect() {
    // Regression: a selection anchored on a huge rect (or after a layout
    // collapse) must not keep reverse-videoing agent/realm chrome.
    let huge = rect(0, 0, 160, 48);
    let mut sel = Selection::new(PaneId::Transcript, huge, 2, 2);
    sel.extend(150, 40);
    let live = rect(2, 4, 70, 30);
    assert!(sel.rebind(live), "overlapping full-page pick must rebind");
    assert_eq!(sel.rect, live);
    for (y, x0, x1) in selection_spans(&sel) {
        assert!((live.y..live.y + live.height).contains(&y), "row {y}");
        assert!(x0 >= live.x && x1 < live.x + live.width, "cols {x0}..={x1}");
    }
    let clip = rect(2, 4, 70, 30);
    for (y, x0, x1) in selection_spans_in(&sel, clip) {
        assert!((clip.y..clip.y + clip.height).contains(&y));
        assert!(x0 >= clip.x && x1 < clip.x + clip.width);
    }
}

#[test]
fn rebind_drops_selection_that_lived_only_in_the_side_column_zone() {
    // A1: full-width transcript → split layout. A single-row pick that sat
    // only in columns claimed by the new agent bay must not pin onto the
    // prose right edge (false highlight / wrong clipboard).
    let full = rect(1, 2, 150, 30); // prose while backdrop off
    let mut sel = Selection::new(PaneId::Transcript, full, 120, 10);
    sel.extend(140, 10);
    let live = rect(1, 2, 90, 30); // narrowed after side column appears
    assert!(
        !sel.rebind(live),
        "side-column-only pick must be dropped, not edge-pinned"
    );
    // Unrelated multi-row pick that crossed the surviving prose stays.
    let mut keep = Selection::new(PaneId::Transcript, full, 10, 8);
    keep.extend(130, 12);
    assert!(
        keep.rebind(live),
        "prose-overlapping pick must survive rebind"
    );
    assert_eq!(keep.rect, live);
}

/// A1 adversarial: `selection_spans_in` must not invent a phantom edge cell
/// when the pick only lived in columns outside `clip` (same layout shift as
/// rebind side-column drop). Clamping endpoints alone used to reverse-video
/// / extract one unrelated glyph on the prose edge.
#[test]
fn selection_spans_in_does_not_invent_edge_cell_for_disjoint_pick() {
    let full = rect(0, 0, 160, 40);
    let mut sel = Selection::new(PaneId::Transcript, full, 120, 10);
    sel.extend(140, 10);
    let live = rect(0, 0, 90, 40);
    assert!(
        selection_spans_in(&sel, live).is_empty(),
        "disjoint pick must yield no clipped spans (no phantom edge cell)"
    );
    // Overlapping pick still produces spans inside the live prose.
    let mut keep = Selection::new(PaneId::Transcript, full, 10, 8);
    keep.extend(130, 12);
    let spans = selection_spans_in(&keep, live);
    assert!(!spans.is_empty(), "overlapping pick must still highlight");
    for (y, x0, x1) in spans {
        assert!((live.y..live.y + live.height).contains(&y), "row {y}");
        assert!(x0 >= live.x && x1 < live.x + live.width, "cols {x0}..={x1}");
    }
}

#[test]
fn rebind_rejects_empty_live_rect() {
    let mut sel = Selection::new(PaneId::Transcript, rect(0, 0, 40, 10), 2, 2);
    sel.extend(8, 4);
    assert!(!sel.rebind(rect(0, 0, 0, 0)));
    assert!(!sel.rebind(rect(5, 5, 10, 0)));
}

/// A1: a finished multi-cell pick that collapses onto one edge cell after a
/// layout shrink must be dropped — otherwise reverse-video sticks on a
/// phantom cell and copy can lift one unrelated glyph.
#[test]
fn rebind_drops_finished_selection_that_collapses_to_one_cell() {
    let tall = rect(0, 0, 80, 40);
    // Vertical strip on the far-right column (x=79). Live prose becomes a
    // single cell at a different (x,y) that still intersects middle full
    // rows of the linear selection, so naive clamp pins both ends there.
    let mut sel = Selection::new(PaneId::Transcript, tall, 79, 5);
    sel.extend(79, 30);
    sel.finish();
    assert!(!sel.is_empty());
    // Live rect is one cell at (10, 15) — middle rows of the old linear
    // selection cover the full width, so cells intersect; clamp collapses.
    let live = rect(10, 15, 1, 1);
    assert!(
        !sel.rebind(live),
        "collapsed finished pick must drop (no stuck white cell)"
    );
}

/// A1: a brand-new click is empty while dragging and must still rebind so
/// the drag can grow inside the live prose.
#[test]
fn rebind_keeps_empty_dragging_click_alive() {
    let wide = rect(0, 0, 100, 20);
    let mut sel = Selection::new(PaneId::Transcript, wide, 10, 5);
    assert!(sel.is_empty() && sel.is_dragging());
    let live = rect(2, 2, 60, 16);
    assert!(
        sel.rebind(live),
        "empty drag start must rebind into live prose"
    );
    assert_eq!(sel.rect, live);
    assert!(sel.is_empty());
}

#[test]
fn rect_of_prefers_the_last_registration() {
    let mut reg = PaneRegistry::default();
    reg.push(PaneId::Transcript, rect(0, 0, 160, 40)); // full-body
    reg.push(PaneId::Transcript, rect(1, 4, 72, 28)); // prose only
    assert_eq!(reg.rect_of(PaneId::Transcript), Some(rect(1, 4, 72, 28)));
}

#[test]
fn linear_selection_spans_full_rows_in_the_middle() {
    let r = rect(0, 0, 20, 10);
    let mut sel = Selection::new(PaneId::Transcript, r, 5, 1);
    sel.extend(8, 3); // anchor (5,1) → cursor (8,3)
    let spans = selection_spans(&sel);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0], (1, 5, 19)); // first row: from anchor col to right edge
    assert_eq!(spans[1], (2, 0, 19)); // middle row: full width
    assert_eq!(spans[2], (3, 0, 8)); // last row: left edge to cursor col
}

#[test]
fn ordering_is_reading_order_regardless_of_drag_direction() {
    let r = rect(0, 0, 20, 10);
    // Drag bottom-right → top-left; ordering must still read top-left first.
    let mut sel = Selection::new(PaneId::Transcript, r, 8, 3);
    sel.extend(5, 1);
    assert_eq!(sel.ordered(), ((5, 1), (8, 3)));
    let cell_selected = |x: u16, y: u16| {
        selection_spans(&sel)
            .iter()
            .any(|(sy, x0, x1)| *sy == y && x >= *x0 && x <= *x1)
    };
    assert!(cell_selected(5, 1)); // the anchor
    assert!(cell_selected(8, 3)); // the cursor
    assert!(!cell_selected(4, 1)); // before the anchor on its row
}

// ---- text extraction from a rendered buffer ----

fn buffer_from(lines: &[&str]) -> Buffer {
    let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    let h = lines.len() as u16;
    let mut buf = Buffer::empty(rect(0, 0, w, h));
    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
                cell.set_symbol(&ch.to_string());
            }
        }
    }
    buf
}

#[test]
fn extract_trims_trailing_spaces_and_joins_rows() {
    let buf = buffer_from(&["hello world   ", "second line   ", "third         "]);
    let r = rect(0, 0, 14, 3);
    let mut sel = Selection::new(PaneId::Transcript, r, 0, 0);
    sel.extend(13, 2); // whole block
    let text = extract_text(&buf, &sel);
    assert_eq!(text, "hello world\nsecond line\nthird");
}

/// A1: zero-width (click without drag) must not copy a single glyph.
#[test]
fn extract_text_returns_empty_for_zero_width_selection() {
    let buf = buffer_from(&["abcdef"]);
    let r = rect(0, 0, 6, 1);
    let sel = Selection::new(PaneId::Transcript, r, 2, 0);
    assert!(sel.is_empty());
    assert_eq!(extract_text(&buf, &sel), "");
    // Even extract_text_in with a wide clip stays empty.
    assert_eq!(extract_text_in(&buf, &sel, r), "");
}

/// A1: zero-width picks must not reverse-video a single cell either
/// (highlight used to paint one glyph while extract copied nothing).
#[test]
fn highlight_skips_zero_width_selection() {
    let mut buf = buffer_from(&["abcdef"]);
    let r = rect(0, 0, 6, 1);
    let sel = Selection::new(PaneId::Transcript, r, 2, 0);
    assert!(sel.is_empty());
    assert!(
        selection_spans_in(&sel, r).is_empty(),
        "empty pick must yield no highlight spans"
    );
    highlight_in(&mut buf, &sel, r);
    // No cell should carry REVERSED after a zero-width highlight pass.
    for x in 0..6u16 {
        assert!(
            !buf.cell((x, 0))
                .unwrap()
                .modifier
                .contains(Modifier::REVERSED),
            "x={x}: zero-width must not reverse-video"
        );
    }
    // Multi-cell pick still paints.
    let mut wide = Selection::new(PaneId::Transcript, r, 1, 0);
    wide.extend(3, 0);
    highlight_in(&mut buf, &wide, r);
    assert!(
        buf.cell((2, 0))
            .unwrap()
            .modifier
            .contains(Modifier::REVERSED)
    );
}

/// A1: extract_text_in must not invent edge text when the pick is disjoint
/// from the live clip (same layout shift as selection_spans_in).
#[test]
fn extract_text_in_drops_disjoint_clip() {
    let buf = buffer_from(&["LEFTSIDE RIGHTSIDE"]);
    let full = rect(0, 0, 18, 1);
    let mut sel = Selection::new(PaneId::Transcript, full, 10, 0);
    sel.extend(17, 0); // only "RIGHTSIDE"
    let left = rect(0, 0, 8, 1);
    assert_eq!(
        extract_text_in(&buf, &sel, left),
        "",
        "disjoint clip must not copy a phantom edge glyph"
    );
    // Overlapping clip still extracts the visible slice.
    let mid = rect(8, 0, 10, 1);
    let got = extract_text_in(&buf, &sel, mid);
    assert!(
        got.contains('R') || got.contains('I'),
        "overlapping clip should still yield prose, got {got:?}"
    );
}

#[test]
fn extract_is_confined_to_the_pane_columns() {
    // Two "panes" worth of content on each row; a selection over the left
    // pane must not pick up the right pane's text (chrome-free guarantee).
    let buf = buffer_from(&["LEFT  | RIGHT", "left2 | right"]);
    let left = rect(0, 0, 5, 2); // cols 0..=4 only
    let mut sel = Selection::new(PaneId::Transcript, left, 0, 0);
    sel.extend(99, 99); // drag across into the right pane's columns
    let text = extract_text(&buf, &sel);
    assert_eq!(text, "LEFT\nleft2");
    assert!(!text.contains("RIGHT"));
}

#[test]
fn extract_skips_wide_grapheme_continuations_but_keeps_real_spaces() {
    let area = rect(0, 0, 14, 2);
    let mut buf = Buffer::empty(area);
    Paragraph::new("前 A 🙂 B\n終  real  ").render(area, &mut buf);

    // Exercise Ratatui's real representation: each width-2 lead cell is
    // followed by a cleared space cell, indistinguishable by symbol alone
    // from the intentional space after it.
    assert_eq!(buf.cell((1, 0)).unwrap().symbol(), " ");
    assert_eq!(buf.cell((6, 0)).unwrap().symbol(), " ");
    assert_eq!(buf.cell((1, 1)).unwrap().symbol(), " ");

    let mut sel = Selection::new(PaneId::Transcript, area, 0, 0);
    sel.extend(area.right() - 1, area.bottom() - 1);
    assert_eq!(extract_text(&buf, &sel), "前 A 🙂 B\n終  real");
}

/// A1: a pick that *starts* on a wide-glyph trailing cell must not invent a
/// leading space in the clipboard (continuation pads look like real spaces).
#[test]
fn extract_skips_selection_start_on_wide_grapheme_continuation() {
    let area = rect(0, 0, 10, 1);
    let mut buf = Buffer::empty(area);
    Paragraph::new("前xyz").render(area, &mut buf);
    // "前" is width 2: lead at col 0, cleared continuation at col 1.
    assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "前");
    assert_eq!(buf.cell((1, 0)).unwrap().symbol(), " ");
    assert_eq!(buf.cell((2, 0)).unwrap().symbol(), "x");

    // Anchor/cursor only on the continuation pad + following ASCII.
    let mut sel = Selection::new(PaneId::Transcript, area, 1, 0);
    sel.extend(4, 0); // cols 1..=4: pad + "xyz"
    assert!(
        !sel.is_empty(),
        "multi-cell pick including continuation must not be empty"
    );
    let text = extract_text(&buf, &sel);
    assert_eq!(
        text, "xyz",
        "must not copy a phantom leading space from the wide pad: {text:?}"
    );
    assert!(
        !text.starts_with(' '),
        "continuation pad must not prefix clipboard: {text:?}"
    );

    // Full-row still includes the CJK lead (no double space after it).
    let mut full = Selection::new(PaneId::Transcript, area, 0, 0);
    full.extend(4, 0);
    assert_eq!(extract_text(&buf, &full), "前xyz");
}

#[test]
fn highlight_marks_only_selected_cells_reversed() {
    let mut buf = buffer_from(&["abcd", "efgh"]);
    let r = rect(0, 0, 4, 2);
    let mut sel = Selection::new(PaneId::Transcript, r, 1, 0);
    sel.extend(2, 0); // cells (1,0)..=(2,0) on the first row
    highlight(&mut buf, &sel);
    let reversed = |x: u16, y: u16| {
        buf.cell((x, y))
            .unwrap()
            .modifier
            .contains(Modifier::REVERSED)
    };
    assert!(reversed(1, 0) && reversed(2, 0));
    assert!(!reversed(0, 0) && !reversed(3, 0));
    assert!(!reversed(1, 1)); // untouched row
}

// ---- OSC-52 clipboard payload ----

#[test]
fn osc52_payload_is_base64_wrapped_in_the_escape() {
    // "hi" → base64 "aGk=".
    assert_eq!(base64_clipboard("hi"), "aGk=");
    assert_eq!(osc52_sequence("hi"), "\x1b]52;c;aGk=\x07");
    // Round-trips an arbitrary payload.
    let payload = "line1\nline2 with spaces\t€";
    let seq = osc52_sequence(payload);
    assert!(seq.starts_with("\x1b]52;c;") && seq.ends_with('\x07'));
    let b64 = &seq["\x1b]52;c;".len()..seq.len() - 1];
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .unwrap();
    assert_eq!(String::from_utf8(decoded).unwrap(), payload);
}

#[test]
fn osc52_limit_applies_to_the_encoded_payload() {
    let largest_raw_payload = (OSC52_MAX_PAYLOAD_BYTES / 4) * 3;
    let fits = "x".repeat(largest_raw_payload);
    let exceeds = "x".repeat(largest_raw_payload + 1);

    assert_eq!(
        base64_clipboard_len(&fits),
        Some(OSC52_MAX_PAYLOAD_BYTES - 2)
    );
    assert!(osc52_payload_fits(&fits));
    assert_eq!(
        base64_clipboard_len(&exceeds),
        Some(OSC52_MAX_PAYLOAD_BYTES + 2)
    );
    assert!(!osc52_payload_fits(&exceeds));
}

// ---- PTY mouse forwarding ----

fn ev(kind: MouseEventKind, col: u16, row: u16, mods: KeyModifiers) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: mods,
    }
}

#[test]
fn shift_drag_overrides_a_mouse_aware_shell_until_release() {
    let area = rect(10, 5, 20, 10);
    let shifted_down = ev(
        MouseEventKind::Down(MouseButton::Left),
        12,
        7,
        KeyModifiers::SHIFT,
    );
    assert!(selection_overrides_pty_mouse(&shifted_down, None));

    let ordinary_down = ev(
        MouseEventKind::Down(MouseButton::Left),
        12,
        7,
        KeyModifiers::NONE,
    );
    assert!(!selection_overrides_pty_mouse(&ordinary_down, None));

    let mut shell_selection = Selection::new(PaneId::Shell, area, 12, 7);
    let unmodified_drag = ev(
        MouseEventKind::Drag(MouseButton::Left),
        15,
        8,
        KeyModifiers::NONE,
    );
    assert!(selection_overrides_pty_mouse(
        &unmodified_drag,
        Some(&shell_selection)
    ));

    shell_selection.finish();
    assert!(!selection_overrides_pty_mouse(
        &unmodified_drag,
        Some(&shell_selection)
    ));
}

#[test]
fn sgr_press_encodes_pane_relative_1based_coords() {
    // Shell pane at (10,5); a left-press at absolute (12,7) → pane-relative
    // (3,3), 1-based, button 0, press → ESC [ < 0 ; 3 ; 3 M
    let r = rect(10, 5, 20, 10);
    let bytes = pty_mouse_bytes(
        &ev(
            MouseEventKind::Down(MouseButton::Left),
            12,
            7,
            KeyModifiers::NONE,
        ),
        TrackMode::PressRelease,
        ReportEncoding::Sgr,
        r,
    )
    .unwrap();
    assert_eq!(bytes, b"\x1b[<0;3;3M");
}

#[test]
fn sgr_release_uses_lowercase_m_and_drag_sets_motion_bit() {
    let r = rect(0, 0, 20, 10);
    let up = pty_mouse_bytes(
        &ev(
            MouseEventKind::Up(MouseButton::Left),
            4,
            2,
            KeyModifiers::NONE,
        ),
        TrackMode::PressRelease,
        ReportEncoding::Sgr,
        r,
    )
    .unwrap();
    assert_eq!(up, b"\x1b[<0;5;3m"); // lowercase m on release

    let drag = pty_mouse_bytes(
        &ev(
            MouseEventKind::Drag(MouseButton::Left),
            4,
            2,
            KeyModifiers::NONE,
        ),
        TrackMode::ButtonMotion,
        ReportEncoding::Sgr,
        r,
    )
    .unwrap();
    assert_eq!(drag, b"\x1b[<32;5;3M"); // 0 + 32 motion bit
}

#[test]
fn sgr_wheel_and_modifiers_encode() {
    let r = rect(0, 0, 20, 10);
    let wheel = pty_mouse_bytes(
        &ev(MouseEventKind::ScrollUp, 0, 0, KeyModifiers::NONE),
        TrackMode::PressRelease,
        ReportEncoding::Sgr,
        r,
    )
    .unwrap();
    assert_eq!(wheel, b"\x1b[<64;1;1M");
    // ctrl+left-press → button 0 + ctrl(16) = 16
    let ctrl = pty_mouse_bytes(
        &ev(
            MouseEventKind::Down(MouseButton::Left),
            0,
            0,
            KeyModifiers::CONTROL,
        ),
        TrackMode::Press,
        ReportEncoding::Sgr,
        r,
    )
    .unwrap();
    assert_eq!(ctrl, b"\x1b[<16;1;1M");
}

#[test]
fn x10_encoding_offsets_by_32() {
    let r = rect(0, 0, 20, 10);
    // left-press at pane-relative (1,1) → bytes ESC [ M ' ' '!' '!'
    let bytes = pty_mouse_bytes(
        &ev(
            MouseEventKind::Down(MouseButton::Left),
            0,
            0,
            KeyModifiers::NONE,
        ),
        TrackMode::Press,
        ReportEncoding::X10,
        r,
    )
    .unwrap();
    assert_eq!(bytes, vec![0x1b, b'[', b'M', 32, 33, 33]);
}

#[test]
fn mode_gating_matches_xterm() {
    let r = rect(0, 0, 20, 10);
    let mk = |kind, mode| {
        pty_mouse_bytes(
            &ev(kind, 0, 0, KeyModifiers::NONE),
            mode,
            ReportEncoding::Sgr,
            r,
        )
    };
    // None: nothing is ever forwarded (app handles selection instead).
    assert!(mk(MouseEventKind::Down(MouseButton::Left), TrackMode::None).is_none());
    // Press mode: press yes, release no, drag no.
    assert!(mk(MouseEventKind::Down(MouseButton::Left), TrackMode::Press).is_some());
    assert!(mk(MouseEventKind::Up(MouseButton::Left), TrackMode::Press).is_none());
    assert!(mk(MouseEventKind::Drag(MouseButton::Left), TrackMode::Press).is_none());
    // PressRelease: release yes, drag no.
    assert!(
        mk(
            MouseEventKind::Up(MouseButton::Left),
            TrackMode::PressRelease
        )
        .is_some()
    );
    assert!(
        mk(
            MouseEventKind::Drag(MouseButton::Left),
            TrackMode::PressRelease
        )
        .is_none()
    );
    // ButtonMotion: drag yes, bare move no.
    assert!(
        mk(
            MouseEventKind::Drag(MouseButton::Left),
            TrackMode::ButtonMotion
        )
        .is_some()
    );
    assert!(mk(MouseEventKind::Moved, TrackMode::ButtonMotion).is_none());
    // AnyMotion: bare move yes.
    assert!(mk(MouseEventKind::Moved, TrackMode::AnyMotion).is_some());
}
