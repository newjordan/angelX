//! The shared grade and braille atmosphere of the arcade stage and the
//! terminal cell painter: black paper, one restrained grade, coloured dots
//! over crisp pixels.
use super::overworld::ink::{Img, Rgb};
use super::braille::{ColoredBrailleImage, braille_bits, braille_dot_bit};
pub const BLACK: Rgb = [0, 0, 0];
pub const IVORY: Rgb = [226, 222, 204];
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
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

