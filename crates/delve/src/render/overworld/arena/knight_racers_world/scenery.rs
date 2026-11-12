//! Set pieces beside the road: stands and their crowds, foundries, falls of
//! water and lava, statues, lanterns, pylons, ramparts and spires.
use super::geo::{Fr, Soup};
use super::palette::*;
use super::pieces::Ctx;
use super::route::Kind;
use crate::render::world3d::math::{V3, v3};
use std::f32::consts::TAU;

pub fn beside(g: &mut Soup, c: &Ctx) {
    match c.p.kind {
        Kind::Grandstand => grandstand(g, c),
        Kind::Foundry => foundry(g, c),
        Kind::Waterfall => fall(g, c, WATER, SPRAY),
        Kind::LavaFall => fall(g, c, LAVA, LAVA_HOT),
        Kind::Statues => statues(g, c),
        Kind::Lanterns => lanterns(g, c),
        Kind::Pylons => pylons(g, c),
        Kind::Rampart => rampart(g, c),
        Kind::Spires => spires(g, c),
        Kind::Stacks => stacks(g, c),
        Kind::Anvil => anvil(g, c),
        _ => {}
    }
}

/// The sides a piece stands on: its own, or both when it spans (side 0).
fn sides(c: &Ctx) -> Vec<f32> { if c.p.side == 0. { vec![-1., 1.] } else { vec![c.p.side.signum()] } }

/// How far to sink a footing at `fr` so it reaches the land, within reason.
fn sink(c: &Ctx, p: V3, road: f32) -> f32 { (road - c.ground(p)).clamp(0.5, 24.) + 1.0 }

fn grandstand(g: &mut Soup, c: &Ctx) {
    let seg = 6.0;
    let n = (c.len() / seg).ceil() as i32;
    let tiers = 5;
    for sd in sides(c) {
        for k in 0..n {
            let s = c.p.s0 + (k as f32 + 0.5) * seg;
            if !c.has(s) { continue; }
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let l0 = sd * (row.half(sd) + 4.5);
            let foot = fr.pt(0., l0 + sd * 6., 0.);
            let down = sink(c, foot, row.h);
            for t in 0..tiers {
                let (la, lb) = (l0 + sd * 2.2 * t as f32, l0 + sd * 2.2 * (t as f32 + 1.));
                let top = 1.4 * (t as f32 + 1.);
                g.bx_open(&fr, (-seg * 0.5 - 0.05, seg * 0.5 + 0.05), (la.min(lb), la.max(lb)), (-down, top), if t % 2 == 0 { c.st.timber } else { c.st.masonry });
                // The crowd: upright figures along the tier, facing the road.
                let per = 5;
                for i in 0..per {
                    let a = -seg * 0.5 + (i as f32 + 0.5) * seg / per as f32;
                    let h = c.rnd(k * 13 + i, t * 7 + sd as i32);
                    let col = if c.st.beat == 1 { if h < 0.1 { c.st.crowd[4] } else if h < 0.55 { c.st.crowd[0] } else { c.st.crowd[1] } } else { [c.st.crowd[0], c.st.crowd[1], c.st.crowd[2], c.st.crowd[3], c.st.crowd[0], c.st.crowd[1], c.st.crowd[4], c.st.crowd[5]][((h * 8.) as usize).min(7)] };
                    let lf = la + sd * 0.55;
                    let hh = 1.2 + 0.4 * c.rnd(i, k + t);
                    g.quad(fr.pt(a - 0.38, lf, top), fr.pt(a + 0.38, lf, top), fr.pt(a + 0.38, lf, top + hh), fr.pt(a - 0.38, lf, top + hh), col);
                }
            }
            // Roof on posts over the top tier, and a banner on the front post.
            let (lt, ltop) = (l0 + sd * 2.2 * tiers as f32, 1.4 * tiers as f32);
            if k % 2 == 0 {
                for l in [l0 + sd * 1.0, lt - sd * 0.4] {
                    g.bx_open(&fr, (-0.25, 0.25), (l - 0.25, l + 0.25), (-down, ltop + 4.8), c.st.timber);
                }
                g.bx(&fr, (-0.3, 0.3), (l0.min(lt), l0.max(lt)), (ltop + 4.6, ltop + 5.0), c.st.rail);
                g.bx(&fr, (-0.2, 0.2), (l0 + sd * 0.8, l0 + sd * 1.2), (ltop + 3.3, ltop + 4.5), c.st.glow);
            }
            g.slab(&fr, (-seg * 0.5 - 0.05, seg * 0.5 + 0.05), (l0.min(lt), l0.max(lt)), (ltop + 4.9, ltop + 4.9), 0.3, c.st.slate);
        }
    }
}

fn foundry(g: &mut Soup, c: &Ctx) {
    for sd in sides(c) {
        let (s0, s1) = (c.p.s0, c.p.s1);
        let mid = (s0 + s1) * 0.5;
        let row = c.row(mid);
        let fr = Fr::on(&row, 0.);
        let l0 = sd * (row.half(sd) + 7.0);
        let depth = 16.0;
        let hl = (c.len() * 0.5).min(30.);
        let foot = fr.pt(0., l0 + sd * depth, 0.);
        let down = sink(c, foot, row.h);
        let (la, lb) = (l0.min(l0 + sd * depth), l0.max(l0 + sd * depth));
        g.bx_open(&fr, (-hl, hl), (la, lb), (-down, 12.), c.st.plate_b);
        g.slab(&fr, (-hl - 1., hl + 1.), (la - 1., lb + 1.), (12.6, 14.), 0.5, c.st.plate);
        // Furnace mouths and a row of lit windows on the road face.
        let face = l0;
        let n = ((hl * 2.) / 9.).max(2.) as i32;
        for i in 0..n {
            let a = -hl + (i as f32 + 0.5) * 2. * hl / n as f32;
            g.quad(fr.pt(a - 2.0, face, 0.2), fr.pt(a + 2.0, face, 0.2), fr.pt(a + 2.0, face, 4.6), fr.pt(a - 2.0, face, 4.6), LAVA);
            g.quad(fr.pt(a - 2.4, face, 0.1), fr.pt(a - 2.0, face, 0.1), fr.pt(a - 2.0, face, 5.0), fr.pt(a - 2.4, face, 5.0), c.st.plate);
            g.quad(fr.pt(a - 1.2, face, 8.0), fr.pt(a + 1.2, face, 8.0), fr.pt(a + 1.2, face, 10.2), fr.pt(a - 1.2, face, 10.2), c.st.glow);
            // A chimney behind each other furnace.
            if i % 2 == 0 {
                let base = fr.pt(a, l0 + sd * 10., 12.);
                g.column(base, 20. + 4. * c.rnd(i, 0), 2.2, 1.5, 8, c.st.plate_b);
                g.column(v3(base.x, base.y, base.z + 20. + 4. * c.rnd(i, 0)), 0.8, 1.7, 1.7, 8, LAVA);
                g.spike(v3(base.x, base.y, base.z + 20.8 + 4. * c.rnd(i, 0)), v3(base.x, base.y, base.z + 25. + 4. * c.rnd(i, 0)), 1.3, LAVA_HOT);
            }
        }
        // Pipes along the face.
        for z in [6.6, 7.2] {
            g.bx(&fr, (-hl, hl), (face - sd * 0.9 - 0.25, face - sd * 0.9 + 0.25), (z, z + 0.5), c.st.plate);
        }
        // A trough of molten metal along the foot, toward the road.
        g.quad(fr.pt(-hl, face - sd * 1.6, 0.06), fr.pt(hl, face - sd * 1.6, 0.06), fr.pt(hl, face - sd * 3.4, 0.06), fr.pt(-hl, face - sd * 3.4, 0.06), LAVA);
    }
}

fn fall(g: &mut Soup, c: &Ctx, sheet: u8, foam: u8) {
    for sd in sides(c) {
        let row = c.row(c.mid());
        let fr = Fr::on(&row, 0.);
        let hl = c.len() * 0.5;
        let face = sd * (row.half(sd) + 9.0);
        let top = 34.0;
        // The cliff it falls from, in strata.
        let foot = fr.pt(0., face, 0.);
        let down = sink(c, foot, row.h);
        let n = 8;
        for i in 0..n {
            let (a0, a1) = (-hl + 2. * hl * i as f32 / n as f32, -hl + 2. * hl * (i + 1) as f32 / n as f32);
            let (t0, t1) = (top * (0.8 + 0.4 * c.rnd(i, 3)), top * (0.8 + 0.4 * c.rnd(i + 1, 3)));
            g.wall(fr.pt(a0, face, -down), fr.pt(a1, face, -down), fr.pt(a1, face, t1), fr.pt(a0, face, t0), c.st.rock);
        }
        // Three sheets and a pool at the foot.
        for (k, a) in [-0.45f32, 0.0, 0.5].iter().enumerate() {
            let a = a * hl;
            let w = 3.0 + 1.5 * c.rnd(k as i32, 9);
            g.wall(fr.pt(a - w, face - sd * 0.3, 0.2), fr.pt(a + w, face - sd * 0.3, 0.2), fr.pt(a + w * 0.8, face - sd * 0.3, top * 0.9), fr.pt(a - w * 0.8, face - sd * 0.3, top * 0.9), sheet);
            g.wall(fr.pt(a - w * 0.25, face - sd * 0.5, 0.4), fr.pt(a + w * 0.25, face - sd * 0.5, 0.4), fr.pt(a + w * 0.2, face - sd * 0.5, top * 0.85), fr.pt(a - w * 0.2, face - sd * 0.5, top * 0.85), foam);
        }
        g.quad(fr.pt(-hl, face - sd * 0.2, 0.05), fr.pt(hl, face - sd * 0.2, 0.05), fr.pt(hl, face - sd * 5.5, 0.05), fr.pt(-hl, face - sd * 5.5, 0.05), sheet);
    }
}

fn statues(g: &mut Soup, c: &Ctx) {
    let n = ((c.len() / 26.) as i32).max(1);
    for sd in sides(c) {
        for k in 0..n {
            let s = c.p.s0 + (k as f32 + 0.5) * c.len() / n as f32;
            if !c.has(s) { continue; }
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let l = sd * (row.half(sd) + 7.0);
            let down = sink(c, fr.pt(0., l, 0.), row.h);
            // Pedestal, legs, tabard, arms, helm, sword and shield: a knight twelve metres tall.
            g.bx_open(&fr, (-2.2, 2.2), (l - 2.2, l + 2.2), (-down, 2.6), c.st.masonry);
            g.bx(&fr, (-2.5, 2.5), (l - 2.5, l + 2.5), (2.6, 3.2), c.st.masonry);
            for dl in [-0.9, 0.9] { g.bx(&fr, (-0.7, 0.7), (l + dl - 0.55, l + dl + 0.55), (3.2, 7.4), c.st.rail); }
            g.bx(&fr, (-1.1, 1.1), (l - 1.9, l + 1.9), (7.4, 11.4), c.st.rail);
            g.bx(&fr, (-1.15, 1.15), (l - 1.3, l + 1.3), (7.4, 10.2), c.st.cloth);
            for dl in [-2.5, 2.5] { g.bx(&fr, (-0.55, 0.55), (l + dl - 0.55, l + dl + 0.55), (7.8, 11.0), c.st.rail); }
            g.bx(&fr, (-0.8, 0.8), (l - 0.8, l + 0.8), (11.4, 13.4), c.st.rail);
            g.bx(&fr, (0.79, 0.85), (l - 0.55, l + 0.55), (12.1, 12.5), c.st.glow);
            g.bx(&fr, (-0.12, 0.12), (l + sd * 2.9 - 0.12, l + sd * 2.9 + 0.12), (3.6, 16.), STEEL);
            g.spike(fr.pt(0., l + sd * 2.9, 16.), fr.pt(0., l + sd * 2.9, 18.4), 0.3, STEEL);
            g.bx(&fr, (0.3, 0.5), (l - sd * 3.4 - 0.9, l - sd * 3.4 + 0.9), (7.8, 11.), c.st.cloth_b);
        }
    }
}

fn lanterns(g: &mut Soup, c: &Ctx) {
    let n = (c.len() / 14.) as i32;
    for sd in sides(c) {
        for k in 0..=n {
            if !c.has(c.p.s0 + k as f32 * 14.) { continue; }
            let row = c.row(c.p.s0 + k as f32 * 14.);
            let fr = Fr::on(&row, 0.);
            let l = sd * (row.half(sd) + 2.2);
            g.bx_open(&fr, (-0.2, 0.2), (l - 0.2, l + 0.2), (-1., 6.5), c.st.post);
            g.bx(&fr, (-0.15, 0.15), (l - sd * 1.4, l), (6.2, 6.5), c.st.post);
            g.bx(&fr, (-0.35, 0.35), (l - sd * 1.8 - 0.35, l - sd * 1.8 + 0.35), (5.1, 6.1), c.st.glow);
            g.bx(&fr, (-0.4, 0.4), (l - sd * 1.8 - 0.4, l - sd * 1.8 + 0.4), (6.1, 6.2), c.st.plate_b);
        }
    }
}

fn pylons(g: &mut Soup, c: &Ctx) {
    let n = (c.len() / 20.) as i32;
    for sd in sides(c) {
        for k in 0..=n {
            if !c.has(c.p.s0 + k as f32 * 20.) { continue; }
            let row = c.row(c.p.s0 + k as f32 * 20.);
            let fr = Fr::on(&row, 0.);
            let l = sd * (row.half(sd) + 3.0);
            let h = 14. + 4. * c.rnd(k, 5);
            g.bx_open(&fr, (-0.8, 0.8), (l - 0.8, l + 0.8), (-1.5, h), c.st.rail);
            g.bx(&fr, (-1.1, 1.1), (l - 1.1, l + 1.1), (h, h + 0.6), c.st.rail_cap);
            g.spike(fr.pt(0., l, h + 0.6), fr.pt(0., l, h + 4.5), 0.7, c.st.lamp);
        }
    }
}

fn rampart(g: &mut Soup, c: &Ctx) {
    let seg = 6.0;
    let n = (c.len() / seg).ceil() as i32;
    for sd in sides(c) {
        for k in 0..n {
            if !c.has(c.p.s0 + (k as f32 + 0.5) * seg) { continue; }
            let row = c.row(c.p.s0 + (k as f32 + 0.5) * seg);
            let fr = Fr::on(&row, 0.);
            let l0 = sd * (row.half(sd) + 6.0);
            let l1 = l0 + sd * 2.6;
            let down = sink(c, fr.pt(0., l1, 0.), row.h);
            let (la, lb) = (l0.min(l1), l0.max(l1));
            g.bx_open(&fr, (-seg * 0.5 - 0.02, seg * 0.5 + 0.02), (la, lb), (-down, 9.), c.st.masonry);
            if k % 2 == 0 { g.bx(&fr, (-seg * 0.5, 0.0), (la - 0.2, lb + 0.2), (9., 10.2), c.st.masonry); }
            else { g.bx(&fr, (0.0, seg * 0.5), (la - 0.2, lb + 0.2), (9., 10.2), c.st.masonry); }
            if k % 5 == 2 {
                let (lc0, lc1) = (l0 - sd * 1.0, l1 + sd * 1.0);
                g.bx_open(&fr, (-3., 3.), (lc0.min(lc1), lc0.max(lc1)), (-down, 16.), c.st.masonry);
                g.spike(fr.pt(0., (lc0 + lc1) * 0.5, 16.), fr.pt(0., (lc0 + lc1) * 0.5, 23.), 3.6, c.st.slate);
                g.quad(fr.pt(-3.02, l0 - sd * 0.6, 13.), fr.pt(-3.02, l0 + sd * 0.6, 13.), fr.pt(-3.02, l0 + sd * 0.6, 10.6), fr.pt(-3.02, l0 - sd * 0.6, 10.6), c.st.glow);
                g.quad(fr.pt(-3.04, l0 - sd * 1.4, 8.6), fr.pt(-3.04, l0 + sd * 1.4, 8.6), fr.pt(-3.04, l0 + sd * 1.4, 3.4), fr.pt(-3.04, l0 - sd * 1.4, 3.4), if k % 10 == 2 { c.st.cloth } else { c.st.cloth_b });
            }
        }
    }
}

fn spires(g: &mut Soup, c: &Ctx) {
    let n = (c.len() / 7.) as i32;
    for sd in sides(c) {
        for k in 0..n {
            let s = c.p.s0 + (k as f32 + c.rnd(k, 1)) * c.len() / n as f32;
            if !c.has(s) { continue; }
            let row = c.row(s);
            let fr = Fr::on(&row, 0.);
            let l = sd * (row.half(sd) + 5. + 30. * c.rnd(k, 2).powi(2));
            let base = fr.pt(0., l, 0.);
            let z = c.ground(base) - 1.;
            let h = 8. + 22. * c.rnd(k, 3);
            let r = 1.2 + 2.4 * c.rnd(k, 4);
            let lean = v3((c.rnd(k, 5) - 0.5) * 3., (c.rnd(k, 6) - 0.5) * 3., 0.);
            g.spike(v3(base.x, base.y, z), v3(base.x, base.y, z + h) + lean, r, if k % 3 == 0 && !c.st.wall_glow { CRYSTAL } else { c.st.rock });
            if c.st.wall_glow { g.spike(v3(base.x, base.y, z + h * 0.82) + lean * 0.82, v3(base.x, base.y, z + h) + lean, r * 0.28, c.st.hot); }
        }
    }
}

/// Crystals standing in a vault: faceted prisms to the roof, in clusters along both walls, cyan
/// and violet, with the pools of their light on the road. Seen from far down the dark.
pub fn crystals(g: &mut Soup, c: &Ctx) {
    let n = (c.len() / 5.) as i32;
    for k in 0..n {
        let s = c.p.s0 + (k as f32 + 0.5) * c.len() / n as f32;
        if !c.has(s) { continue; }
        let row = c.row(s);
        let fr = Fr::on(&row, 0.);
        for sd in [-1.0f32, 1.0] {
            if c.rnd(k, (sd as i32) + 3) < 0.35 { continue; }
            let l = sd * (row.half(sd) + 1.4 + 1.8 * c.rnd(k, 8));
            let base = fr.pt(0., l, -0.5);
            let h = 3. + 9. * c.rnd(k, 10 + sd as i32);
            let r = 0.5 + 0.8 * c.rnd(k, 12);
            let a = c.rnd(k, 14) * TAU;
            let violet = c.rnd(k, 15 + (sd as i32 + 1)) < 0.42;
            let (fa, fb, tip) = if violet { (VIOLET_DIM, VIOLET_FACET, VIOLET_GLOW) } else { (CYAN_DIM, CYAN_FACET, CYAN_GLOW) };
            super::trackside::facet(g, base, h + r * 3., r * 1.4, v3(a.cos() * 0.9, a.sin() * 0.9, 0.), fa, fb, tip);
            // Its light on the road, by the foot of the wall.
            if k % 2 == 0 {
                let pc = fr.pt(0., sd * (row.half(sd) - 1.6), 0.05);
                let (lv, fv) = (fr.l * (2.4 * sd), fr.f * 2.6);
                g.quad(pc - lv - fv, pc + lv - fv, pc + lv + fv, pc - lv + fv, if violet { VIOLET_POOL } else { CYAN_POOL });
            }
        }
        // Hanging from the roof.
        if k % 2 == 0 {
            let l = (c.rnd(k, 20) - 0.5) * row.wl.min(row.wr) * 1.6;
            let top = fr.pt(0., l, 14.);
            g.spike(top, top - v3(0., 0., 3.0 + 3.5 * c.rnd(k, 21)), 0.6 + 0.4 * c.rnd(k, 22), if k % 4 == 0 { VIOLET_FACET } else { CYAN_FACET });
        }
    }
}
const BONE_MAT: u8 = crate::render::world3d::mesh::mat::BONE;

/// Where a smelter's stack stands: along a stretch, one every sixty metres, alternate sides and
/// distances, far enough off that the whole of it is in the lens. Also read by the sparks.
pub fn stack_at(c: &Ctx, k: i32) -> Option<(V3, f32)> {
    let s = c.p.s0 + (k as f32 + 0.5) * 60.;
    if s > c.p.s1 || !c.has(s) { return None; }
    let row = c.row(s);
    let sd = if k % 2 == 0 { 1.0 } else { -1.0 };
    let l = sd * (row.half(sd) + 42. + 26. * c.rnd(k, 41));
    let fr = Fr::on(&row, 0.);
    let p = fr.pt(0., l, 0.);
    Some((v3(p.x, p.y, c.ground(p).max(row.h - 22.)), 46. + 40. * c.rnd(k, 42)))
}

/// A smelter: a brick house with glowing windows and a tall stack that burns at the crown, so a
/// stretch of road is known from far off by the fires on its skyline.
fn stacks(g: &mut Soup, c: &Ctx) {
    let n = (c.len() / 60.).ceil() as i32;
    for k in 0..n {
        let Some((base, h)) = stack_at(c, k) else { continue };
        let b = v3(base.x, base.y, base.z - 3.);
        // The house, its roof and its windows (on all four faces: it is seen from the road at any angle).
        g.cuboid(b + v3(-9., -9., 0.), b + v3(9., 9., 15.), c.st.masonry);
        g.cuboid(b + v3(-10., -10., 15.), b + v3(10., 10., 16.2), c.st.plate_b);
        for (i, z) in [4.0f32, 9.0].iter().enumerate() {
            for (dx, dy) in [(9.04f32, 0.), (-9.04, 0.), (0., 9.04), (0., -9.04)] {
                let (ax, ay) = if dx != 0. { (0., 1.) } else { (1., 0.) };
                let m = if (i + k as usize) % 2 == 0 { WINDOW_HOT } else { WINDOW_DIM };
                for off in [-4.5f32, 0., 4.5] {
                    let o = v3(dx + ax * off, dy + ay * off, *z);
                    let w = v3(ax, ay, 0.) * 1.4;
                    g.quad(b + o - w, b + o + w, b + o + w + v3(0., 0., 3.), b + o - w + v3(0., 0., 3.), m);
                }
            }
        }
        // The stack: a tapering column on the house, a hot band at the crown and a flame.
        let top = b + v3(0., 0., 16.2);
        g.column(top, h, 3.4, 2.3, 8, c.st.slate);
        g.column(top + v3(0., 0., h - 3.), 1.2, 2.6, 2.6, 8, SEAM);
        g.column(top + v3(0., 0., h), 0.9, 2.5, 2.5, 8, LAVA);
        g.spike(top + v3(0., 0., h + 0.9), top + v3(0., 0., h + 12. + 5. * c.rnd(k, 43)), 2.2, FLAME_LICK);
        g.spike(top + v3(0., 0., h + 0.9), top + v3(0., 0., h + 6.), 1.4, EMBER_BRIGHT);
    }
}

/// The Hammer Turn's monument: an anvil the size of a hall on the outside of the bend, a hammer
/// as tall as a tower laid across it, the metal between them white hot.
fn anvil(g: &mut Soup, c: &Ctx) {
    let mid = c.mid();
    let row = c.row(mid);
    let fr = Fr::on(&row, 0.);
    let l = c.p.side * (row.half(c.p.side) + 48.);
    let p = fr.pt(0., l, 0.);
    let z = c.ground(v3(p.x, p.y, 0.)).max(row.h - 12.) - 4.;
    let o = v3(p.x, p.y, z);
    // Foot, waist, face and horn.
    g.cuboid(o + v3(-9., -6., 0.), o + v3(9., 6., 7.), c.st.plate_b);
    g.cuboid(o + v3(-5., -3.6, 7.), o + v3(5., 3.6, 14.), c.st.plate);
    g.cuboid(o + v3(-14., -5.2, 14.), o + v3(14., 5.2, 20.), c.st.plate_b);
    g.spike(o + v3(14., 0., 17.), o + v3(30., 0., 17.), 3.2, c.st.plate);
    // The billet on the face, and the heat round it.
    g.cuboid(o + v3(-6., -3., 20.), o + v3(6., 3., 21.4), LAVA_BRIGHT);
    g.cuboid(o + v3(-14.2, -5.4, 19.), o + v3(14.2, 5.4, 19.4), SEAM);
    // The hammer, raised: a haft leaning up and away, a head of iron with a hot rim.
    let haft_top = o + v3(-12., 0., 62.);
    let haft_foot = o + v3(-3., 0., 21.4);
    let n = 6;
    for i in 0..n {
        let (a, b) = (haft_foot.lerp(haft_top, i as f32 / n as f32), haft_foot.lerp(haft_top, (i + 1) as f32 / n as f32));
        g.cuboid(a + v3(-0.9, -0.9, 0.), b + v3(0.9, 0.9, 0.), c.st.timber);
    }
    let h = haft_top;
    g.cuboid(h + v3(-6., -4., -3.), h + v3(6., 4., 8.), c.st.plate);
    g.cuboid(h + v3(-6.2, -4.2, -3.2), h + v3(6.2, 4.2, -2.4), SEAM);
    g.cuboid(h + v3(-6.2, -4.2, 7.2), h + v3(6.2, 4.2, 8.), SEAM);
    g.cuboid(h + v3(-5., -3., 8.), h + v3(5., 3., 9.), LAVA_BRIGHT);
}
