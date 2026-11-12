//! What stands on the road itself: the start gate, section arches, boost
//! strips and the hazards the sim hits with.
use super::cache::{self, Key, Tris};
use super::frame::{Row, row, row_on};
use super::geo::{Fr, Soup};
use super::look::{Style, style};
use super::palette::*;
use super::route::{HazardKind, Route};
use crate::render::world3d::math::v3;
use crate::render::world3d::mesh::Tri;

/// The props of road block `k` (see `road::blocks`): those standing in its stretch.
pub fn block(r: &dyn Route, k: i32) -> Tris {
    let (n, _, len) = super::road::blocks(r, 0);
    let k = k.rem_euclid(n);
    cache::get(Key { course: r.key(), kind: 4, a: k, b: 0, lod: 0 }, || build(r, k as f32 * len, (k + 1) as f32 * len))
}

/// The props of fork `line` (numbered from 1), all of them: forks are short.
pub fn fork(r: &dyn Route, line: u8) -> Tris {
    cache::get(Key { course: r.key(), kind: 4, a: -1, b: line as i32, lod: 0 }, || {
        let f = &r.forks()[line as usize - 1];
        let mut g = Soup::new();
        for h in &f.hazards { hazard(&mut g, r, line, h); }
        // A lit gate over each end of the fork, so the parting of the roads can be seen from far off.
        for s in [f.s0 + 16., f.s1 - 16.] { fork_gate(&mut g, r, line, s); }
        g.t
    })
}

fn at(r: &Row, y: f32) -> Fr {
    let o = r.pt(y, 0.);
    Fr { o, f: v3(r.cs, r.sn, 0.), l: v3(-r.sn, r.cs, 0.) }
}

fn build(r: &dyn Route, s0: f32, s1: f32) -> Vec<Tri> {
    let mut g = Soup::new();
    let lap = r.lap();
    let within = |s: f32| s >= s0 && s < s1 || (s0 == 0. && s >= lap - 1e-3);
    if within(0.) { gate(&mut g, r); }
    for s in r.landmarks() { if within(s) { section_arch(&mut g, r, s); } }
    for h in r.hazards() {
        if within(h.s) { hazard(&mut g, r, 0, &h); }
    }
    g.t
}

fn hazard(g: &mut Soup, r: &dyn Route, line: u8, h: &super::route::Hazard) {
    {
        let st = style(match line.checked_sub(1) { Some(f) => r.forks()[f as usize].look, None => r.zone(h.s).0 });
        let row = row_on(r, line, h.s);
        let fr = at(&row, h.y);
        match h.kind {
            HazardKind::Slag => {
                // A block in hazard bars with a hot crown, and a lit pole at each corner of its row.
                g.bx(&fr, (-h.half_l, h.half_l), (-h.half_w, h.half_w), (0.04, 1.25), st.warn);
                g.bx(&fr, (-h.half_l * 0.7, h.half_l * 0.7), (-h.half_w * 0.55, h.half_w * 0.55), (1.25, 1.5), st.hot);
                beacons(g, &fr, h, st.post);
            }
            HazardKind::Rockfall => {
                // The scar where it lands: the boulders themselves come from `dynamic`.
                g.bx(&fr, (-h.half_l, h.half_l), (-h.half_w, h.half_w), (0.03, 0.05), st.plate_b);
                for i in 0..3 {
                    let (a, l) = ((i as f32 - 1.) * h.half_l * 0.6, ((i * 5 % 3) as f32 - 1.) * h.half_w * 0.5);
                    g.bx(&fr, (a - 0.4, a + 0.4), (l - 0.4, l + 0.4), (0.05, 0.3), st.rock);
                }
                // The scar is ringed in bars, and two lit poles mark where it falls.
                g.bx(&fr, (-h.half_l, h.half_l), (-h.half_w - 0.35, -h.half_w), (0.05, 0.1), st.warn);
                g.bx(&fr, (-h.half_l, h.half_l), (h.half_w, h.half_w + 0.35), (0.05, 0.1), st.warn);
                beacons(g, &fr, h, st.post);
            }
            HazardKind::Jet => {
                g.bx(&fr, (-h.half_l, h.half_l), (-h.half_w, h.half_w), (0.02, 0.35), st.plate_b);
                g.bx(&fr, (-h.half_l * 0.55, h.half_l * 0.55), (-h.half_w * 0.55, h.half_w * 0.55), (0.35, 0.5), st.plate);
                g.bx(&fr, (-h.half_l * 0.3, h.half_l * 0.3), (-h.half_w * 0.3, h.half_w * 0.3), (0.5, 0.56), st.hot);
                beacons(g, &fr, h, st.post);
            }
            HazardKind::Slick => {
                g.bx(&fr, (-h.half_l, h.half_l), (-h.half_w, h.half_w), (0.03, 0.06), st.plate_b);
                g.bx(&fr, (-h.half_l * 0.6, h.half_l * 0.3), (-h.half_w * 0.5, h.half_w * 0.2), (0.06, 0.08), st.cloth);
                g.bx(&fr, (-h.half_l, h.half_l), (-h.half_w - 0.3, -h.half_w), (0.03, 0.09), st.warn);
                g.bx(&fr, (-h.half_l, h.half_l), (h.half_w, h.half_w + 0.3), (0.03, 0.09), st.warn);
                beacons(g, &fr, h, st.post);
            }
        }
    }
}

/// Two lit poles, one each side of a hazard, tall enough to be seen from far down the road.
fn beacons(g: &mut Soup, fr: &Fr, h: &super::route::Hazard, post: u8) {
    for sd in [-1.0f32, 1.0] {
        let l = sd * (h.half_w + 0.9);
        g.bx(fr, (-0.12, 0.12), (l - 0.12, l + 0.12), (0., 4.6), post);
        g.bx(fr, (-0.38, 0.38), (l - 0.38, l + 0.38), (4.6, 5.3), BEACON);
    }
}

/// The start and finish: a chequered line and a gate with a banner.
fn gate(g: &mut Soup, r: &dyn Route) {
    let row = row(r, 0.);
    let fr = at(&row, 0.);
    let st = style(r.zone(0.).0);
    let half = row.wl.min(row.wr);
    let n = 12;
    for i in 0..n {
        for j in 0..2 {
            let (y0, y1) = (-half + 2. * half * i as f32 / n as f32, -half + 2. * half * (i + 1) as f32 / n as f32);
            let (a0, a1) = (j as f32 * 1.2, (j + 1) as f32 * 1.2);
            g.quad(row.pt(y0, 0.05) + fr.f * a0, row.pt(y0, 0.05) + fr.f * a1, row.pt(y1, 0.05) + fr.f * a1, row.pt(y1, 0.05) + fr.f * a0, if (i + j) % 2 == 0 { STEEL } else { st.plate_b });
        }
    }
    let edge = half + 1.6;
    for sd in [-1.0f32, 1.0] {
        g.bx_open(&fr, (-0.5, 0.5), (sd * edge - 0.6, sd * edge + 0.6), (-2., 12.), st.rail);
        g.bx(&fr, (-0.8, 0.8), (sd * edge - 0.9, sd * edge + 0.9), (-0.5, 0.9), st.rail_cap);
    }
    g.bx(&fr, (-0.5, 0.5), (-edge - 0.6, edge + 0.6), (11.2, 12.2), st.rail);
    g.quad(fr.pt(-0.02, -edge, 11.2), fr.pt(-0.02, edge, 11.2), fr.pt(-0.02, edge, 8.6), fr.pt(-0.02, -edge, 8.6), st.cloth);
    g.quad(fr.pt(0.02, -edge, 11.2), fr.pt(0.02, edge, 11.2), fr.pt(0.02, edge, 8.6), fr.pt(0.02, -edge, 8.6), st.cloth);
    g.bx(&fr, (-0.55, 0.55), (-edge, edge), (12.2, 12.7), st.glow);
    if st.plated { forge_gate(g, &fr, &st, edge); }
}

/// The Foundry Run's start: a brick furnace tower each side of the road, windows glowing up its
/// face, a stack on top that burns, a hot lintel between them. Seen from a kilometre off.
fn forge_gate(g: &mut Soup, fr: &Fr, st: &Style, edge: f32) {
    for sd in [-1.0f32, 1.0] {
        let (l0, l1) = if sd > 0. { (edge + 1.6, edge + 11.) } else { (-edge - 11., -edge - 1.6) };
        g.bx_open(fr, (-4.5, 4.5), (l0, l1), (-4., 26.), st.masonry);
        g.bx(fr, (-5., 5.), (l0 - 0.5, l1 + 0.5), (26., 27.), st.plate_b);
        // Furnace windows and heat bands on the face the pods see and the face toward the road.
        for (k, z) in [4.0f32, 9.5, 15., 20.5].iter().enumerate() {
            let c = (l0 + l1) * 0.5;
            g.quad(fr.pt(-4.52, c - 2.6, *z + 3.2), fr.pt(-4.52, c + 2.6, *z + 3.2), fr.pt(-4.52, c + 2.6, *z), fr.pt(-4.52, c - 2.6, *z), if k % 2 == 0 { WINDOW_HOT } else { WINDOW_DIM });
            let face = if sd > 0. { l0 } else { l1 };
            g.quad(fr.pt(-2.6, face, *z + 3.2), fr.pt(2.6, face, *z + 3.2), fr.pt(2.6, face, *z), fr.pt(-2.6, face, *z), if k % 2 == 0 { WINDOW_DIM } else { WINDOW_HOT });
            g.bx(fr, (-4.55, 4.55), (l0 - 0.05, l1 + 0.05), (*z - 0.7, *z - 0.45), SEAM);
        }
        // The stack, and what comes out of it.
        let c = (l0 + l1) * 0.5;
        let top = fr.pt(0., c, 27.);
        g.column(top, 16., 2.3, 1.7, 8, st.slate);
        g.column(v3(top.x, top.y, top.z + 16.), 0.8, 1.9, 1.9, 8, LAVA);
        g.spike(top + v3(0., 0., 16.8), top + v3(0., 0., 23.), 1.5, FLAME_LICK);
    }
    // A hot lintel between the towers, high above the lens.
    g.bx(fr, (-1.5, 1.5), (-edge - 1.6, edge + 1.6), (23., 25.), st.plate);
    g.bx(fr, (-1.55, 1.55), (-edge - 1., edge + 1.), (22.4, 22.8), SEAM);
}

/// A banner arch over the road at a section's start.
fn section_arch(g: &mut Soup, r: &dyn Route, s: f32) {
    let row = row(r, s);
    let fr = Fr::on(&row, 0.);
    let st = style(r.zone(s).0);
    let w = row.wl.max(row.wr) + 1.0;
    for sd in [-1.0f32, 1.0] {
        g.bx_open(&fr, (-0.4, 0.4), (sd * w - 0.45, sd * w + 0.45), (-2., 12.0), st.rail);
        g.bx(&fr, (-0.3, 0.3), (sd * w - 0.3, sd * w + 0.3), (12.0, 12.4), st.lamp);
    }
    // Above the lens: the chase eye rides eight metres up, and a beam at its height wipes the picture.
    g.bx(&fr, (-0.25, 0.25), (-w, w), (11.4, 12.2), st.rail_cap);
    g.quad(fr.pt(-0.28, -w, 11.4), fr.pt(-0.28, w, 11.4), fr.pt(-0.28, w, 9.8), fr.pt(-0.28, -w, 9.8), st.cloth);
}

/// A gate across a fork's road: two posts with lamps, and a banner over the road, above the lens.
fn fork_gate(g: &mut Soup, r: &dyn Route, line: u8, s: f32) {
    let row = row_on(r, line, s);
    let fr = Fr::on(&row, 0.);
    let st = style(r.forks()[line as usize - 1].look);
    let w = row.wl.max(row.wr) + 1.0;
    for sd in [-1.0f32, 1.0] {
        g.bx_open(&fr, (-0.5, 0.5), (sd * w - 0.5, sd * w + 0.5), (-2., 12.4), st.rail);
        g.bx(&fr, (-0.7, 0.7), (sd * w - 0.7, sd * w + 0.7), (12.4, 13.2), st.glow);
    }
    g.bx(&fr, (-0.35, 0.35), (-w, w), (11.6, 12.4), st.rail_cap);
    g.quad(fr.pt(-0.4, -w, 11.6), fr.pt(-0.4, w, 11.6), fr.pt(-0.4, w, 9.8), fr.pt(-0.4, -w, 9.8), st.cloth_b);
}
