//! The pit floor: raked sand with scuffs and old blood baked once into a
//! decal table, the dais at the middle, a stone pad under every pickup and
//! the flags before the gates.
use super::{CELL, COLS, ROWS};
use crate::render::overworld::ink::{hash, vnoise};
use crate::rules::deathmatch as dm;

/// Decal texels to a world unit.
const RES: f32 = 16.0;
const W: usize = (COLS as f32 * CELL * RES) as usize;
const H: usize = (ROWS as f32 * CELL * RES) as usize;

/// Old blood and scuffs, baked: one byte of each per texel.
pub(super) struct Decals {
    blood: Vec<u8>,
    scuff: Vec<u8>,
}

fn unit(h: u32) -> f32 {
    (h & 0xffff) as f32 / 65535.0
}

impl Decals {
    pub(super) fn build(open: &dyn Fn(f32, f32) -> bool) -> Decals {
        let mut d = Decals { blood: vec![0; W * H], scuff: vec![0; W * H] };
        // Pools where knights fell, far from the first spawn but near the stone.
        let mut made = 0;
        let mut n = 0u32;
        while made < 20 && n < 400 {
            let (a, b, c) = (unit(hash(n as i32, 1, 0xb1)), unit(hash(n as i32, 2, 0xb1)), unit(hash(n as i32, 3, 0xb1)));
            n += 1;
            let (x, y) = (3.5 + a * 41.0, 3.5 + b * 29.0);
            if !open(x, y) || !open(x + 1.0, y) || !open(x - 1.0, y) || !open(x, y + 1.0) || !open(x, y - 1.0) {
                continue;
            }
            let r = 0.45 + c * c * 1.1;
            d.pool(x, y, r, made as u32);
            made += 1;
        }
        // Scuffs: boot drags and skids, three short dashes along a gentle curve.
        let mut made = 0;
        let mut n = 0u32;
        while made < 170 && n < 2000 {
            let (a, b, c, e) = (unit(hash(n as i32, 4, 0x5c)), unit(hash(n as i32, 5, 0x5c)), unit(hash(n as i32, 6, 0x5c)), unit(hash(n as i32, 7, 0x5c)));
            n += 1;
            let (x, y) = (3.0 + a * 42.0, 3.0 + b * 30.0);
            if !open(x, y) {
                continue;
            }
            let ang = c * std::f32::consts::TAU;
            let bend = (e - 0.5) * 0.9;
            let (mut px, mut py, mut th) = (x, y, ang);
            let len = 0.5 + e * 0.9;
            for k in 0..4 {
                let (nx, ny) = (px + th.cos() * len * 0.25, py + th.sin() * len * 0.25);
                if k != 2 {
                    d.stroke(&mut |t, v| t.scuff[v] = t.scuff[v].max(150), px, py, nx, ny, 0.05 + 0.04 * c);
                }
                (px, py, th) = (nx, ny, th + bend * 0.4);
            }
            made += 1;
        }
        d
    }

    fn texel(x: f32, y: f32) -> Option<usize> {
        let (tx, ty) = ((x * RES) as i32, (y * RES) as i32);
        ((0..W as i32).contains(&tx) && (0..H as i32).contains(&ty)).then(|| ty as usize * W + tx as usize)
    }

    fn stroke(&mut self, put: &mut dyn FnMut(&mut Decals, usize), x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let steps = (((x1 - x0).hypot(y1 - y0)) * RES * 1.5).ceil().max(1.0) as i32;
        let reach = (r * RES).ceil() as i32;
        for i in 0..=steps {
            let f = i as f32 / steps as f32;
            let (x, y) = (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f);
            let (cx, cy) = ((x * RES) as i32, (y * RES) as i32);
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    if ((dx * dx + dy * dy) as f32).sqrt() > r * RES {
                        continue;
                    }
                    if let Some(v) = Self::texel((cx + dx) as f32 / RES, (cy + dy) as f32 / RES) {
                        put(self, v);
                    }
                }
            }
        }
    }

    /// An irregular pool with spatter round it and sometimes a drag out of it.
    fn pool(&mut self, cx: f32, cy: f32, r: f32, k: u32) {
        let reach = r * 1.7;
        let (x0, x1, y0, y1) = (cx - reach, cx + reach, cy - reach, cy + reach);
        let mut ty = (y0 * RES) as i32;
        while (ty as f32) < y1 * RES {
            let mut tx = (x0 * RES) as i32;
            while (tx as f32) < x1 * RES {
                let (x, y) = (tx as f32 / RES, ty as f32 / RES);
                let (dx, dy) = (x - cx, y - cy);
                let dist = dx.hypot(dy);
                let th = dy.atan2(dx);
                let rim = r * (0.75 + 0.55 * vnoise(th.cos() * 1.7 + cx * 3.1, th.sin() * 1.7 + cy * 3.1 + k as f32, 0x6b));
                if dist < rim {
                    if let Some(v) = Self::texel(x, y) {
                        let core = 1.0 - dist / rim;
                        let grain = 0.8 + 0.2 * unit(hash(tx, ty, 0x77));
                        self.blood[v] = self.blood[v].max((90.0 + 165.0 * core.sqrt() * grain).min(255.0) as u8);
                    }
                }
                tx += 1;
            }
            ty += 1;
        }
        for j in 0..9u32 {
            let a = unit(hash(k as i32, j as i32, 0x91)) * std::f32::consts::TAU;
            let rr = r * (1.15 + 1.7 * unit(hash(k as i32, j as i32 + 40, 0x92)));
            let (x, y) = (cx + a.cos() * rr, cy + a.sin() * rr);
            let s = 0.05 + 0.1 * unit(hash(k as i32, j as i32 + 80, 0x93));
            self.stroke(&mut |t, v| t.blood[v] = t.blood[v].max(200), x, y, x, y, s);
        }
        if k % 3 == 0 {
            let a = unit(hash(k as i32, 200, 0x94)) * std::f32::consts::TAU;
            let len = 1.6 + 2.0 * unit(hash(k as i32, 201, 0x94));
            self.stroke(&mut |t, v| t.blood[v] = t.blood[v].max(150), cx, cy, cx + a.cos() * len, cy + a.sin() * len, 0.1);
        }
    }

    #[inline]
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        match Self::texel(x, y) {
            Some(v) => (f32::from(self.blood[v]) / 255.0, f32::from(self.scuff[v]) / 255.0),
            None => (0.0, 0.0),
        }
    }
}

const SAND: [f32; 3] = [0.55, 0.45, 0.31];
const FLAG: [f32; 3] = [0.40, 0.38, 0.34];
const DAIS: (f32, f32) = (25.0, 17.0);

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// How near a wall (and so how shaded by it) the floor at `(x, y)` is: 0 at
/// the foot, 1 by the time it is a stride away.
fn open_ground(solid: &dyn Fn(i32, i32) -> bool, x: f32, y: f32) -> f32 {
    let (cx, cy) = ((x / CELL) as i32, (y / CELL) as i32);
    let (fx, fy) = (x - cx as f32 * CELL, y - cy as f32 * CELL);
    let mut near = 9.0f32;
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        if solid(cx + dx, cy + dy) {
            near = near.min(match (dx, dy) {
                (-1, 0) => fx,
                (1, 0) => CELL - fx,
                (0, -1) => fy,
                _ => CELL - fy,
            });
        }
    }
    for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        if solid(cx + dx, cy + dy) {
            let ex = if dx < 0 { fx } else { CELL - fx };
            let ey = if dy < 0 { fy } else { CELL - fy };
            near = near.min(ex.hypot(ey));
        }
    }
    (near / 1.2).clamp(0.0, 1.0)
}

/// The floor's colour in an even light at `(x, y)`: sand, rakes, flags, pads, stains, scuffs.
fn albedo(solid: &dyn Fn(i32, i32) -> bool, decals: &Decals, x: f32, y: f32) -> [f32; 3] {
    let big = vnoise(x * 0.16, y * 0.16, 0x11);
    let mid = vnoise(x * 0.9, y * 0.9, 0x12);
    let tone = 0.80 + 0.36 * big + (mid - 0.5) * 0.16;
    let (rx, ry) = (x - 24.0, y - 18.0);
    let furrow = 0.5 + 0.5 * (rx.hypot(ry) * 8.5 + vnoise(x * 0.25, y * 0.25, 0x13) * 4.0).sin();
    let grain = unit(hash((x * 9.0) as i32, (y * 9.0) as i32, 0x14));
    let t = tone * (1.0 - 0.09 * furrow) * (1.0 + (grain - 0.5) * 0.14);
    let mut a = [SAND[0] * t, SAND[1] * t, SAND[2] * t];
    // Stone under the gates and the dais.
    let front = if x < 24.0 { x - 6.0 } else { 42.0 - x };
    let in_gate = (-0.2..2.8 + 0.9 * vnoise(y * 0.9, 3.0, 0x31)).contains(&front) && (y - 17.0).abs() < 3.3 + 0.5 * vnoise(x, 7.0, 0x32);
    let (dx, dy) = (x - DAIS.0, y - DAIS.1);
    let dr = dx.hypot(dy);
    let on_dais = dr < 2.6 + 0.25 * vnoise(dx * 2.0, dy * 2.0, 0x33);
    let mut stone = 0.0f32;
    if in_gate {
        stone = 0.8;
    }
    if on_dais {
        stone = 0.92;
    }
    // A pad under every pickup, the ring picking out what it holds.
    let mut ring = None;
    for &(px, py, kind) in dm::PICKUPS.iter().filter(|p| !matches!(p.2, dm::Pickup::Power)) {
        let pr = (x - px).hypot(y - py);
        if pr < 1.15 {
            stone = stone.max(0.95);
            let tint = match kind {
                dm::Pickup::Health => [0.55, 0.14, 0.10],
                dm::Pickup::Armor => [0.36, 0.44, 0.52],
                dm::Pickup::Power => [0.7, 0.5, 0.2],
            };
            if (0.82..1.0).contains(&pr) {
                ring = Some(tint);
            }
        }
    }
    if stone > 0.0 {
        // Flags, with joints; the dais cut with rings and rays.
        let (jx, jy) = ((x * 0.9 + (y * 0.9).floor() * 0.5).fract(), (y * 0.9).fract());
        let joint = if jx < 0.07 || jy < 0.07 { 0.62 } else { 1.0 };
        let id = unit(hash((x * 0.9) as i32, (y * 0.9) as i32, 0x41));
        let mut f = joint * (0.88 + 0.24 * id);
        if on_dais {
            let th = dy.atan2(dx);
            let rayc = ((th * 8.0 / std::f32::consts::TAU * 2.0).sin()).abs();
            if ((dr - 1.55).abs() < 0.07) || ((dr - 2.4).abs() < 0.08) || (dr < 1.5 && dr > 0.6 && rayc < 0.09) {
                f *= 0.55;
            }
            if dr > 0.55 && dr < 0.75 {
                f *= 1.2;
            }
        }
        let drift = vnoise(x * 0.7, y * 0.7, 0x42);
        let cover = ((drift - 0.55) * 3.0).clamp(0.0, 0.8) * (1.0 - stone * 0.5);
        let s = mix([FLAG[0] * f, FLAG[1] * f, FLAG[2] * f], a, cover);
        a = mix(a, s, stone.min(1.0));
        if let Some(c) = ring {
            a = mix(a, c, 0.75);
        }
    }
    // Blood and scuffs.
    let (blood, scuff) = decals.at(x, y);
    if scuff > 0.0 {
        a = mix(a, [a[0] * 0.62, a[1] * 0.60, a[2] * 0.58], scuff * 0.9);
    }
    if blood > 0.0 {
        let dry = [0.30, 0.11, 0.07];
        let fresh = [0.45, 0.06, 0.05];
        let c = mix(dry, fresh, (blood - 0.5).max(0.0) * 1.4);
        a = mix(a, c, (blood * 1.15).min(0.92));
    }
    // The foot of a wall is in its shade.
    let shade = 0.62 + 0.38 * open_ground(solid, x, y);
    [a[0] * shade, a[1] * shade, a[2] * shade]
}

/// One resolution of the baked floor.
struct Level {
    res: f32,
    w: usize,
    h: usize,
    px: Vec<[u8; 3]>,
}

/// The floor, baked once at 12 dots to the unit, and two blurrier copies for
/// what lies farther off, so distance never sparkles.
pub(super) struct Floor {
    levels: [Level; 3],
}

impl Floor {
    pub(super) fn bake(solid: &(dyn Fn(i32, i32) -> bool + Sync), decals: &Decals) -> Floor {
        let res = 12.0f32;
        let (w, h) = ((COLS as f32 * CELL * res) as usize, (ROWS as f32 * CELL * res) as usize);
        let mut px = vec![[0u8; 3]; w * h];
        let rows_per = h.div_ceil(6);
        let jobs: Vec<super::Job> = px
            .chunks_mut(rows_per * w)
            .enumerate()
            .map(|(k, chunk)| {
                Box::new(move || {
                    for (i, out) in chunk.iter_mut().enumerate() {
                        let (tx, ty) = (i % w, k * rows_per + i / w);
                        let a = albedo(solid, decals, (tx as f32 + 0.5) / res, (ty as f32 + 0.5) / res);
                        *out = [(a[0] * 255.0).min(255.0) as u8, (a[1] * 255.0).min(255.0) as u8, (a[2] * 255.0).min(255.0) as u8];
                    }
                }) as super::Job
            })
            .collect();
        super::together(jobs);
        let l0 = Level { res, w, h, px };
        let down = |l: &Level| {
            let (w, h) = (l.w / 2, l.h / 2);
            let mut px = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [0u32; 3];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = l.px[(y * 2 + dy) * l.w + x * 2 + dx];
                        for c in 0..3 {
                            sum[c] += u32::from(p[c]);
                        }
                    }
                    px.push([(sum[0] / 4) as u8, (sum[1] / 4) as u8, (sum[2] / 4) as u8]);
                }
            }
            Level { res: l.res / 2.0, w, h, px }
        };
        let l1 = down(&l0);
        let l2 = down(&l1);
        Floor { levels: [l0, l1, l2] }
    }

    /// The floor's colour at `(x, y)` seen from `depth` away.
    #[inline]
    pub(super) fn at(&self, x: f32, y: f32, depth: f32) -> [f32; 3] {
        let l = &self.levels[if depth < 9.0 { 0 } else if depth < 19.0 { 1 } else { 2 }];
        let tx = ((x * l.res) as usize).min(l.w - 1);
        let ty = ((y * l.res) as usize).min(l.h - 1);
        let p = l.px[ty * l.w + tx];
        [f32::from(p[0]) * (1.0 / 255.0), f32::from(p[1]) * (1.0 / 255.0), f32::from(p[2]) * (1.0 / 255.0)]
    }
}

/// A knight standing at `(fx, fy)` throws a small shadow on `(x, y)`.
#[inline]
pub(super) fn shadowed(x: f32, y: f32, feet: &[(f32, f32)]) -> f32 {
    let mut shade = 1.0;
    for &(fx, fy) in feet {
        let d2 = (x - fx) * (x - fx) + (y - fy) * (y - fy);
        if d2 < 0.55 {
            shade *= 0.5 + 0.5 * (d2 / 0.55);
        }
    }
    shade
}
