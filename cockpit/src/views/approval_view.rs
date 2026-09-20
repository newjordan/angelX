use crate::{
    approval::Decision,
    glyphs::Glyph,
    hud::{HUD_BLUE, HUD_GOLD, HUD_TEXT, panel_style},
};
use ratatui::{
    crossterm::event::KeyCode,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use std::sync::OnceLock;
use unicode_width::UnicodeWidthStr;

const PROMPT_STYLE: Style = Style::new().fg(HUD_TEXT);
const SCOPE_STYLE: Style = Style::new().fg(HUD_BLUE).add_modifier(Modifier::BOLD);
const CONTROLS_STYLE: Style = Style::new().fg(HUD_GOLD).add_modifier(Modifier::BOLD);
const SCOPE_PREFIX: &str = "scope: ";
const APPROVAL_CONTROLS: &str = "[y] approve   [a] approve all (this turn)   [n] deny";
const MODAL_MIN_WIDTH: u16 = 20;
const MODAL_MAX_WIDTH: u16 = 72;

pub fn modal_area(full: Rect, prompt: &str, scope_label: Option<&str>) -> Rect {
    if full.width == 0 || full.height == 0 {
        return Rect::new(full.x, full.y, 0, 0);
    }

    let max_w = full.width.min(MODAL_MAX_WIDTH);
    let min_w = MODAL_MIN_WIDTH.min(max_w);
    let w = full.width.saturating_sub(8).clamp(min_w, max_w);
    // The block consumes one cell on each side. Widths below three cells have
    // no physical body, but a width-one measurement still keeps the height
    // calculation finite while the returned rectangle remains fully bounded.
    let inner_w = w.saturating_sub(2).max(1);
    let body_rows = approval_body_rows(prompt, scope_label, inner_w);
    let h = body_rows.saturating_add(2).min(full.height);
    Rect {
        x: full.x.saturating_add(full.width.saturating_sub(w) / 2),
        y: full.y.saturating_add(full.height.saturating_sub(h) / 2),
        width: w,
        height: h,
    }
}

fn display_cell_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn wrapped_text_rows(text: &str, width: u16) -> u16 {
    let width = width.max(1);
    let explicit_rows = text.split('\n').count().max(1);
    if text
        .split('\n')
        .all(|line| display_cell_width(line) <= width as usize)
    {
        return explicit_rows.min(u16::MAX as usize) as u16;
    }

    // Text::raw(&str) uses str::lines(), which drops a trailing empty row.
    // Match body() even when an earlier explicit row needs wrapping.
    Paragraph::new(text.split('\n').map(Line::from).collect::<Vec<_>>())
        .wrap(Wrap { trim: false })
        .line_count(width)
        .min(u16::MAX as usize) as u16
}

fn approval_body_rows(prompt: &str, scope_label: Option<&str>, width: u16) -> u16 {
    let mut rows = wrapped_text_rows(prompt, width).saturating_add(1); // prompt + blank
    if let Some(scope) = scope_label {
        let scope = format!("{SCOPE_PREFIX}{scope}");
        rows = rows
            .saturating_add(wrapped_text_rows(&scope, width))
            .saturating_add(1); // scope + blank
    }
    rows.saturating_add(wrapped_text_rows(APPROVAL_CONTROLS, width))
}

pub fn body<'a>(prompt: &'a str, scope_label: Option<&'a str>) -> Vec<Line<'a>> {
    // A ratatui Line does not preserve embedded newlines in a single Span.
    // Keep explicit prompt rows (including trailing blanks) aligned with the
    // same split used by approval_body_rows before wrapping each row.
    let mut lines: Vec<Line<'a>> = prompt
        .split('\n')
        .map(|row| Line::from(Span::styled(row, PROMPT_STYLE)))
        .collect();
    lines.push(approval_blank_line());
    if let Some(scope) = scope_label {
        lines.push(Line::from(vec![
            Span::styled(SCOPE_PREFIX, SCOPE_STYLE),
            Span::styled(scope, SCOPE_STYLE),
        ]));
        lines.push(approval_blank_line());
    }
    lines.push(approval_controls_line());
    lines
}

fn approval_blank_line() -> Line<'static> {
    static BLANK: OnceLock<Line<'static>> = OnceLock::new();
    BLANK.get_or_init(|| Line::from("")).clone()
}

fn approval_controls_line() -> Line<'static> {
    static CONTROLS: OnceLock<Line<'static>> = OnceLock::new();
    CONTROLS
        .get_or_init(|| Line::from(Span::styled(APPROVAL_CONTROLS, CONTROLS_STYLE)))
        .clone()
}

pub fn block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(approval_title())
        .style(panel_style())
}

fn approval_title() -> &'static str {
    static TITLE: OnceLock<String> = OnceLock::new();
    TITLE
        .get_or_init(|| format!(" {} approval required ", Glyph::Approval.token()))
        .as_str()
}

pub fn decision_for_key(code: KeyCode) -> Option<Decision> {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Decision::Approve),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(Decision::ApproveAll),
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Decision::Deny),
        _ => None,
    }
}

pub fn decision_text(decision: Decision) -> &'static str {
    match decision {
        Decision::Approve => "approved",
        Decision::ApproveAll => "approved (all this turn)",
        Decision::Deny => "denied",
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/approval_view__tests.rs"]
mod tests;
