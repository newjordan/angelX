//! The six engines. Seen from behind and above, which is how a race is watched, each one
//! draws its own mark: iron tubes, brass bells, standing hoops, needles, battlement blocks
//! and swept wings. All of them carry the same four things: a glowing intake at the nose,
//! a hot throat at the tail, the rider's colour in enamel, and a little flag.
//!
//! Nose at +x, axis at (y, z). The long plume of the exhaust is the FX lane's; the throat
//! here is the engine's own glow and a short hot stub at it.
use super::*;
use super::kit::*;

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum Lod { Far, Mid, Near }

#[derive(Clone, Copy)]
pub struct Rig<'a> {
    pub p: &'a Pod,
    pub lv: &'a Livery,
    pub t: f32,
    pub burn: f32,
    pub lod: Lod,
    pub flare: f32,
    pub snug: f32,
}

impl Rig<'_> {
    pub fn tall(&self, h: f32) -> f32 { h * (1. - 0.55 * self.snug) }
    pub fn lamp(&self) -> Glow { Glow::Lamp(self.lv.n) }
    fn sides(&self, near: usize, far: usize) -> usize { if self.lod == Lod::Far { far } else { near } }
}

pub fn throat(p: &Pod, side: f32) -> (V3, f32) {
    let r = match p.form { Form::Bell => p.r * 1.3, Form::Hoop => p.r * 0.32, Form::Keep => p.r * 0.55, Form::Needle => p.r * 0.8, _ => p.r * 0.8 };
    (v3(-p.len * 0.5, side * p.gap, p.z), r)
}

fn nozzle(m: &mut Mesh, g: &Rig, x: f32, y: f32, z: f32, r: f32) {
    let n = g.sides(8, 6);
    if g.lod >= Lod::Mid {
        band(m, [x, x + 0.4], [y, z], [r * 0.82, r * 1.04], n, IRON, IRON, STEEL);
        band(m, [x - 0.1, x + 0.06], [y, z], [r * 0.7, r * 0.9], n, BRASS, BRASS, BRASS);
    }
    let big = g.flare.min(8.);
    let wob = 1. + 0.1 * (g.t * 23. + y * 3.).sin();
    glowing(m, Glow::Hot, |m| disc(m, x - 0.02, [y, z], r * 0.8 * big.sqrt(), n, mat::EMBER));
    glowing(m, Glow::Core, |m| disc(m, x - 0.06, [y, z], r * 0.54 * big.sqrt(), n, mat::EMBER));
    let stub = ((0.35 + g.burn * 0.55) * r * 1.5 * wob).min(1.5);
    glowing(m, Glow::Hot, |m| spike(m, v3(x - 0.08, y, z), v3(x - 0.08 - stub, y, z), r * 0.62, mat::EMBER));
    glowing(m, Glow::Core, |m| spike(m, v3(x - 0.1, y, z), v3(x - 0.1 - stub * 0.55, y, z), r * 0.34, mat::EMBER));
}

fn intake(m: &mut Mesh, g: &Rig, x: f32, y: f32, z: f32, r: f32) {
    let n = g.sides(8, 6);
    if g.lod >= Lod::Mid { band(m, [x - 0.28, x], [y, z], [r * 0.64, r * 1.06], n, STEEL, GUN, STEEL); }
    glowing(m, Glow::Intake, |m| disc(m, x - 0.1, [y, z], r * 0.66, n, mat::CRYSTAL));
    if g.lod >= Lod::Mid { spike(m, v3(x - 0.1, y, z), v3(x + r * 0.9, y, z), r * 0.28, STEEL); }
}

fn flag(m: &mut Mesh, g: &Rig, x: f32, y: f32, z0: f32, h: f32) {
    let cl = g.lv.cloth;
    let sway = 0.1 * (g.t * 6. + y * 2.).sin();
    let h = g.tall(h);
    let top = z0 + h;
    cuboid(m, v3(x - 0.04, y - 0.04, z0 - 0.05), v3(x + 0.04, y + 0.04, top), STEEL);
    glowing(m, g.lamp(), |m| gem(m, v3(x, y, top + 0.1), 0.1 * g.flare.min(8.) * if g.lod == Lod::Far { 1.5 } else { 1. }, 0.18 * g.flare.min(8.) * if g.lod == Lod::Far { 1.5 } else { 1. }, V3::UP, mat::CRYSTAL));
    if g.lod < Lod::Mid { return; }
    let fold = 1. - 0.4 * g.snug;
    let w = (g.p.r + 0.15).min(0.62) * fold;
    let hh = 1.25 * (1. - 0.45 * g.snug);
    let bx = x - 0.06;
    let at = |dy: f32, dz: f32, s: f32| v3(bx - sway * s, y + dy, top - dz);
    match g.p.form {
        Form::Barrel => {
            m.push_quad(at(-w, 0.02, 0.), at(w, 0.02, 1.), at(w, hh, 1.), at(-w, hh, 0.), cl);
            tri(m, at(-w, hh, 0.), at(0., hh * 0.62, 1.), at(0., hh + 0.3, 0.), cl);
            tri(m, at(w, hh, 1.), at(0., hh * 0.62, 1.), at(0., hh + 0.3, 0.), cl);
            let (sx, e) = (w * 0.16, 0.012);
            m.push_quad(at(-sx, 0.02, 0.5) - v3(e, 0., 0.), at(sx, 0.02, 0.5) - v3(e, 0., 0.), at(sx, hh * 0.9, 0.5) - v3(e, 0., 0.), at(-sx, hh * 0.9, 0.5) - v3(e, 0., 0.), GUN);
            m.push_quad(at(-w, 0.02, 0.) - v3(e, 0., 0.), at(w, 0.02, 1.) - v3(e, 0., 0.), at(w, 0.12, 1.) - v3(e, 0., 0.), at(-w, 0.12, 0.) - v3(e, 0., 0.), GUN);
        }
        Form::Bell => {
            let (bw, drop) = (w * 1.1, hh * 0.55);
            cuboid(m, v3(bx - 0.04, y - bw - 0.1, top - 0.1), v3(bx + 0.04, y + bw + 0.1, top - 0.02), BRASS);
            m.push_quad(at(-bw, 0.1, 0.), at(bw, 0.1, 1.), at(bw * 0.85, drop, 1.), at(-bw * 0.85, drop, 0.), cl);
            tri(m, at(-bw * 0.85, drop, 0.), at(bw * 0.85, drop, 1.), at(0., hh * 1.15, 1.), cl);
        }
        Form::Hoop => {
            let rad = (0.46 * fold).max(0.3);
            let centre = (0., 0.1 + rad);
            let ring = |rad: f32, i: usize| { let a = i as f32 / 8. * TAU; (rad * a.cos(), centre.1 + rad * a.sin()) };
            for i in 0..8 {
                let (a, b) = (ring(rad + 0.1, i), ring(rad + 0.1, i + 1));
                tri(m, at(centre.0, centre.1, 0.) + v3(0.04, 0., 0.), at(a.0, a.1, 0.) + v3(0.04, 0., 0.), at(b.0, b.1, 0.) + v3(0.04, 0., 0.), STEEL);
                let (a, b) = (ring(rad, i), ring(rad, i + 1));
                tri(m, at(centre.0, centre.1, 0.5), at(a.0, a.1, 0.5), at(b.0, b.1, 0.5), cl);
            }
        }
        Form::Needle => {
            let (nw, len) = (0.13 * fold + 0.05, hh * 1.5);
            m.push_quad(at(-nw, 0.02, 0.), at(nw, 0.02, 1.), at(nw, len, 1.), at(-nw, len, 0.), cl);
            m.push_quad(at(-nw * 0.25, 0.02, 0.5) - v3(0.012, 0., 0.), at(nw * 0.25, 0.02, 0.5) - v3(0.012, 0., 0.), at(nw * 0.25, len * 0.9, 0.5) - v3(0.012, 0., 0.), at(-nw * 0.25, len * 0.9, 0.5) - v3(0.012, 0., 0.), GUN);
            tri(m, at(-nw, len, 0.), at(0., len * 0.82, 1.), at(-nw * 0.1, len + 0.35, 0.), cl);
            tri(m, at(nw, len, 1.), at(0., len * 0.82, 1.), at(nw * 0.1, len + 0.35, 1.), cl);
        }
        Form::Keep => {
            let bw = w * 0.95;
            let top_edge = 0.3;
            m.push_quad(at(-bw, top_edge, 0.), at(bw, top_edge, 1.), at(bw, hh * 0.85, 1.), at(-bw, hh * 0.85, 0.), cl);
            for k in 0..3 {
                let cx = (k as f32 - 1.) * bw * 0.72;
                let tw = bw * 0.26;
                m.push_quad(at(cx - tw, 0., 0.5), at(cx + tw, 0., 0.5), at(cx + tw, top_edge, 0.5), at(cx - tw, top_edge, 0.5), cl);
            }
            m.push_quad(at(-bw, hh * 0.85, 0.), at(bw, hh * 0.85, 1.), at(bw, hh * 0.85 + 0.1, 1.), at(-bw, hh * 0.85 + 0.1, 0.), GUN);
        }
        Form::Wyvern => {
            let len = hh * 0.8;
            for sg in [-1., 1.] {
                tri(m, at(0., 0.02, 0.), at(0., 0.3, 0.), at(sg * len * 0.55, 0.26, 1.), cl);
                tri(m, at(0., 0.3, 0.), at(0., 0.58, 0.), at(sg * len * 0.55, 0.26, 1.), cl);
            }
        }
    }
}

pub fn nacelle(m: &mut Mesh, g: &Rig, side: f32, y: f32, z: f32) {
    let (p, cl) = (g.p, g.lv.cloth);
    let (l, r) = (p.len, p.r);
    let near = g.lod >= Lod::Near;
    let mid = g.lod >= Lod::Mid;
    let flag_at = match p.form {
        Form::Barrel => (-l * 0.3, z + r + 0.6, 1.1),
        Form::Needle => (-l * 0.3, z + r + 0.6, 1.5),
        Form::Bell => (-l * 0.15, z + r * 0.62 + 0.1, 1.1),
        Form::Hoop => (0., z + r * 1.0, 0.9),
        Form::Keep => (-l * 0.3, z + r * 0.8 + 0.46 + 0.5, 0.9),
        Form::Wyvern => (-l * 0.1, z + r * 1.0 + 0.35, 0.9),
    };
    match p.form {
        Form::Barrel => {
            barrel(m, [-l * 0.44, l * 0.02], [y, z], [r * 0.9, r * 0.96], g.sides(8, 6), cl, true);
            barrel(m, [l * 0.02, l * 0.3], [y, z], [r * 0.96, r], g.sides(8, 6), IRON, true);
            barrel(m, [-l * 0.5, -l * 0.34], [y, z], [r * 1.12; 2], g.sides(8, 6), GUN, false);
            if mid {
                for x in [-l * 0.2, l * 0.12] { barrel(m, [x, x + 0.22], [y, z], [r * 1.07; 2], 8, BRASS, false); }
                barrel(m, [l * 0.3, l * 0.4], [y, z], [r * 1.12; 2], 8, STEEL, false);
            }
            if near {
                for i in 0..4 {
                    let x = -l * 0.3 + i as f32 * l * 0.12;
                    let h = 0.5 - 0.08 * (i as f32 - 1.5).abs();
                    cuboid(m, v3(x, y - 0.05, z + r * 0.9), v3(x + 0.16, y + 0.05, z + r + h), STEEL);
                    m.push_quad(v3(x, y - 0.08, z + r + h + 0.01), v3(x + 0.16, y - 0.08, z + r + h + 0.01), v3(x + 0.16, y + 0.08, z + r + h + 0.01), v3(x, y + 0.08, z + r + h + 0.01), BRASS);
                }
                for sg in [-1., 1.] {
                    let at = v3(-l * 0.14, y + sg * r * 0.72, z + r * 0.7);
                    tube(m, at, at + v3(0., 0., 0.75), 0.13, 5, IRON);
                    glowing(m, Glow::Hot, |m| disc_up(m, at + v3(0., 0., 0.76), 0.11, 5, mat::EMBER));
                }
            }
            intake(m, g, l * 0.4, y, z, r);
            nozzle(m, g, -l * 0.5, y, z, r * 0.92);
        }
        Form::Bell => {
            let bell = |x: f32| {
                let u = ((x + l * 0.5) / (l * 0.42)).clamp(0., 1.);
                r * (1.4 + (0.62 - 1.4) * u)
            };
            let n = g.sides(10, 6);
            barrel(m, [-l * 0.5, -l * 0.08], [y, z], [bell(-l * 0.5), bell(-l * 0.08)], n, BRASS, false);
            barrel(m, [-l * 0.5, -l * 0.4], [y, z], [r * 1.46, r * 1.26], n, cl, false);
            if mid { barrel(m, [-l * 0.27, -l * 0.21], [y, z], [r * 1.0; 2], n, GUN, false); }
            let rx = -l * 0.5 + l * 0.18;
            glowing(m, Glow::Hot, |m| disc(m, rx, [y, z], bell(rx) * 0.96, n, mat::EMBER));
            glowing(m, Glow::Core, |m| disc(m, rx - 0.04, [y, z], bell(rx) * 0.5 * g.flare.min(8.).sqrt(), n, mat::EMBER));
            if mid {
                let sw = 0.12 * (g.t * 7. + y).sin();
                tube(m, v3(rx + 0.1, y, z + bell(rx) * 0.9), v3(rx - 0.2, y + sw, z + 0.35), 0.05, 4, STEEL);
                gem(m, v3(rx - 0.25, y + sw, z + 0.18), r * 0.34, r * 0.4, V3::UP, STEEL);
                gem(m, v3(rx - 0.25, y + sw, z + 0.18), r * 0.34, r * 0.3, v3(1., 0., 0.), STEEL);
            }
            barrel(m, [-l * 0.08, l * 0.36], [y, z], [r * 0.62, r * 0.54], n, cl, true);
            if mid {
                for x in [0., l * 0.2] { barrel(m, [x, x + 0.2], [y, z], [r * 0.7; 2], 8, BRASS, false); }
                cuboid(m, v3(-l * 0.28, y - 0.1, z + r * 1.3), v3(-l * 0.22, y + 0.1, z + r * 1.9), GUN);
                cuboid(m, v3(-l * 0.34, y - 0.3, z + r * 1.9), v3(-l * 0.16, y + 0.3, z + r * 2.05), cl);
            }
            barrel(m, [l * 0.36, l * 0.5], [y, z], [r * 0.54, r * 0.3], 8, IRON, true);
            if near { for sg in [-1., 1.] { spike(m, v3(l * 0.38, y + sg * r * 0.4, z), v3(l * 0.5 + 0.9, y + sg * r * 0.7, z), 0.1, STEEL); } }
            glowing(m, Glow::Intake, |m| disc(m, l * 0.5 + 0.02, [y, z], r * 0.26, 8, mat::CRYSTAL));
            nozzle(m, g, -l * 0.5, y, z, r * 0.5);
        }
        Form::Hoop => {
            let n = g.sides(12, 8);
            band(m, [-l * 0.2, l * 0.2], [y, z], [r * 0.72, r], n, cl, GUN, STEEL);
            if mid { band(m, [-l * 0.3, -l * 0.21], [y, z], [r * 0.86, r * 1.1], n, BRASS, BRASS, BRASS); }
            glowing(m, Glow::Arc, |m| band(m, [-l * 0.02, l * 0.02], [y, z], [r * 0.6, r * 0.7], n, mat::CRYSTAL, mat::CRYSTAL, mat::CRYSTAL));
            barrel(m, [-l * 0.5, l * 0.5], [y, z], [r * 0.2; 2], 6, GUN, true);
            barrel(m, [-l * 0.16, l * 0.16], [y, z], [r * 0.36; 2], 6, cl, false);
            for i in 0..4 {
                let a = i as f32 / 4. * TAU + PI / 4.;
                let (c, s) = (a.cos(), a.sin());
                let b = |rad: f32, w: f32| v3(0., y + c * rad - s * w, z + s * rad + c * w);
                m.push_quad(b(r * 0.2, -0.1), b(r * 0.2, 0.1), b(r * 0.72, 0.1), b(r * 0.72, -0.1), STEEL);
            }
            if near {
                for i in 0..6 {
                    let a = i as f32 / 6. * TAU + 0.2;
                    let d = v3(0., a.cos(), a.sin());
                    let from = v3(0., y, z) + d * r;
                    spike(m, from, from + d * 0.18 + v3(0.1, 0., 0.), 0.08, GUN);
                    glowing(m, Glow::Intake, |m| gem(m, from + d * 0.26, 0.06, 0.08, d, mat::CRYSTAL));
                }
            }
            intake(m, g, l * 0.5, y, z, r * 0.42);
            nozzle(m, g, -l * 0.5, y, z, r * 0.4);
        }
        Form::Needle => {
            let n = g.sides(6, 5);
            barrel(m, [-l * 0.5, l * 0.28], [y, z], [r * 0.9, r], n, cl, true);
            barrel(m, [-l * 0.5, -l * 0.3], [y, z], [r * 1.22; 2], n, IRON, false);
            if mid { barrel(m, [l * 0.02, l * 0.1], [y, z], [r * 1.14; 2], n, BRASS, false); }
            barrel(m, [l * 0.28, l * 0.5 + 0.6], [y, z], [r * 0.92, r * 0.1], n, STEEL, false);
            glowing(m, Glow::Intake, |m| for sg in [-1., 1.] {
                cuboid(m, v3(l * 0.3, y + sg * r * 0.9 - 0.05, z - 0.06), v3(l * 0.44, y + sg * r * 0.96 + 0.05, z + 0.06), mat::CRYSTAL);
            });
            let base = v3(-l * 0.42, y, z + r * 0.9);
            let hgt = g.tall(1.5);
            m.push_quad(base + v3(0., -0.34, 0.), base + v3(0., 0.34, 0.), base + v3(-0.3, 0.07, hgt), base + v3(-0.3, -0.07, hgt), cl);
            glowing(m, g.lamp(), |m| { let k = g.flare.min(8.); gem(m, base + v3(-0.3, 0., hgt + 0.1 * k), 0.1 * k, 0.2 * k, V3::UP, mat::CRYSTAL); });
            let b = v3(-l * 0.32, y, z + r);
            m.push_quad(b, b + v3(l * 0.26, 0., 0.), b + v3(l * 0.07, 0., g.tall(0.8)), b + v3(-l * 0.13, 0., g.tall(0.8)), GUN);
            nozzle(m, g, -l * 0.5, y, z, r * 0.9);
        }
        Form::Keep => {
            let (w, h) = (r * 0.95, r * 0.8);
            cuboid(m, v3(-l * 0.5, y - w, z - h), v3(l * 0.5, y + w, z), IRON);
            cuboid(m, v3(-l * 0.5, y - w, z), v3(l * 0.5, y + w, z + h), cl);
            cuboid(m, v3(-l * 0.5 - 0.1, y - w * 1.06, z - h - 0.05), v3(l * 0.5 + 0.1, y + w * 1.06, z - h + 0.3), GUN);
            if mid { cuboid(m, v3(-l * 0.1, y - w * 1.04, z - h * 0.2), v3(l * 0.2, y + w * 1.04, z + h * 0.45), IRON); }
            let merlons = if g.lod == Lod::Far { 3 } else { 5 };
            for i in 0..merlons {
                let x = -l * 0.46 + i as f32 * (l * 0.92 / merlons as f32);
                cuboid(m, v3(x, y - w, z + h), v3(x + l * 0.11, y + w, z + h + 0.4), GUN);
                cuboid(m, v3(x, y - w, z + h + 0.4), v3(x + l * 0.11, y + w, z + h + 0.5), cl);
            }
            glowing(m, Glow::Intake, |m| cuboid(m, v3(l * 0.5, y - w * 0.16, z - h * 0.1), v3(l * 0.5 + 0.12, y + w * 0.16, z + h * 0.7), mat::CRYSTAL));
            spike(m, v3(l * 0.5, y, z), v3(l * 0.5 + 1.5, y, z - h * 0.2), w * 0.8, GUN);
            let (px, hw, hh) = (-l * 0.5 - 0.04, w * 0.72, h * 0.7);
            glowing(m, Glow::Hot, |m| m.push_quad(v3(px, y - hw, z - hh), v3(px, y + hw, z - hh), v3(px, y + hw, z + hh), v3(px, y - hw, z + hh), mat::EMBER));
            glowing(m, Glow::Core, |m| m.push_quad(v3(px - 0.02, y - hw * 0.5, z - hh * 0.5), v3(px - 0.02, y + hw * 0.5, z - hh * 0.5), v3(px - 0.02, y + hw * 0.5, z + hh * 0.5), v3(px - 0.02, y - hw * 0.5, z + hh * 0.5), mat::EMBER));
            if mid {
                for i in 0..4 {
                    let by = y - hw + hw * 2. * (i as f32 + 0.5) / 4.;
                    cuboid(m, v3(px - 0.1, by - 0.05, z - hh - 0.05), v3(px - 0.04, by + 0.05, z + hh + 0.05), IRON);
                }
                cuboid(m, v3(px - 0.1, y - hw - 0.05, z - 0.05), v3(px - 0.04, y + hw + 0.05, z + 0.05), IRON);
                let stub = ((0.3 + g.burn * 0.5) * hw * 1.2).min(1.3);
                glowing(m, Glow::Hot, |m| spike(m, v3(px - 0.12, y, z), v3(px - 0.12 - stub, y, z), hw * 0.7, mat::EMBER));
            }
            if near {
                for sg in [-1., 1.] {
                    let at = v3(-l * 0.42, y + sg * w * 0.8, z + h);
                    barrel(m, [at.x - 0.28, at.x + 0.28], [at.y, at.z + 0.1], [0.3; 2], 6, STEEL, false);
                    tube(m, at + v3(0., 0., 0.1), at + v3(0., 0., 0.8), 0.3, 6, STEEL);
                    spike(m, at + v3(0., 0., 0.8), at + v3(0., 0., 1.5), 0.33, cl);
                }
            }
        }
        Form::Wyvern => {
            let n = g.sides(6, 5);
            barrel(m, [-l * 0.5, l * 0.3], [y, z], [r * 0.7, r * 1.05], n, GUN, true);
            barrel(m, [l * 0.3, l * 0.46], [y, z], [r * 1.05, r * 0.4], n, STEEL, true);
            spike(m, v3(l * 0.46, y, z), v3(l * 0.46 + r * 2.0, y, z - r * 0.2), r * 0.4, STEEL);
            glowing(m, g.lamp(), |m| for sg in [-1., 1.] {
                cuboid(m, v3(l * 0.4, y + sg * r * 0.55 - 0.06, z + r * 0.35), v3(l * 0.46, y + sg * r * 0.55 + 0.06, z + r * 0.62), mat::CRYSTAL);
            });
            let (lat, up) = (0.76f32, g.tall(1.7));
            for sg in [side] {
                let at = |x: f32, u: f32| v3(x, y + sg * (r * 0.8 + lat * u), z + 0.05 + up * u);
                let root = |x: f32| at(x, 0.);
                let wrist = at(-l * 0.04, 0.55);
                let knuckle = at(-l * 0.3, 0.8);
                let tip = at(-l * 0.5 - 0.5, 1.0);
                let rear = at(-l * 0.5 - 0.2, 0.4);
                m.push_quad(root(l * 0.28), wrist, knuckle, root(-l * 0.1), cl);
                m.push_quad(root(-l * 0.1), knuckle, tip, rear, cl);
                tri(m, root(-l * 0.1), rear, root(-l * 0.4), cl);
                if mid {
                    tube(m, root(l * 0.28), wrist, 0.08, 4, STEEL);
                    tube(m, wrist, tip, 0.07, 4, STEEL);
                    tube(m, wrist, rear, 0.05, 4, STEEL);
                    spike(m, wrist, wrist + v3(0.6, sg * 0.1, 0.2), 0.1, GUN);
                }
            }
            let b = v3(-l * 0.3, y, z + r * 0.8);
            m.push_quad(b, b + v3(l * 0.45, 0., 0.), b + v3(l * 0.2, 0., g.tall(1.6)), b + v3(-l * 0.12, 0., g.tall(1.4)), cl);
            nozzle(m, g, -l * 0.5, y, z, r * 0.9);
        }
    }
    flag(m, g, flag_at.0, y, flag_at.1, flag_at.2 * 1.3);
}

fn disc_up(m: &mut Mesh, c: V3, r: f32, n: usize, k: u8) {
    let at = |i: usize| { let a = i as f32 / n as f32 * TAU; c + v3(r * a.cos(), r * a.sin(), 0.) };
    for i in 0..n { tri(m, c, at(i), at(i + 1), k); }
}
