//! What the knights kick up and set off: dust under a running foot, a puff and a ring on a landing,
//! sparks off a skid, a stomp's burst and flying score, coins spun out of a rune block and flown
//! to the purse, a slam's ring and rubble. Every one is a pure function of a mark's age (the sim's
//! `show`), drawn with the realm's pixel kit: nothing is kept between frames, so the host and a
//! friend's mirror draw the same pixels.
//!
//! Two layers: `back` goes behind the knights (dust at the feet), `front` goes over them (sparks,
//! numbers, coins).
use super::super::super::ink::Img;
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::{H, W};
use crate::rules::bannerpeak::{
    session::Session,
    show::{Kind, Mark},
};
use crate::rules::juice::Weight;
use std::f32::consts::PI;

/// Where the purse sits on the HUD, for a coin to fly to.
pub(super) const PURSE: (i32, i32) = (9, 9);

fn smoke(pen: &mut Pen, at: (f32, f32), age: f32, seed: u32, n: u32, angle: f32, spread: f32, speed: (f32, f32), size: u8, tick: u64) {
    let spray = Spray {
        angle,
        spread,
        gravity: -0.02,
        drag: 0.16,
        squash: 0.8,
        ..Spray::round(seed, n, at, speed, (9.0, 17.0))
    };
    spray.each(age, |m| pfx::mote(pen, &m, Shape::Smoke, ramps::BONE, size, tick));
}

/// One mark, drawn behind (`back`) or over (`front`) the knights.
fn one(back: &mut Img, front: &mut Img, m: &Mark, s: &Session, id: u32, cam: (i32, i32), tick: u64) {
    let now = s.elapsed;
    let age = m.age(now);
    let a = age as f32;
    let (x, y) = (m.x - cam.0, m.y - cam.1);
    if !(-60..W + 60).contains(&x) || !(-90..H + 60).contains(&y) {
        return;
    }
    let seed = (m.x.wrapping_mul(31) ^ m.y.wrapping_mul(17) ^ m.born as i32) as u32;
    let at = (x as f32, y as f32 - 1.0);
    match m.kind {
        Kind::Step => {
            let dir = m.n.signum() as i32;
            let mut pen = Pen::new(back, 60);
            let angle = if dir > 0 { -2.5 } else { -0.64 };
            smoke(&mut pen, (at.0 - dir as f32 * 3.0, at.1), a, seed, 3, angle, 0.7, (0.3, 0.8), 2, tick);
        }
        Kind::Hop => {
            let mut pen = Pen::new(back, 80);
            smoke(&mut pen, at, a, seed, 4, -PI / 2.0, 2.6, (0.4, 1.1), 2, tick);
            // A triple jump leaves a ring of sparks behind its feet.
            if m.n == 2 && age < 10 {
                let mut p = Pen::new(front, 80);
                pfx::rays(&mut p, (at.0, at.1 - 2.0), 7, 11.0, 0.7, seed as f32, ramps::GOLD, a / 10.0);
            }
        }
        Kind::Land => {
            let hard = (f32::from(m.n) / 12.0).clamp(0.25, 1.0);
            let mut pen = Pen::new(back, 160);
            let n = 4 + (hard * 5.0) as u32;
            smoke(&mut pen, (at.0 - 3.0, at.1), a, seed, n, PI + 0.15, 0.8, (0.5, 0.9 + hard * 1.4), 2 + u8::from(hard > 0.7), tick);
            smoke(&mut pen, (at.0 + 3.0, at.1), a, seed ^ 0x77, n, -0.15, 0.8, (0.5, 0.9 + hard * 1.4), 2 + u8::from(hard > 0.7), tick);
            if hard > 0.55 && age < 14 {
                pen.shock((at.0, at.1), 2.0 + a * (1.2 + hard * 0.9), 1.2, 0.3, ramps::SMOKE, a / 14.0);
            }
        }
        Kind::Skid => {
            let dir = m.n as i32;
            let mut pen = Pen::new(front, 80);
            let angle = if dir > 0 { -PI + 0.5 } else { -0.5 };
            let sparks = Spray { angle, spread: 0.7, gravity: 0.05, ..Spray::round(seed, 5, (at.0, at.1 - 1.0), (1.0, 2.2), (4.0, 9.0)) };
            sparks.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Spark, ramps::EMBER, 1, tick));
            let mut b = Pen::new(back, 60);
            smoke(&mut b, (at.0 - dir as f32 * 4.0, at.1), a, seed, 3, -PI / 2.0, 2.0, (0.2, 0.7), 1, tick);
        }
        Kind::Kick => {
            let mut pen = Pen::new(back, 60);
            smoke(&mut pen, at, a, seed, 4, if m.n > 0 { 0.1 } else { PI - 0.1 }, 1.4, (0.3, 1.0), 1, tick);
            let mut p = Pen::new(front, 60);
            let sparks = Spray { spread: 1.0, gravity: 0.1, angle: if m.n > 0 { -0.6 } else { -PI + 0.6 }, ..Spray::round(seed, 4, at, (0.8, 1.6), (5.0, 9.0)) };
            sparks.each(a, |mote| pfx::mote(&mut p, &mote, Shape::Spark, ramps::BONE, 1, tick));
        }
        Kind::Stomp => {
            let weight = if m.n >= 800 { Weight::Crushing } else { Weight::Solid };
            juice::impact_burst(front, at, age, seed, weight, tick);
            let text = format!("+{}", m.n);
            juice::pop_number(front, &text, (x, y - 8), age, 28, 1, if m.n >= 400 { '6' } else { 'T' });
            // The wisp's light, returned: a warm mote that climbs away.
            let mut pen = Pen::new(front, 80);
            let lights = Spray { gravity: -0.06, drag: 0.05, ..Spray::round(seed ^ 0x9, 3, (at.0, at.1 - 3.0), (0.4, 1.0), (14.0, 26.0)) };
            lights.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Ember, ramps::GOLD, 1, tick));
            let cold = Spray { gravity: -0.03, drag: 0.1, ..Spray::round(seed ^ 0x13, 5, at, (0.3, 1.0), (10.0, 18.0)) };
            cold.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Smoke, ramps::BONE, 1, tick));
        }
        Kind::Coin => {
            coin(front, m, s, id, cam, x, y, age, seed, tick);
        }
        Kind::Block => {
            let mut pen = Pen::new(front, 80);
            let chips = Spray { spread: 2.2, angle: -PI / 2.0, gravity: 0.2, ..Spray::round(seed, 5, (at.0, at.1 + 4.0), (0.8, 1.9), (8.0, 14.0)) };
            let ink = if m.n == 2 { ramps::EARTH } else { ramps::BONE };
            chips.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Shard, ink, 1, tick));
            if age < 8 {
                pfx::rays(&mut pen, (at.0, at.1 + 2.0), 6, 10.0, 0.6, seed as f32, ramps::GOLD, a / 8.0);
            }
            if m.n == 2 {
                let mut b = Pen::new(back, 80);
                smoke(&mut b, (at.0, at.1 + 6.0), a, seed, 5, -PI / 2.0, 3.0, (0.4, 1.0), 2, tick);
            }
        }
        Kind::Spark => {
            let mut pen = Pen::new(front, 160);
            if age < 12 {
                pen.shock(at, 2.0 + a * 1.3, 1.0, 1.0, ramps::GOLD, a / 12.0);
            }
            let stars = Spray { gravity: 0.02, ..Spray::round(seed, 8, at, (0.7, 2.2), (14.0, 28.0)) };
            stars.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Star, ramps::GOLD, 1, tick));
            pfx::rays(&mut pen, at, 8, 14.0, 1.0, seed as f32, ramps::GOLD, (a / 9.0).min(1.0));
        }
        Kind::Gate => {
            let mut pen = Pen::new(front, 160);
            let embers = Spray { angle: -PI / 2.0, spread: 1.2, gravity: -0.03, ..Spray::round(seed, 12, at, (0.8, 2.2), (16.0, 30.0)) };
            embers.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Ember, ramps::EMBER, 1, tick));
            if age < 14 {
                pen.shock((at.0, at.1), 2.0 + a * 1.6, 1.2, 0.4, ramps::GOLD, a / 14.0);
            }
        }
        Kind::Slam => {
            let r = m.n.max(24) as f32;
            let weight = Weight::Crushing;
            juice::impact_burst(front, (at.0, at.1 - 3.0), age, seed, weight, tick);
            let mut pen = Pen::new(back, 360);
            if age < 16 {
                pen.shock((at.0, at.1), 3.0 + a * r / 11.0, 1.8, 0.28, ramps::SMOKE, a / 16.0);
                pen.shock((at.0, at.1), 1.0 + a * r / 15.0, 1.2, 0.3, ramps::BONE, a / 16.0);
            }
            smoke(&mut pen, (at.0 - 4.0, at.1), a, seed, 6, PI + 0.1, 0.6, (0.8, 2.6), 2, tick);
            smoke(&mut pen, (at.0 + 4.0, at.1), a, seed ^ 0x55, 6, -0.1, 0.6, (0.8, 2.6), 2, tick);
            let rubble = Spray { spread: 2.4, angle: -PI / 2.0, gravity: 0.22, ..Spray::round(seed ^ 0x4, 9, at, (1.4, 3.2), (12.0, 22.0)) };
            let mut p = Pen::new(front, 90);
            rubble.each(a, |mote| pfx::mote(&mut p, &mote, Shape::Shard, ramps::EARTH, 1, tick));
        }
        Kind::Spring => {
            let mut pen = Pen::new(front, 200);
            if age < 14 {
                pen.shock((at.0, at.1), 2.0 + a * 1.5, 1.4, 0.4, ramps::FROST, a / 14.0);
                pen.shock((at.0, at.1 - 6.0), 2.0 + a * 1.0, 1.0, 0.4, ramps::FROST, (a / 14.0).min(1.0));
            }
            // Arrows that rise off the shield: the lift.
            for i in 0..3i32 {
                let rise = a * 1.6 - i as f32 * 4.0;
                if (0.0..16.0).contains(&rise) {
                    let (px, py) = (x + (i - 1) * 5, y - 4 - rise as i32);
                    front.put(px, py, 'W');
                    front.put(px - 1, py + 1, 'z');
                    front.put(px + 1, py + 1, 'z');
                    front.put(px, py + 1, 'W');
                }
            }
        }
        Kind::Hurt => {
            let weight = Weight::Solid;
            juice::impact_burst(front, (at.0, at.1 - 6.0), age, seed, weight, tick);
            let mut pen = Pen::new(front, 80);
            let bits = Spray { gravity: 0.2, angle: if m.n > 0 { -0.7 } else { -PI + 0.7 }, spread: 1.2, ..Spray::round(seed ^ 0x6, 6, (at.0, at.1 - 6.0), (1.0, 2.4), (10.0, 18.0)) };
            bits.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Shard, ramps::BONE, 1, tick));
        }
        Kind::Pit => {
            let mut pen = Pen::new(front, 100);
            let curls = Spray { angle: -PI / 2.0, spread: 1.0, gravity: -0.05, ..Spray::round(seed, 6, at, (0.4, 1.2), (14.0, 26.0)) };
            curls.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Smoke, ramps::SMOKE, 2, tick));
        }
        Kind::Rescue => {
            let mut pen = Pen::new(front, 260);
            if age < 14 {
                pen.shock((at.0, at.1), 3.0 + a * 1.6, 1.4, 1.0, ramps::FROST, a / 14.0);
            }
            let shards = Spray { gravity: 0.04, ..Spray::round(seed, 14, at, (1.0, 2.8), (16.0, 30.0)) };
            shards.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Bubble, ramps::FROST, 1, tick));
            let hearts = Spray { gravity: -0.05, angle: -PI / 2.0, spread: 1.2, ..Spray::round(seed ^ 0x8, 4, at, (0.5, 1.3), (22.0, 36.0)) };
            hearts.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Heart, ramps::ROSE, 1, tick));
        }
        Kind::Power => {
            let mut pen = Pen::new(front, 240);
            for (i, r) in [10.0f32, 16.0].into_iter().enumerate() {
                let t = (a - i as f32 * 4.0) / 14.0;
                if (0.0..1.0).contains(&t) {
                    pen.shock(at, 2.0 + t * r, 1.4, 1.0, ramps::GOLD, t);
                }
            }
            let stars = Spray { gravity: -0.03, ..Spray::round(seed, 10, at, (0.6, 1.8), (16.0, 28.0)) };
            stars.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Star, ramps::GOLD, 1, tick));
        }
        Kind::Down => {
            juice::death_burst(front, (at.0, at.1), age, seed, tick);
        }
        Kind::Cap => {
            juice::impact_burst(front, at, age, seed, Weight::Graze, tick);
        }
        Kind::Plate => {
            // A plate off the crown: a heavy burst and a falling plate of gold.
            juice::impact_burst(front, at, age, seed, Weight::Kill, tick);
            let mut pen = Pen::new(front, 160);
            let plates = Spray { spread: 2.8, angle: -PI / 2.0, gravity: 0.2, ..Spray::round(seed ^ 0x3, 8, at, (1.5, 3.4), (18.0, 32.0)) };
            plates.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Shard, ramps::GOLD, 2, tick));
        }
        Kind::Crown => {
            juice::impact_burst(front, at, age, seed, Weight::Finale, tick);
            juice::death_burst(front, (at.0, at.1 + 12.0), age, seed ^ 0x21, tick);
            if age < 120 {
                // A column of embers lifting off the fallen king.
                let mut pen = Pen::new(front, 300);
                let col = Spray { spread: 1.0, angle: -PI / 2.0, gravity: -0.02, ..Spray::round(seed ^ 0x7, 24, (at.0, at.1 + 26.0), (0.8, 2.6), (30.0, 62.0)) };
                col.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Ember, ramps::EMBER, 1, tick));
            }
        }
        Kind::Banner => {}
    }
}

/// A coin out of a rune block (or off the ground): it leaps, spins, hangs a moment glinting and
/// then flies to the purse if it is yours; a friend's drops away in a sparkle.
#[allow(clippy::too_many_arguments)]
fn coin(front: &mut Img, m: &Mark, _s: &Session, id: u32, _cam: (i32, i32), x: i32, y: i32, age: u32, seed: u32, tick: u64) {
    let a = age as f32;
    let mine = u32::from(m.who) == id;
    if m.n == 1 {
        // Gathered: a ping of light where it was.
        let mut pen = Pen::new(front, 60);
        if age < 8 {
            pfx::rays(&mut pen, (x as f32, y as f32), 6, 9.0, 1.0, seed as f32, ramps::GOLD, a / 8.0);
        }
        let stars = Spray { gravity: -0.02, ..Spray::round(seed, 4, (x as f32, y as f32), (0.4, 1.4), (8.0, 15.0)) };
        stars.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Star, ramps::GOLD, 1, tick));
        return;
    }
    // Phases: leap (0..9), hang (9..16), fly (16..32).
    let (cx, cy) = if age < 9 {
        let t = a / 9.0;
        (x as f32, y as f32 - 4.0 - 24.0 * (1.0 - (1.0 - t) * (1.0 - t)))
    } else if age < 16 || !mine {
        (x as f32, y as f32 - 28.0 + (a - 9.0).min(7.0) * if mine { 0.0 } else { 0.8 })
    } else {
        let t = ((a - 16.0) / 16.0).clamp(0.0, 1.0);
        let (px, py) = (x as f32, y as f32 - 28.0);
        let e = t * t * (3.0 - 2.0 * t);
        (px + (PURSE.0 as f32 - px) * e, py + (PURSE.1 as f32 - py) * e - 20.0 * (e * (1.0 - e)) * 2.0)
    };
    if !mine && age > 20 {
        return;
    }
    let (ix, iy) = (cx.round() as i32, cy.round() as i32);
    let face = (tick as u32 / 2 + seed) % 4;
    let w = [5, 4, 1, 4][face as usize];
    front.rect(ix - w / 2 - 1, iy - 5, w + 2, 10, 'k');
    front.rect(ix - w / 2, iy - 4, w, 8, '5');
    front.rect(ix - w / 2, iy - 4, w, 1, '6');
    if w > 2 {
        front.rect(ix, iy - 2, 1, 4, '4');
    }
    // A glint on the hang.
    if (9..16).contains(&age) {
        let g = (age - 9) as i32;
        front.put(ix - 4 + g, iy - 4, 'w');
        front.put(ix - 4 + g - 1, iy - 4, 'T');
    }
    // The burst out of the block at the leap.
    if age < 10 {
        let mut pen = Pen::new(front, 80);
        let spray = Spray { gravity: 0.1, angle: -PI / 2.0, spread: 2.2, ..Spray::round(seed, 7, (x as f32, y as f32 - 2.0), (0.8, 2.4), (8.0, 16.0)) };
        spray.each(a, |mote| pfx::mote(&mut pen, &mote, Shape::Spark, ramps::GOLD, 1, tick));
    }
}

/// The marks of the session, behind and over the knights.
pub(super) fn layers(s: &Session, id: u32, cam: (i32, i32), phase: u64) -> Option<(Img, Img)> {
    let live = |m: &&Mark| m.kind != Kind::Banner && (-60..W + 60).contains(&(m.x - cam.0)) && (-90..H + 60).contains(&(m.y - cam.1));
    if !s.show.marks.iter().any(|m| live(&m)) {
        return None;
    }
    let mut back = Img::new(W, H);
    let mut front = Img::new(W, H);
    for m in s.show.marks.iter().filter(live) {
        one(&mut back, &mut front, m, s, id, cam, phase);
    }
    Some((back, front))
}
