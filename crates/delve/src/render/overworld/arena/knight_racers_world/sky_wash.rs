//! Moonward's night: cold, clear and nearly black overhead over a pale blue horizon, one huge low moon
//! with its two halos and its sea of dim marks, stars, thin silver streaks of cloud, and two ranges of
//! far mountains whose faces to the moon are lit. The sky is not read from a band of the sky table: it
//! is sixteen flat steps of colour, one per ring of the dome (see `palette::WASH_SKY`), so the four
//! skies of the other places keep every texel they had. Everything is built fresh for the frame, and
//! only what is in the lens.
use super::geo::Soup;
use super::noise::{fbm, hash};
use super::palette::*;
use crate::render::world3d::math::{V3, v3};
use std::f32::consts::{PI, TAU};

use super::sky::{CONE, RADIUS};
const SEGS: usize = 11;

const MOON_SIDE: f32 = 0.10;
const MOON_EL: f32 = 0.095;
const DISC: f32 = 0.14;
const HALO_IN: f32 = 0.19;
const HALO_OUT: f32 = 0.26;

fn dir(elev: f32, az: f32) -> V3 { v3(elev.cos() * az.cos(), elev.cos() * az.sin(), elev.sin()) }

fn wrap(a: f32) -> f32 { (a + PI).rem_euclid(TAU) - PI }

pub fn dome(g: &mut Soup, eye: V3, heading: f32, ticks: u32, pan: f32) {
    let seen = |az: f32, slack: f32| wrap(az - heading).abs() < CONE + slack;
    for (k, w) in WASH_RINGS.windows(2).enumerate() {
        let mat = flat((WASH_SKY + k) as u8);
        let at = |e: f32, j: usize| eye + dir(e, heading - CONE + 2. * CONE * j as f32 / SEGS as f32) * RADIUS;
        for j in 0..SEGS {
            let (p00, p01, p10, p11) = (at(w[0], j), at(w[0], j + 1), at(w[1], j), at(w[1], j + 1));
            g.tri(p00, p01, p11, mat);
            g.tri(p00, p11, p10, mat);
        }
    }
    let moon_az = pan + MOON_SIDE;
    stars(g, eye, ticks, &seen);
    clouds(g, eye, pan, ticks, &seen);
    if seen(moon_az, HALO_OUT) { moon(g, eye, moon_az); }
    ridges(g, eye, heading, pan, moon_az);
}

fn speck(g: &mut Soup, eye: V3, el: f32, az: f32, size: f32, mat: u8) {
    let c = eye + dir(el, az) * (RADIUS - 24.);
    let (r, u) = (c - eye, v3(-az.sin(), az.cos(), 0.));
    let up = r.normalize().cross(u).normalize();
    g.tri(c - u * size, c + u * size, c + up * size, mat);
}

fn stars(g: &mut Soup, eye: V3, ticks: u32, seen: &dyn Fn(f32, f32) -> bool) {
    for i in 0..220u32 {
        let az = hash(i as i32, 1, 177) * TAU;
        let el = 0.06 + hash(i as i32, 2, 177).powf(1.25) * 1.38;
        if !seen(az, 0.) || (i % 5 == 0 && (ticks / 7 + i) % 6 == 0) { continue; }
        let size = 5. * (0.7 + hash(i as i32, 3, 177) * 0.9);
        speck(g, eye, el, az, size, STAR);
    }
}

fn clouds(g: &mut Soup, eye: V3, pan: f32, ticks: u32, seen: &dyn Fn(f32, f32) -> bool) {
    let drift = ticks as f32 * 0.00022;
    for i in 0..22i32 {
        let az = pan + drift + (hash(i, 1, 181) - 0.5) * 3.0;
        let el = 0.05 + 0.62 * hash(i, 2, 181).powf(1.3);
        if !seen(az, -0.1) { continue; }
        let (len, thick) = (0.07 + 0.3 * hash(i, 3, 181), 0.0025 + 0.006 * hash(i, 4, 181));
        let tilt = (hash(i, 5, 181) - 0.5) * 0.06;
        let d = RADIUS - 30.;
        let c = eye + dir(el, az) * d;
        let u = v3(-az.sin(), az.cos(), 0.);
        let up = v3(0., 0., 1.);
        let (hl, ht) = (len * d * 0.5, thick * d);
        let tip = u * hl;
        g.tri(c - tip - up * (tilt * d), c + tip + up * (tilt * d), c + up * ht, WA_CLOUD);
        g.tri(c - tip - up * (tilt * d), c + tip + up * (tilt * d), c - up * ht, WA_CLOUD);
    }
}

fn disc(g: &mut Soup, eye: V3, rr: V3, u: V3, up: V3, (ox, oy): (f32, f32), rad: f32, n: usize, wob: f32, seed: i32, dist: f32, mat: u8) {
    let on = |dx: f32, dy: f32| eye + (rr + u * dx + up * dy).normalize() * dist;
    let c = on(ox, oy);
    let pt = |i: usize| {
        let a = i as f32 / n as f32 * TAU;
        let r = rad.tan() * (1. - wob + 2. * wob * hash((i % n) as i32, seed, 191));
        on(ox + a.cos() * r, oy + a.sin() * r)
    };
    for i in 0..n { g.tri(c, pt(i), pt(i + 1), mat); }
}

fn moon(g: &mut Soup, eye: V3, az: f32) {
    let rr = dir(MOON_EL, az);
    let u = v3(-az.sin(), az.cos(), 0.);
    let up = rr.cross(u).normalize();
    const OUT: f32 = 1445.;
    const IN: f32 = 1428.;
    const FACE: f32 = 1414.;
    const SEA: f32 = 1405.;
    disc(g, eye, rr, u, up, (0., 0.), HALO_OUT, 48, 0., 0, OUT, WA_HALO_OUT);
    disc(g, eye, rr, u, up, (0., 0.), HALO_IN, 40, 0., 0, IN, WA_HALO_IN);
    disc(g, eye, rr, u, up, (0., 0.), DISC, 48, 0., 0, FACE, WA_MOON);
    let r = DISC.tan();
    for (k, &(x, y, s)) in [(-0.36f32, 0.24f32, 0.26f32), (0.02, 0.42, 0.15), (-0.14, -0.28, 0.30), (0.44, -0.08, 0.14)].iter().enumerate() {
        disc(g, eye, rr, u, up, (x * r, y * r), (s * r).atan(), 12, 0.3, 10 + k as i32, SEA, WA_CLOUD);
    }
    for (k, &(x, y, s)) in [(0.3f32, 0.14f32, 0.06f32), (-0.04, 0.02, 0.045), (0.2, -0.46, 0.055), (0.52, 0.4, 0.04)].iter().enumerate() {
        disc(g, eye, rr, u, up, (x * r, y * r), (s * r).atan(), 9, 0.15, 30 + k as i32, SEA - 3., WA_CLOUD);
        disc(g, eye, rr, u, up, (x * r - 0.012 * s * r * 4., y * r), (s * r * 0.55).atan(), 9, 0.15, 40 + k as i32, SEA - 6., WA_MOON);
    }
    for &(y, len) in &[(0.05f32, 1.6f32), (-0.55, 1.2)] {
        let at = eye + (rr + up * (y * r)).normalize() * (SEA - 12.);
        let (hl, ht) = (len * r * SEA * 0.5, r * SEA * 0.022);
        g.tri(at - u * hl, at + u * hl, at + up * ht, WA_CLOUD);
        g.tri(at - u * hl, at + u * hl, at - up * ht, WA_CLOUD);
    }
}

fn ridges(g: &mut Soup, eye: V3, heading: f32, pan: f32, moon_az: f32) {
    for &(dist, tall, step, seed, lit, dark) in &[
        (1320., 0.21f32, 0.028f32, 331u32, WA_HALO_OUT, MOON_HALO_FAR),
        (1230., 0.07, 0.032, 337u32, NIGHT_BLUE, SKYLINE),
    ] {
        let n = ((2. * (CONE + 0.1)) / step).ceil() as i32;
        let first = heading - CONE - 0.1;
        let el_at = |i: i32| -> f32 {
            let az = first + i as f32 * step;
            let x = (az - pan) * if dist > 1300. { 4.2 } else { 6.5 };
            let r = 1. - (2. * fbm(x + seed as f32, 7.3, seed, 4) - 1.).abs();
            let env = 0.3 + 0.7 * fbm(x * 0.22 + 9., 2.1, seed + 3, 2).powf(1.2);
            0.003 + tall * env * r.powf(1.25)
        };
        let els: Vec<f32> = (0..=n).map(el_at).collect();
        let at = |i: i32| { let az = first + i as f32 * step; eye + v3(az.cos(), az.sin(), 0.) * dist + v3(0., 0., els[i as usize].tan() * dist) };
        let foot = -0.12f32.tan() * dist;
        for i in 0..n {
            let (a, b) = (at(i), at(i + 1));
            let (lo, hi) = ((i - 2).max(0) as usize, (i + 3).min(n) as usize);
            let slope = els[hi] - els[lo];
            let mid = wrap(first + (i as f32 + 0.5) * step - moon_az);
            let lit_face = (mid < 0. && slope < 0.) || (mid > 0. && slope > 0.);
            let m = if lit_face { lit } else { dark };
            let (fa, fb) = (v3(a.x, a.y, eye.z + foot), v3(b.x, b.y, eye.z + foot));
            g.tri(fa, fb, b, m);
            g.tri(fa, b, a, m);
        }
    }
}
