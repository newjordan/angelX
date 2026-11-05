//! The wish scroll in a terminal's cells: the window and the browser draw
//! it in pixels over the knight's view (`angel_delve::render::genie`); a
//! terminal that shows the room in half-blocks draws this, its closest
//! version: parchment with rolled ends on the black, dark ink, the genie
//! in half-blocks beside it.
//!
//! Two moments: while a knight writes or picks their wish (`reforge_box`,
//! the cockpit's own prompt), and once the host's run has opened its
//! choices, sealed it, or freed the genie (`offer_box`, from the run).
use super::*;
use crate::drive::together_shooter::genie::{self, Take};
use angel_delve::render::genie as art;

/// Paper, ink and the rolls, from the realm palette's timber.
const PAPER: Color = Color::Rgb(0xe2, 0xcb, 0x8b);
const PAPER_SHADE: Color = Color::Rgb(0xd8, 0xa9, 0x5e);
const INK: Color = Color::Rgb(0x1a, 0x0e, 0x07);
const INK_SOFT: Color = Color::Rgb(0x68, 0x46, 0x29);
const INK_RED: Color = Color::Rgb(0x58, 0x2b, 0x13);
/// Ink gone faint: a ladder that cannot be climbed now.
const INK_FAINT: Color = Color::Rgb(0x9c, 0x80, 0x55);
const ROLL: Color = Color::Rgb(0xa9, 0x76, 0x39);
const KNOB: Color = Color::Rgb(0x58, 0x2b, 0x13);
const BLACK: Color = Color::Rgb(0, 0, 0);

/// What the scroll shows of the knight besides their wish: the wishes
/// held and granted, the genie's gauge, and the genie himself.
pub(crate) struct Scroll {
    pub(crate) held: String,
    pub(crate) granted: String,
    pub(crate) freedom: u32,
    pub(crate) free: bool,
    pub(crate) hint: &'static str,
    pub(crate) tick: u64,
    /// The identity start's question to a knight still unmade, and what an
    /// answer looks like (`identity::Making::heading`).
    pub(crate) ask: Option<(String, &'static str)>,
}

/// The scroll's side of knight `id` in `run`.
pub(crate) fn scroll_of(run: &Run, id: u32) -> Option<Scroll> {
    let hero = run.players.get(&id)?;
    let mut held = format!(
        "{} wish{}",
        hero.wishes,
        if hero.wishes == 1 { "" } else { "es" }
    );
    if hero.great_wishes > 0 {
        held.push_str(&format!(" + {} super", hero.great_wishes));
    }
    // Caught under a shooting star (`starfall`).
    if hero.star_wish {
        held.push_str(" + a star's");
    }
    let granted: Vec<String> = hero
        .deck
        .iter()
        .filter(|c| c.starts_with("wish-") && !c.starts_with("wish-kit"))
        .filter_map(|c| run.book.get(c).map(|card| card.name.clone()))
        .collect();
    Some(Scroll {
        held,
        granted: granted.join(", "),
        freedom: run.genie.freedom.min(genie::FREEDOM),
        free: run.genie.free(),
        hint: run.genie.hint(),
        tick: run.tick,
        ask: hero
            .making(&run.book)
            .and_then(|m| Some((m.heading()?, m.asks().first()?.hint()))),
    })
}

/// One line of ink on the paper.
fn ink(frame: &mut Frame, x: u16, y: u16, w: u16, text: &str, style: Style) {
    frame.render_widget(
        Paragraph::new(fit_text(text, w)).style(style.bg(PAPER)),
        Rect::new(x, y, w, 1),
    );
}

fn centred(frame: &mut Frame, area: Rect, y: u16, text: &str, style: Style) {
    frame.render_widget(
        Paragraph::new(fit_text(text, area.width))
            .alignment(ratatui::layout::Alignment::Center)
            .style(style.bg(PAPER)),
        Rect::new(area.x, y, area.width, 1),
    );
}

/// Parchment over `area` with a roll along its top and its foot, knobs
/// at their ends; the rows between, inset, for the words.
fn paper(frame: &mut Frame, area: Rect) -> Rect {
    frame.render_widget(Clear, area);
    let buf = frame.buffer_mut();
    let right = area.right().saturating_sub(1);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            let end = x == area.x || x == right;
            let (top, foot) = (y == area.y, y + 1 == area.bottom());
            let (top2, foot2) = (y == area.y + 1, y + 2 == area.bottom());
            if end && (top || top2 || foot || foot2) {
                // The knobs.
                cell.set_char(if top || foot2 { '▄' } else { '▀' })
                    .set_fg(KNOB)
                    .set_bg(BLACK);
            } else if end {
                cell.set_char(' ').set_bg(BLACK);
            } else if top {
                cell.set_char('▄').set_fg(ROLL).set_bg(BLACK);
            } else if top2 {
                cell.set_char('▀').set_fg(ROLL).set_bg(PAPER);
            } else if foot2 {
                cell.set_char('▄').set_fg(ROLL).set_bg(PAPER);
            } else if foot {
                cell.set_char('▀').set_fg(ROLL).set_bg(BLACK);
            } else {
                // Shaded toward the torn sides.
                let side = x == area.x + 1 || x + 2 == area.right();
                cell.set_char(' ')
                    .set_bg(if side { PAPER_SHADE } else { PAPER });
            }
        }
    }
    Rect::new(
        area.x + 3,
        area.y + 2,
        area.width.saturating_sub(6),
        area.height.saturating_sub(4),
    )
}

/// The genie in half-blocks at `area`'s foot, his clear pixels the black
/// world.
fn genie_cells(frame: &mut Frame, area: Rect, img: &Img) {
    let cols = (img.w as u16).min(area.width);
    let rows = ((img.h as u16).div_ceil(2)).min(area.height);
    let x0 = area.x + (area.width - cols) / 2;
    let y0 = area.bottom() - rows;
    let px = |x: u16, y: u16| {
        img.get(i32::from(x), i32::from(y))
            .map(|[r, g, b]| Color::Rgb(r, g, b))
    };
    let buf = frame.buffer_mut();
    for cy in 0..rows {
        for cx in 0..cols {
            let (top, low) = (px(cx, cy * 2), px(cx, cy * 2 + 1));
            if top.is_none() && low.is_none() {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x0 + cx, y0 + cy)) {
                cell.set_char('▀')
                    .set_fg(top.unwrap_or(BLACK))
                    .set_bg(low.unwrap_or(BLACK));
            }
        }
    }
}

/// The gauge's line: its name, the bar, how full.
fn gauge(frame: &mut Frame, x: u16, y: u16, w: u16, s: &Scroll) {
    let label = if s.free {
        "The genie is free "
    } else {
        "Free the genie "
    };
    let count = format!(" {}/{}", s.freedom, genie::FREEDOM);
    let bar = w.saturating_sub((label.chars().count() + count.chars().count()) as u16 + 2);
    let full = if s.free {
        bar
    } else {
        (u32::from(bar) * s.freedom / genie::FREEDOM) as u16
    };
    let line = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(label, Style::new().fg(INK).add_modifier(Modifier::BOLD)),
        ratatui::text::Span::styled("▕", Style::new().fg(INK_SOFT)),
        ratatui::text::Span::styled("█".repeat(usize::from(full)), Style::new().fg(INK_RED)),
        ratatui::text::Span::styled(
            "░".repeat(usize::from(bar - full)),
            Style::new().fg(PAPER_SHADE),
        ),
        ratatui::text::Span::styled("▏", Style::new().fg(INK_SOFT)),
        ratatui::text::Span::styled(count, Style::new().fg(INK)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::new().bg(PAPER)),
        Rect::new(x, y, w, 1),
    );
}

/// Where the scroll and the genie go in `room`: the genie's column at the
/// left, the paper beside it, `h` rows tall, centred.
fn place(room: Rect, h: u16) -> Option<(Rect, Rect)> {
    let w = room.width.saturating_sub(4).min(78);
    let h = h.min(room.height);
    if w < 36 || h < 10 {
        return None;
    }
    let genie_w = if w >= 64 { 18 } else { 0 };
    let x = room.x + (room.width - w) / 2;
    let y = room.y + (room.height - h) / 2;
    Some((
        Rect::new(x, y, genie_w, h),
        Rect::new(x + genie_w, y, w - genie_w, h),
    ))
}

/// Held ladders, two per page so their next-tier lines have room.
fn upgrade_rows(frame: &mut Frame, inner: Rect, y: u16, wish: &Wish) {
    use crate::app::control::UPGRADES_PER_PAGE;
    let bold = |c: Color| Style::new().fg(c).add_modifier(Modifier::BOLD);
    ink(
        frame,
        inner.x + 1,
        y,
        inner.width.saturating_sub(2),
        genie::UPGRADE_HEAD,
        bold(INK),
    );
    let page = wish
        .upgrade_page
        .min(wish.upgrades.len().saturating_sub(1) / UPGRADES_PER_PAGE);
    let shown = wish
        .upgrades
        .iter()
        .skip(page * UPGRADES_PER_PAGE)
        .take(UPGRADES_PER_PAGE);
    for (i, up) in shown.enumerate() {
        let color = if up.available { INK } else { INK_FAINT };
        let key = if up.available {
            format!("{} ", i + 1)
        } else {
            "  ".into()
        };
        let top = format!(
            "{key}{}  {}{}",
            up.name,
            up.tiers(),
            if up.next.is_none() { "  MAX" } else { "" }
        );
        ink(
            frame,
            inner.x + 1,
            y + 1 + i as u16 * 2,
            inner.width.saturating_sub(2),
            &top,
            bold(color),
        );
        ink(
            frame,
            inner.x + 3,
            y + 2 + i as u16 * 2,
            inner.width.saturating_sub(4),
            &up.says,
            Style::new().fg(if up.available { INK_SOFT } else { INK_FAINT }),
        );
    }
    let shown = wish
        .upgrades
        .len()
        .saturating_sub(page * UPGRADES_PER_PAGE)
        .min(UPGRADES_PER_PAGE);
    let row = y + 1 + shown as u16 * 2;
    if let Some(note) = wish.upgrade_note {
        ink(
            frame,
            inner.x + 1,
            row,
            inner.width.saturating_sub(2),
            note,
            Style::new().fg(INK_FAINT),
        );
    }
    if wish.upgrades.len() > UPGRADES_PER_PAGE {
        let pages = wish.upgrades.len().div_ceil(UPGRADES_PER_PAGE);
        ink(
            frame,
            inner.x + 1,
            row + u16::from(wish.upgrade_note.is_some()),
            inner.width.saturating_sub(2),
            &format!("PgUp/PgDn: more held · {}/{pages}", page + 1),
            Style::new().fg(INK_SOFT),
        );
    }
}

/// The scroll while a wish is written or picked: the ladders held to climb,
/// the words, the known wishes to pick from, and the wishes held, the gauge
/// and the genie.
pub(in crate::ui::viz) fn reforge_box(frame: &mut Frame, room: Rect, wish: &Wish) {
    use crate::app::control::UPGRADES_PER_PAGE;
    let ups = wish.upgrades.len().min(UPGRADES_PER_PAGE) as u16;
    let block = 2
        + 2 * ups
        + u16::from(wish.upgrade_note.is_some())
        + u16::from(wish.upgrades.len() > UPGRADES_PER_PAGE);
    // Give the held ladders and typed line room before the optional book
    // and gauge, including in an 80-column terminal.
    let side = if room.height >= 15 + block {
        u16::from(wish.scroll.is_some()) * 3
    } else {
        0
    };
    let list = wish
        .known
        .len()
        .min(usize::from(room.height.saturating_sub(13 + block + side)))
        .min(8) as u16;
    let Some((genie, area)) = place(room, 12 + block + list + u16::from(list > 0) + side) else {
        return;
    };
    let inner = paper(frame, area);
    let mut y = inner.y;
    centred(
        frame,
        inner,
        y,
        "THE WISH SCROLL",
        Style::new().fg(INK).add_modifier(Modifier::BOLD),
    );
    y += 1;
    centred(frame, inner, y, "~ ~ ~", Style::new().fg(INK_SOFT));
    y += 2;
    upgrade_rows(frame, inner, y, wish);
    y += block;
    if wish.sanctuary {
        let (weapon, guard) = if wish.part.starts_with("off") {
            ("‹ your weapon ›", "your guard")
        } else {
            ("your weapon", "‹ your guard ›")
        };
        centred(
            frame,
            inner,
            y,
            &format!("Any wish, for {weapon} {guard} (Tab), or a known one"),
            Style::new().fg(INK_SOFT),
        );
    } else if let Some((ask, hint)) = wish.scroll.as_ref().and_then(|s| s.ask.as_ref()) {
        // A knight still a shape: the scroll asks who they are.
        centred(
            frame,
            inner,
            y,
            ask,
            Style::new().fg(INK_RED).add_modifier(Modifier::BOLD),
        );
        centred(frame, inner, y + 1, hint, Style::new().fg(INK_SOFT));
    } else {
        centred(
            frame,
            inner,
            y,
            "Say a wish, or pick one: the genie offers how",
            Style::new().fg(INK_SOFT),
        );
    }
    y += 2;
    let picked = wish
        .pick
        .as_ref()
        .and_then(|p| wish.known.iter().find(|k| &k.0 == p))
        .map(|k| k.1.as_str());
    let shown = if wish.words.is_empty() {
        picked.unwrap_or("")
    } else {
        wish.words
    };
    centred(
        frame,
        inner,
        y,
        &format!("“{shown}▏”"),
        Style::new().fg(INK).add_modifier(Modifier::BOLD),
    );
    y += 2;
    for (index, name, says) in wish.known.iter().take(usize::from(list)) {
        let on = wish.pick.as_ref() == Some(index);
        let style = if on {
            Style::new().fg(INK_RED).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(INK_SOFT)
        };
        ink(
            frame,
            inner.x + 1,
            y,
            inner.width.saturating_sub(2),
            &format!("{} {name} · {says}", if on { "▸" } else { " " }),
            style,
        );
        y += 1;
    }
    if list > 0 {
        y += 1;
    }
    if let Some(s) = wish.scroll.as_ref().filter(|_| side > 0) {
        ink(
            frame,
            inner.x,
            y,
            inner.width,
            &format!(
                "Held: {}{}{}",
                s.held,
                if s.granted.is_empty() { "" } else { " · " },
                s.granted
            ),
            Style::new().fg(INK_SOFT),
        );
        gauge(frame, inner.x, y + 1, inner.width, s);
        ink(
            frame,
            inner.x,
            y + 2,
            inner.width,
            s.hint,
            Style::new().fg(INK_SOFT),
        );
        y += 3;
        if genie.width > 0 {
            let img = art::figure(
                art::Mood::Idle,
                s.tick,
                (i32::from(genie.height) * 2 - 4).min(26),
            );
            genie_cells(frame, genie, &img);
        }
    }
    centred(
        frame,
        inner,
        y.min(inner.bottom().saturating_sub(1)),
        "1/2 upgrade · ↑/↓ known · words · Enter · Esc",
        Style::new().fg(INK_SOFT),
    );
    if let Some(seal) = wish.confirm {
        super::wish_confirm(frame, room, wish, seal);
    }
}

/// The scroll the host's run opened for knight `id`: its choices, the
/// wish just sealed, or the genie freed (every knight sees that).
pub(crate) fn offer_box(frame: &mut Frame, room: Rect, run: &Run, id: u32) {
    let Some(mood) = art::mood(run, id) else {
        return;
    };
    let Some(s) = scroll_of(run, id) else {
        return;
    };
    let hero = &run.players[&id];
    let freed = run.genie.freed.as_ref().filter(|_| art::freeing(run));
    let rows = match (&hero.offer, freed) {
        (_, Some(_)) => 9,
        (Some(offer), _) => {
            9 + 2 * offer.choices.len() as u16 + 4 + u16::from(offer.target.is_some())
        }
        (None, _) => 13,
    };
    let Some((genie, area)) = place(room, rows) else {
        return;
    };
    let inner = paper(frame, area);
    let mut y = inner.y;
    let bold = |c: Color| Style::new().fg(c).add_modifier(Modifier::BOLD);
    if let Some((_, who)) = freed {
        centred(frame, inner, y, "THE GENIE IS FREE!", bold(INK));
        centred(
            frame,
            inner,
            y + 2,
            &format!("{who} set him free."),
            Style::new().fg(INK),
        );
        centred(
            frame,
            inner,
            y + 3,
            "He stays as your friend:",
            Style::new().fg(INK_SOFT),
        );
        centred(
            frame,
            inner,
            y + 4,
            "every floor now gives 3 wishes",
            Style::new().fg(INK_SOFT),
        );
    } else {
        centred(frame, inner, y, "THE WISH SCROLL", bold(INK));
        if let Some(ask) = hero.offer.as_ref().and_then(|o| o.ask.as_ref()) {
            centred(frame, inner, y + 1, ask, bold(INK_RED));
        }
        y += 2;
        match &hero.offer {
            Some(offer) => {
                let label = if offer.great {
                    "Super wish: "
                } else {
                    "You wish: "
                };
                let line = ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(label, Style::new().fg(INK_SOFT)),
                    ratatui::text::Span::styled(offer.said.clone(), bold(INK)),
                ]);
                frame.render_widget(
                    Paragraph::new(line).style(Style::new().bg(PAPER)),
                    Rect::new(inner.x, y, inner.width, 1),
                );
                y += 2;
                for (i, c) in offer.choices.iter().enumerate() {
                    let genies = matches!(c.take, Take::Deal | Take::Whim);
                    let head = ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            format!(" {} ", i + 1),
                            Style::new()
                                .fg(PAPER)
                                .bg(INK_RED)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ratatui::text::Span::styled(
                            format!(" {}", c.head),
                            bold(if genies { INK_RED } else { INK }),
                        ),
                        ratatui::text::Span::styled(
                            if c.cost.is_empty() {
                                String::new()
                            } else {
                                format!("  · {}", c.cost)
                            },
                            Style::new().fg(INK_RED),
                        ),
                    ]);
                    let bg = if genies { PAPER_SHADE } else { PAPER };
                    frame.render_widget(
                        Paragraph::new(head).style(Style::new().bg(bg)),
                        Rect::new(inner.x, y, inner.width, 1),
                    );
                    frame.render_widget(
                        Paragraph::new(fit_text(&format!("    {}", c.gives), inner.width))
                            .style(Style::new().fg(INK_SOFT).bg(bg)),
                        Rect::new(inner.x, y + 1, inner.width, 1),
                    );
                    y += 2;
                }
                // A summoner's scroll says whom the wish is for, and 4
                // turns it (`dead_wishes`).
                if let Some(on) = offer.target {
                    use crate::drive::together_shooter::dead_wishes::Target;
                    let mut spans = vec![ratatui::text::Span::styled(
                        " For: ",
                        Style::new().fg(INK_SOFT),
                    )];
                    for t in [Target::You, Target::Dead, Target::Both] {
                        let style = if t == on {
                            bold(INK).add_modifier(Modifier::UNDERLINED)
                        } else {
                            Style::new().fg(INK_SOFT)
                        };
                        spans.push(ratatui::text::Span::styled(t.word(), style));
                        spans.push(ratatui::text::Span::raw("  "));
                    }
                    spans.push(ratatui::text::Span::styled("[4]", Style::new().fg(INK_RED)));
                    frame.render_widget(
                        Paragraph::new(ratatui::text::Line::from(spans))
                            .style(Style::new().bg(PAPER)),
                        Rect::new(inner.x, y, inner.width, 1),
                    );
                    y += 1;
                }
            }
            None => {
                let line = run.genie.last.as_ref().map_or("", |l| l.line.as_str());
                centred(
                    frame,
                    inner,
                    y,
                    "The wish is sealed:",
                    Style::new().fg(INK_SOFT),
                );
                centred(frame, inner, y + 1, line, bold(INK));
                y += 3;
            }
        }
        y += 1;
        ink(
            frame,
            inner.x,
            y,
            inner.width,
            &format!(
                "Held: {}{}{}",
                s.held,
                if s.granted.is_empty() { "" } else { " · " },
                s.granted
            ),
            Style::new().fg(INK_SOFT),
        );
        gauge(frame, inner.x, y + 1, inner.width, &s);
        let keys = match &hero.offer {
            Some(o) if o.choices.len() == 1 => "1 seals it · Esc rolls it up",
            Some(o) if o.target.is_some() => "1 2 3 choose · 4 for whom · Esc rolls it up",
            Some(_) => "1 2 3 choose · Esc rolls it up",
            None => s.hint,
        };
        ink(
            frame,
            inner.x,
            y + 2,
            inner.width,
            keys,
            Style::new().fg(INK_SOFT),
        );
    }
    if genie.width > 0 {
        let img = art::figure(mood, s.tick, (i32::from(genie.height) * 2 - 2).min(26));
        genie_cells(frame, genie, &img);
    }
}
