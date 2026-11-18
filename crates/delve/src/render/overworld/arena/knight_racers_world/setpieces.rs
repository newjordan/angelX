//! The set pieces the cup's roads are known by: brick arches to dive under, obelisks with their pennants
//! along a sweeping bend, banners hung down the walls of a chute, the ribs of something fallen arched over
//! the verge, and marker poles with cold lanterns over a wash. Each is built a stretch at a time out of
//! the place's own materials (the `Style` roles), so the same arch is brick at the Foundry and frosted
//! stone on the wash.
use super::geo::{Fr, Soup};
use super::palette::*;
use super::pieces::Ctx;
use super::scenery::{sides, sink};
use std::f32::consts::PI;

const ARCH_EVERY: f32 = 24.;

pub fn forge_arches(g: &mut Soup, c: &Ctx) {
    let n = ((c.len() / ARCH_EVERY).round() as i32).max(1);
    for k in 0..n {
        let s = c.p.s0 + (k as f32 + 0.5) * c.len() / n as f32;
        if !c.has(s) { continue; }
        let row = c.row(s);
        let fr = Fr::on(&row, 0.);
        let w = row.wl.max(row.wr) + 1.4;
        arch(g, c, &fr, w, row.h, k);
    }
}

fn arch(g: &mut Soup, c: &Ctx, fr: &Fr, w: f32, road_h: f32, _k: i32) {
    let st = &c.st;
    let (th, pier) = (1.25f32, 2.6f32);
    let (spring, rise, ring) = (7.2f32, 6.4f32, 1.3f32);
    let top = spring + rise + ring + 0.4;
    for sd in [-1.0f32, 1.0] {
        let down = sink(c, fr.pt(0., sd * (w + pier * 0.5), 0.), road_h);
        let (la, lb) = if sd > 0. { (w, w + pier) } else { (-(w + pier), -w) };
        g.bx_open(fr, (-th, th), (la, lb), (-down, spring), st.masonry);
        g.bx(fr, (-th - 0.25, th + 0.25), (la - 0.3, lb + 0.3), (spring - 0.45, spring), st.rail_cap);
        g.bx(fr, (-0.2, 0.2), (sd * (w - 0.1) - 0.2, sd * (w - 0.1) + 0.2), (3.2, 3.9), st.glow);
    }
    let steps = 14;
    let intr = |t: f32| { let a = PI * t; (-w * a.cos(), spring + rise * a.sin()) };
    let extr = |t: f32| { let a = PI * t; (-(w + ring) * a.cos(), spring + (rise + ring) * a.sin()) };
    g.sweep(fr, (-th, th), steps, intr, st.masonry);
    for face in [-th, th] {
        for i in 0..steps {
            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            let (a0, a1) = (intr(t0), intr(t1));
            g.wall(fr.pt(face, a0.0, a0.1), fr.pt(face, a1.0, a1.1), fr.pt(face, a1.0, top), fr.pt(face, a0.0, top), st.masonry);
        }
        for sd in [-1.0f32, 1.0] {
            let (l0, l1) = (sd * w, sd * (w + pier));
            g.wall(fr.pt(face, l0, spring), fr.pt(face, l1, spring), fr.pt(face, l1, top), fr.pt(face, l0, top), st.masonry);
        }
    }
    for face in [-th, th] {
        let o = if face < 0. { -0.12 } else { 0.12 };
        for i in 0..steps {
            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            let (a0, a1) = (intr(t0), intr(t1));
            let (b0, b1) = (extr(t0), extr(t1));
            g.quad(fr.pt(face + o, a0.0, a0.1), fr.pt(face + o, a1.0, a1.1), fr.pt(face + o, b1.0, b1.1), fr.pt(face + o, b0.0, b0.1), st.rail_cap);
        }
    }
    let wide = w + pier + 0.3;
    g.bx(fr, (-th - 0.3, th + 0.3), (-wide, wide), (top, top + 0.55), st.rail_cap);
    g.bx(fr, (-th + 0.1, th - 0.1), (-w * 0.78, w * 0.78), (top + 0.55, top + 3.5), st.masonry);
    g.bx(fr, (-th, th), (-w * 0.78 - 0.3, w * 0.78 + 0.3), (top + 3.5, top + 3.85), st.rail_cap);
    let f = -th + 0.1 - 0.04;
    let (lo, hi) = (top + 0.75, top + 3.3);
    let bw = w * 0.7;
    g.quad(fr.pt(f, -bw, hi), fr.pt(f, bw, hi), fr.pt(f, bw, lo), fr.pt(f, -bw, lo), st.banner[0]);
    let f2 = f - 0.02;
    g.quad(fr.pt(f2, -bw, hi), fr.pt(f2, bw, hi), fr.pt(f2, bw, hi - 0.32), fr.pt(f2, -bw, hi - 0.32), st.banner[1]);
    g.quad(fr.pt(f2, -bw, lo + 0.32), fr.pt(f2, bw, lo + 0.32), fr.pt(f2, bw, lo), fr.pt(f2, -bw, lo), st.banner[1]);
    let mid = (lo + hi) * 0.5;
    for sd in [-1.0f32, 1.0] {
        g.tri(fr.pt(f2, sd * bw * 0.04, mid + 0.55), fr.pt(f2, sd * bw * 0.4, mid), fr.pt(f2, sd * bw * 0.04, mid - 0.55), st.banner[1]);
    }
}

pub fn obelisks(g: &mut Soup, c: &Ctx) {
    let step = 12.5;
    let n = (c.len() / step).floor() as i32;
    for sd in sides(c) {
        for k in 0..=n {
            let s = c.p.s0 + (k as f32 + if sd > 0. { 0.15 } else { 0.65 }) * step;
            if s > c.p.s1 || !c.has(s) { continue; }
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let l = sd * (row.half(sd) + 3.2 + 1.4 * c.rnd(k, 61 + sd as i32));
            let h = 4.8 + 4.6 * c.rnd(k, 63 + sd as i32);
            let down = sink(c, fr.pt(0., l, 0.), row.h);
            let (w, st) = (0.78 + 0.2 * c.rnd(k, 65), &c.st);
            g.bx_open(&fr, (-w * 1.5, w * 1.5), (l - w * 1.5, l + w * 1.5), (-down, 0.55), st.masonry);
            g.bx(&fr, (-w * 1.25, w * 1.25), (l - w * 1.25, l + w * 1.25), (0.55, 0.85), st.rail_cap);
            let w2 = w * 0.56;
            let corner = |a: f32, b: f32, hw: f32, z: f32| fr.pt(a * hw, l + b * hw, z);
            let cs = [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)];
            for i in 0..4 {
                let (p, q) = (cs[i], cs[(i + 1) % 4]);
                g.wall(corner(p.0, p.1, w, 0.85), corner(q.0, q.1, w, 0.85), corner(q.0, q.1, w2, h), corner(p.0, p.1, w2, h), st.masonry);
                g.tri(corner(p.0, p.1, w2, h), corner(q.0, q.1, w2, h), fr.pt(0., l, h + w2 * 2.6), st.rail_cap);
            }
            let (top, pole) = (h + w2 * 2.6, 1.7);
            g.bx(&fr, (-0.04, 0.04), (l - 0.04, l + 0.04), (top, top + pole), st.post);
            let cloth = if k % 2 == 0 { st.cloth } else { st.cloth_b };
            g.tri(fr.pt(0., l, top + pole), fr.pt(0., l - sd * 1.7, top + pole - 0.38), fr.pt(0., l, top + pole - 0.78), cloth);
        }
    }
}

pub fn banners(g: &mut Soup, c: &Ctx) {
    let step = 5.4;
    let n = ((c.len() / step).floor() as i32).max(1);
    let (lo, hi) = (c.win.0.max(c.p.s0), c.win.1.min(c.p.s1));
    let (face, wall_h) = (1.3f32, 9.0f32);
    let st = &c.st;
    for sd in sides(c) {
        if hi > lo {
            let m = (((hi - lo) / 3.).ceil() as i32).max(1);
            let rows: Vec<_> = (0..=m).map(|i| c.row(lo + (hi - lo) * i as f32 / m as f32)).collect();
            for w in rows.windows(2) {
                let e = |row: &super::frame::Row, d: f32, u: f32| row.pt(sd * (row.half(sd) + d), u);
                let (a, b) = (w[0], w[1]);
                g.wall(e(&a, face, -2.5), e(&b, face, -2.5), e(&b, face, wall_h), e(&a, face, wall_h), st.masonry);
                g.quad(e(&a, face - 0.3, wall_h + 0.35), e(&b, face - 0.3, wall_h + 0.35), e(&b, face + 0.9, wall_h + 0.35), e(&a, face + 0.9, wall_h + 0.35), st.rail_cap);
                g.quad(e(&a, face - 0.3, wall_h), e(&b, face - 0.3, wall_h), e(&b, face - 0.3, wall_h + 0.35), e(&a, face - 0.3, wall_h + 0.35), st.rail_cap);
            }
        }
        for k in 0..=n {
            let sp = c.p.s0 + k as f32 * c.len() / n as f32;
            if c.has(sp) {
                let row = c.row(sp);
                let fr = Fr::on(&row, 0.);
                let l = sd * (row.half(sd) + face);
                g.bx_open(&fr, (-0.35, 0.35), (l.min(l - sd * 0.4) - 0.05, l.max(l - sd * 0.4) + 0.05), (-2.5, wall_h + 0.2), st.rail);
                g.bx(&fr, (-0.45, 0.45), (l.min(l - sd * 0.5), l.max(l - sd * 0.5)), (wall_h + 0.2, wall_h + 0.7), st.rail_cap);
            }
            if k == n { continue; }
            let s = c.p.s0 + (k as f32 + 0.5) * c.len() / n as f32;
            if !c.has(s) { continue; }
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let half = row.half(sd);
            let (top, bot) = (8.0 + 0.3 * c.rnd(k, 71), 2.6 + 0.4 * c.rnd(k, 72));
            let l = sd * (half + face - 0.06);
            let hw = 1.35;
            g.wall(fr.pt(-hw, l, top), fr.pt(hw, l, top), fr.pt(hw, l, bot), fr.pt(-hw, l, bot), st.banner[0]);
            let l2 = l - sd * 0.02;
            let stripe = if k % 2 == 0 { st.banner[1] } else { STEEL };
            g.quad(fr.pt(-0.28, l2, top - 0.35), fr.pt(0.28, l2, top - 0.35), fr.pt(0.28, l2, bot + 0.5), fr.pt(-0.28, l2, bot + 0.5), stripe);
            g.quad(fr.pt(-hw, l2, bot + 0.5), fr.pt(hw, l2, bot + 0.5), fr.pt(hw, l2, bot), fr.pt(-hw, l2, bot), st.banner[1]);
            g.bx(&fr, (-0.08, 0.08), (l - sd * 0.12 - 0.08, l - sd * 0.12 + 0.08), (top, top + 0.18), st.post);
            g.bx(&fr, (-hw - 0.2, hw + 0.2), (l - sd * 0.06 - 0.06, l - sd * 0.06 + 0.06), (top + 0.18, top + 0.3), st.post);
            g.tri(fr.pt(-hw, l2, bot), fr.pt(0., l2, bot), fr.pt(-hw * 0.5, l2, bot - 0.7), st.banner[1]);
            g.tri(fr.pt(0., l2, bot), fr.pt(hw, l2, bot), fr.pt(hw * 0.5, l2, bot - 0.7), st.banner[1]);
        }
    }
}

pub fn ribs(g: &mut Soup, c: &Ctx) {
    let step = 6.5;
    let n = ((c.len() / step).floor() as i32).max(2);
    for sd in sides(c) {
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32;
            let s = c.p.s0 + t * c.len();
            if !c.has(s) { continue; }
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let l0 = sd * (row.half(sd) + 9.0 + 1.2 * c.rnd(k, 81));
            let body = (PI * t).sin().max(0.2);
            let foot = c.ground(fr.pt(0., l0, 0.));
            let a = (c.rnd(k, 82) - 0.5) * 0.8;
            let z0 = foot - row.h;
            g.bx_open(&fr, (a - 0.42, a + 0.42), (l0 - 0.4, l0 + 0.4), (z0 - 0.6, z0 + 0.85), WA_BONE);
            g.spike(fr.pt(a, l0, z0 + 0.85), fr.pt(a - 0.4, l0, z0 + 1.7), 0.18, WA_BONE);
            for dir in [-1.0f32, 1.0] {
                if dir * sd > 0. && k % 2 == 1 { continue; }
                let reach = (3.6 + 1.4 * c.rnd(k, 83 + dir as i32)) * (0.7 + 0.3 * body);
                let high = (3.4 + 3.4 * body) * (0.85 + 0.3 * c.rnd(k, 85));
                let pt = |u: f32| {
                    let (p0, p1, p2) = ((0., 0.7), (dir * reach * 0.25, high * 1.6), (dir * reach, -0.2));
                    let m = 1. - u;
                    let (x, z) = (m * m * p0.0 + 2. * m * u * p1.0 + u * u * p2.0, m * m * p0.1 + 2. * m * u * p1.1 + u * u * p2.1);
                    fr.pt(a + 0.1 * dir * u, l0 + sd * x, z0 + z)
                };
                let segs = 4;
                for i in 0..segs {
                    let (u0, u1) = (i as f32 / segs as f32, (i + 1) as f32 / segs as f32);
                    g.limb_n(pt(u0), pt(u1), 0.6 * (1. - 0.8 * u0), 0.6 * (1. - 0.8 * u1), 3, WA_BONE);
                }
            }
        }
        let s = c.p.s1 - 50.;
        if c.has(s) {
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let l = sd * (row.half(sd) + 12.);
            let z0 = (c.ground(fr.pt(0., l, 0.)) - row.h).max(-3.);
            g.bx_open(&fr, (-1.8, 2.8), (l - 1.1, l + 1.1), (z0 - 1.0, z0 + 1.7), WA_BONE);
            g.bx(&fr, (-0.6, 2.0), (l - 0.9, l + 0.9), (z0 + 1.7, z0 + 2.7), WA_BONE);
            g.bx(&fr, (2.8, 4.9), (l - 0.5, l + 0.5), (z0 + 0.3, z0 + 1.3), WA_BONE);
            for dl in [-0.55f32, 0.55] {
                g.quad(fr.pt(2.0 - 0.01, l + dl - 0.28, z0 + 2.4), fr.pt(2.0 - 0.01, l + dl + 0.28, z0 + 2.4), fr.pt(2.0 - 0.01, l + dl + 0.28, z0 + 1.75), fr.pt(2.0 - 0.01, l + dl - 0.28, z0 + 1.75), WA_POST);
                g.limb(fr.pt(0.4, l + dl * 1.5, z0 + 2.6), fr.pt(-0.6, l + dl * 3.2, z0 + 4.6), 0.34, 0.08, WA_BONE);
            }
        }
    }
}

pub fn pole_lanterns(g: &mut Soup, c: &Ctx) {
    let step = 19.;
    let n = (c.len() / step).floor() as i32;
    for sd in sides(c) {
        for k in 0..=n {
            let s = c.p.s0 + (k as f32 + if sd > 0. { 0.2 } else { 0.7 }) * step;
            if s > c.p.s1 || !c.has(s) { continue; }
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let st = &c.st;
            let l = sd * (row.half(sd) + 1.5);
            let down = sink(c, fr.pt(0., l, 0.), row.h);
            let h = 5.2 + 0.6 * c.rnd(k, 91);
            g.bx_open(&fr, (-0.15, 0.15), (l - 0.15, l + 0.15), (-down, h), st.post);
            g.spike(fr.pt(0., l, h), fr.pt(0., l, h + 1.0), 0.2, st.rail_cap);
            let lx = l - sd * 1.35;
            g.limb(fr.pt(0., l, h - 0.45), fr.pt(0., lx, h - 0.45), 0.07, 0.05, st.post);
            g.bx(&fr, (-0.3, 0.3), (lx - 0.3, lx + 0.3), (h - 1.9, h - 0.8), st.glow);
            g.spike(fr.pt(0., lx, h - 0.8), fr.pt(0., lx, h - 0.4), 0.3, st.rail_cap);
            let cloth = if k % 2 == 0 { st.cloth } else { st.cloth_b };
            g.tri(fr.pt(0., l, h + 0.9), fr.pt(0., l + sd * 1.3, h + 0.65), fr.pt(0., l, h + 0.3), cloth);
        }
    }
}

