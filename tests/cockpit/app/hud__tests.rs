use super::*;
use ratatui::widgets::Widget;

fn draw_box(buf: &mut Buffer, area: Rect) {
    Block::default().borders(Borders::ALL).render(area, buf);
}

#[test]
fn hud_styles_are_foreground_only() {
    assert_eq!(panel_style().fg, Some(HUD_TEXT));
    assert_eq!(dim_panel_style().fg, Some(HUD_DIM));
    assert_eq!(chrome_style().fg, Some(HUD_TEXT));
    assert_eq!(HUD_VERIFIED, Color::Rgb(99, 241, 169));
    assert_eq!(crate::terminal_art::DMD_PALETTE[7], [99, 241, 169]);
}

#[test]
fn merge_turns_shared_corners_into_junctions() {
    // Two vertically stacked boxes overlapping on row 2 (shared border).
    let top = Rect::new(0, 0, 10, 3);
    let bottom = Rect::new(0, 2, 10, 3);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    draw_box(&mut buf, top);
    draw_box(&mut buf, bottom);
    merge_panel_borders(&mut buf, &[top, bottom]);
    assert_eq!(buf.cell((0, 2)).unwrap().symbol(), "├");
    assert_eq!(buf.cell((9, 2)).unwrap().symbol(), "┤");
    // Corners of the overall bounding box stay corners.
    assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "╭");
    assert_eq!(buf.cell((9, 4)).unwrap().symbol(), "╯");
}

#[test]
fn merge_handles_side_by_side_and_cross_joins() {
    // Left | right sharing column 6, both sharing row 2 with a full-width
    // box below → ┬ on top, ┼ where the shared column meets the shared row.
    let left = Rect::new(0, 0, 7, 3);
    let right = Rect::new(6, 0, 6, 3);
    let below = Rect::new(0, 2, 12, 3);
    let mut buf = Buffer::empty(Rect::new(0, 0, 12, 5));
    draw_box(&mut buf, left);
    draw_box(&mut buf, right);
    draw_box(&mut buf, below);
    merge_panel_borders(&mut buf, &[left, right, below]);
    assert_eq!(buf.cell((6, 0)).unwrap().symbol(), "┬");
    assert_eq!(buf.cell((6, 2)).unwrap().symbol(), "┴");
}

/// Pre-scratch-grid reference implementation (HashMap-based) used to pin
/// the flat-grid rewrite to byte-identical glyph decisions.
fn merge_reference(buf: &mut Buffer, rects: &[Rect]) {
    let mut arms: std::collections::HashMap<(u16, u16), u8> = std::collections::HashMap::new();
    for r in rects {
        if r.width < 2 || r.height < 2 {
            continue;
        }
        let (x0, y0) = (r.x, r.y);
        let (x1, y1) = (r.x + r.width - 1, r.y + r.height - 1);
        for x in x0..=x1 {
            let h = match x {
                x if x == x0 => ARM_R,
                x if x == x1 => ARM_L,
                _ => ARM_L | ARM_R,
            };
            let corner_v = if x == x0 || x == x1 { ARM_D } else { 0 };
            *arms.entry((x, y0)).or_insert(0) |= h | corner_v;
            let corner_v = if x == x0 || x == x1 { ARM_U } else { 0 };
            *arms.entry((x, y1)).or_insert(0) |= h | corner_v;
        }
        for y in y0 + 1..y1 {
            *arms.entry((x0, y)).or_insert(0) |= ARM_U | ARM_D;
            *arms.entry((x1, y)).or_insert(0) |= ARM_U | ARM_D;
        }
    }
    for ((x, y), mask) in arms {
        let Some(glyph) = junction_glyph(mask) else {
            continue;
        };
        let Some(cell) = buf.cell_mut((x, y)) else {
            continue;
        };
        if is_box_char(cell.symbol()) && cell.symbol() != glyph {
            cell.set_symbol(glyph);
        }
    }
}

#[test]
fn merge_matches_reference_implementation() {
    let cases: &[&[Rect]] = &[
        // Overlapping stack + side-by-side + cross joins.
        &[
            Rect::new(0, 0, 7, 3),
            Rect::new(6, 0, 6, 3),
            Rect::new(0, 2, 12, 3),
        ],
        // Adjacent (touching, not overlapping) and offset from origin.
        &[Rect::new(2, 1, 5, 4), Rect::new(7, 1, 5, 4)],
        // Nested rects.
        &[Rect::new(0, 0, 12, 6), Rect::new(2, 1, 6, 4)],
        // Degenerate rects mixed in, plus a rect clipped by the buffer.
        &[
            Rect::new(0, 0, 1, 3),
            Rect::new(0, 3, 8, 1),
            Rect::new(4, 2, 10, 5),
        ],
        // No usable rects at all.
        &[Rect::new(3, 3, 1, 1)],
    ];
    for rects in cases {
        let area = Rect::new(0, 0, 12, 6);
        let mut expected = Buffer::empty(area);
        let mut actual = Buffer::empty(area);
        for r in *rects {
            draw_box(&mut expected, r.intersection(area));
            draw_box(&mut actual, r.intersection(area));
        }
        merge_reference(&mut expected, rects);
        merge_panel_borders(&mut actual, rects);
        assert_eq!(expected, actual, "rects: {rects:?}");
    }
}

#[test]
fn merge_leaves_titles_and_content_alone() {
    let a = Rect::new(0, 0, 8, 3);
    let b = Rect::new(0, 2, 8, 3);
    let mut buf = Buffer::empty(Rect::new(0, 0, 8, 5));
    draw_box(&mut buf, a);
    draw_box(&mut buf, b);
    // A title glyph sitting on the shared border row must survive.
    buf.cell_mut((3, 2)).unwrap().set_symbol("T");
    merge_panel_borders(&mut buf, &[a, b]);
    assert_eq!(buf.cell((3, 2)).unwrap().symbol(), "T");
}
