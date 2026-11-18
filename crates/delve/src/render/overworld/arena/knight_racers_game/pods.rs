//! The pod roster: six original rigs, each two engines towing a knight's
//! chariot on tethers and chains with an arc crackling between the engines,
//! and the six seat liveries that tell the riders apart.
//!
//! A pod's *look* (which rig it is) and a seat's *livery* (whose colours it
//! wears) are separate: the garage picks the first, the seat number the
//! second, so six seats of one rig still read as six riders.
//!
//! The drawing is in three files beside this one: `pods_kit.rs` (shapes and the lights
//! a pod burns), `pods_engines.rs` (the six engines) and `pods_cab.rs` (the arc, the
//! tethers, the chariot, the knight and the pennants). This one picks what is drawn at
//! what distance and sets the pod down on the road.
use super::*;

#[path = "pods_kit.rs"]
mod kit;
#[path = "pods_engines.rs"]
mod engines;
#[path = "pods_cab.rs"]
mod cab;

use kit::*;
pub use engines::Lod;
use engines::{nacelle, Rig};

pub const ENAMEL: u8 = 100;
pub const IRON: u8 = 106;
pub const GUN: u8 = 107;
pub const STEEL: u8 = 108;
pub const BRASS: u8 = 109;
pub const SHADE: u8 = 110;
pub const PENUMBRA: u8 = 111;

#[derive(Clone, Copy, PartialEq)]
pub enum Crest { Plume, Horns, Hood, Crown }

#[derive(Clone, Copy, PartialEq)]
pub enum Form { Barrel, Bell, Hoop, Needle, Keep, Wyvern }

pub struct Pod {
    pub name: &'static str,
    pub blurb: &'static str,
    pub form: Form,
    pub len: f32,
    pub r: f32,
    pub gap: f32,
    pub z: f32,
    pub tow: f32,
    pub cart: f32,
    pub crest: Crest,
    pub lance: bool,
    pub yoke: bool,
}

pub const PODS: [Pod; 6] = [
    Pod { name: "WARDEN", blurb: "Twin iron tubes. Even in every way.", form: Form::Barrel, len: 5.0, r: 0.64, gap: 1.2, z: 1.15, tow: 5.4, cart: 1.5, crest: Crest::Plume, lance: true, yoke: false },
    Pod { name: "BELLFOUNDER", blurb: "Flared bells push hardest on the boost.", form: Form::Bell, len: 4.0, r: 0.74, gap: 1.2, z: 1.3, tow: 4.4, cart: 1.6, crest: Crest::Horns, lance: false, yoke: false },
    Pod { name: "RINGWRIGHT", blurb: "Standing hoops grip the road in a bend.", form: Form::Hoop, len: 3.2, r: 0.9, gap: 1.78, z: 1.5, tow: 5.2, cart: 1.5, crest: Crest::Hood, lance: false, yoke: false },
    Pod { name: "LANCEWAIN", blurb: "Long needles. Fast on a straight, nervous in a bend.", form: Form::Needle, len: 7.4, r: 0.34, gap: 0.85, z: 1.0, tow: 6.4, cart: 1.25, crest: Crest::Plume, lance: true, yoke: false },
    Pod { name: "GATEHOUSE", blurb: "Battlement blocks on a yoke. Heavy and steady.", form: Form::Keep, len: 4.0, r: 0.8, gap: 1.45, z: 1.2, tow: 3.8, cart: 1.5, crest: Crest::Crown, lance: false, yoke: true },
    Pod { name: "WYVERN", blurb: "Swept wings. Light and quick off the line.", form: Form::Wyvern, len: 5.2, r: 0.5, gap: 0.8, z: 1.1, tow: 4.6, cart: 1.4, crest: Crest::Horns, lance: true, yoke: false },
];

pub fn foot(look: usize) -> (f32, f32, f32) {
    let s = &rules::pods()[look % rules::pods().len()];
    (s.len, s.tail, s.half_w)
}

#[derive(Clone, Copy)]
pub struct Livery {
    pub name: &'static str,
    pub ink: char,
    pub cloth: u8,
    pub rgb: [u8; 3],
    pub n: u8,
}

pub const LIVERIES: [Livery; 6] = [
    Livery { name: "AUREL", ink: '5', cloth: ENAMEL, rgb: [244, 208, 60], n: 0 },
    Livery { name: "BRANNOC", ink: '7', cloth: ENAMEL + 1, rgb: [196, 62, 48], n: 1 },
    Livery { name: "CAEL", ink: 'z', cloth: ENAMEL + 2, rgb: [72, 142, 218], n: 2 },
    Livery { name: "DUNSTAN", ink: 'C', cloth: ENAMEL + 3, rgb: [84, 172, 98], n: 3 },
    Livery { name: "EIRA", ink: ']', cloth: ENAMEL + 4, rgb: [152, 110, 206], n: 4 },
    Livery { name: "FENWICK", ink: '%', cloth: ENAMEL + 5, rgb: [228, 118, 156], n: 5 },
];

pub fn livery(seat: u32) -> &'static Livery { &LIVERIES[(seat as usize + 5) % LIVERIES.len()] }
pub fn pod(index: usize) -> &'static Pod { &PODS[index % PODS.len()] }

pub fn look_of(_seat: u32, car: &rules::Car) -> usize { car.pod as usize % PODS.len() }

pub fn name_of(car: &rules::Car) -> String { car.spec().name.to_uppercase() }

pub fn recolour(im: &mut Img, rgba: &[u8], boxes: &[(i32, i32, i32, i32)]) {
    const VIOLET: [char; 6] = ['d', '(', ')', '[', ']', '^'];
    const ROSE: [char; 5] = ['!', '?', ';', '%', '+'];
    const GOLD: [char; 7] = ['B', 'a', 'o', 'O', '4', '5', '6'];
    const RED: [char; 3] = ['8', 'p', '7'];
    for &(x0, y0, x1, y1) in boxes {
        for y in y0.max(0)..y1.min(224) {
            for x in x0.max(0)..x1.min(384) {
                let i = ((y * 384 + x) * 4) as usize;
                let (r, g, b) = (rgba[i] as i32, rgba[i + 1] as i32, rgba[i + 2] as i32);
                if r.max(g).max(b) < 24 { continue; }
                let violet = b * 100 > r * 120 && r * 100 > g * 110;
                let rose = r * 100 > b * 106 && b * 100 > g * 120;
                let gold = r >= g && g * 100 > r * 74 && b * 100 < g * 55;
                let red = r >= 60 && g * 100 > r * 25 && g * 100 < r * 40 && b * 100 > r * 18 && b * 100 < r * 34;
                if !(violet || rose || gold || red) { continue; }
                let Some(now) = im.get(x, y) else { continue };
                if now == [0, 0, 0] { continue; }
                let tone = (r.max(g).max(b) as f32 / 218.).clamp(0., 1.).powf(0.9);
                let ramp: &[char] = if violet { &VIOLET } else if rose { &ROSE } else if red { &RED } else { &GOLD };
                let at = ((tone * (ramp.len() - 1) as f32) + 0.5) as usize;
                im.put(x, y, ramp[at.min(ramp.len() - 1)]);
            }
        }
    }
}

pub fn palette() -> &'static raster::Painted {
    static PALETTE: std::sync::OnceLock<raster::Painted> = std::sync::OnceLock::new();
    PALETTE.get_or_init(|| {
        let mut painted = race_palette().clone();
        let plain = |rgb: [u8; 3], grain: i32| -> Vec<[u8; 3]> {
            (0..256 * 256).map(|i| {
                let (x, y) = (i % 256, i / 256);
                let weave = ((x * 7 + y * 13) % 5) as i32 - 2;
                rgb.map(|v| (v as i32 + weave * grain).clamp(0, 255) as u8)
            }).collect()
        };
        for lv in &LIVERIES { painted.set(lv.cloth, plain(lv.rgb, 1)); }
        painted.set(IRON, plain([66, 72, 90], 1));
        painted.set(GUN, plain([104, 114, 132], 1));
        painted.set(STEEL, plain([200, 212, 226], 1));
        painted.set(BRASS, plain([196, 122, 60], 2));
        painted.set(SHADE, plain([4, 4, 6], 0));
        painted.set(PENUMBRA, plain([20, 20, 26], 0));
        painted.patch(super::super::worldkit::palette::SKY, |x, y| {
            GLOWS.iter().find(|&&(kind, _)| (x / 8, y / 8) == super::super::worldkit::palette::special_block(kind)).map(|&(_, rgb)| rgb)
        });
        painted
    })
}

pub fn throats(look: usize) -> [(V3, f32); 2] {
    let p = pod(look);
    let dx = foot_dx(look);
    [engines::throat(p, -1.), engines::throat(p, 1.)].map(|(at, r)| (at + v3(dx, 0., 0.), r))
}

fn arc_mount(p: &Pod) -> (f32, f32) {
    match p.form {
        Form::Barrel => (-p.len * 0.05, p.r * 1.0),
        Form::Bell => (-p.len * 0.3, p.r * 1.2),
        Form::Hoop => (0., p.r * 0.98),
        Form::Needle => (-p.len * 0.1, p.r),
        Form::Keep => (-p.len * 0.2, p.r * 0.95),
        Form::Wyvern => (-p.len * 0.1, p.r * 0.95),
    }
}

fn tow_point(p: &Pod, side: f32, z: f32) -> V3 {
    let drop = match p.form {
        Form::Barrel => p.r * 0.7, Form::Bell => p.r * 1.0, Form::Hoop => 0.35, Form::Needle => p.r * 0.7,
        Form::Keep => p.r * 0.7, Form::Wyvern => p.r * 0.7,
    };
    v3(-p.len * 0.5 + 0.35, side * p.gap, z - drop)
}

pub fn build(m: &mut Mesh, look: usize, seat: u32, t: f32, burn: f32) {
    build_lod(m, look, seat, t, burn, Lod::Near, 1.);
}

pub fn build_lod(m: &mut Mesh, look: usize, seat: u32, t: f32, burn: f32, lod: Lod, flare: f32) {
    build_view(m, look, seat, t, burn, lod, flare, 0.);
}

pub fn build_view(m: &mut Mesh, look: usize, seat: u32, t: f32, burn: f32, lod: Lod, flare: f32, snug: f32) {
    let first = m.tris.len();
    build_raw(m, look, seat, t, burn, lod, flare, snug);
    let dx = foot_dx(look);
    for tr in &mut m.tris[first..] { for p in &mut tr.v { p.x += dx; } }
}

fn foot_dx(look: usize) -> f32 {
    static DX: std::sync::OnceLock<[f32; 6]> = std::sync::OnceLock::new();
    DX.get_or_init(|| std::array::from_fn(|i| {
        let mut m = Mesh::new();
        build_raw(&mut m, i, 1, 0., 0.15, Lod::Near, 1., 0.);
        let nose = m.tris.iter().flat_map(|t| t.v).map(|p| p.x).fold(f32::MIN, f32::max);
        foot(i).0 - nose
    }))[look % 6]
}

fn build_raw(m: &mut Mesh, look: usize, seat: u32, t: f32, burn: f32, lod: Lod, flare: f32, snug: f32) {
    let (p, lv) = (pod(look), livery(seat));
    let g = Rig { p, lv, t, burn, lod, flare, snug };
    let z = p.z + 0.07 * (t * 9. + seat as f32).sin();
    let gy = p.gap;
    nacelle(m, &g, -1., -gy, z);
    nacelle(m, &g, 1., gy, z + 0.03);
    let (ax, inner) = arc_mount(p);
    let lift = p.r * 0.5;
    let inner = if p.form == Form::Keep { inner } else { (inner * inner - lift * lift).max(0.04).sqrt() };
    let (ya, yb) = (-gy + inner, gy - inner);
    let (za, zb) = (z + lift, z + lift + 0.03);
    cab::electrode(m, &g, v3(ax, ya, za), 1.);
    cab::electrode(m, &g, v3(ax, yb, zb), -1.);
    cab::arc(m, &g, v3(ax, ya + 0.5, za), v3(ax, yb - 0.5, zb), seat as f32 * 5.);
    if lod >= Lod::Near {
        let mut thin = g;
        thin.burn *= 0.4;
        thin.flare *= 0.5;
        cab::arc(m, &thin, v3(ax - 0.3, ya + 0.5, za - 0.3), v3(ax - 0.3, yb - 0.5, zb - 0.3), seat as f32 * 5. + 9.1);
    }
    if p.yoke {
        cuboid(m, v3(p.len * 0.34, ya, z - 0.24), v3(p.len * 0.44, yb, z + 0.24), GUN);
        cuboid(m, v3(p.len * 0.33, -0.55, z - 0.3), v3(p.len * 0.45, 0.55, z + 0.3), lv.cloth);
        for i in 0..5 {
            let y = ya + (yb - ya) * (i as f32 + 0.5) / 5.;
            cuboid(m, v3(p.len * 0.33, y - 0.16, z + 0.24), v3(p.len * 0.45, y + 0.16, z + 0.5), GUN);
        }
    }
    let cx = -p.len * 0.5 - p.tow;
    let s = p.cart;
    for sg in [-1.0f32, 1.0] {
        let a = tow_point(p, sg, z);
        let b = v3(cx + 0.38 * s, sg * 0.5 * s, 0.3 * s + 0.5 * s);
        cab::cable(m, &g, a, b, 0.5);
    }
    cab::chariot(m, &g, cx, 0.3 * s, s);
    if lod >= Lod::Mid {
        shade_pool(m, v3(cx, 0., 0.), [1.0 * s, 0.75 * s]);
        for sg in [-1.0f32, 1.0] {
            shade_pool(m, v3(0., sg * gy, 0.), [p.len * 0.55, p.r * 1.1]);
        }
    }
}

pub const SCALE: f32 = 1.0;
const GONE: f32 = 6.0;
const FULL: f32 = 18.0;
const SNUG_NEAR: f32 = 14.0;
const SNUG_FAR: f32 = 26.0;
const NEAR: f32 = 55.;
const MID: f32 = 105.;

pub fn lod_at(dist: f32) -> (Lod, f32) {
    let lod = if dist < NEAR { Lod::Near } else if dist < MID { Lod::Mid } else { Lod::Far };
    (lod, (dist / 30.).clamp(1., 8.))
}

pub fn add(m: &mut Mesh, race: &rules::Session, seat: u32, c: &rules::Car, eye: V3) {
    add_led(m, race, seat, c, eye, leads(race, seat));
}

pub fn leads(race: &rules::Session, seat: u32) -> bool {
    race.ticks > rules::race::COUNTDOWN && race.cars.len() > 1
        && race.cars.get(&seat).is_some_and(|c| c.finish.is_none())
        && race.order().first() == Some(&seat)
}

pub fn pennant(m: &mut Mesh, look: usize, t: f32, snug: f32, lod: Lod) {
    let p = pod(look);
    let cx = -p.len * 0.5 - p.tow + foot_dx(look);
    let s = p.cart;
    let base = v3(cx - 0.55 * s, 0., 0.3 * s + 0.88 * s);
    let top = base + v3(0., 0., (2.7 - 1.2 * snug) * s);
    tube(m, base, top, 0.045 * s, 5, STEEL);
    let flare = if lod == Lod::Far { 2.4 } else { 1. };
    let (fl, drop) = (1.9 * s * (1. - 0.25 * snug), 0.78 * s * (1. - 0.2 * snug));
    let ripple = |u: f32| 0.28 * s * u * (t * 7. - u * 5.).sin();
    let at = |u: f32, v: f32| base + v3(0., 0., (2.7 - 1.2 * snug) * s) + v3(-fl * u, ripple(u), -drop * v) ;
    glowing(m, Glow::Lamp(0), |m| {
        for (u0, u1) in [(0., 0.4), (0.4, 0.75)] {
            m.push_quad(at(u0, 0.), at(u1, 0.), at(u1, 1.), at(u0, 1.), mat::CRYSTAL);
            m.push_quad(at(u1, 1.), at(u1, 0.), at(u0, 0.), at(u0, 1.), mat::CRYSTAL);
        }
        let (tip_a, tip_b, notch) = (at(1., 0.), at(1., 1.), at(0.76, 0.5));
        tri(m, at(0.75, 0.), tip_a, notch, mat::CRYSTAL);
        tri(m, at(0.75, 1.), notch, tip_b, mat::CRYSTAL);
        tri(m, notch, tip_a, at(0.75, 0.), mat::CRYSTAL);
        tri(m, notch, at(0.75, 1.), tip_b, mat::CRYSTAL);
        gem(m, top, 0.12 * s * flare, 0.2 * s * flare, V3::UP, mat::CRYSTAL);
    });
}

pub fn scuffs(m: &mut Mesh, look: usize, marks: &[(u8, i8)]) {
    let p = pod(look);
    let dx = foot_dx(look);
    for (n, &(seat, side)) in marks.iter().enumerate() {
        let sg = f32::from(side.signum());
        if sg == 0. { continue; }
        let lv = livery(u32::from(seat));
        let y = sg * (p.gap + p.r * 1.07);
        let x0 = dx + p.len * (0.32 - 0.2 * n as f32);
        let z = p.z + 0.16 * (n as f32 % 2.) - 0.06;
        for (a, b, c, d) in [(0., 0.5, 0.3, -0.12f32), (0.22, 0.7, 0.18, -0.05)] {
            let q = [v3(x0 - a * p.len, y, z + c * p.r), v3(x0 - b * p.len, y, z + (c + d) * p.r), v3(x0 - b * p.len, y, z + (c + d + 0.22) * p.r), v3(x0 - a * p.len, y, z + (c + 0.22) * p.r)];
            m.push_quad(q[0], q[1], q[2], q[3], lv.cloth);
            m.push_quad(q[3], q[2], q[1], q[0], lv.cloth);
        }
    }
}

pub fn add_led(m: &mut Mesh, race: &rules::Session, seat: u32, c: &rules::Car, eye: V3, lead: bool) {
    let first = m.tris.len();
    let burn = if c.boosting { 1.55 } else { 0.15 + c.v / 50. };
    let o = point(c.d, c.y, c.lift.max(-30.));
    let dist = (o - eye).length();
    let (lod, flare) = lod_at(dist);
    let near = ((dist - SNUG_NEAR) / (SNUG_FAR - SNUG_NEAR)).clamp(0., 1.);
    let snug = 1. - near * near * (3. - 2. * near);
    build_view(m, look_of(seat, c), seat, race.ticks as f32 / 30., burn, lod, flare, snug);
    if lead { pennant(m, look_of(seat, c), race.ticks as f32 / 30., snug, lod); }
    if lod >= Lod::Mid && !c.scuffs.is_empty() { scuffs(m, look_of(seat, c), &c.scuffs); }
    let (yaw_s, yaw_c) = (pose(c.d).2 + c.yaw - c.vy * 0.012).sin_cos();
    let road = race.course().frame(c.route, c.d.rem_euclid(race.course().lap()));
    let (roll_s, roll_c) = (-road.bank - c.vy * 0.018).clamp(-0.5, 0.5).sin_cos();
    let (pitch_s, pitch_c) = road.g.atan().sin_cos();
    let k = ((dist - GONE) / (FULL - GONE)).clamp(0., 1.);
    let scale = SCALE * k * k * (3. - 2. * k);
    if scale < 0.04 { m.tris.truncate(first); return; }
    let rotate = |p: V3| {
        let (x, z) = (p.x * pitch_c - p.z * pitch_s, p.x * pitch_s + p.z * pitch_c);
        let (y, z) = (p.y * roll_c - z * roll_s, p.y * roll_s + z * roll_c);
        v3(x * yaw_c - y * yaw_s, x * yaw_s + y * yaw_c, z)
    };
    for tr in &mut m.tris[first..] {
        for p in &mut tr.v { *p = o + rotate(*p) * scale; }
        tr.normal = rotate(tr.normal);
    }
}

pub fn scorch(m: &mut Mesh, race: &rules::Session, c: &rules::Car, eye: V3) {
    let Some((d, y, route)) = c.scorch else { return };
    let lap = race.course().lap();
    let o = point(d, y, 0.);
    if (o - eye).length() > 190. { return; }
    let age = if c.blown() { rules::race::engines::BLOWOUT_TICKS.saturating_sub(c.crash) as f32 } else { 999. };
    let grow = (age / 45.).clamp(0.15, 1.);
    let road = race.course().frame(route, d.rem_euclid(lap));
    let (yaw_s, yaw_c) = road.th.sin_cos();
    let (roll_s, roll_c) = (-road.bank).clamp(-0.5, 0.5).sin_cos();
    let (pitch_s, pitch_c) = road.g.atan().sin_cos();
    let first = m.tris.len();
    let rot = |q: V3| {
        let (x, z) = (q.x * pitch_c - q.z * pitch_s, q.x * pitch_s + q.z * pitch_c);
        let (yy, z) = (q.y * roll_c - z * roll_s, q.y * roll_s + z * roll_c);
        v3(x * yaw_c - yy * yaw_s, x * yaw_s + yy * yaw_c, z)
    };
    let seed = (d * 7.3 + y * 3.1).sin();
    let reach = |i: usize, w: f32| {
        let a = i as f32 / 14. * TAU;
        (1. + 0.22 * (a * 3. + seed * 5.).sin() + 0.12 * (a * 5. + seed).cos()) * w
    };
    for (w, z, k) in [(1.0f32, 0.12, PENUMBRA), (0.74, 0.16, SHADE), (0.44, 0.2, SHADE)] {
        let at = |i: usize| { let a = i as f32 / 14. * TAU; let r = reach(i, w * grow); v3(6.0 * r * a.cos(), 3.2 * r * a.sin(), z) };
        for i in 0..14 { tri(m, v3(0., 0., z), at(i), at(i + 1), k); }
    }
    if c.blown() {
        let t = race.ticks as f32;
        glowing(m, Glow::Hot, |m| {
            for n in 0..7 {
                let (a, r) = (n as f32 * 2.4 + seed * 4., 0.5 + 0.5 * (n as f32 * 1.7).sin().abs());
                if ((t * 0.31 + n as f32 * 1.9).sin()) < -0.35 { continue; }
                let p = v3(4.2 * r * a.cos() * grow, 2.2 * r * a.sin() * grow, 0.24);
                let q = 0.22 + 0.1 * (t * 0.5 + n as f32).sin().abs();
                m.push_quad(p + v3(-q, -q, 0.), p + v3(q, -q, 0.), p + v3(q, q, 0.), p + v3(-q, q, 0.), mat::FIRE);
            }
        });
    }
    for tr in &mut m.tris[first..] {
        for p in &mut tr.v { *p = o + rot(*p); }
        tr.normal = rot(tr.normal);
    }
}

pub fn boxes(race: &rules::Session, view: &View3) -> Vec<(i32, i32, i32, i32)> {
    let tan_h = (view.fov_rad * 0.5).tan();
    race.cars.values().filter_map(|c| {
        let (x, y, z) = camera::project(view, point(c.d - 3., c.y, 2.))?;
        let r = (10. * SCALE / z * camera::W * 0.5 / tan_h).clamp(6., 150.);
        Some(((x - r) as i32, (y - r * 0.8) as i32, (x + r) as i32, (y + r * 0.8) as i32))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::worldkit::palette as land;

    fn extent(m: &Mesh) -> V3 {
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for t in &m.tris { for p in &t.v {
            assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
            lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        } }
        hi - lo
    }

    #[test]
    fn the_roster_is_the_sims_pods_in_the_sims_order() {
        for (n, spec) in rules::pods().iter().enumerate() {
            assert_eq!(PODS[n].name, spec.name.to_uppercase(), "pod {n} is not the sim's {}", spec.name);
        }
        assert!(PODS.len() >= rules::pods().len());
    }

    #[test]
    fn every_pod_is_one_whole_pod_of_a_size_the_lens_can_hold() {
        for (look, p) in PODS.iter().enumerate() {
            for burn in [0.0, 1.55] {
                let mut m = Mesh::new();
                build(&mut m, look, 1 + look as u32, 0.7, burn);
                let e = extent(&m);
                assert!(e.y > 2.0 * (p.gap + p.r) * 0.9 && e.y < 10., "{} is {:.1} wide", p.name, e.y);
                assert!(e.x > p.len + p.tow && e.x < 20., "{} is {:.1} long", p.name, e.x);
                assert!(e.z < 9., "{} is {:.1} tall", p.name, e.z);
                assert!(m.tris.len() > 150 && m.tris.len() < 4000, "{} has {} triangles", p.name, m.tris.len());
            }
        }
    }

    fn lights(m: &Mesh) -> usize { m.tris.iter().filter(|t| t.mat == land::SKY).count() }

    #[test]
    fn a_far_pod_keeps_its_lights_and_costs_a_fraction() {
        for (look, p) in PODS.iter().enumerate() {
            let (mut near, mut far) = (Mesh::new(), Mesh::new());
            build_lod(&mut near, look, 1 + look as u32, 0.7, 1.0, Lod::Near, 1.);
            build_lod(&mut far, look, 1 + look as u32, 0.7, 1.0, Lod::Far, 4.);
            assert!(near.tris.len() < 2800, "{} has {} triangles near", p.name, near.tris.len());
            assert!(far.tris.len() * 2 < near.tris.len(), "{} is {} far and {} near", p.name, far.tris.len(), near.tris.len());
            assert!(lights(&far) >= 40, "{} keeps only {} light triangles far off", p.name, lights(&far));
            let lamp = land::special_uv(Glow::Lamp(livery(1 + look as u32).n).kind());
            assert!(far.tris.iter().any(|t| t.mat == land::SKY && t.uv[0] == lamp), "{} lost the seat's lamp far off", p.name);
            let arc = land::special_uv(Glow::Arc.kind());
            assert!(far.tris.iter().any(|t| t.mat == land::SKY && t.uv[0] == arc), "{} lost the arc far off", p.name);
        }
    }

    #[test]
    fn the_pods_lights_are_blocks_of_their_own_in_the_sky_tables_spare_row() {
        let mut seen = std::collections::BTreeSet::new();
        for &(kind, _) in &GLOWS { assert!(kind >= 64 && kind < 96 && seen.insert(kind), "light block {kind} is not a spare, single one"); }
        for g in [Glow::Arc, Glow::ArcEdge, Glow::Intake, Glow::Core, Glow::Hot, Glow::Lamp(0), Glow::Lamp(5)] {
            assert!(GLOWS.iter().any(|&(k, _)| k == g.kind()), "{g:?} has no colour in the table");
        }
        for lv in &LIVERIES { assert_eq!(livery(lv.n as u32 + 1).n, lv.n.min(5)); }
    }

    #[test]
    fn the_grid_has_no_pod_over_another() {
        let slots = rules::race::GRID;
        let mut boxes = Vec::new();
        for (slot, &(d, y)) in slots.iter().enumerate() {
            let look = slot;
            let mut m = Mesh::new();
            build_lod(&mut m, look, slot as u32 + 1, 0.3, 0.2, Lod::Near, 1.);
            let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, 0.), v3(f32::MIN, f32::MIN, 0.));
            for t in &m.tris { for p in &t.v { lo = v3(lo.x.min(p.x), lo.y.min(p.y), 0.); hi = v3(hi.x.max(p.x), hi.y.max(p.y), 0.); } }
            boxes.push((PODS[look].name, d + lo.x, d + hi.x, y + lo.y, y + hi.y));
        }
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let apart = a.2 < b.1 || b.2 < a.1 || a.4 < b.3 || b.4 < a.3;
                assert!(apart, "{} ({:.1}..{:.1} by {:.1}..{:.1}) overlaps {} ({:.1}..{:.1} by {:.1}..{:.1}) on the grid", a.0, a.1, a.2, a.3, a.4, b.0, b.1, b.2, b.3, b.4);
            }
            assert!(a.2 < 4., "{} sits over the line", a.0);
        }
    }

    #[test]
    fn the_lens_at_every_seat_looks_down_a_clear_grid() {
        let grid = rules::race::GRID;
        for (i, &(di, yi)) in grid.iter().enumerate() {
            let lens = (di - camera::BACK, yi * 0.45);
            for (j, &(dj, yj)) in grid.iter().enumerate() {
                if i == j { continue; }
                let between = dj > lens.0 && dj < di;
                assert!(!(between && (yj - lens.1).abs() < 6.5), "seat {} stands in seat {}'s lens", j + 1, i + 1);
            }
        }
    }

    #[test]
    fn every_pod_fits_the_footprint_the_sim_gives_it() {
        let mut over = Vec::new();
        for (look, p) in PODS.iter().enumerate() {
            let (ahead, tail, half_w) = foot(look);
            for burn in [0.15, 1.55] {
                let mut m = Mesh::new();
                build_lod(&mut m, look, 1 + look as u32, 0.7, burn, Lod::Near, 1.);
                let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, 0.), v3(f32::MIN, f32::MIN, 0.));
                for t in &m.tris { for q in &t.v { lo = v3(lo.x.min(q.x), lo.y.min(q.y), 0.); hi = v3(hi.x.max(q.x), hi.y.max(q.y), 0.); } }
                if hi.x > ahead * 1.1 { over.push(format!("{} reaches {:.1} ahead of {ahead}", p.name, hi.x)); }
                if -lo.x > tail * 1.1 { over.push(format!("{} reaches {:.1} behind, past {tail}", p.name, -lo.x)); }
                if hi.y.max(-lo.y) > half_w * 1.1 { over.push(format!("{} is {:.2} out from its middle, past {half_w}", p.name, hi.y.max(-lo.y))); }
            }
        }
        over.dedup();
        assert!(over.is_empty(), "{over:#?}");
    }

    #[test]
    fn a_pod_at_the_lens_shrinks_away_and_the_flame_stays_short() {
        let mut run = crate::rules::Run::roundtable_for_test("knight_racers").unwrap();
        let mut race = run.knight_racers.take().unwrap();
        race.join_ids(1..=6);
        let car = race.cars[&1].clone();
        let at = point(car.d, car.y, 0.);
        let mut m = Mesh::new();
        add(&mut m, &race, 1, &car, at + v3(0., 0., 3.));
        assert!(m.tris.is_empty(), "a pod three metres from the eye is still there");
        add(&mut m, &race, 1, &car, at + v3(0., 0., 40.));
        let full = extent(&m);
        let mut boost = car.clone();
        boost.boosting = true;
        let mut b = Mesh::new();
        add(&mut b, &race, 1, &boost, at + v3(0., 0., 40.));
        assert!(extent(&b).x - full.x < 3.0, "a boost's flame grew the pod by {:.1}", extent(&b).x - full.x);
    }

    #[test]
    fn the_pods_materials_are_ids_of_their_own() {
        let mut seen = std::collections::BTreeSet::new();
        let mine = LIVERIES.iter().map(|l| l.cloth).chain([IRON, GUN, STEEL, BRASS, SHADE, PENUMBRA]);
        for m in mine { assert!((71..128).contains(&m) && seen.insert(m), "material {m} is not a spare id of the pods' own"); }
        assert_eq!(seen.len(), 12);
    }

    #[test]
    fn a_pod_near_the_lens_stands_lower_than_one_far_off() {
        for look in 0..PODS.len() {
            let high = |snug: f32| {
                let mut m = Mesh::new();
                build_view(&mut m, look, 1 + look as u32, 0.7, 0.4, Lod::Near, 1., snug);
                m.tris.iter().filter(|t| (t.v[0].z + t.v[1].z + t.v[2].z) / 3. > 2.2)
                    .map(|t| (t.v[1] - t.v[0]).cross(t.v[2] - t.v[0]).length() * 0.5).sum::<f32>()
            };
            assert!(high(1.) < high(0.) * 0.95, "{} has {:.2} m2 over 2.2 m snug and {:.2} not", PODS[look].name, high(1.), high(0.));
        }
    }

    #[test]
    fn recolour_inks_the_seats_by_their_tone_and_leaves_the_land_alone() {
        let mut im = Img::black(384, 224);
        let mut rgba = vec![0u8; 384 * 224 * 4];
        let put = |im: &mut Img, rgba: &mut Vec<u8>, x: i32, c: [u8; 3]| {
            let i = ((10 * 384 + x) * 4) as usize;
            rgba[i..i + 3].copy_from_slice(&c);
            im.set(x, 10, [90, 90, 90]);
        };
        put(&mut im, &mut rgba, 1, [196, 62, 48]);
        put(&mut im, &mut rgba, 2, [96, 34, 30]);
        put(&mut im, &mut rgba, 3, [214, 180, 52]);
        put(&mut im, &mut rgba, 4, [60, 52, 14]);
        put(&mut im, &mut rgba, 5, [92, 26, 12]);
        put(&mut im, &mut rgba, 6, [150, 70, 45]);
        recolour(&mut im, &rgba, &[(0, 0, 384, 224)]);
        let ink = |ch| crate::ink::ink(ch).unwrap();
        assert_eq!(im.get(1, 10), Some(ink('7')), "lit red is the one true red");
        assert_ne!(im.get(2, 10), im.get(1, 10), "shaded red is a darker ink");
        assert_ne!(im.get(3, 10), im.get(4, 10), "lit and shaded gold are different inks");
        assert_eq!(im.get(5, 10), Some([90, 90, 90]), "a lava crust is the land's");
        assert_eq!(im.get(6, 10), Some([90, 90, 90]), "so is a rail");
    }

    #[test]
    fn the_seat_colours_are_six_and_the_land_does_not_wear_them() {
        let mut seen = std::collections::BTreeSet::new();
        for lv in &LIVERIES { assert!(seen.insert(lv.cloth), "two seats share a material"); }
        let land = [land::ROAD_A, land::ROAD_B, land::SAND, land::MESA, land::BASALT, land::ASH, land::CAVE, land::MOOR,
            land::MASONRY, land::SLATE, land::TIMBER, land::PLATE, land::PLATE_B, land::DEEP_A, land::DEEP_B, land::CLOTH, land::CLOTH_R, land::SKY, land::GLOW, land::WATER];
        for m in seen.iter().copied() {
            assert!(!land.contains(&m), "material {m} is both a pod's and the land's");
        }
        let ink: std::collections::BTreeSet<_> = LIVERIES.iter().map(|l| l.ink).collect();
        assert_eq!(ink.len(), 6, "two seats share a blip colour");
    }
}


#[cfg(test)]
#[path = "pods_proof.rs"]
mod pods_proof;
