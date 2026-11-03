//! The knights' ultimates, drawn: every one with its wind-up, its release
//! and what it leaves after, in the motion kit (`pfx`).
//!
//! Each cast makes the same call first — power drawn in off a wide ring,
//! a column of light thrown up off the knight, a flash and a star of rays,
//! a shock off their feet and a rune circle opening on the floor — in the
//! ultimate's own colours; then the ultimate is itself. Pilgrim's Arrow is
//! a lance of light with a road of it behind; Lady's Veil opens feathered
//! wings and a halo on every knight under a dome of light; Bladewind
//! leaves its knight's image at every blink and a crescent at every cut;
//! the Grand Chord rings out in waves and draws its knot round a turning
//! stave; the Sealed Writs fly as letters and break as wax; Midas turns
//! what he touches to gold that crumbles into coin; the assassin's aim is a
//! closing reticle and a red sight, its shot a white-hot tracer; the
//! Phantasm's images slide out of their knight and thin away; the Axe
//! Storm is a whirlwind; Desertion goes in a poof of dust and straw,
//! leaving its scarecrow; the Earthsplitter's seam glows before it bursts;
//! Dismember hoists its foe clear on the Butcher's hook and bleeds it
//! into him; the Philosopher's Rain marks
//! where each flask will break; the Tombstone comes down in a pillar of
//! grave-fire; the Last Light is a reticle and then a column out of the
//! sky; the Needle Squall a gale; the Furnace Verdict a pillar of fire.
//!
//! What is under the figures (a Stillhour's dome, Trebuchet's target
//! rings, the wings, the images, the runes on the floor) and what is over
//! them. Everything is read from the run — the cast as `feel` remembers it,
//! the shots, the ultimates' own state — and drawn the same on every seat.
//! Ultimates are live state: their marks are signal inks.

use super::super::ink::{Img, bayer, hash};
use super::pfx::{self, Pen, Ramp, Shape, Spray, ease, ramps, unit};
use super::{UNIT, at, ranged_events, wishfx};
use crate::rules::feel::UltCast;
use crate::rules::ults::{PHANTOM_TICKS, STRIKE_FALL, ULT_FULL, Ult};
use crate::rules::{HZ, Hero, Run, Shot};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The most motes the ultimates paint in a pass, under and over.
const UNDER_BUDGET: u32 = 1400;
const OVER_BUDGET: u32 = 2200;

/// Lady's Veil's feathers: white to the genie's teal.
const VEIL: Ramp = Ramp(['w', '9', 'c', '3', '2', '1']);
/// Grave-light: a sick green going to the dark of the earth.
const GRAVE: Ramp = Ramp(['Y', 'C', 'y', 'e', 'E', 'F']);
/// A gale: white air through teal into leaf green.
const GALE: Ramp = Ramp(['w', '3', 'C', 'y', 'M', 'l']);
/// A writ's paper, and its wax.
const PAPER: Ramp = Ramp(['9', 'T', 't', 'o', 'O', 'r']);
const WAX: Ramp = Ramp(['c', '$', '7', '7', 'p', 'B']);
/// A tracer: white-hot, a red flank.
const TRACER: Ramp = Ramp(['w', '9', 'c', '7', 'p', 'B']);

/// An ultimate's own colours.
fn ramp_of(u: Ult) -> Ramp {
    match u {
        Ult::PilgrimsArrow | Ult::LastLight => ramps::HOLY,
        Ult::LadysVeil => VEIL,
        Ult::Trebuchet | Ult::FurnaceVerdict => ramps::FIRE,
        Ult::Bladewind | Ult::AxeStorm => ramps::STORM,
        Ult::GrandChord | Ult::MidasTouch => ramps::GOLD,
        Ult::SealedWrits => ramps::ROSE,
        Ult::Stillhour | Ult::Lullaby => ramps::GENIE,
        Ult::Assassinate | Ult::Dismember | Ult::RedHarvest => ramps::BLOOD,
        Ult::SkewerDrive => ramps::STORM,
        Ult::Phantasm => ramps::ARCANE,
        Ult::Desertion | Ult::Earthsplitter => ramps::EARTH,
        Ult::PhilosophersRain => ramps::POISON,
        Ult::Tombstone => GRAVE,
        Ult::NeedleSquall => GALE,
    }
}

/// The points of an ultimate's floor rune.
fn sides_of(u: Ult) -> u32 {
    match u {
        Ult::GrandChord | Ult::Lullaby => 7,
        Ult::LadysVeil | Ult::PhilosophersRain | Ult::Tombstone => 6,
        Ult::Assassinate | Ult::LastLight | Ult::Bladewind => 4,
        Ult::NeedleSquall => 3,
        _ => 5,
    }
}

/// Ticks since `c` was cast.
fn age_of(run: &Run, c: &UltCast) -> f32 {
    run.tick.saturating_sub(c.tick) as f32
}

/// Knight `owner`'s cast of `ult` while its look lasts, and its age.
fn cast(run: &Run, owner: u32, ult: Ult) -> Option<(&UltCast, f32)> {
    run.feel.ults.iter().rev().find(|c| c.owner == owner && c.ult == ult).map(|c| (c, age_of(run, c)))
}

/// A cast's seed: its knight, its ultimate, its tick.
fn seed_of(c: &UltCast) -> u32 {
    hash(c.owner as i32, c.ult as i32, c.tick as u32)
}

/// A foe's feet and middle in native pixels, and its height.
fn foe_mid(run: &Run, e: &crate::rules::Enemy) -> ((f32, f32), (f32, f32), f32) {
    let (x, y, h) = super::works::foe_box(run, e);
    let feet = (x as f32, y as f32);
    (feet, (feet.0, feet.1 - h as f32 * 0.5), h as f32)
}

/// The knight's hand, a little out along their aim.
fn hand(run: &Run, hero: &Hero) -> (f32, f32) {
    let (_, mid) = wishfx::body(run, hero);
    let l = hero.aim_x.hypot(hero.aim_y);
    let (ux, uy) = if l > 0.01 { (hero.aim_x / l, hero.aim_y / l) } else { (1.0, 0.0) };
    (mid.0 + ux * 6.0, mid.1 + uy * 4.0 - 1.0)
}

/// Whether arena point `p` lies within `slack` of the line from `a`
/// along `u`, ahead of `a`.
fn on_line(p: (f32, f32), a: (f32, f32), u: (f32, f32), slack: f32) -> bool {
    let (dx, dy) = (p.0 - a.0, p.1 - a.1);
    let along = dx * u.0 + dy * u.1;
    along > -slack && (dx * u.1 - dy * u.0).abs() <= slack
}

// ─── the passes ──────────────────────────────────────────────────────────────

/// Under the figures: the floor runes of every cast, domes, target rings,
/// wings and images, seams and graves.
pub(super) fn under(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    super::class_looks::bard::song_rings(cv, run);
    super::class_looks::bard::beats(cv, run);
    super::class_looks::peasant::desertion(cv, run);
    super::champions::under(cv, run);
    super::three_heroes::under(cv, run);
    let mut pen = Pen::new(cv, UNDER_BUDGET);
    for c in &run.feel.ults {
        floor_rune(&mut pen, run, c);
    }
    for sphere in &run.spheres {
        stillhour(&mut pen, sphere, tick);
    }
    for strike in &run.strikes {
        shell(&mut pen, strike, tick);
    }
    seams(&mut pen, run);
    flask_marks(&mut pen, run);
    graves(&mut pen, run);
    verdict_floor(&mut pen, run);
    whirl_floor(&mut pen, run);
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0) {
        if hero.angel > 0 {
            wings(&mut pen, run, id, hero);
        }
        images(&mut pen, run, id, hero);
    }
}

/// Over the figures: every cast's call and release, the shots and the
/// marks of the ultimates in play, and a ready ultimate's sparkle.
pub(super) fn over(cv: &mut Img, run: &Run) {
    super::class_looks::bard::blasts(cv, run);
    super::class_looks::bard::glitches(cv, run);
    super::champions::over(cv, run);
    super::three_heroes::over(cv, run);
    let mut pen = Pen::new(cv, OVER_BUDGET);
    for c in &run.feel.ults {
        if let Some(hero) = run.players.get(&c.owner).filter(|h| h.hp > 0) {
            moment(&mut pen, run, c, hero);
            call(&mut pen, run, c, hero);
        }
    }
    pilgrim(&mut pen, run);
    writs(&mut pen, run);
    assassin(&mut pen, run);
    bladewind(&mut pen, run);
    axe_storm(&mut pen, run);
    halos(&mut pen, run);
    dismember(&mut pen, run);
    flasks(&mut pen, run);
    tombstones(&mut pen, run);
    last_light(&mut pen, run);
    needle_squall(&mut pen, run);
    furnace(&mut pen, run);
    for hero in run.players.values().filter(|h| h.hp > 0) {
        if hero.ult_charge >= ULT_FULL && !hero.stone {
            ready(&mut pen, run, hero);
        }
    }
}

// ─── the call every ultimate makes ───────────────────────────────────────────

/// Power drawn in off a wide ring for a breath, then a column of light
/// thrown up off the knight, a flash and a star of rays at their middle,
/// a shock ring running off their feet, and the ultimate's own motes.
fn call(pen: &mut Pen, run: &Run, c: &UltCast, hero: &Hero) {
    let age = age_of(run, c);
    if age >= 28.0 {
        return;
    }
    let r = ramp_of(c.ult);
    let (feet, mid) = wishfx::body(run, hero);
    let s = seed_of(c);
    // Drawn in: streaks off a wide ring, slow and then all at once.
    if age < 5.0 {
        let t = ease::inward(age / 5.0);
        for i in 0..16u32 {
            let a = i as f32 / 16.0 * TAU + unit(i as i32, s as i32, 1) * 0.4;
            let rad = 38.0 * (1.0 - t) + 4.0;
            let p = (mid.0 + a.cos() * rad, mid.1 + a.sin() * rad * 0.7);
            pen.streak(p, (-a.cos(), -a.sin() * 0.7), 2.0 + t * 6.0, r, 0.15);
        }
        pen.glow(mid, 6.0 + age * 1.6, 1.0, r, 0.45 + age * 0.08);
    }
    let since = age - 3.0;
    if since < 0.0 {
        return;
    }
    // What is thrown up off the knight goes behind them: they stay seen.
    pen.keep = true;
    // The column, thrown up and thinning.
    if since < 16.0 {
        let k = since / 16.0;
        let tall = 54.0;
        let top = feet.1 - tall * ease::out((since / 4.0).min(1.0));
        let half = 5.0 * (1.0 - k) + 0.8;
        let mut y = feet.1;
        while y > top {
            let up = (feet.1 - y) / tall;
            let w = half * (1.0 - up * 0.55);
            for dx in -(w.ceil() as i32)..=(w.ceil() as i32) {
                let x = mid.0.round() as i32 + dx;
                let yy = y.round() as i32;
                let edge = dx.abs() as f32 / w.max(0.5);
                if edge > 1.0 || bayer(x, yy) > (1.0 - edge * 0.7) * (1.0 - k * 0.6) * (1.05 - up * 0.7) {
                    continue;
                }
                pen.light(x, yy, r.at(edge * 0.4 + k * 0.35 + up * 0.35));
            }
            y -= 1.0;
        }
    }
    if since < 14.0 {
        let k = since / 14.0;
        pen.glow(mid, 16.0 * (1.0 + 0.25 * ease::bump(k)), 1.0, r, 0.95 * (1.0 - ease::inward(k)));
        pfx::rays(pen, mid, 12, 26.0, 0.85, unit(s as i32, 2, 2) * TAU, r, k);
    }
    pen.keep = false;
    pen.shock((feet.0, feet.1 + 1.0), 4.0 + ease::out(since / 18.0) * 36.0, 2.0, 0.45, r, ease::inward((since / 22.0).min(1.0)));
    // A spray of the ultimate's own motes off the burst.
    let (shape, n) = match c.ult {
        Ult::GrandChord | Ult::Lullaby => (Shape::Note, 10),
        Ult::MidasTouch => (Shape::Coin, 16),
        Ult::LadysVeil | Ult::SealedWrits => (Shape::Petal, 16),
        Ult::Desertion | Ult::Earthsplitter => (Shape::Shard, 14),
        Ult::Tombstone | Ult::PhilosophersRain => (Shape::Bubble, 10),
        Ult::Dismember | Ult::Assassinate | Ult::RedHarvest => (Shape::Droplet, 12),
        _ => (Shape::Star, 14),
    };
    let spray = Spray {
        seed: s,
        n: if pen.calm { n / 2 } else { n },
        at: mid,
        angle: -FRAC_PI_2,
        spread: 4.6,
        speed: (1.2, 3.4),
        life: (14.0, 24.0),
        drag: 0.08,
        gravity: 0.08,
        squash: 1.0,
        stagger: 0.0,
    };
    spray.each(since, |m| pfx::mote(pen, &m, shape, r, 1, run.tick));
}

/// The cast's rune on the floor: drawn on with an overshoot where the
/// knight stood, turning, and dissolving.
fn floor_rune(pen: &mut Pen, run: &Run, c: &UltCast) {
    let age = age_of(run, c);
    if age >= 60.0 || matches!(c.ult, Ult::Stillhour | Ult::Trebuchet) {
        return;
    }
    let side = if run.side_on() { crate::rules::ledge::FOOT } else { 0.0 };
    let at = (c.x * UNIT, (c.y + side) * UNIT + 1.0);
    let open = ease::back((age / 9.0).min(1.0));
    let fade = ((age - 26.0) / 34.0).clamp(0.0, 1.0);
    let r = ramp_of(c.ult);
    pen.glow(at, 22.0 * open.max(0.3), 0.42, r.cooler(2), 0.5 * (1.0 - fade));
    pen.glyph(at, 18.0 * open.max(0.3), 0.42, age * 0.05 + unit(c.owner as i32, 3, 0x51) * TAU, open.min(1.0), sides_of(c.ult), r, fade, seed_of(c));
}

/// Each ultimate's own moment at its cast, where it has one.
fn moment(pen: &mut Pen, run: &Run, c: &UltCast, hero: &Hero) {
    let age = age_of(run, c);
    match c.ult {
        Ult::LadysVeil => veil_dome(pen, run, c, age),
        Ult::GrandChord => chord(pen, run, c, age),
        Ult::MidasTouch => midas(pen, run, c, hero, age),
        Ult::SealedWrits if age < 12.0 => {
            let at = hand(run, hero);
            let spray = Spray { gravity: 0.03, ..Spray::round(seed_of(c), if pen.calm { 8 } else { 16 }, at, (0.8, 2.6), (16.0, 26.0)) };
            spray.each(age, |m| pfx::mote(pen, &m, Shape::Petal, PAPER, 2, run.tick));
            pfx::rays(pen, at, 6, 14.0, 1.0, 0.4, WAX, age / 12.0);
        }
        Ult::Desertion if age < 34.0 => desertion_poof(pen, run, c, hero, age),
        Ult::Earthsplitter if age < 20.0 => {
            let (feet, _) = wishfx::body(run, hero);
            pen.shock((feet.0, feet.1 + 1.0), 6.0 + ease::out(age / 14.0) * 30.0, 3.0, 0.5, ramps::EARTH, ease::inward(age / 20.0));
            for i in 0..3u32 {
                pen.wisp((feet.0 - 6.0 + i as f32 * 6.0, feet.1), age, (0.0, 6.0), 22.0, 12.0, 3.0, ramps::SMOKE, seed_of(c) ^ i);
            }
        }
        Ult::Lullaby if age < 40.0 => {
            let (_, mid) = wishfx::body(run, hero);
            // The music box opening: notes let go in a ring, rising.
            for i in 0..8u32 {
                let t = (age - i as f32 * 2.0) / 26.0;
                if !(0.0..1.0).contains(&t) {
                    continue;
                }
                let a = i as f32 / 8.0 * TAU + t * 1.6;
                let at = (mid.0 + a.cos() * (6.0 + t * 20.0), mid.1 + a.sin() * (3.0 + t * 8.0) - t * 18.0);
                let m = pfx::Mote { i, at, vel: (0.0, -1.0), k: t, seed: i };
                pfx::mote(pen, &m, Shape::Note, if i % 2 == 0 { ramps::GOLD } else { ramps::GENIE }, 1, run.tick);
            }
        }
        _ => {}
    }
}

/// Straw: pale gold to the brown of old hay.
const STRAW: Ramp = Ramp(['T', 't', 'O', 'o', 'r', 'P']);
/// A poof's dust: lit near white on top, the greys under.
const POOF: Ramp = Ramp(['H', 'i', 'h', 'J', 'G', 'j']);

/// Desertion: where the peasant stood, a poof. A flash, then a cloud
/// bursting out in billows, lit on top and grey under, rolling up and
/// thinning away; straw thrown out of it in every way and coming down; a
/// ring of dust off the floor; and a streak of it after the peasant, who
/// is already gone, leaving the scarecrow standing in it.
fn desertion_poof(pen: &mut Pen, run: &Run, c: &UltCast, hero: &Hero, age: f32) {
    // A cloud this size is paid for out of its own purse, so the call
    // every ultimate makes still has its own.
    let calm = pen.calm;
    let mut own = Pen::new(&mut *pen.cv, 1800);
    own.calm = calm;
    let pen = &mut own;
    let s = seed_of(c);
    let side = if run.side_on() { crate::rules::ledge::FOOT } else { 0.0 };
    let feet = (c.x * UNIT, (c.y + side) * UNIT);
    let at = (feet.0, feet.1 - 9.0);
    let k = age / 34.0;
    if age < 4.0 {
        pen.glow(at, 12.0, 0.9, POOF, 1.0 - age / 4.0);
        pfx::rays(pen, at, 10, 20.0, 0.8, unit(s as i32, 1, 7) * TAU, STRAW, age / 4.0);
    }
    pen.shock((feet.0, feet.1 + 1.0), 5.0 + ease::out(age / 16.0) * 30.0, 2.0, 0.42, POOF, ease::inward((age / 20.0).min(1.0)));
    // The cloud: billows thrown out from the middle and slowing, rising a
    // little, each swelling as it thins.
    let n = if pen.calm { 7 } else { 11 };
    for b in 0..n {
        let a = b as f32 / n as f32 * TAU + (unit(b, 2, s) - 0.5) * 0.6;
        let reach = 9.0 + unit(b, 3, s) * 9.0;
        let out = ease::out((age / 14.0).min(1.0)) * reach;
        let rise = ease::out(k) * (5.0 + unit(b, 4, s) * 5.0);
        let centre = (at.0 + a.cos() * out, at.1 + a.sin() * out * 0.6 - rise);
        let r = 4.0 + ease::out((age / 10.0).min(1.0)) * (3.5 + unit(b, 5, s) * 2.5) + k * 2.0;
        billow(pen, centre, r, ((age - 4.0) / 30.0).clamp(0.0, 1.0));
    }
    // The heart of it, last to go.
    billow(pen, (at.0, at.1 - ease::out(k) * 4.0), 7.0 + ease::out((age / 8.0).min(1.0)) * 3.0, ((age - 2.0) / 26.0).clamp(0.0, 1.0));
    let straw = Spray { seed: s ^ 9, n: if pen.calm { 8 } else { 16 }, at, angle: -FRAC_PI_2, spread: TAU, speed: (1.4, 3.6), life: (16.0, 26.0), drag: 0.06, gravity: 0.12, squash: 0.8, stagger: 0.0 };
    straw.each(age, |m| pfx::mote(pen, &m, Shape::Shard, STRAW, 2, run.tick));
    // A streak of dust after the peasant, the way they ran.
    if age < 16.0 {
        let (to, _) = wishfx::body(run, hero);
        let (dx, dy) = (to.0 - feet.0, to.1 - feet.1);
        let l = dx.hypot(dy);
        if l > 8.0 {
            let (ux, uy) = (dx / l, dy / l);
            for i in 0..5 {
                let off = (i as f32 - 2.0) * 3.0;
                let start = 10.0 + unit(i, 6, s) * 6.0;
                let p = (feet.0 + ux * start - uy * off, feet.1 - 5.0 + uy * start + ux * off * 0.5);
                pen.streak(p, (ux, uy), 6.0 + (l - start).clamp(0.0, 18.0) * (1.0 - age / 16.0), POOF, age / 16.0);
            }
        }
    }
}

/// A billow of a poof round `c`, `r` across, `k` of the way through its
/// life: lit on its upper left, grey under, an outline of its darkest
/// where it meets the air, breaking up through the dither as it goes.
fn billow(pen: &mut Pen, c: (f32, f32), r: f32, k: f32) {
    if k >= 1.0 {
        return;
    }
    let ri = r.ceil() as i32 + 1;
    let fill = (1.0 - k).powf(0.8);
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (px, py) = ((c.0 + dx as f32).round() as i32, (c.1 + dy as f32).round() as i32);
            let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
            if d > 1.0 || bayer(px, py) >= fill * (1.2 - d * 0.45) {
                continue;
            }
            let lit = -(dx as f32 * 0.7 + dy as f32) / r;
            let ink = if d > 0.86 { POOF.step(4 + usize::from(k > 0.5)) } else { POOF.at((0.42 - lit * 0.5 + k * 0.4).clamp(0.0, 0.99)) };
            pen.put(px, py, ink);
        }
    }
}

// ─── Stillhour and Trebuchet ─────────────────────────────────────────────────

/// A Stillhour's dome: its rim lit and turning slowly, a clock's face of
/// twelve marks inside it, its hands stopped and glowing, and the air in
/// it hung with motes that do not move; flickering as it goes.
fn stillhour(pen: &mut Pen, sphere: &crate::rules::ults::Sphere, tick: u32) {
    let (cx, cy) = at(sphere.x, sphere.y);
    let c = (cx as f32, cy as f32);
    let r = sphere.r * UNIT;
    if sphere.left < 20 && (tick / 2).is_multiple_of(2) {
        return;
    }
    // Opening: the rim snaps out past its size and settles.
    let full = crate::rules::press::STILL_TICKS.0;
    let born = (full.saturating_sub(sphere.left) as f32 / 8.0).min(1.0);
    let born = if sphere.left > full { 1.0 } else { born };
    let rr = r * ease::back(born);
    pen.ring(c, rr, 1.6, 0.8, ramps::GENIE, 0.1);
    pen.ring(c, rr - 3.0, 0.8, 0.8, ramps::GENIE.cooler(2), 0.3);
    let n = (rr * 1.2) as i32;
    for k in 0..n {
        // A glint going slowly round the rim.
        let a = k as f32 / n as f32 * TAU + tick as f32 * 0.01;
        if k % 9 == 0 {
            pen.twinkle((c.0 + a.cos() * rr, c.1 + a.sin() * rr * 0.8), 1.5, ((tick + k as u32 * 7) % 40) as f32 / 40.0, ramps::GENIE);
        }
    }
    // The clock's face.
    for h in 0..12 {
        let a = h as f32 / 12.0 * TAU;
        let (o, i) = (rr * 0.55, rr * if h % 3 == 0 { 0.44 } else { 0.5 });
        pen.line((c.0 + a.cos() * i, c.1 + a.sin() * i * 0.8), (c.0 + a.cos() * o, c.1 + a.sin() * o * 0.8), if h % 3 == 0 { '3' } else { '1' });
    }
    pen.line(c, (c.0, c.1 - rr * 0.4), '3');
    pen.line(c, (c.0 + rr * 0.28, c.1), '3');
    pen.glow(c, 4.0, 1.0, ramps::GENIE, 0.7);
    // The still air: motes hung where they were.
    for k in 0..24 {
        let a = unit(k, cx, 0x51) * TAU;
        let d = unit(k, cy, 0x52).sqrt() * (rr - 3.0);
        let at = (c.0 + a.cos() * d, c.1 + a.sin() * d * 0.8);
        pen.light(at.0.round() as i32, at.1.round() as i32, if (k as u32 + tick / 20) % 5 == 0 { '2' } else { '0' });
    }
}

/// A shell coming down: its target a rune of fire on the floor drawing in
/// as it nears, its shadow growing, the shell itself a lit stone with a
/// streak of fire behind it.
fn shell(pen: &mut Pen, strike: &crate::rules::ults::Strike, tick: u32) {
    let (x, y) = at(strike.x, strike.y);
    let c = (x as f32, y as f32);
    let waiting = strike.fall >= STRIKE_FALL;
    let k = strike.fall.min(STRIKE_FALL) as f32 / STRIKE_FALL as f32;
    let r = 6.0 + 10.0 * ease::out(k);
    let ramp = ramps::FIRE.cooler(1);
    let rot = tick as f32 * 0.05 + (x + y) as f32;
    // The mark: a ring and four brackets turning on it, faint while the
    // shell waits its turn, drawing in as it comes.
    pen.ring(c, r, 0.8, 0.6, ramp, if waiting { 0.7 } else { 0.2 });
    if waiting {
        return;
    }
    for q in 0..4 {
        let a = rot + q as f32 * FRAC_PI_2;
        let (ux, uy) = (a.cos(), a.sin() * 0.6);
        pen.line((c.0 + ux * (r + 2.0), c.1 + uy * (r + 2.0)), (c.0 + ux * (r - 2.5), c.1 + uy * (r - 2.5)), ramp.step(0));
    }
    pen.light(x, y, if (tick / 3).is_multiple_of(2) { '9' } else { '6' });
    // The shadow, darker and wider as it nears.
    let sh = 1.0 + (1.0 - k) * 3.0;
    for dy in -1..=1 {
        for dx in -(sh as i32)..=(sh as i32) {
            if bayer(x + dx, y + dy) < 0.4 + (1.0 - k) * 0.5 {
                pen.under(x + dx, y + dy, 'k');
            }
        }
    }
    // The shell, falling out of the dark with fire behind it.
    let drop = strike.fall as f32 * 2.5;
    let head = (c.0, c.1 - 2.0 - drop);
    pen.streak(head, (0.15, 1.0), 6.0 + 8.0 * k, ramps::FIRE, 0.15);
    pen.glow(head, 4.5, 1.0, ramps::FIRE, 0.6);
    let (hx, hy) = (head.0.round() as i32, head.1.round() as i32);
    for (dx, dy, ch) in [(-1, -1, 'g'), (0, -1, 'G'), (1, -1, 'g'), (-1, 0, 'G'), (0, 0, 'J'), (1, 0, 'G'), (-1, 1, 'g'), (0, 1, 'G'), (1, 1, 'K')] {
        pen.put(hx + dx, hy + dy, ch);
    }
    pen.put(hx, hy + 1, '@');
}

// ─── Pilgrim's Arrow ─────────────────────────────────────────────────────────

/// The great arrow: a lance of light with a long ribbon of tail, a white
/// head, twinkles shed along its line and a road of gold left on the air
/// behind it; where it goes through a monster, a cross of light breaks.
fn pilgrim(pen: &mut Pen, run: &Run) {
    let r = ramps::HOLY;
    for p in run.projectiles.iter().filter(|p| !p.hostile && p.kind == Shot::Sacred && p.pierce == u8::MAX && p.traits.stun == 45) {
        let head = (p.x * UNIT, p.y * UNIT - 2.0);
        let v = p.vx.hypot(p.vy).max(0.1);
        let (ux, uy) = (p.vx / v, p.vy / v);
        let (nx, ny) = (-uy, ux);
        let per = v * UNIT / HZ as f32;
        let flown = (3 * HZ).saturating_sub(p.ttl) as f32 * per + 4.0;
        // The road it leaves: a thread of gold back to the bow, thinning.
        let road = flown.min(150.0);
        let mut d = 18.0;
        while d < road {
            let q = (head.0 - ux * d, head.1 - uy * d);
            let k = d / road;
            if bayer(q.0 as i32, q.1 as i32) <= 0.85 - k * 0.7 {
                pen.light(q.0.round() as i32, q.1.round() as i32, ramps::GOLD.at(0.35 + k * 0.6));
            }
            d += 1.0;
        }
        let mut pts = [(0.0f32, 0.0f32); 12];
        for (j, pt) in pts.iter_mut().enumerate() {
            let back = (j as f32 * 2.6).min(flown);
            let sway = (back * 0.4 - run.tick as f32 * 0.6).sin() * (j as f32 / 12.0) * 1.2;
            *pt = (head.0 - ux * back + nx * sway, head.1 - uy * back + ny * sway);
        }
        pen.ribbon(&pts, r, 3.2);
        pen.glow(head, 10.0, 1.0, r, 1.0);
        // The head: an arrow of light ahead of the glow.
        for j in 0..5 {
            let t = j as f32;
            for s in [-1.0f32, 1.0] {
                let q = (head.0 - ux * t + nx * s * t * 0.7, head.1 - uy * t + ny * s * t * 0.7);
                pen.light(q.0.round() as i32, q.1.round() as i32, r.step((j / 2).min(3) as usize));
            }
        }
        pen.light(head.0.round() as i32, head.1.round() as i32, 'w');
        pen.light((head.0 + ux).round() as i32, (head.1 + uy).round() as i32, 'w');
        // Twinkles shed along its line, each where the arrow was.
        let along = p.x * ux * UNIT + p.y * uy * UNIT;
        for j in 1..7 {
            let back = j as f32 * 9.0 + along.rem_euclid(9.0);
            if back > flown {
                break;
            }
            let side = (unit(j, (along / 9.0) as i32 - j, 0x91) - 0.5) * 8.0;
            let q = (head.0 - ux * back + nx * side, head.1 - uy * back + ny * side);
            pen.twinkle(q, 2.0, (back / 60.0).min(0.99), if j % 2 == 0 { ramps::GOLD } else { r });
        }
    }
    // Where it went through: a cross of light, rays and stars.
    ranged_events::each(run, |e| {
        if e.hostile || e.kind != Shot::Sacred || e.mark != ranged_events::Mark::Flesh {
            return;
        }
        let Some((c, _)) = cast(run, u32::from(e.owner), Ult::PilgrimsArrow) else { return };
        if !on_line((e.x, e.y), (c.x, c.y), (c.ax, c.ay), 1.5) {
            return;
        }
        let age = f32::from(e.age());
        let at = (e.x * UNIT, e.y * UNIT - 6.0);
        if age < 6.0 {
            pen.glow(at, 12.0, 1.0, r, 1.0 - age / 6.0);
        }
        pfx::rays(pen, at, 4, 20.0, 1.0, 0.0, r, age / 12.0);
        pfx::rays(pen, at, 8, 11.0, 1.0, PI / 8.0, ramps::GOLD, age / 10.0);
        pen.ring(at, 3.0 + ease::out(age / 10.0) * 12.0, 1.2, 1.0, r, ease::inward((age / 12.0).min(1.0)));
        let spray = Spray { gravity: 0.06, ..Spray::round(hash((e.x * 8.0) as i32, (e.y * 8.0) as i32, 0xa7), 10, at, (1.0, 2.8), (10.0, 18.0)) };
        spray.each(age, |m| pfx::mote(pen, &m, Shape::Star, r, 1, run.tick));
    });
}

// ─── Lady's Veil ─────────────────────────────────────────────────────────────

/// The veil coming down: a dome of light running out from Lynette over
/// the whole party, and a shaft of it onto every knight it reaches.
fn veil_dome(pen: &mut Pen, run: &Run, c: &UltCast, age: f32) {
    if age >= 36.0 {
        return;
    }
    let from = (c.x * UNIT, c.y * UNIT - 4.0);
    let r = 6.0 + ease::out(age / 22.0) * 120.0;
    pen.shock(from, r, 2.4, 0.55, VEIL, ease::inward((age / 30.0).min(1.0)));
    if age < 12.0 {
        pen.ring(from, r * 0.7, 1.2, 0.55, ramps::HOLY, age / 12.0);
    }
    for hero in run.players.values().filter(|h| h.hp > 0 && h.angel > 0) {
        let (feet, mid) = wishfx::body(run, hero);
        let reach = ((feet.0 - from.0).hypot(feet.1 - from.1) / 120.0) * 22.0;
        let t = (age - reach) / 14.0;
        if !(0.0..1.0).contains(&t) {
            continue;
        }
        // The shaft: down out of the dark onto them, narrowing as it lands.
        let top = mid.1 - 60.0;
        let low = top + (feet.1 - top) * ease::out((t * 2.0).min(1.0));
        let half = 3.5 * (1.0 - t) + 0.5;
        let mut y = top;
        while y < low {
            for dx in -(half.ceil() as i32)..=(half.ceil() as i32) {
                let x = mid.0.round() as i32 + dx;
                let yy = y.round() as i32;
                let edge = dx.abs() as f32 / half.max(0.5);
                if bayer(x, yy) > (1.0 - edge) * (1.0 - t * 0.6) {
                    continue;
                }
                pen.light(x, yy, VEIL.at(edge * 0.5 + t * 0.4));
            }
            y += 1.0;
        }
        pen.glow(mid, 12.0, 1.1, VEIL, 0.7 * (1.0 - t));
    }
}

/// How open the wings are: spreading with an overshoot in the veil's
/// first ticks; full once it has been on a while.
fn wings_open(run: &Run) -> f32 {
    let born = run.feel.ults.iter().rev().find(|c| c.ult == Ult::LadysVeil).map_or(1.0, |c| ease::back((age_of(run, c) / 10.0).min(1.0)));
    born.max(0.05)
}

/// Lady's Veil's wings, spread behind a knight: off each shoulder the
/// wing's arm sweeps up and out to its tip, lit white along its leading
/// edge, and its feathers hang from it — short by the body, long and
/// leaning out at the tip — cream going to the genie's teal at their ends,
/// beating slowly; a soft light behind; and as the veil ends they come
/// apart into feathers.
fn wings(pen: &mut Pen, run: &Run, id: u32, hero: &Hero) {
    let (feet, mid) = wishfx::body(run, hero);
    let t = run.tick as f32;
    let open = wings_open(run);
    let ending = (hero.angel as f32 / 18.0).min(1.0);
    let beat = (t * 0.2 + id as f32).sin();
    pen.glow((mid.0, mid.1 - 4.0), 16.0 * open.min(1.0), 0.85, VEIL.cooler(1), 0.4 * ending);
    let shown = |x: i32, y: i32| ending >= 1.0 || bayer(x, y) <= ending;
    for side in [-1.0f32, 1.0] {
        let sh = (feet.0 + side * 2.0, feet.1 - 15.0);
        let reach = 14.0 * open.clamp(0.15, 1.12);
        let tip = (sh.0 + side * reach, sh.1 - 11.0 * open.min(1.1) - beat * 2.5);
        let ctl = (sh.0 + side * reach * 0.25, sh.1 - 12.0 * open.min(1.1) - beat * 1.2);
        let arm = |u: f32| {
            let v = 1.0 - u;
            (v * v * sh.0 + 2.0 * v * u * ctl.0 + u * u * tip.0, v * v * sh.1 + 2.0 * v * u * ctl.1 + u * u * tip.1)
        };
        let n = 10;
        for k in 0..n {
            let u = (k as f32 + 0.6) / n as f32;
            let a = arm(u);
            let len = (5.0 + 12.0 * u.powf(0.8) - (k % 2) as f32 * 2.0) * open.clamp(0.35, 1.0);
            let lean = (0.08 + 0.8 * u) + beat * 0.06;
            let dir = (side * lean.sin(), lean.cos());
            let steps = len.ceil() as i32;
            for j in 1..=steps {
                let v = j as f32 / len;
                let (x, y) = ((a.0 + dir.0 * j as f32).round() as i32, (a.1 + dir.1 * j as f32).round() as i32);
                if !shown(x, y) {
                    continue;
                }
                let ch = if v > 0.82 { VEIL.step(if k % 2 == 0 { 3 } else { 4 }) } else { VEIL.at(0.18 + v * 0.42 + (k % 2) as f32 * 0.12) };
                pen.light(x, y, ch);
                // Each feather two wide, its inner half a shade down.
                pen.light(x - side as i32, y, VEIL.at(0.35 + v * 0.45));
            }
        }
        // The arm's leading edge, white.
        for j in 0..=16 {
            let p = arm(j as f32 / 16.0);
            let (x, y) = (p.0.round() as i32, p.1.round() as i32);
            if shown(x, y) {
                pen.light(x, y, VEIL.step(0));
                pen.light(x, y + 1, VEIL.step(1));
            }
        }
    }
    // Folding away: feathers let go off them as the veil ends.
    if hero.angel < 24 {
        let age = 24.0 - hero.angel as f32;
        let spray = Spray { gravity: 0.03, angle: -FRAC_PI_2, spread: 4.0, ..Spray::round(id ^ 0x7e11, 12, (mid.0, mid.1 - 6.0), (0.6, 1.8), (16.0, 26.0)) };
        spray.each(age, |m| pfx::mote(pen, &m, Shape::Petal, VEIL, 2, run.tick));
    }
}

/// Over the veiled: a halo, and a feather now and then drifting down.
fn halos(pen: &mut Pen, run: &Run) {
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.angel > 0) {
        let (feet, _) = wishfx::body(run, hero);
        let t = run.tick as f32;
        let ending = (hero.angel as f32 / 18.0).min(1.0);
        let c = (feet.0, feet.1 - 23.0 + (t * 0.1).sin());
        for i in 0..22 {
            let a = i as f32 / 22.0 * TAU;
            let (x, y) = ((c.0 + a.cos() * 4.5).round() as i32, (c.1 + a.sin() * 1.4).round() as i32);
            if bayer(x, y) > ending {
                continue;
            }
            pen.light(x, y, if a.sin() > 0.0 { '6' } else { '4' });
        }
        pen.twinkle((c.0 + (t * 0.15).cos() * 4.5, c.1 + (t * 0.15).sin() * 1.4), 1.0, (t * 0.05) % 1.0, ramps::HOLY);
        for i in 0..3u32 {
            let life = 40.0;
            let start = unit(i as i32, id as i32, 0xfe) * life;
            let k = ((t + start) % life) / life;
            let loop_n = ((t + start) / life) as i32;
            let x = feet.0 + (unit(i as i32 + loop_n * 7, id as i32, 0xff) - 0.5) * 26.0 + (k * 9.0).sin() * 2.0;
            let y = feet.1 - 20.0 + k * 20.0;
            let m = pfx::Mote { i, at: (x, y), vel: (0.0, 0.4), k: k * 0.8, seed: i + loop_n as u32 };
            pfx::mote(pen, &m, Shape::Petal, VEIL, 2, run.tick);
        }
    }
}

// ─── Bladewind ───────────────────────────────────────────────────────────────

/// Bladewind: wind gathered about Galahad, then at every blink his image
/// left where he stood and a streak of steel to where he lands, and a
/// crescent through the monster at every cut; after, a few motes of steel
/// drifting on the air.
fn bladewind(pen: &mut Pen, run: &Run) {
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0) {
        let Some((c, age)) = cast(run, id, Ult::Bladewind) else { continue };
        let r = ramps::STORM;
        let (_, mid) = wishfx::body(run, hero);
        // The blinks, newest first: the spark that lands on him, the one
        // that landed where that one left, and so on back.
        let mut to = (hero.x, hero.y);
        for n in 0..4 {
            let Some(s) = run.sparks.iter().filter(|s| s.ttl <= 7).find(|s| (s.to.0 - to.0).hypot(s.to.1 - to.1) < 0.3) else { break };
            let a = (s.from.0 * UNIT, s.from.1 * UNIT);
            let b = (s.to.0 * UNIT, s.to.1 * UNIT);
            let k = 1.0 - f32::from(s.ttl) / 7.0;
            // The streak: steel along the dash, its tail thinning.
            if k < 0.7 {
                let pts: Vec<(f32, f32)> = (0..10)
                    .map(|j| {
                        let u = j as f32 / 9.0;
                        (b.0 + (a.0 - b.0) * u, b.1 - 9.0 + (a.1 - b.1) * u - (u * PI).sin() * 3.0)
                    })
                    .collect();
                pen.ribbon(&pts, r, 2.4 * (1.0 - k));
            }
            // His image where he stood, thinning away.
            let mut im = ghost(&super::knights::portrait(hero, id));
            if b.0 < a.0 {
                im = im.flip_h();
            }
            let (x0, y0) = (a.0.round() as i32 - im.w / 2, (a.1 + 2.0).round() as i32 - im.h);
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some(px) = im.get(x, y)
                        && bayer(x0 + x, y0 + y) > k + n as f32 * 0.15
                        && pen.cv.get(x0 + x, y0 + y).is_none()
                    {
                        pen.cv.set(x0 + x, y0 + y, px);
                    }
                }
            }
            to = s.from;
        }
        if hero.slashes > 0 || hero.swing > 0 {
            // The cut: a crescent through the monster in front of him.
            if hero.swing > 2 {
                let k = 1.0 - hero.swing as f32 / 6.0;
                let l = hero.aim_x.hypot(hero.aim_y).max(0.01);
                let (ux, uy) = (hero.aim_x / l, hero.aim_y / l);
                let at = (mid.0 + ux * 12.0, mid.1 + uy * 8.0);
                let flip = if hero.slashes % 2 == 0 { 1.0 } else { -1.0 };
                let base = uy.atan2(ux) + FRAC_PI_2 * flip;
                let mut pts = [(0.0f32, 0.0f32); 12];
                for (j, p) in pts.iter_mut().enumerate() {
                    let u = j as f32 / 11.0;
                    let a = base - flip * u * PI * 0.95 * ease::out(k * 1.6);
                    let rr = 11.0 - u * 3.0;
                    *p = (at.0 + a.cos() * rr, at.1 + a.sin() * rr * 0.8);
                }
                pen.ribbon(&pts, r, 2.6);
                pen.glow(at, 7.0, 1.0, r, 0.8 * (1.0 - k));
            }
            // Wind about him while the blinks last.
            let t = run.tick as f32;
            for arm in 0..2 {
                gust(pen, (mid.0, mid.1 + 3.0 - arm as f32 * 6.0), (11.0, 4.0), t * 0.5 + arm as f32 * PI, 2.2, r.cooler(1), false, 0.0);
            }
        } else if age < 90.0 {
            // After: steel motes drifting off on the air.
            let spray = Spray { gravity: -0.02, angle: -FRAC_PI_2, spread: 3.0, stagger: 2.0, ..Spray::round(seed_of(c), 10, mid, (0.3, 1.0), (20.0, 30.0)) };
            spray.each(age - 30.0, |m| pfx::mote(pen, &m, Shape::Shard, r, 1, run.tick));
        }
        // Wind gathering before the first blink.
        if age < 6.0 {
            for i in 0..10u32 {
                let a = i as f32 / 10.0 * TAU + age * 0.3;
                let rad = 24.0 * (1.0 - ease::inward(age / 6.0)) + 4.0;
                pen.streak((mid.0 + a.cos() * rad, mid.1 + a.sin() * rad * 0.6), (-a.sin(), a.cos() * 0.6), 5.0, r, 0.2);
            }
        }
    }
}

// ─── the Grand Chord ─────────────────────────────────────────────────────────

/// The Grand Chord: three waves of it ringing out off the Composer, every
/// caught monster drawn into the knot along a stream of gold, and the knot
/// itself a turning stave with notes going round it while they are held.
fn chord(pen: &mut Pen, run: &Run, c: &UltCast, age: f32) {
    let from = (c.x * UNIT, c.y * UNIT - 8.0);
    for w in 0..3 {
        let t = (age - w as f32 * 4.0) / 18.0;
        if !(0.0..1.0).contains(&t) {
            continue;
        }
        let r = if w % 2 == 0 { ramps::GOLD } else { ramps::GENIE };
        pen.shock(from, 6.0 + ease::out(t) * 88.0, 2.0, 0.6, r, ease::inward(t));
    }
    let (kx, ky) = (c.x + c.ax * 3.2, c.y + c.ay * 3.2);
    let knot = (kx * UNIT, ky * UNIT);
    // The pulls: each a stream of gold into the knot.
    for s in run.sparks.iter().filter(|s| s.ttl <= 8 && (s.to.0 - kx).hypot(s.to.1 - ky) < 2.5) {
        let a = (s.from.0 * UNIT, s.from.1 * UNIT - 8.0);
        let b = (s.to.0 * UNIT, s.to.1 * UNIT - 8.0);
        let k = 1.0 - f32::from(s.ttl) / 8.0;
        let pts: Vec<(f32, f32)> = (0..10)
            .map(|j| {
                let u = j as f32 / 9.0;
                (b.0 + (a.0 - b.0) * u, b.1 + (a.1 - b.1) * u - (u * PI).sin() * 6.0)
            })
            .collect();
        pen.ribbon(&pts, ramps::GOLD, 2.0 * (1.0 - k) + 0.5);
    }
    // The knot, while any it caught is still held.
    let held = run.enemies.iter().any(|e| e.hp > 0 && e.frozen > 0 && (e.x - kx).hypot(e.y - ky) < 4.0);
    if age < 3.0 || (!held && age > 20.0) {
        return;
    }
    let open = ease::back(((age - 3.0) / 8.0).min(1.0));
    let fade = if held { 0.0 } else { ((age - 12.0) / 8.0).clamp(0.0, 1.0) };
    let mid = (knot.0, knot.1 - 10.0);
    // The stave: five lines bent round the knot, turning.
    for line in 0..5 {
        let rr = (12.0 + line as f32 * 1.6) * open;
        let n = (rr * 3.0) as i32;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU + age * 0.04 * if line % 2 == 0 { 1.0 } else { -1.0 };
            if (i + line * 3) % 9 < 3 {
                continue;
            }
            let (x, y) = ((mid.0 + a.cos() * rr).round() as i32, (mid.1 + 6.0 + a.sin() * rr * 0.35).round() as i32);
            if bayer(x, y) < fade {
                continue;
            }
            if a.sin() > 0.0 {
                pen.light(x, y, ramps::GOLD.step(2 + line as usize % 2));
            } else {
                pen.under(x, y, ramps::GOLD.step(4));
            }
        }
    }
    for i in 0..5u32 {
        let a = i as f32 / 5.0 * TAU + age * 0.08;
        let bob = ((age + i as f32 * 5.0) * 0.3).sin() * 2.0;
        let at = (mid.0 + a.cos() * 15.0 * open, mid.1 + a.sin() * 5.0 * open + bob);
        let m = pfx::Mote { i, at, vel: (0.0, -1.0), k: 0.1 + fade * 0.8, seed: i };
        pfx::mote(pen, &m, Shape::Note, if i % 2 == 0 { ramps::GOLD } else { ramps::GENIE }, 1, run.tick);
    }
}

// ─── the Sealed Writs ────────────────────────────────────────────────────────

/// A writ in flight, nine by six: a folded letter in a dark edge, its
/// flap's fold, its red seal at the fold's point.
const LETTER: [&str; 6] = ["BBBBBBBBB", "BtTTTTTtB", "BTtTTTtTB", "BTTt7tTTB", "BTTT$TTTB", "BBBBBBBBB"];

/// The Sealed Writs: each a letter flying home on a ribbon of red, paper
/// flecks shed behind it; where one strikes, its wax breaks in a ring with
/// red drops and scraps of paper thrown.
fn writs(pen: &mut Pen, run: &Run) {
    let letter = Img::from_rows(&LETTER);
    for p in run.projectiles.iter().filter(|p| !p.hostile && p.kind == Shot::Missile && p.traits.homing == 8 && p.traits.burst == 40) {
        let head = (p.x * UNIT, p.y * UNIT - 3.0);
        let v = p.vx.hypot(p.vy).max(0.1);
        let (ux, uy) = (p.vx / v, p.vy / v);
        let age = (4 * HZ).saturating_sub(p.ttl) as f32;
        let flown = age * v * UNIT / HZ as f32;
        let pts: Vec<(f32, f32)> = (0..9)
            .map(|j| {
                let back = (j as f32 * 2.2).min(flown);
                let wave = (back * 0.5 - run.tick as f32 * 0.7).sin() * (j as f32 / 9.0) * 1.5;
                (head.0 - ux * back - uy * wave, head.1 - uy * back + ux * wave)
            })
            .collect();
        pen.ribbon(&pts, WAX, 1.6);
        let (x0, y0) = (head.0.round() as i32 - 4, head.1.round() as i32 - 3);
        // Its corners lift and fall as it flies.
        let flutter = i32::from((run.tick / 3 + p.x as u64).is_multiple_of(4));
        for y in 0..letter.h {
            for x in 0..letter.w {
                if let Some(c) = letter.get(x, y) {
                    let lift = if (x == 0 || x == letter.w - 1) && y == 0 { flutter } else { 0 };
                    pen.cv.set(x0 + x, y0 + y - lift, c);
                }
            }
        }
        // Flecks of paper off it, each where it was.
        for j in 0..3u32 {
            let back = 5.0 + j as f32 * 5.0 + (age * 1.3).rem_euclid(5.0);
            if back > flown {
                continue;
            }
            let k = back / 20.0;
            let at = (head.0 - ux * back + (unit(j as i32, age as i32 / 4, 0x3f) - 0.5) * 6.0, head.1 - uy * back + k * 6.0);
            let m = pfx::Mote { i: j, at, vel: (-ux, -uy), k, seed: j + age as u32 };
            pfx::mote(pen, &m, Shape::Petal, PAPER, 1, run.tick);
        }
    }
    ranged_events::each(run, |e| {
        if e.hostile || e.kind != Shot::Missile || matches!(e.mark, ranged_events::Mark::Release) {
            return;
        }
        if cast(run, u32::from(e.owner), Ult::SealedWrits).is_none() {
            return;
        }
        let age = f32::from(e.age());
        let at = (e.x * UNIT, e.y * UNIT - 5.0);
        let s = hash((e.x * 8.0) as i32, (e.y * 8.0) as i32, 0x5e);
        if age < 5.0 {
            pen.glow(at, 9.0, 1.0, ramps::ROSE, 1.0 - age / 5.0);
        }
        // The seal breaking: a ring with a cross through it.
        let r = 4.0 + ease::out(age / 8.0) * 9.0;
        pen.ring(at, r, 1.2, 1.0, WAX, ease::inward((age / 12.0).min(1.0)));
        if age < 8.0 {
            pen.line((at.0 - r * 0.6, at.1 - r * 0.6), (at.0 + r * 0.6, at.1 + r * 0.6), WAX.step(1));
            pen.line((at.0 + r * 0.6, at.1 - r * 0.6), (at.0 - r * 0.6, at.1 + r * 0.6), WAX.step(1));
        }
        let drops = Spray { gravity: 0.16, angle: -FRAC_PI_2, spread: 4.0, ..Spray::round(s, 10, at, (1.2, 3.0), (10.0, 16.0)) };
        drops.each(age, |m| pfx::mote(pen, &m, Shape::Droplet, WAX, 1, run.tick));
        let scraps = Spray { gravity: 0.03, ..Spray::round(s ^ 1, 8, at, (0.6, 2.0), (14.0, 20.0)) };
        scraps.each(age, |m| pfx::mote(pen, &m, Shape::Petal, PAPER, 2, run.tick));
    });
}

// ─── Midas Touch ─────────────────────────────────────────────────────────────

/// Midas Touch: a stream of gold out of the Money Knight's hand to each
/// monster it takes, the monster gilded where it stood — a statue with a
/// shine running over it — and then crumbling from the top into coin.
fn midas(pen: &mut Pen, run: &Run, c: &UltCast, hero: &Hero, age: f32) {
    use crate::rules::feel::DEATH_TICKS;
    let from = hand(run, hero);
    let s = seed_of(c);
    for (n, d) in run.feel.deaths.iter().enumerate() {
        let dage = f32::from(DEATH_TICKS.saturating_sub(d.ttl));
        if (dage - age).abs() > 2.0 || (d.x - c.x).hypot(d.y - c.y) > 13.5 {
            continue;
        }
        let feet = (d.x * UNIT, d.y * UNIT);
        let mid = (feet.0, feet.1 - d.r * UNIT);
        // The touch: gold arcing out to it, head first.
        if age < 10.0 {
            let k = ease::out(age / 5.0);
            let pts: Vec<(f32, f32)> = (0..12)
                .map(|j| {
                    let u = (1.0 - j as f32 / 11.0) * k;
                    let h = (u * PI).sin() * 14.0;
                    (from.0 + (mid.0 - from.0) * u, from.1 + (mid.1 - from.1) * u - h)
                })
                .collect();
            pen.ribbon(&pts, ramps::GOLD, 2.2);
        }
        // The statue, crumbling from the top as the coin goes.
        if let Some(kind) = d.kind {
            let im = super::stature::monster(kind, super::sprites::enemy(kind, 0), (d.r / kind.radius()).max(1.0));
            // Gold climbing it from the feet as the touch lands, a shine
            // running over it, then it goes from the top down into coin.
            let gilt = im.h as f32 * (1.0 - ease::out(((age - 2.0) / 4.0).clamp(0.0, 1.0)));
            let crumble = ((age - 8.0) / 6.0).clamp(0.0, 1.0);
            let band = ((age - 5.0) * 3.0) as i32 - 4;
            let (x0, y0) = (feet.0.round() as i32 - im.w / 2, feet.1.round() as i32 - im.h + 1);
            let lum = |[r, g, b]: [u8; 3]| (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
            let im = &im;
            let (lo, hi) = (0..im.h)
                .flat_map(|y| (0..im.w).filter_map(move |x| im.get(x, y)))
                .fold((u32::MAX, 0), |(lo, hi), c| (lo.min(lum(c)), hi.max(lum(c))));
            for y in 0..im.h {
                for x in 0..im.w {
                    let Some(c) = im.get(x, y) else { continue };
                    if (y as f32) < crumble * im.h as f32 && bayer(x0 + x, y0 + y) < 0.9 {
                        continue;
                    }
                    if (y as f32) < gilt {
                        pen.cv.set(x0 + x, y0 + y, c);
                        continue;
                    }
                    // Its own light, spread over the gold: dark in its
                    // creases, bright on its faces, its rim in old gold.
                    let l = (lum(c) - lo.min(lum(c))) as f32 / (hi.saturating_sub(lo)).max(1) as f32;
                    let shine = age >= 5.0 && (x + y - band).abs() <= 1;
                    let edge = (y as f32 - gilt).abs() < 1.0 && age < 6.0;
                    let ch = if shine || edge { '9' } else if im.edge(x, y) { 'a' } else { ['r', 'O', '4', '@', '5', '6'][((l * 6.0) as usize).min(5)] };
                    pen.put(x0 + x, y0 + y, ch);
                }
            }
            pen.glow(mid, 9.0, 1.0, ramps::GOLD, 0.35 * (1.0 - crumble));
        }
        let coins = Spray { gravity: 0.18, angle: -FRAC_PI_2, spread: 3.6, ..Spray::round(s ^ n as u32, if pen.calm { 8 } else { 16 }, mid, (1.2, 3.2), (14.0, 24.0)) };
        coins.each(age - 8.0, |m| pfx::mote(pen, &m, Shape::Coin, ramps::GOLD, 1, run.tick));
        if age > 8.0 {
            pfx::rays(pen, mid, 8, 14.0, 0.9, 0.3, ramps::GOLD, (age - 8.0) / 12.0);
        }
    }
}

// ─── Assassinate ─────────────────────────────────────────────────────────────

/// Assassinate: the aim a reticle closing and turning on the room's
/// toughest monster with a red sight drawn to it from the knight; the shot
/// a white-hot tracer with a flash and smoke at the muzzle; where it goes
/// through, a red burst.
fn assassin(pen: &mut Pen, run: &Run) {
    let r = ramps::BLOOD;
    for hero in run.players.values().filter(|h| h.hp > 0) {
        let Some((target, left)) = hero.aiming else { continue };
        let Some(enemy) = run.enemies.iter().find(|e| e.id == target) else { continue };
        let (_, mid, h) = foe_mid(run, enemy);
        let t = 1.0 - left as f32 / HZ as f32;
        let hot = left < 8;
        let rad = (h * 0.5 + 12.0) * (1.0 - 0.55 * ease::out(t)) + 3.0;
        pen.ring(mid, rad, 0.9, 1.0, if hot && run.tick % 2 == 0 { TRACER } else { r }, 0.15);
        let rot = t * 1.4;
        for q in 0..4 {
            let a = rot + q as f32 * FRAC_PI_2;
            let (ux, uy) = (a.cos(), a.sin());
            pen.line((mid.0 + ux * (rad + 4.0), mid.1 + uy * (rad + 4.0)), (mid.0 + ux * (rad - 3.0), mid.1 + uy * (rad - 3.0)), if hot { 'w' } else { '7' });
        }
        pen.light(mid.0.round() as i32, mid.1.round() as i32, if run.tick % 4 < 2 { 'w' } else { '7' });
        // The sight: dashes from the hand to it, a bead running along.
        let from = hand(run, hero);
        let (dx, dy) = (mid.0 - from.0, mid.1 - from.1);
        let len = dx.hypot(dy).max(1.0);
        let bead = (run.tick as f32 * 3.0) % len;
        let mut d = 4.0;
        while d < len - rad {
            if ((d as i32) / 2) % 3 != 0 {
                let q = (from.0 + dx / len * d, from.1 + dy / len * d);
                pen.light(q.0.round() as i32, q.1.round() as i32, if (d - bead).abs() < 3.0 { 'c' } else { r.step(3) });
            }
            d += 1.0;
        }
        pen.twinkle(from, 2.0, (t * 3.0) % 1.0, TRACER);
    }
    for p in run.projectiles.iter().filter(|p| !p.hostile && p.kind == Shot::Sacred && p.pierce == u8::MAX && p.traits.stun == 0) {
        if cast(run, u32::from(p.traits.owner), Ult::Assassinate).is_none() {
            continue;
        }
        let head = (p.x * UNIT, p.y * UNIT - 6.0);
        let v = p.vx.hypot(p.vy).max(0.1);
        let age = (2 * HZ).saturating_sub(p.ttl) as f32;
        let flown = age * v * UNIT / HZ as f32;
        let muzzle = (head.0 - p.vx / v * flown, head.1 - p.vy / v * flown);
        pen.streak(head, (p.vx, p.vy), flown.min(40.0), TRACER, 0.0);
        pen.glow(head, 6.0, 1.0, r, 0.9);
        if age < 8.0 {
            pen.glow(muzzle, 9.0, 1.0, TRACER, 1.0 - age / 8.0);
            pfx::rays(pen, muzzle, 6, 12.0, 1.0, p.vy.atan2(p.vx), TRACER, age / 8.0);
        }
        if age < 30.0 {
            pen.wisp(muzzle, age, (0.0, 6.0), 22.0, 12.0, 2.5, ramps::SMOKE, p.ttl);
        }
    }
    ranged_events::each(run, |e| {
        if e.hostile || e.kind != Shot::Sacred || e.mark != ranged_events::Mark::Flesh {
            return;
        }
        let Some((c, cage)) = cast(run, u32::from(e.owner), Ult::Assassinate) else { return };
        if cage < HZ as f32 - 2.0 || cast(run, u32::from(e.owner), Ult::PilgrimsArrow).is_some_and(|(p, _)| p.tick > c.tick) {
            return;
        }
        let age = f32::from(e.age());
        let at = (e.x * UNIT, e.y * UNIT - 6.0);
        if age < 5.0 {
            pen.glow(at, 12.0, 1.0, TRACER, 1.0 - age / 5.0);
        }
        pfx::rays(pen, at, 10, 18.0, 1.0, e.dy.atan2(e.dx), r, age / 12.0);
        pen.shock(at, 3.0 + ease::out(age / 12.0) * 18.0, 1.6, 0.8, r, ease::inward((age / 14.0).min(1.0)));
        let drops = Spray { angle: e.dy.atan2(e.dx), spread: 1.8, gravity: 0.14, ..Spray::round(hash((e.x * 8.0) as i32, 3, 0xa55), 14, at, (1.5, 4.0), (10.0, 18.0)) };
        drops.each(age, |m| pfx::mote(pen, &m, Shape::Droplet, r, 2, run.tick));
    });
}

// ─── Phantasm ────────────────────────────────────────────────────────────────

/// The Phantasm's images: each slides out of its knight with a streak of
/// arcane light and a flash where it stops, stands as a ghost of them with
/// a shimmer running down it and a thread back to them, and thins away
/// into the dither as it goes.
fn images(pen: &mut Pen, run: &Run, id: u32, hero: &Hero) {
    let (hfeet, _) = wishfx::body(run, hero);
    for phantom in run.phantoms.iter().filter(|p| p.owner == id) {
        let age = PHANTOM_TICKS.saturating_sub(phantom.left) as f32;
        let (x, y) = at(phantom.x, phantom.y);
        let goal = (x as f32, y as f32);
        let k = ease::out((age / 8.0).min(1.0));
        let feet = (hfeet.0 + (goal.0 - hfeet.0) * k, hfeet.1 + (goal.1 - hfeet.1) * k);
        let r = ramps::ARCANE;
        if age < 10.0 {
            let pts: Vec<(f32, f32)> = (0..8)
                .map(|j| {
                    let u = j as f32 / 7.0;
                    (feet.0 + (hfeet.0 - feet.0) * u, feet.1 - 9.0 + (hfeet.1 - feet.1) * u)
                })
                .collect();
            pen.ribbon(&pts, r, 2.0);
        }
        if (7.0..18.0).contains(&age) {
            pen.ring((feet.0, feet.1 - 9.0), 4.0 + (age - 7.0) * 1.4, 1.0, 1.0, r, (age - 7.0) / 11.0);
        }
        let thin = if phantom.left < 36 { phantom.left as f32 / 36.0 } else { 1.0 };
        pen.glow((feet.0, feet.1 + 1.0), 9.0, 0.42, r.cooler(1), 0.45 * thin);
        let mut im = ghost(&super::knights::portrait(hero, id));
        if hero.aim_x < -0.3 {
            im = im.flip_h();
        }
        let band = ((run.tick as f32 * 0.7 + phantom.dx * 9.0).rem_euclid(im.h as f32 + 10.0)) as i32 - 4;
        let (x0, y0) = (feet.0.round() as i32 - im.w / 2, feet.1.round() as i32 - im.h + 1);
        for py in 0..im.h {
            for px in 0..im.w {
                let Some(c) = im.get(px, py) else { continue };
                if bayer(x0 + px, y0 + py + (run.tick / 3) as i32) > thin {
                    continue;
                }
                if (py - band).abs() <= 1 {
                    pen.cv.put(x0 + px, y0 + py, if py == band { 'w' } else { '3' });
                } else {
                    pen.cv.set(x0 + px, y0 + py, c);
                }
            }
        }
        // The thread back to the knight, a bead of light running along it.
        if age > 8.0 && thin > 0.3 {
            let a = (feet.0, feet.1 - 6.0);
            let b = (hfeet.0, hfeet.1 - 6.0);
            let len = (b.0 - a.0).hypot(b.1 - a.1).max(1.0);
            let bead = (run.tick as f32 * 1.5) % len;
            let mut d = 3.0;
            while d < len - 3.0 {
                let q = (a.0 + (b.0 - a.0) * d / len, a.1 + (b.1 - a.1) * d / len - (d / len * PI).sin() * 3.0);
                let on_bead = (d - bead).abs() < 2.0;
                if (d as i32) % 3 == 0 || on_bead {
                    pen.light(q.0.round() as i32, q.1.round() as i32, if on_bead { '3' } else { r.step(4) });
                }
                d += 1.0;
            }
        }
    }
}

/// A knight's image: their figure in the genie's blues by brightness, lit
/// at its edge, and only most of it there.
fn ghost(im: &Img) -> Img {
    const RAMP: [char; 5] = ['q', '1', '2', '3', 'w'];
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                if im.edge(x, y) {
                    out.put(x, y, '3');
                    continue;
                }
                if bayer(x, y) < 0.22 {
                    continue;
                }
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(x, y, RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)]);
            }
        }
    }
    out
}

// ─── winds ───────────────────────────────────────────────────────────────────

/// A band of wind going round `c` on an ellipse `rx` by `ry`: from angle
/// `from` back through `span`, hot at its head and thinning to its tail;
/// in front of what it goes round lit over it, behind it only on bare
/// paper. `thin` (0 to 1) wears all of it away.
#[allow(clippy::too_many_arguments)]
fn gust(pen: &mut Pen, c: (f32, f32), (rx, ry): (f32, f32), from: f32, span: f32, ramp: Ramp, wide: bool, thin: f32) {
    let n = (span * rx.max(ry)).ceil().max(2.0) as i32;
    for i in 0..n {
        let f = i as f32 / n as f32;
        let a = from - f * span;
        let (x, y) = ((c.0 + a.cos() * rx).round() as i32, (c.1 + a.sin() * ry).round() as i32);
        if (f > 0.55 && bayer(x, y) < (f - 0.55) * 2.2) || bayer(x + 3, y) < thin {
            continue;
        }
        if a.sin() > -0.1 {
            pen.light(x, y, ramp.at(0.05 + f * 0.75));
            if wide && f < 0.7 {
                pen.light(x, y + 1, ramp.at(0.3 + f * 0.7));
            }
        } else {
            pen.under(x, y, ramp.at(0.45 + f * 0.55));
        }
    }
}

// ─── the Axe Storm ───────────────────────────────────────────────────────────

/// How hard Dame Thrudi's storm blows: up over its first ticks, down over
/// its last.
fn whirl_power(hero: &Hero) -> f32 {
    let age = (3 * HZ).saturating_sub(hero.whirl) as f32;
    (hero.whirl as f32 / 12.0).min(1.0) * (age / 6.0).min(1.0)
}

/// Under Dame Thrudi's storm: a ring of wind-scoured floor and dust.
fn whirl_floor(pen: &mut Pen, run: &Run) {
    for hero in run.players.values().filter(|h| h.hp > 0 && h.whirl > 0) {
        let (feet, _) = wishfx::body(run, hero);
        let k = whirl_power(hero);
        pen.ring((feet.0, feet.1 + 1.0), 15.0, 1.6, 0.42, ramps::SMOKE, 1.0 - k * 0.8);
        pen.ring((feet.0, feet.1 + 1.0), 10.0, 1.0, 0.42, ramps::STORM.cooler(2), 1.0 - k * 0.6);
    }
}

/// The Axe Storm: a whirlwind about her — four ribbons of wind going
/// round, wider as they climb, in front of her and behind, her axes
/// glinting on their circle, dust kicked out off the floor; gathering at
/// the start, settling at the end.
fn axe_storm(pen: &mut Pen, run: &Run) {
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.whirl > 0) {
        let (feet, mid) = wishfx::body(run, hero);
        let age = (3 * HZ).saturating_sub(hero.whirl) as f32;
        let t = run.tick as f32;
        let k = whirl_power(hero);
        let r = ramps::STORM;
        for arm in 0..5 {
            let level = arm as f32 / 4.0;
            let rr = 9.0 + level * 9.0 + (t * 0.2 + arm as f32).sin();
            let c = (mid.0, feet.1 - 1.0 - level * 20.0);
            gust(pen, c, (rr, rr * 0.3), t * 0.6 + arm as f32 * 1.9, 3.8, r, true, 1.0 - k);
            gust(pen, (c.0, c.1 - 1.0), (rr - 1.0, rr * 0.3), t * 0.6 + arm as f32 * 1.9 - 0.3, 2.0, r.cooler(1), false, 1.0 - k);
        }
        // What the wind has picked up, going round in it.
        for i in 0..6u32 {
            let level = unit(i as i32, id as i32, 0xa1);
            let a = t * (0.5 + level * 0.2) + i as f32 * 1.7;
            let rr = 9.0 + level * 9.0;
            if a.sin() > -0.1 && k > 0.3 {
                let at = (mid.0 + a.cos() * rr, feet.1 - 2.0 - level * 20.0 + a.sin() * rr * 0.3);
                let m = pfx::Mote { i, at, vel: (-a.sin(), 0.3 * a.cos()), k: 0.3, seed: i ^ id };
                pfx::mote(pen, &m, Shape::Shard, ramps::BONE, 1, run.tick);
            }
        }
        for axe in 0..2 {
            let a = t * 0.6 + axe as f32 * PI;
            if a.sin() > -0.1 {
                let at = (mid.0 + a.cos() * 13.0, mid.1 + 1.0 + a.sin() * 4.0);
                pen.twinkle(at, 2.5, ((t * 0.1 + axe as f32 * 0.5) % 1.0).min(0.99), r);
            }
        }
        // Dust off the floor, thrown out by the wind.
        if k > 0.2 {
            for wave in 0..2 {
                let cycle = (age as i32) / 8 - wave;
                let since = age - (cycle * 8) as f32;
                let spray = Spray {
                    seed: hash(cycle, id as i32, 0xd5),
                    n: if pen.calm { 4 } else { 8 },
                    at: feet,
                    angle: 0.0,
                    spread: TAU,
                    speed: (1.4, 2.8),
                    life: (12.0, 18.0),
                    drag: 0.12,
                    gravity: -0.03,
                    squash: 0.35,
                    stagger: 0.0,
                };
                spray.each(since, |m| pfx::mote(pen, &m, Shape::Smoke, ramps::SMOKE, 1, run.tick));
            }
        }
    }
}

// ─── the champions' ──────────────────────────────────────────────────────────

/// The Earthsplitter's seam under the figures: molten light along it as
/// it runs and before it bursts, brighter as the burst nears; sparks off
/// its running tip; the burst all along it, rock thrown up; and once it
/// has gone, smoke curling off the split.
fn seams(pen: &mut Pen, run: &Run) {
    use crate::rules::champions::{CRACK_ERUPT, CRACK_LIFE, CRACK_RUN};
    for c in &run.champions.cracks {
        let (x0, y0) = (c.x * UNIT, c.y * UNIT);
        let grown = (c.age as f32 / CRACK_RUN as f32).min(1.0) * c.len * UNIT;
        let hot = (c.age as f32 / CRACK_ERUPT as f32).min(1.0);
        let gone = c.age >= CRACK_ERUPT;
        let mut d = 0.0;
        let mut n = 0;
        while d < grown {
            let p = (x0 + c.dx * d, y0 + c.dy * d * 0.6);
            if !gone {
                // Molten in the seam, brighter and quicker to pulse as the
                // burst comes.
                if n % 2 == 0 {
                    pen.glow(p, 4.0 + hot * 4.0, 0.5, ramps::FIRE, 0.35 + 0.6 * hot);
                }
                let pulse = ((run.tick as f32 * (0.2 + hot * 0.5) + n as f32 * 0.7).sin() * 0.5 + 0.5) * hot;
                pen.light(p.0.round() as i32, p.1.round() as i32, ramps::FIRE.at(0.5 - pulse * 0.5));
            }
            if gone && c.age < CRACK_ERUPT + 6 && n % 2 == 0 {
                // The burst: fire out of the seam all along it at once.
                pen.glow((p.0, p.1 - 3.0), 7.0, 0.8, ramps::FIRE, 1.0 - (c.age - CRACK_ERUPT) as f32 / 6.0);
            }
            if gone && c.age < CRACK_LIFE && n % 3 == 0 {
                // Cooling: embers let go off the split and rising.
                let since = (c.age - CRACK_ERUPT) as f32;
                let spray = Spray { gravity: -0.04, angle: -FRAC_PI_2, spread: 1.2, stagger: 3.0, ..Spray::round(hash(n, c.owner as i32, 0xe7), 3, p, (0.3, 0.8), (14.0, 20.0)) };
                spray.each(since, |m| pfx::mote(pen, &m, Shape::Ember, ramps::EMBER, 1, run.tick));
            }
            d += 4.0;
            n += 1;
        }
        if c.age < CRACK_RUN {
            let tip = (x0 + c.dx * grown, y0 + c.dy * grown * 0.6);
            pen.glow(tip, 6.0, 0.8, ramps::FIRE, 0.9);
            for back in 0..3 {
                let since = (c.age % 4) as f32 + back as f32 * 4.0;
                let from = (tip.0 - c.dx * since * 2.0, tip.1 - c.dy * since * 1.2);
                let spray = Spray { gravity: 0.2, angle: -FRAC_PI_2, spread: 2.4, ..Spray::round(hash(c.age as i32 / 4 - back, c.owner as i32, 0x5c), 4, from, (1.0, 2.6), (6.0, 10.0)) };
                spray.each(since, |m| pfx::mote(pen, &m, Shape::Spark, ramps::FIRE, 1, run.tick));
            }
        }
        if (CRACK_ERUPT..CRACK_ERUPT + 16).contains(&c.age) {
            let since = (c.age - CRACK_ERUPT) as f32;
            let mut d = 6.0;
            let mut n = 0;
            while d < grown {
                let p = (x0 + c.dx * d, y0 + c.dy * d * 0.6);
                if n % 2 == 0 {
                    pen.shock(p, 3.0 + ease::out(since / 10.0) * 9.0, 1.2, 0.5, ramps::EARTH, ease::inward(since / 16.0));
                }
                let rocks = Spray { gravity: 0.22, angle: -FRAC_PI_2, spread: 1.8, ..Spray::round(hash(n, c.owner as i32, 0x60c), if pen.calm { 2 } else { 3 }, (p.0, p.1 - 2.0), (1.6, 3.4), (10.0, 16.0)) };
                rocks.each(since, |m| pfx::mote(pen, &m, Shape::Shard, if n % 3 == 0 { ramps::BONE } else { ramps::EARTH }, 2, run.tick));
                d += 10.0;
                n += 1;
            }
        }
    }
}

/// Dismember: the monster held up clear of the Butcher on his hook
/// (`champions::hoist`), and at every cut a crescent through it, a spray
/// of blood off it, and the blood drawn home in a stream to his chest.
fn dismember(pen: &mut Pen, run: &Run) {
    use crate::rules::champions::GRIP_TICKS;
    /// How often the cleaver falls: on the tens (`champions::GRIP_EVERY`).
    const EVERY: u64 = 10;
    for g in &run.champions.grips {
        let (Some(owner), Some(foe)) = (run.players.get(&g.owner), run.enemies.iter().find(|e| e.id == g.foe)) else {
            continue;
        };
        let age = GRIP_TICKS.saturating_sub(g.left);
        let cut = run.tick / EVERY;
        let since = (run.tick % EVERY) as f32;
        let (_, mid, h) = foe_mid(run, foe);
        let (_, chest) = wishfx::body(run, owner);
        let s = hash(cut as i32, g.foe as i32, 0xd15);
        // The cut: one crescent through it, the way the cleaver went.
        if since < 4.0 && age >= 2 {
            let flip = if cut % 2 == 0 { 1.0 } else { -1.0 };
            let k = since / 4.0;
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|j| {
                    let u = j as f32 / 9.0 * ease::out(k * 1.5).min(1.0);
                    let a = -2.3 + u * 1.7;
                    (mid.0 + flip * a.cos() * h * 0.55, mid.1 + a.sin() * h * 0.45 + h * 0.2)
                })
                .collect();
            pen.ribbon(&pts, TRACER, 2.0);
            if since < 1.0 {
                pen.twinkle(mid, 3.0, 0.5, TRACER);
            }
        }
        let drops = Spray { gravity: 0.22, angle: -FRAC_PI_2, spread: 2.6, ..Spray::round(s, if pen.calm { 4 } else { 7 }, mid, (1.0, 2.4), (9.0, 14.0)) };
        drops.each(since, |m| pfx::mote(pen, &m, Shape::Droplet, ramps::BLOOD, 2, run.tick));
        // What it gives him: drops of it drawn home along an arc over the
        // gap between them.
        for i in 0..3u32 {
            let t = ((since + i as f32 * 3.3) / 10.0) % 1.0;
            let lift = (t * PI).sin() * 6.0;
            let e = ease::inward(t);
            let at = (mid.0 + (chest.0 - mid.0) * e, mid.1 + (chest.1 - mid.1) * e - lift);
            let m = pfx::Mote { i, at, vel: (chest.0 - mid.0, chest.1 - mid.1), k: 0.2 + t * 0.5, seed: i };
            pfx::mote(pen, &m, Shape::Dot, ramps::BLOOD, 2, run.tick);
        }
        if since < 6.0 && age >= 2 {
            pen.glow(chest, 6.0, 1.0, ramps::BLOOD.cooler(1), 0.45 * (1.0 - since / 6.0));
        }
    }
}

/// The Philosopher's Rain under the figures: where each flask will break,
/// a mark of its brew drawing in as it comes down.
fn flask_marks(pen: &mut Pen, run: &Run) {
    use crate::rules::champions::{Brew, Thrown};
    for t in &run.champions.throws {
        let Thrown::Flask(brew) = t.kind else { continue };
        if brew == Brew::Fire || cast(run, t.owner, Ult::PhilosophersRain).is_none() {
            continue;
        }
        let r = if brew == Brew::Foul { ramps::POISON } else { ramps::FIRE };
        let k = (t.age as f32 / t.life.max(1) as f32).clamp(0.0, 1.0);
        let at = (t.to.0 * UNIT, t.to.1 * UNIT + 1.0);
        let rad = 14.0 * (1.0 - ease::out(k)) + 6.0;
        pen.glyph(at, rad, 0.45, t.age as f32 * 0.08, ease::out((k * 3.0).min(1.0)), 6, r, 0.15, hash(t.owner as i32, t.life as i32, 0xf1));
    }
}

/// The flasks of the Philosopher's Rain in the air: each lit with its
/// brew, a ribbon of it trailing along its arc.
fn flasks(pen: &mut Pen, run: &Run) {
    use crate::rules::champions::{Brew, Thrown, lob};
    for t in &run.champions.throws {
        let Thrown::Flask(brew) = t.kind else { continue };
        if brew == Brew::Fire || cast(run, t.owner, Ult::PhilosophersRain).is_none() {
            continue;
        }
        let r = if brew == Brew::Foul { ramps::POISON } else { ramps::FIRE };
        let pts: Vec<(f32, f32)> = (0..8)
            .map(|j| {
                let mut was = *t;
                was.age = t.age.saturating_sub(j);
                let ((x, y), high) = lob(&was);
                (x * UNIT, (y - high) * UNIT - 8.0)
            })
            .collect();
        pen.ribbon(&pts, r, 1.8);
        pen.glow(pts[0], 6.0, 1.0, r, 0.7);
    }
}

/// The Tombstone's floor: a ring of grave-light about it with a rune of
/// six turning slowly, guttering at the end; grave-light under the dead as
/// they come up.
fn graves(pen: &mut Pen, run: &Run) {
    for stone in &run.minions.stones {
        let (x, y) = at(stone.x, stone.y);
        let c = (x as f32, y as f32 + 2.0);
        let open = ease::back((stone.age as f32 / 10.0).min(1.0));
        let gutter = if stone.left < 2 * HZ { if (run.tick / 3) % 3 == 0 { 0.6 } else { 0.2 } } else { 0.1 };
        pen.glow(c, 18.0 * open.max(0.3), 0.45, GRAVE.cooler(2), 0.5 * (1.0 - gutter));
        pen.glyph(c, 16.0 * open.max(0.3), 0.45, stone.age as f32 * 0.02, open.min(1.0), 6, GRAVE, gutter, stone.owner ^ 0x6a);
    }
    for minion in run.minions.raised.iter().filter(|m| !m.down() && m.rising > 0) {
        let (x, y) = at(minion.x, minion.y);
        pen.glow((x as f32, y as f32), 10.0, 0.5, GRAVE, 0.7);
    }
}

/// The Tombstone over the figures: the pillar of grave-fire it comes down
/// in, wisps curling up off it while it stands, and a light in its cut.
fn tombstones(pen: &mut Pen, run: &Run) {
    for stone in &run.minions.stones {
        let (x, y) = at(stone.x, stone.y);
        let c = (x as f32, y as f32);
        let age = stone.age as f32;
        if age < 16.0 {
            pillar(pen, c, 6.0 * (1.0 - age / 16.0) + 1.0, 80.0, GRAVE, age / 16.0, run.tick);
        }
        for i in 0..3u32 {
            let base = (c.0 - 5.0 + i as f32 * 5.0, c.1 - 4.0);
            let emit = ((stone.age / 20) * 20) as f32;
            pen.wisp(base, age, (emit, emit + 10.0), 26.0, 18.0, 2.5, GRAVE.cooler(1), stone.owner ^ i);
        }
        let flick = if stone.left < 2 * HZ && (run.tick / 3) % 3 == 0 { 0.2 } else { 0.7 };
        pen.glow((c.0, c.1 - 10.0), 5.0, 1.0, GRAVE, flick);
    }
}

// ─── the heroes' ─────────────────────────────────────────────────────────────

/// The Last Light: a reticle of light closing on its mark through the
/// second it holds, a faint sight to it; then the shot — a beam with a
/// white core, a column of light out of the sky onto the mark, and a burst.
fn last_light(pen: &mut Pen, run: &Run) {
    let r = ramps::HOLY;
    for m in &run.heroes.marks {
        let (Some(e), Some(owner)) = (run.enemies.iter().find(|e| e.id == m.target), run.players.get(&m.owner)) else { continue };
        let (_, mid, h) = foe_mid(run, e);
        let t = 1.0 - m.left as f32 / HZ as f32;
        let rad = (h * 0.5 + 12.0) * (1.0 - 0.5 * ease::out(t)) + 3.0;
        pen.glyph(mid, rad, 1.0, t * 2.0, ease::out((t * 2.5).min(1.0)), 4, r, 0.1, m.target);
        let from = hand(run, owner);
        let (dx, dy) = (mid.0 - from.0, mid.1 - from.1);
        let len = dx.hypot(dy).max(1.0);
        let mut d = 4.0;
        while d < len - rad {
            if (d as i32) % 4 == 0 {
                pen.light((from.0 + dx / len * d).round() as i32, (from.1 + dy / len * d).round() as i32, r.step(4));
            }
            d += 1.0;
        }
    }
    for b in run.heroes.beams.iter().filter(|b| !b.fire) {
        let mine = run.feel.ults.iter().any(|c| {
            c.ult == Ult::LastLight && (HZ as f32 - 2.0..HZ as f32 + 16.0).contains(&age_of(run, c)) && (b.from.0 - c.x).hypot(b.from.1 - c.y) < 4.0
        });
        if !mine {
            continue;
        }
        let k = 1.0 - b.left as f32 / 12.0;
        let a = (b.from.0 * UNIT, b.from.1 * UNIT - 9.0);
        let z = (b.to.0 * UNIT, b.to.1 * UNIT - 9.0);
        beam(pen, a, z, r, 2.6 * (1.0 - k) + 0.6);
        pillar(pen, (z.0, z.1 + 9.0), 5.0 * (1.0 - k) + 1.0, 90.0, r, k, run.tick);
        pfx::rays(pen, z, 10, 20.0, 1.0, 0.2, r, k);
        pen.shock((z.0, z.1 + 10.0), 4.0 + ease::out(k) * 22.0, 1.6, 0.5, r, ease::inward(k));
    }
}

/// The Needle Squall: a gale wound about the Galeweaver while her focus
/// lasts, leaves going round in it, and a turning rune of wind on her mark.
fn needle_squall(pen: &mut Pen, run: &Run) {
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.hero_kit.focus > 0) {
        let (_, mid) = wishfx::body(run, hero);
        let t = run.tick as f32;
        let age = (6 * HZ).saturating_sub(hero.hero_kit.focus) as f32;
        let thin = (hero.hero_kit.focus as f32 / 20.0).min(1.0);
        for arm in 0..3 {
            gust(pen, (mid.0, mid.1 + 5.0 - arm as f32 * 6.0), (11.0 + arm as f32 * 2.0, 3.5 + arm as f32), t * 0.35 + arm as f32 * 2.1, 2.6, GALE, arm == 1, 1.0 - thin);
        }
        for i in 0..4u32 {
            let a = t * 0.3 + i as f32 * TAU / 4.0;
            let at = (mid.0 + a.cos() * 14.0, mid.1 - 2.0 + a.sin() * 5.0 - (t * 0.2 + i as f32).sin() * 2.0);
            let m = pfx::Mote { i, at, vel: (-a.sin(), a.cos()), k: 0.2, seed: i ^ id };
            pfx::mote(pen, &m, Shape::Petal, ramps::NATURE, 2, run.tick);
        }
        if age < 12.0 {
            pfx::rays(pen, mid, 8, 18.0, 0.8, 0.3, GALE, age / 12.0);
        }
        // Her shots go as needles of the gale.
        for p in run.projectiles.iter().filter(|p| !p.hostile && u32::from(p.traits.owner) == id) {
            pen.streak((p.x * UNIT, p.y * UNIT - 6.0), (p.vx, p.vy), 10.0, GALE, 0.05);
        }
        if let Some(e) = hero.hero_kit.target.and_then(|t| run.enemies.iter().find(|e| e.id == t && e.hp > 0)) {
            let (_, emid, h) = foe_mid(run, e);
            pen.glyph(emid, h * 0.5 + 7.0, 1.0, t * 0.12, 1.0, 3, GALE, 1.0 - thin * 0.8, e.id);
        }
    }
}

/// Whether a fire beam is the Furnace Verdict's.
fn verdict(run: &Run, b: &crate::rules::heroes::Beam) -> bool {
    run.feel.ults.iter().any(|c| c.ult == Ult::FurnaceVerdict && age_of(run, c) < 22.0 && (b.from.0 - c.x).hypot(b.from.1 - c.y) < 4.0)
}

/// The Furnace Verdict's floor: the verdict's rune of fire burnt under its
/// mark.
fn verdict_floor(pen: &mut Pen, run: &Run) {
    for b in run.heroes.beams.iter().filter(|b| b.fire && verdict(run, b)) {
        let k = 1.0 - b.left as f32 / 18.0;
        let at = (b.to.0 * UNIT, b.to.1 * UNIT + 1.0);
        let open = ease::back((k * 3.0).min(1.0));
        pen.glyph(at, 18.0 * open.max(0.3), 0.45, k * 1.5, open.min(1.0), 5, ramps::FIRE, ease::inward(k), 0xf0);
    }
}

/// The Furnace Verdict: a beam of fire out of the staff, a pillar of the
/// furnace slammed down onto its mark out of the dark, shocks off the
/// floor, embers thrown up and smoke after.
fn furnace(pen: &mut Pen, run: &Run) {
    let r = ramps::FIRE;
    for b in run.heroes.beams.iter().filter(|b| b.fire && verdict(run, b)) {
        let k = 1.0 - b.left as f32 / 18.0;
        let age = 18.0 - b.left as f32;
        let a = (b.from.0 * UNIT, b.from.1 * UNIT - 9.0);
        let z = (b.to.0 * UNIT, b.to.1 * UNIT);
        if k < 0.6 {
            beam(pen, a, (z.0, z.1 - 9.0), r, 3.0 * (1.0 - k));
        }
        pillar(pen, z, 7.0 * (1.0 - k * 0.7) + 1.0, 100.0, r, k * 0.85, run.tick);
        for w in 0..2 {
            let t = (age - w as f32 * 5.0) / 14.0;
            if (0.0..1.0).contains(&t) {
                pen.shock((z.0, z.1 + 1.0), 5.0 + ease::out(t) * 30.0, 2.0, 0.45, r, ease::inward(t));
            }
        }
        let embers = Spray { gravity: -0.05, angle: -FRAC_PI_2, spread: 1.6, stagger: 0.5, ..Spray::round(hash(z.0 as i32, 5, 0xfe), if pen.calm { 8 } else { 18 }, (z.0, z.1 - 4.0), (1.0, 3.0), (12.0, 20.0)) };
        embers.each(age, |m| pfx::mote(pen, &m, Shape::Ember, r, 1, run.tick));
        for i in 0..3u32 {
            pen.wisp((z.0 - 6.0 + i as f32 * 6.0, z.1 - 2.0), age - 6.0, (0.0, 8.0), 20.0, 20.0, 3.0, ramps::SMOKE, i ^ 0xf00);
        }
    }
}

/// A beam from `a` to `z`: a white core and `ramp` flanks `wide` across.
fn beam(pen: &mut Pen, a: (f32, f32), z: (f32, f32), ramp: Ramp, wide: f32) {
    let (dx, dy) = (z.0 - a.0, z.1 - a.1);
    let len = dx.hypot(dy).max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    let n = len.ceil() as i32;
    let w = wide.ceil() as i32;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let (x, y) = (a.0 + dx * t, a.1 + dy * t);
        pen.light(x.round() as i32, y.round() as i32, 'w');
        for s in 1..=w {
            let f = s as f32 / wide.max(0.5);
            if f > 1.0 && bayer(x as i32 + s, y as i32) > 0.5 {
                continue;
            }
            for side in [-1.0f32, 1.0] {
                pen.light((x + nx * s as f32 * side).round() as i32, (y + ny * s as f32 * side).round() as i32, ramp.at(f * 0.6));
            }
        }
    }
    pen.glow(a, 5.0 + wide, 1.0, ramp, 0.8);
    pen.glow(z, 8.0 + wide * 2.0, 1.0, ramp, 0.9);
}

/// A column of `ramp` out of the dark `tall` pixels onto `feet`, `half`
/// wide each side of its line, dissolving as `k` goes to 1; it flickers.
fn pillar(pen: &mut Pen, feet: (f32, f32), half: f32, tall: f32, ramp: Ramp, k: f32, tick: u64) {
    if k >= 1.0 {
        return;
    }
    let top = feet.1 - tall;
    let mut y = top;
    while y <= feet.1 {
        let down = (y - top) / tall;
        let w = half * (0.6 + 0.4 * down) + (y * 0.3 + tick as f32 * 0.9).sin() * 0.6;
        for dx in -(w.ceil() as i32)..=(w.ceil() as i32) {
            let x = feet.0.round() as i32 + dx;
            let yy = y.round() as i32;
            let edge = dx.abs() as f32 / w.max(0.5);
            if edge > 1.0 || bayer(x, yy + tick as i32) > (1.0 - edge * 0.6) * (1.0 - k) * (0.75 + down * 0.5) {
                continue;
            }
            pen.light(x, yy, ramp.at(edge * 0.6 + k * 0.3 + (1.0 - down) * 0.15));
        }
        y += 1.0;
    }
    pen.glow((feet.0, feet.1 - 4.0), half * 2.5 + 4.0, 0.7, ramp, 1.0 - k);
}

// ─── ready ───────────────────────────────────────────────────────────────────

/// A ready ultimate: two glints going round the knight's feet in its own
/// colours, and now and then a star over them.
fn ready(pen: &mut Pen, run: &Run, hero: &Hero) {
    let (feet, _) = wishfx::body(run, hero);
    let r = ramp_of(hero.ult());
    let t = run.tick as f32;
    for k in 0..2 {
        let a = t * 0.2 + k as f32 * PI;
        let (x, y) = ((feet.0 + a.cos() * 8.0).round() as i32, (feet.1 + 1.0 + a.sin() * 2.5).round() as i32);
        pen.light(x, y, r.step(0));
        pen.light(x - 1, y, r.step(2));
        pen.light(x + 1, y, r.step(2));
        let b = a - 0.35;
        pen.light((feet.0 + b.cos() * 8.0).round() as i32, (feet.1 + 1.0 + b.sin() * 2.5).round() as i32, r.step(3));
    }
    let beat = run.tick % 40;
    if beat < 10 {
        pen.twinkle((feet.0 + (unit((run.tick / 40) as i32, 3, 0x2d) - 0.5) * 12.0, feet.1 - 24.0), 2.0, beat as f32 / 10.0, r);
    }
}

/// A monster held still, iced over: its figure in the stone blues.
pub(super) fn iced(im: &Img) -> Img {
    const RAMP: [char; 6] = ['s', 'x', 'u', 'U', 'v', 'W'];
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(x, y, RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)]);
            }
        }
    }
    out
}
