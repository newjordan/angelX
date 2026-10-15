//! Quiet establishing shots at 384×224: rounded stone, timber and living forms.
//! Material shading, point-light falloff and haze are painted once. Playback
//! copies the painting, stamps a small breathing light and drifts a few motes.

use super::super::ink::{BLACK, Img, Rgb, hash, ink, rgb};
use crate::rules::floor_plates::{DESTINATIONS, DURATION, Kind, Playing};
use std::sync::OnceLock;

pub const W: i32 = 384;
pub const H: i32 = 224;
const HORIZON: i32 = 188;

#[derive(Clone, Copy)]
struct Lamp {
    x: i32,
    y: i32,
    radius: f32,
    colour: char,
    strength: f32,
}

impl Lamp {
    fn warm(x: i32, y: i32, radius: f32) -> Self {
        Self {
            x,
            y,
            radius,
            colour: 't',
            strength: 0.72,
        }
    }
    fn cool(x: i32, y: i32, radius: f32) -> Self {
        Self {
            x,
            y,
            radius,
            colour: 'U',
            strength: 0.65,
        }
    }
    fn amount(self, x: i32, y: i32) -> f32 {
        let d = ((x - self.x) as f32).powi(2) + ((y - self.y) as f32).powi(2);
        self.strength * (1.0 - d / self.radius.powi(2)).max(0.0).powi(2)
    }
}

// Coloured shadows and warm material highlights, all existing realm inks.
#[derive(Clone, Copy, PartialEq)]
struct Ramp(char, char, char);
const STONE: Ramp = Ramp('s', 'u', '$');
const CLIFF: Ramp = Ramp('s', 'S', 'u');
const WOOD: Ramp = Ramp('s', 'P', 't');
const METAL: Ramp = Ramp('s', 'U', 'V');
const MOSS: Ramp = Ramp('s', 'e', 'C');
const CAP: Ramp = Ramp('s', 'S', 'U');
const PAPER: Ramp = Ramp('x', '$', 'T');

// A small palette lookup is baked once. Its adjacent-colour interpolation
// softens falloff without grey inks entering the amber or teal material ramps.
fn nearest_material(c: [f32; 3], foliage: bool, x: i32, y: i32) -> Rgb {
    type Blend = (Rgb, Rgb, f32);
    static TABLES: OnceLock<[Vec<Blend>; 2]> = OnceLock::new();
    let tables = TABLES.get_or_init(|| {
        let tones: Vec<_> = [
            0x000000, 0x080505, 0x0b0b0f, 0x1a100c, 0x0a0c12, 0x132724, 0x263b40, 0x362715,
            0x45291c, 0x512d18,
        ]
        .into_iter()
        .map(rgb)
        .chain("sxSuUqQzVnbIPRotT".chars().map(|c| ink(c).unwrap()))
        .collect();
        std::array::from_fn(|bank| {
            let mut colours = tones.clone();
            if bank == 1 {
                colours.extend("fDFeCmY".chars().map(|c| ink(c).unwrap()));
            }
            (0..32768)
                .map(|index| {
                    let c = [
                        (index >> 10) as f32 * 8.0,
                        ((index >> 5) & 31) as f32 * 8.0,
                        (index & 31) as f32 * 8.0,
                    ];
                    let dist = |p: &Rgb| (0..3).map(|i| (p[i] as f32 - c[i]).powi(2)).sum::<f32>();
                    let a = *colours
                        .iter()
                        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
                        .unwrap();
                    let mut best = (dist(&a), a, 0.0);
                    for &b in &colours {
                        let length: f32 = (0..3).map(|i| (b[i] as f32 - a[i] as f32).powi(2)).sum();
                        if length == 0.0 || length > 4200.0 {
                            continue;
                        }
                        let t = ((0..3)
                            .map(|i| (c[i] - a[i] as f32) * (b[i] as f32 - a[i] as f32))
                            .sum::<f32>()
                            / length)
                            .clamp(0.0, 1.0);
                        let error: f32 = (0..3)
                            .map(|i| (c[i] - a[i] as f32 * (1.0 - t) - b[i] as f32 * t).powi(2))
                            .sum();
                        let score = error + length * 0.018;
                        if score < best.0 {
                            best = (score, b, t);
                        }
                    }
                    (a, best.1, best.2)
                })
                .collect()
        })
    });
    let [r, g, b] = c.map(|v| ((v.clamp(0.0, 255.0) as usize + 4) / 8).min(31));
    let (a, b, t) = tables[usize::from(foliage)][(r << 10) | (g << 5) | b];
    const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    if t > 0.68
        || (t >= 0.32
            && (t - 0.32) / 0.36 * 16.0
                > BAYER[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f32 + 0.5)
    {
        b
    } else {
        a
    }
}

struct Brush {
    im: Img,
    lamps: Vec<Lamp>,
    surfaces: Vec<Option<(Ramp, f32)>>,
}

impl Brush {
    fn new(lamps: Vec<Lamp>) -> Self {
        Self {
            im: Img::black(W, H),
            lamps,
            surfaces: vec![None; (W * H) as usize],
        }
    }

    fn shade(&self, x: i32, y: i32, ramp: Ramp, plane: f32, flicker: f32) -> Rgb {
        let mut warm = 0.0_f32;
        let mut cool = 0.0_f32;
        for (i, lamp) in self.lamps.iter().enumerate() {
            let a = lamp.amount(x, y) * if i == 0 { flicker } else { 1.0 };
            if lamp.colour == 't' {
                warm += a;
            } else {
                cool += a;
            }
        }
        if ramp == CLIFF {
            // A quiet slate face keeps its own colour under the two lamps.
            // Close grey/stone tones make the falloff gentle without broad
            // amber rings or the rock helper's many overlapping facets.
            let c: [f32; 3] = std::array::from_fn(|i| {
                [14.0, 22.0, 29.0][i]
                    + [30.0, 47.0, 54.0][i] * plane
                    + [70.0, 46.0, 26.0][i] * warm
                    + [18.0, 32.0, 38.0][i] * cool
            });
            let colours = [
                0x0a0c12, 0x192934, 0x22303a, 0x263b40, 0x2f4a52, 0x456070, 0x322b2b, 0x3d3a36,
                0x515349, 0x63655c,
            ]
            .map(rgb);
            let distance = |p: &Rgb| (0..3).map(|i| (p[i] as f32 - c[i]).powi(2)).sum::<f32>();
            let a = *colours
                .iter()
                .min_by(|a, b| distance(a).total_cmp(&distance(b)))
                .unwrap();
            let mut blend = (distance(&a), a, 0.0);
            for &b in &colours {
                let length: f32 = (0..3).map(|i| (b[i] as f32 - a[i] as f32).powi(2)).sum();
                if length == 0.0 || length > 1800.0 {
                    continue;
                }
                let t = ((0..3)
                    .map(|i| (c[i] - a[i] as f32) * (b[i] as f32 - a[i] as f32))
                    .sum::<f32>()
                    / length)
                    .clamp(0.0, 1.0);
                let error: f32 = (0..3)
                    .map(|i| (c[i] - a[i] as f32 * (1.0 - t) - b[i] as f32 * t).powi(2))
                    .sum();
                if error + length * 0.01 < blend.0 {
                    blend = (error + length * 0.01, b, t);
                }
            }
            const GRAIN: [[u8; 4]; 4] =
                [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
            return if blend.2 >= 0.66
                || (blend.2 > 0.34
                    && (blend.2 - 0.34) / 0.32 * 16.0
                        > GRAIN[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f32 + 0.5)
            {
                blend.1
            } else {
                a
            };
        }
        let texture = if ramp == STONE || ramp == METAL {
            (hash(x / 2, y / 3, 97) % 11) as f32 * 0.012 - 0.060
        } else if ramp == WOOD {
            (hash(x / 2, y / 9, 68) % 7) as f32 * 0.016 - 0.048
        } else {
            0.0
        };
        let plane = (plane + texture).clamp(0.0, 1.0);
        // Light multiplies the surface plane; it never paints a flat oval over
        // beams or stone. Lit faces are amber, unlit faces stay blue-green.
        let diffuse = 0.22 + plane * 0.95;
        let ambient = 0.09 + plane * 0.40;
        let (shadow, fire, moon) = if ramp == WOOD {
            ([22.0, 25.0, 29.0], [230.0, 133.0, 43.0], [37.0, 69.0, 77.0])
        } else if ramp == MOSS {
            ([13.0, 29.0, 27.0], [136.0, 154.0, 65.0], [40.0, 91.0, 70.0])
        } else if ramp == PAPER {
            (
                [45.0, 65.0, 70.0],
                [244.0, 203.0, 134.0],
                [110.0, 161.0, 169.0],
            )
        } else if ramp == CAP {
            (
                [18.0, 41.0, 46.0],
                [179.0, 157.0, 83.0],
                [108.0, 223.0, 212.0],
            )
        } else {
            (
                [29.0, 51.0, 64.0],
                [232.0, 153.0, 62.0],
                [67.0, 117.0, 146.0],
            )
        };
        let c = std::array::from_fn(|i| {
            shadow[i] * ambient
                + fire[i] * warm.min(1.0) * diffuse
                + moon[i] * cool.min(0.85) * diffuse * 0.64
        });
        nearest_material(c, ramp == MOSS, x, y)
    }

    fn pixel(&mut self, x: i32, y: i32, ramp: Ramp, plane: f32) {
        if !(0..W).contains(&x) || !(0..HORIZON).contains(&y) {
            return;
        }
        self.surfaces[(y * W + x) as usize] = Some((ramp, plane));
        self.im.set(x, y, self.shade(x, y, ramp, plane, 1.0));
    }

    fn poly(&mut self, pts: &[(i32, i32)], ramp: Ramp, plane: f32) {
        let lo = pts.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let hi = pts.iter().map(|p| p.1).max().unwrap_or(0).min(HORIZON);
        for y in lo..hi {
            let mut xs = Vec::with_capacity(pts.len());
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
                    xs.push(a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1));
                }
            }
            xs.sort_unstable();
            for pair in xs.chunks_exact(2) {
                for x in pair[0]..=pair[1] {
                    self.pixel(x, y, ramp, plane);
                }
            }
        }
    }

    fn round(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, ramp: Ramp, plane: f32) {
        for yy in y.max(0)..(y + h).min(HORIZON) {
            for xx in x.max(0)..(x + w).min(W) {
                let dx = (x + radius - xx).max(xx - (x + w - radius - 1)).max(0);
                let dy = (y + radius - yy).max(yy - (y + h - radius - 1)).max(0);
                if dx * dx + dy * dy <= radius * radius {
                    let bevel = ((xx - x)
                        .min(x + w - xx - 1)
                        .min((yy - y).min(y + h - yy - 1)) as f32
                        / 3.0)
                        .clamp(0.0, 1.0);
                    self.pixel(xx, yy, ramp, plane * (0.70 + 0.30 * bevel));
                }
            }
        }
    }

    fn oval(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, ramp: Ramp, plane: f32) {
        for y in (cy - ry).max(0)..(cy + ry).min(HORIZON) {
            for x in (cx - rx).max(0)..(cx + rx).min(W) {
                let dx = (x as f32 + 0.5 - cx as f32) / rx as f32;
                let dy = (y as f32 + 0.5 - cy as f32) / ry as f32;
                let d = dx * dx + dy * dy;
                if d < 1.0 {
                    let volume = (1.0 - d).sqrt() * 0.65 + (1.0 - dy) * 0.18;
                    self.pixel(x, y, ramp, volume * plane);
                }
            }
        }
    }

    fn arch(&mut self, cx: i32, top: i32, half: i32, bottom: i32, thickness: i32, plane: f32) {
        // Semicircular, thick stone vault with a gently shaded curved crown.
        for y in top.max(0)..bottom.min(HORIZON) {
            for x in (cx - half).max(0)..(cx + half).min(W) {
                let dx = x - cx;
                let dy = (y - top - half).min(0);
                let outer = dx * dx + dy * dy;
                let inner = half - thickness;
                if outer < half * half && (outer >= inner * inner || dx.abs() >= inner) {
                    let rim = (half as f32 - (outer as f32).sqrt()) / thickness as f32;
                    self.pixel(x, y, STONE, plane * (0.55 + (rim * 3.0).min(1.0) * 0.45));
                }
            }
        }
        for y in (top + half + 7..bottom - 2).step_by(13) {
            for side in [-1, 1] {
                self.stroke(
                    (cx + side * (half - thickness + 2), y),
                    (cx + side * (half - 2), y),
                    STONE,
                    plane * 0.28,
                );
                self.stroke(
                    (cx + side * (half - thickness + 3), y + 1),
                    (cx + side * (half - 3), y + 1),
                    STONE,
                    plane * 0.85,
                );
            }
        }
        // Voussoirs fan around the arch. Their joints cross its thickness;
        // short inner bevels catch warm light instead of a drawn grey outline.
        for i in 1..12 {
            let angle = i as f32 * std::f32::consts::PI / 12.0;
            let (sn, cs) = angle.sin_cos();
            let a = half - thickness + 1;
            let z = half - 1;
            let inner = (
                cx + (cs * a as f32) as i32,
                top + half - (sn * a as f32) as i32,
            );
            let outer = (
                cx + (cs * z as f32) as i32,
                top + half - (sn * z as f32) as i32,
            );
            self.stroke(inner, outer, STONE, plane * 0.18);
            self.stroke(
                (inner.0 + 1, inner.1 + 1),
                (outer.0 + 1, outer.1 + 1),
                STONE,
                plane + 0.22,
            );
            if i % 2 == 0 {
                self.stroke(
                    (outer.0 - 2, outer.1 + 3),
                    (outer.0 + 1, outer.1 + 4),
                    STONE,
                    plane * 0.55,
                );
            }
        }
    }

    fn stroke(&mut self, a: (i32, i32), z: (i32, i32), ramp: Ramp, plane: f32) {
        let steps = (z.0 - a.0).abs().max((z.1 - a.1).abs()).max(1);
        for i in 0..=steps {
            self.pixel(
                a.0 + (z.0 - a.0) * i / steps,
                a.1 + (z.1 - a.1) * i / steps,
                ramp,
                plane,
            );
        }
    }

    fn masonry(&mut self, x: i32, y: i32, w: i32, h: i32, scale: i32, plane: f32) {
        for row in 0..h / scale {
            let offset = if row % 2 == 0 { 0 } else { scale };
            for col in 0..w / (scale * 2) {
                let bx = x + col * scale * 2 + offset;
                let by = y + row * scale;
                let width = (w + x - bx - 1).min(scale * 2 - 2);
                if width > 3 {
                    let n = hash(bx, by, 84);
                    self.round(
                        bx,
                        by,
                        width,
                        scale - 2,
                        2,
                        STONE,
                        plane + (n % 5) as f32 * 0.025,
                    );
                    self.stroke(
                        (bx + 2, by + 1),
                        (bx + width - 3, by + 1),
                        STONE,
                        plane + 0.30,
                    );
                    self.stroke(
                        (bx + width - 2, by + 3),
                        (bx + width - 2, by + scale - 4),
                        STONE,
                        plane * 0.35,
                    );
                    for mark in 0..3 {
                        let xx = bx + 4 + ((n >> (mark * 4)) as i32).rem_euclid((width - 7).max(1));
                        let yy =
                            by + 3 + ((n >> (mark * 3 + 2)) as i32).rem_euclid((scale - 6).max(1));
                        self.stroke((xx, yy), (xx + 2, yy), STONE, plane * 0.54);
                        self.pixel(xx + 1, yy + 1, STONE, plane + 0.09);
                    }
                    if n % 4 == 0 {
                        self.stroke((bx + 3, by + 2), (bx + 7, by + 2), STONE, plane + 0.18);
                    }
                }
            }
        }
    }

    fn rock(&mut self, x: i32, y: i32, rx: i32, ry: i32, plane: f32) {
        // Chipped silhouettes and three coherent facets, not cloud lobes.
        let a = (x - rx, y - ry / 4);
        let b = (x - rx * 2 / 3, y - ry * 4 / 5);
        let c = (x + rx / 5, y - ry);
        let d = (x + rx * 4 / 5, y - ry / 2);
        let e = (x + rx, y + ry / 3);
        let f = (x + rx / 3, y + ry * 4 / 5);
        let g = (x - rx * 3 / 4, y + ry);
        let joint = (x - rx / 7, y - ry / 5);
        self.poly(&[a, b, c, d, joint], STONE, plane + 0.28);
        self.poly(&[a, joint, f, g], STONE, plane);
        self.poly(&[joint, d, e, f], STONE, plane * 0.43);
        self.stroke(b, c, STONE, plane + 0.42);
        self.stroke(joint, f, STONE, plane * 0.60);
        // Short mineral strata lie on the face, parallel to its plane.
        for i in 0..(rx / 12).max(1) {
            let xx = x - rx / 2 + i * 11;
            let yy = y + ry / 4 + i * ry / 18;
            self.stroke((xx, yy), (xx + 7, yy - 2), STONE, plane + 0.07);
        }
    }

    fn beam(&mut self, a: (i32, i32), z: (i32, i32), width: i32, plane: f32) {
        let dx = z.0 - a.0;
        let dy = z.1 - a.1;
        let length = ((dx * dx + dy * dy) as f32).sqrt();
        let nx = (-dy as f32 / length * width as f32 / 2.0) as i32;
        let ny = (dx as f32 / length * width as f32 / 2.0) as i32;
        self.poly(
            &[
                (a.0 + nx, a.1 + ny),
                (z.0 + nx, z.1 + ny),
                (z.0 - nx, z.1 - ny),
                (a.0 - nx, a.1 - ny),
            ],
            WOOD,
            plane,
        );
        self.stroke(
            (a.0 + nx, a.1 + ny),
            (z.0 + nx, z.1 + ny),
            WOOD,
            plane + 0.25,
        );
        for offset in [-1, 2] {
            self.stroke(
                (a.0 + offset, a.1 + 3),
                (z.0 + offset, z.1 - 3),
                WOOD,
                plane - 0.12,
            );
        }
        for &(x, y) in &[a, z] {
            self.round(x - 2, y - 2, 5, 5, 1, METAL, 0.25);
            self.pixel(x - 1, y - 2, METAL, 0.9);
        }
    }

    fn chain(&mut self, x: i32, top: i32, bottom: i32, plane: f32) {
        for y in (top..bottom).step_by(5) {
            self.round(x - 1, y, 3, 4, 1, METAL, plane);
            self.pixel(x - 1, y, METAL, plane + 0.35);
            self.pixel(x, y + 1, STONE, 0.01);
        }
    }

    fn skull(&mut self, x: i32, y: i32, plane: f32) {
        self.oval(x, y, 4, 4, PAPER, plane);
        self.round(x - 2, y + 3, 5, 3, 1, PAPER, plane * 0.8);
        for dx in [-2, 1] {
            self.round(x + dx, y - 1, 2, 2, 1, STONE, 0.0);
        }
        self.pixel(x, y + 2, STONE, 0.0);
    }

    fn haze(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, colour: char, power: f32) {
        for y in (cy - ry).max(0)..(cy + ry).min(HORIZON) {
            for x in (cx - rx).max(0)..(cx + rx).min(W) {
                let d =
                    ((x - cx) as f32 / rx as f32).powi(2) + ((y - cy) as f32 / ry as f32).powi(2);
                if d < 1.0 {
                    let base = self.im.get(x, y).unwrap();
                    self.im.set(
                        x,
                        y,
                        nearest_material(
                            std::array::from_fn(|i| {
                                base[i] as f32
                                    + ink(colour).unwrap()[i] as f32
                                        * power
                                        * 0.16
                                        * (1.0 - d).powi(3)
                            }),
                            false,
                            x,
                            y,
                        ),
                    );
                }
            }
        }
    }

    fn lantern(&mut self, x: i32, y: i32, size: i32) {
        self.haze(x, y, size * 8, size * 9, 't', 0.18);
        self.round(
            x - size - 2,
            y - size * 2 - 2,
            size * 2 + 5,
            size * 4 + 5,
            3,
            METAL,
            0.45,
        );
        self.im
            .ellipse(x as f32, y as f32, size as f32, (size * 2) as f32, 'o');
        self.im.ellipse(
            x as f32,
            (y - 1) as f32,
            (size - 1).max(1) as f32,
            (size * 2 - 1) as f32,
            't',
        );
        self.im.line(x, y - size * 2 + 2, x, y + size, 'T');
        self.im.line(x, y - size * 3 - 4, x, y - size * 2 - 2, 'S');
    }

    fn candle(&mut self, x: i32, y: i32, h: i32) {
        self.round(x - 1, y - h, 3, h, 1, PAPER, 0.7);
        self.haze(x, y - h - 2, 14, 18, 't', 0.16);
        self.im.ellipse(x as f32, (y - h - 2) as f32, 1.5, 2.5, 't');
        self.im.put(x, y - h - 3, 'T');
        self.round(x - 4, y, 9, 3, 1, WOOD, 0.4);
    }

    fn floor(&mut self, vanish: i32, top: i32) {
        self.poly(
            &[
                (0, top + 15),
                (vanish, top),
                (W, top + 15),
                (W, HORIZON),
                (0, HORIZON),
            ],
            STONE,
            0.2,
        );
        for row in 0..4 {
            let y = top + 4 + row * (row + 2);
            let width = 24 + row * 13;
            for col in -5..8 {
                let x = vanish + col * width - row * 6;
                self.round(
                    x,
                    y,
                    width - 3,
                    3 + row * 2,
                    2,
                    STONE,
                    0.30 + row as f32 * 0.015,
                );
            }
        }
    }

    fn vignette(x: i32, y: i32) -> f32 {
        let dx = (x - 196) as f32 / 193.0;
        let dy = (y - 102) as f32 / 106.0;
        let radial = ((1.0 - dx * dx - dy * dy) / 0.44).clamp(0.0, 1.0);
        let foot = ((HORIZON - 1 - y) as f32 / 30.0).clamp(0.0, 1.0);
        let fade = radial.min(foot);
        fade * fade * (3.0 - 2.0 * fade)
    }

    fn finish(&self) -> Img {
        let mut im = self.im.clone();
        for y in 0..HORIZON {
            for x in 0..W {
                let fade = Self::vignette(x, y);
                if fade < 1.0 {
                    im.set(
                        x,
                        y,
                        nearest_material(
                            im.get(x, y).unwrap().map(|v| v as f32 * fade),
                            false,
                            x,
                            y,
                        ),
                    );
                }
            }
        }
        im
    }
}

fn gate() -> (Brush, Lamp) {
    let breathing = Lamp::warm(218, 108, 66.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::warm(94, 125, 62.0),
        Lamp::cool(249, 42, 110.0),
    ]);
    b.haze(235, 89, 128, 86, 's', 0.50);
    // A low, rounded gatehouse among banked stones, viewed across the steps.
    b.round(114, 54, 175, 113, 20, STONE, 0.34);
    b.masonry(119, 73, 167, 86, 11, 0.35);
    b.round(105, 58, 187, 12, 5, STONE, 0.49);
    b.round(129, 42, 151, 22, 9, STONE, 0.50);
    b.round(186, 94, 64, 72, 15, STONE, 0.015);
    b.oval(218, 103, 32, 31, STONE, 0.015);
    b.arch(218, 67, 45, 167, 12, 0.85);
    b.arch(218, 87, 27, 164, 7, 0.42);
    b.arch(218, 102, 15, 159, 4, 0.25);
    b.round(209, 126, 18, 32, 7, WOOD, 0.44);
    for x in [213, 219, 224] {
        b.im.line(x, 132, x, 156, 'P');
    }
    for (x, y, w) in [
        (121, 81, 31),
        (126, 102, 24),
        (260, 82, 21),
        (259, 107, 23),
        (135, 132, 27),
    ] {
        b.round(x, y, w, 12, 4, STONE, 0.46);
    }
    b.floor(218, 156);
    for (y, w) in [(158, 70), (166, 91), (177, 120)] {
        b.round(218 - w / 2, y, w, 7, 3, STONE, 0.52);
        b.im.line(221 - w / 2, y + 1, 214 + w / 2, y + 1, 'S');
    }
    for x in [205, 211, 217, 223, 229] {
        b.stroke((x, 119), (x, 154), METAL, 0.16);
        b.stroke((x - 1, 122), (x - 1, 133), METAL, 0.62);
        b.poly(&[(x - 2, 121), (x, 116), (x + 2, 121)], METAL, 0.42);
    }
    for y in [134, 147] {
        b.stroke((203, y), (231, y), METAL, 0.24);
    }
    b.chain(181, 96, 142, 0.26);
    // Moss and round rubble frame the entrance instead of a city silhouette.
    for (x, y, r) in [
        (32, 153, 27),
        (63, 153, 24),
        (99, 156, 18),
        (306, 152, 19),
        (335, 146, 24),
        (360, 151, 27),
    ] {
        b.oval(x, y, r, r * 2 / 3, STONE, 0.32);
        b.oval(x - 5, y - r / 3, r * 2 / 3, r / 4, MOSS, 0.40);
    }
    for (x, y) in [(43, 167), (61, 174), (108, 173), (306, 170), (331, 172)] {
        b.rock(x, y, 7, 4, 0.29);
    }
    for (x, y) in [(119, 118), (120, 132), (122, 143), (282, 121), (281, 135)] {
        b.oval(x, y, 4, 3, MOSS, 0.44);
        b.oval(x + 3, y + 4, 3, 2, MOSS, 0.35);
    }
    b.round(86, 128, 18, 39, 4, WOOD, 0.36);
    b.lantern(94, 125, 3);
    b.lantern(218, 108, 2);
    (b, breathing)
}

fn churchyard() -> (Brush, Lamp) {
    let breathing = Lamp::warm(222, 120, 76.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::warm(104, 144, 58.0),
        Lamp::cool(186, 64, 106.0),
    ]);
    // The church rises behind a gate and graves. Three depth planes share
    // the same light, but nearer iron and stones have darker facing sides.
    b.floor(220, 153);
    b.round(143, 69, 144, 91, 6, STONE, 0.28);
    b.poly(
        &[(135, 75), (153, 53), (208, 29), (273, 54), (297, 76)],
        CAP,
        0.45,
    );
    for row in 0..5 {
        for col in 0..8 {
            b.stroke(
                (159 + col * 14 + row * 3, 57 + row * 4),
                (170 + col * 14 + row * 3, 56 + row * 4),
                CAP,
                0.51 - row as f32 * 0.03,
            );
        }
    }
    b.masonry(147, 81, 137, 71, 10, 0.33);
    b.round(132, 44, 35, 116, 7, STONE, 0.30);
    b.masonry(134, 76, 32, 81, 9, 0.35);
    b.poly(&[(127, 47), (141, 25), (149, 19), (168, 47)], CAP, 0.43);
    b.arch(149, 54, 10, 76, 4, 0.45);
    b.arch(222, 96, 20, 160, 7, 0.66);
    b.round(209, 122, 26, 36, 4, WOOD, 0.42);
    for x in [212, 218, 224, 230] {
        b.stroke((x, 126), (x, 154), WOOD, 0.58);
    }
    for x in [183, 259] {
        b.arch(x, 83, 9, 111, 3, 0.51);
        b.round(x - 4, 91, 8, 16, 3, PAPER, 0.34);
        b.stroke((x, 91), (x, 108), METAL, 0.17);
        b.stroke((x - 4, 98), (x + 3, 98), METAL, 0.2);
    }
    // Low stone boundary, iron finials and a partly open wrought gate.
    b.masonry(57, 139, 81, 28, 10, 0.25);
    b.masonry(270, 137, 65, 32, 10, 0.24);
    for x in [137, 269] {
        b.round(x - 7, 115, 15, 54, 3, STONE, 0.31);
        b.round(x - 9, 113, 19, 5, 2, STONE, 0.57);
    }
    for x in (146..202).step_by(8) {
        let top = 129 - (x - 146) / 8;
        b.stroke((x, top), (x, 167), METAL, 0.16);
        b.poly(&[(x - 2, top), (x, top - 4), (x + 2, top)], METAL, 0.37);
        b.stroke((x - 1, top + 4), (x - 1, top + 12), METAL, 0.66);
    }
    for y in [139, 158] {
        b.stroke((146, y), (200, y - 6), METAL, 0.32);
    }
    for (x, y, h) in [
        (79, 157, 25),
        (110, 169, 21),
        (306, 159, 28),
        (283, 174, 16),
    ] {
        b.round(x - 7, y - h, 15, h, 5, STONE, 0.16);
        b.stroke((x - 5, y - h + 2), (x + 3, y - h + 2), STONE, 0.68);
        b.stroke((x, y - h + 6), (x, y - h + 14), STONE, 0.05);
        b.stroke((x - 3, y - h + 9), (x + 3, y - h + 9), STONE, 0.05);
        b.rock(x, y + 2, 11, 4, 0.12);
    }
    b.lantern(222, 120, 2);
    b.lantern(104, 144, 2);
    for (x, y) in [(54, 170), (91, 179), (317, 172)] {
        b.rock(x, y, 14, 8, 0.04);
    }
    for (x, y, r) in [(44, 117, 15), (337, 111, 16)] {
        b.beam((x, y), (x + 2, 172), 4, 0.08);
        b.oval(x, y - 10, r, 31, MOSS, 0.06);
        b.oval(x - 2, y - 24, r - 3, 24, MOSS, 0.05);
        for dy in [0, 11, 22] {
            b.stroke((x - 5, y - dy), (x + 5, y - dy - 2), MOSS, 0.18);
        }
    }
    b.chain(268, 119, 149, 0.36);
    (b, breathing)
}

fn crypt_vault() -> (Brush, Lamp) {
    let breathing = Lamp::warm(172, 130, 64.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::warm(74, 94, 57.0),
        Lamp::warm(307, 100, 60.0),
        Lamp::cool(218, 72, 85.0),
    ]);
    b.haze(216, 103, 92, 73, 'S', 0.19);
    b.masonry(52, 57, 52, 105, 12, 0.25);
    b.masonry(307, 57, 50, 105, 12, 0.20);
    // Three round vaults, each darker and smaller toward the sanctuary.
    for (top, half, thick, shade) in [
        (33, 139, 17, 0.45),
        (51, 95, 13, 0.40),
        (70, 58, 9, 0.33),
        (87, 31, 6, 0.27),
    ] {
        b.arch(214, top, half, 163, thick, shade);
    }
    b.floor(214, 151);
    for (x, y) in [(42, 59), (88, 77), (286, 74), (331, 56)] {
        b.round(x, y, 24, 77, 6, STONE, 0.35);
        b.round(x + 6, y + 16, 13, 21, 5, STONE, 0.01);
        b.round(x + 6, y + 49, 13, 21, 5, STONE, 0.01);
        b.arch(x + 12, y + 8, 9, y + 34, 3, 0.52);
        b.arch(x + 12, y + 43, 9, y + 69, 3, 0.48);
        // Small ivory remains sit within the ossuary recesses.
        for yy in [y + 28, y + 63] {
            b.skull(x + 11, yy - 2, 0.68);
            b.stroke((x + 5, yy + 5), (x + 18, yy + 5), PAPER, 0.4);
        }
    }
    // A low sarcophagus, with a rounded lid and a selectively lit lip.
    b.poly(
        &[(118, 151), (168, 138), (241, 153), (198, 176), (122, 166)],
        STONE,
        0.36,
    );
    b.round(123, 154, 76, 21, 6, STONE, 0.33);
    for x in [133, 151, 173] {
        b.round(x, 161, 12, 7, 3, STONE, 0.23);
    }
    b.poly(
        &[(198, 160), (241, 151), (239, 171), (198, 180)],
        STONE,
        0.23,
    );
    b.poly(
        &[(115, 149), (167, 133), (244, 149), (199, 167), (119, 157)],
        STONE,
        0.72,
    );
    b.im.line(122, 149, 166, 136, 'U');
    b.im.line(169, 136, 232, 149, 'j');
    b.stroke((151, 149), (190, 153), STONE, 0.26);
    b.stroke((173, 144), (164, 159), STONE, 0.27);
    b.stroke((153, 148), (189, 152), STONE, 0.90);
    for (x, y) in [(141, 164), (166, 168), (219, 166)] {
        b.round(x - 5, y - 3, 10, 6, 2, METAL, 0.28);
        b.stroke((x - 3, y - 2), (x + 2, y - 2), METAL, 0.68);
    }
    for (x, y, h) in [(151, 148, 12), (172, 149, 17), (192, 155, 9)] {
        b.candle(x, y, h);
    }
    b.candle(74, 108, 12);
    b.candle(307, 114, 14);
    for (x, y) in [(45, 163), (78, 169), (289, 165), (330, 170)] {
        b.rock(x, y, 8, 5, 0.24);
    }
    b.round(209, 111, 12, 30, 5, STONE, 0.45);
    b.im.ellipse(215.0, 114.0, 2.0, 3.0, 'U');
    for (x, y, w, h) in [(27, 49, 19, 126), (346, 49, 20, 128)] {
        b.round(x, y, w, h, 5, STONE, 0.015);
        b.stroke((x + w - 3, y + 9), (x + w - 3, y + h - 8), STONE, 0.28);
        b.round(x - 3, y + 25, w + 6, 7, 2, STONE, 0.08);
    }
    b.chain(120, 47, 101, 0.20);
    b.chain(286, 49, 104, 0.23);
    (b, breathing)
}

fn wheel(b: &mut Brush, x: i32, y: i32, r: i32) {
    b.oval(x, y, r + 3, r + 3, WOOD, 0.50);
    b.oval(x, y, r, r, WOOD, 0.12);
    for (dx, dy) in [
        (r - 2, 0),
        (0, r - 2),
        (r * 2 / 3, r * 2 / 3),
        (r * 2 / 3, -r * 2 / 3),
    ] {
        b.stroke((x - dx, y - dy), (x + dx, y + dy), WOOD, 0.52);
    }
    b.oval(x, y, 3, 3, METAL, 0.60);
    b.stroke((x - r / 2, y - r + 1), (x + r / 3, y - r + 1), METAL, 0.85);
}

fn mine_head() -> (Brush, Lamp) {
    let breathing = Lamp::warm(249, 111, 72.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::warm(92, 91, 75.0),
        Lamp::cool(188, 47, 160.0),
        Lamp::warm(316, 143, 49.0),
    ]);
    b.haze(207, 111, 150, 74, 's', 0.20);
    // One quiet cliff: a broad face, a lit crown and a recessed right side.
    // Small strata describe the rock without splitting it into rubble facets.
    b.poly(
        &[
            (31, 65),
            (91, 40),
            (156, 31),
            (212, 45),
            (272, 29),
            (323, 46),
            (354, 70),
            (361, 157),
            (28, 165),
        ],
        CLIFF,
        0.22,
    );
    b.poly(
        &[
            (31, 65),
            (91, 40),
            (156, 31),
            (212, 45),
            (272, 29),
            (323, 46),
            (319, 60),
            (270, 44),
            (210, 60),
            (154, 45),
            (94, 54),
        ],
        CLIFF,
        0.50,
    );
    b.poly(
        &[(319, 60), (323, 46), (354, 70), (361, 157), (326, 159)],
        CLIFF,
        0.10,
    );
    // Long, gently sloping bedding seams, interrupted around the entrance.
    for (a, z) in [
        ((130, 78), (194, 74)),
        ((141, 107), (204, 101)),
        ((151, 135), (208, 131)),
        ((294, 78), (325, 83)),
        ((309, 124), (332, 129)),
    ] {
        b.stroke(a, z, CLIFF, 0.08);
        b.stroke((a.0, a.1 + 1), (z.0, z.1 + 1), CLIFF, 0.30);
    }
    for (x, y) in [
        (155, 60),
        (178, 88),
        (159, 119),
        (201, 65),
        (305, 94),
        (319, 145),
    ] {
        b.stroke((x, y), (x + 5, y - 1), CLIFF, 0.15);
        b.pixel(x + 2, y - 2, CLIFF, 0.35);
    }
    // Timbered adit: the tunnel stays black behind thick oak posts and lintel.
    b.round(222, 104, 67, 64, 5, STONE, 0.01);
    b.oval(256, 108, 32, 29, STONE, 0.01);
    b.arch(256, 72, 45, 167, 9, 0.42);
    b.arch(256, 91, 30, 167, 6, 0.15);
    b.floor(256, 157);
    b.beam((220, 105), (220, 163), 10, 0.67);
    b.beam((287, 104), (287, 164), 10, 0.54);
    b.beam((214, 104), (293, 104), 11, 0.71);
    b.beam((223, 111), (239, 121), 5, 0.45);
    b.beam((284, 111), (271, 121), 5, 0.39);
    // A-frame winding head: two legs meet under the sheave, with a clear
    // crossbeam, iron joints and a rope dropping into an open shaft collar.
    b.round(66, 158, 58, 10, 2, STONE, 0.09);
    b.im.rect(73, 158, 42, 7, 's');
    b.beam((90, 57), (48, 165), 10, 0.64);
    b.beam((99, 57), (140, 164), 10, 0.46);
    b.beam((67, 113), (122, 113), 9, 0.63);
    b.beam((55, 151), (90, 120), 5, 0.32);
    b.beam((129, 151), (102, 121), 5, 0.30);
    b.beam((81, 66), (110, 66), 9, 0.61);
    wheel(&mut b, 95, 49, 16);
    // A sheave has an open centre, rim and spokes; its hanging rope is hemp.
    b.stroke((111, 48), (111, 160), WOOD, 0.79);
    b.stroke((112, 49), (112, 160), WOOD, 0.34);
    b.round(109, 151, 5, 6, 1, METAL, 0.32);
    b.stroke((90, 70), (90, 82), METAL, 0.40);
    b.lantern(92, 91, 2);
    // Rails curve toward the adit and catch its lantern at the vanishing end.
    for y in 163..HORIZON {
        let d = y - 163;
        let left = 243 - d * 4 - d * d / 35;
        let right = 265 - d * 3;
        if d % 5 == 1 {
            b.im.line(left - 6, y, right + 6, y, 'P');
        }
        b.im.line(left, y, left + 1, y, 'S');
        b.im.line(right, y, right + 1, y, 'u');
    }
    b.round(308, 141, 43, 23, 6, METAL, 0.40);
    b.oval(329, 142, 22, 5, STONE, 0.25);
    b.round(308, 145, 43, 3, 1, METAL, 0.72);
    for x in [316, 328, 340] {
        b.stroke((x, 149), (x, 160), METAL, 0.24);
    }
    b.rock(325, 140, 7, 5, 0.35);
    b.rock(336, 139, 8, 6, 0.28);
    wheel(&mut b, 317, 165, 5);
    wheel(&mut b, 342, 163, 5);
    b.poly(
        &[(308, 142), (348, 139), (353, 145), (310, 148)],
        METAL,
        0.66,
    );
    b.stroke((310, 148), (350, 145), METAL, 0.87);
    b.stroke((311, 151), (348, 149), METAL, 0.22);
    for x in [313, 324, 337, 348] {
        b.round(x, 151, 2, 9, 1, METAL, 0.48);
        b.pixel(x, 152, METAL, 0.86);
        b.pixel(x, 159, METAL, 0.70);
    }
    b.lantern(316, 143, 1);
    b.lantern(249, 111, 3);
    (b, breathing)
}

fn forge() -> (Brush, Lamp) {
    let breathing = Lamp::warm(248, 114, 106.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::warm(96, 91, 61.0),
        Lamp::cool(179, 53, 82.0),
    ]);
    b.masonry(68, 46, 257, 113, 12, 0.28);
    // Vaults and furnace stack lead to the cooler room behind the fire.
    b.arch(183, 31, 108, 164, 14, 0.30);
    b.arch(175, 55, 58, 158, 9, 0.20);
    b.floor(215, 150);
    b.round(220, 57, 69, 107, 8, STONE, 0.43);
    b.masonry(224, 61, 62, 96, 10, 0.50);
    b.arch(248, 90, 26, 156, 9, 0.72);
    b.round(231, 112, 35, 36, 5, WOOD, 0.1);
    // Emission is crisp, with its diffuse light already on nearby bricks.
    for (x, y, h) in [
        (236, 137, 14),
        (242, 140, 25),
        (249, 139, 32),
        (256, 139, 20),
        (261, 138, 11),
    ] {
        b.poly(
            &[
                (x - 3, y),
                (x - 2, y - h / 2),
                (x, y - h),
                (x + 3, y - h / 3),
                (x + 3, y),
            ],
            WOOD,
            0.95,
        );
        b.im.line(x, y - h + 4, x - 1, y - 3, 't');
        b.im.put(x, y - 5, 'T');
    }
    for x in (230..268).step_by(7) {
        b.stroke((x, 125), (x, 148), METAL, 0.18);
        b.pixel(x - 1, 126, METAL, 0.78);
    }
    b.round(226, 149, 47, 6, 2, METAL, 0.39);
    b.chain(190, 43, 116, 0.24);
    b.chain(307, 47, 135, 0.25);
    // Roof braces, with visible beam ends, scarf joints and iron straps.
    b.beam((69, 57), (306, 60), 10, 0.32);
    b.beam((76, 57), (102, 140), 8, 0.27);
    b.beam((304, 60), (279, 92), 7, 0.30);
    b.lantern(96, 91, 3);
    // A heavy anvil on its stump: bright top, charcoal front and horn.
    b.oval(173, 172, 34, 8, WOOD, 0.16);
    b.round(150, 143, 43, 28, 4, WOOD, 0.29);
    for x in [155, 164, 175, 187] {
        b.stroke((x, 150), (x - 2, 167), WOOD, 0.17);
    }
    b.poly(
        &[(136, 128), (174, 125), (204, 130), (183, 136), (152, 136)],
        METAL,
        0.82,
    );
    b.poly(
        &[(150, 136), (183, 136), (179, 144), (154, 145)],
        METAL,
        0.15,
    );
    b.poly(
        &[(183, 131), (211, 127), (205, 133), (184, 139)],
        METAL,
        0.39,
    );
    b.round(156, 141, 26, 5, 2, METAL, 0.31);
    b.stroke((140, 128), (176, 126), METAL, 0.97);
    b.im.line(150, 128, 171, 126, 'u');
    b.im.line(181, 133, 203, 129, 'o');
    b.beam((185, 127), (202, 149), 3, 0.48);
    b.round(181, 123, 14, 7, 2, METAL, 0.7);
    // Tongs, ore basket, rail offcuts and a dark nearer barrel.
    b.stroke((116, 141), (132, 163), METAL, 0.28);
    b.stroke((121, 140), (134, 160), METAL, 0.36);
    b.round(279, 151, 35, 19, 3, METAL, 0.18);
    for x in [284, 294, 305] {
        b.stroke((x, 152), (x + 3, 168), METAL, 0.43);
    }
    for (x, y) in [(286, 148), (298, 146), (305, 150)] {
        b.rock(x, y, 7, 5, 0.38);
    }
    b.oval(78, 169, 19, 6, WOOD, 0.12);
    b.round(59, 144, 38, 24, 5, WOOD, 0.12);
    b.oval(78, 144, 19, 5, WOOD, 0.30);
    for y in [149, 163] {
        b.stroke((61, y), (94, y), METAL, 0.18);
    }
    for x in [68, 78, 88] {
        b.stroke((x, 149), (x, 161), WOOD, 0.04);
    }
    for y in [173, 177] {
        b.stroke((213, y), (275, y - 2), METAL, 0.27);
    }
    (b, breathing)
}

fn keep_blocks(b: &mut Brush, x: i32, top: i32, w: i32, bottom: i32, curved: bool) {
    b.round(x, top, w, bottom - top, 2, STONE, 0.08);
    for (row, y) in (top..bottom).step_by(9).enumerate() {
        let mut xx = x - if row % 2 == 0 { 0 } else { 8 };
        while xx < x + w {
            let left = xx.max(x);
            let right = (xx + 16).min(x + w);
            let height = 8.min(bottom - y);
            if right - left > 2 {
                let t = (left + right - 2 * x) as f32 / (2 * w) as f32;
                let volume = if curved {
                    (t * std::f32::consts::PI).sin()
                } else {
                    0.72
                };
                let plane = 0.14 + volume * 0.29 + (hash(xx, y, 36) % 4) as f32 * 0.025;
                b.round(left, y, right - left - 1, height, 1, STONE, plane);
                b.stroke((left + 1, y + 1), (right - 3, y + 1), STONE, plane + 0.28);
                b.stroke(
                    (right - 2, y + 3),
                    (right - 2, y + height - 2),
                    STONE,
                    plane * 0.25,
                );
                if hash(xx, y, 36) % 3 == 0 {
                    b.stroke((left + 3, y + 5), (left + 6, y + 5), STONE, plane * 0.5);
                }
            }
            xx += 17;
        }
    }
}

fn merlon(b: &mut Brush, x: i32, y: i32, w: i32) {
    // A solid battlement with a top coping and a visible dark side face.
    b.round(x, y, w, 12, 1, STONE, 0.48);
    b.poly(
        &[
            (x + w - 3, y + 2),
            (x + w, y),
            (x + w, y + 12),
            (x + w - 3, y + 12),
        ],
        STONE,
        0.13,
    );
    b.stroke((x + 1, y), (x + w - 2, y), STONE, 0.84);
    b.stroke((x + 1, y + 6), (x + w - 4, y + 6), STONE, 0.15);
}

fn arrow_slit(b: &mut Brush, x: i32, y: i32) {
    b.round(x - 4, y - 3, 9, 23, 2, STONE, 0.10);
    b.stroke((x - 4, y - 1), (x - 4, y + 18), STONE, 0.63);
    b.im.line(x, y, x, y + 15, 'o');
    b.im.line(x, y + 2, x, y + 11, 't');
    b.im.line(x - 2, y + 10, x + 2, y + 10, 'o');
    b.stroke((x - 5, y + 20), (x + 4, y + 20), STONE, 0.71);
}

fn tower(b: &mut Brush, x: i32, top: i32, w: i32, bottom: i32) {
    keep_blocks(b, x, top + 7, w, bottom, true);
    b.round(x - 2, top + 5, w + 4, 5, 1, STONE, 0.47);
    b.stroke((x - 1, top + 5), (x + w + 1, top + 5), STONE, 0.74);
    for dx in [0, 17, 34] {
        merlon(b, x + dx, top - 6, 12);
    }
    arrow_slit(b, x + w / 2, top + 32);
    b.round(x - 2, bottom - 9, w + 4, 5, 1, STONE, 0.28);
}

fn keep() -> (Brush, Lamp) {
    let breathing = Lamp::warm(208, 134, 67.0);
    let mut moon = Lamp::cool(213, 56, 184.0);
    moon.strength = 0.84;
    let mut b = Brush::new(vec![
        breathing,
        Lamp::warm(123, 89, 27.0),
        Lamp::warm(291, 79, 27.0),
        moon,
    ]);
    b.haze(208, 85, 155, 100, 's', 0.19);
    b.oval(202, 169, 151, 35, STONE, 0.17);
    // A taller hall behind the curtain wall, with coursed walls and slate
    // shingles laid along the roof planes rather than a single teal wedge.
    keep_blocks(&mut b, 163, 64, 88, 145, false);
    b.poly(&[(155, 67), (205, 31), (206, 63)], CAP, 0.39);
    b.poly(&[(205, 31), (259, 66), (206, 63)], CAP, 0.19);
    for row in 0..5 {
        let y = 39 + row * 5;
        let half = (y - 31) * 3 / 2;
        b.stroke((205 - half, y), (203, y - 2), CAP, 0.58);
        b.stroke((209, y - 2), (205 + half, y), CAP, 0.27);
        for dx in (-half + 4..-2).step_by(9) {
            b.stroke((205 + dx, y), (208 + dx, y - 4), CAP, 0.22);
        }
        for dx in (5..half - 2).step_by(9) {
            b.stroke((205 + dx, y), (202 + dx, y - 4), CAP, 0.10);
        }
    }
    b.stroke((205, 33), (205, 62), CAP, 0.64);
    b.round(157, 65, 100, 5, 1, STONE, 0.44);
    keep_blocks(&mut b, 134, 93, 144, 166, false);
    b.round(133, 90, 145, 5, 1, STONE, 0.48);
    for x in (137..276).step_by(19) {
        merlon(&mut b, x, 80, 12);
    }
    tower(&mut b, 99, 56, 48, 167);
    tower(&mut b, 266, 46, 50, 168);
    for x in [165, 246] {
        arrow_slit(&mut b, x, 105);
    }
    // Recessed scorched arch and an iron portcullis, warm behind its bars.
    b.round(192, 132, 33, 33, 2, WOOD, 0.07);
    b.oval(208, 132, 17, 17, WOOD, 0.07);
    b.arch(208, 112, 27, 167, 8, 0.58);
    b.arch(208, 120, 19, 166, 3, 0.10);
    b.round(201, 139, 12, 26, 1, WOOD, 0.60);
    for x in (195..224).step_by(6) {
        b.stroke((x, 132), (x, 163), METAL, 0.19);
        b.stroke((x + 1, 135), (x + 1, 160), METAL, 0.58);
        b.poly(&[(x - 1, 162), (x + 2, 162), (x, 167)], METAL, 0.26);
    }
    for y in [140, 151] {
        b.round(191, y, 35, 3, 1, METAL, 0.29);
        for x in [195, 207, 219] {
            b.pixel(x, y, METAL, 0.86);
        }
    }
    b.chain(179, 121, 158, 0.29);
    b.chain(237, 121, 158, 0.24);
    b.lantern(208, 134, 2);
    b.floor(208, 165);
    b.round(179, 167, 60, 5, 2, STONE, 0.51);
    b.round(167, 175, 84, 5, 2, STONE, 0.40);
    // A horned dragon skull mounted over the gate: long muzzle, hollow eyes,
    // swept horns and small teeth, lit by the doorway below it.
    b.poly(
        &[
            (197, 104),
            (199, 95),
            (207, 92),
            (218, 96),
            (220, 104),
            (214, 109),
            (204, 110),
        ],
        PAPER,
        0.57,
    );
    b.poly(
        &[(199, 98), (190, 90), (191, 84), (195, 91), (203, 95)],
        PAPER,
        0.41,
    );
    b.poly(
        &[(214, 96), (223, 89), (226, 84), (225, 94), (219, 101)],
        PAPER,
        0.33,
    );
    b.round(201, 99, 4, 3, 1, STONE, 0.01);
    b.round(213, 99, 4, 3, 1, STONE, 0.01);
    b.poly(
        &[(207, 104), (213, 104), (216, 110), (206, 111)],
        PAPER,
        0.47,
    );
    b.pixel(211, 108, STONE, 0.01);
    for x in [203, 207, 216] {
        b.stroke((x, 108), (x, 111), PAPER, 0.68);
    }
    // Old claw scores break only a few blocks beside the scorched threshold.
    for dx in [0, 4, 8] {
        b.stroke((242 + dx, 133), (239 + dx, 148), STONE, 0.01);
        b.stroke((243 + dx, 134), (240 + dx, 147), STONE, 0.48);
    }
    for (x, y, rx) in [(27, 153, 35), (65, 150, 30), (333, 154, 24), (366, 147, 35)] {
        b.oval(x, y, rx, 20, MOSS, 0.35);
        b.oval(x - 10, y - 14, rx * 2 / 3, 17, MOSS, 0.42);
    }
    (b, breathing)
}

fn shelves(b: &mut Brush, x: i32, y: i32, w: i32, h: i32, near: bool) {
    b.round(x, y, w, h, 7, WOOD, if near { 0.44 } else { 0.23 });
    b.round(x + 5, y + 7, w - 10, h - 14, 3, STONE, 0.07);
    let step = if near { 23 } else { 17 };
    for yy in (y + step..y + h - 3).step_by(step as usize) {
        let mut xx = x + 8;
        while xx < x + w - 9 {
            let n = hash(xx, yy, 73);
            let bw = 3 + (n % 4) as i32;
            let bh = step - 6 - ((n >> 4) % 6) as i32;
            let ramp = match n % 4 {
                0 => WOOD,
                1 => MOSS,
                2 => METAL,
                _ => PAPER,
            };
            b.round(xx, yy - bh, bw, bh, 1, ramp, if near { 0.53 } else { 0.21 });
            if near && n % 3 == 0 {
                for dy in [3, bh - 3] {
                    b.stroke(
                        (xx + 1, yy - bh + dy),
                        (xx + bw - 2, yy - bh + dy),
                        PAPER,
                        0.61,
                    );
                }
                b.stroke((xx + 1, yy - bh + 5), (xx + 1, yy - 4), ramp, 0.73);
            }
            xx += bw + 2;
        }
        b.round(x + 3, yy, w - 6, 4, 2, WOOD, 0.63);
    }
    b.round(x + 2, y + 3, 4, h - 6, 2, WOOD, 0.49);
}

fn archive() -> (Brush, Lamp) {
    let breathing = Lamp::warm(200, 96, 82.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::cool(80, 110, 95.0),
        Lamp::cool(314, 80, 99.0),
        Lamp::warm(165, 138, 58.0),
        Lamp::warm(286, 98, 54.0),
    ]);
    b.haze(203, 104, 110, 70, 'S', 0.25);
    b.arch(201, 40, 100, 151, 10, 0.37);
    shelves(&mut b, 150, 80, 34, 64, false);
    shelves(&mut b, 222, 77, 32, 66, false);
    shelves(&mut b, 78, 57, 56, 96, true);
    shelves(&mut b, 274, 50, 55, 104, true);
    shelves(&mut b, 17, 28, 53, 126, true);
    shelves(&mut b, 339, 24, 34, 131, true);
    // Still flood water holds quiet, grouped reflections beneath the shelves.
    b.poly(
        &[(0, 155), (198, 145), (384, 155), (384, 188), (0, 188)],
        CAP,
        0.15,
    );
    b.haze(200, 168, 61, 21, 'P', 0.25);
    for (x, y, w, c) in [
        (187, 154, 26, 'o'),
        (182, 159, 38, 'P'),
        (194, 164, 20, 'o'),
        (176, 172, 45, 'P'),
        (70, 169, 27, 'S'),
        (303, 160, 22, 'S'),
        (264, 178, 30, 's'),
    ] {
        b.im.line(x, y, x + w, y, c);
    }
    b.im.line(200, 49, 200, 84, 'S');
    b.lantern(200, 96, 4);
    // A dry stone reading island with a warmly lit open folio.
    b.oval(134, 175, 48, 10, STONE, 0.31);
    b.round(129, 151, 9, 24, 3, WOOD, 0.50);
    b.poly(
        &[(105, 144), (132, 133), (159, 143), (137, 159)],
        WOOD,
        0.66,
    );
    b.poly(
        &[(108, 142), (130, 135), (134, 150), (119, 150)],
        PAPER,
        0.68,
    );
    b.poly(
        &[(132, 135), (156, 143), (138, 154), (134, 150)],
        PAPER,
        0.80,
    );
    b.im.line(132, 137, 135, 150, 'P');
    for y in [141, 144] {
        b.im.line(115, y, 127, y - 3, 'j');
        b.im.line(139, y - 1, 150, y + 3, 'j');
    }
    b.candle(165, 157, 17);
    b.lantern(286, 98, 2);
    b.chain(200, 49, 84, 0.35);
    (b, breathing)
}

// Living light is baked into the cap itself, independently of incident light.
// This small teal-green ramp stays on the realm palette and is only used here.
fn fungal_glow(b: &mut Brush, x: i32, y: i32, strength: f32) {
    let Some(base) = b.im.get(x, y) else { return };
    let colours = [
        0x132724, 0x263b40, 0x2f4a52, 0x3a6d5d, 0x6e8a8e, 0x50c6b3, 0x8fe8dc,
    ]
    .map(rgb);
    let c: [f32; 3] =
        std::array::from_fn(|i| (base[i] as f32).max([72.0, 226.0, 180.0][i] * strength));
    let distance = |p: &Rgb| (0..3).map(|i| (p[i] as f32 - c[i]).powi(2)).sum::<f32>();
    let colour = *colours
        .iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .unwrap();
    b.im.set(x, y, colour);
}

fn mushroom(b: &mut Brush, x: i32, y: i32, r: i32, foot: i32, near: bool) {
    b.oval(x, foot, r / 2 + 3, 5, MOSS, 0.16);
    let height = (foot - y).max(1);
    for yy in y..foot {
        let t = (yy - y) as f32 / height as f32;
        let bend = (t * std::f32::consts::PI).sin() * r as f32 * 0.13;
        let centre = x + bend as i32;
        let half = (r as f32 * (0.11 + 0.055 * t * t)) as i32;
        for xx in centre - half..=centre + half {
            let side = (xx - centre) as f32 / half.max(1) as f32;
            let volume = (1.0 - side * side).max(0.0) * 0.48 + 0.13;
            b.pixel(xx, yy, CAP, volume * if near { 0.92 } else { 0.40 });
        }
    }
    b.oval(x, y + 4, r, r / 4, CAP, if near { 0.73 } else { 0.27 });
    // A high dome with a flattened, scalloped lip; gills lie below the crown.
    for yy in (y - r * 3 / 5).max(0)..=y + 2 {
        for xx in (x - r).max(0)..(x + r).min(W) {
            let dx = (xx - x) as f32 / r as f32;
            let dy = (yy - y - 2) as f32 / (r as f32 * 0.64);
            let d = dx * dx + dy * dy;
            if d < 1.0 {
                let volume = (1.0 - d).sqrt() * 0.55 + (1.0 - dy) * 0.19;
                b.pixel(xx, yy, CAP, volume * if near { 0.86 } else { 0.32 });
            }
        }
    }
    if near {
        for (dx, dy, rx) in [
            (-r / 3, -r / 4, r / 7),
            (r / 4, -r / 5, r / 8),
            (r / 2, 0, r / 9),
        ] {
            b.oval(x + dx, y + dy, rx.max(2), 2, CAP, 0.94);
            b.oval(x + dx + 1, y + dy - 1, (rx - 2).max(1), 1, CAP, 0.76);
        }
    }
    // The underside emits a soft fan of gills. Rims are curved and broader
    // at the front, while the crown remains a quiet, shaded material dome.
    let glow = if near { 1.20 } else { 0.45 };
    for dx in -r + 2..r - 1 {
        let arc = (1.0 - (dx as f32 / r as f32).powi(2)).sqrt();
        let lip = y + 4 + (arc * r as f32 * 0.22) as i32;
        for yy in y + 3..=lip {
            let edge = (yy - y - 3) as f32 / (lip - y - 2).max(1) as f32;
            fungal_glow(b, x + dx, yy, glow * (0.30 + edge * 0.24));
        }
        fungal_glow(b, x + dx, lip, glow * (0.58 + arc * 0.32));
    }
    for dx in (-r + 5..r - 4).step_by(if near { 7 } else { 6 }) {
        let arc = (1.0 - (dx as f32 / r as f32).powi(2)).sqrt();
        let lip = y + 4 + (arc * r as f32 * 0.22) as i32;
        let end = (x + dx / 3, lip);
        let start = (x + dx, y + 3);
        let steps = (end.0 - start.0).abs().max((end.1 - start.1).abs()).max(1);
        for i in 0..=steps {
            let xx = start.0 + (end.0 - start.0) * i / steps;
            let yy = start.1 + (end.1 - start.1) * i / steps;
            fungal_glow(b, xx, yy, glow * 0.77);
        }
    }
}

fn fungal() -> (Brush, Lamp) {
    let breathing = Lamp::cool(236, 109, 90.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::cool(84, 97, 83.0),
        Lamp::cool(336, 78, 88.0),
        Lamp::warm(179, 148, 53.0),
    ]);
    b.haze(195, 127, 139, 53, 'S', 0.27);
    for (x, y, r) in [
        (62, 112, 21),
        (130, 96, 24),
        (187, 100, 19),
        (286, 91, 30),
        (330, 117, 24),
    ] {
        mushroom(&mut b, x, y, r, 160, false);
    }
    b.poly(
        &[
            (0, 173),
            (146, 153),
            (261, 160),
            (384, 166),
            (384, 188),
            (0, 188),
        ],
        MOSS,
        0.24,
    );
    b.haze(197, 158, 118, 24, 'S', 0.21);
    mushroom(&mut b, 84, 81, 46, 177, true);
    mushroom(&mut b, 236, 102, 39, 166, true);
    mushroom(&mut b, 340, 57, 55, 184, true);
    mushroom(&mut b, 27, 150, 23, 185, true);
    for (x, y) in [(80, 172), (237, 164), (337, 179)] {
        for dx in [-17, -8, 8, 18] {
            b.stroke((x, y - 3), (x + dx, y + 2), CAP, 0.35);
        }
    }
    // Small bioluminescent fruits, gathered at the path and giant cap roots.
    for (x, y, r) in [
        (152, 160, 8),
        (175, 154, 6),
        (199, 159, 9),
        (278, 166, 8),
        (302, 159, 5),
    ] {
        b.oval(x, y + 8, 2, 8, CAP, 0.43);
        b.oval(x, y, r, r / 2, CAP, 0.80);
        b.im.ellipse(x as f32, (y + 1) as f32, (r - 2) as f32, 2.0, 'U');
    }
    // A small amber lantern contrasts with the cool living light.
    b.lantern(179, 148, 2);
    (b, breathing)
}

fn unknown() -> (Brush, Lamp) {
    let breathing = Lamp::cool(208, 101, 79.0);
    let mut b = Brush::new(vec![
        breathing,
        Lamp::cool(206, 151, 69.0),
        Lamp::warm(78, 153, 42.0),
    ]);
    b.haze(215, 113, 130, 73, 'S', 0.22);
    // An ancient, half-buried ring and a suspended cool stone. No sky or eyes.
    for (x, y, w) in [(116, 95, 13), (293, 89, 15), (67, 116, 12), (324, 111, 13)] {
        b.round(x, y, w, 58, 5, STONE, 0.21);
        b.oval(x + w / 2, y, w / 2 + 3, 5, STONE, 0.26);
        for yy in (y + 13..y + 52).step_by(11) {
            b.stroke((x + 2, yy), (x + w - 3, yy), STONE, 0.05);
            b.stroke((x + 3, yy + 1), (x + w - 4, yy + 1), STONE, 0.32);
        }
    }
    b.floor(208, 157);
    b.arch(208, 48, 66, 163, 15, 0.57);
    for (a, z) in [
        ((159, 115), (165, 113)),
        ((155, 129), (164, 127)),
        ((255, 98), (263, 98)),
        ((253, 121), (261, 123)),
    ] {
        b.stroke(a, z, STONE, 0.72);
    }
    b.round(141, 151, 35, 17, 6, STONE, 0.45);
    b.round(240, 151, 35, 17, 6, STONE, 0.39);
    b.oval(208, 160, 35, 9, STONE, 0.45);
    b.oval(208, 162, 24, 4, CAP, 0.66);
    b.poly(
        &[
            (208, 83),
            (219, 96),
            (214, 115),
            (203, 120),
            (196, 104),
            (199, 93),
        ],
        METAL,
        0.70,
    );
    b.im.line(207, 88, 204, 106, 'U');
    b.im.line(204, 106, 206, 115, 'v');
    // A faint inlaid path offers scale and a way into the darkness.
    for (x, y, w) in [(201, 170, 14), (190, 178, 29)] {
        b.round(x, y, w, 3, 1, CAP, 0.56);
    }
    for (x, y, r) in [(42, 165, 28), (87, 174, 21), (307, 173, 30), (352, 155, 25)] {
        b.oval(x, y, r, r / 2, STONE, 0.25);
    }
    for (x, y) in [(153, 143), (257, 134), (163, 78), (247, 69), (199, 54)] {
        b.stroke((x, y), (x + 4, y + 5), STONE, 0.15);
        b.stroke((x + 4, y + 5), (x + 2, y + 10), STONE, 0.15);
        b.stroke((x + 1, y), (x + 5, y + 4), STONE, 0.62);
    }
    b.poly(
        &[(199, 94), (208, 84), (208, 108), (203, 120), (196, 104)],
        METAL,
        0.77,
    );
    b.poly(&[(208, 84), (219, 96), (214, 115), (208, 108)], METAL, 0.37);
    b.stroke((199, 94), (208, 84), METAL, 0.9);
    b.lantern(78, 153, 2);
    (b, breathing)
}

// Rounded inset cartouche; title and depth strings are the original game text.
fn cartouche(im: &mut Img, x: i32, y: i32, w: i32, h: i32, r: i32) {
    for yy in 0..h {
        for xx in 0..w {
            let dx = (r - xx).max(xx - (w - r - 1)).max(0);
            let dy = (r - yy).max(yy - (h - r - 1)).max(0);
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let edge = xx.min(w - xx - 1).min(yy.min(h - yy - 1));
            let curved = dx * dx + dy * dy > (r - 1) * (r - 1);
            if edge == 0 || curved {
                im.put(x + xx, y + yy, if yy < h / 2 { 'S' } else { 's' });
            } else {
                // Integer rows also serve the small live terminal title.
                let light = (h - yy) * 3 / h + (w / 2 - (xx - w / 2).abs()) * 2 / w;
                let c = match light {
                    0 => rgb(0x0a0c12),
                    1 => rgb(0x132724),
                    2 => ink('s').unwrap(),
                    _ => ink('x').unwrap(),
                };
                im.set(x + xx, y + yy, c);
            }
        }
    }
    im.line(x + r + 4, y, x + w / 2 - 9, y, 'u');
    im.line(x + w / 2 + 9, y, x + w - r - 5, y, 'S');
}

fn title(im: &mut Img, kind: Kind, depth: u32) {
    cartouche(im, 49, 189, 286, 34, 8);
    let label = kind.title();
    let mut letters = Img::new(super::super::ink::text_width(label), 7);
    letters.text(0, 0, label, '$');
    let left = (W - letters.w * 2) / 2;
    for y in 0..14 {
        for x in 0..letters.w * 2 {
            if let Some(c) = letters.get(x / 2, y / 2) {
                im.set(left + x, 196 + y, c);
            }
        }
    }
    let line = format!("FLOOR {depth}");
    im.text(
        (W - super::super::ink::text_width(&line)) / 2,
        214,
        &line,
        'U',
    );
}

struct Scene {
    base: Img,
    breaths: [Vec<(i32, i32, Rgb)>; 4],
    motes: [(i32, i32); 10],
    cool: bool,
    spores: bool,
}

fn build(kind: Kind, depth: u32) -> Scene {
    let (brush, lamp) = match kind {
        Kind::Gate => gate(),
        Kind::Crypt => {
            if depth == 1 {
                churchyard()
            } else {
                crypt_vault()
            }
        }
        Kind::Mines => {
            if depth == 1 {
                mine_head()
            } else {
                forge()
            }
        }
        Kind::Keep => keep(),
        Kind::Archive => archive(),
        Kind::Fungal => fungal(),
        Kind::Unknown => unknown(),
    };
    let mut base = brush.finish();
    title(&mut base, kind, depth);
    // Bake four surface-light states inside the small pool, then retain only
    // pixels that differ from the base. Playback never walks the whole patch.
    // Flame cores and inked detail are kept above the diffuse light.
    let breaths = std::array::from_fn(|phase| {
        let mut patch = Img::new(73, 73);
        let flicker = [0.86, 0.98, 1.10, 1.20][phase];
        for py in 0..73 {
            for px in 0..73 {
                let x = lamp.x + px - 36;
                let y = lamp.y + py - 36;
                if !(0..W).contains(&x) || !(0..HORIZON).contains(&y) {
                    continue;
                }
                if let Some((ramp, plane)) = brush.surfaces[(y * W + x) as usize] {
                    if brush.im.get(x, y) == Some(brush.shade(x, y, ramp, plane, 1.0)) {
                        let radius =
                            ((px - 36) * (px - 36) + (py - 36) * (py - 36)) as f32 / 1296.0;
                        let local = 1.0 + (flicker - 1.0) * (1.0 - radius).max(0.0).powi(2);
                        let c = brush.shade(x, y, ramp, plane, local);
                        let fade = Brush::vignette(x, y);
                        patch.set(
                            px,
                            py,
                            nearest_material(c.map(|v| v as f32 * fade), ramp == MOSS, x, y),
                        );
                    }
                }
            }
        }
        patch.put(
            36,
            35 + phase as i32 % 2,
            if lamp.colour == 't' { 'T' } else { 'U' },
        );
        let mut changes = Vec::new();
        for py in 0..73 {
            for px in 0..73 {
                let (x, y) = (lamp.x + px - 36, lamp.y + py - 36);
                if let Some(c) = patch.get(px, py) {
                    if base.get(x, y) != Some(c) {
                        changes.push((x, y, c));
                    }
                }
            }
        }
        changes
    });
    let motes = std::array::from_fn(|i| {
        let (x, y) = if kind == Kind::Fungal {
            (
                75 + (hash(i as i32, 0, 42) % 250) as i32,
                76 + (hash(i as i32, 1, 42) % 80) as i32,
            )
        } else {
            (
                lamp.x - 49 + (hash(i as i32, 0, 42) % 98) as i32,
                lamp.y - 34 + (hash(i as i32, 1, 42) % 60) as i32,
            )
        };
        (x.clamp(18, W - 18), y.clamp(22, HORIZON - 15))
    });
    Scene {
        base,
        breaths,
        motes,
        cool: matches!(kind, Kind::Fungal | Kind::Unknown),
        spores: kind == Kind::Fungal,
    }
}

fn cached(kind: Kind, depth: u32) -> &'static Scene {
    static SCENES: [OnceLock<Scene>; DESTINATIONS.len()] =
        [const { OnceLock::new() }; DESTINATIONS.len()];
    let index = DESTINATIONS
        .iter()
        .position(|&(k, d)| k == kind && d == depth)
        .or_else(|| DESTINATIONS.iter().position(|&(k, _)| k == kind))
        .unwrap_or(0);
    let (kind, depth) = DESTINATIONS[index];
    SCENES[index].get_or_init(|| build(kind, depth))
}

/// Fully opened artwork; only a cached light pool and ten dust/spore dots move.
pub fn painting(kind: Kind, depth: u32, age: u8) -> Img {
    let scene = cached(kind, depth);
    let mut im = scene.base.clone();
    let phase = [0, 2, 1, 3, 2, 1, 2, 0][usize::from(age.min(DURATION - 1) / 7)];
    for &(x, y, c) in &scene.breaths[phase] {
        im.set(x, y, c);
    }
    for (i, &(x, y)) in scene.motes.iter().enumerate() {
        let drift = (i32::from(age) + i as i32 * 4) / 13;
        let x = x + if i % 3 == 0 { -drift / 2 } else { drift / 2 };
        let y = y - drift;
        let colour = if scene.spores {
            if i % 3 == 0 { '2' } else { 'U' }
        } else if scene.cool {
            if i % 3 == 0 { 'U' } else { 'S' }
        } else if i % 3 == 0 {
            'o'
        } else {
            'u'
        };
        im.put(x, y, colour);
        if i % 3 == 0 {
            im.put(x + 1, y, if scene.cool { 'S' } else { 'P' });
        }
    }
    im
}

pub fn frame(plate: Playing, w: i32, h: i32) -> Img {
    let mut native = painting(plate.kind, plate.depth, plate.age);
    // Quick horizontal iris: black -> painting in 7 ticks, close in 6.
    // No interpolated RGB: all frames remain exactly on the master palette.
    let aperture = if plate.age < 7 {
        i32::from(plate.age) * H / 7
    } else if plate.age > DURATION - 7 {
        i32::from(DURATION.saturating_sub(plate.age)) * H / 6
    } else {
        H
    };
    let edge = (H - aperture) / 2;
    for y in 0..H {
        if y < edge || y >= H - edge {
            for x in 0..W {
                native.set(x, y, BLACK);
            }
        }
    }
    if w == W && h == H {
        return native;
    }
    let mut out = Img::black(w.max(0), h.max(0));
    if w <= 0 || h <= 0 {
        return out;
    }
    // Fit with the arena's aspect ratio. Pure nearest-pixel enlargement and
    // reduction keep the large silhouettes and title shapes on their inks.
    let (tw, th) = if w * H <= h * W {
        (w, w * H / W)
    } else {
        (h * W / H, h)
    };
    if tw == 0 || th == 0 {
        return out;
    }
    let (ox, oy) = ((w - tw) / 2, (h - th) / 2);
    for yy in 0..th {
        for xx in 0..tw {
            if let Some(c) = native.get(xx * W / tw, yy * H / th) {
                out.set(ox + xx, oy + yy, c);
            }
        }
    }
    // Half-block terminals need an actual small font, rather than a 5×7
    // glyph reduced below one pixel. Keep this inside the fitted picture.
    if tw < 230 && tw >= 90 && th >= 40 && (7..=DURATION - 7).contains(&plate.age) {
        for yy in oy + th - 20..oy + th {
            for xx in ox..ox + tw {
                out.set(xx, yy, BLACK);
            }
        }
        cartouche(&mut out, ox + 2, oy + th - 20, tw - 4, 20, 4);
        let label = plate.kind.title();
        out.text(
            ox + (tw - super::super::ink::text_width(label)) / 2,
            oy + th - 18,
            label,
            '$',
        );
        let label = format!("FLOOR {}", plate.depth);
        out.text(
            ox + (tw - super::super::ink::text_width(&label)) / 2,
            oy + th - 8,
            &label,
            'U',
        );
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__plates_tests.rs"]
mod tests;
