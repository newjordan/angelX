//! The air of a place, made fresh each frame for what is in view: flames that lick, sparks that
//! rise, embers and ash drifting past the lens. Every particle is a pure function of the race
//! clock and a hash of its cell, so the same tick draws the same frame on every seat. The
//! particles are fixed in the world, so a pod racing through them sees them stream by: that is
//! most of what makes the speed felt.
use super::frame::row_on;
use super::geo::{Fr, Soup};
use super::look::style;
use super::noise::hash;
use super::palette::*;
use super::route::{Kind, Route};
use super::trackside;
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Tri;
use std::f32::consts::TAU;

/// The cell the drifting particles are scattered in, metres.
const CELL: f32 = 9.;

/// What drifts: the flake (steady, pale), the two lights of its glints, how many are glints, how
/// thick it lies near and far, the wind (m/s) and a size.
struct Air { flake: u8, glint: [u8; 2], glints: f32, near: f32, far: f32, wind: (f32, f32, f32), size: f32 }

pub fn air(out: &mut Vec<Tri>, r: &dyn Route, s: f32, eye: V3, heading: f32, ticks: u32) {
    let here = s.rem_euclid(r.lap());
    let look = r.zone(here + 60.).0;
    let mut g = Soup::new();
    match style(look).beat {
        1 => {
            flames(&mut g, r, here, ticks);
            let lava = r.sink(here + 40.) > 0.;
            drift(&mut g, eye, heading, ticks, &Air { flake: ASH_FLAKE, glint: [EMBER_BRIGHT, SPARK], glints: if lava { 0.5 } else { 0.32 }, near: 0.24, far: 0.12, wind: (-1.6, 0.7, 1.1), size: 1. });
        }
        2 => {
            // Motes of crystal dust hang in the dark and turn slowly; here and there one flashes.
            drift(&mut g, eye, heading, ticks, &Air { flake: MOTE, glint: [CYAN_GLOW, VIOLET_GLOW], glints: 0.45, near: 0.3, far: 0.16, wind: (0.5, -0.3, 0.45), size: 0.9 });
        }
        3 => {
            flames(&mut g, r, here, ticks);
            towers(&mut g, r, here, ticks);
            glints(&mut g, r, here, eye, ticks);
            // Moths and sparks round the lanterns: few, warm, quick.
            drift(&mut g, eye, heading, ticks, &Air { flake: SPRAY, glint: [TORCH, LANTERN_GLINT], glints: 0.85, near: 0.1, far: 0.05, wind: (0.8, 0.6, 0.9), size: 0.8 });
        }
        _ => {}
    }
    out.extend(g.t);
}

/// The flames of the beat's objects in view (pylons, braziers), tongues of three heights that
/// jump with the clock, and a spark or two climbing off each.
fn flames(g: &mut Soup, r: &dyn Route, here: f32, ticks: u32) {
    let beat = trackside::beat_of(r);
    let pitch = trackside::pitch(beat);
    let off = if beat == 2 { 9. } else { 0. };
    let (k0, k1) = (((here - 30. - off) / pitch).floor() as i32, ((here + 190. - off) / pitch).ceil() as i32);
    let t = ticks as f32;
    let (hot, cool) = if beat == 3 { (TORCH, TORCH_DIM) } else { (EMBER_BRIGHT, FLAME_LICK) };
    let size = if beat == 3 { 0.8 } else { 1. };
    for k in k0.max(0)..=k1 {
        // The beats run from the line to the lap's end and stop there: the seam has no flames.
        let Some(base) = trackside::flame_base(r, k) else { continue };
        for i in 0..3 {
            let h = hash(k, i, 91);
            let flick = 0.55 + 0.45 * (t * (0.55 + 0.3 * h) + h * TAU).sin().abs();
            let (dx, dy) = ((h - 0.5) * 0.5, (hash(k, i, 92) - 0.5) * 0.5);
            let hgt = (0.9 + 1.5 * hash(k, i, 93)) * flick * size;
            g.spike(base + v3(dx * 0.6, dy * 0.6, 0.), base + v3(dx, dy, hgt), (0.3 - 0.06 * i as f32) * size, if i == 0 { hot } else { cool });
        }
        for i in 0..2 {
            let life = ((t * 0.045 + hash(k, i, 94) * 5.) % 1.).abs();
            let (ox, oy) = ((hash(k, i, 95) - 0.5) * 2.6 * life, (hash(k, i, 96) - 0.5) * 2.6 * life);
            let c = base + v3(ox, oy, 0.8 + 4.5 * life);
            speck(g, c, 0.2 * (1. - life * 0.6), SPARK, v3(0., 1., 0.));
        }
    }
}

/// A little square of light facing along `up_axis`'s perpendicular, the speck of an ember.
fn speck(g: &mut Soup, c: V3, r: f32, mat: u8, along: V3) {
    let l = v3(-along.y, along.x, 0.);
    let l = if l.length() < 1e-3 { v3(1., 0., 0.) } else { l.normalize() };
    g.quad(c - l * r - v3(0., 0., r), c + l * r - v3(0., 0., r), c + l * r + v3(0., 0., r), c - l * r + v3(0., 0., r), mat);
}

/// Embers and ash, scattered through a lattice of cells round the lens and carried on a wind:
/// each one wraps inside its own cell, so it never leaves the lattice and never pops.
fn drift(g: &mut Soup, eye: V3, heading: f32, ticks: u32, a: &Air) {
    let t = ticks as f32 / 30.;
    let (fx, fy) = (heading.cos(), heading.sin());
    let (lx, ly) = (-fy, fx);
    // The box that holds the view's cone: centred 70 m ahead, 90 m a side.
    let (cx, cy) = (eye.x + fx * 70., eye.y + fy * 70.);
    let (i0, i1) = (((cx - 95.) / CELL).floor() as i32, ((cx + 95.) / CELL).ceil() as i32);
    let (j0, j1) = (((cy - 95.) / CELL).floor() as i32, ((cy + 95.) / CELL).ceil() as i32);
    let layer0 = ((eye.z - 12.) / CELL).floor() as i32;
    for i in i0..i1 {
        for j in j0..j1 {
            let (mx, my) = ((i as f32 + 0.5) * CELL - eye.x, (j as f32 + 0.5) * CELL - eye.y);
            let (ahead, side) = (mx * fx + my * fy, mx * lx + my * ly);
            if ahead < 2. || ahead > 150. || side.abs() > ahead * 0.95 + 14. { continue; }
            for kz in 0..4 {
                let layer = layer0 + kz;
                let h = hash(i * 7 + layer, j * 13 - layer, 0xA5);
                let density = if ahead < 60. { a.near } else { a.far };
                if h > density { continue; }
                // Its place in the cell, carried by the wind and held inside the cell.
                let (hx, hy, hz) = (hash(i, j, layer as u32 + 11), hash(i, j, layer as u32 + 12), hash(i, j, layer as u32 + 13));
                let wind = (a.wind.0 * (1. + 0.75 * hx), a.wind.1 * (1. + 1.1 * hy), a.wind.2 * (1. + 1.2 * hz));
                let wrap = |v: f32| v.rem_euclid(1.);
                let (px, py, pz) = (wrap(hx + wind.0 * t / CELL), wrap(hy + wind.1 * t / CELL), wrap(hz + wind.2 * t / CELL));
                // A particle at the cell's edge would pop across it: it fades out first.
                if px < 0.06 || px > 0.94 || py < 0.06 || py > 0.94 || pz < 0.06 || pz > 0.94 { continue; }
                let pos = v3((i as f32 + px) * CELL, (j as f32 + py) * CELL, (layer as f32 + pz) * CELL);
                let d = ((pos.x - eye.x) * fx + (pos.y - eye.y) * fy).max(1.);
                let size = (0.1 + d * 0.003 + 0.07 * hash(i, j, 99)) * a.size;
                let ember = hash(i, j, layer as u32 + 14) < a.glints;
                // Embers flicker; ash is steady and pale.
                if ember {
                    let blink = (t * (3. + 4. * hx) + hy * TAU).sin();
                    if blink < -0.5 { continue; }
                    speck(g, pos, size * 0.8, if blink > 0.4 { a.glint[1] } else { a.glint[0] }, v3(fx, fy, 0.));
                } else {
                    speck(g, pos, size, a.flake, v3(fx, fy, 0.));
                }
            }
        }
    }
}


/// Fire on the drawbridge's towers, where the leap is staged: a great flame at the head of each of
/// the four, in sight from the whole rampart.
fn towers(g: &mut Soup, r: &dyn Route, here: f32, ticks: u32) {
    let t = ticks as f32;
    for p in r.pieces().iter().filter(|p| p.kind == Kind::Drawbridge) {
        if p.s1 < here - 40. || p.s0 > here + 300. { continue; }
        for (s, dir) in [(p.s0, -1.0f32), (p.s1, 1.0)] {
            let row = row_on(r, 0, s);
            let w = row.wl.max(row.wr) + 1.4;
            let fr = Fr::on(&row, 0.);
            for (j, sd) in [-1.0f32, 1.0].into_iter().enumerate() {
                let base = fr.pt(dir, sd * (w + 2.), 19.2);
                for i in 0..4 {
                    let h = hash(j as i32 * 9 + i, s as i32, 211);
                    let flick = 0.5 + 0.5 * (t * (0.5 + 0.4 * h) + h * TAU).sin().abs();
                    let (dx, dy) = ((h - 0.5) * 1.6, (hash(i, s as i32, 212) - 0.5) * 1.6);
                    g.spike(base + v3(dx * 0.5, dy * 0.5, 0.), base + v3(dx, dy, (2.2 + 3.2 * hash(i, j as i32, 213)) * flick), 0.8 - 0.14 * i as f32, if i == 0 { TORCH } else { TORCH_DIM });
                }
            }
        }
    }
}

/// The moat takes the lanterns of the barrier and gives them back: dashes of warm light on the
/// water, drawn out toward the eye and shivering, below each lamp where the land falls to water.
fn glints(g: &mut Soup, r: &dyn Route, here: f32, eye: V3, ticks: u32) {
    let t = ticks as f32;
    let step = 16.;
    let (k0, k1) = (((here - 10.) / step).ceil() as i32, ((here + 130.) / step).floor() as i32);
    for k in k0..=k1 {
        let s = k as f32 * step;
        let sink = r.sink(s);
        if sink <= 0. || style(r.zone(s).0).liquid != Some(WATER) { continue; }
        let row = row_on(r, 0, s);
        let level = row.h - (0.55 * sink).min(super::terrain::WATER_DEEP);
        for sd in [-1.0f32, 1.0] {
            let edge = row.pt(sd * (row.half(sd) + 2.4), 0.);
            let p = v3(edge.x, edge.y, level + 0.04);
            let to = v3(eye.x - p.x, eye.y - p.y, 0.);
            let len = to.length().max(1.);
            let d = to * (1. / len);
            let side = v3(-d.y, d.x, 0.);
            for i in 0..6 {
                let h = hash(k * 2 + (sd > 0.) as i32, i, 221);
                let ph = (t * (0.12 + 0.1 * h) + h * TAU).sin();
                if ph < -0.35 { continue; }
                let at = p + d * (1.2 + i as f32 * 1.5 * (1. + len * 0.012)) + side * ((h - 0.5) * 1.4 + 0.25 * ph);
                let (hl, hw) = (0.8 + 0.6 * h, 0.14 + 0.08 * ph.abs());
                g.quad(at - d * hl - side * hw, at + d * hl - side * hw, at + d * hl + side * hw, at - d * hl + side * hw, if i % 2 == 0 { LANTERN_GLINT } else { TORCH_DIM });
            }
        }
    }
}
