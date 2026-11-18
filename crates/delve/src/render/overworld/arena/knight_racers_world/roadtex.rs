//! The road's own surfaces, laid in the road's own frame. A table here is not tiled over the
//! world: `u` runs along the road (`ALONG` metres to a unit, so a table is eight units, a whole
//! number of the place's pylons or crystals long) and `v` runs across it, the width of the road
//! in eight units, the right edge at 0 and the left at 8. A mark made for the edge is therefore
//! at the edge on every bend and every width. Everything here is a pure function of its arguments.
#![allow(dead_code)]
use super::noise::hash;
use super::tex::{Rgb, add, mix, mulc, table};

pub const EMBER_ALONG: f32 = 7.0;
const WIDE: f32 = 22.0;

pub(super) fn pvalue(x: f32, y: f32, px: i32, seed: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let s = |t: f32| t * t * (3. - 2. * t);
    let (tx, ty) = (s(x - fx), s(y - fy));
    let h = |i: i32, j: i32| hash(i.rem_euclid(px), j, seed);
    let (a, b, c, d) = (h(ix, iy), h(ix + 1, iy), h(ix, iy + 1), h(ix + 1, iy + 1));
    (a + (b - a) * tx) + ((c + (d - c) * tx) - (a + (b - a) * tx)) * ty
}

pub(super) fn voronoi(x_m: f32, y_m: f32, cx: f32, cy: f32, nx: i32, seed: u32) -> (f32, f32) {
    let (gx, gy) = (x_m / cx, y_m / cy);
    let (ix, iy) = (gx.floor() as i32, gy.floor() as i32);
    let (mut f1, mut f2) = (f32::MAX, f32::MAX);
    for dj in -1..=1 {
        for di in -1..=1 {
            let (i, j) = (ix + di, iy + dj);
            let (jx, jy) = (hash(i.rem_euclid(nx), j, seed), hash(i.rem_euclid(nx), j, seed + 7));
            let (px, py) = ((i as f32 + 0.15 + 0.7 * jx) * cx, (j as f32 + 0.15 + 0.7 * jy) * cy);
            let d = (px - x_m).hypot(py - y_m);
            if d < f1 { f2 = f1; f1 = d; } else if d < f2 { f2 = d; }
        }
    }
    (f1, f2)
}

pub(super) fn smoothstep(a: f32, b: f32, x: f32) -> f32 { let t = ((x - a) / (b - a)).clamp(0., 1.); t * t * (3. - 2. * t) }

pub fn ember_road(over_drop: bool) -> Vec<Rgb> {
    const BASALT: Rgb = [30, 13, 9];
    const JOINT: Rgb = [3, 2, 2];
    const GLOW_LO: Rgb = [170, 46, 12];
    const GLOW_HI: Rgb = [255, 160, 56];
    let reach = if over_drop { 4.6 } else { 3.2 };
    table(|u, v| {
        let along = u * EMBER_ALONG;
        let f = v / 8.;
        let (across, ed) = (f * WIDE, f.min(1. - f) * WIDE);
        let plate = (along / 7.).floor() as i32;
        let ph = hash(plate.rem_euclid(8), 0, 301);
        let mut c = mulc(BASALT, 0.75 + 0.6 * ph);
        if hash(plate.rem_euclid(8), 1, 302) < 0.3 { c = mix(c, [74, 26, 14], 0.55); }
        let streak = pvalue(along / 3.5, across / 0.7, 16, 311);
        c = mulc(c, 0.7 + 0.6 * streak);
        let heat = (-ed / reach).exp();
        let (f1, f2) = voronoi(along, across, 9., 3.0, 6, 321);
        let edge = f2 - f1;
        let wid = 0.22 + 1.1 * heat * if over_drop { 1.3 } else { 1. };
        let crack = 1. - smoothstep(0.02, wid, edge);
        let keep = smoothstep(0.12, 0.4, heat + 0.55 * pvalue(along / 6., across / 3., 8, 331) - 0.1);
        let k = crack * keep;
        let ember = mix(GLOW_LO, GLOW_HI, (k * 1.1 + heat * 0.45).clamp(0., 1.));
        c = mix(c, ember, (k * 1.0).clamp(0., 1.));
        c = add(c, [120, 34, 10], heat.powf(1.4) * 0.55);
        let rim = (1. - ed / 1.1).clamp(0., 1.);
        c = add(c, [190, 80, 24], rim * 0.7 * (0.6 + 0.4 * pvalue(along / 2., across / 0.5, 16, 341)));
        for k in 0..4 {
            let side_left = k % 2 == 0;
            let pa = (along - 14. * k as f32 + 28.).rem_euclid(56.);
            let da = if pa > 28. { pa - 56. } else { pa };
            let de = if side_left { (1. - f) * WIDE } else { f * WIDE };
            let g = (-((da / 4.5).powi(2) + (de / 3.4).powi(2))).exp();
            c = add(c, [150, 70, 22], g * 0.9);
        }
        let jd = along.rem_euclid(7.);
        if jd < 0.2 || jd > 6.85 { c = mix(c, JOINT, 0.92); }
        c
    })
}

fn voronoi_wrap(x_m: f32, y_m: f32, cell: f32, n: i32, seed: u32) -> (f32, f32) {
    let (gx, gy) = (x_m / cell, y_m / cell);
    let (ix, iy) = (gx.floor() as i32, gy.floor() as i32);
    let (mut f1, mut f2) = (f32::MAX, f32::MAX);
    for dj in -1..=1 {
        for di in -1..=1 {
            let (i, j) = (ix + di, iy + dj);
            let (wi, wj) = (i.rem_euclid(n), j.rem_euclid(n));
            let (px, py) = ((i as f32 + 0.15 + 0.7 * hash(wi, wj, seed)) * cell, (j as f32 + 0.15 + 0.7 * hash(wi, wj, seed + 7)) * cell);
            let d = (px - x_m).hypot(py - y_m);
            if d < f1 { f2 = f1; f1 = d; } else if d < f2 { f2 = d; }
        }
    }
    (f1, f2)
}

pub fn cinder_ground() -> Vec<Rgb> {
    table(|u, v| {
        let (x, y) = (u * 4., v * 4.);
        let mut c = mulc([30, 15, 10], 0.7 + 0.7 * pvalue_both(x, y, 4., 701));
        let (f1, f2) = voronoi_wrap(x, y, 4., 8, 711);
        let live = smoothstep(0.38, 0.6, pvalue_both(x, y, 8., 721));
        let width = 0.1 + 0.3 * live;
        let crack = (1. - smoothstep(0.0, width, f2 - f1)) * live.max(0.12);
        c = mix(c, mix([160, 40, 10], [255, 150, 50], crack), crack.min(1.) * 0.95);
        let pool = smoothstep(0.7, 0.74, pvalue_both(x, y, 8., 731) * 0.7 + 0.3 * pvalue_both(x, y, 4., 732));
        c = mix(c, mix([200, 70, 18], [255, 170, 60], pvalue_both(x, y, 2., 741)), pool * 0.9);
        c
    })
}

pub(super) fn pvalue_both(x_m: f32, y_m: f32, cell: f32, seed: u32) -> f32 {
    let n = (32. / cell).round() as i32;
    let (x, y) = (x_m / cell, y_m / cell);
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let s = |t: f32| t * t * (3. - 2. * t);
    let (tx, ty) = (s(x - fx), s(y - fy));
    let h = |i: i32, j: i32| hash(i.rem_euclid(n), j.rem_euclid(n), seed);
    let (a, b, c, d) = (h(ix, iy), h(ix + 1, iy), h(ix, iy + 1), h(ix + 1, iy + 1));
    (a + (b - a) * tx) + ((c + (d - c) * tx) - (a + (b - a) * tx)) * ty
}
