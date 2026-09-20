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
#[path = "../../tests/cockpit/app/hud__tests.rs"]
mod tests;
