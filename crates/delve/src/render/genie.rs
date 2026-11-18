//! The wish scroll and its genie, drawn over a knight's own view after the
//! game art (`rules::genie`): a parchment scroll with rolled ends on the
//! black world, its words in dark ink, the three ways to have the wish,
//! the wishes held, the party's freedom gauge, and the genie floating over
//! his lamp beside it, talking while the choices show and cheering at a
//! great wish. When he is freed, every knight sees it.
//!
//! The genie is the art team's (`assets/genie`, cut by
//! `scripts/delve/genie_art.py` to the size a 768-wide screen draws; a
//! 384-wide one halves it). Without them, a few inks stand in.
//!
//! The scroll is laid out in the game's own pixels (`NATIVE_W` wide) and
//! drawn whole-pixel larger on a larger screen; a screen narrower than
//! that (a terminal's cells) draws its own (`cockpit`'s scroll).
//!
//! It is a magic thing, drawn with the motion kit (`pfx`): it unrolls
//! from its top roll as it opens, its ink coming up glowing blue just
//! above the travelling roll and drying dark, a glint of that light going
//! over its title now and then; the genie's smoke curls up off his lamp
//! and round the paper's edges; sparkles come and go about it in its
//! wish's colours — the genie's teal, a super wish's gold, a fairy wish's
//! falling stars, a freedom wish's gold and teal. Sealing, a wax seal
//! comes down on it with a ring of gold and a burst of stars, and at the
//! last it rolls itself up. All of it in the margins or for a moment: the
//! words stay as legible as the paper.

use crate::render::overworld::arena::pfx::{self, Pen, Ramp, Shape, Spray, ease, ramps};
use crate::render::overworld::ink::{GLYPH_ADVANCE, Img, bayer, hash, ink, text_width};
use crate::rules::dead_wishes::Target;
use crate::rules::genie::{self, Choice, Take};
use crate::rules::{HZ, Run};
use std::sync::OnceLock;

const NATIVE_W: i32 = 384;

const PAPER: char = 'T';
const PAPER_SHADE: char = 't';
const PAPER_EDGE: char = 'o';
const ROLL: [char; 4] = ['O', 'o', 't', 'T'];
const ROLL_LINE: char = 'b';
const KNOB: char = 'B';
const INK: char = 'n';
const INK_SOFT: char = 'P';
const INK_RED: char = 'B';
const SEAL: char = 'p';

struct Art {
    idle: Vec<Img>,
    talk: Vec<Img>,
    cheer: Vec<Img>,
    free: Img,
    friend: Img,
    lamp: Img,
}

fn decode(png: &[u8]) -> Option<Img> {
    let png = image::load_from_memory(png).ok()?.to_rgba8();
    let mut out = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, c) in png.enumerate_pixels() {
        if c[3] > 127 {
            out.set(x as i32, y as i32, [c[0], c[1], c[2]]);
        }
    }
    Some(out)
}

fn frames(png: &[u8]) -> Option<Vec<Img>> {
    let strip = decode(png)?;
    let w = strip.w / 4;
    Some((0..4).map(|i| Img::from_view(&strip, i * w, 0, w, strip.h)).collect())
}

fn half(im: &Img) -> Img {
    let mut out = Img::new(im.w / 2, im.h / 2);
    for y in 0..out.h {
        for x in 0..out.w {
            let px = [(0, 0), (1, 0), (0, 1), (1, 1)].iter().find_map(|(dx, dy)| im.get(2 * x + dx, 2 * y + dy));
            if let Some(c) = px {
                out.set(x, y, c);
            }
        }
    }
    out
}

fn art(full: bool) -> &'static Art {
    static FULL: OnceLock<Art> = OnceLock::new();
    static HALF: OnceLock<Art> = OnceLock::new();
    let load = || {
        let strip = |png: &[u8]| frames(png).unwrap_or_else(|| (0..4).map(|f| ink_genie(f, false)).collect());
        let idle = strip(include_bytes!("../../assets/genie/genie-idle.png"));
        let free = decode(include_bytes!("../../assets/genie/genie-free.png")).unwrap_or_else(|| ink_genie(0, true));
        Art {
            friend: scaled(&free, idle[0].h),
            idle,
            talk: strip(include_bytes!("../../assets/genie/genie-talk.png")),
            cheer: strip(include_bytes!("../../assets/genie/genie-cheer.png")),
            free,
            lamp: decode(include_bytes!("../../assets/genie/lamp.png")).unwrap_or_else(ink_lamp),
        }
    };
    if full {
        FULL.get_or_init(load)
    } else {
        HALF.get_or_init(|| {
            let a = art(true);
            let halve = |v: &[Img]| v.iter().map(half).collect();
            Art { idle: halve(&a.idle), talk: halve(&a.talk), cheer: halve(&a.cheer), free: half(&a.free), friend: half(&a.friend), lamp: half(&a.lamp) }
        })
    }
}

pub(crate) fn lamp_small() -> &'static Img {
    &art(false).lamp
}

fn ink_lamp() -> Img {
    let mut im = Img::new(34, 28);
    im.ellipse(16.0, 19.0, 11.0, 5.0, 'R');
    im.ellipse(16.0, 18.0, 9.0, 3.5, 'o');
    im.line(26, 18, 33, 13, 'o');
    im.ellipse(16.0, 13.0, 4.0, 2.0, 'O');
    im.rect(12, 24, 9, 3, 'R');
    im
}

fn ink_genie(f: usize, freed: bool) -> Img {
    let mut im = Img::new(97, 132);
    let bob = [0, 1, 2, 1][f % 4];
    im.stamp(&ink_lamp(), 30, 104);
    for i in 0..30 {
        let t = i as f32 / 30.0;
        let x = 48.0 + (t * 6.0 + f as f32).sin() * 6.0;
        im.ellipse(x, 104.0 - t * 40.0, 3.0 + t * 8.0, 3.0, if i % 2 == 0 { '2' } else { '3' });
    }
    let y = 40 + bob;
    im.ellipse(48.0, y as f32 + 22.0, 22.0, 18.0, 'R');
    im.ellipse(48.0, y as f32 + 22.0, 18.0, 14.0, 'o');
    im.ellipse(48.0, y as f32 - 2.0, 14.0, 13.0, 'o');
    im.ellipse(48.0, y as f32 - 14.0, 17.0, 10.0, 'T');
    im.rect(38, y + 2, 21, 3, 'b');
    im.put(43, y - 4, 'n');
    im.put(53, y - 4, 'n');
    if !freed {
        im.rect(70, y + 18, 6, 5, 'j');
    }
    im
}

pub fn paint(out: &mut Img, run: &Run, focus: Option<u32>) {
    let k = if out.w >= 2 * NATIVE_W && out.h >= 300 {
        2
    } else if out.w >= NATIVE_W && out.h >= 150 {
        1
    } else {
        return;
    };
    if freeing(run)
        && let Some((at, who)) = &run.genie.freed
    {
        freed(out, run, k, *at, who);
        return;
    }
    let id = focus.unwrap_or(1);
    let (Some(hero), Some(mood)) = (run.players.get(&id), mood(run, id)) else {
        return;
    };
    let body = match &hero.offer {
        Some(offer) => Body::Offer(offer),
        None if hero.wish_open => Body::Line(hero.wish_line.as_str()),
        None => Body::Sealed(run.genie.last.as_ref().map_or("", |s| s.line.as_str())),
    };
    let view_h = out.h / k;
    dim(out);
    let kind = Variant::of(run, hero, &body);
    let (age, open) = match &body {
        Body::Offer(offer) => {
            let age = run.tick.saturating_sub(offer.opened) as f32;
            (age, ease::out(age / OPEN_TICKS))
        }
        Body::Line(_) => (OPEN_TICKS, 1.0),
        Body::Sealed(_) => {
            let age = run.genie.last.as_ref().map_or(0, |s| run.tick.saturating_sub(s.tick)) as f32;
            let shut = genie::GRANTED_TICKS as f32 - SHUT_TICKS;
            (age, 1.0 - ease::inward((age - shut) / SHUT_TICKS))
        }
    };
    let mut scroll = scroll(run, hero, &body, view_h);
    let sealed = matches!(body, Body::Sealed(_));
    if kind == Variant::Super {
        gild(&mut scroll, run.tick);
    }
    let shown = unrolled(&scroll, open, sealed, age, run.tick, kind);
    let a = art(k == 2);
    let tick = run.tick;
    let frame = (tick / u64::from(HZ / 5)) as usize % 4;
    let genie = match mood {
        Mood::Talk => &a.talk[frame],
        Mood::Cheer => &a.cheer[frame],
        Mood::Idle => &a.idle[frame],
        Mood::Free => &a.friend,
    };
    let gw = genie.w / k + 4;
    let total = gw + scroll.w;
    let x0 = (NATIVE_W - total) / 2 + (out.w / k - NATIVE_W) / 2;
    let y0 = ((view_h - scroll.h) / 2).max(0);
    let rect = (x0 + gw, y0, scroll.w, scroll.h);
    let lamp = (x0 + genie.w / k / 2, y0 + scroll.h - 6);
    let mut back = Img::new(out.w / k, out.h / k);
    let mut front = Img::new(out.w / k, out.h / k);
    magic(&mut back, &mut front, rect, lamp, kind, age, open, sealed, run.tick);
    out.stamp_times(&back, 0, 0, k);
    out.stamp_times(&shown, (x0 + gw) * k, y0 * k, k);
    out.stamp_times(&front, 0, 0, k);
    if sealed {
        seal_stamp(out, k, (rect.0 + rect.2 / 2, rect.1 + 9 + ((rect.3 - 16) as f32 * open.clamp(0.0, 1.0)) as i32), age);
    }
    let gy = (y0 + scroll.h) * k - genie.h - 6 * k;
    out.stamp(genie, x0 * k, gy.max(0));
    let line = match &body {
        Body::Offer(offer) => offer.line.as_str(),
        Body::Line(_) => "Tell me your wish.",
        Body::Sealed(_) if mood == Mood::Cheer => "Now THAT is a wish!",
        Body::Sealed(_) if mood == Mood::Free => "Happy to help, friend!",
        Body::Sealed(_) => "Done! Use it well.",
    };
    let bubble = bubble(line, (gw - 8).max(40));
    let by = (gy / k - bubble.h - 2).max(2);
    out.stamp_times(&bubble, (x0 - 6).max(0) * k, by * k, k);
}

fn dim(out: &mut Img) {
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some([r, g, b]) = out.get(x, y) {
                out.set(x, y, [r / 3, g / 3, b / 3]);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Idle,
    Talk,
    Cheer,
    Free,
}

pub fn portrait(run: &Run, id: u32, rows: i32) -> Option<(Mood, Img)> {
    let mood = mood(run, id)?;
    Some((mood, figure(mood, run.tick, rows)))
}

pub fn figure(mood: Mood, tick: u64, rows: i32) -> Img {
    let a = art(true);
    let frame = (tick / u64::from(HZ / 5)) as usize % 4;
    let full = match mood {
        Mood::Talk => &a.talk[frame],
        Mood::Cheer => &a.cheer[frame],
        Mood::Idle => &a.idle[frame],
        Mood::Free => &a.free,
    };
    scaled(full, rows)
}

fn scaled(full: &Img, rows: i32) -> Img {
    let rows = rows.max(2);
    let cols = (full.w * rows / full.h).max(1);
    let mut out = Img::new(cols, rows);
    for y in 0..rows {
        for x in 0..cols {
            let (x0, y0) = (x * full.w / cols, y * full.h / rows);
            let (x1, y1) = (((x + 1) * full.w / cols).max(x0 + 1), ((y + 1) * full.h / rows).max(y0 + 1));
            let mid = full.get((x0 + x1) / 2, (y0 + y1) / 2);
            let any = || (y0..y1).flat_map(|py| (x0..x1).map(move |px| (px, py))).filter_map(|(px, py)| full.get(px, py)).nth(((x1 - x0) * (y1 - y0) / 3) as usize);
            if let Some(c) = mid.or_else(any) {
                out.set(x, y, c);
            }
        }
    }
    out
}

pub fn mood(run: &Run, id: u32) -> Option<Mood> {
    if freeing(run) {
        return Some(Mood::Free);
    }
    let hero = run.players.get(&id)?;
    let mood = if hero.offer.is_some() || hero.wish_open {
        Mood::Talk
    } else {
        run.genie
            .last
            .as_ref()
            .filter(|s| s.knight == id && !s.quiet && run.tick.saturating_sub(s.tick) < genie::GRANTED_TICKS)
            .map(|s| if s.cheer { Mood::Cheer } else { Mood::Idle })?
    };
    Some(if run.genie.free() { Mood::Free } else { mood })
}

pub fn freeing(run: &Run) -> bool {
    run.genie.freed.as_ref().is_some_and(|(at, _)| run.tick.saturating_sub(*at) < genie::FREED_TICKS)
}

enum Body<'a> {
    Offer(&'a genie::Offer),
    Line(&'a str),
    Sealed(&'a str),
}

fn fit(s: &str, most: usize) -> String {
    if s.chars().count() <= most {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(most.saturating_sub(2)).collect();
        t.push_str("..");
        t
    }
}

fn wrap(s: &str, most: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new()];
    for word in s.split_whitespace() {
        let line = lines.last_mut().expect("one");
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > most {
            lines.push(word.to_string());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    lines
}

pub(super) fn bold(im: &mut Img, x: i32, y: i32, s: &str, ch: char) {
    im.text(x, y, s, ch);
    im.text(x + 1, y, s, ch);
}

pub(super) fn centred(im: &mut Img, y: i32, s: &str, ch: char, heavy: bool) {
    let w = text_width(s) + i32::from(heavy);
    let x = (im.w - w) / 2;
    if heavy {
        bold(im, x, y, s, ch);
    } else {
        im.text(x, y, s, ch);
    }
}

pub(super) fn paper(im: &mut Img, x: i32, y: i32, w: i32, h: i32, seed: u32) {
    for py in y..y + h {
        let bite = |side: u32| (hash(py / 3, side as i32, seed) % 3) as i32;
        let (l, r) = (x + bite(1), x + w - 1 - bite(2));
        for px in l..=r {
            let edge = (px - l).min(r - px);
            let n = hash(px, py, seed) % 97;
            let ch = if edge == 0 {
                PAPER_EDGE
            } else if edge < 3 || n == 0 || (edge < 6 && n < 30) {
                PAPER_SHADE
            } else {
                PAPER
            };
            im.put(px, py, ch);
        }
    }
}

pub(super) fn roll(im: &mut Img, x: i32, y: i32, w: i32) {
    for dy in 0..9 {
        let ch = match dy {
            0 | 8 => ROLL_LINE,
            1 => ROLL[3],
            2 | 3 => ROLL[2],
            4 | 5 => ROLL[1],
            _ => ROLL[0],
        };
        for px in x..x + w {
            im.put(px, y + dy, ch);
        }
    }
    for end in [x, x + w - 4] {
        im.rect(end, y + 1, 4, 7, 'o');
        im.rect(end + 1, y + 3, 2, 3, 'r');
    }
    for kx in [x - 7, x + w] {
        im.rect(kx, y + 2, 7, 5, KNOB);
        im.rect(kx + 1, y + 1, 5, 7, KNOB);
        im.rect(kx + 2, y + 2, 2, 2, 'R');
        im.frame(kx, y + 1, 7, 7, ROLL_LINE);
    }
}

pub(super) fn seal(im: &mut Img, x: i32, y: i32, n: usize, genies: bool) {
    im.ellipse(x as f32 + 5.5, y as f32 + 5.5, 6.0, 6.0, if genies { '8' } else { SEAL });
    im.ellipse(x as f32 + 5.0, y as f32 + 5.0, 4.5, 4.5, if genies { 'B' } else { 'R' });
    im.text(x + 3, y + 2, &format!("{}", n + 1), 'T');
}

fn held(run: &Run, hero: &crate::rules::Hero) -> Vec<String> {
    hero.deck
        .iter()
        .filter(|c| c.starts_with("wish-") && !c.starts_with("wish-kit"))
        .filter_map(|c| run.book.get(c).map(|card| card.name.clone()))
        .collect()
}

fn scroll(run: &Run, hero: &crate::rules::Hero, body: &Body, view_h: i32) -> Img {
    let w = 262;
    let inner = w - 20;
    let most = (inner / GLYPH_ADVANCE) as usize;
    let mut rows: Vec<(Row, u8)> = vec![(Row::Title, 0), (Row::Gap(2), 0)];
    match body {
        Body::Offer(offer) => {
            let label = if hero.place_wish.is_some() {
                "FREEDOM WISH:"
            } else if offer.star {
                "STAR WISH:"
            } else if fairy(run, hero) {
                "FAIRY WISH:"
            } else if offer.great {
                "SUPER WISH:"
            } else {
                "YOU WISH:"
            };
            let room = most - label.chars().count() - 1;
            if let Some(ask) = &offer.ask {
                rows.push((Row::Said("", fit(&ask.to_uppercase(), most.saturating_sub(2))), 0));
                rows.push((Row::Gap(1), 0));
            }
            rows.push((Row::Said(label, fit(&offer.said.to_uppercase(), room)), 0));
            rows.push((Row::Gap(3), 5));
            for (i, choice) in offer.choices.iter().enumerate() {
                rows.push((Row::Choice(i, choice.clone(), i == usize::from(hero.wish_pick)), 0));
            rows.push((Row::Gap(6), 0));
            }
            if let Some(target) = offer.target {
                rows.push((Row::Gap(1), 7));
                rows.push((Row::Target(target), 0));
            }
        }
        Body::Line(line) => {
            let shown = if line.is_empty() { "_" } else { line };
            rows.push((Row::Said("YOU WISH:", fit(&shown.to_uppercase(), most - 10)), 0));
            rows.push((Row::Gap(3), 0));
            rows.push((Row::Small("ENTER SEALS IT. ESC ROLLS IT UP.".into()), 0));
        }
        Body::Sealed(line) => {
            rows.push((Row::Said("SEALED:", fit(&line.to_uppercase(), most - 8)), 0));
            rows.push((Row::Gap(4), 0));
        }
    }
    rows.push((Row::Rule, 0));
    let mut wishes = format!("HELD: {} WISH{}", hero.wishes, if hero.wishes == 1 { "" } else { "ES" });
    if hero.great_wishes > 0 {
        wishes.push_str(&format!(" + {} SUPER", hero.great_wishes));
    }
    if hero.freedom_wishes > 0 {
        wishes.push_str(&format!(" + {} FREEDOM", hero.freedom_wishes));
    }
    if hero.star_wish {
        wishes.push_str(" + 1 STAR");
    }
    let names = held(run, hero);
    if names.is_empty() {
        rows.push((Row::Small(wishes), 0));
    } else {
        let mut lines = vec![format!("{wishes}:")];
        for name in &names {
            let name = name.to_uppercase();
            let line = lines.last_mut().expect("one");
            if line.chars().count() + name.chars().count() + 2 > most {
                lines.push(name);
            } else {
                if !line.ends_with(':') {
                    line.push(',');
                }
                line.push(' ');
                line.push_str(&name);
            }
        }
        for (i, l) in lines.iter().take(2).enumerate() {
            let cut = i == 1 && lines.len() > 2;
            rows.push((Row::Small(if cut { fit(&format!("{l} ..."), most) } else { l.clone() }), if i == 0 { 0 } else { 3 }));
        }
    }
    rows.push((Row::Gap(3), 4));
    rows.push((Row::Gauge, 0));
    rows.push((Row::Small(run.genie.hint().to_uppercase()), 2));
    if matches!(body, Body::Offer(_)) {
        rows.push((Row::Gap(2), 6));
        rows.push((Row::Keys, 1));
    }
    let roll_h = 9;
    let height = |rows: &[(Row, u8)]| rows.iter().map(|(r, _)| r.height()).sum::<i32>() + 10 + 2 * roll_h - 2;
    while height(&rows) > view_h - 2 {
        let Some(at) = rows.iter().enumerate().filter(|(_, (_, d))| *d > 0).max_by_key(|(_, (_, d))| *d).map(|(i, _)| i) else {
            break;
        };
        rows.remove(at);
    }
    let h = height(&rows);
    let paper_h = h - 2 * roll_h + 2;
    let mut im = Img::new(w + 16, h);
    let (px, py) = (8, roll_h - 1);
    paper(&mut im, px + 2, py, w - 4, paper_h, run.raid_id as u32);
    roll(&mut im, px, 0, w);
    roll(&mut im, px, py + paper_h - 1, w);
    let mut y = py + 5;
    let x = px + 10;
    for (row, _) in &rows {
        match row {
            Row::Title => {
                centred(&mut im, y, "THE WISH SCROLL", INK, true);
                let tw = text_width("THE WISH SCROLL") / 2 + 8;
                let cx = im.w / 2;
                for side in [-1, 1] {
                    let sx = cx + side * tw;
                    im.line(sx, y + 3, sx + side * 30, y + 3, INK_SOFT);
                    im.put(sx + side * 32, y + 3, INK_RED);
                    im.put(sx + side * 31, y + 2, INK_SOFT);
                    im.put(sx + side * 31, y + 4, INK_SOFT);
                }
            }
            Row::Gap(_) => {}
            Row::Rule => {
                for dx in (x..x + inner).step_by(3) {
                    im.put(dx, y + 2, INK_SOFT);
                }
            }
            Row::Small(s) => {
                im.text(x, y, s, INK_SOFT);
            }
            Row::Said(label, s) => {
                let after = im.text(x, y + 2, label, INK_SOFT);
                bold(&mut im, after + GLYPH_ADVANCE, y + 2, s, INK);
            }
            Row::Choice(i, c, on) => choice(&mut im, x, y, inner, *i, c, *on),
            Row::Target(t) => target(&mut im, x, y, inner, *t),
            Row::Gauge => gauge(&mut im, x, y, inner, run),
            Row::Keys => {
                let keys = match body {
                    Body::Offer(o) if o.choices.len() == 1 => "1: SEAL IT    ESC: ROLL IT UP",
                    _ => "1  2  3: CHOOSE    ESC: ROLL IT UP",
                };
                centred(&mut im, y, keys, INK_SOFT, false);
            }
        }
        y += row.height();
    }
    im
}

enum Row {
    Title,
    Gap(i32),
    Rule,
    Small(String),
    Said(&'static str, String),
    Choice(usize, Choice, bool),
    Target(Target),
    Gauge,
    Keys,
}

impl Row {
    fn height(&self) -> i32 {
        match self {
            Row::Title => 11,
            Row::Gap(h) => *h,
            Row::Rule => 6,
            Row::Small(_) | Row::Keys => 9,
            Row::Said(..) | Row::Target(_) => 11,
            Row::Choice(..) => 20,
            Row::Gauge => 13,
        }
    }
}

fn choice(im: &mut Img, x: i32, y: i32, w: i32, i: usize, c: &Choice, on: bool) {
    if on {
        im.rect(x - 2, y - 2, w + 4, 20, 'T');
        im.rect(x - 2, y - 2, w + 4, 1, 'n');
        im.rect(x - 2, y + 17, w + 4, 1, 'n');
        im.rect(x - 2, y - 2, 1, 20, 'n');
        im.rect(x + w + 1, y - 2, 1, 20, 'n');
    }
    let genies = matches!(c.take, Take::Deal | Take::Whim);
    if genies {
        for py in y - 2..y + 18 {
            for px in x - 3..x + w + 3 {
                if (px + py) % 2 == 0 {
                    im.put(px, py, PAPER_SHADE);
                }
            }
        }
    }
    seal(im, x, y + 3, i, genies);
    let tx = x + 16;
    let most = ((w - 16) / GLYPH_ADVANCE) as usize;
    let head = c.head.to_uppercase();
    bold(im, tx, y, &head, if genies { INK_RED } else { INK });
    if c.take == Take::Deal {
        let lamp = &art(false).lamp;
        im.stamp(lamp, tx + text_width(&head) + 4, y - 4);
    }
    if !c.cost.is_empty() {
        let room = most.saturating_sub(head.chars().count() + 2);
        let cost = fit(&c.cost.to_uppercase(), room);
        im.text(x + w - text_width(&cost), y, &cost, INK_RED);
    }
    im.text(tx, y + 10, &fit(&c.gives.to_uppercase(), most), INK_SOFT);
}

fn target(im: &mut Img, x: i32, y: i32, w: i32, on: Target) {
    let mut px = im.text(x, y + 2, "FOR:", INK_SOFT) + GLYPH_ADVANCE;
    for t in [Target::You, Target::Dead, Target::Both] {
        let word = t.word().to_uppercase();
        let tw = text_width(&word);
        if t == on {
            bold(im, px, y + 2, &word, INK);
            im.line(px, y + 10, px + tw, y + 10, INK_RED);
        } else {
            im.text(px, y + 2, &word, INK_SOFT);
        }
        px += tw + 2 * GLYPH_ADVANCE;
    }
    let key = "4: CHANGE";
    im.text(x + w - text_width(key), y + 2, key, INK_RED);
}

fn gauge(im: &mut Img, x: i32, y: i32, w: i32, run: &Run) {
    let label = if run.genie.free() { "THE GENIE IS FREE" } else { "FREE THE GENIE" };
    bold(im, x, y + 3, label, INK);
    let bx = x + text_width(label) + 16;
    im.ellipse(bx as f32 - 7.0, y as f32 + 6.5, 4.5, 4.5, 'G');
    im.ellipse(bx as f32 - 7.0, y as f32 + 6.5, 2.5, 2.5, PAPER);
    if run.genie.free() {
        im.rect(bx - 8, y + 1, 3, 3, PAPER);
    }
    let count = format!("{}/{}", run.genie.freedom.min(genie::FREEDOM), genie::FREEDOM);
    let count_x = x + w - text_width(&count) - 8;
    let bw = (count_x - bx - 6).max(8);
    im.rect(bx, y + 2, bw, 9, ROLL_LINE);
    im.rect(bx + 1, y + 3, bw - 2, 7, PAPER_SHADE);
    let fill = if run.genie.free() {
        bw - 2
    } else {
        (bw - 2) * run.genie.freedom.min(genie::FREEDOM) as i32 / genie::FREEDOM as i32
    };
    for px in 0..fill {
        let notch = (px + 1) % ((bw - 2) / 10).max(1) == 0;
        im.rect(bx + 1 + px, y + 3, 1, 7, if notch { 'r' } else { 'R' });
        im.put(bx + 1 + px, y + 3, 'o');
    }
    im.text(count_x, y + 3, &count, INK);
}

pub(super) fn bubble(line: &str, w: i32) -> Img {
    let most = ((w - 10) / GLYPH_ADVANCE).max(6) as usize;
    let lines = wrap(&line.to_uppercase(), most);
    let tw = lines.iter().map(|l| text_width(l)).max().unwrap_or(0);
    let (bw, bh) = (tw + 12, lines.len() as i32 * 9 + 9);
    let mut im = Img::new(bw, bh + 5);
    im.rect(1, 0, bw - 2, bh, ROLL_LINE);
    im.rect(0, 1, bw, bh - 2, ROLL_LINE);
    paper(&mut im, 1, 1, bw - 2, bh - 2, 7);
    for (i, l) in lines.iter().enumerate() {
        im.text(6, 5 + i as i32 * 9, l, INK);
    }
    for dy in 0..5 {
        im.rect(14 + dy / 2, bh - 1 + dy, (5 - dy).max(1), 1, if dy == 4 { ROLL_LINE } else { PAPER });
        im.put(13 + dy / 2, bh - 1 + dy, ROLL_LINE);
    }
    im
}

const OPEN_TICKS: f32 = 10.0;
const SHUT_TICKS: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Variant {
    Plain,
    Super,
    Fairy,
    Freedom,
    Star,
}

const STARLIGHT: Ramp = Ramp(['w', '9', '+', '^', ']', '[']);

impl Variant {
    fn of(run: &Run, hero: &crate::rules::Hero, body: &Body) -> Variant {
        match body {
            Body::Offer(_) if hero.place_wish.is_some() => Variant::Freedom,
            Body::Offer(offer) if offer.star => Variant::Star,
            Body::Offer(_) if fairy(run, hero) => Variant::Fairy,
            Body::Offer(offer) if offer.great => Variant::Super,
            _ => Variant::Plain,
        }
    }

    fn smoke(self) -> Ramp {
        match self {
            Variant::Super => ramps::GOLD.cooler(1),
            Variant::Fairy => ramps::NATURE.cooler(1),
            Variant::Star => ramps::VOID.cooler(1),
            Variant::Plain | Variant::Freedom => ramps::GENIE.cooler(1),
        }
    }

    fn sparkle(self, i: u32) -> Ramp {
        match self {
            Variant::Plain => ramps::GENIE,
            Variant::Super => ramps::GOLD,
            Variant::Fairy => if i % 2 == 0 { ramps::HOLY } else { ramps::NATURE },
            Variant::Freedom => if i % 2 == 0 { ramps::GOLD } else { ramps::GENIE },
            Variant::Star => match i % 3 { 0 => STARLIGHT, 1 => ramps::ROSE, _ => ramps::HOLY },
        }
    }

    fn wet(self) -> char {
        match self {
            Variant::Super => 'R',
            Variant::Fairy => 'l',
            Variant::Star => '[',
            Variant::Plain | Variant::Freedom => '1',
        }
    }
}

fn fairy(run: &Run, hero: &crate::rules::Hero) -> bool {
    hero.fairy_wishes > 0 && hero.making(&run.book).filter(|m| !m.made()).is_none()
}

fn inky(c: [u8; 3]) -> bool {
    [INK, INK_SOFT, INK_RED].iter().any(|&ch| ink(ch) == Some(c))
}

pub(super) fn unrolled(im: &Img, open: f32, sealed: bool, age: f32, tick: u64, kind: Variant) -> Img {
    const ROLL_H: i32 = 9;
    let paper = im.h - 2 * ROLL_H + 2;
    let shown = ((paper as f32 * open.clamp(0.0, 1.0)).round() as i32).clamp(1, paper);
    let bottom = ROLL_H - 1 + shown - 1;
    let mut out = Img::new(im.w, im.h);
    let fresh = |y: i32| -> Option<f32> {
        let d = (bottom - y) as f32;
        (open < 1.0 || (!sealed && age < OPEN_TICKS + 6.0)).then_some(d).filter(|d| (0.0..14.0).contains(d)).map(|d| d / 14.0)
    };
    let glint = (tick % 120) as i32 * 6 - 40;
    for y in 0..bottom.min(im.h) {
        for x in 0..im.w {
            let Some(c) = im.get(x, y) else { continue };
            let burnt = sealed && age < 26.0 && (SAID_ROWS.0..SAID_ROWS.1).contains(&y) && bayer(x, y) < 1.0 - age / 26.0;
            let wet = fresh(y)
                .or_else(|| burnt.then_some(0.1))
                .or_else(|| (y > ROLL_H && y < ROLL_H + 14 && (x - glint - (y - ROLL_H)).abs() < 5).then_some(0.3));
            match wet {
                Some(k) if inky(c) => out.put(x, y, if burnt { 'R' } else if k < 0.6 { kind.wet() } else { 'q' }),
                Some(k) if k < 0.6 && [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| im.get(x + dx, y + dy).is_some_and(inky)) => out.put(x, y, '9'),
                _ => out.set(x, y, c),
            }
        }
    }
    let from = im.h - ROLL_H;
    for dy in 0..ROLL_H {
        for x in 0..im.w {
            if let Some(c) = im.get(x, from + dy) {
                out.set(x, bottom + dy, c);
            }
        }
    }
    out
}

const SAID_ROWS: (i32, i32) = (26, 38);

fn gild(im: &mut Img, tick: u64) {
    let glint = (tick % 90) as i32 * 5 - 30;
    for y in 13..24 {
        for x in 0..im.w {
            if im.get(x, y).is_some_and(inky) {
                im.put(x, y, if (x - glint).abs() < 4 { '6' } else { 'R' });
            }
        }
    }
    for y in 9..im.h - 9 {
        let Some(l) = (0..im.w / 2).find(|&x| im.get(x, y).is_some()) else { continue };
        let Some(r) = (im.w / 2..im.w).rev().find(|&x| im.get(x, y).is_some()) else { continue };
        for (i, x) in [l, l + 1, r - 1, r].into_iter().enumerate() {
            let lit = (y + (tick / 2) as i32 + i as i32 * 4) % 9 < 3;
            im.put(x, y, if lit { '6' } else if i == 0 || i == 3 { 'a' } else { '4' });
        }
    }
}

pub(super) fn seal_stamp(out: &mut Img, k: i32, at: (i32, i32), age: f32) {
    let land = ease::back((age / 6.0).min(1.0));
    let r = (8.0 * (1.0 + 0.5 * (1.0 - land.min(1.0)))).max(6.0);
    let size = (2.0 * r) as i32 + 6;
    let mut im = Img::new(size, size + 8);
    let (cx, cy) = (size as f32 / 2.0, r + 2.0);
    for (side, tail) in [(-1i32, 'R'), (1, 'r')] {
        for j in 0..9 {
            let x = cx as i32 + side * (2 + j / 3);
            im.put(x, cy as i32 + 4 + j, tail);
            im.put(x + side, cy as i32 + 4 + j, 'p');
        }
    }
    im.ellipse(cx + 1.0, cy + 1.5, r + 0.5, r * 0.9 + 0.5, 'B');
    im.ellipse(cx, cy, r, r * 0.92, SEAL);
    im.ellipse(cx - 0.5, cy - 0.5, r * 0.74, r * 0.68, 'R');
    let s = (r * 0.5).round() as i32;
    let (x, y) = (cx.round() as i32, cy.round() as i32);
    for d in -s..=s {
        im.put(x + d, y, 'p');
        im.put(x, y + d, 'p');
    }
    for d in -(s / 2)..=(s / 2) {
        im.put(x + d, y + d, 'p');
        im.put(x + d, y - d, 'p');
    }
    im.put(x - s / 2 - 1, y - s / 2 - 1, 'c');
    im.put(x - s / 2, y - s / 2 - 2, 'c');
    out.stamp_times(&im, (at.0 - size / 2) * k, (at.1 - cy as i32 + 3) * k, k);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn magic(back: &mut Img, front: &mut Img, rect: (i32, i32, i32, i32), lamp: (i32, i32), kind: Variant, age: f32, open: f32, sealed: bool, tick: u64) {
    let (sx, sy, sw, sh) = (rect.0 as f32, rect.1 as f32, rect.2 as f32, rect.3 as f32);
    let t = tick as f32;
    let bottom = sy + 8.0 + (sh - 16.0) * open.clamp(0.0, 1.0);
    let smoke = |i: u32| match kind {
        Variant::Freedom if i % 2 == 0 => ramps::GOLD,
        Variant::Fairy if i % 2 == 0 => ramps::HOLY,
        _ => kind.smoke(),
    };
    {
        let mut pen = Pen::new(back, 6000);
        for i in 0..2u32 {
            pen.wisp((lamp.0 as f32 + 6.0 + i as f32 * 4.0, lamp.1 as f32 - 4.0), t + 400.0, (0.0, 1.0e9), 34.0, 46.0 + i as f32 * 12.0, 5.0 + i as f32 * 2.0, smoke(i), 0x51 + i);
        }
        for (i, at) in [(sx + sw + 8.0, bottom + 4.0), (sx + sw + 12.0, sy + sh * 0.45), (sx + 6.0, sy + 3.0), (sx + sw - 6.0, sy + 3.0)].into_iter().enumerate() {
            pen.wisp(at, t + 300.0 + i as f32 * 17.0, (0.0, 1.0e9), 30.0, if i < 2 { 30.0 } else { 16.0 }, 4.0, smoke(i as u32 + 1), 0x61 + i as u32);
        }
        if sealed && open < 1.0 {
            let shut = genie::GRANTED_TICKS as f32 - SHUT_TICKS;
            for i in 0..4u32 {
                pen.wisp((sx + sw * (0.2 + i as f32 * 0.2), sy + 10.0), age - shut, (0.0, 8.0), 22.0, 18.0, 4.0, ramps::GENIE.cooler(1), 0x71 + i);
            }
        }
    }
    let mut pen = Pen::new(front, 9000);
    for (i, (fx, fy)) in [(0.06, 1.0), (0.32, 1.0), (0.68, 1.0), (0.94, 1.0), (0.0, 0.55), (1.0, 0.3)].into_iter().enumerate() {
        let at = (sx + sw * fx, if fy >= 1.0 { bottom + 5.0 } else { sy + (bottom - sy) * fy });
        pen.wisp(at, t + 200.0 + i as f32 * 23.0, (0.0, 1.0e9), 24.0, 12.0, 3.0, smoke(i as u32), 0x81 + i as u32);
    }
    let (ix0, iy0, ix1, iy1) = (rect.0 + 12, rect.1 + 9, rect.0 + rect.2 - 12, bottom as i32 - 1);
    for y in iy0..iy1 {
        for x in ix0..ix1 {
            pen.cv.clear(x, y);
        }
    }
    let n = match kind {
        Variant::Plain => 9,
        Variant::Freedom => 11,
        Variant::Super | Variant::Fairy | Variant::Star => 14,
    };
    for i in 0..n {
        let life = 40.0;
        let start = (hash(i as i32, 3, 0x5c7) % 1000) as f32 / 1000.0 * life;
        let k = ((t + start) % life) / life;
        let round = ((t + start) / life) as i32;
        let u = (hash(i as i32 + round * 13, 5, 0x5c8) % 1000) as f32 / 1000.0;
        let side = bottom - sy;
        let d = u * 2.0 * (sw + side);
        let at = if d < sw {
            (sx + d, sy - 2.0)
        } else if d < sw + side {
            (sx + sw + 2.0, sy + d - sw)
        } else if d < 2.0 * sw + side {
            (sx + sw - (d - sw - side), bottom + 9.0)
        } else {
            (sx - 2.0, bottom - (d - 2.0 * sw - side))
        };
        pen.twinkle(at, 2.0 + (i % 2) as f32, k, kind.sparkle(i));
    }
    match kind {
        Variant::Super => {
            for i in 0..12u32 {
                let life = 34.0;
                let start = (hash(i as i32, 9, 0x5d1) % 1000) as f32 / 1000.0 * life;
                let k = ((t + start) % life) / life;
                let round = ((t + start) / life) as i32;
                let x = sx + 8.0 + (hash(i as i32 + round * 7, 2, 0x5d2) % 1000) as f32 / 1000.0 * (sw - 16.0);
                let m = pfx::Mote { i, at: (x + (k * 6.0).sin() * 2.0, sy - k * 22.0), vel: (0.0, -1.0), k: k * 0.9, seed: i };
                pfx::mote(&mut pen, &m, if i % 4 == 0 { Shape::Star } else { Shape::Dot }, ramps::GOLD, 2, tick);
            }
        }
        Variant::Fairy => {
            for i in 0..16u32 {
                let life = 50.0;
                let start = (hash(i as i32, 4, 0x5e1) % 1000) as f32 / 1000.0 * life;
                let k = ((t + start) % life) / life;
                let round = ((t + start) / life) as i32;
                let x = sx - 14.0 + (hash(i as i32 + round * 5, 6, 0x5e2) % 1000) as f32 / 1000.0 * (sw + 28.0);
                let at = (x + (k * 8.0 + i as f32).sin() * 4.0, sy - 16.0 + k * (sh + 24.0));
                if at.0 > sx + 2.0 && at.0 < sx + sw - 2.0 && at.1 > sy + 6.0 && at.1 < bottom {
                    continue;
                }
                let m = pfx::Mote { i, at, vel: (0.0, 1.0), k: k * 0.6, seed: i };
                pfx::mote(&mut pen, &m, Shape::Star, kind.sparkle(i), 1, tick);
            }
            for i in 0..4u32 {
                let a = t * 0.03 + i as f32 * std::f32::consts::TAU / 4.0;
                let at = (sx + sw / 2.0 + a.cos() * (sw / 2.0 + 10.0), sy + sh / 2.0 + a.sin() * (sh / 2.0 + 6.0));
                pen.glow(at, 4.0, 1.0, ramps::HOLY, 0.8);
                pen.light(at.0.round() as i32, at.1.round() as i32, '9');
            }
        }
        Variant::Star => {
            for i in 0..14u32 {
                let life = 56.0;
                let start = (hash(i as i32, 7, 0x5f1) % 1000) as f32 / 1000.0 * life;
                let k = ((t + start) % life) / life;
                let round = ((t + start) / life) as i32;
                let x = sx - 14.0 + (hash(i as i32 + round * 5, 8, 0x5f2) % 1000) as f32 / 1000.0 * (sw + 28.0);
                let at = (x + (k * 7.0 + i as f32).sin() * 3.0, sy - 18.0 + k * (sh + 26.0));
                if at.0 > sx + 2.0 && at.0 < sx + sw - 2.0 && at.1 > sy + 6.0 && at.1 < bottom {
                    continue;
                }
                let m = pfx::Mote { i, at, vel: (0.0, 1.0), k: k * 0.6, seed: i };
                pfx::mote(&mut pen, &m, if i % 3 == 0 { Shape::Star } else { Shape::Dot }, kind.sparkle(i), 1, tick);
            }
            let cross = (t % 90.0) / 26.0;
            if cross < 1.0 {
                let head = (sx - 10.0 + (sw + 20.0) * ease::both(cross), sy - 16.0 + 8.0 * cross);
                for j in 0..16 {
                    let p = (head.0 - j as f32 * 1.6, head.1 - j as f32 * 0.5);
                    pen.light(p.0.round() as i32, p.1.round() as i32, STARLIGHT.at(0.1 + j as f32 / 18.0));
                }
                pen.glow(head, 4.0, 1.0, ramps::HOLY, 0.9);
                pen.twinkle(head, 3.0, 0.5, STARLIGHT);
            }
        }
        _ => {}
    }
    if !sealed && age < 16.0 {
        let at = (sx + sw / 2.0, sy + 4.0);
        pfx::rays(&mut pen, at, 10, 26.0, 0.6, 0.2, kind.sparkle(0), age / 16.0);
        let spray = Spray { gravity: 0.08, angle: -std::f32::consts::FRAC_PI_2, spread: 3.4, ..Spray::round(0x5ec7, 14, at, (1.4, 3.4), (14.0, 22.0)) };
        spray.each(age, |m| pfx::mote(&mut pen, &m, Shape::Star, kind.sparkle(m.i), 1, tick));
    }
    if sealed && (4.0..30.0).contains(&age) {
        let (cx, cy) = (sx + sw / 2.0, bottom + 9.0);
        let since = age - 4.0;
        pen.ring((cx, cy), 6.0 + ease::out(since / 14.0) * 40.0, 1.6, 0.7, ramps::GOLD, ease::inward((since / 18.0).min(1.0)));
        pfx::rays(&mut pen, (cx, cy), 12, 26.0, 0.8, 0.1, ramps::GOLD, since / 16.0);
        let spray = Spray { gravity: 0.1, angle: -std::f32::consts::FRAC_PI_2, spread: 4.0, ..Spray::round(0x5ea1, 18, (cx, cy), (1.4, 3.6), (14.0, 24.0)) };
        spray.each(since, |m| pfx::mote(&mut pen, &m, if m.i % 3 == 0 { Shape::Coin } else { Shape::Star }, ramps::GOLD, 1, tick));
    }
}

fn freed(out: &mut Img, run: &Run, k: i32, at: u64, who: &str) {
    let a = art(k == 2);
    let w = out.w / k;
    let h = out.h / k;
    let age = run.tick.saturating_sub(at);
    dim(out);
    let rise = (HZ as u64).saturating_sub(age).min(HZ as u64) as i32 * 2;
    let title = "THE GENIE IS FREE!";
    let lines = [
        format!("{} SET HIM FREE.", who.to_uppercase()),
        "HE STAYS AS YOUR FRIEND:".into(),
        "EVERY FLOOR GIVES 3 WISHES".into(),
        "AND EVERY KNIGHT A FREEDOM WISH,".into(),
        "FOR THE SAFE ROOM OR THE WEAPONS LOCKER".into(),
    ];
    let pw = lines.iter().map(|l| text_width(l)).max().unwrap_or(0).max(text_width(title) + 2) + 24;
    let ph = 16 + 10 * lines.len() as i32 + 8;
    let gh = a.free.h / k;
    let top = ((h - gh - ph - 6) / 2).max(0);
    let mut plate = Img::new(pw + 16, ph + 16);
    paper(&mut plate, 10, 8, pw - 4, ph, 3);
    roll(&mut plate, 8, 0, pw);
    roll(&mut plate, 8, ph, pw);
    centred(&mut plate, 12, title, INK, true);
    for (i, l) in lines.iter().enumerate() {
        let ink = match i {
            0 => INK,
            3 | 4 => INK_RED,
            _ => INK_SOFT,
        };
        centred(&mut plate, 24 + i as i32 * 10, l, ink, false);
    }
    let (gx, gy) = ((out.w - a.free.w) / 2, (top + rise) * k);
    let centre = (gx + a.free.w / 2, gy + a.free.h * 2 / 5);
    send_off(out, run.tick, age, k, centre, (a.free.w, a.free.h), false);
    out.stamp(&a.free, gx, gy);
    send_off(out, run.tick, age, k, centre, (a.free.w, a.free.h), true);
    out.stamp_times(&plate, (w - plate.w) / 2 * k, (top + gh + 2) * k, k);
}

fn send_off(out: &mut Img, tick: u64, age: u64, k: i32, (cx, cy): (i32, i32), (gw, gh): (i32, i32), front: bool) {
    use crate::render::overworld::ink::ink;
    let mut put = |x: i32, y: i32, ch: char| {
        if let Some(c) = ink(ch) {
            for dy in 0..k {
                for dx in 0..k {
                    out.set(x / k * k + dx, y / k * k + dy, c);
                }
            }
        }
    };
    let t = age as f32;
    let tau = std::f32::consts::TAU;
    if !front {
        let reach = gw.max(gh) as f32 * (0.62 + (t / 40.0).min(1.0) * 0.3);
        for i in 0..20 {
            let a = i as f32 / 20.0 * tau + t * 0.008;
            let long = i % 2 == 0;
            let (r0, r1) = (gh as f32 * 0.22, reach * if long { 1.0 } else { 0.62 });
            let steps = ((r1 - r0) / k as f32) as i32;
            for s in 0..steps.max(0) {
                let r = r0 + s as f32 * k as f32;
                let f = s as f32 / steps.max(1) as f32;
                if f > 0.55 && (s + i) % 2 == 0 {
                    continue;
                }
                let ch = if f < 0.3 { '6' } else if f < 0.6 { '5' } else if f < 0.85 { '4' } else { 'a' };
                let (x, y) = (cx + (a.cos() * r) as i32, cy + (a.sin() * r) as i32);
                put(x, y, ch);
                if long && f < 0.5 {
                    let (nx, ny) = ((-a.sin() * k as f32) as i32, (a.cos() * k as f32) as i32);
                    put(x + nx, y + ny, ch);
                }
            }
        }
        return;
    }
    if age < 3 * HZ as u64 {
        for i in 0..9 {
            let a = -0.3 - i as f32 * 0.32;
            let speed = (2.6 + (hash(i, 1, 0x11c) % 4) as f32 * 0.6) * k as f32;
            let side = if i % 3 == 0 { -0.8 } else { 1.0 };
            let (vx, vy) = (a.cos() * speed * side, a.sin() * speed);
            let (x, y) = (
                cx as f32 + gw as f32 * 0.3 * side.signum() + vx * t * 0.5,
                cy as f32 + vy * t * 0.5 + 0.05 * k as f32 * t * t,
            );
            let spin = t * 0.25 + i as f32;
            for j in 0..12 {
                let ang = j as f32 / 12.0 * tau;
                let (ex, ey) = (ang.cos() * 3.2 * k as f32, ang.sin() * 2.0 * k as f32);
                let (rx, ry) = (ex * spin.cos() - ey * spin.sin(), ex * spin.sin() + ey * spin.cos());
                let ch = if ry < -1.0 { 'W' } else if ry < 1.0 { 'i' } else { 'G' };
                put((x + rx) as i32, (y + ry) as i32, ch);
            }
            if (tick as i32 + i) % 4 == 0 {
                put(x as i32, y as i32 - 4 * k, 'w');
            }
        }
    }
    for arm in 0..2 {
        for i in 0..48 {
            let s = i as f32 / 48.0;
            let a = s * 10.0 + t * 0.12 + arm as f32 * std::f32::consts::PI;
            if a.sin() < -0.1 {
                continue;
            }
            let r = (gw as f32 * 0.22 + s * gw as f32 * 0.42) * (1.0 - (t / 300.0).min(0.5));
            let (x, y) = (
                cx + (a.cos() * r) as i32,
                cy + gh / 3 + (a.sin() * r * 0.42) as i32 - (s * gh as f32 * 0.35) as i32,
            );
            if (i + tick as i32) % 4 != 0 {
                let ch = if i % 5 == 0 { '3' } else if i % 2 == 0 { '2' } else { '1' };
                put(x, y, ch);
                put(x + k, y, ch);
            }
        }
    }
    for i in 0..22 {
        let (hx, hy) = (hash(i, 7, 0x57a) % 1000, hash(i, 9, 0x57a) % 1000);
        let a = hx as f32 / 1000.0 * tau;
        let r = gh as f32 * (0.45 + hy as f32 / 1000.0 * 0.5);
        let (x, y) = (cx + (a.cos() * r * 1.35) as i32, cy + (a.sin() * r * 0.75) as i32);
        let phase = (tick as i32 / 4 + i * 3) % 8;
        if phase > 5 {
            continue;
        }
        let arm = if phase == 2 || phase == 3 { 3 } else { 1 + i32::from(phase == 1 || phase == 4) };
        put(x, y, 'w');
        for d in 1..=arm {
            let ch = if d == arm { '5' } else { '6' };
            put(x - d * k, y, ch);
            put(x + d * k, y, ch);
            put(x, y - d * k, ch);
            put(x, y + d * k, ch);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/render/genie__tests.rs"]
mod tests;
