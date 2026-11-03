//! A few drawing calls on a frame of pixels: filled and shaded boxes and the
//! game's own 5x7 font. The garage's labels and bars are drawn with these on
//! top of the lit 3D scene.

use crate::render::overworld::ink::{GLYPH_ADVANCE, glyph};

pub type Rgb = [u8; 3];

/// A frame being drawn on.
pub struct Canvas<'a> {
    pub w: usize,
    pub h: usize,
    pub px: &'a mut [Rgb],
}

impl<'a> Canvas<'a> {
    pub fn new(w: usize, h: usize, px: &'a mut [Rgb]) -> Canvas<'a> {
        debug_assert_eq!(px.len(), w * h);
        Canvas { w, h, px }
    }

    pub fn dot(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let p = &mut self.px[y as usize * self.w + x as usize];
            for k in 0..3 {
                p[k] = (f32::from(p[k]) * (1.0 - a) + f32::from(c[k]) * a) as u8;
            }
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.dot(xx, yy, c);
            }
        }
    }

    /// A box laid over the picture: `a` of `c`, the rest what was there.
    pub fn shade(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb, a: f32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c, a);
            }
        }
    }

    /// A one-pixel outline.
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        self.rect(x, y, w, 1, c);
        self.rect(x, y + h - 1, w, 1, c);
        self.rect(x, y, 1, h, c);
        self.rect(x + w - 1, y, 1, h, c);
    }

    /// Text in the 5x7 font, `scale` times as big; returns the x after it.
    pub fn text(&mut self, x: i32, y: i32, s: &str, c: Rgb, scale: i32) -> i32 {
        let mut cx = x;
        for g in s.chars() {
            for (ry, bits) in glyph(g).iter().enumerate() {
                for bx in 0..5 {
                    if bits & (1 << (4 - bx)) != 0 {
                        self.rect(cx + bx * scale, y + ry as i32 * scale, scale, scale, c);
                    }
                }
            }
            cx += GLYPH_ADVANCE * scale;
        }
        cx
    }

    /// The same with a one-pixel shadow, so it reads on any ground.
    pub fn label(&mut self, x: i32, y: i32, s: &str, c: Rgb, scale: i32) -> i32 {
        self.text(x + scale, y + scale, s, [4, 4, 8], scale);
        self.text(x, y, s, c, scale)
    }

    pub fn width(s: &str, scale: i32) -> i32 {
        (s.chars().count() as i32 * GLYPH_ADVANCE - 1) * scale
    }

    pub fn centred(&mut self, cx: i32, y: i32, s: &str, c: Rgb, scale: i32) {
        self.label(cx - Self::width(s, scale) / 2, y, s, c, scale);
    }
}

/// `c` scaled by `k`.
pub fn dim(c: Rgb, k: f32) -> Rgb {
    c.map(|v| (f32::from(v) * k).clamp(0.0, 255.0) as u8)
}

/// `a` mixed toward `b` by `t`.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|k| (f32::from(a[k]) * (1.0 - t) + f32::from(b[k]) * t) as u8)
}
