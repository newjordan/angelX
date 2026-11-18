//! The approval ask: one card, the same in the window and in a terminal.
//!
//! The prompt leads with its first line as a headline and wraps by words
//! (bullets hang under their words), the trusted scope sits on its own
//! labelled row, and the three answers stand side by side with the chosen
//! one under the pill. Keys answer at once (y / a / n, Esc denies);
//! ← → or Tab move the choice and Enter takes it; a click answers too.
//! In the window the card pops in with the squire on guard; in a terminal
//! it is the setup card's black box with a hairline round it.

use super::setup_view::{ACCENT, BRIGHT, INSET, SOFT, TEXT};
use crate::approval::Decision;
use ratatui::{
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(crate) const TITLE: &str = "Approval needed";
pub(crate) const ANSWERS: [Decision; 3] = [Decision::Approve, Decision::ApproveAll, Decision::Deny];
const SCOPE_LABEL: &str = "SCOPE";
const MAX_COLS: u16 = 76;
const MIN_COLS: u16 = 28;
const MOST_PROMPT_ROWS: usize = 12;
const BOX_TOP_ROWS: u16 = 3;
const PICK_BG: Color = Color::Rgb(30, 33, 40);
const ANSWER_GAP: u16 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyOutcome {
    Answer(Decision),
    Focus(usize),
    Ignore,
}

pub(crate) fn key_outcome(code: KeyCode, focus: usize) -> KeyOutcome {
    let focus = focus.min(ANSWERS.len() - 1);
    if let Some(decision) = decision_for_key(code) {
        return KeyOutcome::Answer(decision);
    }
    match code {
        KeyCode::Enter | KeyCode::Char(' ') => KeyOutcome::Answer(ANSWERS[focus]),
        KeyCode::Left | KeyCode::Up => KeyOutcome::Focus(focus.saturating_sub(1)),
        KeyCode::Right | KeyCode::Down => KeyOutcome::Focus((focus + 1).min(ANSWERS.len() - 1)),
        KeyCode::Tab => KeyOutcome::Focus((focus + 1) % ANSWERS.len()),
        KeyCode::BackTab => KeyOutcome::Focus((focus + ANSWERS.len() - 1) % ANSWERS.len()),
        _ => KeyOutcome::Ignore,
    }
}

pub fn decision_for_key(code: KeyCode) -> Option<Decision> {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => Some(Decision::Approve),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(Decision::ApproveAll),
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Decision::Deny),
        _ => None,
    }
}

pub fn decision_text(decision: Decision) -> &'static str {
    match decision {
        Decision::Approve => "approved",
        Decision::ApproveAll => "approved (this scope, rest of turn)",
        Decision::Deny => "denied",
    }
}

pub(crate) fn decision_token(decision: Decision) -> &'static str {
    match decision {
        Decision::Approve => "approve",
        Decision::ApproveAll => "approve_all",
        Decision::Deny => "deny",
    }
}

pub(crate) fn serial(prompt: &str, scope_label: Option<&str>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prompt.hash(&mut hasher);
    scope_label.hash(&mut hasher);
    hasher.finish()
}

fn answer_words(decision: Decision, scoped: bool) -> (&'static str, &'static str) {
    match decision {
        Decision::Approve => ("y", "Approve"),
        Decision::ApproveAll if scoped => ("a", "Allow this scope for the turn"),
        Decision::ApproveAll => ("a", "Approve all this turn"),
        Decision::Deny => ("n", "Deny"),
    }
}

pub(crate) struct Composed {
    pub(crate) buffer: Buffer,
    pub(crate) cursor: Option<u16>,
    pub(crate) cursor_cols: Option<(u16, u16)>,
    pub(crate) hits: Vec<(Rect, Decision)>,
}

pub(crate) fn compose(
    prompt: &str,
    scope_label: Option<&str>,
    focus: usize,
    width: u16,
    most_rows: u16,
    top: u16,
    glass: bool,
) -> Composed {
    let focus = focus.min(ANSWERS.len() - 1);
    let right = width.saturating_sub(INSET);
    let room = usize::from(right.saturating_sub(INSET)).max(1);
    let scoped = scope_label.is_some();
    let answers: Vec<(&str, &str, Decision)> = ANSWERS
        .iter()
        .map(|&d| {
            let (key, words) = answer_words(d, scoped);
            (key, words, d)
        })
        .collect();
    let answer_cols = |(key, words, _): &(&str, &str, Decision)| (key.width() + 2 + words.width()) as u16;
    let side_by_side_cols = answers.iter().map(answer_cols).sum::<u16>() + ANSWER_GAP * (answers.len() as u16 - 1);
    let side_by_side = usize::from(side_by_side_cols) <= room;
    let answer_rows = if side_by_side { 1 } else { answers.len() as u16 };
    let keys: &[(&str, &str)] = &[("← →", "choose"), ("Enter", "answer"), ("Esc", "deny")];
    let keys_cols = keys.iter().map(|(k, w)| k.width() + 1 + w.width()).sum::<usize>() + 3 * (keys.len() - 1);

    let scope_width = room.saturating_sub(SCOPE_LABEL.len() + 2).max(1);
    let mut scope_rows: Vec<String> = scope_label.map(|scope| wrap(scope, scope_width)).unwrap_or_default();
    let spare = most_rows.saturating_sub(top.saturating_add(1 + answer_rows + 1 + 1));
    let spare = if scope_rows.is_empty() {
        spare
    } else {
        match usize::from(spare.saturating_sub(1)).min(scope_rows.len()) {
            0 => {
                scope_rows.clear();
                spare
            }
            kept => {
                if kept < scope_rows.len() {
                    scope_rows.truncate(kept);
                    if let Some(last) = scope_rows.last_mut() {
                        while !last.is_empty() && last.width() + 1 > scope_width {
                            last.pop();
                        }
                        last.push('…');
                    }
                }
                spare - 1 - kept as u16
            }
        }
    };
    let show_keys = keys_cols <= room && spare >= 2;
    let fixed = top
        + if scope_rows.is_empty() { 0 } else { 1 + scope_rows.len() as u16 }
        + 1
        + answer_rows
        + if show_keys { 2 } else { 0 }
        + 1;
    let prompt_room = usize::from(most_rows.saturating_sub(fixed)).clamp(1, MOST_PROMPT_ROWS);
    let prompt_rows = prompt_rows(prompt, room, prompt_room);

    let height = (fixed + prompt_rows.len() as u16).min(most_rows.max(top + 2));
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
    let fits = |y: u16| y + 1 < height;
    let mut y = top;
    for (row, ink) in &prompt_rows {
        if !fits(y) {
            break;
        }
        buffer.set_stringn(INSET, y, row, room, *ink);
        y += 1;
    }
    if !scope_rows.is_empty() {
        y += 1;
        let x = INSET + SCOPE_LABEL.len() as u16 + 2;
        if fits(y) {
            buffer.set_stringn(INSET, y, SCOPE_LABEL, room, Style::new().fg(SOFT));
        }
        for row in &scope_rows {
            if !fits(y) {
                break;
            }
            buffer.set_stringn(x, y, row, usize::from(right.saturating_sub(x)), Style::new().fg(TEXT));
            y += 1;
        }
    }
    y += 1;

    let mut hits = Vec::new();
    let mut cursor = None;
    let mut cursor_cols = None;
    let mut x = if side_by_side { INSET + (room as u16).saturating_sub(side_by_side_cols) / 2 } else { INSET };
    for (i, answer) in answers.iter().enumerate() {
        if !fits(y) {
            break;
        }
        let (key, words, decision) = *answer;
        let chosen = i == focus;
        let cols = answer_cols(answer);
        if chosen && !glass {
            let (from, span) = if side_by_side { (x.saturating_sub(1), cols + 2) } else { (INSET - 1, room as u16 + 2) };
            for cx in from..(from + span).min(width) {
                if let Some(cell) = buffer.cell_mut((cx, y)) {
                    cell.set_bg(PICK_BG);
                }
            }
        }
        let shade = |style: Style| if chosen && !glass { style.bg(PICK_BG) } else { style };
        let after_key = buffer.set_stringn(x, y, key, 1, shade(Style::new().fg(ACCENT).add_modifier(Modifier::BOLD))).0;
        let ink = if chosen {
            Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(TEXT)
        };
        buffer.set_stringn(after_key + 2, y, words, usize::from(right.saturating_sub(after_key + 2)), shade(ink));
        let hit = if side_by_side {
            Rect::new(x.saturating_sub(1), y, cols + 2, 1)
        } else {
            Rect::new(INSET - 1, y, width.saturating_sub(2 * INSET) + 2, 1)
        };
        hits.push((hit, decision));
        if chosen {
            cursor = Some(y);
            cursor_cols = side_by_side.then_some((x.saturating_sub(1), cols + 2));
        }
        if side_by_side {
            x += cols + ANSWER_GAP;
        } else {
            y += 1;
        }
    }
    if side_by_side {
        y += 1;
    }

    if show_keys && fits(y + 1) {
        y += 1;
        let words_cols = keys_cols - 3 * (keys.len() - 1);
        let gap = (room.saturating_sub(words_cols) / (keys.len() - 1)).min(5);
        let spread = words_cols + gap * (keys.len() - 1);
        let mut x = INSET + (room.saturating_sub(spread) / 2) as u16;
        for (key, word) in keys {
            x = buffer.set_stringn(x, y, key, usize::from(right.saturating_sub(x)), Style::new().fg(ACCENT)).0;
            x = buffer
                .set_stringn(x, y, format!(" {word}{}", " ".repeat(gap)), usize::from(right.saturating_sub(x)), Style::new().fg(SOFT))
                .0;
        }
    }
    Composed { buffer, cursor, cursor_cols, hits }
}

fn prompt_rows(prompt: &str, room: usize, most: usize) -> Vec<(String, Style)> {
    let headline = Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD);
    let body = Style::new().fg(TEXT);
    let mut rows: Vec<(String, Style)> = Vec::new();
    let mut led = false;
    let mut blank = false;
    for line in prompt.split('\n') {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank = led;
            continue;
        }
        if blank {
            rows.push((String::new(), body));
            blank = false;
        }
        let ink = if led { body } else { headline };
        led = true;
        rows.extend(wrap(line, room).into_iter().map(|row| (row, ink)));
    }
    if rows.is_empty() {
        rows.push((String::new(), body));
    }
    if rows.len() > most {
        rows.truncate(most.max(1));
        if let Some((last, _)) = rows.last_mut() {
            while !last.is_empty() && last.width() + 1 > room {
                last.pop();
            }
            last.push('…');
        }
    }
    rows
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let words = text.trim_start_matches(' ');
    let lead = text.len() - words.len();
    let mark = ["• ", "- ", "* ", "· "]
        .iter()
        .find(|m| words.starts_with(**m))
        .map(|m| m.width())
        .or_else(|| {
            words
                .split_once(". ")
                .filter(|(n, _)| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()))
                .map(|(n, _)| n.len() + 2)
        })
        .unwrap_or(0);
    let hang = (lead + mark).min(width / 2);
    let mut lines: Vec<String> = Vec::new();
    let mut line = " ".repeat(lead.min(width / 2));
    let mut line_cols = line.width();
    let mut fresh = true;
    for word in words.split(' ').filter(|w| !w.is_empty()) {
        let cols = word.width();
        if !fresh && line_cols + 1 + cols > width {
            lines.push(std::mem::take(&mut line));
            line = " ".repeat(hang);
            line_cols = hang;
            fresh = true;
        }
        if !fresh {
            line.push(' ');
            line_cols += 1;
        }
        for ch in word.chars() {
            let w = ch.width().unwrap_or(0);
            if line_cols + w > width && line_cols > hang {
                lines.push(std::mem::take(&mut line));
                line = " ".repeat(hang);
                line_cols = hang;
            }
            line.push(ch);
            line_cols += w;
        }
        fresh = false;
    }
    lines.push(line);
    lines
}

fn card_width(root: Rect) -> u16 {
    root.width.saturating_sub(8).min(MAX_COLS)
}

pub(crate) fn terminal_area(root: Rect, prompt: &str, scope_label: Option<&str>) -> Rect {
    boxed(root, prompt, scope_label).unwrap_or(root)
}

fn boxed(root: Rect, prompt: &str, scope_label: Option<&str>) -> Option<Rect> {
    if !roomy(root) {
        return None;
    }
    let width = card_width(root);
    let composed = compose(prompt, scope_label, 0, width, root.height.saturating_sub(2), BOX_TOP_ROWS, false);
    if composed.hits.len() < ANSWERS.len() {
        return None;
    }
    let height = composed.buffer.area.height;
    Some(Rect::new(
        root.x + root.width.saturating_sub(width) / 2,
        root.y + root.height.saturating_sub(height) / 2,
        width,
        height.min(root.height),
    ))
}

fn roomy(root: Rect) -> bool {
    root.width >= MIN_COLS + 8 && root.height >= 10
}

pub(crate) fn paint_terminal(
    buf: &mut Buffer,
    root: Rect,
    prompt: &str,
    scope_label: Option<&str>,
    focus: usize,
) -> Vec<(Rect, Decision)> {
    use super::setup_view::{offset, paint, paint_box};
    use ratatui::widgets::{Clear, Widget};
    if root.width == 0 || root.height == 0 {
        return Vec::new();
    }
    let Some(area) = boxed(root, prompt, scope_label) else {
        let composed = compose(prompt, scope_label, focus, root.width, root.height, 1, false);
        Clear.render(root, buf);
        paint(buf, root, &composed.buffer);
        buf.set_stringn(
            root.x + INSET.min(root.width.saturating_sub(1)),
            root.y,
            TITLE,
            usize::from(root.width.saturating_sub(INSET)),
            Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD),
        );
        return offset(composed.hits, root);
    };
    let composed = compose(prompt, scope_label, focus, area.width, root.height.saturating_sub(2), BOX_TOP_ROWS, false);
    paint_box(buf, area, TITLE, &composed.buffer);
    offset(composed.hits, area)
}

#[cfg(feature = "glass")]
pub(crate) fn card(
    root: Rect,
    prompt: &str,
    scope_label: Option<&str>,
    focus: usize,
) -> Option<(angel_glass::card::Card, Vec<(Rect, Decision)>)> {
    use angel_glass::card::{Card, Entrance, Knight, TITLE_ROWS};
    let width = card_width(root);
    if width < MIN_COLS || root.height < TITLE_ROWS + 8 {
        return None;
    }
    let composed = compose(prompt, scope_label, focus, width, root.height.saturating_sub(6), TITLE_ROWS, true);
    if composed.hits.len() < ANSWERS.len() {
        return None;
    }
    let height = composed.buffer.area.height;
    let area = Rect::new(
        root.x + root.width.saturating_sub(width) / 2,
        (root.y + root.height.saturating_sub(height) / 2).max(root.y + 3.min(root.height)),
        width,
        height,
    );
    if area.bottom() > root.bottom() {
        return None;
    }
    let mut card = Card::new(area, composed.buffer)?;
    card.title = TITLE.into();
    card.serial = serial(prompt, scope_label);
    card.cursor = composed.cursor;
    card.cursor_cols = composed.cursor_cols;
    card.knight = Knight::Guard;
    card.entrance = Entrance::Pop;
    Some((card, super::setup_view::offset(composed.hits, area)))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/approval_view__tests.rs"]
mod tests;
