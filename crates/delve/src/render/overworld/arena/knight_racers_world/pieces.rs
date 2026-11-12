//! Set pieces that span or enclose the road: bridges, tunnels, arches, gates,
//! gantries, drawbridges and the lips of a chasm jump. Beside-the-road
//! scenery lives in `scenery.rs`.
use super::cache::{self, Key, Tris};
use super::frame::{Row, row_on};
use super::geo::{Fr, Soup};
use super::look::{Style, style};
use super::noise::hash;
use super::palette::*;
use super::route::{Kind, Piece, Route};
use crate::render::world3d::math::{V3, v3};
use std::f32::consts::{PI, TAU};

/// Long pieces are built, cached and gathered in stretches this long.
pub const SEG: f32 = 48.;

pub struct Ctx<'a> {
    pub r: &'a dyn Route,
    /// The stretch of the piece this build covers, in distance along the road.
    pub win: (f32, f32),
    /// The road the piece stands on: 0 the main line, n a fork.
    pub line: u8,
    pub p: Piece,
    pub st: Style,
    pub seed: u32,
}

impl Ctx<'_> {
    pub fn row(&self, s: f32) -> Row { row_on(self.r, self.line, s) }
    /// Whether distance `s` falls in the stretch this build covers.
    pub fn has(&self, s: f32) -> bool { s >= self.win.0 && s < self.win.1 }
    pub fn mid(&self) -> f32 { (self.p.s0 + self.p.s1) * 0.5 }
    pub fn len(&self) -> f32 { self.p.s1 - self.p.s0 }
    pub fn rnd(&self, i: i32, k: i32) -> f32 { hash(i, k, self.seed) }
    /// Ground below a world point, no higher than the road beside it.
    pub fn ground(&self, p: V3) -> f32 { self.r.ground(p.x, p.y) }
}

/// The pieces of road `line`.
pub fn list(r: &dyn Route, line: u8) -> &[Piece] {
    match line.checked_sub(1) { Some(f) => &r.forks()[f as usize].pieces, None => r.pieces() }
}

/// How many stretches a piece is built in: the repeating ones are cut up so a
/// frame gathers only the stretches in range; the single ones are one.
pub fn segments(p: &Piece) -> usize {
    match p.kind {
        Kind::Tunnel | Kind::CrystalVault | Kind::Bridge | Kind::Viaduct | Kind::Grandstand | Kind::Statues | Kind::Lanterns | Kind::Pylons | Kind::Rampart | Kind::Spires | Kind::Stacks => (((p.s1 - p.s0) / SEG).ceil() as usize).max(1),
        _ => 1,
    }
}

/// The stretch of road a piece's segment `seg` covers.
pub fn window(p: &Piece, seg: usize) -> (f32, f32) {
    let n = segments(p);
    (if seg == 0 { f32::NEG_INFINITY } else { p.s0 + seg as f32 * SEG }, if seg + 1 >= n { f32::INFINITY } else { p.s0 + (seg + 1) as f32 * SEG })
}

pub fn piece(r: &dyn Route, line: u8, i: usize, seg: usize) -> Tris {
    cache::get(Key { course: r.key(), kind: 3, a: i as i32, b: line as i32 * 1000 + seg as i32, lod: 0 }, || build(r, line, i, seg))
}

fn build(r: &dyn Route, line: u8, i: usize, seg: usize) -> Vec<crate::render::world3d::mesh::Tri> {
    let p = list(r, line)[i];
    let look = match line.checked_sub(1) { Some(f) => r.forks()[f as usize].look, None => r.zone((p.s0 + p.s1) * 0.5).0 };
    let c = Ctx { r, line, win: window(&p, seg), p, st: style(look), seed: p.seed.wrapping_mul(2654435761).wrapping_add(i as u32 * 31 + line as u32) };
    let mut g = Soup::new();
    match p.kind {
        Kind::Arch => arch(&mut g, &c),
        Kind::Gantry => gantry(&mut g, &c),
        Kind::Tunnel => tunnel(&mut g, &c, false),
        Kind::CrystalVault => { tunnel(&mut g, &c, true); super::scenery::crystals(&mut g, &c); }
        Kind::CastleGate => castle_gate(&mut g, &c),
        Kind::Bridge => bridge(&mut g, &c, false),
        Kind::Viaduct => bridge(&mut g, &c, true),
        Kind::Drawbridge => drawbridge(&mut g, &c),
        Kind::ChasmJump => lips(&mut g, &c),
        _ => super::scenery::beside(&mut g, &c),
    }
    g.t
}

/// A level frame across the road at `s`, its origin on the centreline.
fn across(c: &Ctx, s: f32) -> (Row, Fr) {
    let row = c.row(s);
    (row, Fr::on(&row, 0.))
}

fn merlons(g: &mut Soup, fr: &Fr, a: (f32, f32), l0: f32, l1: f32, u: f32, mat: u8) {
    let n = ((l1 - l0) / 1.8).max(1.) as usize;
    for k in 0..n {
        if k % 2 == 0 {
            let (x0, x1) = (l0 + (l1 - l0) * k as f32 / n as f32, l0 + (l1 - l0) * (k + 1) as f32 / n as f32);
            g.bx(fr, a, (x0, x1), (u, u + 1.1), mat);
        }
    }
}

fn arch(g: &mut Soup, c: &Ctx) {
    let (row, fr) = across(c, c.mid());
    let w = row.wl.max(row.wr) + 1.6;
    let th = (c.len() * 0.3).clamp(1.5, 3.0);
    for sd in [-1.0f32, 1.0] {
        g.bx_open(&fr, (-th, th), (sd * w - 1.1, sd * w + 1.1), (-3., 12.4), c.st.rail);
        g.bx(&fr, (-th - 0.3, th + 0.3), (sd * w - 1.4, sd * w + 1.4), (11.6, 12.4), c.st.rail_cap);
        g.cuboid(fr.pt(th * 0.2, sd * w - 0.5, 3.0), fr.pt(th * 0.2 + 0.6, sd * w + 0.5, 3.6), c.st.glow);
    }
    // The lintel and the banner stand above the lens: the chase eye is eight metres up.
    g.bx(&fr, (-th, th), (-w - 1.2, w + 1.2), (12.4, 14.2), c.st.rail);
    merlons(g, &fr, (-th, th), -w - 1.2, w + 1.2, 14.2, c.st.rail);
    // The banner hangs on the road face, lit from under by two lamps.
    g.quad(fr.pt(-th - 0.02, -w, 12.2), fr.pt(-th - 0.02, w, 12.2), fr.pt(-th - 0.02, w, 9.4), fr.pt(-th - 0.02, -w, 9.4), c.st.cloth);
    g.quad(fr.pt(-th - 0.03, -w, 9.4), fr.pt(-th - 0.03, w, 9.4), fr.pt(-th - 0.03, w, 9.0), fr.pt(-th - 0.03, -w, 9.0), c.st.rail_cap);
}

fn gantry(g: &mut Soup, c: &Ctx) {
    let n = ((c.len() / 40.) as i32).max(1);
    for k in 0..n {
        let s = c.p.s0 + (k as f32 + 0.5) * c.len() / n as f32;
        let (row, fr) = across(c, s);
        let w = row.wl.max(row.wr) + 1.4;
        for sd in [-1.0f32, 1.0] {
            g.bx_open(&fr, (-0.5, 0.5), (sd * w - 0.5, sd * w + 0.5), (-3., 14.), c.st.plate_b);
            g.bx(&fr, (-0.9, 0.9), (sd * w - 0.9, sd * w + 0.9), (-3., -1.6), c.st.plate);
        }
        // Everything across the road hangs above the lens (the chase eye is eight metres up).
        g.bx(&fr, (-0.4, 0.4), (-w, w), (13.0, 14.0), c.st.plate_b);
        g.bx(&fr, (-0.3, 0.3), (-w, w), (11.4, 12.0), c.st.plate);
        let t = 8;
        for i in 0..t {
            let (l0, l1) = (-w + 2. * w * i as f32 / t as f32, -w + 2. * w * (i + 1) as f32 / t as f32);
            let (a, b) = if i % 2 == 0 { (12.0, 13.0) } else { (13.0, 12.0) };
            g.quad(fr.pt(0., l0, a), fr.pt(0., l1, b), fr.pt(0., l1, b + 0.25), fr.pt(0., l0, a + 0.25), c.st.plate);
        }
        for l in [-w * 0.6, 0., w * 0.6] {
            g.bx(&fr, (-0.5, 0.5), (l - 0.5, l + 0.5), (10.2, 11.4), c.st.lamp);
            g.bx(&fr, (-0.04, 0.04), (l - 0.04, l + 0.04), (11.4, 13.0), c.st.plate_b);
        }
        g.quad(fr.pt(-0.5, -w * 0.9, 11.2), fr.pt(-0.5, -w * 0.3, 11.2), fr.pt(-0.5, -w * 0.3, 9.4), fr.pt(-0.5, -w * 0.9, 9.4), c.st.cloth_b);
    }
}

/// The half-width of the road at `s`: where a vault's feet stand.
fn rib_width(_fr: &Fr, c: &Ctx, s: f32) -> f32 { let row = c.row(s); row.wl.max(row.wr) }

/// A ribbed vault over the road. `rough` roughens the roof and keeps it low and
/// close, for a vault of crystal.
fn tunnel(g: &mut Soup, c: &Ctx, rough: bool) {
    let step = 8.;
    let n = (c.len() / step).ceil() as i32;
    let rib = |k: i32| {
        let s = (c.p.s0 + k as f32 * step).min(c.p.s1);
        let (row, fr) = across(c, s);
        let rad = row.wl.max(row.wr) + 2.4;
        let rise = rad * if rough { 1.1 + 0.5 * c.rnd(k, 1) } else { 0.95 };
        (s, fr, rad, rise)
    };
    const SEGS: usize = 10;
    // Lamps hang in the crown: lanterns in a mine, a glow of crystal in a vault.
    let lamp = if rough { CYAN_GLOW } else { LANTERN };
    for k in 0..=n {
        let (s, fr, rad, rise) = rib(k);
        if !c.has(s) { continue; }
        let ring = move |t: f32| { let a = PI * t; (-rad * a.cos(), 3.0 + rise * a.sin()) };
        if k > 0 {
            // The shell between the last rib and this: walls, then the roof.
            let (_, pf, pr, prise) = rib(k - 1);
            let ring0 = move |t: f32| { let a = PI * t; (-pr * a.cos(), 3.0 + prise * a.sin()) };
            for i in 0..SEGS {
                let (t0, t1) = (i as f32 / SEGS as f32, (i + 1) as f32 / SEGS as f32);
                let (a0, a1) = (ring0(t0), ring0(t1));
                let (b0, b1) = (ring(t0), ring(t1));
                g.wall(pf.pt(0., a0.0, a0.1), fr.pt(0., b0.0, b0.1), fr.pt(0., b1.0, b1.1), pf.pt(0., a1.0, a1.1), c.st.rock);
            }
            for sd in [-1.0f32, 1.0] {
                g.wall(pf.pt(0., sd * pr, -1.5), fr.pt(0., sd * rad, -1.5), fr.pt(0., sd * rad, 3.), pf.pt(0., sd * pr, 3.), c.st.rock);
            }
            // A foundry's hall runs with molten metal: a trough along the foot of each wall, and a
            // furnace mouth in the wall every few metres, alternate sides. They are the light of the place.
            if c.st.liquid == Some(LAVA) {
                for sd in [-1.0f32, 1.0] {
                    g.quad(pf.pt(0., sd * (pr - 1.7), 0.05), fr.pt(0., sd * (rad - 1.7), 0.05), fr.pt(0., sd * (rad - 0.2), 0.05), pf.pt(0., sd * (pr - 0.2), 0.05), LAVA);
                }
                let sd = if (k / 2) % 2 == 0 { 1.0f32 } else { -1.0 };
                if k % 2 == 0 {
                    let l = sd * (pr - 0.08);
                    g.quad(pf.pt(1.2, l, 0.4), pf.pt(6.0, sd * (rad - 0.08), 0.4), pf.pt(6.0, sd * (rad - 0.08), 4.2), pf.pt(1.2, l, 4.2), LAVA);
                    g.quad(pf.pt(1.2, l, 4.2), pf.pt(6.0, sd * (rad - 0.08), 4.2), pf.pt(6.0, sd * (rad - 0.08), 4.9), pf.pt(1.2, l, 4.9), LAVA_HOT);
                }
            }
        }
        // Every other rib stands proud: a band of the rail's stone, and posts at its feet.
        if k % 2 == 0 {
            g.sweep(&fr, (-0.5, 0.5), SEGS, |t| { let (l, u) = ring(t); (l * 0.985, u - 0.12) }, c.st.rail);
            for sd in [-1.0f32, 1.0] {
                g.bx_open(&fr, (-0.7, 0.7), (sd * rad - 0.8, sd * rad + 0.8), (-1.5, 3.2), c.st.rail);
            }
        }
        // A shaft of daylight through a crack in the roof, now and again, beside the road.
        if rough && k % 6 == 4 {
            let sd = if c.rnd(k, 31) < 0.5 { -1.0 } else { 1.0 };
            let l = sd * (rib_width(&fr, c, s) - 2.6);
            let (top, foot) = (fr.pt(0., l, 3.0 + rise * 0.8), fr.pt(0., l, 0.06));
            g.column(foot, top.z - foot.z, 1.7, 0.5, 6, SHAFT);
            for i in 0..6 {
                let (a0, a1) = (i as f32 / 6. * TAU, (i + 1) as f32 / 6. * TAU);
                g.tri(foot + v3(0., 0., 0.02), foot + v3(4.6 * a0.cos(), 4.6 * a0.sin(), 0.02), foot + v3(4.6 * a1.cos(), 4.6 * a1.sin(), 0.02), SHAFT_POOL);
            }
        }
        // A lamp at the crown of every other rib.
        if k % 2 == 1 {
            let (l, u) = ring(0.5);
            g.bx(&fr, (-0.4, 0.4), (l - 0.4, l + 0.4), (u - 1.5, u - 0.2), lamp);
        }
    }
    // Portals: a heavy frame at each mouth, standing proud of the rock.
    for (s, face) in [(c.p.s0, -1.0f32), (c.p.s1, 1.0)] {
        if (face < 0. && !c.win.0.is_infinite()) || (face > 0. && !c.win.1.is_infinite()) { continue; }
        let (row, fr) = across(c, s);
        let rad = row.wl.max(row.wr) + 2.4;
        let (a0, a1) = if face < 0. { (-1.6, 0.3) } else { (-0.3, 1.6) };
        for sd in [-1.0f32, 1.0] {
            g.bx_open(&fr, (a0, a1), (sd * rad - 1.6, sd * rad + 1.4), (-3., 5.), c.st.rail);
        }
        g.sweep(&fr, (a0, a1), 14, |t| { let a = PI * t; (-(rad + 1.5) * a.cos(), 3.0 + (rad * 0.95 + 1.2) * a.sin()) }, c.st.rail);
        g.sweep(&fr, (a0, a1), 14, |t| { let a = PI * t; (-(rad + 0.2) * a.cos(), 3.0 + (rad * 0.95 + 0.1) * a.sin()) }, c.st.rail_cap);
    }
}

fn castle_gate(g: &mut Soup, c: &Ctx) {
    let (row, fr) = across(c, c.mid());
    let w = row.wl.max(row.wr) + 1.0;
    let (th, tw) = (5.0, 9.0);
    // Towers either side, crowned and windowed.
    for sd in [-1.0f32, 1.0] {
        let (l0, l1) = if sd > 0. { (w, w + tw) } else { (-w - tw, -w) };
        g.bx_open(&fr, (-th, th), (l0, l1), (-4., 26.), c.st.masonry);
        g.bx(&fr, (-th - 0.6, th + 0.6), (l0 - 0.6, l1 + 0.6), (26., 27.2), c.st.masonry);
        for (a0, a1) in [(-th - 0.6, -th + 1.4), (th - 1.4, th + 0.6)] {
            for (l0, l1) in [(l0 - 0.6, l0 + 1.4), (l1 - 1.4, l1 + 0.6)] { g.bx(&fr, (a0, a1), (l0, l1), (27.2, 28.6), c.st.masonry); }
        }
        g.spike(fr.pt(0., (l0 + l1) * 0.5, 27.2), fr.pt(0., (l0 + l1) * 0.5, 38.), 3.4, c.st.slate);
        for u in [12., 17., 22.] {
            g.quad(fr.pt(-th - 0.03, (l0 + l1) * 0.5 - 0.6, u + 1.8), fr.pt(-th - 0.03, (l0 + l1) * 0.5 + 0.6, u + 1.8), fr.pt(-th - 0.03, (l0 + l1) * 0.5 + 0.6, u), fr.pt(-th - 0.03, (l0 + l1) * 0.5 - 0.6, u), c.st.glow);
        }
        g.quad(fr.pt(-th - 0.05, l0 + 1.2, 24.), fr.pt(-th - 0.05, l1 - 1.2, 24.), fr.pt(-th - 0.05, l1 - 1.2, 14.), fr.pt(-th - 0.05, l0 + 1.2, 14.), if sd > 0. { c.st.cloth } else { c.st.cloth_b });
    }
    // The gatehouse over the road: a block with a round-headed passage.
    let top = 24.;
    let pass = |t: f32| { let a = PI * t; (-w * a.cos(), 3.0 + (10.5 - 3.0) * a.sin()) };
    for face in [-th, th] {
        for i in 0..12 {
            let (p0, p1) = (pass(i as f32 / 12.), pass((i + 1) as f32 / 12.));
            g.quad(fr.pt(face, p0.0, p0.1), fr.pt(face, p1.0, p1.1), fr.pt(face, p1.0, top), fr.pt(face, p0.0, top), c.st.masonry);
        }
    }
    g.sweep(&fr, (-th, th), 12, pass, c.st.masonry);
    g.bx(&fr, (-th, th), (-w, w), (top, top + 1.2), c.st.masonry);
    merlons(g, &fr, (-th, th), -w, w, top + 1.2, c.st.masonry);
    // A raised portcullis and the banner over it.
    for k in 0..9 {
        let l = -w * 0.8 + 1.6 * w * k as f32 / 8.;
        let h = 3.0 + (7.4) * (1. - ((l / w).powi(2))).max(0.).sqrt();
        g.spike(fr.pt(-th + 0.6, l, h + 3.4), fr.pt(-th + 0.6, l, h + 0.9), 0.22, c.st.plate_b);
    }
    g.quad(fr.pt(-th - 0.03, -w * 0.6, 22.5), fr.pt(-th - 0.03, w * 0.6, 22.5), fr.pt(-th - 0.03, w * 0.6, 15.5), fr.pt(-th - 0.03, -w * 0.6, 15.5), c.st.cloth);
}

/// The deck's own substructure: girders under the road, piers to the ground.
fn bridge(g: &mut Soup, c: &Ctx, arches: bool) {
    let step = 4.;
    let n = (c.len() / step).ceil() as i32;
    let mut prev: Option<Row> = None;
    for k in 0..=n {
        let row = c.row((c.p.s0 + k as f32 * step).min(c.p.s1));
        if let (Some(a), true) = (prev, c.has(row.s)) {
            let (lw, rw) = (a.wl + 0.9, a.wr + 0.9);
            // The deck's underside, and its two fascias.
            g.quad(a.pt(-rw, -1.8), row.pt(-rw, -1.8), row.pt(lw, -1.8), a.pt(lw, -1.8), c.st.rail);
            g.quad(a.pt(lw, 0.), row.pt(lw, 0.), row.pt(lw, -1.8), a.pt(lw, -1.8), c.st.rail);
            g.quad(a.pt(-rw, 0.), row.pt(-rw, 0.), row.pt(-rw, -1.8), a.pt(-rw, -1.8), c.st.rail);
        }
        prev = Some(row);
    }
    // Piers and, over a viaduct, the arcade between them.
    let span = if arches { 22. } else { 30. };
    let piers = (c.len() / span).ceil() as i32 + 1;
    for k in 0..piers {
        let s = (c.p.s0 + k as f32 * c.len() / (piers - 1).max(1) as f32).min(c.p.s1);
        if !c.has(s) { continue; }
        let row = c.row(s);
        let fr = Fr::on(&row, 0.);
        for sd in [-1.0f32, 1.0] {
            let foot = fr.pt(0., sd * (row.half(sd) - 1.0), 0.);
            let ground = c.ground(foot).min(row.h - 4.);
            let h = row.h - 1.8 - ground;
            g.column(v3(foot.x, foot.y, ground - 1.), h + 1., 2.0, 1.4, 8, c.st.rail);
            g.bx(&fr, (-2.0, 2.0), (sd * (row.half(sd) - 1.0) - 1.8, sd * (row.half(sd) - 1.0) + 1.8), (-3.2, -1.8), c.st.rail_cap);
        }
        if arches && k + 1 < piers {
            let s2 = (c.p.s0 + (k + 1) as f32 * c.len() / (piers - 1).max(1) as f32).min(c.p.s1);
            let (sa, sb) = (row.s, s2);
            let rise = 9.0;
            for sd in [-1.0f32, 1.0] {
                let steps = 10;
                let pt = |t: f32, depth: f32| { let r = c.row(sa + (sb - sa) * t); r.pt(sd * (r.half(sd) + 0.9), -1.8 - depth) };
                for i in 0..steps {
                    let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
                    let d = |t: f32| rise * (1. - (PI * t).sin().max(0.).powf(0.7));
                    g.quad(pt(t0, 0.), pt(t1, 0.), pt(t1, d(t1)), pt(t0, d(t0)), c.st.rail);
                }
            }
        }
    }
    // Lamp posts on the parapet.
    let lamps = (c.len() / 14.) as i32;
    for k in 0..=lamps {
        if !c.has(c.p.s0 + k as f32 * 14.) { continue; }
        let row = c.row(c.p.s0 + k as f32 * 14.);
        let fr = Fr::on(&row, 0.);
        for sd in [-1.0f32, 1.0] {
            let l = sd * (row.half(sd) + 0.35);
            g.bx(&fr, (-0.2, 0.2), (l - 0.2, l + 0.2), (1., 4.2), c.st.post);
            g.bx(&fr, (-0.35, 0.35), (l - 0.35, l + 0.35), (4.2, 4.9), c.st.lamp);
        }
    }
}

fn chain(g: &mut Soup, a: V3, b: V3, mat: u8) {
    let n = ((b - a).length() / 0.6) as usize + 1;
    for i in 0..n {
        let (p0, p1) = (a.lerp(b, i as f32 / n as f32), a.lerp(b, (i as f32 + 0.8) / n as f32));
        let w = if i % 2 == 0 { v3(0.1, 0., 0.) } else { v3(0., 0.1, 0.) };
        g.quad(p0 - w, p1 - w, p1 + w, p0 + w, mat);
    }
}

fn drawbridge(g: &mut Soup, c: &Ctx) {
    // Planks over the road, a tower at each end and chains from the tower heads.
    let step = 1.6;
    let n = (c.len() / step).ceil() as i32;
    let mut prev: Option<Row> = None;
    for k in 0..=n {
        let row = c.row((c.p.s0 + k as f32 * step).min(c.p.s1));
        if let Some(a) = prev {
            let m = if k % 2 == 0 { c.st.timber } else { c.st.rail };
            g.quad(a.pt(-a.wr, 0.07), row.pt(-row.wr, 0.07), row.pt(row.wl, 0.07), a.pt(a.wl, 0.07), m);
        }
        prev = Some(row);
    }
    for (s, dir) in [(c.p.s0, -1.0f32), (c.p.s1, 1.0)] {
        let (row, fr) = across(c, s);
        let w = row.wl.max(row.wr) + 1.4;
        for sd in [-1.0f32, 1.0] {
            let l = sd * (w + 2.);
            g.bx_open(&fr, (dir * 1.0 - 2.5, dir * 1.0 + 2.5), (l - 2.2, l + 2.2), (-4., 17.), c.st.masonry);
            g.bx(&fr, (dir * 1.0 - 3., dir * 1.0 + 3.), (l - 2.8, l + 2.8), (17., 18.), c.st.masonry);
            merlons(g, &fr, (dir * 1.0 - 3., dir * 1.0 + 3.), l - 2.8, l + 2.8, 18., c.st.masonry);
            g.quad(fr.pt(dir * 1.0 - 2.55 * 1.0, l - 0.5, 15.), fr.pt(dir * 1.0 - 2.55, l + 0.5, 15.), fr.pt(dir * 1.0 - 2.55, l + 0.5, 12.), fr.pt(dir * 1.0 - 2.55, l - 0.5, 12.), c.st.glow);
            // Chains from the tower head to the far end of the deck.
            let far = c.row(if dir < 0. { (s + c.len() * 0.45).min(c.p.s1) } else { (s - c.len() * 0.45).max(c.p.s0) });
            chain(g, fr.pt(dir * 1.0, sd * w, 15.), Fr::on(&far, 0.).pt(0., sd * (far.half(sd) + 0.2), 0.8), c.st.plate_b);
        }
    }
}

/// The lips of a chasm jump: raw faces down from each edge of the gap, a glow far below.
/// The gap is where the road is missing, which sits inside the piece (the piece also
/// holds the run-up and the landing), so the lips stand at the gap's own edges.
fn lips(g: &mut Soup, c: &Ctx) {
    let gaps: Vec<_> = c.r.gaps().into_iter().filter(|q| q.s0 >= c.p.s0 - 1. && q.s1 <= c.p.s1 + 1.).collect();
    let (g0, g1) = match (gaps.first(), gaps.last()) { (Some(a), Some(b)) => (a.s0, b.s1), _ => (c.p.s0, c.p.s1) };
    for (s, face) in [(g0, 1.0f32), (g1, -1.0)] {
        let row = c.row(s);
        let fr = Fr::on(&row, 0.);
        let (l, r) = (row.wl + 1., row.wr + 1.);
        // The cut face, leaning in, and the slab's edge in the rail's cap colour with a lamp each side.
        g.wall(fr.pt(0., -r, 0.), fr.pt(0., l, 0.), fr.pt(face * 2.0, l, -9.), fr.pt(face * 2.0, -r, -9.), c.st.rock);
        g.bx(&fr, (-0.5, 0.5), (-r, l), (0.0, 0.5), c.st.rail_cap);
        g.bx(&fr, (-0.5, 0.5), (-r, l), (-1.6, 0.0), c.st.rail);
        for sd in [-1.0f32, 1.0] {
            g.bx(&fr, (-0.3, 0.3), (sd * (row.half(sd) + 0.6) - 0.3, sd * (row.half(sd) + 0.6) + 0.3), (0., 2.8), c.st.post);
            g.bx(&fr, (-0.45, 0.45), (sd * (row.half(sd) + 0.6) - 0.45, sd * (row.half(sd) + 0.6) + 0.45), (2.8, 3.5), LAVA_HOT);
        }
    }
    // The deep: a floor of glow, far below the road.
    let (a, b) = (c.row(g0), c.row(g1));
    let w = a.wl.max(a.wr).max(b.wl.max(b.wr)) + 16.;
    let (fa, fb) = (Fr::on(&a, 0.), Fr::on(&b, 0.));
    let z = a.h.min(b.h) - 46.;
    g.quad(fa.pt(0., -w, 0.).at_z(z), fb.pt(0., -w, 0.).at_z(z), fb.pt(0., w, 0.).at_z(z), fa.pt(0., w, 0.).at_z(z), c.st.liquid.unwrap_or(LAVA));
}

trait AtZ { fn at_z(self, z: f32) -> V3; }
impl AtZ for V3 { fn at_z(mut self, z: f32) -> V3 { self.z = z; self } }
const _: f32 = TAU;
