//! Cinder Stack's frame: the viewer's own corner (name, next pair, clock, pot), a live little board
//! for every rival, the kiln's crack meter, the race rail that shows the whole table's cracks at
//! once, and the one rule of the screen: every line of the 5x7 font is a `Word` made here, so what
//! a test reads off the screen is what was planned. Five-by-seven words live only in the two
//! gutters and the right panel; every banner and big number the kit draws lives in the well
//! (x 90 to 246) or over the kiln, so the two never meet.
use super::super::super::ink::{Img, bayer, text_width};
use super::super::knights;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::{BX, BY, CH, CW, fx};
use crate::rules::cinder::{Board, COLS, CRACKS, GOLD_PER_SEAT, LENGTH, ROWS, Session};
use crate::rules::cinder_show::{Cry, Kind};
use crate::rules::{HZ, Run};

pub(super) type Rect = (i32, i32, i32, i32);

pub(super) const W: i32 = 384;
pub(super) const H: i32 = 224;
pub(super) const KILN_X: i32 = 311;
pub(super) const GUTTER_X: i32 = 45;
pub(super) const METER: Rect = (264, 60, 94, 5);
pub(super) const RAIL: (i32, i32, i32) = (264, 358, 186);
pub(super) const FLY: u32 = 16;
pub(super) const TOAST: u32 = 54;

pub(super) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub(super) fn inside(r: Rect) -> bool {
    r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= W && r.1 + r.3 <= H
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Word {
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub ink: char,
}

fn word(x: i32, y: i32, text: impl Into<String>, ink: char) -> Word {
    Word { x, y, text: text.into(), ink }
}

fn centre(cx: i32, y: i32, text: impl Into<String>, ink: char) -> Word {
    let text = text.into();
    Word { x: cx - text_width(&text) / 2, y, text, ink }
}

pub(super) fn called(run: &Run, id: u32, me: u32, take: usize) -> String {
    if id == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take).collect()
}

pub(super) fn tag(run: &Run, id: u32, take: usize) -> String {
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" || full.starts_with("KNIGHT ") {
        return format!("K{id}");
    }
    full.chars().take(take).collect()
}

pub(super) fn ink_of(run: &Run, id: u32) -> char {
    let colours = run.players.get(&id).map_or(knights::seat_colours(id), |h| {
        if h.colours().is_empty() { knights::seat_colours(id) } else { h.colours() }
    });
    colours.iter().find(|&&(from, _)| from == '7').map_or('7', |&(_, to)| to)
}

pub(super) fn elapsed(s: &Session) -> u32 {
    LENGTH - s.left
}

pub(super) fn shown(b: &Board, bclock: u32) -> u16 {
    let flying: u32 = b
        .show
        .marks
        .iter()
        .filter_map(|m| match m.kind {
            Kind::Pop { gain, .. } if m.age(bclock) < FLY => Some(u32::from(gain)),
            _ => None,
        })
        .sum();
    (u32::from(b.cracks).saturating_sub(flying)) as u16
}

pub(super) fn landed(b: &Board, bclock: u32) -> Option<(u32, u8)> {
    b.show
        .marks
        .iter()
        .filter_map(|m| match m.kind {
            Kind::Pop { gain, .. } => m.age(bclock).checked_sub(FLY).filter(|&a| a < 26).map(|a| (a, gain)),
            _ => None,
        })
        .min_by_key(|&(a, _)| a)
}

pub(super) fn pot(s: &Session) -> u32 {
    GOLD_PER_SEAT * s.boards.len() as u32
}

pub(super) fn toast(run: &Run, s: &Session, id: u32) -> Option<(String, String)> {
    let now = elapsed(s);
    let call = s
        .calls
        .list
        .iter()
        .filter(|c| c.who != id && c.age(now) < TOAST)
        .max_by_key(|c| (c.cry.rank(), c.born))?;
    let title = match call.cry {
        Cry::Lead => "TAKES LEAD".to_string(),
        Cry::Chain(n) => format!("CHAIN X{n}"),
        Cry::Stage(1) => "KILN CRACKS".into(),
        Cry::Stage(_) => "KILN SPLITS".into(),
        Cry::Point => "MATCH POINT".into(),
        Cry::Cough => "KILN COUGH".into(),
        Cry::Cracked => "KILN CRACKED".into(),
        Cry::Close => "NECK AND NECK".into(),
    };
    Some((title, called(run, call.who, id, 18)))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Tile {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub s: i32,
    pub take: usize,
}

impl Tile {
    pub fn board(&self) -> (i32, i32) {
        (self.x + (self.w - COLS as i32 * self.s) / 2, self.y + 8)
    }
}

pub(super) fn tiles(s: &Session, me: u32) -> Vec<Tile> {
    let rivals: Vec<u32> = s.boards.keys().copied().filter(|&id| id != me).collect();
    let (cols, w, size, take) = match rivals.len() {
        0 => return Vec::new(),
        1 => (1, 86, 6, 12),
        2 => (2, 43, 6, 7),
        3 | 4 => (2, 43, 4, 7),
        _ => (3, 28, 4, 4),
    };
    let tall = 17 + ROWS as i32 * size;
    rivals
        .into_iter()
        .enumerate()
        .map(|(n, id)| Tile {
            id,
            x: 2 + (n as i32 % cols) * w,
            y: 108 + (n as i32 / cols) * tall,
            w,
            s: size,
            take,
        })
        .collect()
}

fn time_ink(s: &Session) -> char {
    if s.left <= 10 * HZ && s.left > 0 && s.left / 8 % 2 == 0 { 'R' } else { 'i' }
}

const KEYS_Y: i32 = 201;

pub(super) fn words(run: &Run, s: &Session, id: u32, bclock: u32) -> Vec<Word> {
    let b = &s.boards[&id];
    let now = elapsed(s);
    let mut out = vec![
        centre(GUTTER_X, 8, called(run, id, id, 9), 'u'),
        centre(GUTTER_X, 48, "NEXT", 'G'),
        centre(GUTTER_X, 87, format!("{}S", s.left.div_ceil(HZ)), time_ink(s)),
        centre(GUTTER_X, 99, format!("POT {}G", pot(s)), 't'),
        centre(KILN_X, 7, "CINDER STACK", 't'),
    ];
    if let Some((title, who)) = toast(run, s, id) {
        out.push(centre(KILN_X, 21, title, 'T'));
        out.push(centre(KILN_X, 31, who, 'i'));
    } else if now < 20 * HZ {
        out.push(centre(KILN_X, 22, "MATCH THREE POTS", 'i'));
    }
    out.push(centre(KILN_X, 44, "CROCK", 't'));
    let counter = format!("CRACKS {}/{}", shown(b, bclock), CRACKS);
    let cx = KILN_X - text_width(&counter) / 2;
    out.push(word(cx, 177, counter, 'u'));
    if let Some((age, gain)) = landed(b, bclock) {
        out.push(word(KILN_X + 40, 177, format!("+{gain}"), if age < 8 { 'H' } else { 'T' }));
    }
    out.push(centre(280, KEYS_Y, "F TURN", 'G'));
    out.push(centre(342, KEYS_Y, "E DROP", 'G'));
    if s.to_beat > 0 {
        out.push(centre(KILN_X, 211, format!("BEST {}", clock_text(s.to_beat)), 'G'));
    }
    for t in tiles(s, id) {
        let tag = tag(run, t.id, t.take);
        let name = word(t.x + (t.w - text_width(&tag)) / 2, t.y, tag, ink_of(run, t.id));
        out.push(name);
        let n = s.boards[&t.id].cracks.to_string();
        let (_, by) = t.board();
        out.push(word(t.x + (t.w - text_width(&n)) / 2, by + ROWS as i32 * t.s + 1, n, 'i'));
    }
    out
}

pub(super) fn clock_text(ticks: u32) -> String {
    format!("{}.{}S", ticks / HZ, ticks % HZ * 10 / HZ)
}

fn pot_ink(c: u8) -> (char, char) {
    match c {
        1 => ('Q', 'z'),
        2 => ('R', 'o'),
        3 => ('m', 'Y'),
        _ => ('P', 'o'),
    }
}

pub(super) fn paint_tiles(out: &mut Img, run: &Run, s: &Session, id: u32, tick: u32) {
    let best = s.boards.values().map(|b| b.cracks).max().unwrap_or(0);
    let leaders = s.boards.values().filter(|b| b.cracks == best).count();
    for t in tiles(s, id) {
        let b = &s.boards[&t.id];
        let ink = ink_of(run, t.id);
        let (bx, by) = t.board();
        let (pw, ph) = (COLS as i32 * t.s, ROWS as i32 * t.s);
        let hot = b.show.marks.iter().any(|m| matches!(m.kind, Kind::Pop { .. } | Kind::Stage { .. }) && m.age(b.clock) < 10);
        out.rect(t.x + 1, t.y + 7, t.w - 2, ph + 3, 's');
        out.rect(t.x + 1, t.y + 7, t.w - 2, 1, ink);
        out.rect(bx, by, pw, ph, 'k');
        for y in 0..ROWS {
            for x in 0..COLS {
                let c = b.cells[y][x];
                if c == 0 {
                    continue;
                }
                let (dark, light) = pot_ink(c);
                let white = b.pop[y][x] && b.flash > 0;
                let fill = if white { if b.flash % 2 == 0 { 'H' } else { 'i' } } else { dark };
                let (px, py) = (bx + x as i32 * t.s, by + y as i32 * t.s);
                out.rect(px, py, t.s, t.s, fill);
                if t.s >= 4 && !white {
                    out.put(px, py, light);
                }
            }
        }
        if b.flash == 0 && b.retry == 0 && s.left > 0 {
            let colours = b.colours();
            for (n, (x, y)) in Board::pair_at(b.x, b.y, b.turn).into_iter().enumerate() {
                let (dark, light) = pot_ink(colours[n]);
                let (px, py) = (bx + i32::from(x) * t.s, by + i32::from(y) * t.s);
                out.rect(px, py, t.s, t.s, dark);
                out.put(px, py, light);
            }
        }
        if b.tell > 0 && b.retry == 0 {
            let x = bx + i32::from(b.column) * t.s;
            for y in 0..ph {
                if bayer(x, y) < 0.45 {
                    out.put(x + (y & 1), by + y, 'R');
                }
            }
        }
        if b.retry > 0 {
            for y in 0..ph {
                for x in 0..pw {
                    if bayer(x + tick as i32, y) < 0.3 {
                        out.put(bx + x, by + y, 'R');
                    }
                }
            }
        }
        for m in &b.show.marks {
            let age = m.age(b.clock);
            if let Kind::Pop { by: popped, chain, .. } = m.kind {
                if age < 14 {
                    let (cx, cy) = fx::centroid(&popped);
                    let at = (bx as f32 + (cx - BX as f32) / CW as f32 * t.s as f32, by as f32 + (cy - BY as f32) / CH as f32 * t.s as f32);
                    let mut pen = Pen::new(out, 160);
                    pen.shock(at, 2.0 + age as f32 * (1.2 + f32::from(chain) * 0.3), 1.2, 1.0, ramps::GOLD, age as f32 / 14.0);
                    let sparks = Spray { gravity: 0.05, ..Spray::round(m.born ^ t.id, 6, at, (0.8, 2.2), (6.0, 13.0)) };
                    sparks.each(age as f32, |mote| pfx::mote(&mut pen, &mote, Shape::Spark, ramps::GOLD, 1, u64::from(tick)));
                }
            }
            match m.kind {
                Kind::Pop { chain, .. } if chain >= 2 && age < 30 => {
                    for k in 0..chain.min(6) {
                        out.rect(t.x + t.w - 4, by + ph - 3 - i32::from(k) * 4, 3, 3, if age < 6 { 'H' } else { 't' });
                    }
                }
                Kind::Stage { .. } if age < 24 && age / 3 % 2 == 0 => out.frame(t.x, t.y + 6, t.w, ph + 5, 'T'),
                _ => {}
            }
        }
        if b.cracks == best && leaders == 1 && best >= crate::rules::cinder_show::LEAD_MIN {
            out.frame(t.x, t.y + 6, t.w, ph + 5, 't');
        }
        if hot {
            out.frame(t.x, t.y + 6, t.w, ph + 5, ink);
        }
    }
}

pub(super) fn rail_x(cracks: u16) -> i32 {
    RAIL.0 + i32::from(cracks.min(CRACKS)) * (RAIL.1 - RAIL.0) / i32::from(CRACKS)
}

pub(super) fn paint_meters(out: &mut Img, run: &Run, s: &Session, id: u32, bclock: u32, tick: u32) {
    let b = &s.boards[&id];
    let (mx, my, mw, mh) = METER;
    out.rect(mx, my, mw, mh, 's');
    let left = |c: u16| (mw - 2) * i32::from(CRACKS.saturating_sub(c.min(CRACKS))) / i32::from(CRACKS);
    let (now_w, was_w) = (left(shown(b, bclock)), left(b.cracks));
    out.rect(mx + 1, my + 1, now_w, mh - 2, 'r');
    if was_w < now_w {
        out.rect(mx + 1 + was_w, my + 1, now_w - was_w, mh - 2, if tick / 2 % 2 == 0 { 'H' } else { 'T' });
    }
    for stage in 1..3 {
        out.rect(mx + 1 + (mw - 2) * stage / 3, my - 1, 1, mh + 2, 'u');
    }
    let (x0, x1, top) = RAIL;
    let n = s.boards.len() as i32;
    let pitch = (12 / n.max(1)).clamp(2, 4);
    let height = n * pitch;
    out.rect(x0 - 4, top - 1, x1 - x0 + 9, height + 2, 's');
    for stage in 1..=3u16 {
        out.rect(rail_x(stage * 12), top - 2, 1, height + 4, if stage == 3 { 't' } else { 'u' });
    }
    let best = s.boards.values().map(|b| b.cracks).max().unwrap_or(0);
    let leaders = s.boards.values().filter(|b| b.cracks == best).count();
    for (lane, (&who, board)) in s.boards.iter().enumerate() {
        let y = top + lane as i32 * pitch;
        let at = if who == id { shown(board, bclock) } else { board.cracks };
        let ink = ink_of(run, who);
        let mine = who == id;
        out.rect(x0, y, rail_x(at) - x0, pitch - 1, ink);
        let hop = board.show.marks.iter().any(|m| matches!(m.kind, Kind::Pop { .. }) && m.age(board.clock) < 6);
        let head = rail_x(at);
        out.rect(head - 1, y - i32::from(mine || hop), 3, pitch - 1 + 2 * i32::from(mine || hop), if hop { 'H' } else { 'T' });
        if mine {
            out.rect(x0 - 3, y, 2, pitch - 1, ink);
            out.put(x0 - 2, y - 1, ink);
            out.put(x0 - 2, y + pitch - 1, ink);
        }
        if board.cracks == best && leaders == 1 && best >= crate::rules::cinder_show::LEAD_MIN {
            let (cx, cy) = (x1 + 6, y + (pitch - 1) / 2);
            out.rect(cx - 2, cy, 5, 2, 't');
            for k in [-2, 0, 2] {
                out.put(cx + k, cy - 1, 't');
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    static CLAIMS: std::cell::RefCell<Vec<(&'static str, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(super) fn claim(name: &'static str, layer: &Img, at: (i32, i32)) {
    #[cfg(test)]
    {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..layer.h {
            for x in 0..layer.w {
                if layer.get(x, y).is_some() {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        if x1 > x0 {
            CLAIMS.with(|c| c.borrow_mut().push((name, (at.0 + x0, at.1 + y0, x1 - x0, y1 - y0))));
        }
    }
    #[cfg(not(test))]
    let _ = (name, layer, at);
}

#[cfg(test)]
pub(super) fn claimed<R>(f: impl FnOnce() -> R) -> (R, Vec<(&'static str, Rect)>) {
    CLAIMS.with(|c| c.borrow_mut().clear());
    let out = f();
    (out, CLAIMS.with(|c| std::mem::take(&mut *c.borrow_mut())))
}

pub(super) fn cell(x: usize, y: usize) -> (i32, i32) {
    (BX + x as i32 * CW, BY + y as i32 * CH)
}
