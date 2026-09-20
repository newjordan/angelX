use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

pub const HUD_BLUE: Color = crate::identity::HUD_BLUE;
pub const HUD_DIM: Color = crate::identity::HUD_DIM;
pub const HUD_TEXT: Color = crate::identity::HUD_TEXT;
/// Hot CRT-phosphor cyan-blue: the agent's live thinking glow.
pub const HUD_PHOSPHOR: Color = crate::identity::HUD_PHOSPHOR;
/// Candlelight gold: the Camelot-noir accent. One flame against the HUD
/// blues — heading sigils, inline code, the knight's tag. Use sparingly.
pub const HUD_GOLD: Color = crate::identity::HUD_GOLD;
/// Warning coral: faults and destructive states only.
pub const HUD_DANGER: Color = crate::identity::HUD_DANGER;
/// Alchemical green: verified success and recovered state only.
pub const HUD_VERIFIED: Color = crate::identity::HUD_VERIFIED;
/// Warning amber: caution, never blame. The quest HUD's danger ladder and
/// the world pane's danger border ride it — it is not a failure colour.
pub const HUD_AMBER: Color = crate::identity::HUD_AMBER;
/// Regal purple: the operator's speaker label in the primary conversation pane.
pub const HUD_PURPLE: Color = crate::identity::HUD_PURPLE;
const PANEL_STYLE: Style = Style::new().fg(HUD_TEXT);
const DIM_PANEL_STYLE: Style = Style::new().fg(HUD_DIM);
const CHROME_STYLE: Style = Style::new().fg(HUD_TEXT);
const BORDER_STYLE: Style = Style::new().fg(HUD_DIM);
const TITLE_STYLE: Style = Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD);

pub fn panel_style() -> Style {
    PANEL_STYLE
}

pub fn dim_panel_style() -> Style {
    DIM_PANEL_STYLE
}

pub fn chrome_style() -> Style {
    CHROME_STYLE
}

/// Circuit node cap that brackets every pane title: `─◇ title ◇─`. The
/// caps ride the border colour so the title reads as a labelled node on the
/// chrome trace rather than text pasted over a line.
pub fn title_cap() -> &'static str {
    if crate::glyphs::ascii_chrome_enabled() {
        "+"
    } else {
        "◇"
    }
}
const TITLE_CAP_STYLE: Style = Style::new().fg(HUD_BLUE);

/// Bracket a non-empty title with the node caps. Empty titles (bare frames)
/// stay empty so a caption-less box keeps a clean top edge.
pub fn node_title<'a>(title: impl Into<Line<'a>>) -> Line<'a> {
    let line: Line<'a> = title.into();
    let blank = line.spans.iter().all(|span| span.content.trim().is_empty());
    if blank {
        return line;
    }
    let mut spans = Vec::with_capacity(line.spans.len() + 2);
    spans.push(Span::styled(title_cap(), TITLE_CAP_STYLE));
    let mut inner = line.spans;
    // Titles arrive space-padded (" shell "); keep exactly one pad each side.
    if let Some(first) = inner.first_mut()
        && !first.content.starts_with(' ')
    {
        first.content = std::borrow::Cow::Owned(format!(" {}", first.content));
    }
    if let Some(last) = inner.last_mut()
        && !last.content.ends_with(' ')
    {
        last.content = std::borrow::Cow::Owned(format!("{} ", last.content));
    }
    spans.extend(inner);
    spans.push(Span::styled(title_cap(), TITLE_CAP_STYLE));
    Line::from(spans).alignment(line.alignment.unwrap_or(ratatui::layout::Alignment::Left))
}

pub fn hud_block<'a, T>(title: T) -> Block<'a>
where
    T: Into<Line<'a>>,
{
    Block::default()
        .borders(Borders::ALL)
        .border_type(if crate::glyphs::ascii_chrome_enabled() {
            BorderType::Plain
        } else {
            BorderType::Rounded
        })
        .style(PANEL_STYLE)
        .border_style(BORDER_STYLE)
        .title_style(TITLE_STYLE)
        .title(node_title(title))
}

pub fn transparent_hud_block<'a, T>(title: T) -> Block<'a>
where
    T: Into<Line<'a>>,
{
    Block::default()
        .borders(Borders::ALL)
        .border_type(if crate::glyphs::ascii_chrome_enabled() {
            BorderType::Plain
        } else {
            BorderType::Rounded
        })
        .style(PANEL_STYLE)
        .border_style(BORDER_STYLE)
        .title_style(TITLE_STYLE)
        .title(node_title(title))
}

// ── Collapsed-border junction repair ────────────────────────────────────────
// The cockpit's panels overlap by one cell (Layout spacing −1) so neighbors
// share a single border line instead of stacking two. Each Block still draws
// plain corners on that shared line; this pass recomputes what every border
// cell *should* be from the union of all panel rects and rewrites mismatched
// box-drawing glyphs into proper T-junctions (├ ┤ ┬ ┴ ┼). Only cells already
// holding a plain box char are touched, so titles, scrollbars and content are
// never disturbed.

const ARM_U: u8 = 1;
const ARM_D: u8 = 2;
const ARM_L: u8 = 4;
const ARM_R: u8 = 8;

fn junction_glyph(mask: u8) -> Option<&'static str> {
    const UD: u8 = ARM_U | ARM_D;
    const LR: u8 = ARM_L | ARM_R;
    Some(if crate::glyphs::ascii_chrome_enabled() {
        match mask {
            m if m == UD | LR => "+",
            m if m == UD | ARM_L => "+",
            m if m == UD | ARM_R => "+",
            m if m == LR | ARM_U => "+",
            m if m == LR | ARM_D => "+",
            m if m == UD => "|",
            m if m == LR => "-",
            m if m == ARM_D | ARM_R => "+",
            m if m == ARM_D | ARM_L => "+",
            m if m == ARM_U | ARM_R => "+",
            m if m == ARM_U | ARM_L => "+",
            _ => return None,
        }
    } else {
        match mask {
            m if m == UD | LR => "┼",
            m if m == UD | ARM_L => "┤",
            m if m == UD | ARM_R => "├",
            m if m == LR | ARM_U => "┴",
            m if m == LR | ARM_D => "┬",
            m if m == UD => "│",
            m if m == LR => "─",
            m if m == ARM_D | ARM_R => "╭",
            m if m == ARM_D | ARM_L => "╮",
            m if m == ARM_U | ARM_R => "╰",
            m if m == ARM_U | ARM_L => "╯",
            _ => return None,
        }
    })
}

fn is_box_char(symbol: &str) -> bool {
    matches!(
        symbol,
        "─" | "│"
            | "┌"
            | "┐"
            | "└"
            | "┘"
            | "╭"
            | "╮"
            | "╰"
            | "╯"
            | "├"
            | "┤"
            | "┬"
            | "┴"
            | "┼"
    )
}

thread_local! {
    // Reusable arm-bit grid so steady-state frames never allocate here.
    static ARMS_SCRATCH: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Repair border junctions where panel boxes share edges. `rects` are the
/// OUTER (border-inclusive) rects of every bordered panel this frame.
pub fn merge_panel_borders(buf: &mut Buffer, rects: &[Rect]) {
    // Bounding box of the border-worthy rects; degenerate rects don't count.
    let (mut bx0, mut by0, mut bx1, mut by1) = (u16::MAX, u16::MAX, 0u16, 0u16);
    for r in rects {
        if r.width < 2 || r.height < 2 {
            continue;
        }
        bx0 = bx0.min(r.x);
        by0 = by0.min(r.y);
        bx1 = bx1.max(r.x + r.width - 1);
        by1 = by1.max(r.y + r.height - 1);
    }
    if bx0 > bx1 || by0 > by1 {
        return;
    }
    let gw = (bx1 - bx0 + 1) as usize;
    ARMS_SCRATCH.with(|scratch| {
        let mut arms = scratch.borrow_mut();
        arms.clear();
        arms.resize(gw * ((by1 - by0 + 1) as usize), 0);
        let idx = |x: u16, y: u16| (y - by0) as usize * gw + (x - bx0) as usize;
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
                arms[idx(x, y0)] |= h | corner_v;
                let corner_v = if x == x0 || x == x1 { ARM_U } else { 0 };
                arms[idx(x, y1)] |= h | corner_v;
            }
            for y in y0 + 1..y1 {
                arms[idx(x0, y)] |= ARM_U | ARM_D;
                arms[idx(x1, y)] |= ARM_U | ARM_D;
            }
        }
        for (i, &mask) in arms.iter().enumerate() {
            if mask == 0 {
                continue;
            }
            let Some(glyph) = junction_glyph(mask) else {
                continue;
            };
            let x = bx0 + (i % gw) as u16;
            let y = by0 + (i / gw) as u16;
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            if is_box_char(cell.symbol()) && cell.symbol() != glyph {
                cell.set_symbol(glyph);
            }
        }
    });
}

#[cfg(test)]
mod tests {
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
}
