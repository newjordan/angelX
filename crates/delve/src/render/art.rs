//! The opt-in angelX art contract. Black paper, material ramps, bounded light,
//! coloured occlusion and braille atmosphere are shared by every v2 surface.
use super::overworld::ink::{Img, Rgb, ink};
use super::braille::{ColoredBrailleImage, braille_bits, braille_dot_bit};
pub const BLACK: Rgb = [0, 0, 0];
pub const IVORY: Rgb = [226, 222, 204];
pub const WELL: Rgb = [12, 22, 31];
pub const TRIM: Rgb = [160, 121, 67];
pub const KEY_GUTTER: i32 = 8;
pub const TEXT: Rgb = [167, 180, 179];
/// Whether the host chose the second art kit (`settings::Settings::art_v2`).
pub fn enabled() -> bool {
    crate::settings::get().art_v2
}
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}
pub fn material(c: char) -> Rgb {
    // Material anchors shared by the Delve, kingdom walk and terminal kit.
    match c {
        's' => [24, 32, 47],
        'x' => [38, 48, 60],
        'S' => [65, 79, 91],
        'u' => [114, 137, 153],
        'i' => [179, 201, 207],
        'U' => [142, 171, 190],
        'q' => [18, 29, 39],
        'b' => [50, 37, 56],
        'B' => [88, 56, 61],
        'P' => [135, 85, 56],
        'o' => [187, 133, 71],
        'T' => [237, 196, 119],
        'f' => [35, 55, 67],
        'F' => [56, 89, 75],
        'e' => [91, 120, 62],
        'A' => [140, 162, 72],
        'M' => [196, 216, 117],
        '7' => [180, 48, 39],
        'R' => [216, 99, 61],
        '9' | 'w' => IVORY,
        '@' => [209, 127, 58],
        '6' => [225, 181, 100],
        _ => ink(c).expect("authored material ink"),
    }
}

#[derive(Clone, Copy)]
pub struct Ramp {
    pub dark: Rgb,
    pub mid: Rgb,
    pub light: Rgb,
}
impl Ramp {
    pub fn inks(a: char, b: char, c: char) -> Self {
        Self {
            dark: mix(material(a), material('q'), 0.18),
            mid: material(b),
            light: mix(material(c), material('T'), 0.13),
        }
    }
    pub fn at(self, t: f32) -> Rgb {
        let t = (t.clamp(0.0, 1.0) * 12.0).round() / 12.0;
        if t < 0.5 {
            mix(self.dark, self.mid, t * 2.0)
        } else {
            mix(self.mid, self.light, t * 2.0 - 1.0)
        }
    }
}

#[derive(Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub colour: Rgb,
    pub power: f32,
}
impl Point {
    pub fn new(x: f32, y: f32, radius: f32, colour: char, power: f32) -> Self {
        Self {
            x,
            y,
            radius,
            colour: material(colour),
            power,
        }
    }
}

/// Selective coloured occlusion, including retained bitmap contour samples.
pub fn outline(c: Rgb) -> Rgb {
    if c.iter().copied().max().unwrap_or(0) < 18 {
        mix(material('s'), material('q'), 0.25)
    } else {
        c
    }
}
/// Shared sparse floor grade. Feather dim material into true black paper.
pub fn surface(c: Rgb, light: [f32; 3], ground: bool, x: i32, y: i32) -> Rgb {
    if !ground {
        return grade(c, light, true);
    }
    let peak = light.iter().copied().fold(0.0, f32::max);
    let lit = grade(c, light, false);
    let threshold = 14.0 + super::overworld::ink::bayer(x, y) * 9.0;
    if peak < 0.065 || (lit.iter().copied().max().unwrap_or(0) as f32) < threshold {
        BLACK
    } else {
        lit
    }
}
/// Compact quadratic falloff, exactly zero beyond the authored radius.
pub fn irradiance(lights: &[Point], x: f32, y: f32) -> [f32; 3] {
    let mut out = [0.0; 3];
    for p in lights {
        let d = ((x - p.x).powi(2) + (y - p.y).powi(2)) / (p.radius * p.radius);
        let a = (1.0 - d).max(0.0).powi(2) * p.power;
        for k in 0..3 {
            out[k] += p.colour[k] as f32 / 255.0 * a;
        }
    }
    out
}
/// One restrained grade; cool occlusion, warm material highlights, tonal ivory.
pub fn grade(c: Rgb, light: [f32; 3], body: bool) -> Rgb {
    let base = if body { [0.40, 0.43, 0.48] } else { [0.0; 3] };
    std::array::from_fn(|k| (c[k] as f32 * (base[k] + light[k] * 0.70)).min(IVORY[k] as f32) as u8)
}
/// Bodies retain a modest visibility floor; use the identical light and grade.
pub fn grade_actor(c: Rgb, light: [f32; 3]) -> Rgb {
    grade(c, light.map(|v| v * 0.55 + 0.44), true)
}
/// Existing ride/ambient converter, with box coverage so small sparks survive.
pub fn braille(fx: &Img, w: usize, h: usize) -> ColoredBrailleImage {
    let mut rgba = image::RgbaImage::new((w * 2) as u32, (h * 4) as u32);
    for y in 0..h * 4 {
        for x in 0..w * 2 {
            let (x0, y0) = (x * fx.w as usize / (w * 2), y * fx.h as usize / (h * 4));
            let (x1, y1) = (
                ((x + 1) * fx.w as usize / (w * 2)).max(x0 + 1),
                ((y + 1) * fx.h as usize / (h * 4)).max(y0 + 1),
            );
            let mut sum = [0u32; 3];
            let mut peak = [0u8; 3];
            let mut n = 0;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let c = fx.get(sx as i32, sy as i32).unwrap_or(BLACK);
                    for k in 0..3 {
                        sum[k] += c[k] as u32;
                        peak[k] = peak[k].max(c[k]);
                    }
                    n += 1;
                }
            }
            let c: Rgb = std::array::from_fn(|k| ((sum[k] / n) as u8).max(peak[k] / 3));
            rgba.put_pixel(x as u32, y as u32, image::Rgba([c[0], c[1], c[2], 255]));
        }
    }
    super::braille::frame_to_braille_graded(&rgba, w, h, false)
}
/// Native pictures carry the same coloured dots as cells, over crisp pixels.
pub fn native(base: &Img, fx: &Img) -> Img {
    let (w, h) = ((base.w as usize / 8).max(1), (base.h as usize / 16).max(1));
    let dots = braille(fx, w, h);
    let mut out = base.clone();
    for cy in 0..h {
        for cx in 0..w {
            let cell = dots.cells[cy * w + cx];
            let bits = braille_bits(cell.glyph);
            if bits == 0 {
                continue;
            }
            for dy in 0..4 {
                for dx in 0..2 {
                    if bits & braille_dot_bit(dx, dy) == 0 {
                        continue;
                    }
                    let (x, y) = (
                        (cx * 2 + dx) * base.w as usize / (w * 2),
                        (cy * 4 + dy) * base.h as usize / (h * 4),
                    );
                    for yy in y + 1..(y + 3).min(base.h as usize) {
                        for xx in x + 1..(x + 3).min(base.w as usize) {
                            let c = base.get(xx as i32, yy as i32).unwrap_or(BLACK);
                            // Bright material/attack samples own their silhouette.
                            if c.iter().copied().max().unwrap_or(0) < 100 {
                                out.set(xx as i32, yy as i32, mix(c, cell.fg, 0.52));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

