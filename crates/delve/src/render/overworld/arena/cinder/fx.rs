//! Cinder Stack's effects, all pure functions of an age in ticks on the realm's inks: shards of the
//! pots that popped, a ring and rays for a chain, dust where a pair lands, a drop's streaks, slag
//! that breaks on the stack, the kiln cracking apart, a spark that carries a pop's cracks to the
//! meter, and the hearth's embers. Nothing is kept between frames; the pen's budget caps what any
//! one effect may paint.
use super::super::super::ink::{Img, bayer, hash};
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::hud::{self, cell};
use super::{CH, CW};
use crate::motion::{self, ease};
use crate::rules::cinder_show::Cells;
use std::f32::consts::{FRAC_PI_2, PI};

/// The kiln spirit's middle, where its cracks break out.
pub(super) const KILN: (f32, f32) = (311.0, 112.0);
/// Where the pots that popped are fed to him.
pub(super) const EAT: (f32, f32) = (288.0, 98.0);
/// Where the meter takes a spark in.
pub(super) const METER_AT: (f32, f32) = (311.0, 62.0);

fn ramp_of(colour: usize) -> Ramp {
    match colour {
        0 => ramps::WATER,
        1 => ramps::EARTH,
        2 => ramps::NATURE,
        _ => ramps::EMBER,
    }
}

/// The middle of the cells popped.
pub(super) fn centroid(by: &[Cells; 4]) -> (f32, f32) {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for cells in by {
        for (x, y) in cells.each() {
            let (px, py) = cell(x, y);
            sx += px as f32 + CW as f32 / 2.0;
            sy += py as f32 + CH as f32 / 2.0;
            n += 1.0;
        }
    }
    if n == 0.0 { (0.0, 0.0) } else { (sx / n, sy / n) }
}

/// Pots popping: a warm flash where each stood, shards of each in its own colour thrown up and out,
/// chips that fall, hot sparks, a ring that grows with the chain and rays when it is one. `age` ticks
/// since they popped.
pub(super) fn pop(world: &mut Img, by: &[Cells; 4], chain: u8, gain: u8, age: u32, seed: u32, tick: u64) {
    let mut pen = Pen::new(world, 1200);
    let c = centroid(by);
    let mut shown = 0;
    let big = chain >= 2 || gain >= 6;
    for (colour, cells) in by.iter().enumerate() {
        for (x, y) in cells.each() {
            // Twelve cells' worth of shards is plenty; a big pop throws harder instead.
            if shown >= 12 {
                break;
            }
            shown += 1;
            let (px, py) = cell(x, y);
            let at = (px as f32 + CW as f32 / 2.0, py as f32 + CH as f32 / 2.0);
            let sd = seed ^ ((y * 6 + x) as u32).wrapping_mul(0x9e37);
            let push = 1.0 + f32::from(chain.min(4) - 1) * 0.3;
            if age < 34 {
                let shards = Spray {
                    gravity: 0.17,
                    drag: 0.05,
                    angle: -FRAC_PI_2,
                    spread: 3.0,
                    ..Spray::round(sd, 5, at, (1.6 * push, 4.2 * push), (16.0, 30.0))
                };
                shards.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramp_of(colour), if big { 3 } else { 2 }, tick));
                let chips = Spray {
                    gravity: 0.22,
                    drag: 0.04,
                    angle: -FRAC_PI_2,
                    spread: 2.2,
                    ..Spray::round(sd ^ 0xa5, 3, at, (1.0 * push, 2.8 * push), (14.0, 26.0))
                };
                chips.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramp_of(colour), 2, tick));
            }
            if age < 16 {
                let sparks = Spray { gravity: 0.08, ..Spray::round(sd ^ 0x5a, 3, at, (2.2, 5.0), (7.0, 15.0)) };
                sparks.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 2, tick));
                let stars = Spray::round(sd ^ 0x33, 1, at, (0.4, 1.4), (10.0, 18.0));
                stars.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
            }
            // A dithered flash of warm light around each cell for the first ticks.
            if age < 4 {
                let k = 0.62 * (1.0 - age as f32 / 4.0);
                for yy in py - 3..py + CH + 3 {
                    for xx in px - 3..px + CW + 3 {
                        if bayer(xx, yy) < k && pen.has(xx, yy) {
                            pen.cv.put(xx, yy, 'T');
                        }
                    }
                }
            }
        }
    }
    let power = 1.0 + f32::from(chain - 1) * 0.45 + f32::from(gain) / 12.0;
    let life = 13.0 + f32::from(chain) * 3.0;
    pen.shock(c, 4.0 + age as f32 * 3.6 * power.min(3.2), 2.8, 0.85, if chain >= 2 { ramps::GOLD } else { ramps::EMBER }, age as f32 / life);
    if chain >= 3 && age >= 3 {
        pen.shock(c, 2.0 + (age - 3) as f32 * 2.6 * power.min(3.2), 2.0, 0.85, ramps::BONE, (age - 3) as f32 / life);
    }
    if chain >= 2 && age < 10 {
        pfx::rays(&mut pen, c, 6 + u32::from(chain) * 2, 18.0 + f32::from(chain) * 8.0, 0.9, seed as f32, ramps::GOLD, age as f32 / 10.0);
    }
}

/// A pair coming to rest: dust off each pot's feet, and for a dropped pair the streaks it fell in.
pub(super) fn land(world: &mut Img, a: (u8, u8), b: (u8, u8), hard: bool, from: u8, age: u32, seed: u32, tick: u64) {
    let mut pen = Pen::new(world, 260);
    for (n, (x, y)) in [a, b].into_iter().enumerate() {
        let (px, py) = cell(usize::from(x), usize::from(y));
        let feet = (px as f32 + CW as f32 / 2.0, py as f32 + CH as f32 - 2.0);
        for (side, angle) in [0.0, PI].into_iter().enumerate() {
            let dust = Spray {
                angle,
                spread: 0.5,
                gravity: -0.01,
                drag: 0.16,
                squash: 0.35,
                ..Spray::round(seed + (n * 2 + side) as u32, if hard { 4 } else { 2 }, feet, (0.9, 1.9), (8.0, 15.0))
            };
            dust.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::SMOKE, 2, tick));
        }
    }
    if hard && age < 6 {
        let fade = 1.0 - age as f32 / 6.0;
        for (x, y) in [a, b] {
            let (px, py) = cell(usize::from(x), usize::from(y));
            let top = cell(usize::from(x), usize::from(from)).1 + CH / 2;
            let mut yy = top;
            while yy < py {
                for dx in [5, CW - 6] {
                    if bayer(px + dx, yy) < fade * 0.9 {
                        pen.put(px + dx, yy, if (yy / 4) % 2 == 0 { 'i' } else { 'h' });
                    }
                }
                yy += 1;
            }
        }
    }
}

/// Slag breaking on the stack: a ring along the floor, chips thrown up in ember, a puff of smoke.
pub(super) fn slag(world: &mut Img, col: u8, row: u8, hit: bool, age: u32, seed: u32, tick: u64) {
    let mut pen = Pen::new(world, 320);
    let (px, py) = cell(usize::from(col), usize::from(row));
    let at = (px as f32 + CW as f32 / 2.0, py as f32 + CH as f32 / 2.0);
    pen.shock((at.0, at.1 + 6.0), 3.0 + age as f32 * 3.4, 2.2, 0.4, if hit { ramps::FIRE } else { ramps::EMBER }, age as f32 / 14.0);
    let chips = Spray {
        gravity: 0.15,
        drag: 0.06,
        angle: -FRAC_PI_2,
        spread: 2.4,
        ..Spray::round(seed, if hit { 18 } else { 12 }, at, (1.4, 4.0), (12.0, 26.0))
    };
    chips.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::EMBER, 3, tick));
    let sparks = Spray { gravity: 0.1, ..Spray::round(seed ^ 0x19, 6, at, (2.0, 4.6), (6.0, 12.0)) };
    sparks.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, tick));
    let smoke = Spray { gravity: -0.05, drag: 0.08, ..Spray::round(seed ^ 0x77, 4, (at.0, at.1 - 4.0), (0.3, 0.9), (14.0, 26.0)) };
    smoke.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::SMOKE, 2, tick));
}

/// The kiln cracking: rings out of his body, rays, and a spray of crock from his sides. The last
/// stage is the biggest by far.
pub(super) fn stage(world: &mut Img, n: u8, age: u32, seed: u32, tick: u64) {
    let last = n >= 3;
    let mut pen = Pen::new(world, 900);
    let a = age as f32;
    pen.shock(KILN, 5.0 + a * if last { 6.5 } else { 4.4 }, 2.6, 0.9, ramps::EMBER, a / if last { 30.0 } else { 22.0 });
    if age >= 4 {
        pen.shock(KILN, 3.0 + (a - 4.0) * if last { 5.0 } else { 3.4 }, 2.0, 0.9, ramps::BONE, (a - 4.0) / 20.0);
    }
    if age < 12 {
        pfx::rays(&mut pen, KILN, if last { 18 } else { 12 }, if last { 70.0 } else { 44.0 }, 1.0, seed as f32, ramps::GOLD, a / 12.0);
    }
    let crock = Spray {
        gravity: 0.13,
        drag: 0.04,
        angle: -FRAC_PI_2,
        spread: 3.4,
        ..Spray::round(seed, if last { 40 } else { 22 }, KILN, (2.0, if last { 6.4 } else { 4.8 }), (18.0, 36.0))
    };
    crock.each(a, |m| {
        let ramp = if m.i % 3 == 0 { ramps::WATER } else { ramps::EARTH };
        pfx::mote(&mut pen, &m, Shape::Shard, ramp, if last { 3 } else { 2 }, tick);
    });
    let sparks = Spray { gravity: 0.06, ..Spray::round(seed ^ 0xc3, if last { 24 } else { 12 }, KILN, (2.4, 6.0), (10.0, 22.0)) };
    sparks.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 2, tick));
}

/// Pots reaching the kiln's mouth: sparks and a small ring where they are eaten.
pub(super) fn eat(world: &mut Img, gain: u8, age: u32, seed: u32, tick: u64) {
    let mut pen = Pen::new(world, 260);
    let sparks = Spray { gravity: 0.1, ..Spray::round(seed, 6 + u32::from(gain.min(24)) / 2, EAT, (1.4, 4.0), (8.0, 16.0)) };
    sparks.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, tick));
    pen.shock(EAT, 2.0 + age as f32 * 3.0, 1.8, 0.8, ramps::EMBER, age as f32 / 10.0);
}

/// The spark that carries a pop's cracks from the board to the meter: it arcs up, then dives, with
/// a tail of three. `age` runs from the pop; it flies from tick 3 to `hud::FLY`.
pub(super) fn carry(world: &mut Img, from: (f32, f32), gain: u8, age: u32) {
    const START: u32 = 3;
    if age < START || age >= hud::FLY {
        return;
    }
    let t = (age - START) as f32 / (hud::FLY - START) as f32;
    let size = if gain >= 12 { 3 } else { 2 };
    for (k, ink) in ['H', 'T', 'o', 'R'].into_iter().enumerate() {
        let (x, y) = motion::arc(from, METER_AT, 34.0, ease::both((t - k as f32 * 0.07).max(0.0)));
        let s = if k == 0 { size } else { (size - 1).max(1) };
        world.rect(x as i32 - s / 2, y as i32 - s / 2, s, s, ink);
    }
}

/// Steam off the kiln's lid: three puffs a beat apart, rising and thinning.
pub(super) fn steam(world: &mut Img, tick: u64) {
    for i in 0..3 {
        let period = 58;
        let t = (tick as i32 + i * 19).rem_euclid(period);
        let k = t as f32 / period as f32;
        let (x, y) = (303 + ((k * 6.0 + i as f32 * 2.0).sin() * 3.0) as i32 + i * 2, 70 - (k * 20.0) as i32);
        let ink = ramps::SMOKE.at(k * 0.9);
        let r = 1 + (k * 2.0) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r + 1 && bayer(x + dx, y + dy) > k * 0.8 {
                    world.put(x + dx, y + dy, ink);
                }
            }
        }
    }
}

/// The hearth's embers rising behind the kiln.
pub(super) fn embers(world: &mut Img, tick: u64) {
    for i in 0..14 {
        let h = hash(i, 7, 0xe3);
        let period = 54 + (h % 40) as i32;
        let t = (tick as i32 + (h >> 8) as i32 % period).rem_euclid(period);
        let k = t as f32 / period as f32;
        let x = 262 + ((h >> 16) % 96) as i32 + ((t as f32 * 0.16 + i as f32).sin() * 3.0) as i32;
        let y = 160 - (k * 78.0) as i32;
        world.put(x, y, ramps::EMBER.at(k));
        if k < 0.3 {
            world.put(x, y + 1, ramps::EMBER.at(k + 0.3));
        }
    }
}
