//! The pit's light: sconces on the walls and braziers on the sand, each one
//! a warm pool that falls on floor and wall alike and stops where a pillar
//! stands between. Where a light can be seen from is worked out once, a
//! half-unit at a time, with a soft edge; the falloff and the flicker are
//! worked out per dot.
use super::{CELL, COLS, ROWS};
use crate::render::overworld::ink::hash;

const GRID: f32 = 0.5;
pub(super) const GW: usize = (COLS as f32 * CELL / GRID) as usize;
pub(super) const GH: usize = (ROWS as f32 * CELL / GRID) as usize;

pub(super) struct Light {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) z: f32,
    pub(super) r2: f32,
    pub(super) inv_r2: f32,
    pub(super) k: f32,
    pub(super) tint: [f32; 3],
    pub(super) brazier: bool,
}

const BIN: f32 = 4.0;
const BW: usize = (COLS as f32 * CELL / BIN) as usize;
const BH: usize = (ROWS as f32 * CELL / BIN) as usize;

/// A light that lives a few ticks: a blow's flash, a bolt in flight, a muzzle.
/// It has no memory of the walls, so it stays small.
pub(in super::super) struct Flash {
    pub(in super::super) x: f32,
    pub(in super::super) y: f32,
    pub(in super::super) z: f32,
    pub(in super::super) r: f32,
    pub(in super::super) k: f32,
    pub(in super::super) tint: [f32; 3],
}

pub(super) struct Lights {
    pub(super) list: Vec<Light>,
    vis: Vec<Vec<u8>>,
    /// Which lights can reach each four-unit square of the pit.
    bins: Vec<Vec<u16>>,
}

fn blocked(solid: &(dyn Fn(i32, i32) -> bool + Sync), a: (f32, f32), b: (f32, f32)) -> bool {
    let n = ((b.0 - a.0).hypot(b.1 - a.1) * 5.0).ceil().max(1.0) as i32;
    (1..n).any(|i| {
        let f = i as f32 / n as f32;
        let (x, y) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
        solid((x / CELL).floor() as i32, (y / CELL).floor() as i32)
    })
}

impl Lights {
    pub(super) fn build(list: Vec<Light>, solid: &(dyn Fn(i32, i32) -> bool + Sync)) -> Lights {
        let one = |l: &Light| {
                let mut v = vec![0u8; GW * GH];
                for gy in 0..GH {
                    for gx in 0..GW {
                        let (cx, cy) = ((gx as f32 + 0.5) * GRID, (gy as f32 + 0.5) * GRID);
                        if solid((cx / CELL) as i32, (cy / CELL) as i32) {
                            continue;
                        }
                        let (dx, dy) = (cx - l.x, cy - l.y);
                        if dx * dx + dy * dy > l.r2 {
                            continue;
                        }
                        let mut seen = 0u32;
                        for (jx, jy) in [(-0.17, -0.17), (0.17, -0.17), (-0.17, 0.17), (0.17, 0.17)] {
                            if !blocked(solid, (l.x, l.y), (cx + jx, cy + jy)) {
                                seen += 1;
                            }
                        }
                        v[gy * GW + gx] = (seen * 255 / 4) as u8;
                    }
                }
                v
        };
        let mut vis: Vec<Vec<u8>> = vec![Vec::new(); list.len()];
        let one = &one;
        let jobs: Vec<super::Job> = list.iter().zip(vis.iter_mut()).map(|(l, slot)| Box::new(move || *slot = one(l)) as super::Job).collect();
        super::together(jobs);
        let mut bins = vec![Vec::new(); BW * BH];
        for by in 0..BH {
            for bx in 0..BW {
                let (x0, y0) = (bx as f32 * BIN, by as f32 * BIN);
                for (i, l) in list.iter().enumerate() {
                    let (nx, ny) = (l.x.clamp(x0, x0 + BIN), l.y.clamp(y0, y0 + BIN));
                    let (dx, dy) = (l.x - nx, l.y - ny);
                    // Walls stand a little proud of their square's edge: keep a stride of slack.
                    if dx * dx + dy * dy < l.r2 + 1.0 {
                        bins[by * BW + bx].push(i as u16);
                    }
                }
            }
        }
        Lights { list, vis, bins }
    }

    /// Each light's brightness this tick: a slow breath and a quick stutter.
    pub(super) fn flicker(&self, ticks: u32) -> Vec<f32> {
        (0..self.list.len())
            .map(|i| {
                let quick = (hash(i as i32, (ticks / 2) as i32, 7) & 0xff) as f32 / 255.0;
                let slow = (ticks as f32 * 0.19 + i as f32 * 1.7).sin();
                let swing = if self.list[i].brazier { 0.12 } else { 0.09 };
                0.88 + swing * quick + 0.05 * slow
            })
            .collect()
    }

    /// The torchlight on a surface at `p` facing `n`, whose open ground
    /// (where the lights are seen from) is at `ground`.
    #[inline]
    pub(super) fn at(&self, p: [f32; 3], n: [f32; 3], ground: (f32, f32), flick: &[f32], flashes: &[Flash]) -> [f32; 3] {
        let (gx, gy) = ((ground.0 / GRID) as usize, (ground.1 / GRID) as usize);
        let mut out = [0.0f32; 3];
        for l in flashes {
            let (dx, dy, dz) = (l.x - p[0], l.y - p[1], l.z - p[2]);
            let d2 = dx * dx + dy * dy + dz * dz;
            let r2 = l.r * l.r;
            if d2 >= r2 {
                continue;
            }
            let ndl = ((n[0] * dx + n[1] * dy + n[2] * dz) / d2.sqrt().max(0.2)).max(0.0);
            let att = 1.0 - d2 / r2;
            let e = l.k * att * att * (d2 / (d2 + 1.5)) * (0.35 + 0.65 * ndl);
            out[0] += e * l.tint[0];
            out[1] += e * l.tint[1];
            out[2] += e * l.tint[2];
        }
        if gx >= GW || gy >= GH {
            return out;
        }
        let cell = gy * GW + gx;
        let bin = ((ground.1 / BIN) as usize).min(BH - 1) * BW + ((ground.0 / BIN) as usize).min(BW - 1);
        for &i in &self.bins[bin] {
            let i = usize::from(i);
            let l = &self.list[i];
            let (dx, dy, dz) = (l.x - p[0], l.y - p[1], l.z - p[2]);
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 >= l.r2 {
                continue;
            }
            let v = self.vis[i][cell];
            if v == 0 {
                continue;
            }
            let ndl = ((n[0] * dx + n[1] * dy + n[2] * dz) / d2.sqrt().max(0.2)).max(0.0);
            let att = 1.0 - d2 * l.inv_r2;
            let e = l.k * att * att * (d2 / (d2 + 2.5)) * (0.28 + 0.72 * ndl) * f32::from(v) * (1.0 / 255.0) * flick[i];
            out[0] += e * l.tint[0];
            out[1] += e * l.tint[1];
            out[2] += e * l.tint[2];
        }
        out
    }
}
