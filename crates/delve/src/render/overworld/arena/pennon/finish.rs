//! The end of the siege: the last blow holds the room, film bars close, the
//! call goes up, the iris shuts; then the wall top at night with fireworks
//! over the camp, three steps rising, the best knight laureled, paper
//! falling, the pot counting up, the table's tally and the best-ever line.
//! All of it from the show's `end` tick, so every seat sees it alike.

use super::super::super::ink::{Img, bayer, text_width};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::{self, Pen, ramps};
use super::super::{NATIVE_H, NATIVE_W};
use super::hud::{Words, called, claim, claim_big};
use super::{knight, scene};
use crate::motion::ease;
use crate::rules::Run;
use crate::rules::pennon::Session;

/// Ticks the live finish plays before the scene cuts to the wall top.
pub(super) const FINALE: u32 = 54;
/// The wall top, where the steps stand.
const FLOOR: i32 = 170;

/// Ticks since the siege ended, while it is the show.
pub(super) fn age(s: &Session) -> Option<u32> {
    s.show.end.map(|e| s.show.t.saturating_sub(e))
}

/// Why it ended: a knight broke the siege, or the time ran out.
fn title(s: &Session) -> &'static str {
    if s.won { "SIEGE BROKEN" } else { "TIME UP" }
}

/// The knights by the board's own order: the best first.
fn order(s: &Session) -> Vec<u32> {
    if s.order.is_empty() {
        let mut ids: Vec<u32> = s.lanes.keys().copied().collect();
        ids.sort_by_key(|&id| (s.score(id), id));
        ids
    } else {
        s.order.clone()
    }
}

/// The line under the title.
fn sub(run: &Run, s: &Session, me: u32) -> String {
    let ids = order(s);
    let Some(&best) = ids.first() else { return String::new() };
    if s.lanes.len() < 2 {
        let falls = s.lanes[&best].falls;
        return if !s.won {
            "THE SIEGE HOLDS".into()
        } else if falls == 0 {
            "NO FALLS".into()
        } else {
            format!("{falls} FALLS")
        };
    }
    let tied = ids.iter().filter(|&&r| s.score(r) == s.score(best)).count() > 1;
    if tied {
        "TIE SPLIT".into()
    } else if best == me {
        "YOU TAKE THE POT".into()
    } else {
        format!("{} TAKES THE POT", called(run, best, Some(me), 9))
    }
}

/// The live finish, `e` ticks after the last: a flash, the film's bars, the
/// call, and the iris shutting for the wall top.
pub(super) fn finale(im: &mut Img, run: &Run, s: &Session, me: u32, e: u32) {
    if e < 8 {
        let k = (1.0 - e as f32 / 8.0) * 0.09;
        for y in 0..NATIVE_H {
            for x in 0..NATIVE_W {
                if bayer(x, y) < k {
                    im.put(x, y, 'i');
                }
            }
        }
    }
    let bars = (ease::out(((e as f32 - 2.0) / 10.0).clamp(0.0, 1.0)) * 16.0) as i32;
    for y in 0..bars {
        im.rect(0, y, NATIVE_W, 1, 'k');
        im.rect(0, NATIVE_H - 1 - y, NATIVE_W, 1, 'k');
    }
    if (4..FINALE - 10).contains(&e) {
        banner(im, 70, title(s), &sub(run, s, me), e - 4, FINALE - 14, Tone::Warm, 3);
    }
    let out = ((e as f32 - (FINALE - 14) as f32) / 14.0).clamp(0.0, 1.0);
    juice::iris(im, (NATIVE_W / 2, NATIVE_H / 2 - 10), out, 'k');
}

fn banner(im: &mut Img, y: i32, title: &str, sub: &str, age: u32, life: u32, tone: Tone, cap: i32) {
    if age >= life {
        return;
    }
    juice::banner(im, y, title, sub, age, life, tone, cap);
    let tw = text_width(title).max(1);
    let rest = ((im.w - 24) / tw).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let tail = life.saturating_sub(14);
    let out = if age > tail { (age - tail) as f32 / 14.0 } else { 0.0 };
    let lift = (ease::in_quad(out) * 10.0) as i32;
    let (bw, bh) = (tw * scale + 4, 7 * scale + 4);
    claim_big("banner", (im.w / 2 - bw / 2, y - lift - (scale - rest) * 3, bw, bh), title);
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub).max(1) + 4;
        claim_big("banner-sub", (im.w / 2 - sw / 2, y + 7 * rest + 12 - lift, sw, 11), sub);
    }
}

/// A line of small words with a dark edge, stamped and claimed.
fn line(im: &mut Img, x: i32, y: i32, text: &str, fill: char, what: &'static str) {
    let t = juice::big_text_img(text, 1, fill, 'k');
    im.stamp(&t, x, y);
    claim_big(what, (x, y, t.w, t.h), text);
}

/// The wall top for a seat, `e` ticks after the siege ended.
pub(super) fn scene_for(run: &Run, s: &Session, me: u32, e: u32, words: &mut Words) -> Img {
    let age = e.saturating_sub(FINALE);
    let t = s.show.t;
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    let seed = s.seed;
    scene::paint(&mut im, seed, 300, t);
    // Fireworks over the camp, three launchers in turn.
    for (j, (x, top, ramp)) in [(70.0f32, 40.0f32, ramps::GOLD), (192.0, 30.0, ramps::EMBER), (322.0, 46.0, ramps::HOLY)].into_iter().enumerate() {
        let tt = age as i32 - 4 - j as i32 * 15;
        if tt >= 0 {
            let round = (tt / 52) as u32;
            juice::firework(&mut im, x + (round as f32 * 17.0) % 30.0 - 15.0, 130.0, top, (tt % 52) as u32, 29 * j as u32 + round * 5, ramp, t as u64);
        }
    }
    // The stone they stand on, over the wall's own top.
    for y in FLOOR..FLOOR + 6 {
        im.rect(0, y, NATIVE_W, 1, if y == FLOOR { 'h' } else if y == FLOOR + 1 { 'J' } else { 'G' });
    }
    im.rect(0, FLOOR + 6, NATIVE_W, NATIVE_H - FLOOR - 6, 'K');
    for x in (0..NATIVE_W).step_by(24) {
        im.rect(x, FLOOR + 6, 1, NATIVE_H - FLOOR - 6, 'k');
    }
    for y in (FLOOR + 14..NATIVE_H).step_by(12) {
        im.rect(0, y, NATIVE_W, 1, 'k');
    }
    let ids = order(s);
    let rivals = ids.len() > 1;
    let places: Vec<Place> = ids
        .iter()
        .take(3)
        .map(|&id| {
            let l = &s.lanes[&id];
            let falls = if l.falls == 0 { "NO FALLS".to_string() } else { format!("{} FALLS", l.falls) };
            Place { sprite: knight(run, id, 9, 1, false), name: called(run, id, Some(me), 9), line: format!("{} {falls}", l.score).chars().take(13).collect() }
        })
        .collect();
    let area = (0, 0, NATIVE_W, FLOOR + 6);
    let head = juice::podium(&mut im, area, &places, age);
    // The rank, name and line the kit draws on each step: note their boxes.
    for (n, stand) in juice::podium_stands((NATIVE_W, NATIVE_H), area, &places).iter().enumerate() {
        if age >= stand.words_at {
            claim("podium-rank", stand.rank, &(n + 1).to_string());
            claim("podium-name", stand.name, &stand.name_text);
            claim("podium-line", stand.line, &stand.line_text);
        }
    }
    // The laurel over the best knight once they have stood a moment.
    if age >= 44 && rivals {
        let k = (((age - 44) as f32) / 10.0).min(1.0);
        let bob = ((age as f32 * 0.2).sin() * 1.5) as i32;
        let (x, y) = (head.0, head.1 - 8 + ((1.0 - k) * -20.0) as i32 + bob);
        for a in 0..11 {
            let u = a as f32 / 10.0;
            let (dx, dy) = ((u * 2.0 - 1.0) * 22.0, -(1.0 - (u * 2.0 - 1.0).powi(2)) * 9.0 + 6.0);
            im.rect(x + dx as i32 - 2, y + dy as i32, 5, 3, if a % 2 == 0 { 't' } else { 'O' });
            im.rect(x + dx as i32 - 1, y + dy as i32, 2, 1, 'T');
        }
    }
    // The rest of the table, small, to the left of the steps.
    for (k, &id) in ids.iter().enumerate().skip(3) {
        if age < 24 + k as u32 * 3 {
            continue;
        }
        let l = &s.lanes[&id];
        let text = format!("{} {} {}", k + 1, called(run, id, Some(me), 8), l.score);
        line(&mut im, 6, 84 + (k as i32 - 3) * 12, &text, if id == me { 'T' } else { 'h' }, "rest");
    }
    if age >= 14 {
        juice::confetti(&mut im, (0, 0, NATIVE_W, FLOOR), age, 9, 120);
        let mut pen = Pen::new(&mut im, 120);
        for k in 0..4u32 {
            let a = age as f32 - 14.0 - k as f32 * 9.0;
            pfx::rays(&mut pen, (60.0 + k as f32 * 88.0, 24.0), 6, 12.0, 1.0, 0.0, ramps::GOLD, (a / 14.0).clamp(0.0, 1.0));
        }
    }
    // The title, big, off the top.
    if age >= 6 {
        banner(&mut im, 8, title(s), &sub(run, s, me), age - 6, 70, Tone::Warm, 3);
    }
    // A new best, stamped.
    let best = run.home.countryside.games.pennon.best;
    if best > s.show.best0 && age >= 20 {
        let pulse = i32::from((age / 6) % 2 == 0);
        let t = juice::big_text_img("NEW BEST", 2, if pulse == 1 { 'T' } else { 'O' }, 'B');
        let (x, y) = (NATIVE_W - 6 - t.w, 62);
        im.stamp(&t, x, y);
        claim_big("new-best", (x, y, t.w, t.h), "NEW BEST");
    }
    // What the seat takes home, counting up; and the best-ever line.
    let gold = s.payout(me);
    if age >= 24 && gold > 0 {
        let shown = (gold as f32 * ease::out(((age - 24) as f32 / 36.0).min(1.0))) as u32;
        let tag = juice::big_text_img(&format!("+{shown} GOLD"), 2, 'T', 'B');
        let (x, y) = (NATIVE_W / 2 - tag.w / 2, 196);
        im.stamp(&tag, x, y);
        claim_big("gold", (x, y, tag.w, tag.h), &format!("+{shown} GOLD"));
    }
    if age >= 30 {
        let text = format!("BEST {best}");
        words.say("best", 8, 212, &text, 'o');
    }
    if age >= 40 && (age / 20) % 2 == 0 {
        words.say("again", NATIVE_W - 8 - text_width("E AGAIN"), 212, "E AGAIN", 'T');
    }
    // The wipe open from the live picture's iris.
    let open = 1.0 - ((e as f32 - FINALE as f32) / 12.0).clamp(0.0, 1.0);
    juice::iris(&mut im, (NATIVE_W / 2, NATIVE_H / 2 - 10), open, 'k');
    im
}
