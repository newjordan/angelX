//! Cinder Stack's moments: the title that slams in and the GO behind it, the banner a chain, a
//! cracked stage, a lead or match point earns, and the last five seconds as big numbers over the
//! kiln. Each is a pure function of an age, drawn on a layer of its own (the kit's words are
//! pictures, not the 5x7 font), and each layer claims its pixels (`hud::claim`) so the words test
//! sees every box that was drawn.
use super::super::super::ink::Img;
use super::super::juice::{self, Tone};
use super::hud::{self, Rect};
use crate::rules::cinder::{CRACKS, Session};
use crate::rules::cinder_show::{Cry, Kind};
use crate::rules::{HZ, Run};

/// Ticks the title holds, and the GO that follows it.
pub(super) const TITLE: u32 = 40;
pub(super) const GO: u32 = 26;

/// The layer the banners are drawn on, over the well: wide enough for a twelve-letter title at twice
/// its size and the rails either side of it.
const BANNER: Rect = (84, 26, 168, 64);
/// The layer the GO is drawn on.
const GO_AT: Rect = (84, 30, 168, 100);
/// The layer of the last five seconds, over the kiln.
const COUNT: Rect = (246, 60, 138, 100);

/// The layer a pop's cracks rise on, under the banners' own and inside the well.
const NUM: Rect = (96, 96, 144, 113);

/// A banner on show.
#[derive(Clone, Debug)]
pub(super) struct Banner {
    pub title: String,
    pub sub: String,
    pub tone: Tone,
    pub age: u32,
    pub life: u32,
    pub cap: i32,
    pub rank: u8,
}

/// The banner seat `id`'s screen shows at board tick `bclock`: the loudest of what is alive.
pub(super) fn banner(run: &Run, s: &Session, id: u32, bclock: u32, ended: u32) -> Option<Banner> {
    let b = &s.boards[&id];
    let now = hud::elapsed(s);
    let mut all: Vec<Banner> = Vec::new();
    let mut add = |title: String, sub: String, tone: Tone, age: u32, life: u32, rank: u8| {
        if age < life {
            all.push(Banner { title, sub, tone, age, life, cap: 2, rank });
        }
    };
    if s.left == 0 {
        // The last breath: the kiln comes apart, or the clock runs out.
        let winner = s.order.first().copied().unwrap_or(id);
        if !s.won {
            add("TIME UP".into(), String::new(), Tone::Steel, ended, 30, 60);
        } else if winner == id {
            add("YOU CRACKED IT".into(), String::new(), Tone::Warm, ended, super::finish::FINALE + super::finish::WIPE, 60);
        } else {
            add("KILN CRACKED".into(), format!("{} WINS", hud::called(run, winner, id, 14)), Tone::Warm, ended, super::finish::FINALE + super::finish::WIPE, 60);
        }
    }
    if s.left > 0 && now < TITLE {
        add("CINDER STACK".into(), "MATCH THREE".into(), Tone::Warm, now, TITLE, 50);
    }
    for m in &b.show.marks {
        let age = m.age(bclock);
        match m.kind {
            Kind::Pop { chain, gain, .. } if chain >= 2 => {
                add(format!("CHAIN X{chain}"), format!("+{gain}"), if chain >= 4 { Tone::Hot } else { Tone::Warm }, age, 38, 12 + chain);
            }
            Kind::Stage { n: 1 } => add("KILN CRACKS".into(), "ONE THIRD".into(), Tone::Hot, age, 56, 31),
            Kind::Stage { n: 2 } => add("KILN SPLITS".into(), "TWO THIRDS".into(), Tone::Hot, age, 56, 32),
            Kind::Cough => add("KILN COUGH".into(), String::new(), Tone::Hot, age, 32, 8),
            _ => {}
        }
    }
    for c in s.calls.list.iter().filter(|c| c.who == id) {
        let age = c.age(now);
        match c.cry {
            Cry::Lead => add("YOU LEAD".into(), String::new(), Tone::Warm, age, 40, 12),
            Cry::Close => add("NECK AND NECK".into(), String::new(), Tone::Hot, age, 54, 36),
            Cry::Point => add("MATCH POINT".into(), format!("{} TO GO", CRACKS.saturating_sub(b.cracks)), Tone::Hot, age, 54, 40),
            _ => {}
        }
    }
    all.into_iter().max_by_key(|b| (b.rank, std::cmp::Reverse(b.age)))
}

/// The digit the last five seconds show, and how far into it.
pub(super) fn last_five(s: &Session) -> Option<(String, u32)> {
    (1..=5 * HZ).contains(&s.left).then(|| (s.left.div_ceil(HZ).to_string(), HZ - 1 - (s.left - 1) % HZ))
}

/// Draw a kit moment on a layer of its own, claim its pixels and lay it over `out`.
fn on_layer(out: &mut Img, name: &'static str, (x, y, w, h): Rect, draw: impl FnOnce(&mut Img)) {
    let mut layer = Img::new(w, h);
    draw(&mut layer);
    hud::claim(name, &layer, (x, y));
    out.stamp(&layer, x, y);
}

/// Everything the moments put over the play screen of seat `id`.
pub(super) fn paint(out: &mut Img, run: &Run, s: &Session, id: u32, bclock: u32, ended: u32) {
    let now = hud::elapsed(s);
    let mut quiet = true;
    if s.left > 0 && (TITLE..TITLE + GO).contains(&now) {
        quiet = false;
        on_layer(out, "go", GO_AT, |im| juice::countdown(im, (84, 46), "GO", now - TITLE, GO, true));
    } else if let Some(b) = banner(run, s, id, bclock, ended) {
        quiet = false;
        on_layer(out, "banner", BANNER, |im| juice::banner(im, 16, &b.title, &b.sub, b.age, b.life, b.tone, b.cap));
    }
    // With no banner up, the cracks of the latest pop rise off the spot it popped.
    if quiet && s.left > 0 {
        let latest = s.boards[&id].show.marks.iter().rev().find_map(|m| match m.kind {
            Kind::Pop { by, gain, .. } if m.age(bclock) < 28 => Some((by, gain, m.age(bclock))),
            _ => None,
        });
        if let Some((by, gain, age)) = latest {
            let (cx, cy) = super::fx::centroid(&by);
            let at = (cx as i32 - NUM.0, (cy as i32 - NUM.1).max(14));
            on_layer(out, "num", NUM, |im| juice::pop_number(im, &format!("+{gain}"), at, age, 28, if gain >= 6 { 2 } else { 1 }, 'T'));
        }
    }
    if let Some((label, age)) = last_five(s) {
        on_layer(out, "count", COUNT, |im| juice::countdown(im, (69, 44), &label, age, HZ, false));
    }
}
