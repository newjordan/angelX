//! `/loop` as a card. In the window it is the clear glass card
//! (`angel_glass::card`) with the knight racers' own hell track playing in
//! it: the knight on the grid with the flag waving, the objective called
//! from a crag at a far keep, and three pickers (laps, clock, fuel) over
//! DROP THE FLAG. The squire rides its roof. Dropping the flag plays the
//! start, and the loop then docks as a race ticker in the subheader under
//! the shell's header until it ends. In a
//! terminal it is the same page in a hairline box. One composer lays out
//! both; the dialog's state is `drive::loop_dialog`, the glue to the
//! cockpit `ui::views::loop_view`.

use crate::drive::loop_dialog::{
    LoopDialogAction, LoopDialogFocus, LoopDialogHit, LoopLaunchDialog, Picker,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Widget},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Tonal inks, the setup and approval cards' own: no white, one warm accent.
const TEXT: Color = Color::Rgb(176, 180, 188);
const SOFT: Color = Color::Rgb(118, 123, 132);
const FAINT: Color = Color::Rgb(84, 88, 98);
const BRIGHT: Color = Color::Rgb(224, 226, 230);
const ACCENT: Color = Color::Rgb(196, 134, 92);
const EDGE: Color = Color::Rgb(62, 66, 74);
/// The chosen picker in a terminal, where no glass pill glides under it.
const PICK_BG: Color = Color::Rgb(30, 33, 40);

/// The card's widest; wider windows centre it.
const MAX_COLS: u16 = 86;
/// Three pickers side by side need this much.
const MIN_COLS: u16 = 60;
/// Words sit this far in from the card's sides.
const INSET: u16 = 3;
/// The race film's rows at most, and the fewest worth showing.
const FILM_ROWS: u16 = 8;
const FILM_MIN_ROWS: u16 = 5;
/// The goal shot's rows and columns.
const GOAL_ROWS: u16 = 6;
const GOAL_COLS: u16 = 28;
/// A picker's value sits between its arrows in this many columns.
const VALUE_COLS: u16 = 9;

const START: &str = "DROP THE FLAG";
const CANCEL: &str = "cancel";

// --- the composer -------------------------------------------------------------

/// How much of the page there is room for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) width: u16,
    /// The race film's rows (0: none).
    pub(crate) film_rows: u16,
    /// The goal shot's rows (0: the objective in words alone).
    pub(crate) goal_rows: u16,
}

impl Plan {
    /// The card's rows, `title_rows` above its words.
    pub(crate) fn height(&self, title_rows: u16) -> u16 {
        let film = if self.film_rows > 0 {
            self.film_rows + 1
        } else {
            0
        };
        let goal = if self.goal_rows > 0 {
            self.goal_rows
        } else {
            2
        };
        // goal, gap, three picker rows, gap, the actions, gap, the keys, a margin
        title_rows + film + goal + 1 + 3 + 1 + 1 + 1 + 1 + 1
    }
}

/// The plan that fits `root`: in the window the race film and the goal
/// shot, shrinking the film before dropping pictures; in a terminal words.
pub(crate) fn plan(root: Rect, title_rows: u16, pictures: bool, margin: u16) -> Option<Plan> {
    let width = root.width.saturating_sub(8).min(MAX_COLS);
    if width < MIN_COLS {
        return None;
    }
    let room = root.height.saturating_sub(margin);
    let fits = |p: Plan| p.height(title_rows) <= room;
    if pictures {
        for film_rows in (FILM_MIN_ROWS..=FILM_ROWS).rev() {
            let p = Plan {
                width,
                film_rows,
                goal_rows: GOAL_ROWS,
            };
            if fits(p) {
                return Some(p);
            }
        }
        let p = Plan {
            width,
            film_rows: 0,
            goal_rows: GOAL_ROWS,
        };
        if fits(p) {
            return Some(p);
        }
    }
    let p = Plan {
        width,
        film_rows: 0,
        goal_rows: 0,
    };
    fits(p).then_some(p)
}

/// One page laid out on a card: its cells, where the pill goes, what
/// answers a click, and where the pictures play, all from its top-left.
pub(crate) struct Composed {
    pub(crate) buffer: Buffer,
    pub(crate) cursor: Option<u16>,
    pub(crate) cursor_cols: Option<(u16, u16)>,
    pub(crate) hits: Vec<LoopDialogHit>,
    pub(crate) film: Option<Rect>,
    pub(crate) goal: Option<Rect>,
}

/// The card's title.
pub(crate) fn title(dialog: &LoopLaunchDialog) -> &'static str {
    match (dialog.is_handoff_rl(), dialog.is_podrace(), dialog.deli) {
        (true, true, _) => "Start a five-day handoff RL",
        (true, false, _) => "Start handoff RL",
        (false, true, _) => "Start a five-day loop",
        (false, false, true) => "Start a deli loop",
        (false, false, false) => "Start a loop",
    }
}

/// What a lap is called: a round of the loop, or a forced roll.
fn lap_word(dialog: &LoopLaunchDialog) -> &'static str {
    if dialog.is_handoff_rl() {
        "ROLLS"
    } else {
        "LAPS"
    }
}

/// The objective: the task as typed, or the goal it drives.
fn objective(dialog: &LoopLaunchDialog) -> String {
    let task = dialog.task.split_whitespace().collect::<Vec<_>>().join(" ");
    if task.is_empty() {
        "the active goal".to_string()
    } else {
        task
    }
}

/// Words wrapped to `width` columns, at most `most` lines; a cut ends in `…`.
fn wrap(text: &str, width: usize, most: usize) -> Vec<String> {
    if width == 0 || most == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.width() + 1 + word.width() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        for ch in word.chars() {
            if line.width() + ch.width().unwrap_or(0) > width {
                lines.push(std::mem::take(&mut line));
            }
            line.push(ch);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > most {
        lines.truncate(most);
        if let Some(last) = lines.last_mut() {
            while last.width() + 1 > width {
                last.pop();
            }
            last.push('…');
        }
    }
    lines
}

fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        if out.width() + ch.width().unwrap_or(0) + 1 > width {
            break;
        }
        out.push(ch);
    }
    if width > 0 {
        out.push('…')
    }
    out
}

/// The page on a card `plan.width` wide, its words starting `title_rows`
/// down. `glass` leaves the chosen picker to the window's pill; a terminal
/// shades it.
pub(crate) fn compose(
    dialog: &LoopLaunchDialog,
    plan: Plan,
    title_rows: u16,
    glass: bool,
) -> Composed {
    let width = plan.width;
    let height = plan.height(title_rows);
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
    let mut hits = Vec::new();
    let right = width.saturating_sub(INSET);
    let mut y = title_rows;

    let film =
        (plan.film_rows > 0).then(|| Rect::new(2, y, width.saturating_sub(4), plan.film_rows));
    if film.is_some() {
        y += plan.film_rows + 1;
    }

    // The objective: beside the knight calling it in the window, a line
    // of words in a terminal.
    let label = if dialog.is_handoff_rl() {
        "CAMPAIGN"
    } else {
        "OBJECTIVE"
    };
    let goal = (plan.goal_rows > 0).then(|| Rect::new(INSET, y, GOAL_COLS, plan.goal_rows));
    if let Some(goal) = goal {
        let x = goal.right() + 3;
        let room = usize::from(right.saturating_sub(x)).max(1);
        let lines = wrap(
            &objective(dialog),
            room,
            usize::from(plan.goal_rows.saturating_sub(2)).max(1),
        );
        let top = y + (plan.goal_rows.saturating_sub(lines.len() as u16 + 1)) / 2;
        buffer.set_stringn(x, top, label, room, Style::new().fg(SOFT));
        for (i, line) in lines.iter().enumerate() {
            let ink = if i == 0 { BRIGHT } else { TEXT };
            buffer.set_stringn(x, top + 1 + i as u16, line, room, Style::new().fg(ink));
        }
        y += plan.goal_rows + 1;
    } else {
        let x = INSET + label.len() as u16 + 2;
        let room = usize::from(right.saturating_sub(x)).max(1);
        buffer.set_stringn(
            INSET,
            y,
            label,
            usize::from(right - INSET),
            Style::new().fg(SOFT),
        );
        for (i, line) in wrap(&objective(dialog), room, 2).iter().enumerate() {
            let ink = if i == 0 { BRIGHT } else { TEXT };
            buffer.set_stringn(x, y + i as u16, line, room, Style::new().fg(ink));
        }
        y += 3;
    }

    // The pickers, side by side.
    let column = (width.saturating_sub(2 * INSET)) / 3;
    let pickers = dialog.pickers();
    let focus = dialog.focus();
    let mut cursor = None;
    let mut cursor_cols = None;
    for (i, picker) in pickers.iter().enumerate() {
        let x = INSET + i as u16 * column;
        let focused = picker.focus == focus;
        let name = match picker.focus {
            LoopDialogFocus::Iterations => lap_word(dialog),
            LoopDialogFocus::Duration => "CLOCK",
            _ => "FUEL",
        };
        buffer.set_stringn(
            x,
            y,
            name,
            usize::from(column),
            Style::new().fg(if focused { TEXT } else { SOFT }),
        );
        let span = picker_value(
            &mut buffer,
            x,
            y + 1,
            column.saturating_sub(2),
            picker,
            focused,
            glass,
        );
        hits.push(LoopDialogHit {
            rect: Rect::new(x, y + 1, 2, 1),
            action: picker.step(false),
        });
        hits.push(LoopDialogHit {
            rect: Rect::new(x + span - 2, y + 1, 2, 1),
            action: picker.step(true),
        });
        strip(
            &mut buffer,
            x,
            y + 2,
            column.saturating_sub(2),
            picker,
            &mut hits,
        );
        if focused {
            cursor = Some(y + 1);
            cursor_cols = Some((x, span));
        }
    }
    y += 4;

    // The actions.
    let gap = 6;
    let total = START.len() as u16 + gap + CANCEL.len() as u16;
    let sx = (width.saturating_sub(total)) / 2;
    let cx = sx + START.len() as u16 + gap;
    let start_focused = focus == LoopDialogFocus::Start;
    let cancel_focused = focus == LoopDialogFocus::Cancel;
    let shade = |on: bool, style: Style| {
        if on && !glass {
            style.bg(PICK_BG)
        } else {
            style
        }
    };
    let start_style = if start_focused {
        Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
    };
    let cancel_style = if cancel_focused {
        Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(SOFT)
    };
    if !glass {
        for (on, x, w) in [
            (start_focused, sx, START.len() as u16),
            (cancel_focused, cx, CANCEL.len() as u16),
        ] {
            if on {
                buffer.set_stringn(
                    x - 1,
                    y,
                    " ".repeat(usize::from(w) + 2),
                    usize::from(w) + 2,
                    Style::new().bg(PICK_BG),
                );
            }
        }
    }
    buffer.set_stringn(sx, y, START, START.len(), shade(start_focused, start_style));
    buffer.set_stringn(
        cx,
        y,
        CANCEL,
        CANCEL.len(),
        shade(cancel_focused, cancel_style),
    );
    hits.push(LoopDialogHit {
        rect: Rect::new(sx - 1, y, START.len() as u16 + 2, 1),
        action: LoopDialogAction::Start,
    });
    hits.push(LoopDialogHit {
        rect: Rect::new(cx - 1, y, CANCEL.len() as u16 + 2, 1),
        action: LoopDialogAction::Cancel,
    });
    if start_focused || cancel_focused {
        cursor = Some(y);
        cursor_cols = Some(if start_focused {
            (sx, START.len() as u16)
        } else {
            (cx, CANCEL.len() as u16)
        });
    }
    y += 2;

    // What the keys do.
    let keys: &[(&str, &str)] = if dialog.custom_entry_active() {
        if dialog.is_handoff_rl() {
            &[
                ("0-9", "rolls"),
                ("⌫", "edit"),
                ("Enter", "set"),
                ("Esc", "drop it"),
            ]
        } else {
            &[
                ("0-9", "laps"),
                ("⌫", "edit"),
                ("Enter", "set"),
                ("Esc", "drop it"),
            ]
        }
    } else {
        &[
            ("Tab", "next"),
            ("← →", "change"),
            ("Enter", "drop the flag"),
            ("Esc", "cancel"),
        ]
    };
    let mut x = INSET;
    let words_width: usize = keys
        .iter()
        .map(|(key, word)| key.width() + 1 + word.width())
        .sum();
    let gap = (usize::from(right - INSET).saturating_sub(words_width) / (keys.len() - 1)).min(5);
    for (key, word) in keys {
        x = buffer
            .set_stringn(
                x,
                y,
                key,
                usize::from(right.saturating_sub(x)),
                Style::new().fg(ACCENT),
            )
            .0;
        x = buffer
            .set_stringn(
                x,
                y,
                format!(" {word}{}", " ".repeat(gap)),
                usize::from(right.saturating_sub(x)),
                Style::new().fg(SOFT),
            )
            .0;
    }
    Composed {
        buffer,
        cursor,
        cursor_cols,
        hits,
        film,
        goal,
    }
}

/// A picker's value between its arrows at `x`: the arrows warm when it
/// has focus. Returns the columns it spans.
fn picker_value(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    room: u16,
    picker: &Picker,
    focused: bool,
    glass: bool,
) -> u16 {
    let value_cols = VALUE_COLS
        .max(picker.value.width() as u16)
        .min(room.saturating_sub(4));
    let span = value_cols + 4;
    if focused && !glass {
        buffer.set_stringn(
            x,
            y,
            " ".repeat(usize::from(span)),
            usize::from(span),
            Style::new().bg(PICK_BG),
        );
    }
    let bg = |style: Style| {
        if focused && !glass {
            style.bg(PICK_BG)
        } else {
            style
        }
    };
    let arrow = bg(Style::new().fg(if focused { ACCENT } else { FAINT }));
    let value = fit(&picker.value, usize::from(value_cols));
    let pad = (usize::from(value_cols) - value.width()) / 2;
    let ink = if focused {
        Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(TEXT)
    };
    buffer.set_stringn(x, y, "‹", 1, arrow);
    buffer.set_stringn(
        x + 2 + pad as u16,
        y,
        &value,
        usize::from(value_cols),
        bg(ink),
    );
    buffer.set_stringn(x + span - 1, y, "›", 1, arrow);
    span
}

/// A picker's choices in a row under its value, the chosen one lit. A row
/// too long for its column closes up, then keeps the chosen one in view.
fn strip(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    room: u16,
    picker: &Picker,
    hits: &mut Vec<LoopDialogHit>,
) {
    let widths: Vec<u16> = picker
        .choices
        .iter()
        .map(|c| (c.width() as u16).min(room))
        .collect();
    let width = |gap: u16, from: usize, to: usize| {
        widths[from..=to].iter().sum::<u16>() + gap * (to - from) as u16
    };
    let gap = if width(2, 0, widths.len() - 1) <= room {
        2
    } else {
        1
    };
    let fits = |from: usize, to: usize| width(gap, from, to) <= room;
    let mut first = 0;
    while first < picker.selected && !fits(first, picker.selected) {
        first += 1;
    }
    let mut at = x;
    for (i, choice) in picker.choices.iter().enumerate().skip(first) {
        let w = widths[i];
        if at + w > x + room {
            break;
        }
        let chosen = i == picker.selected;
        let style = if chosen {
            Style::new().fg(if picker.entry { ACCENT } else { TEXT })
        } else {
            Style::new().fg(FAINT)
        };
        buffer.set_stringn(at, y, fit(choice, usize::from(w)), usize::from(w), style);
        hits.push(LoopDialogHit {
            rect: Rect::new(at, y, w, 1),
            action: (picker.action)(i),
        });
        at += w + gap;
    }
}

/// Where a card `width` by `height` sits in `root`: centred.
fn centred(root: Rect, width: u16, height: u16) -> Rect {
    let (width, height) = (width.min(root.width), height.min(root.height));
    Rect::new(
        root.x + root.width.saturating_sub(width) / 2,
        root.y + root.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn offset(hits: Vec<LoopDialogHit>, area: Rect) -> Vec<LoopDialogHit> {
    hits.into_iter()
        .map(|hit| LoopDialogHit {
            rect: Rect {
                x: hit.rect.x + area.x,
                y: hit.rect.y + area.y,
                ..hit.rect
            },
            ..hit
        })
        .collect()
}

// --- the terminal -------------------------------------------------------------

/// Keep the window card's title space in the terminal's hairline shell.
const BOX_TITLE_ROWS: u16 = 4;

/// The terminal card's place in `root`, for what must keep out from under
/// it. Too small a window gets the whole of it.
pub(crate) fn terminal_area(root: Rect) -> Rect {
    match plan(root, BOX_TITLE_ROWS, true, 6) {
        Some(plan) => centred(root, plan.width, plan.height(BOX_TITLE_ROWS)),
        None => centred(root, root.width.min(44), root.height.min(4)),
    }
}

/// The card in a terminal: black, a hairline round it, its title on the
/// first row inside. Returns what answers a click, on screen.
pub(crate) fn render_terminal(
    buf: &mut Buffer,
    root: Rect,
    dialog: &LoopLaunchDialog,
) -> Vec<LoopDialogHit> {
    let area = terminal_area(root);
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }
    let Some(plan) = plan(root, BOX_TITLE_ROWS, true, 6) else {
        // Too small for the pickers: say so, and still take Esc.
        Clear.render(area, buf);
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(EDGE))
            .render(area, buf);
        let words = if dialog.is_handoff_rl() {
            "handoff RL: make the window larger"
        } else {
            "loop: make the window larger"
        };
        if area.width > 4 && area.height > 2 {
            buf.set_stringn(
                area.x + 2,
                area.y + 1,
                words,
                usize::from(area.width.saturating_sub(4)),
                Style::new().fg(SOFT),
            );
        }
        return Vec::new();
    };
    let mut composed = compose(dialog, plan, BOX_TITLE_ROWS, false);
    terminal_scenes(&mut composed, plan);
    Clear.render(area, buf);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(Color::Black);
            }
        }
    }
    for y in 0..area.height.min(composed.buffer.area.height) {
        for x in 0..area.width.min(composed.buffer.area.width) {
            let from = &composed.buffer[(x, y)];
            if let Some(cell) = buf.cell_mut((area.x + x, area.y + y))
                && (from.symbol() != " " || from.bg != Color::Reset)
            {
                *cell = from.clone();
                if cell.bg == Color::Reset {
                    cell.set_bg(Color::Black);
                }
            }
        }
    }
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(EDGE))
        .render(area, buf);
    buf.set_stringn(
        area.x + INSET,
        area.y + 1,
        title(dialog),
        usize::from(area.width.saturating_sub(2 * INSET)),
        Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD),
    );
    offset(composed.hits, area)
}

// --- the corner widget --------------------------------------------------------

/// The same scene rectangles as the glass card, fitted to terminal dots.
/// Cache the stills so changing focus never rebuilds a 3D scene.
fn terminal_scenes(composed: &mut Composed, plan: Plan) {
    use angel_delve::render::racers::card;
    use std::sync::{Mutex, OnceLock};
    static SCENES: OnceLock<Mutex<Option<(Plan, Buffer)>>> = OnceLock::new();
    if composed.film.is_none() && composed.goal.is_none() {
        return;
    }
    let mut cache = SCENES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if cache.as_ref().is_none_or(|(p, _)| *p != plan) {
        let mut buf = Buffer::empty(composed.buffer.area);
        for (at, goal) in [(composed.film, false), (composed.goal, true)] {
            let Some(at) = at else { continue };
            let (w, h) = (u32::from(at.width) * 2, u32::from(at.height) * 4);
            let film = if goal {
                card::goal(w, h)
            } else {
                card::idle(w, h)
            };
            paint_braille(&mut buf, at, w, h, film.frames[0].clone());
        }
        *cache = Some((plan, buf));
    }
    if let Some((_, buf)) = cache.as_ref() {
        for at in [composed.film, composed.goal].into_iter().flatten() {
            for y in at.top()..at.bottom() {
                for x in at.left()..at.right() {
                    composed.buffer[(x, y)] = buf[(x, y)].clone();
                }
            }
        }
    }
}

fn paint_braille(buf: &mut Buffer, at: Rect, w: u32, h: u32, mut rgba: Vec<u8>) {
    flatten_on_black(&mut rgba);
    let image = image::RgbaImage::from_raw(w, h, rgba).expect("scene pixels match its dimensions");
    let dots = angel_delve::render::braille::frame_to_braille_world3d(
        &image,
        usize::from(at.width),
        usize::from(at.height),
        false,
    );
    for (i, dot) in dots.cells.iter().enumerate() {
        let (x, y) = (i as u16 % at.width + at.x, i as u16 / at.width + at.y);
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char(dot.glyph)
                .set_fg(Color::Rgb(dot.fg[0], dot.fg[1], dot.fg[2]));
        }
    }
}

fn flatten_on_black(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        let a = u16::from(pixel[3]);
        for c in &mut pixel[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }
}

/// The ticker's racer slot, in absolute cells: two rows of the strip's
/// content, hard against its left border.
pub(crate) fn ticker_racer_rect(area: Rect) -> Rect {
    let width = if area.width >= 52 { 12 } else { 9 };
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        width.min(area.width.saturating_sub(2)),
        2.min(area.height.saturating_sub(2).max(1)),
    )
}

/// The running loop in a terminal: the same strip the window draws, with
/// the knight in braille.
pub(crate) fn render_terminal_ticker(
    buf: &mut Buffer,
    area: Rect,
    race: &Race,
    s: Option<f32>,
    phase: f32,
) {
    render_ticker(buf, area, race, s, phase, true);
}

/// The race ticker itself: one strip under the header. Its top and bottom
/// rules are the header's and the body's own borders (the layout shares
/// them), so this fills only the rows between, sets the side borders, and
/// lays the LOOP tab over the top rule. Left to right: the knight in side
/// profile, the lap he is on, a dotted road, then the clock and fuel left.
/// `phase` is seconds since the flag dropped; it walks the road's bright
/// head and flickers the exhaust. `s` is a lap run in flight.
pub(crate) fn render_ticker(
    buf: &mut Buffer,
    area: Rect,
    race: &Race,
    s: Option<f32>,
    phase: f32,
    paint_racer_cells: bool,
) {
    if area.width < TICKER_MIN_COLS || area.height < 3 {
        return;
    }
    let top = area.y.saturating_add(1);
    let bottom = area.bottom().saturating_sub(1);
    // No ground of its own: the strip is part of the shell, so its glyphs
    // sit on whatever the frame already holds. Only the side borders and
    // the words are drawn.
    for y in top..bottom {
        for x in [area.left(), area.right().saturating_sub(1)] {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char('│');
                cell.set_fg(EDGE);
            }
        }
    }

    // The tab rides the top rule, hard at the left like a header's title.
    let (state, state_ink) = if race.complete {
        ("✓", Color::Rgb(110, 190, 120))
    } else if race.paused {
        ("❚❚", Color::Rgb(214, 164, 84))
    } else {
        ("●", Color::Rgb(110, 190, 120))
    };
    let tab = if race.rolls { " RL " } else { " LOOP " };
    let tab_x = area.x.saturating_add(2);
    let after_tab = buf
        .set_stringn(
            tab_x,
            area.y,
            tab,
            usize::from(area.width),
            Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD),
        )
        .0;
    let after_state = buf
        .set_stringn(
            after_tab,
            area.y,
            state,
            2,
            Style::new().fg(state_ink).add_modifier(Modifier::BOLD),
        )
        .0;
    buf.set_stringn(after_state, area.y, " ", 1, Style::new());

    // The knight, in profile, engines lit while he runs.
    let racer = ticker_racer_rect(area);
    if paint_racer_cells && racer.width > 0 && racer.height > 0 {
        use angel_delve::render::racers::card;
        use std::sync::OnceLock;
        static FILMS: OnceLock<[card::Film; 3]> = OnceLock::new();
        let films = FILMS.get_or_init(|| {
            [
                card::ticker_racer(48, 16, 0.0),
                card::ticker_racer(48, 16, 0.45),
                card::ticker_racer(48, 16, 0.8),
            ]
        });
        let film = if race.paused || race.complete {
            &films[0]
        } else if (phase * 3.0) as usize % 2 == 0 {
            &films[1]
        } else {
            &films[2]
        };
        if let Some(frame) = film.frames.first() {
            paint_braille(buf, racer, film.width, film.height, frame.clone());
        }
    }

    // The lap he is on: finished laps plus the one under way.
    let text_y = top;
    let current = if race.complete {
        race.laps_done.max(1)
    } else {
        race.laps_done.saturating_add(1)
    };
    let current = if race.laps > 0 {
        current.min(race.laps)
    } else {
        current
    };
    let lap = format!("{}{current:02}", if race.rolls { 'R' } else { 'L' });
    let lap_x = racer.right().saturating_add(1);
    let mut lap_end = buf
        .set_stringn(
            lap_x,
            text_y,
            &lap,
            usize::from(area.right().saturating_sub(lap_x)),
            Style::new().fg(BRIGHT).add_modifier(Modifier::BOLD),
        )
        .0;
    if race.laps > 0 && area.width >= 58 {
        lap_end = buf
            .set_stringn(
                lap_end,
                text_y,
                format!("/{}", race.laps),
                usize::from(area.right().saturating_sub(lap_end)),
                Style::new().fg(SOFT),
            )
            .0;
    }

    // What is left, at the right: the clock as a timer, the fuel compact,
    // either endless as ∞. Paused carries its bars; complete its chequers.
    let clock = race.clock.map(|(left, _)| ticker_clock(left));
    let fuel = race.fuel.map(|(left, _)| fuel_words(left));
    let mut right = match (clock, fuel) {
        (Some(clock), Some(fuel)) => format!("{clock}  {fuel}"),
        (Some(clock), None) => format!("{clock}  ∞"),
        (None, Some(fuel)) => format!("∞  {fuel}"),
        (None, None) => "∞".to_string(),
    };
    if race.complete {
        right = format!("▚▞ {right}");
    } else if race.paused {
        right.push_str(" ❚❚");
    }
    let right_w = right.width() as u16;
    let mut right_x = area.right().saturating_sub(1).saturating_sub(right_w);
    if right_x <= lap_end.saturating_add(3) {
        // No room for both gauges: the clock is the one that runs out first.
        right = race
            .clock
            .map(|(left, _)| ticker_clock(left))
            .unwrap_or_else(|| "∞".to_string());
        right_x = area
            .right()
            .saturating_sub(1)
            .saturating_sub(right.width() as u16);
    }
    if right_x > lap_end {
        buf.set_stringn(
            right_x,
            text_y,
            &right,
            usize::from(area.right().saturating_sub(right_x)),
            Style::new().fg(TEXT).add_modifier(Modifier::BOLD),
        );
    }

    // The dotted road under the words: spent dots warm, the rest faint, a
    // bright head walking it while the knight runs.
    let road_y = if area.height >= 4 {
        top.saturating_add(1)
    } else {
        text_y
    };
    let road_start = if area.height >= 4 {
        lap_x
    } else {
        lap_end.saturating_add(2)
    };
    let road_end = right_x.saturating_sub(2);
    if road_end > road_start && road_y < bottom {
        let len = usize::from(road_end - road_start);
        let (clock_left, _) = race.left();
        let progress = if race.complete {
            1.0
        } else if race.laps > 0 {
            race.laps_done as f32 / race.laps as f32
        } else if let Some(left) = clock_left {
            1.0 - left
        } else {
            0.0
        }
        .clamp(0.0, 1.0);
        for i in (0..len).step_by(2) {
            let frac = if len > 1 {
                i as f32 / (len - 1) as f32
            } else {
                0.0
            };
            let (mark, ink) = if frac <= progress {
                ("•", ACCENT)
            } else {
                ("·", FAINT)
            };
            buf.set_stringn(
                road_start + i as u16,
                road_y,
                mark,
                1,
                Style::new().fg(ink),
            );
        }
        if race.complete && len >= 2 {
            buf.set_stringn(
                road_end.saturating_sub(2),
                road_y,
                "▚▞",
                2,
                Style::new().fg(BRIGHT),
            );
        } else if !race.paused && len > 2 {
            let head = match s {
                Some(s) => (s.clamp(0.0, 1.0) * (len - 1) as f32) as usize,
                None => (phase * 7.0) as usize % len,
            };
            let head = head - head % 2;
            buf.set_stringn(
                road_start + head as u16,
                road_y,
                "›",
                1,
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            );
        }
    }
}

/// A count of seconds as the ticker's timer shows it: `04:23`, then
/// hours, then days.
pub(crate) fn ticker_clock(secs: u64) -> String {
    match secs {
        s if s >= 86_400 => format!("{}d {:02}h", s / 86_400, s % 86_400 / 3_600),
        s if s >= 3_600 => format!("{}:{:02}:{:02}", s / 3_600, s % 3_600 / 60, s % 60),
        s => format!("{:02}:{:02}", s / 60, s % 60),
    }
}

/// What the ticker shows of a running loop.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Race {
    pub(crate) laps_done: usize,
    /// The cap on laps (0: endless).
    pub(crate) laps: usize,
    /// Seconds left of the clock, and of how many (none: endless).
    pub(crate) clock: Option<(u64, u64)>,
    /// Tokens left of the fuel, and of how many (none: endless).
    pub(crate) fuel: Option<(usize, usize)>,
    pub(crate) rolls: bool,
    pub(crate) paused: bool,
    /// The loop ran to its end: chequers, held briefly, then gone.
    pub(crate) complete: bool,
    pub(crate) task: String,
}

impl Race {
    /// What is left of the clock and of the fuel, 0 to 1.
    pub(crate) fn left(&self) -> (Option<f32>, Option<f32>) {
        let part = |left: f64, of: f64| {
            if of > 0.0 {
                (left / of).clamp(0.0, 1.0) as f32
            } else {
                0.0
            }
        };
        (
            self.clock.map(|(l, of)| part(l as f64, of as f64)),
            self.fuel.map(|(l, of)| part(l as f64, of as f64)),
        )
    }
}

/// The ticker's outer rows: the shared top rule, two content rows (the
/// knight and words over the dotted road), and the shared bottom rule.
pub(crate) const TICKER_ROWS: u16 = 4;
/// Narrower than this the lap, road, and timer cannot share a strip.
pub(crate) const TICKER_MIN_COLS: u16 = 34;

/// A count of tokens as the gauge shows it.
pub(crate) fn fuel_words(tokens: usize) -> String {
    match tokens {
        t if t >= 1_000_000 => format!("{:.1}M", t as f64 / 1e6).replace(".0M", "M"),
        t if t >= 1_000 => format!("{}K", t / 1_000),
        t => t.to_string(),
    }
}

/// Seconds a lap run takes along the ticker's road.
pub(crate) const LAP_RUN: f32 = 2.4;

/// How far along a lap run is `t` seconds in: eased off the line, flat
/// out, eased home. None once he is home.
pub(crate) fn lap_run(t: f32) -> Option<f32> {
    (0.0..LAP_RUN).contains(&t).then(|| {
        let u = t / LAP_RUN;
        if u < 0.5 {
            2.0 * u * u
        } else {
            1.0 - (-2.0 * u + 2.0).powi(2) / 2.0
        }
    })
}

// --- the window ---------------------------------------------------------------

#[cfg(feature = "glass")]
pub(crate) use window::*;

#[cfg(feature = "glass")]
mod window {
    use super::*;
    use angel_glass::card::{Card, Entrance, Film, Knight, TITLE_ROWS};
    use angel_glass::graphics::Picture;
    use angel_glass::{Layer, LayerKind, Placement};
    use std::sync::{Arc, Mutex, OnceLock};

    /// Picture ids: the card's films and the widget's pictures.
    const BASE: u32 = angel_glass::LAYER_PICTURES + 0x4C4F_0000;
    const IDLE: u32 = BASE + 1;
    const LAUNCH: u32 = BASE + 2;
    const GOAL: u32 = BASE + 3;
    pub(crate) const TICKER_RACER_RUN: u32 = BASE + 5;
    pub(crate) const TICKER_RACER_IDLE: u32 = BASE + 6;
    const TEXT_SHADE: u32 = BASE + 7;
    /// The films are never drawn wider than this: a larger window scales.
    const MOST_FILM_PX: u32 = 1400;

    /// What the card is doing: open for choosing, or playing the start.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum Phase {
        Open,
        Launch,
    }

    /// The films at one size: the card's three.
    pub(crate) struct Art {
        pub(crate) key: ArtKey,
        idle: Reel,
        launch: Reel,
        goal: Reel,
    }

    /// One film as its sheet and how to read it.
    #[derive(Clone)]
    pub(crate) struct Reel {
        pub(crate) sheet: Arc<Picture>,
        pub(crate) frame: (u32, u32),
        pub(crate) frames: u16,
        pub(crate) cols: u16,
        pub(crate) fps: f32,
    }

    impl Reel {
        fn film(&self, at: Rect, once: bool) -> Film {
            Film {
                sheet: Arc::clone(&self.sheet),
                frame: self.frame,
                frames: self.frames,
                cols: self.cols,
                fps: self.fps,
                once,
                at,
            }
        }
    }

    /// The sizes the films are made at: the window's cell, and the film's
    /// and the goal shot's boxes in cells.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub(crate) struct ArtKey {
        pub(crate) cell: (u16, u16),
        pub(crate) film: (u16, u16),
        pub(crate) goal: (u16, u16),
    }

    #[derive(Default)]
    struct Slot {
        ready: Option<Arc<Art>>,
        making: Option<ArtKey>,
    }

    static ART: OnceLock<Mutex<Slot>> = OnceLock::new();

    /// The films for `key`, once made. The first ask starts making them on
    /// a thread of their own (a fraction of a second); until then the card
    /// shows without them.
    pub(crate) fn art(key: ArtKey) -> Option<Arc<Art>> {
        let slot = ART.get_or_init(Default::default);
        let mut held = slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(art) = held.ready.as_ref().filter(|a| a.key == key) {
            return Some(Arc::clone(art));
        }
        // A resize can ask for many sizes before one film is finished.
        // Keep one builder and scale the last ready film in the meantime.
        if held.making.is_none() {
            held.making = Some(key);
            let spawned = std::thread::Builder::new()
                .name("loop-card-art".into())
                .spawn(move || {
                    let art = Arc::new(make(key));
                    let mut held = ART
                        .get_or_init(Default::default)
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if held.making == Some(key) {
                        held.ready = Some(art);
                        held.making = None;
                    }
                });
            if spawned.is_err() {
                held.making = None;
            }
        }
        held.ready.as_ref().map(Arc::clone)
    }

    /// Make the films for `key` now, on this thread: what `art` does off it.
    pub(crate) fn make(key: ArtKey) -> Art {
        use angel_delve::render::racers::card;
        let px = |(cols, rows): (u16, u16)| {
            let (w, h) = (
                u32::from(cols) * u32::from(key.cell.0),
                u32::from(rows) * u32::from(key.cell.1),
            );
            let k = (MOST_FILM_PX as f32 / w.max(1) as f32).min(1.0);
            (
                ((w as f32 * k) as u32).max(16),
                ((h as f32 * k) as u32).max(8),
            )
        };
        let (fw, fh) = px(key.film);
        let (gw, gh) = px(key.goal);
        Art {
            key,
            idle: reel(IDLE, &card::idle(fw, fh)),
            launch: reel(LAUNCH, &card::launch(fw, fh)),
            goal: reel(GOAL, &card::goal(gw, gh)),
        }
    }

    fn reel(id: u32, film: &angel_delve::render::racers::card::Film) -> Reel {
        // As many to a row as keep the sheet inside a texture's side.
        let cols = (8192 / film.width.max(1)).clamp(1, film.frames.len().max(1) as u32) as usize;
        let (width, height, rgba) = film.sheet(cols);
        Reel {
            sheet: Arc::new(Picture {
                id,
                width,
                height,
                rgba,
                cols: 0,
                rows: 0,
            }),
            frame: (film.width, film.height),
            frames: film.frames.len() as u16,
            cols: cols as u16,
            fps: film.fps,
        }
    }

    /// The sizes the card at `plan` wants its films made at.
    pub(crate) fn art_key(plan: Plan, cell: (u16, u16)) -> ArtKey {
        ArtKey {
            cell,
            film: (
                plan.width.saturating_sub(4),
                plan.film_rows.max(FILM_MIN_ROWS),
            ),
            goal: (GOAL_COLS, GOAL_ROWS),
        }
    }

    /// The card's plan in `root`, with room kept above for the squire.
    pub(crate) fn card_plan(root: Rect) -> Option<Plan> {
        plan(root, TITLE_ROWS, true, 6)
    }

    /// The card for `dialog` in `root`: its words composed, its films from
    /// `art` when they are made, the squire presenting it, or cheering
    /// the start. Returns what answers a click, on screen.
    pub(crate) fn card(
        root: Rect,
        dialog: &LoopLaunchDialog,
        art: Option<&Art>,
        phase: Phase,
    ) -> Option<(Card, Vec<LoopDialogHit>)> {
        let plan = card_plan(root)?;
        // Through the start the flag's button stays pressed under the pill.
        let mut pressed;
        let dialog = if phase == Phase::Launch {
            pressed = dialog.clone();
            pressed.apply(LoopDialogAction::Start);
            &pressed
        } else {
            dialog
        };
        let composed = compose(dialog, plan, TITLE_ROWS, true);
        let height = composed.buffer.area.height;
        let area = centred(root, plan.width, height);
        let area = Rect {
            y: area.y.max(root.y + 3.min(root.height)),
            ..area
        };
        let shade = text_shade(&composed);
        let mut card = Card::new(area, composed.buffer)?;
        card.title = title(dialog).to_string();
        card.serial = serial(dialog);
        card.entrance = Entrance::Kick;
        card.films.push(shade);
        if let Some(art) = art {
            if let Some(at) = composed.film {
                // A film made for a taller box plays in this one, scaled.
                card.films.push(match phase {
                    Phase::Open => art.idle.film(at, false),
                    Phase::Launch => art.launch.film(at, true),
                });
            }
            if let Some(at) = composed.goal {
                card.films.push(art.goal.film(at, false));
            }
        }
        card.cursor = composed.cursor;
        card.cursor_cols = composed.cursor_cols;
        card.knight = match phase {
            Phase::Open => Knight::Present,
            Phase::Launch => Knight::Cheer,
        };
        Some((card, offset(composed.hits, area)))
    }

    /// A still under the words: opaque tonal ink across each text row,
    /// feathered into the glass outside it. Empty rows and pictures stay clear.
    /// The title is set by the window in rows 1..3, outside the cell buffer.
    fn text_shade(composed: &Composed) -> Film {
        struct Shade {
            area: Rect,
            rows: Vec<Rect>,
            sheet: Arc<Picture>,
        }
        static SHADE: OnceLock<Mutex<Option<Shade>>> = OnceLock::new();
        let buffer = &composed.buffer;
        let area = buffer.area;
        let mut rows = vec![Rect::new(INSET, 1, area.width - 2 * INSET, 2)];
        for y in TITLE_ROWS..area.height {
            let written = (0..area.width).any(|x| {
                let cell = &buffer[(x, y)];
                !cell.symbol().trim().is_empty() && !cell.modifier.contains(Modifier::HIDDEN)
            });
            if written {
                let left = composed
                    .goal
                    .filter(|goal| y >= goal.y && y < goal.bottom())
                    .map_or(INSET, |goal| goal.right() + 3);
                rows.push(Rect::new(left, y, area.width - INSET - left, 1));
            }
        }
        let mut held = SHADE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if held
            .as_ref()
            .is_none_or(|s| s.area != area || s.rows != rows)
        {
            // A logical cell raster scales with the card, including when the
            // films are still being made. Keep a full cell opaque beneath the
            // glyphs; only the surrounding margin feathers, never the words.
            let (cw, ch) = (8, 16);
            let (w, h) = (u32::from(area.width) * cw, u32::from(area.height) * ch);
            let mut rgba = vec![0; (w * h * 4) as usize];
            let (fx, fy) = (cw as f32, ch as f32 * 0.375);
            for row in &rows {
                let (left, top) = (u32::from(row.x) * cw, u32::from(row.y) * ch);
                let (right, bottom) = (u32::from(row.right()) * cw, u32::from(row.bottom()) * ch);
                for y in top.saturating_sub(fy as u32)..(bottom + fy as u32).min(h) {
                    for x in left.saturating_sub(fx as u32)..(right + fx as u32).min(w) {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let dx = (left as f32 - px).max(px - right as f32).max(0.0) / fx;
                        let dy = (top as f32 - py).max(py - bottom as f32).max(0.0) / fy;
                        let d = dx.hypot(dy).min(1.0);
                        let a = ((1.0 - d * d * (3.0 - 2.0 * d)) * 255.0).round() as u8;
                        let o = ((y * w + x) * 4) as usize;
                        rgba[o..o + 3].copy_from_slice(&[10, 12, 16]);
                        rgba[o + 3] = rgba[o + 3].max(a);
                    }
                }
            }
            *held = Some(Shade {
                area,
                rows,
                sheet: Arc::new(Picture {
                    id: TEXT_SHADE,
                    width: w,
                    height: h,
                    rgba,
                    cols: 0,
                    rows: 0,
                }),
            });
        }
        let sheet = Arc::clone(&held.as_ref().unwrap().sheet);
        Film {
            frame: (sheet.width, sheet.height),
            sheet,
            frames: 1,
            cols: 1,
            fps: 0.0,
            once: false,
            at: area,
        }
    }

    /// The card's identity: the same launch keeps its words in place while
    /// its pickers change.
    fn serial(dialog: &LoopLaunchDialog) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        "loop-card".hash(&mut hasher);
        title(dialog).hash(&mut hasher);
        dialog.task.hash(&mut hasher);
        hasher.finish()
    }

    /// The ticker's knight in side profile, engines lit and cold, made at
    /// the window's cell size for its widest slot. Handed to the window
    /// once per size.
    pub(crate) fn ticker_pictures(cell: (u16, u16)) -> [Picture; 2] {
        use angel_delve::render::racers::card;
        let (cw, ch) = (u32::from(cell.0.max(1)), u32::from(cell.1.max(1)));
        let (w, h) = (12 * cw, 2 * ch);
        let picture = |id: u32, burn: f32| {
            let film = card::ticker_racer(w, h, burn);
            Picture {
                id,
                width: film.width,
                height: film.height,
                rgba: film.frames.into_iter().next().unwrap_or_default(),
                cols: 0,
                rows: 0,
            }
        };
        [
            picture(TICKER_RACER_RUN, 0.65),
            picture(TICKER_RACER_IDLE, 0.0),
        ]
    }

    /// The ticker's glass for this frame: the knight in profile over his
    /// slot in the strip, bobbing a touch while he runs. The strip's
    /// words and road are cells (`render_ticker`); only the racer is a
    /// picture. `fade` brings him in after the flag drops.
    pub(crate) fn ticker_layers(race: &Race, area: Rect, phase: f32, fade: f32) -> Vec<Layer> {
        let slot = ticker_racer_rect(area);
        if slot.width == 0 || slot.height == 0 {
            return Vec::new();
        }
        let bob = if race.paused || race.complete {
            0.0
        } else {
            (phase * std::f32::consts::TAU * 0.9).sin() * 0.045
        };
        let picture = if race.paused || race.complete {
            TICKER_RACER_IDLE
        } else {
            TICKER_RACER_RUN
        };
        let mut knight = Layer::new(
            2,
            1,
            Placement::Cells([
                f32::from(slot.x),
                f32::from(slot.y) - 0.08 + bob,
                f32::from(slot.width),
                f32::from(slot.height) + 0.16,
            ]),
            LayerKind::Picture(picture),
        );
        knight.opacity = fade.clamp(0.0, 1.0);
        vec![knight]
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/loop_card__tests.rs"]
mod tests;
