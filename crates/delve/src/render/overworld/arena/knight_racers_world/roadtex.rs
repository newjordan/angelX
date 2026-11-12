//! The road's own surfaces, laid in the road's own frame. A table here is not tiled over the
//! world: `u` runs along the road (`ALONG` metres to a unit, so a table is eight units, a whole
//! number of the place's pylons or crystals long) and `v` runs across it, the width of the road
//! in eight units, the right edge at 0 and the left at 8. A mark made for the edge is therefore
//! at the edge on every bend and every width. Everything here is a pure function of its arguments.
#![allow(dead_code)]
use super::noise::hash;
use super::tex::{Rgb, add, mix, mulc, table};

/// Metres along the road to one unit of `u`: the Foundry Run's table is 56 m, four pylons.
pub const EMBER_ALONG: f32 = 7.0;
/// What a road is taken to measure across when a mark is cut to a width in metres: the table is
/// stretched to the real width in the world.
const WIDE: f32 = 22.0;

/// Smooth value noise that repeats every `px` cells along `x`, so a table closes along the road.
pub(super) fn pvalue(x: f32, y: f32, px: i32, seed: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let s = |t: f32| t * t * (3. - 2. * t);
    let (tx, ty) = (s(x - fx), s(y - fy));
    let h = |i: i32, j: i32| hash(i.rem_euclid(px), j, seed);
    let (a, b, c, d) = (h(ix, iy), h(ix + 1, iy), h(ix, iy + 1), h(ix + 1, iy + 1));
    (a + (b - a) * tx) + ((c + (d - c) * tx) - (a + (b - a) * tx)) * ty
}

/// The nearest two feature points of a lattice of `nx` by `ny` cells, `cx` by `cy` metres each,
/// that closes along `x`: the distances to them, in metres. Cells are cut where the two are equal.
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

/// The Foundry Run's road: black basalt in plates seven metres long, cooled cracks that glow
/// along the road's own direction and thicken toward both edges where the heat is, a lava pool
/// of light at each pylon, a hot edge. `over_drop` is the same road where the land falls to lava:
/// the heat is wider and the cracks open.
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
        // Plates: seven metres each, each with its own tone, and now and then one still hot.
        let plate = (along / 7.).floor() as i32;
        let ph = hash(plate.rem_euclid(8), 0, 301);
        let mut c = mulc(BASALT, 0.75 + 0.6 * ph);
        if hash(plate.rem_euclid(8), 1, 302) < 0.3 { c = mix(c, [74, 26, 14], 0.55); }
        // Long dark streaks the way the road runs.
        let streak = pvalue(along / 3.5, across / 0.7, 16, 311);
        c = mulc(c, 0.7 + 0.6 * streak);
        // Heat: strongest at the edge, fading over `reach` metres.
        let heat = (-ed / reach).exp();
        // Cracks: cells stretched along the road, cut where two points are equally near.
        let (f1, f2) = voronoi(along, across, 9., 3.0, 6, 321);
        let edge = f2 - f1;
        let wid = 0.22 + 1.1 * heat * if over_drop { 1.3 } else { 1. };
        let crack = 1. - smoothstep(0.02, wid, edge);
        let keep = smoothstep(0.12, 0.4, heat + 0.55 * pvalue(along / 6., across / 3., 8, 331) - 0.1);
        let k = crack * keep;
        let ember = mix(GLOW_LO, GLOW_HI, (k * 1.1 + heat * 0.45).clamp(0., 1.));
        c = mix(c, ember, (k * 1.0).clamp(0., 1.));
        // The rock itself runs hot toward the edge, a deep red under the black.
        c = add(c, [120, 34, 10], heat.powf(1.4) * 0.55);
        // The edge itself: a rim of heat, orange on the rock.
        let rim = (1. - ed / 1.1).clamp(0., 1.);
        c = add(c, [190, 80, 24], rim * 0.7 * (0.6 + 0.4 * pvalue(along / 2., across / 0.5, 16, 341)));
        // The pool of a pylon's fire: where each of the four stands, on the side it stands.
        for k in 0..4 {
            let side_left = k % 2 == 0;
            let pa = (along - 14. * k as f32 + 28.).rem_euclid(56.);
            let da = if pa > 28. { pa - 56. } else { pa };
            let de = if side_left { (1. - f) * WIDE } else { f * WIDE };
            let g = (-((da / 4.5).powi(2) + (de / 3.4).powi(2))).exp();
            c = add(c, [150, 70, 22], g * 0.9);
        }
        // Plate joints, last: black, seven metres apart.
        let jd = along.rem_euclid(7.);
        if jd < 0.2 || jd > 6.85 { c = mix(c, JOINT, 0.92); }
        c
    })
}

/// Nearest two feature points of a lattice of `n` by `n` cells of `cell` metres that closes both
/// ways (a table of `n * cell` metres a side): the distances to them.
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

/// The ground beside the Foundry Run's road, over eight units (32 m) of world: black cinder with
/// fissures of molten rock that open here and there, a few pools. Tiled over the world.
pub fn cinder_ground() -> Vec<Rgb> {
    table(|u, v| {
        let (x, y) = (u * 4., v * 4.);
        let mut c = mulc([30, 15, 10], 0.7 + 0.7 * pvalue_both(x, y, 4., 701));
        let (f1, f2) = voronoi_wrap(x, y, 4., 8, 711);
        // The fissures run in patches, wide where they run.
        let live = smoothstep(0.38, 0.6, pvalue_both(x, y, 8., 721));
        let width = 0.1 + 0.3 * live;
        let crack = (1. - smoothstep(0.0, width, f2 - f1)) * live.max(0.12);
        c = mix(c, mix([160, 40, 10], [255, 150, 50], crack), crack.min(1.) * 0.95);
        // Pools: round, a few, molten.
        let pool = smoothstep(0.7, 0.74, pvalue_both(x, y, 8., 731) * 0.7 + 0.3 * pvalue_both(x, y, 4., 732));
        c = mix(c, mix([200, 70, 18], [255, 170, 60], pvalue_both(x, y, 2., 741)), pool * 0.9);
        c
    })
}

/// Value noise in cells of `cell` metres that closes both ways over a 32 m table.
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
