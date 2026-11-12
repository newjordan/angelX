//! Painted surfaces for the courses' own materials: tables of 256 by 256 base colours that tile
//! over eight world units (32 texels to a unit). One generator per kind of surface; the palette
//! picks the colours. Everything here is a pure function of its arguments.
#![allow(dead_code)]
use super::noise::{fbm, hash, value};
use std::f32::consts::TAU;

pub type Rgb = [u8; 3];

pub fn table(f: impl Fn(f32, f32) -> Rgb) -> Vec<Rgb> {
    (0..256 * 256).map(|i| f((i % 256) as f32 / 32., (i / 256) as f32 / 32.)).collect()
}

pub fn mulc(c: Rgb, k: f32) -> Rgb { c.map(|v| (v as f32 * k).clamp(0., 255.) as u8) }
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { let t = t.clamp(0., 1.); [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8) }
pub fn add(a: Rgb, b: Rgb, k: f32) -> Rgb { [0, 1, 2].map(|i| (a[i] as f32 + b[i] as f32 * k).clamp(0., 255.) as u8) }

/// A periodic value over the table's eight units, so a surface tiles without a seam.
fn wrap(x: f32, period: f32, seed: u32, scale: f32) -> f32 {
    let a = x / period * TAU;
    value(a.cos() * scale + 31., a.sin() * scale + 17., seed)
}

/// Paved plates `w` by `h` units with a joint and, per plate, its own tone; a plate wears
/// a fine grain, and an edge sheen where it meets its neighbour.
pub fn plates(base: Rgb, joint: Rgb, w: f32, h: f32, seed: u32, tone: f32, grain: f32) -> Vec<Rgb> {
    table(|u, v| {
        let (col, row) = ((u / w).floor() as i32, (v / h).floor() as i32);
        let (fu, fv) = ((u / w).fract(), (v / h).fract());
        let edge = fu.min(1. - fu).min(fv.min(1. - fv) * h / w * 0.6);
        let k = 1. - tone + tone * 2. * hash(col, row, seed);
        let g = (value(u * 9., v * 9., seed + 3) - 0.5) * grain;
        let c = mulc(base, k * (1. + g));
        if fu < 0.035 || fv < 0.05 { joint } else if edge < 0.09 { mulc(c, 0.84) } else { c }
    })
}

/// Rock in horizontal strata with an ember glow rising from the foot of the face: `v` is height
/// above the road in tenths of a unit, so the lowest three units glow and the rest darkens.
pub fn ember_wall(bands: &[Rgb], glow: Rgb, hot: Rgb, seed: u32) -> Vec<Rgb> {
    table(|u, v| {
        let k = (v * 1.7).floor() as i32;
        let band = (hash(k.rem_euclid(20), 0, seed) * bands.len() as f32) as usize % bands.len();
        let grain = value(u * 2., v * 3., seed + 2) - 0.5;
        let crack = (1. - ((value(u * 1.6, v * 1.1, seed + 9) - 0.5).abs() * 7.)).clamp(0., 1.).powi(2);
        let rock = mulc(bands[band], 0.9 + 0.2 * grain);
        // The glow: strongest at the foot, broken by cracks and by the rock's own grain.
        let g = (1. - v / 3.4).clamp(0., 1.).powf(1.5);
        let lit = g * (0.75 + 0.5 * (value(u * 5., v * 2., seed + 5)));
        let c = mix(rock, glow, lit.clamp(0., 1.) * 0.95);
        let c = add(c, hot, crack * (0.15 + 0.8 * g));
        c
    })
}

/// Molten rock. `phase` shifts the swells (0..1), `heat` lifts the whole table to the next
/// step of a lava's pulse. A whole number of waves to the table's eight units, so it tiles.
pub fn lava(dark: Rgb, mid: Rgb, hot: Rgb, phase: f32, heat: f32, seed: u32) -> Vec<Rgb> {
    table(|u, v| {
        let (a, b) = (u / 8. * TAU, v / 8. * TAU);
        let p = phase * TAU;
        let w1 = (2. * a + 1.6 * (b + p).sin()).sin();
        let w2 = (3. * b + 1.3 * (a * 2. + p).sin() + 1.).sin();
        let w3 = (3. * a - 2. * b + 2. * (a + b).sin()).sin();
        let swell = (0.5 + 0.22 * (w1 + w2) + 0.1 * w3).clamp(0., 1.);
        let crack = (1. - (w1 * w2).abs() * 4.5).clamp(0., 1.);
        let crust = (1. - (w3 * w1).abs() * 7.).clamp(0., 1.);
        let grain = value(u * 4., v * 4., seed) - 0.5;
        let base = mix(dark, mid, (swell * 1.1 + heat * 0.4).clamp(0., 1.));
        let c = mix(base, hot, (crack * 0.85 + heat * 0.35 + crust * 0.2).clamp(0., 1.));
        mulc(c, 1. + 0.08 * grain)
    })
}

/// Cooled slag crust: near black, with a red vein where it cracked.
pub fn crust(base: Rgb, vein: Rgb, seed: u32) -> Vec<Rgb> {
    table(|u, v| {
        let n = fbm(u * 1.4, v * 1.4, seed, 3);
        let line = (1. - ((fbm(u * 1.1 + 5., v * 1.1, seed + 4, 3) - 0.5).abs() * 12.)).clamp(0., 1.).powi(2);
        add(mulc(base, 0.7 + 0.6 * n), vein, line * 0.8)
    })
}

/// Speckled ground: base with a fine grain and a scatter of bright flecks (cinders).
pub fn flecked(base: Rgb, fleck: Rgb, density: f32, seed: u32, amount: f32) -> Vec<Rgb> {
    table(|u, v| {
        let n = fbm(u * 2.2, v * 2.2, seed, 3);
        let c = mulc(base, 1. + amount * (n - 0.5) * 2.);
        let h = hash((u * 20.) as i32, (v * 20.) as i32, seed + 77);
        if h > 1. - density { mix(c, fleck, 0.8) } else { c }
    })
}

/// Riveted iron: broad plate faces, thin seams and a dot of rivet at the corners.
pub fn iron(base: Rgb, seam: Rgb, rivet: Rgb, seed: u32) -> Vec<Rgb> {
    table(|u, v| {
        let (fu, fv) = ((u * 2.).fract(), (v * 2.).fract());
        let k = 0.85 + 0.3 * hash((u * 2.) as i32, (v * 2.) as i32, seed);
        let n = value(u * 8., v * 8., seed + 1) - 0.5;
        if fu < 0.06 || fv < 0.06 { return seam; }
        if (fu - 0.2).abs() < 0.05 && (fv - 0.2).abs() < 0.05 { return rivet; }
        mulc(base, k * (1. + 0.14 * n))
    })
}

/// Hazard bars on a post or a kerb: diagonal bands of two colours.
pub fn bars(a: Rgb, b: Rgb, period: f32) -> Vec<Rgb> {
    table(|u, v| if ((u + v) / period).rem_euclid(2.) < 1. { a } else { b })
}

/// Coursed brick with a warm mortar joint.
pub fn brick(base: Rgb, mortar: Rgb, w: f32, h: f32, seed: u32) -> Vec<Rgb> {
    table(|u, v| {
        let row = (v / h).floor();
        let off = if (row as i32) % 2 == 0 { 0. } else { w * 0.5 };
        let col = ((u + off) / w).floor();
        let (fu, fv) = (((u + off) / w).fract(), (v / h).fract());
        if fu < 0.06 || fv < 0.1 { return mortar; }
        let soot = value(u * 2., v * 3., seed) - 0.5;
        mulc(base, 0.75 + 0.5 * hash(col as i32, row as i32, seed + 1) + 0.25 * soot)
    })
}

/// A scattering of dither-friendly noise for the sky's smoke: periodic round the horizon.
pub fn ring_noise(az: f32, y: f32, seed: u32, freq: f32) -> f32 {
    let r = freq;
    fbm(az.cos() * r + y, az.sin() * r - y * 0.6, seed, 4)
}

pub fn wrapped(x: f32, period: f32, seed: u32, scale: f32) -> f32 { wrap(x, period, seed, scale) }
