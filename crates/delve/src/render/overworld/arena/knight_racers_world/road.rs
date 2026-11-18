//! The racing surface, its kerbs and its barriers, built in blocks of road.
use super::cache::{self, Key, Tris};
use super::frame::{Row, row_on};
use super::geo::Soup;
use super::look::{Style, style};
use super::route::{Gap, Route};
use crate::render::world3d::math::v3;

pub const BLOCK: f32 = 32.;
const GORE: f32 = 40.;

pub fn blocks(r: &dyn Route, line: u8) -> (i32, f32, f32) {
    let (s0, s1) = match line.checked_sub(1) { Some(i) => { let f = &r.forks()[i as usize]; (f.s0, f.s1) } None => (0., r.lap()) };
    let n = ((s1 - s0) / BLOCK).round().max(1.) as i32;
    (n, s0, (s1 - s0) / n as f32)
}

fn rows_per_block(lod: u8) -> usize { [16, 8, 4][lod.min(2) as usize] }

pub fn block(r: &dyn Route, line: u8, k: i32, lod: u8) -> Tris {
    let (n, s0, len) = blocks(r, line);
    let k = if line == 0 { k.rem_euclid(n) } else { k.clamp(0, n - 1) };
    cache::get(Key { course: r.key(), kind: 1, a: k, b: line as i32, lod }, || build(r, line, s0 + k as f32 * len, len, lod))
}

fn open_air(gaps: &[Gap], s: f32) -> bool { gaps.iter().any(|g| s >= g.s0 && s <= g.s1) }

fn open_side(r: &dyn Route, line: u8, s: f32, sd: f32) -> bool {
    let near = |f: &super::route::ForkInfo| (s - f.s0).abs() < GORE * 0.6 + 4. && s < f.s0 + GORE || (s - f.s1).abs() < GORE * 0.6 + 4. && s > f.s1 - GORE;
    match line.checked_sub(1) {
        None => r.forks().iter().any(|f| f.side == sd && near(f)),
        Some(i) => { let f = &r.forks()[i as usize]; f.side == -sd && near(f) }
    }
}

fn build(r: &dyn Route, line: u8, s0: f32, len: f32, lod: u8) -> Vec<crate::render::world3d::mesh::Tri> {
    let mut g = Soup::new();
    let gaps = if line == 0 { r.gaps() } else { Vec::new() };
    let n = rows_per_block(lod);
    let strips = if lod == 0 { 6 } else { 4 };
    let rows: Vec<Row> = (0..=n).map(|i| row_on(r, line, s0 + len * i as f32 / n as f32)).collect();
    let lift = if line == 0 { 0. } else { 0.04 };
    let lamp_row = |i: usize| -> bool {
        let Some(row) = rows.get(i) else { return false };
        let st = style(r.zone(row.s).0);
        st.lamp_every > 0. && st.beat != 1 && i % (if lod == 0 { 4 } else { 2 }) == 0 && ((row.s / 8.).round() as i32) % ((st.lamp_every / 8.) as i32).max(1) == 0
    };
    for (i, w) in rows.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let mid = (a.s + b.s) * 0.5;
        if open_air(&gaps, mid) { continue; }
        let look = match line.checked_sub(1) { Some(f) => r.forks()[f as usize].look, None => r.zone(mid).0 };
        let st = style(look);
        let check = (mid / 3.).floor() as i32;
        let lat = |row: &Row, k: usize| -row.wr + (row.wl + row.wr) * k as f32 / strips as f32;
        let pooled = lod == 0 && (lamp_row(i) || lamp_row(i + 1));
        let hot = st.plated && r.sink(mid) > 0.;
        let plate = (mid / 8.).floor() as i32;
        for j in 0..strips {
            let (y0a, y1a) = (lat(&a, j), lat(&a, j + 1));
            let (y0b, y1b) = (lat(&b, j), lat(&b, j + 1));
            let edge = j == 0 || j + 1 == strips;
            if st.road_along > 0. {
                let uv = |row: &Row, k: usize| [row.s / st.road_along, 8. * k as f32 / strips as f32];
                g.quad_uv(a.pt(y0a, lift), b.pt(y0b, lift), b.pt(y1b, lift), a.pt(y1a, lift), [uv(&a, j), uv(&b, j), uv(&b, j + 1), uv(&a, j + 1)], st.road[hot as usize]);
                continue;
            }
            let m = if st.beat != 1 && pooled && edge { st.pool } else if st.plated {
                if hot && edge { st.road_hot } else { st.road[(plate + (j as i32 % 2)).rem_euclid(2) as usize] }
            } else if pooled && edge { st.pool } else { st.road[((j as i32 + check).rem_euclid(2)) as usize] };
            g.quad(a.pt(y0a, lift), b.pt(y0b, lift), b.pt(y1b, lift), a.pt(y1a, lift), m);
        }
        if st.plated && lod < 2 { markings(&mut g, r, line, &st, a, b, lift, lod); }
        for sd in [-1.0f32, 1.0] {
            let (ea, eb) = (sd * a.half(sd), sd * b.half(sd));
            let kc = st.kerb[(check.rem_euclid(2)) as usize];
            g.quad(a.pt(ea - sd * 1.0, 0.03 + lift), b.pt(eb - sd * 1.0, 0.03 + lift), b.pt(eb, 0.03 + lift), a.pt(ea, 0.03 + lift), kc);
            if open_side(r, line, mid, sd) { continue; }
            let top = if r.sink(mid) > 0. { 0.5 } else { 1.15 };
            let (ra, rb) = (a.pt(ea + sd * 0.1, 0.), b.pt(eb + sd * 0.1, 0.));
            let (ta, tb) = (a.pt(ea + sd * 0.1, top), b.pt(eb + sd * 0.1, top));
            g.quad(ra, rb, tb, ta, st.rail);
            let (ca, cb) = (a.pt(ea + sd * 0.7, top), b.pt(eb + sd * 0.7, top));
            g.quad(ta, tb, cb, ca, st.rail_cap);
            let (da, db) = (a.pt(ea + sd * 0.7, top - 3.2), b.pt(eb + sd * 0.7, top - 3.2));
            g.quad(ca, cb, db, da, st.rail);
            if st.beat == 1 {
                if lod == 0 { g.quad(a.pt(ea + sd * 0.09, 0.28), b.pt(eb + sd * 0.09, 0.28), b.pt(eb + sd * 0.09, 0.5), a.pt(ea + sd * 0.09, 0.5), st.lane); }
                continue;
            }
            if lod == 0 && i % 4 == 0 {
                let p = a.pt(ea + sd * 0.4, 0.);
                g.cuboid(p + v3(-0.28, -0.28, 0.), p + v3(0.28, 0.28, 2.0), st.post);
                if st.lamp_every > 0. && ((a.s / 8.).round() as i32) % ((st.lamp_every / 8.) as i32).max(1) == 0 {
                    let big = if st.beat == 3 { 1.7 } else { 1. };
                    g.cuboid(p + v3(-0.2 * big, -0.2 * big, 2.0), p + v3(0.2 * big, 0.2 * big, 2.5 + 0.35 * (big - 1.) * 2.), st.lamp);
                }
            } else if lod == 1 && lamp_row(i) {
                let p = a.pt(ea + sd * 0.4, 0.);
                g.cuboid(p + v3(-0.3, -0.3, 1.6), p + v3(0.3, 0.3, 2.4), st.lamp);
            }
        }
    }
    if rows.first().is_some_and(|f| style(r.zone(f.s).0).beat != 0) { super::trackside::along(&mut g, r, line, s0, s0 + len, lod); }
    g.t
}

fn markings(g: &mut Soup, r: &dyn Route, line: u8, st: &Style, a: Row, b: Row, lift: f32, lod: u8) {
    let up = 0.025 + lift;
    let k = if st.seam_every > 0. { (b.s / st.seam_every).floor() } else { -1. };
    let at = k * st.seam_every;
    if k >= 0. && at >= a.s && at < b.s {
        let (r0, r1) = (row_on(r, line, at), row_on(r, line, at + 0.42));
        g.quad(r0.pt(-r0.wr, up), r1.pt(-r1.wr, up), r1.pt(r1.wl, up), r0.pt(r0.wl, up), st.seam);
    }
    let dash = (((a.s + b.s) * 0.5) / 4.).floor() as i32;
    if st.lanes && lod == 0 && dash.rem_euclid(3) == 0 {
        for f in [-1.0f32, 1.0] {
            let (y0, y1) = (if f < 0. { -a.wr * 0.34 } else { a.wl * 0.34 }, if f < 0. { -b.wr * 0.34 } else { b.wl * 0.34 });
            g.quad(a.pt(y0 - 0.09, up), b.pt(y1 - 0.09, up), b.pt(y1 + 0.09, up), a.pt(y0 + 0.09, up), st.lane);
        }
    }
}
