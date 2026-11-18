//! The pit lane: along one rail of a straight, a low wall with a paved lane behind it, bannered awnings
//! over bays of fuel drums, and the squires of each crew standing under them, one hand up, waving the
//! next pod past. It stands on every course's opening straight, in whatever that place is made of
//! (the `Style` roles: its masonry, its cloth, its lamps), so a pit lane at the Foundry is a foundry's
//! and a pit lane on the wash is frost and cold lanterns.
//!
//! What stands still is built once, a stretch at a time, with the other set pieces (`lane`). The waving
//! arms are the one thing that moves, and are built fresh each frame from the race clock (`arms`),
//! only for the bays in sight.
use super::frame::{Row, row_on};
use super::geo::{Fr, Soup};
use super::look::style;
use super::noise::hash;
use super::palette::*;
use super::pieces::Ctx;
use super::route::{Kind, Piece, Route};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Tri;

const BAY: f32 = 24.;
const WALL: f32 = 2.8;
const DEEP: f32 = 7.0;
const STEP: f32 = 8.;
const CREW: [f32; 2] = [-2.6, 2.3];
const CREW_BACK: f32 = 1.7;
const SCALE: f32 = 1.25;

fn side(p: &Piece) -> f32 { if p.side < 0. { -1. } else { 1. } }

fn bay_s(p: &Piece, k: i32) -> f32 { p.s0 + (k as f32 + 0.5) * BAY }

fn bays(p: &Piece) -> i32 { (((p.s1 - p.s0) / BAY).floor() as i32).max(1) }

fn squire(r: &dyn Route, line: u8, p: &Piece, k: i32, i: usize) -> (Fr, f32, f32) {
    let sd = side(p);
    let row = row_on(r, line, bay_s(p, k));
    let at = Fr::on(&row, 0.);
    let l = sd * (row.half(sd) + WALL + CREW_BACK);
    let o = at.o + at.f * (CREW[i] + 0.35 * (hash(k, i as i32, p.seed) - 0.5)) + at.l * l;
    (Fr { o, f: at.f, l: at.l }, sd, hash(k, i as i32 + 7, p.seed))
}

fn limb(g: &mut Soup, a: V3, b: V3, r0: f32, r1: f32, mat: u8) { g.limb(a, b, r0, r1, mat); }

pub fn lane(g: &mut Soup, c: &Ctx) {
    let sd = side(&c.p);
    let (lo, hi) = (c.win.0.max(c.p.s0), c.win.1.min(c.p.s1));
    if hi <= lo { return; }
    let st = &c.st;
    let n = (((hi - lo) / STEP).ceil() as i32).max(1);
    let rows: Vec<Row> = (0..=n).map(|i| c.row(lo + (hi - lo) * i as f32 / n as f32)).collect();
    for w in rows.windows(2) {
        let (a, b) = (w[0], w[1]);
        let e = |row: &Row, d: f32, u: f32| row.pt(sd * (row.half(sd) + d), u);
        g.quad(e(&a, 0.7, 0.04), e(&b, 0.7, 0.04), e(&b, DEEP, 0.04), e(&a, DEEP, 0.04), st.plate_b);
        g.quad(e(&a, DEEP, 0.04), e(&b, DEEP, 0.04), e(&b, DEEP, -2.2), e(&a, DEEP, -2.2), st.plate_b);
        let (t0, t1) = (0.95, 1.05);
        g.quad(e(&a, WALL, 0.04), e(&b, WALL, 0.04), e(&b, WALL, t0), e(&a, WALL, t0), st.masonry);
        g.quad(e(&a, WALL, t1), e(&b, WALL, t1), e(&b, WALL + 0.34, t1), e(&a, WALL + 0.34, t1), st.rail_cap);
        g.quad(e(&a, WALL, t0), e(&b, WALL, t0), e(&b, WALL, t1), e(&a, WALL, t1), st.rail_cap);
        g.quad(e(&a, WALL + 0.34, t1), e(&b, WALL + 0.34, t1), e(&b, WALL + 0.34, 0.04), e(&a, WALL + 0.34, 0.04), st.masonry);
        if (a.s / STEP).round() as i32 % 2 == 0 {
            g.quad(e(&a, WALL - 0.01, 0.04), e(&b, WALL - 0.01, 0.04), e(&b, WALL - 0.01, 0.3), e(&a, WALL - 0.01, 0.3), st.warn);
        }
    }
    for (s, here) in [(c.p.s0, c.win.0.is_infinite()), (c.p.s1, c.win.1.is_infinite())] {
        if !here { continue; }
        let row = c.row(s);
        let fr = Fr::on(&row, 0.);
        let l = sd * (row.half(sd) + WALL + 0.17);
        g.bx_open(&fr, (-0.3, 0.3), (l - 0.3, l + 0.3), (-1.5, 3.6), st.masonry);
        g.bx(&fr, (-0.4, 0.4), (l - 0.4, l + 0.4), (3.6, 3.85), st.rail_cap);
        g.bx(&fr, (-0.2, 0.2), (l - 0.2, l + 0.2), (3.85, 4.3), st.glow);
        let tip = l - sd * 1.6;
        g.tri(fr.pt(0., l - sd * 0.3, 3.4), fr.pt(0., tip, 3.0), fr.pt(0., l - sd * 0.3, 2.6), st.cloth);
    }
    for k in 0..bays(&c.p) {
        let sb = bay_s(&c.p, k);
        if !c.has(sb) { continue; }
        let row = c.row(sb);
        let fr = Fr::on(&row, 0.);
        bay(g, c, &fr, &row, sd, k);
        for i in 0..2 { squire_body(g, c, k, i); }
    }
}

fn bay(g: &mut Soup, c: &Ctx, fr: &Fr, row: &Row, sd: f32, k: i32) {
    let st = &c.st;
    let (d0, d1) = (WALL + 0.7, WALL + 4.4);
    let at = |d: f32| sd * (row.half(sd) + d);
    let (l0, l1) = (at(d0), at(d1));
    let half_a = 4.4;
    let (z0, z1) = (3.1f32, 3.8f32);
    for a in [-half_a, half_a] {
        g.bx_open(fr, (a - 0.1, a + 0.1), (l0 - 0.1, l0 + 0.1), (-0.5, z0), st.timber);
        g.bx_open(fr, (a - 0.1, a + 0.1), (l1 - 0.1, l1 + 0.1), (-0.5, z1), st.timber);
        g.quad(fr.pt(a - 0.2, l0 - sd * 0.22, z0 + 0.5), fr.pt(a + 0.2, l0 - sd * 0.22, z0 + 0.5), fr.pt(a + 0.2, l0 - sd * 0.22, z0 + 0.05), fr.pt(a - 0.2, l0 - sd * 0.22, z0 + 0.05), st.glow);
    }
    g.wall(fr.pt(-half_a, l1, 0.04), fr.pt(half_a, l1, 0.04), fr.pt(half_a, l1, z1), fr.pt(-half_a, l1, z1), st.slate);
    let strips = 4;
    for j in 0..strips {
        let (a0, a1) = (-half_a - 0.3 + (half_a * 2. + 0.6) * j as f32 / strips as f32, -half_a - 0.3 + (half_a * 2. + 0.6) * (j + 1) as f32 / strips as f32);
        let (m, mv) = if (j + k) % 2 == 0 { (st.cloth, st.cloth_b) } else { (st.cloth_b, st.cloth) };
        g.quad(fr.pt(a0, l0 - sd * 0.25, z0), fr.pt(a1, l0 - sd * 0.25, z0), fr.pt(a1, l1 + sd * 0.25, z1), fr.pt(a0, l1 + sd * 0.25, z1), m);
        let am = (a0 + a1) * 0.5;
        g.tri(fr.pt(a0, l0 - sd * 0.25, z0), fr.pt(a1, l0 - sd * 0.25, z0), fr.pt(am, l0 - sd * 0.25, z0 - 0.55), mv);
    }
    let (b0, b1) = (-half_a + 0.9, half_a - 0.9);
    let bl = l0 - sd * 0.3;
    g.wall(fr.pt(b0, bl, z0 + 0.15), fr.pt(b1, bl, z0 + 0.15), fr.pt(b1, bl, z0 + 0.95), fr.pt(b0, bl, z0 + 0.95), st.plate_b);
    let bl2 = bl - sd * 0.02;
    g.quad(fr.pt(b0 + 0.25, bl2, z0 + 0.3), fr.pt(b1 - 0.25, bl2, z0 + 0.3), fr.pt(b1 - 0.25, bl2, z0 + 0.46), fr.pt(b0 + 0.25, bl2, z0 + 0.46), if k % 2 == 0 { st.glow } else { st.hot });
    let wl = at(WALL - 0.02);
    g.wall(fr.pt(-2.4, wl, 0.9), fr.pt(2.4, wl, 0.9), fr.pt(2.4, wl, 0.2), fr.pt(-2.4, wl, 0.2), if k % 2 == 0 { st.cloth_b } else { st.cloth });
    g.quad(fr.pt(-2.4, wl - sd * 0.01, 0.62), fr.pt(2.4, wl - sd * 0.01, 0.62), fr.pt(2.4, wl - sd * 0.01, 0.5), fr.pt(-2.4, wl - sd * 0.01, 0.5), HAZARD_GOLD);
    for (a, d, lvl) in [(-6.9f32, 0.9f32, 0.0f32), (-7.9, 1.1, 0.0), (7.1, 0.9, 0.0), (7.1, 0.95, 0.99)] {
        drum(g, st, fr.pt(a, at(d0 + d), 0.04 + lvl));
    }
}

fn drum(g: &mut Soup, st: &super::look::Style, base: V3) {
    g.column(base, 0.95, 0.36, 0.36, 6, st.plate_b);
    g.column(base + v3(0., 0., 0.5), 0.2, 0.375, 0.375, 6, st.warn);
}

fn squire_body(g: &mut Soup, c: &Ctx, k: i32, i: usize) {
    let st = &c.st;
    let (fr, sd, _) = squire(c.r, c.line, &c.p, k, i);
    let sc = SCALE;
    let tab = if (k + i as i32) % 2 == 0 { st.cloth } else { st.cloth_b };
    let z = |v: f32| v * sc;
    g.bx_open(&fr, (-0.22, 0.22), (-0.11, 0.11), (-0.05, z(0.76)), st.post);
    g.bx(&fr, (-0.26, 0.26), (-0.16, 0.16), (z(0.74), z(1.42)), STEEL);
    g.quad(fr.pt(-0.2, -sd * 0.17, z(1.38)), fr.pt(0.2, -sd * 0.17, z(1.38)), fr.pt(0.2, -sd * 0.17, z(0.5)), fr.pt(-0.2, -sd * 0.17, z(0.5)), tab);
    limb(g, fr.pt(-0.3, 0., z(1.36)), fr.pt(-0.33, -sd * 0.04, z(0.78)), 0.075, 0.06, STEEL);
    g.bx(&fr, (-0.15, 0.15), (-0.16, 0.16), (z(1.44), z(1.74)), STEEL);
    g.quad(fr.pt(-0.12, -sd * 0.165, z(1.6)), fr.pt(0.12, -sd * 0.165, z(1.6)), fr.pt(0.12, -sd * 0.165, z(1.67)), fr.pt(-0.12, -sd * 0.165, z(1.67)), st.post);
    g.spike(fr.pt(0., 0., z(1.74)), fr.pt(-0.12, 0., z(2.05)), 0.07, tab);
}

pub fn arms(out: &mut Vec<Tri>, r: &dyn Route, s: f32, ticks: u32) {
    let lap = r.lap();
    let mut g = Soup::new();
    for p in r.pieces().iter().filter(|p| p.kind == Kind::PitLane) {
        for o in [-lap, 0., lap] {
            if p.s1 + o < s - 70. || p.s0 + o > s + 300. { continue; }
            let st = style(r.zone((p.s0 + p.s1) * 0.5).0);
            for k in 0..bays(p) {
                let sb = bay_s(p, k) + o;
                if sb < s - 70. || sb > s + 300. { continue; }
                for i in 0..2 {
                    let (fr, sd, phase) = squire(r, 0, p, k, i);
                    let w = ticks as f32 * (0.26 + 0.05 * phase) + phase * std::f32::consts::TAU;
                    let (sw, cw) = w.sin_cos();
                    let sc = SCALE;
                    let sh = fr.pt(0.3, 0., 1.36 * sc);
                    let el = sh + fr.f * 0.1 + fr.l * (-sd * 0.2) + v3(0., 0., 0.3 * sc);
                    let hand = el + fr.f * (0.5 * sw) + fr.l * (-sd * 0.06) + v3(0., 0., 0.4 * sc * (0.85 + 0.15 * cw));
                    limb(&mut g, sh, el, 0.075, 0.065, STEEL);
                    limb(&mut g, el, hand, 0.065, 0.055, STEEL);
                    let top = hand + v3(0., 0., 0.62);
                    limb(&mut g, hand, top, 0.022, 0.018, st.post);
                    let trail = fr.f * (-0.7 * cw.signum() * (0.5 + 0.5 * cw.abs()));
                    let cloth = if (k + i as i32) % 2 == 0 { st.cloth_b } else { st.cloth };
                    g.tri(top, top - v3(0., 0., 0.3) + trail * 0.2, top + trail + v3(0., 0., -0.14), cloth);
                }
            }
        }
    }
    out.extend(g.t);
}
