//! RGB drawing at two samples per world pixel. Coverage belongs to geometry;
//! no image-wide smoothing is used. Material colours originate in the ink kit.
use super::super::super::ink::{Img, Rgb, hash};

use crate::render::art;
pub use crate::render::art::{Point, Ramp, material, mix};

#[derive(Clone)]
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Rgb>,
    pub origin: (f32, f32),
    pub fx: Vec<Rgb>,
    pub ground: Vec<Rgb>,
    /// Samples the screen's own marks hold (`stamp_kept`): the atmosphere
    /// composited after the raster never dots them. Empty: none.
    pub kept: Vec<bool>,
}
impl Canvas {
    pub fn new(w: usize, h: usize, c: Rgb) -> Self {
        Self {
            w,
            h,
            px: vec![c; w * h],
            origin: (0.0, 0.0),
            fx: vec![[0, 0, 0]; w * h],
            ground: Vec::new(),
            kept: Vec::new(),
        }
    }
    pub fn view(&self, origin: (i32, i32)) -> Self {
        let mut out = Canvas::new(768, 448, [0, 0, 0]);
        let dx = (origin.0 as f32 - self.origin.0) as i32 * 2;
        let dy = (origin.1 as f32 - self.origin.1) as i32 * 2;
        for y in 0..448 {
            let sy = y + dy;
            if sy < 0 || sy >= self.h as i32 {
                continue;
            }
            let left = 0.max(-dx);
            let right = 768.min(self.w as i32 - dx);
            if right <= left {
                continue;
            }
            let source = sy as usize * self.w + (left + dx) as usize;
            let dest = y as usize * 768 + left as usize;
            out.px[dest..dest + (right - left) as usize]
                .copy_from_slice(&self.px[source..source + (right - left) as usize]);
        }
        for y in 0..448 {
            let sy = y + dy;
            if sy < 0 || sy >= self.h as i32 {
                continue;
            }
            let left = 0.max(-dx);
            let right = 768.min(self.w as i32 - dx);
            if right <= left {
                continue;
            }
            let source = sy as usize * self.w + (left + dx) as usize;
            let dest = y as usize * 768 + left as usize;
            out.fx[dest..dest + (right - left) as usize]
                .copy_from_slice(&self.fx[source..source + (right - left) as usize]);
        }
        out.origin = (origin.0 as f32, origin.1 as f32);
        out
    }
    fn bounds(&self, x: f32, y: f32, w: f32, h: f32) -> (usize, usize, usize, usize) {
        let (x, y) = (x - self.origin.0, y - self.origin.1);
        (
            (x * 2.0).floor().clamp(0.0, self.w as f32) as usize,
            (y * 2.0).floor().clamp(0.0, self.h as f32) as usize,
            ((x + w) * 2.0).ceil().clamp(0.0, self.w as f32) as usize,
            ((y + h) * 2.0).ceil().clamp(0.0, self.h as f32) as usize,
        )
    }
    pub fn blend(&mut self, x: usize, y: usize, c: Rgb, a: f32) {
        if x < self.w && y < self.h {
            let i = y * self.w + x;
            self.px[i] = mix(self.px[i], c, a);
        }
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgb) {
        let (x0, y0, x1, y1) = self.bounds(x, y, w, h);
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.px[yy * self.w + xx] = c;
            }
        }
    }
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, r: Ramp, vertical: bool) {
        let (x0, y0, x1, y1) = self.bounds(x, y, w, h);
        for yy in y0..y1 {
            for xx in x0..x1 {
                let t = if vertical {
                    ((yy as f32 + 0.5) * 0.5 + self.origin.1 - y) / h
                } else {
                    ((xx as f32 + 0.5) * 0.5 + self.origin.0 - x) / w
                };
                self.px[yy * self.w + xx] = r.at(t);
            }
        }
    }
    pub fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32, r: Ramp, seed: u32) {
        let (x0, y0, x1, y1) =
            self.bounds(x - rx - 0.5, y - ry - 0.5, rx * 2.0 + 1.0, ry * 2.0 + 1.0);
        for yy in y0..y1 {
            for xx in x0..x1 {
                let dx = ((xx as f32 + 0.5) * 0.5 + self.origin.0 - x) / rx;
                let dy = ((yy as f32 + 0.5) * 0.5 + self.origin.1 - y) / ry;
                let d = dx * dx + dy * dy;
                let coverage = ((1.0 - d) * rx.min(ry) * 2.0 + 0.5).clamp(0.0, 1.0);
                if coverage <= 0.0 {
                    continue;
                }
                let z = (1.0 - d).max(0.0).sqrt();
                let grain = (hash(xx as i32, yy as i32, seed) % 31) as f32 / 31.0 - 0.5;
                let light =
                    (0.46 - dx * 0.22 - dy * 0.27 + z * 0.19 + grain * 0.055).clamp(0.0, 1.0);
                // Coloured selective rim on the lower/right silhouette, a warm
                // sliver on the upper/left. Interior form remains a clean ramp.
                let c = if d > 0.88 {
                    if dx + dy < -0.4 {
                        r.at(0.83)
                    } else {
                        r.at(0.1)
                    }
                } else {
                    r.at(light)
                };
                self.blend(xx, yy, c, coverage);
            }
        }
    }
    pub fn flat_ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32, c: Rgb, a: f32) {
        let (x0, y0, x1, y1) =
            self.bounds(x - rx - 0.5, y - ry - 0.5, rx * 2.0 + 1.0, ry * 2.0 + 1.0);
        for yy in y0..y1 {
            for xx in x0..x1 {
                let dx = ((xx as f32 + 0.5) * 0.5 + self.origin.0 - x) / rx;
                let dy = ((yy as f32 + 0.5) * 0.5 + self.origin.1 - y) / ry;
                let coverage = ((1.0 - dx * dx - dy * dy) * rx.min(ry) * 2.0 + 0.5).clamp(0.0, 1.0);
                self.blend(xx, yy, c, a * coverage);
            }
        }
    }
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, c: Rgb) {
        let (x0, y0, x1, y1) = self.bounds(
            a.0.min(b.0) - width,
            a.1.min(b.1) - width,
            (a.0 - b.0).abs() + width * 2.0,
            (a.1 - b.1).abs() + width * 2.0,
        );
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let d = (vx * vx + vy * vy).max(0.001);
        for y in y0..y1 {
            // A diagonal visits its narrow strip instead of its whole box.
            let (left, right) = if vy.abs() > 0.01 {
                let py = (y as f32 + 0.5) * 0.5 + self.origin.1;
                let t = ((py - a.1) / vy).clamp(0.0, 1.0);
                let cx = a.0 + vx * t;
                let span = (width * 0.5 + 0.5) * d.sqrt() / vy.abs() + 0.5;
                (
                    ((cx - span - self.origin.0) * 2.0).floor().max(x0 as f32) as usize,
                    ((cx + span - self.origin.0) * 2.0)
                        .ceil()
                        .clamp(x0 as f32, x1 as f32) as usize,
                )
            } else {
                (x0, x1)
            };
            for x in left..right {
                let (px, py) = (
                    (x as f32 + 0.5) * 0.5 + self.origin.0 - a.0,
                    (y as f32 + 0.5) * 0.5 + self.origin.1 - a.1,
                );
                let t = ((px * vx + py * vy) / d).clamp(0.0, 1.0);
                let dist = (px - vx * t).hypot(py - vy * t);
                self.blend(x, y, c, ((width * 0.5 - dist) * 2.0 + 0.5).clamp(0.0, 1.0));
            }
        }
    }
    pub fn polygon(&mut self, pts: &[(f32, f32)], r: Ramp) {
        let x = pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let y = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let right = pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let bottom = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        let (x0, y0, x1, y1) = self.bounds(x, y, right - x, bottom - y);
        for yy in y0..y1 {
            for xx in x0..x1 {
                let (px, py) = (
                    (xx as f32 + 0.5) * 0.5 + self.origin.0,
                    (yy as f32 + 0.5) * 0.5 + self.origin.1,
                );
                let mut inside = false;
                let mut prev = pts[pts.len() - 1];
                for &p in pts {
                    if (p.1 > py) != (prev.1 > py)
                        && px < (prev.0 - p.0) * (py - p.1) / (prev.1 - p.1) + p.0
                    {
                        inside = !inside;
                    }
                    prev = p;
                }
                if inside {
                    self.px[yy * self.w + xx] = r.at(0.7 - (py - y) / (bottom - y).max(1.0) * 0.4);
                }
            }
        }
    }
    pub fn shadow(&mut self, x: f32, y: f32, rx: f32, ry: f32, alpha: f32) {
        let (x0, y0, x1, y1) = self.bounds(x - rx, y - ry, rx * 2.0, ry * 2.0);
        for yy in y0..y1 {
            for xx in x0..x1 {
                let dx = ((xx as f32 + 0.5) * 0.5 + self.origin.0 - x) / rx;
                let dy = ((yy as f32 + 0.5) * 0.5 + self.origin.1 - y) / ry;
                let a = (1.0 - dx * dx - dy * dy).max(0.0);
                self.blend(xx, yy, art::BLACK, a * a * alpha);
            }
        }
    }
    /// A sparse two-lobe bloom kernel, only around authored emissive sources.
    pub fn glow(&mut self, p: Point) {
        let (x0, y0, x1, y1) = self.bounds(
            p.x - p.radius,
            p.y - p.radius,
            p.radius * 2.0,
            p.radius * 2.0,
        );
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = ((x as f32 + 0.5) * 0.5 + self.origin.0 - p.x) / p.radius;
                let dy = ((y as f32 + 0.5) * 0.5 + self.origin.1 - p.y) / p.radius;
                let d = dx * dx + dy * dy;
                let broad = (1.0 - d).max(0.0);
                let core = (1.0 - d * 9.0).max(0.0);
                let a = (broad * broad * 0.55 + core * core * 0.40) * p.power;
                let i = y * self.w + x;
                for k in 0..3 {
                    self.fx[i][k] =
                        (self.fx[i][k] as f32 + p.colour[k] as f32 * a).min(226.0) as u8;
                }
            }
        }
    }
    /// Combat emission only. The kernel is baked per colour/radius/power;
    /// the frame adds integer samples, leaving the danger core untouched.
    pub fn effect_glow(&mut self, p: Point) {
        type Kernel = Vec<(i32, i32, Rgb)>;
        type Key = (i32, Rgb, u32);
        static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::BTreeMap<Key, std::sync::Arc<Kernel>>>> = std::sync::OnceLock::new();
        let r = (p.radius * 2.0).round() as i32;
        let key = (r, p.colour, p.power.to_bits());
        let kernel = CACHE.get_or_init(Default::default).lock().unwrap().entry(key).or_insert_with(|| {
            let mut dots = Vec::new();
            for y in -r..r { for x in -r..r {
                let d = ((x as f32 + 0.5).powi(2) + (y as f32 + 0.5).powi(2)) / (r*r).max(1) as f32;
                let broad = (1.0-d).max(0.0);
                let core = (1.0-d*9.0).max(0.0);
                let a = (broad*broad*0.55 + core*core*0.40)*p.power;
                let c = std::array::from_fn(|k| (p.colour[k] as f32 * a) as u8);
                if c != [0;3] { dots.push((x,y,c)); }
            }}
            std::sync::Arc::new(dots)
        }).clone();
        let cx = ((p.x-self.origin.0)*2.0).round() as i32;
        let cy = ((p.y-self.origin.1)*2.0).round() as i32;
        for &(dx,dy,c) in kernel.iter() {
            let (x,y) = (cx+dx,cy+dy);
            if x>=0 && y>=0 && x<self.w as i32 && y<self.h as i32 {
                let i=y as usize*self.w+x as usize;
                for k in 0..3 { self.fx[i][k]=self.fx[i][k].saturating_add(c[k]).min(226); }
            }
        }
    }
    pub fn shaft(
        &mut self,
        top: (f32, f32),
        bottom: (f32, f32),
        width: f32,
        c: Rgb,
        strength: f32,
    ) {
        let (x0, y0, x1, y1) = self.bounds(
            top.0.min(bottom.0) - width,
            top.1,
            (top.0 - bottom.0).abs() + width * 2.0,
            bottom.1 - top.1,
        );
        for y in y0..y1 {
            for x in x0..x1 {
                let py = (y as f32 + 0.5) * 0.5 + self.origin.1;
                let t = ((py - top.1) / (bottom.1 - top.1)).clamp(0.0, 1.0);
                let center = top.0 + (bottom.0 - top.0) * t;
                let px = (x as f32 + 0.5) * 0.5 + self.origin.0;
                let edge = (1.0 - (px - center).abs() / (width * (0.35 + t * 0.65))).max(0.0);
                self.fx[y * self.w + x] = mix(
                    self.fx[y * self.w + x],
                    c,
                    edge * edge * (1.0 - t) * strength * 3.0,
                );
            }
        }
    }
    pub fn text(&mut self, x: f32, y: f32, label: &str, c: char) {
        let mut im = Img::new(super::super::super::ink::text_width(label), 7);
        im.text(0, 0, label, c);
        let colour = material(c);
        for yy in 0..im.h {
            for xx in 0..im.w {
                if im.get(xx, yy).is_some() {
                    im.set(xx, yy, colour);
                }
            }
        }
        self.stamp(&im, x, y, true);
    }
    pub fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgb) {
        self.rect(x, y, w, 0.5, c);
        self.rect(x, y + h - 0.5, w, 0.5, c);
        self.rect(x, y, 0.5, h, c);
        self.rect(x + w - 0.5, y, 0.5, h, c);
    }
    /// Coherent chips/grain: one small cluster per seed, never a screen noise pass.
    pub fn chips(&mut self, x: f32, y: f32, w: f32, h: f32, r: Ramp, seed: u32, count: i32) {
        for i in 0..count {
            let v = hash(i, seed as i32, 251);
            let px = x + (v % 1024) as f32 / 1024.0 * w;
            let py = y + ((v >> 10) % 1024) as f32 / 1024.0 * h;
            let light = if v % 3 == 0 { 0.68 } else { 0.27 };
            self.rect(px, py, 0.5 + (v >> 20) as f32 % 3.0 * 0.5, 0.5, r.at(light));
            if v % 4 == 0 {
                self.rect(px + 0.5, py + 0.5, 0.5, 0.5, r.at(light + 0.1));
            }
        }
    }
    /// Legacy cues retain their exact signal RGB. Colour outlines replace
    /// the near-black rim of the remaining material sprites.
    pub fn stamp(&mut self, im: &Img, x: f32, y: f32, signal: bool) {
        let ox = ((x - self.origin.0) * 2.0).round() as i32;
        let oy = ((y - self.origin.1) * 2.0).round() as i32;
        for yy in 0..im.h {
            for xx in 0..im.w {
                if let Some(mut c) = im.get(xx, yy) {
                    if !signal && c.iter().max().copied().unwrap_or(0) < 18 {
                        c = art::outline(c);
                    }
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let (tx, ty) = (ox + xx * 2 + dx, oy + yy * 2 + dy);
                            if tx >= 0
                                && ty >= 0
                                && (tx as usize) < self.w
                                && (ty as usize) < self.h
                            {
                                self.px[ty as usize * self.w + tx as usize] = c;
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn mist(&mut self, x: f32, y: f32, rx: f32, ry: f32, c: Rgb, a: f32) {
        // Same ellipse coverage on the separate atmosphere plane.
        std::mem::swap(&mut self.px, &mut self.fx);
        self.flat_ellipse(x, y, rx, ry, c, (a * 3.0).min(1.0));
        std::mem::swap(&mut self.px, &mut self.fx);
    }
    pub fn mark_ground(&mut self) {
        self.ground = self.px.clone();
    }
    pub fn layers(&self) -> (Img, Img) {
        (
            Img::from_rgb(self.w as i32, self.h as i32, &self.px),
            Img::from_rgb(self.w as i32, self.h as i32, &self.fx),
        )
    }
    pub fn image(&self) -> Img {
        let (base, fx) = self.layers();
        let mut out = art::native(&base, &fx);
        for (i, _) in self.kept.iter().enumerate().filter(|(_, k)| **k) {
            out.set((i % self.w) as i32, (i / self.w) as i32, self.px[i]);
        }
        out
    }

    /// `stamp` a signal (the screen's marks), its samples kept from the
    /// atmosphere (`kept`).
    pub fn stamp_kept(&mut self, im: &Img) {
        self.stamp(im, 0.0, 0.0, true);
        if self.kept.len() != self.px.len() {
            self.kept = vec![false; self.px.len()];
        }
        let o = ((-self.origin.0 * 2.0).round() as i32, (-self.origin.1 * 2.0).round() as i32);
        for yy in 0..im.h {
            for xx in 0..im.w {
                if im.get(xx, yy).is_none() {
                    continue;
                }
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (tx, ty) = (o.0 + xx * 2 + dx, o.1 + yy * 2 + dy);
                    if tx >= 0 && ty >= 0 && (tx as usize) < self.w && (ty as usize) < self.h {
                        self.kept[ty as usize * self.w + tx as usize] = true;
                    }
                }
            }
        }
    }
}

/// Baked irradiance: no ambient floor fill, no navy fog, no camera vignette.
pub(super) fn finish_static(cv: &mut Canvas, lights: &[Point], _outside: bool, _vignette: bool) {
    let (w, h) = (cv.w / 8 + 1, cv.h / 8 + 1);
    let mut map = vec![[0.0; 3]; w * h];
    for y in 0..h {
        for x in 0..w {
            map[y * w + x] = art::irradiance(
                lights,
                x as f32 * 4.0 + cv.origin.0,
                y as f32 * 4.0 + cv.origin.1,
            );
        }
    }
    for y in 0..cv.h {
        for x in 0..cv.w {
            let i = y * cv.w + x;
            let c = cv.px[i];
            let (lx, ly, tx, ty) = (x / 8, y / 8, (x % 8) as f32 / 8.0, (y % 8) as f32 / 8.0);
            let light = std::array::from_fn(|k| {
                let a = map[ly * w + lx][k] * (1.0 - tx) + map[ly * w + lx + 1][k] * tx;
                let b = map[(ly + 1) * w + lx][k] * (1.0 - tx) + map[(ly + 1) * w + lx + 1][k] * tx;
                a * (1.0 - ty) + b * ty
            });
            let ground = cv.ground.get(i) == Some(&c);
            let wx = x as i32 + (cv.origin.0 * 2.0) as i32;
            let wy = y as i32 + (cv.origin.1 * 2.0) as i32;
            cv.px[i] = art::surface(c, light, ground, wx, wy);
        }
    }
    cv.ground.clear();
    cv.ground.shrink_to_fit();
    for &p in lights {
        cv.glow(Point {
            radius: p.radius * 0.50,
            power: p.power * 0.60,
            ..p
        });
    }
}
