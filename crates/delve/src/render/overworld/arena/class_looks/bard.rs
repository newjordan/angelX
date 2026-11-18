//! The Bard: a troubadour in a plumed hat and shades who plays a brass sax
//! (`assets/bard/slots.json`), and everything the song draws.
//!
//! - **A note** (`classes::SONG`): a gold ♪ or ♫ by its lane in the volley,
//!   bobbing across its flight, a neon wake of mint and teal swaying behind
//!   it on the path it took.
//! - **A volley** (`bard::strum`): sound blasting off the horn,
//!   lit arcs opening along the aim; on the beat, the first volley of each
//!   slice of the song (`bard::on_beat`), notes thrown out of the bell and
//!   the kick thumping out across the floor, a shock with a dark wake; with
//!   the bass in, a teal ring after it that bends with the bassline.
//! - **The overload** (`bard::DROP_AT`): the picture round the knight
//!   tears and splits, and the horn blows out a ring split into its
//!   colours like a bad signal.
//! - **A note landing** (`feel::Impact::note`, drawn in `vfx`): a flash, a
//!   ring, and little notes thrown up and out of it (`burst_note`).
//! - **The Lullaby** (`bard::Song`): its reach opens out of the Bard past
//!   its size and settles, a turning rim lit gold and teal by turns with
//!   six notes riding it, ripples rolling out to the edge; whoever it
//!   reaches sleeps, Zs drifting up off them.
//!
//! Its rings are drawn in the motion kit (`pfx`).
//!
//! The song's marks are live state: they are signal inks.
use super::super::super::ink::{bayer, hash};
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ease, ramps};
use super::{Img, Layer, Look, View};
use crate::rules::{Hero, Run, Weapon, bard::{self as sim, SONG_REACH, SONG_TICKS}};
use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

struct Slots {
    views: [View; 3],
    note: Img,
}

pub(super) fn rows(value: &serde_json::Value) -> &'static [&'static str] {
    Box::leak(value.as_array().expect("Bard rows").iter().map(|s| {
        &*Box::leak(s.as_str().expect("Bard ink row").to_string().into_boxed_str())
    }).collect::<Vec<_>>().into_boxed_slice())
}

pub(super) fn layer(value: &serde_json::Value) -> Layer {
    Layer {
        at: (value["at"][0].as_i64().unwrap() as i32, value["at"][1].as_i64().unwrap() as i32),
        rows: rows(&value["rows"]),
    }
}

fn slots() -> &'static Slots {
    static SLOTS: OnceLock<Slots> = OnceLock::new();
    SLOTS.get_or_init(|| {
        let spec: serde_json::Value = serde_json::from_str(include_str!("../../../../../assets/bard/slots.json")).expect("Bard slots");
        let views = std::array::from_fn(|i| {
            let v = &spec["knight"]["views"][i];
            View { body: rows(&v["body"]), item: layer(&v["item"]), strike: layer(&v["strike"]) }
        });
        Slots { views, note: Img::from_rows(rows(&spec["note"]["rows"])) }
    })
}

pub(super) fn views() -> &'static [View; 3] { &slots().views }
pub(crate) fn note() -> &'static Img { &slots().note }

fn plays(hero: &Hero) -> bool {
    hero.weapon == Weapon::Notes || Look::of(hero) == Some(Look::Bard)
}

const EIGHTH: [&str; 6] = ["..||.", "..|.|", "..|..", "..|..", "o##..", "###.."];
const PAIR: [&str; 6] = ["..|||||", "..|...|", "..|...|", "..|...|", "o##.o##", "###.###"];

fn lane(vx: f32, vy: f32) -> i32 {
    ((vy.atan2(vx) / (PI / 12.0)).round() as i32).rem_euclid(3)
}

fn sway(lane: i32, tick: u64, at: f32) -> f32 {
    ((tick as f32) * 0.8 + lane as f32 * 2.1 - at * 0.55).sin() * 1.4
}

pub(crate) fn note_shot(cv: &mut Img, (x, y): (i32, i32), (vx, vy): (f32, f32), tick: u64) {
    let len = vx.hypot(vy).max(1e-3);
    let (ux, uy) = (vx / len, vy / len);
    let (px, py) = (-uy, ux);
    let lane = lane(vx, vy);
    let point = |back: f32| {
        let s = sway(lane, tick, back);
        (x + (-ux * back + px * s).round() as i32, y + (-uy * back + py * s).round() as i32)
    };
    for (back, ink) in [(3.0, '3'), (4.5, '3'), (6.0, '2'), (7.5, '2'), (9.0, '0')] {
        let (tx, ty) = point(back);
        cv.put(tx, ty, ink);
    }
    let (gx, gy) = point(3.0 + (tick % 4) as f32 * 1.5);
    cv.put(gx, gy, 'w');
    let glyph: &[&str] = if lane == 1 { &PAIR } else { &EIGHTH };
    let (nx, ny) = point(0.0);
    let (w, h) = (glyph[0].len() as i32, glyph.len() as i32);
    let (stem, head) = if lane == 2 { ('@', '5') } else { ('4', '6') };
    for (r, row) in glyph.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let ink = match ch {
                '|' => stem,
                '#' => head,
                'o' => 'w',
                _ => continue,
            };
            cv.put(nx - w / 2 + c as i32, ny - h / 2 + r as i32, ink);
        }
    }
}

fn since_volley(hero: &Hero) -> Option<i32> {
    (plays(hero) && hero.hp > 0 && hero.fire_cooldown > 0 && hero.loosed > 0)
        .then(|| 4 - hero.fire_cooldown.min(4) as i32)
}

fn bell(hero: &Hero, (x, y): (i32, i32)) -> (f32, f32) {
    let len = hero.aim_x.hypot(hero.aim_y).max(1e-3);
    let (ax, ay) = (hero.aim_x / len, hero.aim_y / len);
    (x as f32 + ax * 8.0, y as f32 - 8.0 + ay * 5.0)
}

const SOUND: Ramp = Ramp(['w', '3', '2', '2', '1', '0']);
const KICK: Ramp = Ramp(['w', '6', '5', '4', '@', 'a']);
const WUB: Ramp = Ramp(['3', '2', '^', '1', ']', '0']);

#[allow(clippy::too_many_arguments)]
fn arc(pen: &mut Pen, c: (f32, f32), r: f32, (a0, a1): (f32, f32), thick: f32, squash: f32, ramp: Ramp, fade: f32, wobble: (f32, f32)) {
    if fade >= 1.0 || r < 1.0 {
        return;
    }
    let n = ((a1 - a0).abs() * r * 1.6).ceil().max(4.0) as i32;
    for i in 0..=n {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        let rr = r * (1.0 + wobble.0 * (a * 6.0 + wobble.1).sin());
        let mid = 1.0 - (2.0 * i as f32 / n as f32 - 1.0).abs();
        let w = (thick * (0.4 + 0.6 * mid)).max(0.5);
        let steps = (w * 2.0).ceil() as i32;
        for j in -steps..=steps {
            let o = j as f32 * 0.5;
            let e = o.abs() / w;
            if e > 1.0 {
                continue;
            }
            let (x, y) = ((c.0 + a.cos() * (rr + o)).round() as i32, (c.1 + a.sin() * (rr + o) * squash).round() as i32);
            let k = (e * 0.6 + fade * 0.7 + (1.0 - mid) * 0.25).min(0.99);
            if fade > 0.4 && bayer(x, y) < (fade - 0.4) * 1.6 {
                continue;
            }
            pen.light(x, y, ramp.at(k));
        }
    }
}

pub(crate) fn blasts(cv: &mut Img, run: &Run) {
    for (&id, hero) in &run.players {
        let Some(age) = since_volley(hero) else { continue };
        let (x, y) = super::super::at(hero.x, hero.y);
        let (bx, by) = bell(hero, (x, y));
        let aim = hero.aim_y.atan2(hero.aim_x);
        let beat = sim::on_beat(hero.groove);
        let mut pen = Pen::new(cv, 500);
        let k = age as f32 / 4.0;
        let spread = if beat { 0.95 } else { 0.65 };
        for (w, lag) in [(0, 0.0f32), (1, 2.5)] {
            let r = 3.0 + ease::out(k) * 12.0 - lag;
            if w == 1 && age == 0 {
                continue;
            }
            arc(&mut pen, (bx, by), r, (aim - spread, aim + spread), if beat { 1.4 } else { 1.0 }, 0.8, SOUND, k * 0.8 + w as f32 * 0.15, (0.0, 0.0));
        }
        if age == 0 {
            pen.glow((bx, by), if beat { 6.0 } else { 4.0 }, 1.0, SOUND, if beat { 0.9 } else { 0.6 });
        }
        if beat {
            let seed = hash(id as i32, hero.loosed as i32, 0xba5);
            let notes = Spray { seed, n: if pen.calm { 2 } else { 3 }, at: (bx, by), angle: aim - 0.5 * aim.cos().signum(), spread: 1.4, speed: (1.4, 2.2), life: (10.0, 14.0), drag: 0.15, gravity: -0.04, squash: 0.8, stagger: 0.0 };
            notes.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Note, ramps::GOLD, 1, run.tick));
        }
    }
}

pub(crate) fn beats(cv: &mut Img, run: &Run) {
    for hero in run.players.values() {
        let Some(age) = since_volley(hero) else { continue };
        if !sim::on_beat(hero.groove) {
            continue;
        }
        let (x, y) = super::super::at(hero.x, hero.y);
        let feet = (x as f32, y as f32 + 1.0);
        let stage = sim::stage(hero.groove);
        let k = age as f32 / 4.0;
        let mut pen = Pen::new(cv, 900);
        let reach = 8.0 + (20.0 + 6.0 * f32::from(stage)) * ease::out(k);
        pen.shock(feet, reach, 1.6, 0.45, KICK, k * 0.85);
        if age == 0 {
            pen.glow(feet, 10.0, 0.45, KICK, 0.7);
        }
        if stage >= 1 {
            let t = run.tick as f32 * 0.9;
            let r = (reach - 7.0).max(3.0);
            arc(&mut pen, feet, r, (0.0, TAU), 1.3, 0.45, WUB, k * 0.7, (0.14, t));
        }
    }
}

pub(crate) fn glitch_mark(mark: &mut Img, tick: u64) {
    let src = mark.clone();
    for y in 0..src.h {
        for x in 0..src.w {
            if src.get(x, y).is_some_and(|c| c != super::super::super::ink::BLACK) {
                if src.get(x - 1, y).is_none_or(|c| c == super::super::super::ink::BLACK) {
                    mark.put(x - 1, y, '7');
                }
                if src.get(x + 1, y).is_none_or(|c| c == super::super::super::ink::BLACK) {
                    mark.put(x + 1, y, '1');
                }
            }
        }
    }
    let torn = mark.clone();
    for y in 0..torn.h {
        let h = hash(y, tick as i32, 77);
        if h % 5 != 0 {
            continue;
        }
        let shift = if h % 2 == 0 { 2 } else { -2 };
        for x in 0..torn.w {
            mark.clear(x, y);
        }
        for x in 0..torn.w {
            if let Some(c) = torn.get(x, y).filter(|&c| c != super::super::super::ink::BLACK) {
                mark.set(x + shift, y, c);
            }
        }
    }
}

pub(crate) fn glitches(cv: &mut Img, run: &Run) {
    use super::super::super::ink::BLACK;
    let tick = run.tick as i32;
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && plays(h)) {
        let stage = sim::stage(hero.groove);
        let (x, y) = super::super::at(hero.x, hero.y);
        if hero.groove == sim::DROP_AT
            && let Some(age) = since_volley(hero)
        {
            let k = age as f32 / 4.0;
            let c = (x as f32, y as f32 - 8.0);
            let r = 8.0 + ease::out(k) * 30.0;
            let mut pen = Pen::new(cv, 900);
            pen.ring((c.0 - 2.0, c.1), r, 1.2, 0.8, ramps::BLOOD, 0.2 + k * 0.7);
            pen.ring((c.0 + 2.0, c.1), r, 1.2, 0.8, ramps::STORM, 0.2 + k * 0.7);
            pen.shock(c, r, 1.6, 0.8, SOUND, k * 0.8);
            pfx::rays(&mut pen, c, 12, 18.0, 0.8, (run.tick % 7) as f32, SOUND, k);
        }
        if stage < 2 {
            continue;
        }
        let seed = id as i32 * 131;
        let (left, top, w, h) = (x - 16, y - 30, 32, 34);
        if hash(tick, seed, 59) % 3 != 0 {
            let lit: Vec<(i32, i32)> = (top..top + h)
                .flat_map(|py| (left..left + w).map(move |px| (px, py)))
                .filter(|&(px, py)| cv.get(px, py).is_some_and(|c| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]) > 300))
                .collect();
            let dark = |cv: &Img, px: i32, py: i32| cv.get(px, py).is_some_and(|c| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]) < 120);
            let spread = 1 + (hash(tick, seed, 71) % 2) as i32;
            for (px, py) in lit {
                if dark(cv, px - spread, py) {
                    cv.put(px - spread, py, '7');
                }
                if dark(cv, px + spread, py) {
                    cv.put(px + spread, py, '1');
                }
            }
        }
        for row in top..top + h {
            let hh = hash(row, tick + seed, 61);
            if hh % 4 != 0 {
                continue;
            }
            let shift = [4, -3, 2, -5, 3, -2][(hh / 4 % 6) as usize];
            let slice: Vec<_> = (0..w).map(|k| cv.get(left + k, row)).collect();
            for (k, c) in slice.into_iter().enumerate() {
                if let Some(c) = c.filter(|&c| c != BLACK) {
                    cv.set(left + k as i32 + shift, row, c);
                }
            }
        }
        for k in 0..10 {
            let hh = hash(k, tick + seed, 67);
            if hh % 4 == 0 {
                continue;
            }
            let bx = x - 20 + (hh % 40) as i32;
            let by = y - 30 + ((hh >> 6) % 34) as i32;
            let bw = 2 + ((hh >> 12) % 9) as i32;
            let ink = ['7', '1', '2', 'w', '3'][((hh >> 16) % 5) as usize];
            cv.line(bx, by, bx + bw, by, ink);
            if (hh >> 20) % 3 == 0 {
                cv.line(bx + 1, by + 1, bx + bw + 1, by + 1, ink);
            }
        }
    }
}

const SMALL: [&str; 4] = ["..|", "..|", "##|", "##."];

pub(crate) fn burst_note(cv: &mut Img, x: i32, y: i32, age: u8) {
    let (stem, head) = match age {
        0..=2 => ('4', '6'),
        3 => ('@', '4'),
        _ => ('a', '@'),
    };
    for (r, row) in SMALL.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let ink = match ch {
                '|' => stem,
                '#' => head,
                _ => continue,
            };
            cv.put(x - 1 + c as i32, y - 3 + r as i32, ink);
        }
    }
}

pub(crate) fn song_rings(cv: &mut Img, run: &Run) {
    let full = SONG_REACH * super::super::UNIT;
    for song in run.bard_songs.iter().filter(|s| s.until > run.tick) {
        let Some(hero) = run.players.get(&song.owner).filter(|h| h.hp > 0) else { continue };
        let (x, y) = super::super::at(hero.x, hero.y);
        let c = (x as f32, y as f32);
        let left = (song.until - run.tick) as i32;
        let age = (SONG_TICKS as i32 - left).max(0);
        let open = ease::back((age as f32 / 10.0).min(1.0));
        let r = full * open;
        let fade = if left < 16 { 1.0 - left as f32 / 16.0 } else { 0.0 };
        let spin = age as f32 * 0.02;
        let mut pen = Pen::new(cv, 1600);
        for k in 0..6 {
            let a0 = spin + k as f32 / 6.0 * TAU;
            let ramp = if k % 2 == 0 { ramps::GOLD } else { SOUND };
            arc(&mut pen, c, r, (a0 + 0.12, a0 + TAU / 6.0 - 0.12), 1.3, 1.0, ramp, 0.15 + fade * 0.85, (0.0, 0.0));
        }
        if open >= 1.0 {
            for wave in 0..2 {
                let t = age - 10 - wave * 12;
                if t < 0 {
                    continue;
                }
                let k = (t % 24) as f32 / 24.0;
                pen.ring(c, 6.0 + (full - 8.0) * ease::out(k), 1.0, 1.0, SOUND, (0.35 + k * 0.65).max(fade));
            }
        }
        if age >= 6 {
            for k in 0..6u32 {
                let a = k as f32 / 6.0 * TAU + spin * 3.0 + TAU / 12.0;
                let bob = ((age + k as i32 * 5) as f32 * 0.4).sin() * 2.0;
                let at = (c.0 + a.cos() * r, c.1 + a.sin() * r + bob);
                let m = pfx::Mote { i: k, at: (at.0, at.1 + 2.0), vel: (0.0, -1.0), k: fade * 0.8, seed: k };
                pfx::mote(&mut pen, &m, Shape::Note, if k % 2 == 0 { ramps::GOLD } else { SOUND }, 1, run.tick);
                let ph = ((age as u32 + k * 11) % 30) as f32 / 10.0;
                if ph < 1.0 && fade < 0.5 {
                    pen.twinkle((at.0 + 3.0, at.1 - 5.0), 2.0, ph, ramps::HOLY);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/render/arena__bard__shots.rs"]
mod shots;
