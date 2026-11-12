//! The loud little moves of Pennon Rush: dust under a boot, a hook-bow's
//! flash, wood flying off a pennon, the ring of the captain's blow, a ward
//! shattering, a splash in the moat, a score flying to the strip. Each is a
//! pure function of the show's records and their age.

use super::super::super::ink::{Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::actors::Cx;
use super::hud::{self, VIEW_TOP, claim_big};
use crate::motion::ease;
use crate::rules::juice::Weight;
use crate::rules::pennon::show::{GO_LIFE, HOLD, Puff, Show};
use crate::rules::pennon::{FLOOR, Q};
use std::f32::consts::PI;

/// Ticks a score takes flying to the strip.
pub(super) const FLY: u32 = 22;
/// Ticks a number stands over a struck pennon.
const POP: u32 = 24;
/// The moat's surface, on the canvas.
const MOAT: i32 = VIEW_TOP + 176;

const DUST: Ramp = Ramp(['h', 'J', 'G', 'P', 'j', 'g']);
/// Smoke over a night wall: grey that never glares.
const ASH: Ramp = Ramp(['J', 'G', 'j', 'g', 'X', 'K']);

/// A flat fan of dust thrown from `at` along the walk.
fn dust(world: &mut Img, at: (f32, f32), age: u32, seed: u32, n: u32, speed: f32, tick: u64) {
    let mut pen = Pen::new(world, 240);
    let spray = Spray { gravity: -0.012, squash: 0.35, drag: 0.1, ..Spray::round(seed, n, at, (speed * 0.4, speed), (10.0, 20.0)) };
    spray.each(age as f32, |m| {
        let r = 1.0 + m.k * 2.4;
        let ri = r.ceil() as i32;
        let (x, y) = (m.at.0.round() as i32, m.at.1.round() as i32);
        let fill = 0.2 + 0.8 * (1.0 - m.k).powf(0.9);
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f32 <= r * r && bayer(x + dx, y + dy) < fill {
                    pen.put(x + dx, y + dy, DUST.at(0.1 + m.k * 0.8));
                }
            }
        }
    });
    let _ = tick;
}

/// Splinters, a seed-weight and a vane thrown off a felled pennon.
fn wood(world: &mut Img, at: (f32, f32), age: u32, seed: u32, big: bool, tick: u64) {
    let mut pen = Pen::new(world, 700);
    let a = age as f32;
    if age < 7 {
        pfx::rays(&mut pen, at, 9, if big { 34.0 } else { 20.0 }, 1.0, seed as f32 * 0.37, ramps::GOLD, a / 7.0);
    }
    let n = if big { 30 } else { 14 };
    let shards = Spray { gravity: 0.2, ..Spray::round(seed, n, at, (1.4, if big { 4.8 } else { 3.6 }), (14.0, 30.0)) };
    shards.each(a, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::EARTH, 2, tick));
    let embers = Spray { gravity: -0.03, ..Spray::round(seed ^ 0x5a, if big { 16 } else { 8 }, at, (0.6, 2.4), (16.0, 32.0)) };
    embers.each(a, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::EMBER, 1, tick));
    let smoke = Spray { gravity: -0.04, drag: 0.08, ..Spray::round(seed ^ 0xc3, if big { 8 } else { 4 }, (at.0, at.1 + 8.0), (0.3, 1.1), (18.0, 36.0)) };
    smoke.each(a, |m| {
        let r = 1.5 + m.k * 3.0;
        let ri = r.ceil() as i32;
        let (x, y) = (m.at.0.round() as i32, m.at.1.round() as i32);
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f32 <= r * r && bayer(x + dx, y + dy) < (1.0 - m.k) * 0.7 {
                    pen.put(x + dx, y + dy, ASH.at(0.2 + m.k * 0.7));
                }
            }
        }
    });
    if age < 22 {
        pen.shock((at.0, at.1 + 12.0), 4.0 + a * if big { 3.4 } else { 2.2 }, 1.2, 0.35, ASH.cooler(1), a / 18.0);
    }
}

/// The "!" over a pennon that has seen you, bobbing, in the danger ink.
fn alert(world: &mut Img, at: (i32, i32), age: u32) {
    let bob = (ease::out((age as f32 / 5.0).min(1.0)) * 4.0) as i32 - ((age as i32 / 3) % 2);
    let fade = age >= 14;
    for dy in 0..5 {
        world.rect(at.0 - 1, at.1 - bob - 11 + dy, 3, 1, 'k');
    }
    for dy in 0..4 {
        if !(fade && age % 2 == 0) {
            world.rect(at.0, at.1 - bob - 10 + dy, 1, 1, 'T');
            world.rect(at.0 - 1, at.1 - bob - 10 + dy, 3, 1, if dy < 3 { 'R' } else { 'k' });
            world.put(at.0, at.1 - bob - 10 + dy, if dy < 3 { 'T' } else { 'k' });
        }
    }
    world.put(at.0, at.1 - bob - 5, 'T');
}

/// What goes on the stone, behind the figures: dust, rings of the captain's
/// blow.
pub(super) fn behind(world: &mut Img, cx: &Cx) {
    let (show, id, t) = (&cx.s.show, cx.id, cx.t);
    let floor = cx.y(FLOOR) as f32;
    for m in show.moments.iter().filter(|m| m.seat == id) {
        let age = m.age(t);
        let x = cx.x(i32::from(m.x)) as f32;
        match m.kind {
            Puff::Jump => dust(world, (x, floor + 1.0), age, hash(m.born as i32, 1, 0x11), 7, 1.5, cx.tick),
            Puff::Land => dust(world, (x, floor + 1.0), age, hash(m.born as i32, 2, 0x11), 10, 2.0, cx.tick),
            Puff::Rise => dust(world, (x, floor + 1.0), age, hash(m.born as i32, 3, 0x11), 12, 2.2, cx.tick),
            Puff::Strike if m.dx == 3 => {
                // The captain's blow: a ring along the stone from where it falls.
                let f = &cx.d.foes[3];
                let mut pen = Pen::new(world, 500);
                let a = age as f32;
                if m.dy == 0 {
                    let mid = cx.x(f.danger.x + f.danger.w / 2) as f32;
                    pen.shock((mid, floor), 3.0 + a * 5.0, 2.0, 0.22, ramps::SMOKE, a / 16.0);
                    pen.shock((mid, floor), 2.0 + a * 3.0, 1.4, 0.22, ramps::EMBER, a / 14.0);
                } else {
                    // The hook sweeps the length of its lane.
                    let tail = cx.x(f.danger.x) as f32;
                    let head = cx.x(f.x - 20) as f32;
                    for k in 0..4 {
                        let along = head + (tail - head) * ease::out(((a - k as f32) / 8.0).clamp(0.0, 1.0));
                        pen.streak((along, floor - 2.0 - k as f32 * 2.0), (-1.0, 0.0), 12.0, ramps::EMBER, a / 16.0);
                    }
                }
            }
            _ => {}
        }
    }
    // Strides raise a little dust of their own: a trail of three puffs.
    let d = cx.d;
    if d.ground.is_some() && matches!(d.pose, 1 | 2) && d.down == 0 {
        let x = cx.x(d.x / Q) as f32;
        for k in 0..3u32 {
            let age = (cx.t + k * 5) % 15;
            let back = -(d.dir as f32) * (4.0 + age as f32 * 0.9 + k as f32 * 2.0);
            let mut pen = Pen::new(world, 40);
            let (px, py) = ((x + back).round() as i32, (floor - 1.0 - age as f32 * 0.12).round() as i32);
            let r = 1 + (age / 6) as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r && bayer(px + dx, py + dy) < 0.8 - age as f32 / 24.0 {
                        pen.put(px + dx, py + dy, DUST.at(0.2 + age as f32 / 20.0));
                    }
                }
            }
        }
    }
}

/// What flies over the figures: bursts, flashes, wards shattering, splashes.
pub(super) fn front(world: &mut Img, cx: &Cx) {
    let (show, id, t, tick) = (&cx.s.show, cx.id, cx.t, cx.tick);
    let floor = cx.y(FLOOR);
    // Blows landed in this lane.
    for h in show.hits.iter().filter(|h| h.seat == id) {
        let age = h.age(t);
        let at = (cx.x(i32::from(h.x)) as f32, cx.y(i32::from(h.y)) as f32);
        let seed = hash(h.born as i32, i32::from(h.foe), 0x51);
        if h.kill {
            wood(world, at, age, seed, h.foe == 3, tick);
        } else {
            juice::impact_burst(world, at, age, seed, h.weight, tick);
        }
    }
    for m in show.moments.iter().filter(|m| m.seat == id) {
        let age = m.age(t);
        let (x, y) = (cx.x(i32::from(m.x)), cx.y(i32::from(m.y)));
        match m.kind {
            Puff::Muzzle => {
                let mut pen = Pen::new(world, 120);
                let c = (x as f32, y as f32);
                pfx::rays(&mut pen, c, 6, 8.0, 1.0, age as f32 * 0.4, ramps::GOLD, age as f32 / 5.0);
                let (dx, dy) = (f32::from(m.dx), f32::from(m.dy));
                if age < 4 && (dx != 0.0 || dy != 0.0) {
                    pen.streak((c.0 + dx * 6.0, c.1 + dy * 6.0), (dx, dy), 7.0, ramps::GOLD, age as f32 / 4.0);
                }
            }
            Puff::Wake => alert(world, (x, y), age),
            Puff::Strike if m.dx < 3 => {
                // A pennon looses its bolt: a puff and a flash at the bow.
                let mut pen = Pen::new(world, 90);
                let c = ((x - 16) as f32, (floor - 24) as f32);
                pfx::rays(&mut pen, c, 5, 8.0, 1.0, 0.3, ramps::EMBER, age as f32 / 6.0);
            }
            Puff::Ward => {
                let mut pen = Pen::new(world, 300);
                let c = (x as f32, y as f32);
                let k = age as f32 / 18.0;
                pfx::rays(&mut pen, c, 8, 18.0, 1.0, 0.2, ramps::HOLY, k);
                pen.shock((c.0, c.1 + 6.0), 3.0 + age as f32 * 1.5, 1.4, 0.5, ramps::HOLY, k);
                let spray = Spray { gravity: -0.03, ..Spray::round(hash(m.born as i32, 9, 0x31), 12, c, (0.4, 1.8), (14.0, 24.0)) };
                spray.each(age as f32, |mm| pfx::mote(&mut pen, &mm, Shape::Star, ramps::HOLY, 1, tick));
            }
            Puff::Rise => {
                let mut pen = Pen::new(world, 160);
                let c = (x as f32, (floor - 20) as f32);
                pen.shock(c, 3.0 + age as f32 * 1.6, 1.2, 1.0, ramps::BONE, age as f32 / 14.0);
            }
            _ => {}
        }
    }
    // A knight struck, a ward broken, a fall into the moat.
    for k in show.downs.iter().filter(|k| k.seat == id) {
        let age = t.saturating_sub(k.born);
        let x = cx.x(i32::from(k.x)) as f32;
        let mut pen = Pen::new(world, 500);
        if k.pit {
            let a = age as f32;
            let y = MOAT as f32;
            if age < 30 {
                pen.shock((x, y + 2.0), 3.0 + a * 1.5, 1.6, 0.3, ramps::WATER, a / 30.0);
                pen.shock((x, y + 2.0), 2.0 + (a - 4.0).max(0.0) * 1.1, 1.2, 0.3, ramps::WATER, a / 26.0);
            }
            let spray = Spray { gravity: 0.15, ..Spray::round(hash(k.born as i32, 4, 0x33), 18, (x, y), (1.0, 3.4), (12.0, 22.0)) };
            spray.each(a, |m| {
                if m.vel.1 < 0.4 || m.k < 0.7 {
                    pfx::mote(&mut pen, &m, Shape::Droplet, ramps::WATER, 1, tick)
                }
            });
            let bubbles = Spray { gravity: -0.05, stagger: 3.0, ..Spray::round(hash(k.born as i32, 5, 0x33), 5, (x, y + 5.0), (0.1, 0.4), (14.0, 24.0)) };
            bubbles.each((a - 6.0).max(0.0), |m| pfx::mote(&mut pen, &m, Shape::Bubble, ramps::WATER, 1, tick));
        } else if k.ward {
            let c = (x, (floor - 22) as f32);
            pen.shock(c, 4.0 + age as f32 * 2.4, 1.8, 1.0, ramps::HOLY, age as f32 / 18.0);
            let shards = Spray { gravity: 0.1, ..Spray::round(hash(k.born as i32, 6, 0x33), 14, c, (1.2, 3.2), (14.0, 26.0)) };
            shards.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::HOLY, 1, tick));
        } else {
            let c = (x, (floor - 20) as f32);
            juice::impact_burst(world, c, age, hash(k.born as i32, 7, 0x33), Weight::Crushing, tick);
        }
    }
    // A bolt that flies past close: a spark where it whistled by.
    let d = cx.d;
    if d.down == 0 && d.hurt == 0 {
        let (kx, ky) = (d.x / Q, d.y / Q);
        let (top, bottom) = (ky - if d.duck { 19 } else { 37 }, ky);
        for s in d.shots.iter().filter(|s| s.enemy) {
            let (bx, by) = (s.x / Q, s.y / Q);
            let near_y = by > top - 14 && by < bottom + 12 && !(by > top - 2 && by < bottom + 2);
            if (bx - kx).abs() <= 7 && near_y {
                let mut pen = Pen::new(world, 60);
                pen.twinkle((cx.x(bx) as f32, cx.y(by) as f32), 2.0, ((t / 2) % 5) as f32 / 5.0, ramps::BONE);
            }
        }
    }
    super::scene::embers(world, cx.cam, t);
}

/// Numbers over the strike, a score flying home: the unshaken layer.
/// `shake` is how far the world is being thrown, so a number stays on its
/// pennon.
pub(super) fn numbers(im: &mut Img, cx: &Cx, shake: (i32, i32), ids: &[u32]) {
    let show: &Show = &cx.s.show;
    // The count and the GO own the middle of the screen; no number flies
    // through them.
    let opening = cx.s.hold > 0 || cx.t < HOLD + GO_LIFE;
    for h in show.hits.iter().filter(|h| h.seat == cx.id) {
        let age = h.age(cx.t);
        let at = (cx.x(i32::from(h.x)) + shake.0, cx.y(i32::from(h.y)) + shake.1);
        if age < 7 {
            juice::hitmarker(im, (at.0, at.1), age, h.kill);
        }
        // One number at a time over the strike: the newest hit's.
        let newest = show.hits.iter().filter(|o| o.seat == cx.id).map(|o| o.age(cx.t)).min();
        if opening || newest != Some(age) {
            continue;
        }
        let text = format!("+{}", h.gain);
        if h.kill {
            let to = hud::anchor(ids, cx.id);
            if age < FLY {
                juice::fly_up(im, &text, (at.0, at.1 - 14), to, age, FLY, 'T');
                // The same box the kit draws: where the number is.
                let t = age as f32 / FLY as f32;
                let (x, y) = crate::motion::arc((at.0 as f32, (at.1 - 14) as f32), (to.0 as f32, to.1 as f32), 26.0, ease::both(t));
                let big = 1 + i32::from(t < 0.5);
                let w = super::super::super::ink::text_width(&text) * big + 4;
                let hh = 7 * big + 4;
                claim_big("fly", (x as i32 - w / 2, y as i32 - hh / 2, w, hh), &text);
            }
        } else if age < POP {
            let scale = 1 + i32::from(h.weight >= Weight::Crushing);
            juice::pop_number(im, &text, (at.0, at.1 - 6), age, POP, scale, 'T');
            let hop = (ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
            let w = super::super::super::ink::text_width(&text) * scale + 4;
            let hh = 7 * scale + 4;
            claim_big("pop", (at.0 - w / 2, at.1 - 6 - hop - hh, w, hh), &text);
        }
    }
}

/// The rim of light round the picture: red for a knight struck, the
/// captain's colour while his blow is coming down on the knight.
pub(super) fn rim(im: &mut Img, cx: &Cx) {
    let show = &cx.s.show;
    let mut k = 0.0f32;
    let mut ink = '7';
    if let Some(a) = show.downs.iter().rev().find(|d| d.seat == cx.id && !d.ward).map(|d| cx.t.saturating_sub(d.born)).filter(|&a| a < 24) {
        k = k.max(0.5 * (1.0 - a as f32 / 24.0));
    }
    // The captain's tell lands on the knight: the rim thickens as it nears.
    let d = cx.d;
    if let Some(f) = d.foes.get(3).filter(|f| f.hp > 0 && f.beat == crate::rules::pennon::Beat::Tell) {
        let (x, y) = (d.x / Q, d.y / Q);
        if f.danger.hits(x, y, if d.duck { 19 } else { 37 }) {
            let s = f.tell_stage() as f32;
            if s >= 1.0 && cx.t / 3 % 2 == 0 {
                k = k.max(0.3 + 0.18 * s);
                ink = 'R';
            }
        }
    }
    if k > 0.0 {
        juice::vignette(im, (0, VIEW_TOP, 384, hud::VIEW_BOTTOM - VIEW_TOP), k, 16, ink);
    }
    let _ = PI;
}
