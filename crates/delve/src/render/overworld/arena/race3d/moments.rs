//! The moments of the race: the lamps and the slam of the start, the
//! banners for a lead taken, a pass, a wreck, a burst and a photo finish,
//! a "+1" flying to the haul counter, the shake of a blow, the red rim of
//! your own wreck. All of it is read off the race's records by their age
//! (`rules::race3d::show`), so every seat sees the same moments, each from
//! its own ride.
use super::super::juice::{self, Tone};
use super::super::pfx::{self, Pen, Ramp, ramps};
use super::fx::{self, Lens};
use crate::motion::{Spring, ease};
use crate::render::overworld::ink::{Img, bayer, text_width};
use crate::rules::juice::Weight;
use crate::rules::race3d::{Course, Race3d, TITLE, show::Kind};
use crate::rules::{HZ, Run};

/// Ticks a banner stands.
const CALL_LIFE: u32 = 62;
/// Where the haul counter sits: what the flying "+1" lands on.
pub(super) const COUNTER: (i32, i32) = (30, 27);

// ─── the shake and the lens ─────────────────────────────────────────────────

/// A blow a notch lighter: what another knight's wreck does to your screen.
fn lighter(w: Weight) -> Weight {
    match w {
        Weight::Finale => Weight::Kill,
        Weight::Kill => Weight::Crushing,
        Weight::Crushing => Weight::Solid,
        _ => Weight::Graze,
    }
}

/// The world's shake for `seat`: its own wrecks and bumps in full, a rival's
/// a notch lighter, and the first knight home. The picture keeps its
/// phase while the room is held, so a freeze still trembles.
pub(super) fn shake(run: &Run, race: &Race3d, seat: Option<u32>) -> (i32, i32) {
    let now = race.t;
    let phase = u64::from(now) * 7 + u64::from(run.hitstop.left) * 3;
    let blows = race.show.fx.iter().filter_map(|f| {
        let mine = seat == Some(u32::from(f.seat)) || (f.kind == Kind::Bump && seat == Some(u32::from(f.other)));
        let weight = match f.kind {
            Kind::Wreck | Kind::Ruffle | Kind::Bump => f.weight,
            Kind::Finish if f.n == 1 => f.weight,
            Kind::Photo => Weight::Crushing,
            _ => return None,
        };
        let seen = if mine || f.kind == Kind::Finish || f.kind == Kind::Photo { weight } else { lighter(weight) };
        Some((seen, f.age(now)))
    });
    juice::shake(blows, phase)
}

/// How far the lens widens right now (radians): the kick of the green light,
/// a burst, a perfect shift.
pub(super) fn kick(race: &Race3d, seat: Option<u32>) -> f32 {
    let mut k = 0.0f32;
    if race.t >= TITLE && race.t < TITLE + 18 {
        k += 0.12 * (1.0 - (race.t - TITLE) as f32 / 18.0).powi(2);
    }
    if let Some(id) = seat {
        for kind in [Kind::Burst, Kind::Kick] {
            if let Some(age) = race.show.since(kind, id, race.t).filter(|&a| a < 12) {
                k += 0.07 * (1.0 - age as f32 / 12.0);
            }
        }
    }
    k
}

/// How hard the wind is driven by the start: 1 at the green, gone by a second.
pub(super) fn surge(race: &Race3d) -> f32 {
    if race.t >= TITLE && race.t < TITLE + 26 {
        (1.0 - (race.t - TITLE) as f32 / 26.0).max(0.0)
    } else if race.titled() && race.t + 20 >= TITLE {
        // The engines come up in the last moments before the green.
        0.2 * (race.t + 20 - TITLE) as f32 / 20.0
    } else {
        0.0
    }
}

/// A thin dither of black paper behind words that sit over a bright scene
/// (a stone gate, a lamp of gold), softest at its edges.
fn scrim(im: &mut Img, y: i32, h: i32, w: i32) {
    let x0 = im.w / 2 - w / 2;
    for yy in y.max(0)..(y + h).min(im.h) {
        for xx in x0..x0 + w {
            let across = ((xx - x0).min(x0 + w - 1 - xx) as f32 / 20.0).min(1.0);
            let down = (((yy - y).min(y + h - 1 - yy)) as f32 / 6.0).min(1.0);
            if bayer(xx, yy) < 0.66 * across.min(down) {
                im.put(xx, yy, 'k');
            }
        }
    }
}

// ─── the start ──────────────────────────────────────────────────────────────

/// A figure slamming in at `at`, springing to size, ringing, and going: the
/// kit's countdown at the size of this screen.
fn slam(im: &mut Img, at: (i32, i32), label: &str, age: u32, life: u32, go: bool) {
    if age >= life {
        return;
    }
    let width = text_width(label).max(5);
    let rest = (88 / width).clamp(4, if go { 7 } else { 5 });
    let pop = Spring::SNAPPY.at(age as f32, 1.6, 1.0, 0.0).clamp(0.9, 1.6);
    let out = ((age as f32 - life as f32 * 0.7) / (life as f32 * 0.3)).max(0.0);
    let scale = (((rest as f32 * pop) * (1.0 - out * 0.4)).round() as i32).max(1);
    let (fill, edge) = if go { ('5', 'p') } else { ('T', 'B') };
    let mut t = juice::big_text_img(label, scale, if age < 2 { 'H' } else { fill }, edge);
    juice::dissolve(&mut t, out);
    {
        let mut pen = Pen::new(im, 700);
        let a = age as f32;
        let c = (at.0 as f32, at.1 as f32 + 4.0);
        pen.shock(c, (6.0 + a * 10.0).min(84.0), 2.2, 0.5, if go { ramps::GOLD } else { ramps::BONE }, (a / 14.0).min(1.0));
        if go {
            pen.shock(c, (2.0 + a * 16.0).min(110.0), 1.6, 0.5, ramps::EMBER, (a / 12.0).min(1.0));
            pfx::rays(&mut pen, (at.0 as f32, at.1 as f32), 14, 64.0, 0.7, 0.2, ramps::GOLD, (a / 14.0).min(1.0));
        }
    }
    im.stamp(&t, at.0 - t.w / 2, at.1 - t.h / 2);
}

/// The gantry of three lamps: red as the beats go by, green on the start.
fn lamps(im: &mut Img, t: u32) {
    let (cx, cy) = (192, 74);
    // The casing: a dark bar with a thin rule.
    im.rect(cx - 46, cy - 12, 92, 24, 'K');
    im.frame(cx - 46, cy - 12, 92, 24, 'g');
    im.rect(cx - 2, cy + 12, 4, 6, 'g');
    let green = t >= TITLE;
    let since = t.saturating_sub(TITLE) as f32;
    for i in 0..3u32 {
        let lit = green || t >= i * HZ;
        let x = cx - 30 + i as i32 * 30;
        // A lamp swells for a moment as it lights.
        let born = if green { since } else { t.saturating_sub(i * HZ) as f32 };
        let swell = if lit && born < 5.0 { 1.0 + (1.0 - born / 5.0) * 0.5 } else { 1.0 };
        let r = 8.0 * swell;
        let ramp: Ramp = if green { ramps::NATURE } else { ramps::BLOOD };
        for dy in -10..=10 {
            for dx in -10..=10 {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d > r {
                    continue;
                }
                let ink = if !lit {
                    if d > r - 1.5 { 'j' } else { 'g' }
                } else if d > r - 1.5 {
                    ramp.step(4)
                } else if d > r * 0.55 {
                    ramp.step(2)
                } else if (dx + 2) * (dx + 2) + (dy + 3) * (dy + 3) < 6 {
                    ramp.step(0)
                } else {
                    ramp.step(1)
                };
                im.put(x + dx, cy + dy, ink);
            }
        }
    }
}

/// The start, over the whole canvas: the name slams in, the lamps count
/// red, the green lamp and GO ring out.
pub(super) fn start(over: &mut Img, race: &Race3d) {
    let t = race.t;
    if t < TITLE {
        let name = if race.practice { "PRACTICE FLIGHT" } else { race.course.name() };
        scrim(over, 15, 44, 276);
        juice::banner(over, 22, name, race.course.words(), t, TITLE, Tone::Warm, 2);
        lamps(over, t);
        let beat = (t / HZ).min(2);
        slam(over, (192, 108), ["3", "2", "1"][beat as usize], t % HZ, HZ, false);
    } else if t < TITLE + HZ {
        lamps(over, t);
        slam(over, (192, 112), "GO", t - TITLE, HZ, true);
    }
}

// ─── the calls ──────────────────────────────────────────────────────────────

/// How the seat `me` calls a knight, at most `take` letters: `YOU` for
/// themselves and only themselves; any other knight by their own name (`KNIGHT
/// n` for one who still carries the default `You`, or none).
pub(super) fn called(run: &Run, id: u32, me: Option<u32>, take: usize) -> String {
    if Some(id) == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take).collect()
}

fn place_word(n: u32) -> String {
    let suffix = match n {
        1 => "ST",
        2 => "ND",
        3 => "RD",
        _ => "TH",
    };
    format!("{n}{suffix}")
}

/// What a call says, how loud, how long, how inked.
struct Call {
    rank: u8,
    age: u32,
    life: u32,
    title: String,
    sub: String,
    tone: Tone,
}

/// The call standing for `seat`: the loudest of what happened lately.
fn call(run: &Run, race: &Race3d, seat: u32) -> Option<Call> {
    let now = race.t;
    let mut best: Option<Call> = None;
    let mut offer = |c: Call| {
        if best.as_ref().is_none_or(|b| c.rank > b.rank || (c.rank == b.rank && c.age < b.age)) {
            best = Some(c);
        }
    };
    for f in &race.show.fx {
        let age = f.age(now);
        let (who, other) = (u32::from(f.seat), u32::from(f.other));
        let mine = who == seat;
        let make = |rank: u8, life: u32, title: &str, sub: &str, tone: Tone| Call { rank, age, life, title: title.into(), sub: sub.into(), tone };
        match f.kind {
            Kind::Photo if age < 76 => {
                let sub = if f.n == 0 { "DEAD HEAT".to_string() } else { format!("{:.2} SECONDS", f64::from(f.n) / f64::from(HZ)) };
                offer(make(9, 76, "PHOTO FINISH", &sub, Tone::Warm));
            }
            Kind::Finish if mine && age < CALL_LIFE => {
                let place = u32::from(f.n);
                let tone = if place == 1 { Tone::Warm } else { Tone::Steel };
                offer(make(8, CALL_LIFE, &format!("{} HOME", place_word(place)), if place == 1 && race.riders.len() > 1 { "TAKE THE LAUREL" } else { "" }, tone));
            }
            Kind::Finish if !mine && age < 40 && f.n == 1 => {
                offer(make(5, 40, &format!("{} HOME", called(run, who, Some(seat), 9)), "FIRST", Tone::Steel));
            }
            Kind::Wreck if mine && age < 44 => {
                let words = super::crash_words(f.wreck, race.riders.get(&who).map_or(0, |r| r.crashes.saturating_sub(1)));
                offer(make(7, 44, words, "", Tone::Hot));
            }
            Kind::Wreck if !mine && f.weight >= Weight::Crushing && age < 36 => {
                offer(make(2, 36, &format!("{} DOWN", called(run, who, Some(seat), 9)), "", Tone::Steel));
            }
            Kind::Lead if mine && age < CALL_LIFE => {
                offer(make(6, CALL_LIFE, "LEAD", &format!("PASSED {}", called(run, other, Some(seat), 9)), Tone::Warm));
            }
            Kind::Lead if !mine && age < 44 => {
                offer(make(if other == seat { 5 } else { 3 }, 44, &format!("{} LEADS", called(run, who, Some(seat), 9)), "", if other == seat { Tone::Hot } else { Tone::Steel }));
            }
            Kind::Pass if mine && age < 48 => {
                offer(make(4, 48, &place_word(u32::from(f.n)), &format!("PASSED {}", called(run, other, Some(seat), 9)), Tone::Warm));
            }
            Kind::Pass if other == seat && age < 40 => {
                offer(make(3, 40, "PASSED", &format!("BY {}", called(run, who, Some(seat), 9)), Tone::Hot));
            }
            Kind::Burst if mine && age < 28 => {
                let title = if race.course == Course::Touge { "BOOST" } else { "BURST" };
                offer(make(4, 28, title, "", Tone::Warm));
            }
            Kind::Kick if mine && age < 30 => offer(make(4, 30, "PERFECT SHIFT", "", Tone::Warm)),
            _ => {}
        }
    }
    best
}

/// The banner for `seat`, under the heads-up display.
pub(super) fn banner(over: &mut Img, run: &Run, race: &Race3d, seat: u32) {
    if race.titled() || race.t < TITLE + 6 && race.show.fx.is_empty() {
        return;
    }
    if let Some(c) = call(run, race, seat) {
        let cap = if c.rank >= 6 { 3 } else { 2 };
        if c.age < c.life.saturating_sub(10) {
            scrim(over, 26, if cap == 3 { 50 } else { 40 }, 264);
        }
        juice::banner(over, 34, &c.title, &c.sub, c.age, c.life, c.tone, cap);
    }
}

// ─── the heads-up flourishes ────────────────────────────────────────────────

/// A "+1" flying from each thing `seat` just took to the haul counter, a
/// "CLOSE" thrown up off a near miss, and the arrow beside the place.
pub(super) fn pops(over: &mut Img, lens: &Lens, seat: u32) {
    let race = lens.race;
    let now = race.t;
    let mut flown = 0;
    for f in race.show.fx.iter().rev() {
        let age = f.age(now);
        if u32::from(f.seat) != seat {
            continue;
        }
        match f.kind {
            Kind::Pick if age < 16 && flown < 2 => {
                flown += 1;
                let from = lens.at(f.at).map_or((192, 120), |p| (p.0 as i32, p.1 as i32));
                let from = (from.0.clamp(10, 374), from.1.clamp(30, 210));
                juice::fly_up(over, "+1", from, COUNTER, age, 16, '6');
            }
            Kind::Near if age < 28 => {
                let at = lens.at(f.at).map_or((192, 140), |p| (p.0 as i32, p.1 as i32 - 26));
                let at = (at.0.clamp(24, 360), at.1.clamp(34, 200));
                let word = match f.n {
                    1 => "CLEAR",
                    3 => "EDGE",
                    _ => "CLOSE",
                };
                juice::pop_number(over, word, at, age, 28, 1, 'O');
            }
            _ => {}
        }
    }
    // The arrow beside the place: up in green for a pass, down in red when passed.
    let up = race.show.fx.iter().filter(|f| matches!(f.kind, Kind::Pass | Kind::Lead) && u32::from(f.seat) == seat).map(|f| f.age(now)).min();
    let down = race.show.fx.iter().filter(|f| matches!(f.kind, Kind::Pass | Kind::Lead) && u32::from(f.other) == seat).map(|f| f.age(now)).min();
    let flick = |age: u32| age < 36 && (age >= 12 || age % 4 < 2);
    if let Some(a) = up.filter(|&a| flick(a)) {
        arrow(over, 52, 8 - (a as i32 / 6).min(2), true);
    } else if let Some(a) = down.filter(|&a| flick(a)) {
        arrow(over, 52, 8 + (a as i32 / 6).min(2), false);
    }
}

/// A little triangle, up or down.
fn arrow(over: &mut Img, x: i32, y: i32, up: bool) {
    let rows = ["...#...", "..###..", ".#####.", "#######"];
    for (j, row) in rows.iter().enumerate() {
        let dy = if up { j as i32 } else { 3 - j as i32 };
        for (i, c) in row.chars().enumerate() {
            if c == '#' {
                over.put(x + i as i32, y + dy, if up { 'Y' } else { '7' });
            }
        }
    }
}

/// The red rim of a wreck of your own and the pale of the big ones.
pub(super) fn rims(over: &mut Img, race: &Race3d, seat: u32) {
    let (w, h) = (over.w, over.h);
    let now = race.t;
    for f in race.show.fx.iter().filter(|f| u32::from(f.seat) == seat) {
        let age = f.age(now);
        match f.kind {
            Kind::Wreck if age < 20 => {
                if age == 0 && f.weight >= Weight::Kill {
                    fx::pale(over, (0, 0, w, h), 0.12, 'v');
                }
                juice::vignette(over, (0, 0, w, h), 1.0 - age as f32 / 20.0, if f.weight >= Weight::Kill { 16 } else { 11 }, '8');
            }
            Kind::Finish if f.n == 1 && age < 2 => fx::pale(over, (0, 0, w, h), 0.14, 'v'),
            _ => {}
        }
    }
    // The photograph's flash, for everyone.
    if race.show.fx.iter().any(|f| f.kind == Kind::Photo && f.age(now) < 2) {
        fx::pale(over, (0, 0, w, h), 0.16, 'v');
    }
}

/// How far a swelling counter has grown, 0 to 1 and gone, after a "+1" lands.
pub(super) fn counter_pulse(race: &Race3d, seat: u32) -> f32 {
    race.show.since(Kind::Pick, seat, race.t).map_or(0.0, |age| juice::bump(age.saturating_sub(15), 8))
}

/// A crown on the pip of whoever leads.
pub(super) fn crown(over: &mut Img, x: i32, y: i32) {
    let rows = ["#.#.#", "#####", ".###."];
    for (j, row) in rows.iter().enumerate() {
        for (i, c) in row.chars().enumerate() {
            if c == '#' {
                over.put(x + i as i32 - 2, y + j as i32, if j == 0 { '6' } else { '4' });
            }
        }
    }
}

/// A rank arrow's ease, kept for the bump of a place that changes.
pub(super) fn place_bump(race: &Race3d, seat: u32) -> f32 {
    let now = race.t;
    let age = race
        .show
        .fx
        .iter()
        .filter(|f| matches!(f.kind, Kind::Pass | Kind::Lead) && (u32::from(f.seat) == seat || u32::from(f.other) == seat))
        .map(|f| f.age(now))
        .min();
    age.map_or(0.0, |a| ease::bump((a as f32 / 14.0).min(1.0)) * if a < 14 { 1.0 } else { 0.0 })
}
