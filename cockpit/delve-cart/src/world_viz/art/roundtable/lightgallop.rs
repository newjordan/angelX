//! Lightgallop's terminal text over its cell picture: the cells are four
//! pixels wide, so the small print of the heads-up display is said in text
//! rows instead (the clock, the sparks, the spur's lantern gauge, the lanterns
//! lit and the Dimming's gap), the words the picture prints in the sky (the
//! title card, the fall, the bell, the keys) are said over their own garbled
//! lettering, and the tally and its rank in a box that covers the picture's
//! panel. Row 0 is the shell's header, which the shared checks read.
use angel_delve::rules::{
    HZ, Run,
    lightgallop::{self as rules, level::level, rider::GAUGE_MAX},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

const GOLD: Color = Color::Rgb(226, 203, 139);
const PALE: Color = Color::Rgb(176, 178, 168);
const AMBER: Color = Color::Rgb(236, 182, 74);
const RED: Color = Color::Rgb(168, 52, 31);

fn clock(ticks: u32) -> String {
    let s = ticks / HZ;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A row of text on black, left-aligned at `area`'s left edge.
fn row(frame: &mut Frame, area: Rect, y: u16, text: String, fg: Color) {
    if y >= area.height || area.width == 0 {
        return;
    }
    let r = Rect::new(area.x, area.y + y, area.width, 1);
    frame.render_widget(Clear, r);
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(fg).bg(Color::Black)), r);
}

pub(super) fn paint(frame: &mut Frame, area: Rect, run: &Run, focus: Option<u32>) {
    let Some(s) = run.lightgallop.as_ref() else {
        return;
    };
    super::header(frame, area, rules::NAME, s.elapsed, s.seats.len());
    let Some(id) = focus.filter(|id| s.seats.contains_key(id)).or_else(|| s.seats.keys().next().copied()) else {
        return;
    };
    let lvl = level();
    let r = &s.seats[&id].rider;
    // The spur's lantern gauge, ten cells.
    let lit = (r.gauge * 10 / GAUGE_MAX).clamp(0, 10) as usize;
    let gauge: String = "\u{2588}".repeat(lit) + &"\u{2591}".repeat(10 - lit);
    let posts = s.posts.count_ones();
    row(
        frame,
        area,
        1,
        format!(
            " SPARKS {:3}   SPUR {gauge}   CHAIN {}   LANTERNS {posts}/{}",
            r.sparks,
            r.chain,
            lvl.posts.len()
        ),
        if r.gauge < 150 { RED } else { AMBER },
    );
    let gap = ((r.x - s.dim) / 256).max(0);
    let act = ["DOWNHILL GATE", "WHEEL ARCH", "REED FEN", "BELL GATE"][rules::Session::act_at(r.x / 256)];
    let hint = if s.elapsed < 300 { "A D RUN   E LEAP   SPACE SPUR" } else { "HOLD ESC LEAVE" };
    row(
        frame,
        area,
        2,
        format!(" {act:<14} {}   GLOAM {gap:4}   {hint}", clock(s.elapsed)),
        if gap < 260 { RED } else { PALE },
    );
    words_over_the_picture(frame, area, s, r);
    if let Some(y) = &s.tally {
        tally_box(frame, area, s, y);
    }
}

/// Where the picture's pixels land on the cells: its 384 by 224 pixels cover
/// the whole area.
fn col_of(area: Rect, x: i32) -> u16 {
    area.x + (x.clamp(0, 383) * i32::from(area.width) / 384) as u16
}

fn row_of(area: Rect, y: i32) -> u16 {
    area.y + (y.clamp(0, 223) * i32::from(area.height) / 224) as u16
}

/// The words the picture prints in pixels, which four-pixel cells turn to mush:
/// said again as text, each on cells cleared of its own lettering. The lines
/// and columns are the picture's (`render/overworld/arena/lightgallop.rs`: the
/// title card in the sky, the fall and the bell's prompt under it, the keys at
/// the bottom, the bell's name over the bell). Every plate is cleared before any
/// text is laid down, so one never wipes another.
fn words_over_the_picture(frame: &mut Frame, area: Rect, s: &rules::Session, r: &rules::rider::Rider) {
    if area.width < 48 || area.height < 14 {
        return;
    }
    // (centre column in picture pixels, top row in picture pixels, text, colour)
    let mut words: Vec<(i32, i32, String, Color)> = Vec::new();
    if s.elapsed < 75 {
        words.push((192, 36, "MOONWELL ROAD".into(), GOLD));
        words.push((192, 47, "THE DIMMING COMES".into(), PALE));
    }
    if r.down > 0 {
        words.push((192, 59, "THE LANTERN CARRIES YOU".into(), GOLD));
    }
    if s.tally.is_none() && s.bell_t.is_some() {
        words.push((192, 70, "RING THE BELL".into(), GOLD));
    }
    if s.elapsed < 360 {
        words.push((192, 213, "A D RUN   E LEAP   SPACE SPUR".into(), GOLD));
    }
    if s.bell_t.is_none() {
        // The bell's name stands over the bell as tall as the bell is drawn.
        let (bx, by) = level().bell;
        let (sx, sy) = (((bx - r.cam_x) * r.zoom).div_euclid(256), ((by - r.cam_y) * r.zoom).div_euclid(256));
        if (0..384).contains(&sx) && (0..224).contains(&sy) {
            words.push((sx, sy - 72 * r.zoom / 256 - 12, "BELL GATE".into(), GOLD));
        }
    }
    let bounds = |cx: i32, y: i32, text: &str| {
        // The plate the picture lays behind its lettering: six pixels a letter, a pixel round.
        let half = (6 * text.chars().count() as i32 + 1) / 2;
        (col_of(area, cx - half), col_of(area, cx + half), row_of(area, y - 1), row_of(area, y + 8))
    };
    for (cx, y, text, _) in &words {
        let (c0, c1, r0, r1) = bounds(*cx, *y, text);
        frame.render_widget(Clear, Rect::new(c0, r0, c1 - c0 + 1, r1 - r0 + 1).intersection(area));
    }
    for (cx, y, text, fg) in &words {
        let n = text.chars().count() as u16;
        let x = col_of(area, *cx).saturating_sub(n / 2).max(area.x);
        let w = n.min(area.right().saturating_sub(x));
        if w == 0 {
            continue;
        }
        let line = Rect::new(x, row_of(area, *y + 3), w, 1).intersection(area);
        frame.render_widget(Paragraph::new(text.as_str()).style(Style::default().fg(*fg).bg(Color::Black)), line);
    }
}

/// The tally in text, over the picture's own panel (which it covers, so none of
/// its garbled lettering shows round the edges): the party's figures, the
/// lantern bonus on its own line, each knight's score, the rank.
fn tally_box(frame: &mut Frame, area: Rect, s: &rules::Session, y: &rules::Tally) {
    let lvl = level();
    let t = s.elapsed.saturating_sub(y.at);
    let rank = ["C", "B", "A", "S"][usize::from(y.rank.min(3))];
    // A hundred for all six lanterns (`make_tally`): in the points, not in each knight's own line.
    let bonus = 100 * y.posts / lvl.posts.len().max(1) as u32;
    let scores: Vec<String> = y
        .scores
        .iter()
        .map(|(id, v)| format!("{id}{} {v}", if *id == y.best { "*" } else { "" }))
        .collect();
    let table = |name: &str, value: String| format!("{name:<15}{value}");
    let mut lines = vec![
        "MOONWELL ROAD CLEAR".to_string(),
        String::new(),
        table("TIME", format!("{}   PAR {}", clock(y.time), clock(lvl.par))),
        table("SPARKS", format!("{} / {}", y.sparks, lvl.sparks.len())),
        table("BEST CHAIN", y.chain.to_string()),
        table("LANTERNS", format!("{} / {}", y.posts, lvl.posts.len())),
        table("LANTERN BONUS", format!("+{bonus}")),
        table("POINTS", y.points.to_string()),
        if t >= 110 { table("RANK", rank.to_string()) } else { String::new() },
        String::new(),
        "EACH KNIGHT".to_string(),
    ];
    // The scores in one row, or two for a full table on a narrow box.
    let one = scores.join("   ");
    if one.chars().count() + 2 <= usize::from((col_of(area, 327) - col_of(area, 56)).max(36)) {
        lines.push(one);
    } else {
        for half in scores.chunks(3) {
            lines.push(half.join("   "));
        }
    }
    lines.push(String::new());
    lines.push(if t >= 45 { "E RIDE AGAIN   HOLD ESC LEAVE".to_string() } else { String::new() });
    let (x0, x1) = (col_of(area, 56), col_of(area, 327) + 1);
    let (y0, y1) = (row_of(area, 36), row_of(area, 203) + 1);
    let w = (x1 - x0).max(36).min(area.width);
    let h = (y1 - y0).max(lines.len() as u16 + 2).min(area.height);
    let (x, y) = (x0.min(area.right() - w), y0.min(area.bottom() - h));
    let inner = usize::from(w.saturating_sub(2));
    // The table is one block, centred; the headings and the scores are each centred alone.
    let block = 30;
    let spaces = |n: usize| " ".repeat(n);
    let table_names = ["TIME", "SPARKS", "BEST CHAIN", "LANTERNS", "LANTERN BONUS", "POINTS", "RANK"];
    let centred: Vec<String> = lines
        .iter()
        .map(|l| {
            let width = if table_names.iter().any(|n| l.starts_with(n) && l.len() > n.len()) { block } else { l.chars().count() };
            spaces(inner.saturating_sub(width) / 2) + l
        })
        .collect();
    let pad = usize::from(h).saturating_sub(2 + centred.len()) / 2;
    let text = (0..pad).map(|_| String::new()).chain(centred).collect::<Vec<_>>().join("\n");
    let r = Rect::new(x, y, w, h);
    frame.render_widget(Clear, r);
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(GOLD).bg(Color::Black))
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GOLD))),
        r,
    );
}
