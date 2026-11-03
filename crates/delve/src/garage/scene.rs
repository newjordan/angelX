//! One frame of the garage: the empty shop (`shop`), then everything that
//! moves in it built fresh into buffers that are kept: the racers on their
//! lifts with their swaps playing, the knights, the part each is about to
//! take, the sparks; lit and drawn by `raster`, then the labels and the
//! stat panel laid over it.

use super::canvas::{Canvas, mix};
use super::look::{self, GLOW_SEAT, SEAT, SEAT_COLOURS, SHADOW, SPARK};
use super::meshes::{self, GLOW, RacerMeshes};
use super::parts::Slot;
use super::prim::*;
use super::racer::{Build, KIT};
use super::raster::{Camera, Light, Lighting, Raster};
use super::room::{DELTA_TICKS, LAND_TICK, SWAP_TICKS, Seat, Shop, SwapAnim};
use super::shop;
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use crate::rules::HZ;
use std::f32::consts::{PI, TAU};

/// How the picture is finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// The lit frame as it is.
    Smooth,
    /// Each colour held to a few steps and the steps dithered: the arcade's
    /// own grain.
    Arcade,
}

/// A frame and every buffer it takes.
pub struct View {
    pub w: usize,
    pub h: usize,
    pub look: Look,
    /// Whether the labels and the stat panel are drawn over the frame. A
    /// terminal draws its own as text, so its frame leaves them off.
    pub hud: bool,
    cam: Camera,
    ras: Raster,
    movers: Mesh,
    parts: RacerMeshes,
    ghost: RacerMeshes,
    item: Mesh,
    lamps: Lighting,
    inspect: Raster,
    inspect_lamps: Lighting,
}

/// The inspect window's size: a part turning on a stand.
pub const INSPECT: (usize, usize) = (84, 64);

impl View {
    pub fn new(w: usize, h: usize) -> View {
        View {
            w,
            h,
            look: Look::Smooth,
            hud: true,
            cam: shop::camera(w, h),
            ras: Raster::new(w, h),
            movers: Mesh::new(),
            parts: RacerMeshes::new(),
            ghost: RacerMeshes::new(),
            item: Mesh::new(),
            lamps: shop::lighting().clone(),
            inspect: Raster::new(INSPECT.0, INSPECT.1),
            inspect_lamps: Lighting {
                ambient: [0.10, 0.11, 0.18],
                lamps: vec![
                    Light { pos: v3(-3.0, -4.0, 5.0), colour: [1.6, 1.2, 0.8], reach: 14.0, halo: 0.0 },
                    Light { pos: v3(4.0, 3.0, 3.0), colour: [0.3, 0.5, 1.0], reach: 12.0, halo: 0.0 },
                ],
            },
        }
    }

    pub fn camera(&self) -> &Camera {
        &self.cam
    }

    /// The finished frame, row by row.
    pub fn pixels(&self) -> &[[u8; 3]] {
        &self.ras.rgb
    }

    /// Draw the shop. `focus` is the seat whose panel is shown.
    pub fn draw(&mut self, shop: &Shop, focus: Option<u32>) {
        let t = shop.tick as f32 / HZ as f32;
        self.movers.tris.clear();
        // the shop's lamps, and the lamps this frame adds
        let base = shop::lighting();
        self.lamps.ambient = base.ambient;
        self.lamps.lamps.clear();
        self.lamps.lamps.extend_from_slice(&base.lamps);
        for seat in &shop.seats {
            self.racer(seat, t);
            self.knight(seat, t);
            self.pick(seat, t);
        }
        let ms = [shop::architecture(), &self.movers];
        self.ras.visibility(&self.cam, &ms);
        self.ras.shade(&self.cam, &ms, &self.lamps, [3, 3, 7]);
        self.ras.halos(&self.cam, &self.lamps);
        if self.look == Look::Arcade {
            arcade(&mut self.ras.rgb, self.w);
        }
        let seat = focus.and_then(|id| shop.seat(id)).or_else(|| shop.seats.first());
        if let Some(p) = seat.and_then(|s| s.pick) {
            self.inspect(p.part, t);
        }
        if self.hud {
            self.hud(shop, seat);
        }
    }

    /// The part a knight is at, large and turning, in the inspect window.
    fn inspect(&mut self, part: &'static super::parts::Part, t: f32) {
        meshes::display(part, t, &mut self.item);
        let Some((lo, hi)) = bounds(&self.item) else { return };
        let size = hi - lo;
        let centre = (lo + hi) * 0.5;
        let k = 4.0 / size.x.max(size.y).max(size.z * 1.4).max(0.1);
        let mut m = Mesh::new();
        // the stand
        cyl(&mut m, v3(0.0, 0.0, -0.5), v3(0.0, 0.0, -0.3), [2.3, 2.3], 14, look::DARK_STEEL, true);
        cyl(&mut m, v3(0.0, 0.0, -0.3), v3(0.0, 0.0, -0.26), [2.2, 2.2], 14, mat::STONE_DARK, true);
        place(&mut self.item, 0, 1.0, 0.0, v3(-centre.x, -centre.y, -lo.z));
        let first = m.tris.len();
        m.tris.extend_from_slice(&self.item.tris);
        place(&mut m, first, k, t * 0.9, v3(0.0, 0.0, -0.22));
        let cam = Camera::look(v3(0.0, -7.2, 3.6), v3(0.0, 0.0, 0.7), 0.72, INSPECT.0, INSPECT.1);
        let ms = [&m];
        self.inspect.visibility(&cam, &ms);
        self.inspect.shade(&cam, &ms, &self.inspect_lamps, [8, 8, 14]);
    }

    // ------------------------------------------------------------ the racer

    fn racer(&mut self, seat: &Seat, t: f32) {
        let (at, yaw) = shop::racer_pose(seat.bay);
        let k = shop::RACER_SCALE;
        meshes::build_parts(&seat.build, t, &mut self.parts);
        // a soft shadow under it
        shadow(&mut self.movers, at, 3.0, 1.5);
        let anim = seat.swap.as_ref();
        let old = anim.and(seat.undo.as_ref());
        if let Some(o) = old {
            meshes::build_parts(o, t, &mut self.ghost);
        }
        let changed: Vec<Slot> = match (anim, old) {
            (Some(a), Some(o)) => a.slot.map_or_else(|| seat.build.differs(o), |s| vec![s]),
            _ => vec![],
        };
        for slot in Slot::ALL {
            let i = slot.index();
            let (mut new_dz, mut old_dz) = (Some(0.0f32), None);
            if let (Some(a), true) = (anim, changed.contains(&slot)) {
                let stagger = changed.iter().position(|&s| s == slot).unwrap_or(0) as f32 * 0.03;
                (new_dz, old_dz) = swap_offsets(a.age as f32 / SWAP_TICKS as f32 - stagger);
            }
            if let Some(dz) = new_dz {
                let first = self.movers.tris.len();
                self.movers.tris.extend_from_slice(&self.parts.slot[i].tris);
                place(&mut self.movers, first, k, yaw, at + v3(0.0, 0.0, dz * k));
            }
            if let Some(dz) = old_dz {
                let first = self.movers.tris.len();
                self.movers.tris.extend_from_slice(&self.ghost.slot[i].tris);
                place(&mut self.movers, first, k, yaw, at + v3(0.0, 0.0, dz * k));
            }
        }
        let first = self.movers.tris.len();
        self.movers.tris.extend_from_slice(&self.parts.rig.tris);
        place(&mut self.movers, first, k, yaw, at);
        // a swap in progress: the bay's lamp flares, and where the part lands the sparks fly
        if let Some(a) = anim {
            let age = a.age;
            let flare = if age >= LAND_TICK { (1.0 - (age - LAND_TICK) as f32 / 14.0).max(0.0) } else { 0.0 };
            let (bx, by) = shop::BAYS[seat.bay];
            self.lamps.lamps.push(Light { pos: v3(bx, by - 0.5, 4.2), colour: [1.8 * flare + 0.25, 1.35 * flare + 0.2, 0.7 * flare + 0.1], reach: 9.0, halo: 0.0 });
            if age >= LAND_TICK {
                let lead = changed.first().copied().unwrap_or(Slot::Engine);
                let c = bounds(&self.parts.slot[lead.index()]).map_or(V3::ZERO, |(lo, hi)| (lo + hi) * 0.5);
                let mut p = c;
                p = v3(p.x * yaw.cos() - p.y * yaw.sin(), p.x * yaw.sin() + p.y * yaw.cos(), p.z) * k + at;
                self.sparks(p, age - LAND_TICK, seat.id as u64 * 7 + seat.bay as u64);
            }
        }
    }

    fn sparks(&mut self, at: V3, age: u32, seed: u64) {
        let s = age as f32 / HZ as f32;
        if s > 0.7 {
            return;
        }
        let (r, u) = (self.cam.right, self.cam.up);
        for i in 0..18u64 {
            let h = hash(seed * 131 + i);
            let ang = (h & 0xff) as f32 / 255.0 * TAU;
            let out = 1.6 + ((h >> 8) & 0xff) as f32 / 255.0 * 3.2;
            let up = 2.0 + ((h >> 16) & 0xff) as f32 / 255.0 * 3.8;
            let p = at + v3(ang.cos() * out * s, ang.sin() * out * s, up * s - 7.0 * s * s);
            if p.z < PAD_Z {
                continue;
            }
            let size = 0.13 * (1.0 - s / 0.7);
            quad(&mut self.movers, p - r * size - u * size, p + r * size - u * size, p + r * size + u * size, p - r * size + u * size, SPARK);
        }
    }

    // ------------------------------------------------------------ the knight

    fn knight(&mut self, seat: &Seat, t: f32) {
        let (x, y) = seat.pos();
        let i = seat.bay % 6;
        let phase = if seat.walking() { seat.stride as f32 * 0.42 } else { 0.0 };
        let bob = if seat.walking() { (phase * 2.0).sin().abs() * 0.06 } else { (t * 2.0 + i as f32).sin() * 0.012 };
        let first = self.movers.tris.len();
        knight_figure(&mut self.movers, i, phase, seat.swap.is_some());
        place(&mut self.movers, first, KNIGHT, seat.yaw, v3(x, y, bob));
        shadow(&mut self.movers, v3(x, y, 0.0), 0.7, 0.55);
    }

    // ------------------------------------------------------------ the part about to be taken

    fn pick(&mut self, seat: &Seat, t: f32) {
        let Some(p) = seat.pick else { return };
        let i = seat.bay % 6;
        let st = shop::station(p.slot);
        let spot = shop::item_spot(st, p.index);
        let c = SEAT_COLOURS[i];
        // a ring on the shelf under it
        let r = (st.cell * 0.5).min(1.2);
        let ring = GLOW_SEAT + i as u8;
        for k in 0..16 {
            let (a0, a1) = (k as f32 / 16.0 * TAU, (k as f32 + 0.7) / 16.0 * TAU);
            let q = |a: f32, rr: f32| spot + v3(a.cos() * rr, a.sin() * rr, 0.04);
            quad(&mut self.movers, q(a0, r), q(a1, r), q(a1, r - 0.1), q(a0, r - 0.1), ring);
        }
        // the part itself, lifted off its shelf and turning
        let hover = 1.2 + (t * 3.0).sin() * 0.06;
        meshes::display(p.part, t, &mut self.item);
        if let Some((lo, hi)) = bounds(&self.item) {
            let ext = (hi - lo).x.max((hi - lo).y).max((hi - lo).z).max(0.1);
            let k = (3.0 / ext).min(if shop::station(p.slot).two_sided { 1.1 } else { 1.6 });
            let centre = (lo + hi) * 0.5;
            let first = self.movers.tris.len();
            // set its middle on the vertical axis, standing on z = 0
            place(&mut self.item, 0, 1.0, 0.0, v3(-centre.x, -centre.y, -lo.z));
            self.movers.tris.extend_from_slice(&self.item.tris);
            place(&mut self.movers, first, k, t * 1.6, spot + v3(0.0, 0.0, hover + 0.2));
        }
        self.lamps.lamps.push(Light { pos: spot + v3(0.0, 0.0, 2.6), colour: [c[0] * 1.5 + 0.3, c[1] * 1.5 + 0.3, c[2] * 1.5 + 0.3], reach: 6.0, halo: 0.0 });
    }

    // ------------------------------------------------------------ the labels and the panel

    fn hud(&mut self, shop: &Shop, focus: Option<&Seat>) {
        let (w, h) = (self.w, self.h);
        let cam = self.cam;
        let mut cv = Canvas::new(w, h, &mut self.ras.rgb);
        // each rack's sign
        for s in shop::stations() {
            let (x, y) = s.cell_at(s.n / 2);
            let at = if s.two_sided { v3(x - s.cell * 0.5 * f32::from(s.n % 2 == 0), y, 2.2) } else { v3(x + s.out.0 * 0.5, y + s.out.1 * 0.5, shop::TOP + 0.9) };
            if let Some((sx, sy, _)) = cam.project(at) {
                cv.centred(sx as i32, sy as i32 - 3, s.slot.name(), [196, 160, 96], 1);
            }
        }
        // a knight away from their lift wears their name
        for seat in &shop.seats {
            let (x, y) = seat.pos();
            let (bx, by) = shop::BAYS[seat.bay];
            if (x - bx).hypot(y - by) < 5.2 {
                continue;
            }
            if let Some((sx, sy, _)) = cam.project(v3(x, y, 2.1)) {
                let name = upper(&seat.name, 12);
                let wdt = Canvas::width(&name, 1) + 8;
                cv.shade(sx as i32 - wdt / 2, sy as i32 - 5, wdt, 11, [6, 6, 12], 0.7);
                cv.rect(sx as i32 - wdt / 2, sy as i32 - 5, 2, 11, seat_rgb(seat.bay));
                cv.text(sx as i32 - wdt / 2 + 5, sy as i32 - 2, &name, [226, 218, 190], 1);
            }
        }
        // each lift's plate, over its racer: the knight it belongs to
        for seat in &shop.seats {
            let (bx, by) = shop::BAYS[seat.bay];
            if let Some((sx, sy, _)) = cam.project(v3(bx, by, shop::PAD_TOP + 2.9)) {
                let name = upper(&seat.name, 14);
                let wdt = Canvas::width(&name, 1) + 10;
                let (x, y) = (sx as i32 - wdt / 2, sy as i32 - 5);
                cv.shade(x, y, wdt, 11, [6, 6, 12], 0.72);
                cv.rect(x, y, 3, 11, seat_rgb(seat.bay));
                cv.text(x + 7, y + 2, &name, [226, 218, 190], 1);
            }
        }
        if let Some(seat) = focus {
            panel(&mut cv, seat, &self.inspect);
        }
    }
}

const PAD_Z: f32 = shop::PAD_TOP;
/// A knight's size against the figure's own 1.4.
const KNIGHT: f32 = 1.35;

fn hash(mut x: u64) -> u64 {
    x = x.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x ^= x >> 29;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^ (x >> 32)
}

fn seat_rgb(i: usize) -> [u8; 3] {
    SEAT_COLOURS[i % 6].map(|c| (c * 255.0).min(255.0) as u8)
}

fn upper(s: &str, n: usize) -> String {
    s.chars().take(n).collect::<String>().to_uppercase()
}

/// The racer's name for a plate: its starting racer's, or its engine's.
pub fn racer_name(b: &Build) -> String {
    (0..KIT.len())
        .find(|&i| *b == Build::kit(i))
        .map_or_else(|| format!("{} RACER", upper(b.part(Slot::Engine).name, 20)), |i| upper(KIT[i], 20))
}

/// Where a swapping slot's new part and its old one are, at `t` of the way
/// through (0 to 1), in racer units above where they sit: the old one lifts
/// off and is gone; the new one drops in, lands and settles.
fn swap_offsets(t: f32) -> (Option<f32>, Option<f32>) {
    let t = t.max(0.0);
    let old = (t < 0.25).then(|| {
        let u = t / 0.25;
        3.6 * u * u
    });
    let new = if t < 0.25 {
        None
    } else if t < 0.55 {
        let u = (t - 0.25) / 0.3;
        Some(4.2 * (1.0 - u * u))
    } else {
        let b = (t - 0.55) / 0.12;
        Some(if b < 1.0 { 0.2 * (b * PI).sin() } else { 0.0 })
    };
    (new, old)
}

/// A dark soft patch laid flat on the floor.
fn shadow(m: &mut Mesh, at: V3, rx: f32, ry: f32) {
    let n = 10;
    for i in 0..n {
        let (a0, a1) = (i as f32 / n as f32 * TAU, (i + 1) as f32 / n as f32 * TAU);
        let p = |a: f32| v3(at.x + a.cos() * rx, at.y + a.sin() * ry, at.z + 0.015);
        tri(m, v3(at.x, at.y, at.z + 0.015), p(a0), p(a1), SHADOW);
    }
}

/// A knight from boxes, 1.4 tall, facing +x.
fn knight_figure(m: &mut Mesh, seat: usize, phase: f32, working: bool) {
    let tunic = SEAT + (seat % 6) as u8;
    let steel = mat::ARMOR;
    let dark = look::DARK_STEEL;
    let sw = phase.sin() * 0.14;
    for (sgn, off) in [(-1.0f32, sw), (1.0, -sw)] {
        // leg and boot
        cuboid(m, v3(-0.08 + off, sgn * 0.13 - 0.07, 0.0), v3(0.16 + off, sgn * 0.13 + 0.07, 0.14), dark);
        cuboid(m, v3(-0.06 + off * 0.5, sgn * 0.13 - 0.06, 0.14), v3(0.06 + off * 0.5, sgn * 0.13 + 0.06, 0.52), steel);
    }
    // tabard and cuirass
    cuboid(m, v3(-0.14, -0.22, 0.52), v3(0.14, 0.22, 0.98), tunic);
    cuboid(m, v3(0.1, -0.2, 0.6), v3(0.16, 0.2, 0.96), steel);
    cuboid(m, v3(-0.16, -0.26, 0.9), v3(0.14, 0.26, 1.02), steel);
    // arms, one raised to the work when a swap plays
    for (sgn, up) in [(-1.0f32, 0.0), (1.0, if working { 0.3 } else { 0.0 })] {
        cuboid(m, v3(-0.05, sgn * 0.3 - 0.05, 0.62 + up), v3(0.07, sgn * 0.3 + 0.05, 0.96 + up), steel);
        cuboid(m, v3(0.0, sgn * 0.3 - 0.04, 0.58 + up), v3(0.12, sgn * 0.3 + 0.04, 0.66 + up), dark);
    }
    // great helm with its visor slit and a plume in the seat's colour
    cuboid(m, v3(-0.12, -0.12, 1.02), v3(0.12, 0.12, 1.28), steel);
    cuboid(m, v3(0.12, -0.09, 1.14), v3(0.135, 0.09, 1.18), GLOW);
    spike(m, v3(0.0, 0.0, 1.28), v3(-0.2, 0.0, 1.5), 0.06, tunic);
    // a wrench in the right hand
    cuboid(m, v3(0.1, 0.26, 0.5), v3(0.17, 0.3, 0.8), dark);
    cuboid(m, v3(0.08, 0.24, 0.8), v3(0.2, 0.32, 0.88), mat::BRASS);
}

/// Hold each colour to a few steps, dithered across them.
fn arcade(px: &mut [[u8; 3]], w: usize) {
    const BAYER: [[f32; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];
    let levels = 9.0f32;
    for (i, p) in px.iter_mut().enumerate() {
        let d = BAYER[(i / w) % 4][(i % w) % 4] / 16.0 - 0.47;
        for c in p.iter_mut() {
            let v = f32::from(*c) / 255.0 * (levels - 1.0);
            *c = (((v + d).round().clamp(0.0, levels - 1.0)) / (levels - 1.0) * 255.0) as u8;
        }
    }
}

const GAIN: [u8; 3] = [118, 208, 128];
const LOSS: [u8; 3] = [226, 96, 84];
const INK: [u8; 3] = [214, 204, 176];
const SOFT: [u8; 3] = [132, 124, 104];
const BAR: [u8; 3] = [78, 152, 190];
const LOAD: [u8; 3] = [168, 132, 78];

/// The strip along the bottom: the focus knight's racer in bars, and the part
/// they stand at, large and turning, with what taking it would change.
fn panel(cv: &mut Canvas, seat: &Seat, inspect: &Raster) {
    let ph = 76;
    let (x0, y0, pw) = (6, cv.h as i32 - ph - 5, cv.w as i32 - 12);
    let cur = seat.build.stats();
    // what they are standing at, and what it would do
    let pick = seat.pick.filter(|p| seat.build.id(p.slot) != p.part.id);
    let preview = pick.map(|p| {
        let mut b = seat.build.clone();
        b.fit(p.part.id);
        b.stats()
    });
    cv.shade(x0, y0, pw, ph, [6, 6, 12], 0.82);
    cv.frame(x0, y0, pw, ph, [94, 78, 52]);
    // the knight and their racer
    cv.rect(x0 + 5, y0 + 5, 4, 9, seat_rgb(seat.bay));
    cv.text(x0 + 13, y0 + 5, &upper(&seat.name, 14), INK, 1);
    let bay = format!("BAY {}", seat.bay + 1);
    cv.text(x0 + 250 - Canvas::width(&bay, 1), y0 + 5, &bay, SOFT, 1);
    cv.text(x0 + 13, y0 + 15, &racer_name(&seat.build), [190, 150, 90], 1);
    // the bars, easing to the new numbers as a part lands
    let tween = |a: i32, b: i32| -> f32 {
        match &seat.swap {
            Some(s) => {
                let k = ((s.age as f32 - LAND_TICK as f32) / 8.0).clamp(0.0, 1.0);
                let k = k * k * (3.0 - 2.0 * k);
                a as f32 + (b - a) as f32 * k
            }
            None => b as f32,
        }
    };
    let (b4, af) = seat.swap.as_ref().map_or((cur, cur), |s: &SwapAnim| (s.before, s.after));
    let p = preview.unwrap_or(cur);
    let rows: [(&str, i32, i32, i32, bool); 5] = [
        ("SPEED", b4.speed, af.speed, p.speed, false),
        ("ACCEL", b4.accel, af.accel, p.accel, false),
        ("HANDLING", b4.handling, af.handling, p.handling, false),
        ("ARMOUR", b4.armour, af.armour, p.armour, false),
        ("WEIGHT", b4.weight, af.weight, p.weight, true),
    ];
    let shown = seat.shown;
    for (r, (name, from, to, pv, heavy)) in rows.iter().enumerate() {
        let (cx, y) = (x0 + 6, y0 + 27 + r as i32 * 9);
        cv.text(cx, y, name, SOFT, 1);
        let (bx, bw) = (cx + 52, 100);
        cv.rect(bx, y, bw, 6, [26, 26, 34]);
        let v = tween(*from, *to);
        cv.rect(bx, y, (v / 100.0 * bw as f32) as i32, 6, if *heavy { LOAD } else { BAR });
        // a preview: the part of the bar that would be gained or lost
        if preview.is_some() && pv != to {
            let (lo, hi) = ((*pv).min(*to), (*pv).max(*to));
            let (xa, xb) = (bx + (lo as f32 / 100.0 * bw as f32) as i32, bx + (hi as f32 / 100.0 * bw as f32) as i32);
            cv.rect(xa, y, (xb - xa).max(1), 6, if (pv > to) != *heavy { GAIN } else { LOSS });
        }
        cv.frame(bx - 1, y - 1, bw + 2, 8, [60, 52, 38]);
        cv.text(bx + bw + 5, y, &format!("{}", v.round() as i32), INK, 1);
        // the change a moment ago, or the change a swap would make
        let delta = if let Some((d, age)) = shown.filter(|_| preview.is_none()) {
            let fade = 1.0 - (age as f32 / DELTA_TICKS as f32).powi(2);
            let dv = [d.speed, d.accel, d.handling, d.armour, d.weight][r];
            (dv != 0).then_some((dv, fade))
        } else if preview.is_some() {
            let dv = pv - to;
            (dv != 0).then_some((dv, 1.0))
        } else {
            None
        };
        if let Some((dv, fade)) = delta {
            let c = mix([20, 20, 28], if (dv > 0) != *heavy { GAIN } else { LOSS }, fade.clamp(0.0, 1.0));
            cv.text(bx + bw + 28, y, &format!("{}{}", if dv > 0 { "+" } else { "-" }, dv.abs()), c, 1);
        }
    }
    // the part at their feet: a picture of it turning, and what it is
    let sx = x0 + 298;
    cv.rect(sx - 10, y0 + 5, 1, ph - 10, [60, 52, 38]);
    let Some(p) = seat.pick else {
        cv.text(sx, y0 + 30, "WALK TO A RACK", SOFT, 1);
        cv.text(sx, y0 + 42, "E  FIT THE PART  F  UNDO", [150, 130, 90], 1);
        return;
    };
    let (iw, ih) = (inspect.w as i32, inspect.h as i32);
    let (ix, iy) = (sx, y0 + 6);
    for yy in 0..ih {
        for xx in 0..iw {
            cv.dot(ix + xx, iy + yy, inspect.rgb[(yy * iw + xx) as usize]);
        }
    }
    cv.frame(ix - 1, iy - 1, iw + 2, ih + 2, [94, 78, 52]);
    let tx = ix + iw + 10;
    let worn = seat.build.id(p.slot) == p.part.id;
    cv.text(tx, iy, p.slot.name(), SOFT, 1);
    cv.text(tx, iy + 10, &upper(p.part.name, 24), INK, 1);
    for (i, line) in wrap(p.part.blurb, 36).iter().take(3).enumerate() {
        cv.text(tx, iy + 24 + i as i32 * 9, line, SOFT, 1);
    }
    let tag = if worn { "FITTED" } else { "E  FIT" };
    cv.text(tx, iy + ih - 7, tag, if worn { SOFT } else { [226, 190, 110] }, 1);
}

/// A sentence cut into lines at spaces, `n` characters wide.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    for word in s.to_uppercase().split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + word.chars().count() <= n => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// How many triangles this frame has in its movers (for tests and the dump).
impl View {
    pub fn movers(&self) -> usize {
        self.movers.tris.len()
    }
}
