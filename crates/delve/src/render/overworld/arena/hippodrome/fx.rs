//! What a race throws up: the dust that trails every team and thickens under the whip, sparks off
//! a hard wheel, the crack of the whip, the burst of a barge and the wreck of a team, wind streaming
//! past the lens. Each is a pure function of an age and a seed, drawn in the realm's inks.
use super::super::super::ink::{Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use crate::rules::chariot::{Track, pos_on};
use crate::rules::juice::Weight;

/// Wood and iron thrown off a broken car.
const TIMBER: Ramp = Ramp(['O', 'o', 'R', 'P', 'B', 'b']);
/// Dry dust, lit from above.
const DUST: Ramp = Ramp(['T', 't', 'O', 'o', 'P', 'r']);

fn to_px(course: &Track, d: f32, lane: f32, cam: (i32, i32)) -> (f32, f32) {
    let (x, y) = pos_on(course, d, lane);
    (x * super::super::TILE as f32 - cam.0 as f32, y * super::super::TILE as f32 - cam.1 as f32)
}

/// The dust a team drags: puffs where it was a few strides ago, each risen, widened and thinned
/// by the ordered dither, so the cloud bends with the road. `pace` is the speed in paces a tick,
/// `lash` 0 to 1 how hard the whip is on.
pub(super) fn plume(layer: &mut Img, course: &Track, cam: (i32, i32), dist: f32, lane: f32, pace: f32, lash: f32, seed: u32, now: u32) {
    if pace < 1.5 {
        return;
    }
    let n = ((pace * 1.4) as i32 + (lash * 4.0) as i32).clamp(4, 17);
    let (w, h) = (layer.w, layer.h);
    for k in (1..=n).rev() {
        let back = k as f32 * 4.2;
        let jitter = (hash(seed as i32, k, now / 2) % 100) as f32 / 100.0 - 0.5;
        let (x, y) = to_px(course, dist - back, lane + jitter * 0.12, cam);
        let rise = k as f32 * (0.8 + lash * 0.4);
        let (x, y) = (x, y - 3.0 - rise);
        if x < -12.0 || y < -12.0 || x > w as f32 + 12.0 || y > h as f32 + 12.0 {
            continue;
        }
        let r = 2.0 + k as f32 * 0.55 + lash * k as f32 * 0.2;
        let thin = 1.0 - k as f32 / (n as f32 + 1.5);
        let step = ((k as f32 / n as f32) * 3.0) as usize + 1;
        for dy in -(r as i32)..=(r as i32) {
            for dx in -(r as i32)..=(r as i32) {
                let d2 = (dx * dx + dy * dy) as f32;
                if d2 > r * r {
                    continue;
                }
                let (px, py) = (x as i32 + dx, y as i32 + dy);
                // Lit from above and left: the top edge of a puff is the light one.
                let lit = usize::from(d2 > r * r * 0.4 && dy + dx > 1);
                if bayer(px + k, py) < thin * 0.92 {
                    layer.put(px, py, DUST.step((step + 1 + lit).min(5).saturating_sub(1)));
                }
            }
        }
    }
}

/// Sparks thrown back off the wheel: `power` of them a tick for the last few ticks, hot to dark.
pub(super) fn sparks(layer: &mut Img, at: (f32, f32), back: (f32, f32), power: u32, seed: u32, now: u32) {
    let mut pen = Pen::new(layer, 160);
    let angle = back.1.atan2(back.0);
    for age in 0..7u32 {
        let born = now.saturating_sub(age);
        let h = hash(seed as i32, born as i32, 0x5a);
        if h % 3 == 0 {
            continue;
        }
        let spray = Spray {
            seed: h,
            n: power,
            at,
            angle: angle - 0.35,
            spread: 1.1,
            speed: (2.0, 4.6),
            life: (5.0, 9.0),
            drag: 0.12,
            gravity: 0.34,
            squash: 0.6,
            stagger: 0.0,
        };
        spray.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::EMBER, 1, u64::from(now)));
    }
}

/// The crack of a whip `age` ticks old, from the driver at `from` to the horses' heads at `to`: a
/// jagged line of light that snaps out and is gone in four ticks, a ring where it bit, and a few
/// motes of dust shaken loose off the team's back.
pub(super) fn crack(layer: &mut Img, from: (f32, f32), to: (f32, f32), age: u32, seed: u32, now: u32) {
    if age > 12 {
        return;
    }
    let mut pen = Pen::new(layer, 260);
    let k = age as f32 / 12.0;
    if age < 5 {
        pen.arc(from, to, seed, 5.0, ramps::BONE, age as f32 / 5.0);
        pfx::rays(&mut pen, to, 6, 9.0, 0.8, seed as f32 * 0.3, ramps::GOLD, age as f32 / 5.0);
    }
    pen.shock((to.0, to.1 + 2.0), 2.0 + age as f32 * 1.7, 1.2, 0.6, ramps::GOLD, k);
    let shaken = Spray { gravity: 0.1, ..Spray::round(seed ^ 0x77, 5, to, (0.6, 1.8), (5.0, 11.0)) };
    shaken.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Dot, DUST, 1, u64::from(now)));
}

/// A barge: the spark and shock of the contact, shards of wood, and the dust it kicks up.
pub(super) fn barge(layer: &mut Img, at: (f32, f32), age: u32, seed: u32, weight: Weight, now: u32) {
    juice::impact_burst(layer, at, age, seed, weight, u64::from(now));
    let mut pen = Pen::new(layer, 300);
    let chips = Spray { gravity: 0.2, ..Spray::round(seed ^ 0x3d, 4 + weight as u32 * 3, (at.0, at.1 - 3.0), (1.2, 3.2), (10.0, 20.0)) };
    chips.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, TIMBER, 1, u64::from(now)));
    if age < 14 {
        for k in 0..3 + weight as u32 {
            pen.wisp((at.0 + k as f32 * 3.0 - 6.0, at.1 + 2.0), age as f32, (0.0, 0.0), 14.0, 0.5, 0.5, ramps::SMOKE, seed ^ k);
        }
    }
}

/// A wreck: a car in pieces. Timber and a wheel's spokes, iron, a cloud that boils up, a dark ring.
pub(super) fn wreck(layer: &mut Img, at: (f32, f32), age: u32, seed: u32, weight: Weight, now: u32) {
    juice::impact_burst(layer, at, age, seed, weight, u64::from(now));
    let mut pen = Pen::new(layer, 700);
    let a = age as f32;
    let planks = Spray { gravity: 0.24, ..Spray::round(seed ^ 0x91, 12, (at.0, at.1 - 4.0), (1.4, 4.0), (16.0, 30.0)) };
    planks.each(a, |m| pfx::mote(&mut pen, &m, Shape::Shard, TIMBER, 2, u64::from(now)));
    let iron = Spray { gravity: 0.26, ..Spray::round(seed ^ 0x92, 6, (at.0, at.1 - 4.0), (1.0, 3.2), (14.0, 24.0)) };
    iron.each(a, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::EMBER, 1, u64::from(now)));
    if age < 30 {
        let k = a / 30.0;
        for j in 0..5u32 {
            let ox = (j as f32 - 2.0) * 5.0;
            pen.wisp((at.0 + ox, at.1 + 3.0), a, (ox * 0.1, -0.3), 26.0, 0.9, 0.7, ramps::EARTH.cooler(1), seed ^ (j * 7));
        }
        pen.shock((at.0, at.1 + 5.0), 4.0 + a * 2.4, 1.6, 0.45, ramps::SMOKE, k);
    }
}

/// Wind: streaks in the direction the viewer is not going, longer and more of them the faster
/// they ride; `pace` is 0 to 1 of the top pace, `surge` extra at the off.
pub(super) fn wind(layer: &mut Img, dir: (f32, f32), pace: f32, surge: f32, now: u32) {
    let load = (pace - 0.35).max(0.0) / 0.65 + surge;
    if load <= 0.02 {
        return;
    }
    let (w, h) = (layer.w, layer.h);
    let n = (4.0 + load * 14.0) as u32;
    let mut pen = Pen::new(layer, 220);
    let span = (w + h) as f32;
    for i in 0..n {
        let u = (hash(i as i32, 3, 0x61) % 1000) as f32 / 1000.0;
        let v = (hash(i as i32, 5, 0x62) % 1000) as f32 / 1000.0;
        let speed = 18.0 + load * 14.0 + (hash(i as i32, 7, 0x63) % 9) as f32;
        let along = ((now as f32 * speed + u * span) % span) - span * 0.25;
        // Streaks run along `dir`, from far ahead to far behind, across a band.
        let (px, py) = (-dir.1, dir.0);
        let across = (v - 0.5) * span * 0.7;
        let x = w as f32 * 0.5 + dir.0 * (span * 0.4 - along) + px * across;
        let y = h as f32 * 0.5 + dir.1 * (span * 0.4 - along) + py * across;
        if x < -20.0 || y < -20.0 || x > w as f32 + 20.0 || y > h as f32 + 20.0 {
            continue;
        }
        pen.streak((x, y), dir, 5.0 + load * 16.0, ramps::SMOKE, 0.25 + (1.0 - load.min(1.0)) * 0.4);
    }
}
