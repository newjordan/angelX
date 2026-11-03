//! Emitters: motes born in a burst and at a rate, each with a life, a
//! speed and a way of its own hashed from its index, thrown, slowed by
//! drag, pulled by gravity and moved their layer's way. Where a mote is at
//! a frame is worked out from that frame alone.

use super::forms::{self, Mote};
use super::pen::Pen;
use super::recipe::{MOST_MOTES, Motion, Spawn};
use super::rng::unit;
use super::Plan;
use std::f32::consts::TAU;

/// Draw `p`'s motes alive at frame `t`. A looping effect of `period`
/// frames also draws the motes still alive from the round before.
pub(crate) fn draw(pen: &mut Pen, p: &Plan, t: f32, period: Option<f32>) {
    let born = if p.rate > 0.0 { ((p.t1 - p.t0) * p.rate).ceil().max(0.0) as u32 } else { 0 };
    let n = (p.count + born).min(MOST_MOTES);
    for i in 0..n {
        let s = if i < p.count { p.t0 } else { p.t0 + (i - p.count) as f32 / p.rate };
        let u = |salt: u32| unit(i as i32, p.seed as i32, salt);
        let life = (p.life.0 + (p.life.1 - p.life.0) * u(1)).max(0.5);
        for age in [Some(t - s), period.map(|n| t + n - s)].into_iter().flatten() {
            if age >= 0.0 && age < life {
                let m = mote(p, i, s, age, life, t);
                forms::draw(pen, p.form, &m, &p.ramp);
            }
        }
    }
}

/// Mote `i` of `p`, born at frame `s`, `age` frames into its `life`, at
/// frame `t`.
fn mote(p: &Plan, i: u32, s: f32, age: f32, life: f32, t: f32) -> Mote {
    let u = |salt: u32| unit(i as i32, p.seed as i32, salt);
    let k = age / life;
    // An overshooting ease runs a mote's clock past its mark and home.
    let clock = if p.ease.overshoots() { life * p.ease.at(k) } else { age };
    let spin = p.spin.to_radians();
    let size = p.size.0 + (p.size.1 - p.size.0) * k;
    let heat = (p.heat.0 + (p.heat.1 - p.heat.0) * k) * p.gain;
    let turn = u(6) * TAU + spin * clock;
    let seed = p.seed ^ i.wrapping_mul(0x9e37_79b9);
    let theta = u(4) * TAU;
    let (ax, ay) = (p.area.0, p.area.1);
    let finish = |at: (f32, f32), vel: (f32, f32)| Mote { at, vel, k, size, heat, turn, seed, frame: t };
    match p.motion {
        Motion::Gather => {
            let c = p.anchor(t);
            let r = 0.6 + 0.4 * u(5);
            let from = (c.0 + theta.cos() * ax * r, c.1 + theta.sin() * ay * r);
            let e = super::time::Ease::In.at(k);
            let at = (from.0 + (c.0 - from.0) * e, from.1 + (c.1 - from.1) * e);
            let pull = 3.0 * k * k / life;
            return finish(at, ((c.0 - from.0) * pull, (c.1 - from.1) * pull));
        }
        Motion::Orbit => {
            let c = p.anchor(t);
            let r = 0.7 + 0.3 * u(5);
            let a = theta + spin * clock;
            let at = (c.0 + a.cos() * ax * r, c.1 + a.sin() * ay * r);
            return finish(at, (-a.sin() * ax * spin, a.cos() * ay * spin));
        }
        _ => {}
    }
    let base = {
        let c = p.anchor(s);
        let off = match p.spawn {
            Spawn::Point => (0.0, 0.0),
            Spawn::Line => ((u(4) - 0.5) * ax, 0.0),
            Spawn::Ring => (theta.cos() * ax, theta.sin() * ay),
            Spawn::Disc => {
                let r = u(5).sqrt();
                (theta.cos() * ax * r, theta.sin() * ay * r)
            }
            Spawn::Box => ((u(4) - 0.5) * ax, (u(5) - 0.5) * ay),
        };
        (c.0 + off.0, c.1 + off.1)
    };
    let a = if p.spawn == Spawn::Ring { theta } else { (p.angle + (u(2) - 0.5) * p.spread).to_radians() };
    let (dx, dy) = (a.cos(), a.sin() * p.squash);
    let v = p.speed.0 + (p.speed.1 - p.speed.0) * u(3);
    let (run, now) = if p.drag > 0.0 {
        let f = (-p.drag * clock).exp();
        (v * (1.0 - f) / p.drag, v * f)
    } else {
        (v * clock, v)
    };
    let g = p.gravity;
    let mut at = (base.0 + dx * run, base.1 + dy * run + 0.5 * g * clock * clock);
    let mut vel = (dx * now, dy * now + g * clock);
    let phase = u(7) * TAU;
    let (nx, ny) = (-a.sin(), a.cos());
    match p.motion {
        Motion::Wobble => {
            let w = (clock * 1.2 + phase).sin() * p.wobble;
            let dw = (clock * 1.2 + phase).cos() * p.wobble * 1.2;
            at = (at.0 + nx * w, at.1 + ny * w);
            vel = (vel.0 + nx * dw, vel.1 + ny * dw);
        }
        Motion::Zigzag => {
            let tri = ((clock * 0.5 + phase) % 2.0 - 1.0).abs() * 2.0 - 1.0;
            at = (at.0 + nx * tri * p.wobble, at.1 + ny * tri * p.wobble);
        }
        Motion::Spiral | Motion::Curl => {
            let turn = spin * clock * if p.motion == Motion::Spiral { 1.0 } else { 0.4 };
            let (c, sn) = (turn.cos(), turn.sin());
            let (ox, oy) = (at.0 - base.0, at.1 - base.1);
            at = (base.0 + ox * c - oy * sn, base.1 + ox * sn + oy * c);
            vel = (vel.0 * c - vel.1 * sn, vel.0 * sn + vel.1 * c);
        }
        Motion::Bounce => {
            at.1 -= (clock * 0.9 + phase).sin().abs() * p.wobble.max(3.0) * (1.0 - k);
        }
        _ => {}
    }
    finish(at, vel)
}
