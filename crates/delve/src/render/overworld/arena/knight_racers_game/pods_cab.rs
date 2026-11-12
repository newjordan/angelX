//! What the engines tow and what binds them: the arc between the engines, the tethers and
//! chains to the chariot, the chariot itself with the knight seated in it, and the pennant.
use super::*;
use super::engines::{Lod, Rig};
use super::kit::*;

/// Jagged energy between two points, re-rolled with the clock: a white core in a blue edge, laid
/// as two crossed ribbons so it reads from every side, with branches that jump off it.
pub fn arc(m: &mut Mesh, g: &Rig, a: V3, b: V3, seed: f32) {
    let t = g.t;
    let n = match g.lod { Lod::Far => 4, Lod::Mid => 6, Lod::Near => 8 };
    let span = (b - a).length();
    let amp = (0.3 + 0.1 * g.burn.min(1.6)) * (span / 2.4).clamp(0.7, 1.8);
    let grow = (g.flare * 1.2).min(6.);
    let (core, edge) = ((0.12 + 0.05 * g.burn.min(1.6)) * grow, (0.28 + 0.08 * g.burn.min(1.6)) * grow);
    let mut pts = vec![a];
    for i in 1..n {
        let u = i as f32 / n as f32;
        let hump = (u * PI).sin();
        let p = a.lerp(b, u) + v3(jitter(t, 31., seed + i as f32 * 1.7) * amp * 0.9, 0., jitter(t, 17., seed * 2.3 + i as f32 * 2.9) * amp * 1.2 * hump + (0.7 + 0.25 * g.burn.min(1.6)) * hump);
        pts.push(p);
    }
    pts.push(b);
    let ribbon = |m: &mut Mesh, w: f32, dx: f32, both: bool, g: Glow| {
        let from = m.tris.len();
        for pair in pts.windows(2) {
            let (p, q) = (pair[0] + v3(dx, 0., 0.), pair[1] + v3(dx, 0., 0.));
            let (up, side) = (v3(0., 0., w), v3(w, 0., 0.));
            m.push_quad(p - up, q - up, q + up, p + up, mat::CRYSTAL);
            if both { m.push_quad(p - side, q - side, q + side, p + side, mat::CRYSTAL); }
        }
        lit(m, from, g);
    };
    // The edge sits a hair behind the core (the race is watched from behind: the core wins).
    ribbon(m, edge, 0.04, false, Glow::ArcEdge);
    ribbon(m, core, -0.04, g.lod >= Lod::Mid, Glow::Arc);
    if g.lod < Lod::Near { return; }
    // Two forks that leap off the bolt and die.
    for k in 0..2 {
        let i = 2 + k * 4;
        let from = pts[i.min(pts.len() - 2)];
        let dir = v3(jitter(t, 13., seed + k as f32) * 0.8, 0., if k == 0 { 1. } else { -0.6 } * (0.5 + 0.4 * (jitter(t, 19., seed + 3. * k as f32) + 0.5)));
        let to = from + dir * (0.9 + span * 0.18);
        let mid = from.lerp(to, 0.5) + v3(0., 0., jitter(t, 29., seed + 7. + k as f32) * 0.3);
        let from_n = m.tris.len();
        for (p, q) in [(from, mid), (mid, to)] {
            let w = v3(0., 0., core * 0.5);
            let s = v3(core * 0.5, 0., 0.);
            m.push_quad(p - w, q - w, q + w, p + w, mat::CRYSTAL);
            m.push_quad(p - s, q - s, q + s, p + s, mat::CRYSTAL);
        }
        lit(m, from_n, Glow::Arc);
    }
}

/// An arc's electrode: a brass cap on the engine's inner face with a spark of light in it.
pub fn electrode(m: &mut Mesh, g: &Rig, at: V3, toward: f32) {
    let cap = (g.lod >= Lod::Mid) as usize;
    if cap == 1 { tube(m, at - v3(0., toward * 0.12, 0.), at + v3(0., toward * 0.42, 0.), 0.2, 6, BRASS); }
    let k = g.flare.min(5.);
    glowing(m, Glow::Arc, |m| gem(m, at + v3(0., toward * 0.5, 0.), 0.2 * k, 0.26 * k, v3(0., toward, 0.), mat::CRYSTAL));
}

/// A tow cable: a brass tether and, hung under it, a sagging run of chain links. The chain swings a little.
pub fn cable(m: &mut Mesh, g: &Rig, a: V3, b: V3, sag: f32) {
    tube(m, a, b, 0.11, 5, BRASS);
    // Eyes where it meets each end.
    for e in [a, b] { gem(m, e, 0.2, 0.16, V3::UP, STEEL); }
    if g.lod < Lod::Mid { return; }
    let swing = 0.12 * (g.t * 5. + a.y).sin();
    let low = a.lerp(b, 0.5) - v3(0., swing, sag);
    let n = (((b - a).length() / 0.5) as usize).max(3);
    let at = |u: f32| {
        let on = if u < 0.5 { a.lerp(low, u * 2.) } else { low.lerp(b, (u - 0.5) * 2.) };
        on - v3(0., 0., 0.06)
    };
    for i in 0..n {
        let (u0, u1) = (i as f32 / n as f32, (i as f32 + 0.66) / n as f32);
        let w = if i % 2 == 0 { v3(0., 0.15, 0.) } else { v3(0., 0., 0.15) };
        m.push_quad(at(u0) - w, at(u1) - w, at(u1) + w, at(u0) + w, STEEL);
    }
}

/// A mast with the seat's colour streaming sideways from it, flat to the road behind so the lens
/// sees all of it, in a swallowtail that ripples; a lamp on top. `dir` is +1 or -1 for the side
/// it flies to, `len` how far it reaches and `wid` how deep it is at the mast.
pub fn streamer(m: &mut Mesh, g: &Rig, base: V3, h: f32, dir: f32, len: f32, wid: f32) {
    let cl = g.lv.cloth;
    let top = base + v3(0., 0., h);
    tube(m, base, top, 0.06, 4, STEEL);
    let k = g.flare.min(8.) * if g.lod == Lod::Far { 1.5 } else { 1. };
    glowing(m, g.lamp(), |m| gem(m, top + v3(0., 0., 0.12 * k), 0.12 * k, 0.22 * k, V3::UP, mat::CRYSTAL));
    let bands = if g.lod == Lod::Far { 2 } else { 4 };
    let at = |u: f32, edge: f32| {
        let ripple = (g.t * 9. - u * 5. + base.y * 3.).sin();
        let deep = wid * (1. - 0.55 * u);
        v3(base.x - 0.05 - 0.12 * u * ripple, base.y + dir * u * len, top.z - 0.05 - edge * deep + 0.1 * u * ripple)
    };
    for i in 0..bands {
        let (u0, u1) = (i as f32 / bands as f32, (i + 1) as f32 / bands as f32);
        m.push_quad(at(u0, 0.), at(u1, 0.), at(u1, 1.), at(u0, 1.), cl);
        // A gunmetal binding along the head of the cloth, so it is cloth with an edge and not a flat tone.
        if g.lod >= Lod::Mid { m.push_quad(at(u0, 0.) + v3(0., 0., 0.012), at(u1, 0.) + v3(0., 0., 0.012), at(u1, 0.2) + v3(0., 0., 0.012), at(u0, 0.2) + v3(0., 0., 0.012), GUN); }
    }
    // The swallowtail: the end notched in.
    tri(m, at(1., 0.), at(1., 1.), at(0.8, 0.5), cl);
}

/// The chariot: a hull in iron with enamel over it, brass at the rails, a seat-back that carries
/// the rider's colour and tail lights, hover pods at its flanks, the knight in the seat, a lance and
/// two streamers. It stays low, so the lens sees the arc over it and the road beyond.
pub fn chariot(m: &mut Mesh, g: &Rig, x: f32, z: f32, s: f32) {
    let cl = g.lv.cloth;
    let near = g.lod >= Lod::Near;
    // Hull and the enamel shell over it, wider at the top than at the keel.
    slab(m, [x - 0.58 * s, x + 0.32 * s, -0.34 * s, 0.34 * s], z, [x - 0.64 * s, x + 0.38 * s, -0.5 * s, 0.5 * s], z + 0.42 * s, IRON);
    slab(m, [x - 0.64 * s, x + 0.38 * s, -0.5 * s, 0.5 * s], z + 0.42 * s, [x - 0.62 * s, x + 0.36 * s, -0.5 * s, 0.5 * s], z + 0.5 * s, cl);
    // The prow: a wedge of gunmetal in front of the cockpit.
    let (a, b) = (v3(x + 0.38 * s, -0.5 * s, z + 0.1 * s), v3(x + 0.38 * s, 0.5 * s, z + 0.1 * s));
    let tip = v3(x + 1.05 * s, 0., z + 0.18 * s);
    let up = v3(0., 0., 0.4 * s);
    tri(m, a, a + up, tip, GUN);
    tri(m, b, b + up, tip, GUN);
    tri(m, a + up, b + up, tip, STEEL);
    tri(m, a, b, tip, GUN);
    // The seat-back: a low board at the tail, iron with an enamel face, a brass cap, and the lamps.
    slab(m, [x - 0.7 * s, x - 0.6 * s, -0.42 * s, 0.42 * s], z + 0.08 * s, [x - 0.76 * s, x - 0.62 * s, -0.36 * s, 0.36 * s], z + 0.82 * s, cl);
    cuboid(m, v3(x - 0.78 * s, -0.38 * s, z + 0.8 * s), v3(x - 0.6 * s, 0.38 * s, z + 0.88 * s), BRASS);
    for sg in [-1., 1.] {
        cuboid(m, v3(x - 0.76 * s, sg * 0.42 * s - 0.03 * s, z + 0.1 * s), v3(x - 0.6 * s, sg * 0.42 * s + 0.03 * s, z + 0.84 * s), BRASS);
    }
    // A gunmetal plate let into the seat-back, between its lamps, with a brass border: the board is not one flat sheet of enamel.
    cuboid(m, v3(x - 0.80 * s, -0.25 * s, z + 0.14 * s), v3(x - 0.76 * s, 0.25 * s, z + 0.42 * s), GUN);
    cuboid(m, v3(x - 0.81 * s, -0.27 * s, z + 0.4 * s), v3(x - 0.77 * s, 0.27 * s, z + 0.44 * s), BRASS);
    let k = g.flare.min(5.);
    glowing(m, g.lamp(), |m| {
        cuboid(m, v3(x - 0.82 * s, -0.3 * s, z + 0.5 * s), v3(x - 0.74 * s, 0.3 * s, z + 0.5 * s + 0.1 * s * k), mat::CRYSTAL);
        for sg in [-1., 1.] {
            cuboid(m, v3(x - 0.84 * s, sg * 0.36 * s - 0.1 * s * k, z + 0.14 * s), v3(x - 0.7 * s, sg * 0.36 * s + 0.1 * s * k, z + 0.44 * s), mat::CRYSTAL);
        }
    });
    // Brass rails along the shell's edge, and the hover pods at the flanks with a cold glow under them.
    if g.lod >= Lod::Mid {
        for sg in [-1., 1.] {
            tube(m, v3(x - 0.62 * s, sg * 0.5 * s, z + 0.54 * s), v3(x + 0.36 * s, sg * 0.5 * s, z + 0.54 * s), 0.05 * s, 4, BRASS);
            barrel(m, [x - 0.5 * s, x + 0.12 * s], [sg * 0.66 * s, z + 0.1 * s], [0.3 * s; 2], 8, GUN, true);
            glowing(m, Glow::Intake, |m| disc(m, x - 0.51 * s, [sg * 0.66 * s, z + 0.1 * s], 0.2 * s, 8, mat::CRYSTAL));
            cuboid(m, v3(x - 0.5 * s, sg * 0.5 * s - 0.03 * s, z + 0.08 * s), v3(x + 0.1 * s, sg * 0.5 * s + 0.03 * s, z + 0.3 * s), IRON);
        }
    }
    // The yoke the knight steers by.
    if near {
        cuboid(m, v3(x + 0.1 * s, -0.3 * s, z + 0.54 * s), v3(x + 0.3 * s, 0.3 * s, z + 0.58 * s), GUN);
        tube(m, v3(x + 0.2 * s, 0., z + 0.5 * s), v3(x + 0.2 * s, 0., z + 0.7 * s), 0.04 * s, 4, GUN);
    }
    knight(m, g, v3(x - 0.12 * s, 0., z + 0.5 * s), s * 1.4);
    // Two streamers fly out from the seat-back's corners, one to each side.
    for sg in [-1., 1.] {
        // Folded in a little when the pod is snug to the lens, where these are the biggest flat marks on the picture.
        streamer(m, g, v3(x - 0.74 * s, sg * 0.46 * s, z + 0.84 * s), 0.9 * s, sg, 0.55 * s * (1. - 0.3 * g.snug), 0.36 * s * (1. - 0.4 * g.snug));
    }
}

/// A knight from the cockpit floor `k` up, `u` tall-ish, as the chase lens sees them: a cape and a
/// seat-colour pauldron on each shoulder, a great helm with the pod's crest, and the lance.
pub fn knight(m: &mut Mesh, g: &Rig, k: V3, u: f32) {
    let (cl, gl, p) = (g.lv.cloth, BRASS, g.p);
    let b = |m: &mut Mesh, lo: [f32; 3], hi: [f32; 3], c: u8| cuboid(m, k + v3(lo[0], lo[1], lo[2]) * u, k + v3(hi[0], hi[1], hi[2]) * u, c);
    let near = g.lod >= Lod::Mid;
    let full = g.lod >= Lod::Near;
    let far = g.lod == Lod::Far;
    // Cuirass with a tabard in front and a cape over the seat-back behind.
    // Gunmetal under the cloth: a bright steel back is the pale slab in the middle of the chase picture.
    b(m, [-0.16, -0.22, 0.0], [0.16, 0.22, 0.5], GUN);
    if !far {
        b(m, [-0.18, -0.25, 0.0], [0.18, 0.25, 0.13], cl);
        b(m, [0.16, -0.15, 0.1], [0.19, 0.15, 0.5], mat::BONE);
        // The cape the rider wears, in the seat's cloth, over the shoulders and down the back, a brass clasp at the neck.
        let sway = 0.03 * (g.t * 8.).sin();
        m.push_quad(k + v3(-0.19, -0.26, 0.54) * u, k + v3(-0.19, 0.26, 0.54) * u, k + v3(-0.27 - sway, 0.2, 0.02) * u, k + v3(-0.27 - sway, -0.2, 0.02) * u, cl);
        b(m, [-0.21, -0.05, 0.5], [-0.17, 0.05, 0.56], gl);
    }
    let wide = p.crest == Crest::Horns || p.crest == Crest::Crown;
    let (sh, hi) = if wide { (0.4, 0.6) } else { (0.35, 0.56) };
    for sg in [-1.0f32, 1.0] {
        b(m, [-0.15, sg * 0.2 - 0.06, 0.36], [0.13, sg * sh + 0.06, hi], cl);
        if full { b(m, [-0.16, sg * 0.2 - 0.07, hi], [0.14, sg * sh + 0.07, hi + 0.05], STEEL); }
    }
    // The great helm: steel, a brass band, a dark visor slit, the crest.
    if !far { b(m, [-0.09, -0.12, 0.5], [0.09, 0.12, 0.57], STEEL); }
    b(m, [-0.14, -0.14, 0.57], [0.14, 0.14, 0.88], STEEL);
    if near {
        b(m, [-0.145, -0.145, 0.7], [0.145, 0.145, 0.74], gl);
        b(m, [0.14, -0.1, 0.77], [0.16, 0.1, 0.8], GUN);
    }
    let top = v3(0., 0., 0.88);
    let sp = |m: &mut Mesh, from: V3, to: V3, w: f32, c: u8| spike(m, k + from * u, k + to * u, w * u, c);
    match p.crest {
        Crest::Plume => {
            sp(m, top, v3(-0.38, 0., 1.26), 0.1, cl);
            sp(m, top + v3(0.04, 0., 0.), v3(-0.14, 0., 1.04), 0.07, gl);
        }
        Crest::Horns => {
            for sg in [-1.0f32, 1.0] {
                sp(m, v3(0., sg * 0.12, 0.82), v3(0.1, sg * 0.4, 1.18), 0.09, STEEL);
            }
            b(m, [-0.33, -0.3, 0.02], [-0.26, 0.3, 0.62], STEEL);
            b(m, [-0.35, -0.25, 0.08], [-0.31, 0.25, 0.56], cl);
        }
        Crest::Hood => {
            sp(m, v3(0., 0., 0.82), v3(-0.1, 0., 1.38), 0.16, cl);
            let (a, c) = (k + v3(-0.2, 0., 0.5) * u, k + v3(-0.84, 0., 0.16) * u);
            m.push_quad(a + v3(0., -0.26, 0.) * u, c + v3(0., -0.36, 0.) * u, c + v3(0., 0.36, 0.) * u, a + v3(0., 0.26, 0.) * u, cl);
        }
        Crest::Crown => {
            for i in 0..5 {
                let a = i as f32 / 5. * TAU;
                let at = v3(0.08 * a.cos(), 0.1 * a.sin(), 0.88);
                sp(m, at, at + v3(0.04 * a.cos(), 0.05 * a.sin(), 0.24), 0.05, gl);
            }
            sp(m, top + v3(-0.04, 0., 0.), v3(-0.3, 0., 1.32), 0.09, cl);
            b(m, [-0.2, -0.42, 0.3], [-0.12, 0.42, 0.52], cl);
        }
    }
    if p.lance && near {
        // Couched at the right side, pointing forward, with the rider's pennon at its head.
        let base = k + v3(-0.2, -0.38, 0.28) * u;
        let tip = base + v3(1.9 * u, 0., 0.25 * u);
        tube(m, base, tip, 0.04 * u, 4, STEEL);
        spike(m, tip, tip + v3(0.3 * u, 0., 0.04 * u), 0.05 * u, STEEL);
        if near {
            let at = |f: f32, d: f32| base.lerp(tip, f) + v3(0., 0.05 * u * (g.t * 10. + f * 6.).sin(), d * 0.2 * u);
            m.push_quad(at(0.78, 0.), at(1., 0.), at(1., -0.5), at(0.78, -0.42), cl);
        }
    }
}
