//! Brasswake's frame: the top bar (hearts, work, the combo, the clock and the pot), the bottom bar
//! (the keys, a race track of every knight's aisle, the table's standings) and the rule that every
//! box of words on the screen is claimed here. The words are `Word`s drawn with the 5x7 font, and
//! the pixel text of the moments (`moments`) claims its boxes the same way, so what a test checks
//! is what is drawn.
use super::super::super::ink::{Img, bayer, text_width};
use super::super::juice;
use super::super::knights;
use super::moments;
use crate::rules::brasswake::{
    Beat, GOLD_PER_SEAT, LENGTH, Lane, Session, WORK_PER_RECORD,
    show::{Cry, Kind},
};
use crate::rules::{HZ, Run};

pub(super) type Rect = (i32, i32, i32, i32);

pub(super) const W: i32 = 384;
pub(super) const H: i32 = 224;
pub(super) const TOP: i32 = 34;
pub(super) const BOTTOM: i32 = 180;

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

impl Word {
    pub fn rect(&self) -> Rect {
        (self.x, self.y, text_width(&self.text), 7)
    }
}

fn word(x: i32, y: i32, text: impl Into<String>, ink: char) -> Word {
    Word { x, y, text: text.into(), ink }
}

fn right(edge: i32, y: i32, text: impl Into<String>, ink: char) -> Word {
    let text = text.into();
    Word { x: edge - text_width(&text), y, text, ink }
}

fn centre(y: i32, text: impl Into<String>, ink: char) -> Word {
    let text = text.into();
    Word { x: (W - text_width(&text)) / 2, y, text, ink }
}

pub(super) fn short(name: &str) -> String {
    let up = name.to_uppercase();
    match up.strip_prefix("KNIGHT ") {
        Some(n) if !n.is_empty() => format!("K{}", n.chars().take(2).collect::<String>()),
        _ => up.chars().take(4).collect(),
    }
}

pub(super) fn name_of(run: &Run, id: u32, n: usize) -> String {
    run.players.get(&id).map_or("KNIGHT", |h| h.name.as_str()).to_uppercase().chars().take(n).collect()
}

pub(super) fn pip(id: u32) -> char {
    knights::seat_mark(id)
}

pub(super) fn ranked(s: &Session) -> Vec<u32> {
    let mut order: Vec<u32> = s.lanes.keys().copied().collect();
    order.sort_by_key(|&id| (s.score(id), id));
    order
}

const COLS: [i32; 2] = [232, 306];
const ROWS: [i32; 3] = [187, 197, 207];
const KEYS_Y: i32 = 184;
const TRACK: (i32, i32, i32) = (14, 218, 212);

pub(super) fn entry(n: usize) -> (i32, i32) {
    (COLS[n % 2], ROWS[(n / 2).min(2)])
}

pub(super) fn shown(d: &Lane, now: u32) -> u32 {
    d.score.saturating_sub(moments::in_flight(d, now))
}

pub(super) fn keys(run: &Run, d: &Lane, elapsed: u32) -> Vec<Word> {
    let _ = run;
    let ink = if elapsed < 20 * HZ { 'i' } else { 'J' };
    let danger = d.foes.iter().any(|e| e.beat == Beat::Tell && e.danger.contains(d.x, d.y));
    let (e_text, e_ink) = if d.holding {
        ("E THROW", 'T')
    } else if danger {
        ("HOLD E GUARD", 'R')
    } else {
        ("E GRAB", ink)
    };
    vec![word(8, KEYS_Y, "ARROWS WALK", ink), word(84, KEYS_Y, "F PUNCH", ink), word(134, KEYS_Y, e_text, e_ink)]
}

pub(super) fn words(run: &Run, s: &Session, id: u32) -> Vec<Word> {
    let d = &s.lanes[&id];
    let now = run.tick as u32;
    let elapsed = LENGTH - s.left;
    let seats = s.lanes.len() as u32;
    let secs = s.left.div_ceil(HZ);
    let late = s.left < 10 * HZ && (now / 8) % 2 == 0;
    let mut out = vec![
        right(W - 8, 3, format!("{secs} S"), if late { 'R' } else { 't' }),
        right(W - 8, 13, format!("{}G POT", GOLD_PER_SEAT * seats), 'O'),
        right(W - 8, 23, format!("BEST {}", u32::from(run.home.countryside.games.brasswake.best) * WORK_PER_RECORD), 'h'),
    ];
    if d.down > 0 {
        out.push(centre(12, "BACK UP", 'R'));
    } else if keeper_down(d) {
        out.push(centre(12, "KEEPER DOWN", 'T'));
    } else if d.chain == 0 {
        out.push(centre(12, if d.wave == 3 { "VAT KEEPER".to_string() } else { format!("AISLE {}", d.wave + 1) }, 'h'));
    }
    out.extend(keys(run, d, elapsed));
    for (n, id_n) in ranked(s).into_iter().enumerate() {
        let (x, y) = entry(n);
        let lane = &s.lanes[&id_n];
        let who = if id_n == id { "YOU".to_string() } else { short(&name_of(run, id_n, 12)) };
        let score = if id_n == id { shown(lane, now) } else { lane.score };
        out.push(word(x + 7, y, who, if id_n == id { 'T' } else { 'i' }));
        out.push(right(x + 68, y, score.to_string(), if id_n == id { 'T' } else { 'h' }));
    }
    out
}

pub(super) fn keeper_down(d: &Lane) -> bool {
    d.wave == 3 && d.foes.iter().all(|e| e.hp == 0)
}

pub(super) fn combo_shown(d: &Lane) -> bool {
    d.chain > 0 && d.down == 0 && !keeper_down(d)
}

pub(super) const SCORE_BOX: Rect = (6, 15, 70, 18);
pub(super) const COMBO_BOX: Rect = (150, 2, 84, 30);

fn heart(full: bool) -> &'static Img {
    static HEARTS: std::sync::OnceLock<[Img; 2]> = std::sync::OnceLock::new();
    &HEARTS.get_or_init(|| [heart_img(false), heart_img(true)])[usize::from(full)]
}

fn figures(text: &str, scale: i32, fill: char) -> Img {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<(String, i32, char), Img>> = Default::default();
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 96 {
            c.clear();
        }
        c.entry((text.to_string(), scale, fill)).or_insert_with(|| juice::big_text_img(text, scale, fill, 'B')).clone()
    })
}

fn heart_img(full: bool) -> Img {
    let rows = [
        "..kk.kk..",
        ".k77k77k.",
        "k7R77777k",
        "k7777777k",
        ".k77777k.",
        "..k7777k.",
        "...k77k..",
        "....k....",
    ];
    let img = Img::from_rows(&rows);
    if full {
        return img;
    }
    let mut out = Img::new(9, 8);
    for y in 0..8 {
        for x in 0..9 {
            if let Some(c) = img.get(x, y) {
                out.put(x, y, if c == super::super::super::ink::ink('k').unwrap() { 'g' } else { 'K' });
            }
        }
    }
    out
}

fn shield(ink: char) -> Img {
    let mut im = Img::new(7, 9);
    for (y, w) in [7, 7, 7, 7, 5, 5, 3, 1].into_iter().enumerate() {
        let x0 = (7 - w) / 2;
        for x in x0..x0 + w {
            im.put(x, y as i32, if x == x0 || x == x0 + w - 1 || y == 0 { 'k' } else { ink });
        }
    }
    im.put(3, 8, 'k');
    im.put(3, 2, 'c');
    im
}

fn keeper_icon() -> Img {
    Img::from_rows(&[
        "..kkkkk..",
        ".kooooRk.",
        "kooTooRRk",
        "kozzoozRk",
        "kooooRRRk",
        "kkRRRRRkk",
        ".kkkkkkk.",
    ])
}

fn gate_x(pos: i32) -> i32 {
    TRACK.0 + pos * (TRACK.1 - TRACK.0) / 960
}

fn track(im: &mut Img, run: &Run, s: &Session, id: u32) {
    let now = run.tick as u32;
    let (x0, x1, y) = TRACK;
    let me = &s.lanes[&id];
    let call = |cry: Cry| s.show.list.iter().filter(|c| c.cry == cry).map(|c| c.age(now)).min();
    let team = call(Cry::Team).filter(|&a| a < 24);
    im.rect(x0, y, x1 - x0 + 1, 3, if team.is_some_and(|a| (a / 3) % 2 == 0) { 'o' } else { 'g' });
    im.rect(x0, y, gate_x(me.x.max(0)) - x0, 3, if team.is_some() { 'T' } else { 'o' });
    for (n, g) in [240, 480, 720].into_iter().enumerate() {
        let open = me.wave as usize > n;
        im.rect(gate_x(g) - 1, y - 3, 2, 9, if open { 't' } else { 'j' });
    }
    let alarm = call(Cry::Keeper).filter(|&a| a < 48).is_some_and(|a| (a / 4) % 2 == 0);
    im.stamp(&if alarm { juice::impact_frame(&keeper_icon(), 'R') } else { keeper_icon() }, x1 - 2, y - 3);
    for (n, (&who, lane)) in s.lanes.iter().enumerate() {
        let done = lane.finish.is_some();
        let x = gate_x(lane.x.clamp(0, 960)) - 3;
        let lift = if done { 0 } else { (n as i32 % 2) * 5 };
        let spot = y - 10 - lift;
        let hit = lane.show.latest(|m| matches!(m.kind, Kind::Blow { .. }).then(|| m.age(now))).is_some_and(|a| a < 3);
        let sh = if hit { juice::impact_frame(&shield(pip(who)), 'H') } else { shield(pip(who)) };
        if who == id {
            let bob = (now / 6 % 2) as i32;
            im.rect(x + 3 - 1, spot - 5 + bob, 3, 1, 't');
            im.rect(x + 3, spot - 4 + bob, 1, 1, 't');
        }
        im.stamp(&sh, x, spot);
        if lane.holding {
            im.rect(x + 1, spot - 3, 5, 3, 'r');
            im.rect(x + 1, spot - 3, 5, 1, 'o');
        }
        if done {
            im.put(x + 3, spot + 3, '6');
        }
    }
}

fn standings(im: &mut Img, run: &Run, s: &Session, id: u32) {
    let now = run.tick as u32;
    for (n, who) in ranked(s).into_iter().enumerate() {
        let (x, y) = entry(n);
        if who == id {
            im.frame(x - 3, y - 2, 73, 10, 'g');
        }
        let lane = &s.lanes[&who];
        im.rect(x, y, 4, 7, 'k');
        im.rect(x + 1, y + 1, 2, 5, pip(who));
        if n == 0 && lane.score > 0 {
            im.rect(x - 1, y - 3, 6, 1, 'T');
            im.put(x - 1, y - 4, 'T');
            im.put(x + 2, y - 4, 'T');
            im.put(x + 4, y - 4, 'T');
        }
        if let Some(age) = lane.show.latest(|m| matches!(m.kind, Kind::Blow { .. }).then(|| m.age(now))) {
            if age < 8 && who != id {
                im.rect(x - 3, y + 8, 73 * (8 - age as i32) / 8, 1, pip(who));
            }
        }
    }
}

pub(super) fn paint(im: &mut Img, run: &Run, s: &Session, id: u32) {
    let d = &s.lanes[&id];
    let now = run.tick as u32;
    im.rect(0, TOP - 1, W, 1, 'S');
    im.rect(0, BOTTOM, W, 1, 'S');
    let lost = d.show.latest(|m| matches!(m.kind, Kind::Hurt { guarded: false, .. }).then(|| m.age(now))).filter(|&a| a < 10);
    let bloom = d.show.latest(|m| matches!(m.kind, Kind::Bread).then(|| m.age(now))).filter(|&a| a < 16);
    let low = d.hp <= 2 && d.down == 0;
    let beat = low && (now / 5) % 2 == 0;
    for n in 0..6i32 {
        let full = n < i32::from(d.hp);
        let lift = i32::from(beat && full);
        if lost.is_some_and(|a| a < 3) && n == i32::from(d.hp) {
            im.stamp(&juice::impact_frame(heart(true), 'H'), 6 + n * 10, 5 - lift);
        } else {
            im.stamp(heart(full), 6 + n * 10, 5 - lift);
        }
        if let Some(a) = bloom.filter(|_| full) {
            if bayer(n, a as i32) < 0.7 - a as f32 / 24.0 {
                im.put(6 + n * 10 + 4, 3 - (a as i32 / 3), '6');
                im.put(6 + n * 10 + 1 + (a as i32 % 5), 4, 'c');
            }
        }
    }
    let bump = moments::landing(d, now).map_or(0.0, |a| juice::bump(a, 8));
    let score = figures(&shown(d, now).to_string(), 2, if bump > 0.3 { 'c' } else { 't' });
    im.stamp(&score, SCORE_BOX.0, SCORE_BOX.1 - (bump * 3.0) as i32);
    if combo_shown(d) {
        let flash = d.show.latest(|m| matches!(m.kind, Kind::Blow { .. }).then(|| m.age(now))).is_some_and(|a| a < 3);
        let chain = figures(&format!("X{}", d.chain), 2, if flash { 'c' } else { 'T' });
        im.stamp(&chain, COMBO_BOX.0 + (COMBO_BOX.2 - chain.w) / 2, COMBO_BOX.1);
        for n in 0..3i32 {
            let lit = i32::from(d.combo) > n || (n == 2 && flash && d.combo == 0);
            let x = COMBO_BOX.0 + COMBO_BOX.2 / 2 - 14 + n * 10;
            let kick = n == 2;
            im.rect(x, COMBO_BOX.1 + 22, 8, 5, 'k');
            im.rect(x + 1, COMBO_BOX.1 + 23, 6, 3, if lit { if kick { '@' } else { 'o' } } else { 'g' });
            if kick && d.combo == 2 && (now / 3) % 2 == 0 {
                im.frame(x - 1, COMBO_BOX.1 + 21, 10, 7, 'T');
            }
        }
    }
    track(im, run, s, id);
    standings(im, run, s, id);
}
