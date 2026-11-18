//! The pen every effect draws with: shapes are fields of heat over model
//! space (game pixels), and the pen samples them at the grain it is drawing
//! at, so one shape reads as a pixel at grain 1 and as a small lit form with
//! a hot core at grain 4.
//!
//! Heat goes from 0 to 1 and becomes an ink by its ramp: the lightest ink
//! is the hot core, the darkest the last of the glow. Neighbouring inks meet
//! in a narrow ordered dither, never a blend, and the faintest heat is a
//! sparse dither of the darkest inks, a halo that lets the floor show
//! through. Every ink is one of the realm's.
//!
//! How an ink meets what is there is the layer's [`Mode`]: light only ever
//! brightens (a glow over a knight leaves the knight), over covers at its
//! depth, under goes only on bare ground (smoke, wakes, soot).

use super::rng::bayer;
use crate::palette::{Ink, Palette, Ramp};
use crate::raster::{PartId, Raster};
use serde::{Deserialize, Serialize};

pub const CUT: f32 = 0.06;
pub const HALO: f32 = 0.3;
const SEAM: f32 = 0.2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Light,
    Over,
    Under,
}

pub struct Pen<'a> {
    raster: &'a mut Raster,
    g: f32,
    off: (f32, f32),
    pub depth: f32,
    pub part: PartId,
    pub mode: Mode,
    luma: Vec<f32>,
    pub laid: u32,
}

impl<'a> Pen<'a> {
    pub fn new(raster: &'a mut Raster, grain: f32, off: (f32, f32)) -> Pen<'a> {
        let p = Palette::realm();
        let luma = (0..=255u8).map(|i| p.luma(Ink(i))).collect();
        Pen { raster, g: grain, off, depth: 0.0, part: 0, mode: Mode::Light, luma, laid: 0 }
    }

    pub fn grain(&self) -> f32 {
        self.g
    }

    pub fn px(&self) -> f32 {
        1.0 / self.g
    }

    pub fn thin(&self, r: f32) -> f32 {
        r.max(0.55 / self.g)
    }

    pub fn lay(&mut self, x: i32, y: i32, ink: Ink) {
        if ink.is_clear() || x < 0 || y < 0 || x as u32 >= self.raster.w || y as u32 >= self.raster.h {
            return;
        }
        let i = (y as u32 * self.raster.w + x as u32) as usize;
        let (here, d) = (self.raster.ink[i], self.raster.depth[i]);
        let under = d < self.depth - 2000.0;
        let ok = match self.mode {
            Mode::Over => d <= self.depth,
            Mode::Light => d <= self.depth && (here.is_clear() || under || self.luma[ink.0 as usize] > self.luma[here.0 as usize]),
            Mode::Under => here.is_clear(),
        };
        if ok {
            self.raster.ink[i] = ink;
            self.raster.depth[i] = if self.mode == Mode::Under { self.depth - 4000.0 } else { self.depth };
            self.raster.part[i] = self.part;
            self.laid += 1;
        }
    }

    fn span(&self, (x0, y0, x1, y1): (f32, f32, f32, f32)) -> Option<(i32, i32, i32, i32)> {
        let g = self.g;
        let lo = |v: f32, o: f32| ((v + o) * g - 0.5).floor().max(0.0);
        let hi = |v: f32, o: f32, n: u32| ((v + o) * g - 0.5).ceil().min(n as f32 - 1.0);
        let (px0, px1) = (lo(x0, self.off.0), hi(x1, self.off.0, self.raster.w));
        let (py0, py1) = (lo(y0, self.off.1), hi(y1, self.off.1, self.raster.h));
        (px0 <= px1 && py0 <= py1).then_some((px0 as i32, px1 as i32, py0 as i32, py1 as i32))
    }

    fn sample(&self, px: i32, py: i32) -> (f32, f32) {
        ((px as f32 + 0.5) / self.g - self.off.0, (py as f32 + 0.5) / self.g - self.off.1)
    }

    pub fn field(&mut self, bbox: (f32, f32, f32, f32), ramp: &Ramp, mut heat: impl FnMut(f32, f32) -> f32) {
        let Some((px0, px1, py0, py1)) = self.span(bbox) else { return };
        for py in py0..=py1 {
            for px in px0..=px1 {
                let (x, y) = self.sample(px, py);
                if let Some(ink) = tone(ramp, heat(x, y), px, py) {
                    self.lay(px, py, ink);
                }
            }
        }
    }

    pub fn field_ink(&mut self, bbox: (f32, f32, f32, f32), mut ink: impl FnMut(f32, f32, i32, i32) -> Option<Ink>) {
        let Some((px0, px1, py0, py1)) = self.span(bbox) else { return };
        for py in py0..=py1 {
            for px in px0..=px1 {
                let (x, y) = self.sample(px, py);
                if let Some(i) = ink(x, y, px, py) {
                    self.lay(px, py, i);
                }
            }
        }
    }
}

pub fn tone(ramp: &Ramp, i: f32, x: i32, y: i32) -> Option<Ink> {
    if i.is_nan() || i < CUT {
        return None;
    }
    let i = i.min(1.0);
    if i < HALO && bayer(x, y) >= (i - CUT) / (HALO - CUT) * 0.8 {
        return None;
    }
    let n = ramp.0.len();
    if n == 1 {
        return Some(ramp.0[0]);
    }
    let f = i * (n - 1) as f32;
    let lo = f.floor();
    let t = f - lo;
    let up = if t < 0.5 - SEAM {
        false
    } else if t > 0.5 + SEAM {
        true
    } else {
        bayer(x, y) < (t - (0.5 - SEAM)) / (2.0 * SEAM)
    };
    Some(ramp.0[(lo as usize + usize::from(up)).min(n - 1)])
}

pub fn seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-9 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    let (qx, qy) = (a.0 + dx * t, a.1 + dy * t);
    ((p.0 - qx).hypot(p.1 - qy), t)
}

pub fn bounds(pts: &[(f32, f32)], r: f32) -> (f32, f32, f32, f32) {
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in pts {
        b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
    }
    (b.0 - r, b.1 - r, b.2 + r, b.3 + r)
}

pub fn ellipse_gap(dx: f32, dy: f32, r: f32, squash: f32) -> f32 {
    let sq = squash.max(0.05);
    let (ux, uy) = (dx, dy / sq);
    let rho = ux.hypot(uy);
    if rho < 1e-6 {
        return r * sq;
    }
    let (c, s) = (ux / rho, uy / rho);
    (rho - r).abs() * (c * c + sq * sq * s * s).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_tones_dark_to_light_and_the_faintest_is_sparse() {
        let p = Palette::realm();
        let fire = Ramp::parse("BpR@69", p).unwrap();
        assert_eq!(tone(&fire, 1.0, 0, 0), Some(p.ink('9').unwrap()));
        assert_eq!(tone(&fire, 0.01, 0, 0), None);
        let halo = (0..16).filter(|&k| tone(&fire, 0.12, k % 4, k / 4).is_some()).count();
        assert!(halo > 0 && halo < 8, "{halo}");
    }

    #[test]
    fn a_thin_stroke_lays_a_pixel_at_grain_one() {
        let mut r = Raster::new(8, 8);
        let ramp = Ramp::parse("BpR@69", Palette::realm()).unwrap();
        let mut pen = Pen::new(&mut r, 1.0, (0.0, 0.0));
        let w = pen.thin(0.1);
        pen.field((1.0, 1.0, 7.0, 7.0), &ramp, |x, y| if seg((x, y), (1.5, 4.5), (6.5, 4.5)).0 <= w { 1.0 } else { 0.0 });
        assert_eq!(r.ink_rows(Palette::realm())[4], ".999999.");
    }

    #[test]
    fn light_never_dims_and_under_keeps_to_bare_ground() {
        let p = Palette::realm();
        let (dark, white) = (p.ink('B').unwrap(), p.ink('9').unwrap());
        let mut r = Raster::new(2, 1);
        let mut pen = Pen::new(&mut r, 1.0, (0.0, 0.0));
        pen.lay(0, 0, white);
        pen.lay(0, 0, dark);
        pen.mode = Mode::Under;
        pen.lay(0, 0, dark);
        pen.lay(1, 0, dark);
        assert_eq!(r.ink_rows(p)[0], "9B");
    }
}
