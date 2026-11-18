//! What stands beside the road in a steady beat, so the world whips past the lens: ember pylons
//! on the barrier, chevron boards on a bend's outer side, truss arches overhead. Built into the
//! road's own blocks, so each is made once. The beat is the place's own (see `style`).
use super::frame::{Row, row_on};
use super::geo::{Fr, Soup};
use super::noise::hash;
use super::look::style;
use super::palette::*;
use super::route::{Kind, Route};
use crate::render::world3d::math::{V3, v3};

pub fn beat_of(r: &dyn Route) -> u8 { style(r.zone(0.).0).beat }

pub fn pitch(beat: u8) -> f32 { match beat { 1 => 14., 2 => 18., 3 => 20., 4 => 17., _ => 1e9 } }
pub const TRUSS: f32 = 96.;
const CHEVRON: f32 = 9.;
const BEND: f32 = 0.0065;

fn roofed(r: &dyn Route, s: f32) -> bool {
    r.pieces().iter().any(|p| matches!(p.kind, Kind::Tunnel | Kind::CrystalVault) && s >= p.s0 - 2. && s <= p.s1 + 2.)
}

fn open_air(r: &dyn Route, s: f32) -> bool { r.gaps().iter().any(|g| s >= g.s0 - 6. && s <= g.s1 + 6.) }

pub fn beat_at(r: &dyn Route, line: u8, k: i32) -> Option<(V3, Row, f32)> {
    let beat = beat_of(r);
    let s = k as f32 * pitch(beat) + if beat == 2 { 9. } else { 0. };
    if line != 0 || s < 24. || s >= r.lap() { return None; }
    if style(r.zone(s).0).beat != beat || roofed(r, s) || open_air(r, s) { return None; }
    let sd = if k % 2 == 0 { 1.0 } else { -1.0 };
    let row = row_on(r, line, s);
    Some((row.pt(sd * (row.half(sd) + 0.45), 1.15), row, sd))
}

pub fn along(g: &mut Soup, r: &dyn Route, line: u8, s0: f32, s1: f32, lod: u8) {
    if line != 0 { return; }
    let beat = beat_of(r);
    let pitch = pitch(beat);
    let off = if beat == 2 { 9. } else { 0. };
    let (k0, k1) = (((s0 - off) / pitch).ceil() as i32, ((s1 - off - 1e-3) / pitch).floor() as i32);
    for k in k0..=k1 {
        let Some((p, row, sd)) = beat_at(r, line, k) else { continue };
        match beat {
            1 => pylon(g, r, p, row, lod),
            2 => cluster(g, r, p, row, sd, k, lod),
            3 => if k % 2 == 0 { brazier(g, r, p, row, sd, lod) } else { banner(g, r, p, row, sd, k, lod) },
            4 => match k.rem_euclid(3) { 0 => cairn(g, r, p, row, sd, k, lod), 1 => reeds(g, p, row, sd, k, lod), _ => bone_spike(g, p, row, sd, k, lod) },
            _ => {}
        }
    }
    let (c0, c1) = ((s0 / CHEVRON).ceil() as i32, ((s1 - 1e-3) / CHEVRON).floor() as i32);
    for k in c0..=c1 {
        let s = k as f32 * CHEVRON;
        let curve = r.curve(s);
        if curve.abs() < BEND || lod > 1 { continue; }
        if !style(r.zone(s).0).plated || roofed(r, s) || open_air(r, s) || s < 20. { continue; }
        let sd = if curve > 0. { -1.0 } else { 1.0 };
        chevron(g, r, s, sd, curve.signum());
    }
    if beat != 1 { return; }
    let (a0, a1) = (((s0 - 40.) / TRUSS).ceil() as i32, ((s1 - 40. - 1e-3) / TRUSS).floor() as i32);
    for k in a0..=a1 {
        let s = k as f32 * TRUSS + 40.;
        if s < s0 || s >= s1 || s < 60. { continue; }
        if !style(r.zone(s).0).plated || roofed(r, s) || open_air(r, s) { continue; }
        if r.landmarks().iter().any(|l| (l - s).abs() < 24.) { continue; }
        if r.pieces().iter().any(|p| matches!(p.kind, Kind::Arch | Kind::Gantry | Kind::Bridge | Kind::Viaduct) && s >= p.s0 - 20. && s <= p.s1 + 20.) { continue; }
        truss(g, r, s);
    }
}

pub fn flame_base(r: &dyn Route, k: i32) -> Option<V3> {
    let (p, _, _) = beat_at(r, 0, k)?;
    match beat_of(r) { 1 => Some(p + v3(0., 0., 4.86)), 3 if k % 2 == 0 => Some(p + v3(0., 0., 2.05)), _ => None }
}

fn cluster(g: &mut Soup, _r: &dyn Route, p: V3, row: Row, sd: f32, k: i32, lod: u8) {
    let violet = hash(k, 7, 31) < 0.5;
    let (a, b, tip, pool) = if violet { (VIOLET_DIM, VIOLET_FACET, VIOLET_GLOW, VIOLET_POOL) } else { (CYAN_DIM, CYAN_FACET, CYAN_GLOW, CYAN_POOL) };
    let out = v3(-row.sn * sd, row.cs * sd, 0.);
    let along = v3(row.cs, row.sn, 0.);
    let base = p + v3(0., 0., -1.15);
    if lod >= 2 {
        let q = base + out * 0.8 + v3(0., 0., 2.4);
        g.cuboid(q + v3(-0.3, -0.3, 0.), q + v3(0.3, 0.3, 0.8), tip);
        return;
    }
    let n = if lod == 0 { 4 } else { 2 };
    for i in 0..n {
        let h = 2.6 + 4.2 * hash(k, i, 33) + if i == 0 { 2.4 } else { 0. };
        let w = 0.6 + 0.6 * hash(k, i, 34);
        let at = base + out * (0.9 + 1.1 * hash(k, i, 35) + i as f32 * 0.5) + along * ((i as f32 - 1.) * 1.4);
        facet(g, at, h, w, out * (0.5 + 0.6 * hash(k, i, 36)) + along * ((hash(k, i, 37) - 0.5) * 0.8), a, b, tip);
    }
    if lod == 0 {
        let c = row.pt(sd * (row.half(sd) - 1.4), 0.05);
        let (l, f) = (v3(-row.sn * sd, row.cs * sd, 0.) * 3.0, along * 3.4);
        g.quad(c - l - f, c + l - f, c + l + f, c - l + f, pool);
    }
}

pub fn facet(g: &mut Soup, base: V3, h: f32, w: f32, lean: V3, a: u8, b: u8, tip: u8) {
    let ring = |z: f32, r: f32, shift: V3, i: usize| {
        let ang = i as f32 / 4. * std::f32::consts::TAU + 0.6;
        base + shift + v3(r * ang.cos(), r * ang.sin(), z)
    };
    let top = h * 0.72;
    for i in 0..4 {
        let j = (i + 1) % 4;
        g.quad(ring(0., w, v3(0., 0., 0.), i), ring(0., w, v3(0., 0., 0.), j), ring(top, w * 0.82, lean * 0.72, j), ring(top, w * 0.82, lean * 0.72, i), if i % 2 == 0 { a } else { b });
        g.tri(ring(top, w * 0.82, lean * 0.72, i), ring(top, w * 0.82, lean * 0.72, j), base + lean + v3(0., 0., h), if i % 2 == 0 { tip } else { b });
    }
}

fn brazier(g: &mut Soup, r: &dyn Route, p: V3, row: Row, sd: f32, lod: u8) {
    let st = style(r.zone(row.s).0);
    g.cuboid(p + v3(-0.14, -0.14, -0.4), p + v3(0.14, 0.14, 1.5), st.post);
    g.cuboid(p + v3(-0.55, -0.55, 1.5), p + v3(0.55, 0.55, 1.78), st.post);
    g.cuboid(p + v3(-0.42, -0.42, 1.78), p + v3(0.42, 0.42, 2.0), TORCH_DIM);
    g.spike(p + v3(0., 0., 2.0), p + v3(0., 0., 3.0), 0.34, TORCH);
    if lod == 0 {
        let c = row.pt(sd * (row.half(sd) - 1.2), 0.045);
        let (l, f) = (v3(-row.sn * sd, row.cs * sd, 0.) * 2.0, v3(row.cs, row.sn, 0.) * 2.4);
        g.quad(c - l - f, c + l - f, c + l + f, c - l + f, st.pool);
    }
}

fn cairn(g: &mut Soup, r: &dyn Route, p: V3, row: Row, sd: f32, k: i32, lod: u8) {
    let st = style(r.zone(row.s).0);
    let fr = Fr { o: p, f: v3(row.cs, row.sn, 0.), l: v3(-row.sn, row.cs, 0.) };
    let c = sd * (0.7 + 0.25 * hash(k, 3, 41));
    if lod >= 2 {
        g.bx(&fr, (-0.2, 0.2), (c - 0.2, c + 0.2), (1.6, 2.2), st.glow);
        return;
    }
    let j = |i: i32| (hash(k, i, 42) - 0.5) * 0.3;
    g.bx_open(&fr, (-0.8 + j(1), 0.8 + j(1)), (c - 0.72, c + 0.72), (-1.5, 0.55), st.masonry);
    g.bx(&fr, (-0.55 + j(2), 0.55 + j(2)), (c + j(3) - 0.52, c + j(3) + 0.52), (0.55, 1.25), st.rail);
    g.bx(&fr, (-0.36 + j(4), 0.36 + j(4)), (c + j(5) - 0.34, c + j(5) + 0.34), (1.25, 1.75), st.masonry);
    g.bx(&fr, (-0.3, 0.3), (c - 0.3, c + 0.3), (1.75, 1.83), st.rail_cap);
    g.bx(&fr, (-0.17, 0.17), (c - 0.17, c + 0.17), (1.83, 2.25), st.glow);
    g.bx(&fr, (-0.22, 0.22), (c - 0.22, c + 0.22), (2.25, 2.33), st.rail_cap);
    if lod == 0 {
        let at = row.pt(sd * (row.half(sd) - 1.2), 0.045);
        let (l, f) = (v3(-row.sn * sd, row.cs * sd, 0.) * 2.0, v3(row.cs, row.sn, 0.) * 2.4);
        g.quad(at - l - f, at + l - f, at + l + f, at - l + f, st.pool);
    }
}

fn reeds(g: &mut Soup, p: V3, row: Row, sd: f32, k: i32, lod: u8) {
    if lod >= 2 { return; }
    let out = v3(-row.sn * sd, row.cs * sd, 0.);
    let along = v3(row.cs, row.sn, 0.);
    let n = if lod == 0 { 7 } else { 4 };
    let base = p + v3(0., 0., -1.0);
    for i in 0..n {
        let h = 3.0 + 3.0 * hash(k, i, 44);
        let at = base + out * (0.5 + 0.9 * hash(k, i, 45)) + along * ((hash(k, i, 46) - 0.5) * 2.4);
        let tip = at + out * (0.4 + 1.2 * hash(k, i, 47)) + v3(0., 0., h);
        g.spike(at, tip, 0.1 + 0.05 * hash(k, i, 48), WA_BONE);
        if i == 0 { g.cuboid(tip - v3(0.1, 0.1, 0.1), tip + v3(0.1, 0.1, 0.16), WA_LANTERN_DIM); }
    }
}

fn bone_spike(g: &mut Soup, p: V3, row: Row, sd: f32, k: i32, lod: u8) {
    let out = v3(-row.sn * sd, row.cs * sd, 0.);
    let h = 5.0 + 3.0 * hash(k, 51, 49);
    let foot = p + out * 0.9 + v3(0., 0., -1.4);
    let r = 0.55 + 0.2 * hash(k, 52, 49);
    if lod >= 2 { g.spike(foot + v3(0., 0., 1.4), foot + v3(0., 0., h * 0.7), r, WA_BONE); return; }
    g.column(foot, 1.9, r * 1.5, r * 1.1, 5, WA_BONE);
    g.spike(foot + v3(0., 0., 1.9), foot + out * (0.6 + 0.8 * hash(k, 53, 49)) + v3(0., 0., h), r, WA_BONE);
    if lod == 0 {
        let side = v3(row.cs, row.sn, 0.) * 0.7;
        g.spike(foot + side + v3(0., 0., 1.4), foot + side + out * 0.5 + v3(0., 0., h * 0.55), r * 0.6, WA_BONE);
    }
}

fn banner(g: &mut Soup, r: &dyn Route, p: V3, row: Row, sd: f32, k: i32, lod: u8) {
    let st = style(r.zone(row.s).0);
    let fr = Fr { o: p, f: v3(row.cs, row.sn, 0.), l: v3(-row.sn, row.cs, 0.) };
    g.bx(&fr, (-0.12, 0.12), (-0.12, 0.12), (-0.4, 6.4), st.post);
    if lod >= 2 { return; }
    let arm = -sd;
    let (la, lb) = if arm > 0. { (0., 1.7) } else { (-1.7, 0.) };
    g.bx(&fr, (-0.08, 0.08), (la, lb), (5.9, 6.1), st.post);
    let cloth = if (k / 2) % 2 == 0 { st.cloth_b } else { st.cloth };
    let (l0, l1) = (arm * 0.2, arm * 1.6);
    g.quad(fr.pt(0.05, l0, 5.9), fr.pt(0.05, l1, 5.9), fr.pt(0.05, l1, 2.6), fr.pt(0.05, l0, 2.6), cloth);
    g.quad(fr.pt(0.06, l0, 5.9), fr.pt(0.06, l1, 5.9), fr.pt(0.06, l1, 5.6), fr.pt(0.06, l0, 5.6), TORCH_DIM);
}

fn pylon(g: &mut Soup, r: &dyn Route, p: V3, row: Row, lod: u8) {
    let st = style(r.zone(row.s).0);
    let fr = Fr { o: p, f: v3(row.cs, row.sn, 0.), l: v3(-row.sn, row.cs, 0.) };
    if lod >= 2 {
        g.quad(p + v3(-0.4, 0., 3.9), p + v3(0.4, 0., 3.9), p + v3(0.4, 0., 4.8), p + v3(-0.4, 0., 4.8), WINDOW_HOT);
        return;
    }
    g.bx_open(&fr, (-0.28, 0.28), (-0.28, 0.28), (-0.4, 4.2), st.post);
    g.bx_open(&fr, (-0.6, 0.6), (-0.6, 0.6), (4.2, 4.62), st.plate_b);
    g.quad(p + v3(-0.46, -0.46, 4.63), p + v3(0.46, -0.46, 4.63), p + v3(0.46, 0.46, 4.63), p + v3(-0.46, 0.46, 4.63), WINDOW_HOT);
    if lod == 0 {
        g.bx_open(&fr, (-0.33, 0.33), (-0.33, 0.33), (2.2, 2.4), SEAM);
        g.spike(p + v3(0., 0., 4.63), p + v3(0., 0., 5.7), 0.34, FLAME_LICK);
    }
}

fn chevron(g: &mut Soup, r: &dyn Route, s: f32, sd: f32, turn: f32) {
    let row = row_on(r, 0, s);
    let st = style(r.zone(s).0);
    let fr = Fr::on(&row, 0.);
    let l = sd * (row.half(sd) + 1.7);
    let z0 = row.pt(l, 0.).z - row.h;
    g.cuboid(fr.pt(-0.12, l - 0.12, z0 - 0.4), fr.pt(0.12, l + 0.12, z0 + 1.0), st.post);
    let (w, h) = (1.5, 0.95);
    let (c, zc) = (l, z0 + 1.0 + h);
    g.quad(fr.pt(-0.14, c - w, zc - h), fr.pt(-0.14, c + w, zc - h), fr.pt(-0.14, c + w, zc + h), fr.pt(-0.14, c - w, zc + h), st.chevron[0]);
    let tip = c + turn * 0.9;
    let back = c - turn * 0.8;
    for dz in [1.0f32, -1.0] {
        let (top0, top1) = (zc + dz * 0.78, zc + dz * 0.5);
        g.quad(fr.pt(-0.17, back, top0), fr.pt(-0.17, tip, zc + dz * 0.08), fr.pt(-0.17, tip, zc - dz * 0.2), fr.pt(-0.17, back, top1), st.chevron[1]);
    }
}

fn truss(g: &mut Soup, r: &dyn Route, s: f32) {
    let row = row_on(r, 0, s);
    let st = style(r.zone(s).0);
    let fr = Fr::on(&row, 0.);
    let w = row.wl.max(row.wr) + 1.6;
    for sd in [-1.0f32, 1.0] {
        g.bx_open(&fr, (-0.45, 0.45), (sd * w - 0.5, sd * w + 0.5), (-3., 13.), st.post);
        g.bx(&fr, (-0.8, 0.8), (sd * w - 0.85, sd * w + 0.85), (-3., -1.5), st.plate_b);
        g.bx(&fr, (-0.55, 0.55), (sd * w - 0.6, sd * w + 0.6), (7.6, 7.8), SEAM);
    }
    g.bx(&fr, (-0.4, 0.4), (-w, w), (12.4, 13.4), st.plate);
    let t = 10;
    for i in 0..t {
        let (l0, l1) = (-w + 2. * w * i as f32 / t as f32, -w + 2. * w * (i + 1) as f32 / t as f32);
        let (a, b) = if i % 2 == 0 { (11.6, 12.4) } else { (12.4, 11.6) };
        g.quad(fr.pt(0., l0, a), fr.pt(0., l1, b), fr.pt(0., l1, b + 0.22), fr.pt(0., l0, a + 0.22), st.plate);
    }
    g.bx(&fr, (-0.3, 0.3), (-w * 0.92, w * 0.92), (11.3, 11.55), SEAM);
    for i in 0..5 {
        let l = -w * 0.8 + 1.6 * w * i as f32 / 4.;
        g.bx(&fr, (-0.55, 0.55), (l - 0.55, l + 0.55), (10.4, 11.3), WINDOW_HOT);
    }
    g.quad(fr.pt(-0.46, -w * 0.5, 12.2), fr.pt(-0.46, w * 0.5, 12.2), fr.pt(-0.46, w * 0.5, 10.5), fr.pt(-0.46, -w * 0.5, 10.5), HAZARD_BLACK);
    g.quad(fr.pt(-0.5, -w * 0.46, 11.6), fr.pt(-0.5, w * 0.46, 11.6), fr.pt(-0.5, w * 0.46, 11.1), fr.pt(-0.5, -w * 0.46, 11.1), HAZARD_GOLD);
}
