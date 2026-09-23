//! Inks, paper and pen for the overworld.
//!
//! The realm master palette as one-character inks, a small RGB image with the
//! drawing primitives the kit needs, a 5x7 pixel font, and the deterministic
//! noise every mark keys on. Nothing here knows about places or state.

pub(crate) type Rgb = [u8; 3];

pub(crate) const BLACK: Rgb = [0, 0, 0];

/// Colours per palette bank.
pub(crate) const BANK: usize = 15;
/// Bank 4 is reserved for meaning: beacons, cues, lamplight, alerts.
pub(crate) const SIGNAL_BANK: usize = 4;

/// The realm master palette (`cockpit/assets/realm/palette.json`): structure,
/// foliage, timber, stone and signal, fifteen colours each. Every overworld
/// pixel is one of these or black paper.
pub(crate) const PALETTE: [u32; 75] = [
    0x000000, 0x080505, 0x0b0a08, 0x0b0b0f, 0x151614, 0x1a100c, 0x25140b, 0x322b2b, 0x3d3a36,
    0x515349, 0x63655c, 0x7a7c72, 0x94968c, 0xb0b2a8, 0xcdcfc6, // structure
    0x0d1a0c, 0x182414, 0x263b1c, 0x335022, 0x35543f, 0x3f6428, 0x4b591f, 0x5a7330, 0x69793e,
    0x6a7b24, 0x6f8738, 0x768e39, 0x8aa34a, 0x6fa46d, 0xa8c05f, // foliage
    0x1a0e07, 0x331a0c, 0x362715, 0x452712, 0x45291c, 0x512d18, 0x582b13, 0x652c10, 0x684629,
    0x784620, 0x924f1d, 0xa97639, 0xbe914b, 0xd8a95e, 0xe2cb8b, // timber
    0x0a0c12, 0x132724, 0x192934, 0x22303a, 0x263b40, 0x2f4a52, 0x12516a, 0x3a6d5d, 0x456070,
    0x527597, 0x639dc2, 0x6e8a8e, 0x8fa3a8, 0xb3c2c4, 0xd5dee0, // stone
    0x12414f, 0x2079a6, 0x50c6b3, 0x8fe8dc, 0xe4fff8, 0x7a4a12, 0xecb64a, 0xf6b450, 0xf7ca58,
    0xfbd070, 0xc7b48f, 0xe3d2c3, 0xf4e1cb, 0x431e0f, 0xa8341f, // signal
];

pub(crate) const fn rgb(v: u32) -> Rgb {
    [
        ((v >> 16) & 255) as u8,
        ((v >> 8) & 255) as u8,
        (v & 255) as u8,
    ]
}

pub(crate) fn palette_index(c: Rgb) -> Option<usize> {
    PALETTE.iter().position(|&v| rgb(v) == c)
}

pub(crate) fn bank_of(c: Rgb) -> Option<usize> {
    palette_index(c).map(|i| i / BANK)
}

pub(crate) fn is_signal(c: Rgb) -> bool {
    bank_of(c) == Some(SIGNAL_BANK)
}

/// Sprite inks: one character per palette entry. Digits and `w a @ $ c` are
/// the signal bank — game state only, never decoration.
pub(crate) fn ink(ch: char) -> Option<Rgb> {
    let v: u32 = match ch {
        // structure
        'k' => 0x0b0a08,
        'K' => 0x151614,
        'Z' => 0x25140b,
        'X' => 0x322b2b,
        'g' => 0x3d3a36,
        'j' => 0x515349,
        'G' => 0x63655c,
        'J' => 0x7a7c72,
        'h' => 0x94968c,
        'i' => 0xb0b2a8,
        'H' => 0xcdcfc6,
        // foliage
        'f' => 0x0d1a0c,
        'D' => 0x182414,
        'F' => 0x263b1c,
        'E' => 0x335022,
        'e' => 0x35543f,
        'l' => 0x3f6428,
        'N' => 0x4b591f,
        'm' => 0x5a7330,
        'L' => 0x6a7b24,
        'A' => 0x6f8738,
        'M' => 0x768e39,
        'y' => 0x8aa34a,
        'C' => 0x6fa46d,
        'Y' => 0xa8c05f,
        // timber
        'n' => 0x1a0e07,
        'b' => 0x331a0c,
        'I' => 0x452712,
        'B' => 0x582b13,
        'p' => 0x652c10,
        'P' => 0x684629,
        'r' => 0x784620,
        'R' => 0x924f1d,
        'o' => 0xa97639,
        'O' => 0xbe914b,
        't' => 0xd8a95e,
        'T' => 0xe2cb8b,
        // stone
        's' => 0x192934,
        'x' => 0x22303a,
        'S' => 0x2f4a52,
        'u' => 0x456070,
        'U' => 0x6e8a8e,
        'v' => 0x8fa3a8,
        'V' => 0xb3c2c4,
        'W' => 0xd5dee0,
        'q' => 0x12516a,
        'Q' => 0x527597,
        'z' => 0x639dc2,
        // signal
        '0' => 0x12414f,
        '1' => 0x2079a6,
        '2' => 0x50c6b3,
        '3' => 0x8fe8dc,
        'w' => 0xe4fff8,
        'a' => 0x7a4a12,
        '4' => 0xecb64a,
        '@' => 0xf6b450,
        '5' => 0xf7ca58,
        '6' => 0xfbd070,
        '$' => 0xc7b48f,
        'c' => 0xe3d2c3,
        '9' => 0xf4e1cb,
        '8' => 0x431e0f,
        '7' => 0xa8341f,
        _ => return None,
    };
    Some(rgb(v))
}

fn dist2(p: Rgb, c: [f32; 3]) -> f32 {
    (0..3).map(|i| (p[i] as f32 - c[i]).powi(2)).sum()
}

/// Nearest palette entry outside the signal bank: derived tones (shadow,
/// firelight) stay on the palette and can never masquerade as state.
pub(crate) fn nearest_plain(c: [f32; 3]) -> Rgb {
    let mut best = (f32::MAX, BLACK);
    for &v in PALETTE.iter().take(BANK * SIGNAL_BANK) {
        let p = rgb(v);
        let d = dist2(p, c);
        if d < best.0 {
            best = (d, p);
        }
    }
    best.1
}

pub(crate) fn shade(c: Rgb, k: f32) -> Rgb {
    nearest_plain([c[0] as f32 * k, c[1] as f32 * k, c[2] as f32 * k])
}

// ─── deterministic noise ─────────────────────────────────────────────────────

pub(crate) fn hash(x: i32, y: i32, s: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d)
        ^ (y as u32).wrapping_mul(0x1656_67b1)
        ^ s.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

/// Smooth value noise in `0..=1`.
pub(crate) fn vnoise(x: f32, y: f32, s: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let g = |ix: i32, iy: i32| (hash(ix, iy, s) & 0xffff) as f32 / 65535.0;
    let (ix, iy) = (x0 as i32, y0 as i32);
    let top = g(ix, iy) + (g(ix + 1, iy) - g(ix, iy)) * sx;
    let bot = g(ix, iy + 1) + (g(ix + 1, iy + 1) - g(ix, iy + 1)) * sx;
    top + (bot - top) * sy
}

const BAYER: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

/// 4x4 ordered-dither threshold in `(0, 1)`, keyed on world pixels so the
/// pattern never swims when the view moves.
pub(crate) fn bayer(x: i32, y: i32) -> f32 {
    (BAYER[(y & 3) as usize][(x & 3) as usize] + 0.5) / 16.0
}

pub(crate) fn norm3(x: f32, y: f32, z: f32) -> [f32; 3] {
    let l = (x * x + y * y + z * z).sqrt().max(1e-6);
    [x / l, y / l, z / l]
}

// ─── images ──────────────────────────────────────────────────────────────────

/// A small RGB image; `None` is transparent (black paper when saved).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Img {
    pub(crate) w: i32,
    pub(crate) h: i32,
    px: Vec<Option<Rgb>>,
}

impl Img {
    pub(crate) fn new(w: i32, h: i32) -> Img {
        Img {
            w,
            h,
            px: vec![None; (w.max(0) * h.max(0)) as usize],
        }
    }

    pub(crate) fn black(w: i32, h: i32) -> Img {
        Img {
            w,
            h,
            px: vec![Some(BLACK); (w.max(0) * h.max(0)) as usize],
        }
    }

    /// Build a sprite from rows of inks; unknown characters are transparent.
    pub(crate) fn from_rows(rows: &[&str]) -> Img {
        let h = rows.len() as i32;
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut im = Img::new(w, h);
        for (y, r) in rows.iter().enumerate() {
            for (x, ch) in r.chars().enumerate() {
                im.put(x as i32, y as i32, ch);
            }
        }
        im
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    pub(crate) fn get(&self, x: i32, y: i32) -> Option<Rgb> {
        if self.inside(x, y) {
            self.px[(y * self.w + x) as usize]
        } else {
            None
        }
    }

    pub(crate) fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if self.inside(x, y) {
            let w = self.w;
            self.px[(y * w + x) as usize] = Some(c);
        }
    }

    pub(crate) fn clear(&mut self, x: i32, y: i32) {
        if self.inside(x, y) {
            let w = self.w;
            self.px[(y * w + x) as usize] = None;
        }
    }

    pub(crate) fn put(&mut self, x: i32, y: i32, ch: char) {
        if let Some(c) = ink(ch) {
            self.set(x, y, c);
        }
    }

    pub(crate) fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, ch);
            }
        }
    }

    pub(crate) fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char) {
        for xx in x..x + w {
            self.put(xx, y, ch);
            self.put(xx, y + h - 1, ch);
        }
        for yy in y..y + h {
            self.put(x, yy, ch);
            self.put(x + w - 1, yy, ch);
        }
    }

    pub(crate) fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, ch: char) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.put(x, y, ch);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub(crate) fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, ch: char) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    self.put(x, y, ch);
                }
            }
        }
    }

    pub(crate) fn stamp(&mut self, o: &Img, x: i32, y: i32) {
        for yy in 0..o.h {
            for xx in 0..o.w {
                if let Some(c) = o.get(xx, yy) {
                    self.set(x + xx, y + yy, c);
                }
            }
        }
    }

    pub(crate) fn solid(&self, x: i32, y: i32) -> bool {
        self.get(x, y).is_some()
    }

    pub(crate) fn edge(&self, x: i32, y: i32) -> bool {
        self.solid(x, y)
            && (!self.solid(x - 1, y)
                || !self.solid(x + 1, y)
                || !self.solid(x, y - 1)
                || !self.solid(x, y + 1))
    }

    /// Repaint the silhouette's own rim.
    pub(crate) fn outline_inside(&mut self, ch: char) {
        let src = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                if src.edge(x, y) {
                    self.put(x, y, ch);
                }
            }
        }
    }

    /// Grow the silhouette by a one-pixel rim.
    pub(crate) fn outline_outside(&mut self, ch: char) {
        let src = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                if !src.solid(x, y)
                    && (src.solid(x - 1, y)
                        || src.solid(x + 1, y)
                        || src.solid(x, y - 1)
                        || src.solid(x, y + 1))
                {
                    self.put(x, y, ch);
                }
            }
        }
    }

    /// Swap inks: every pixel painted `from` becomes `to`.
    pub(crate) fn recolor(&self, pairs: &[(char, char)]) -> Img {
        let mut o = self.clone();
        for &(from, to) in pairs {
            let (Some(f), Some(t)) = (ink(from), ink(to)) else {
                continue;
            };
            for p in o.px.iter_mut() {
                if *p == Some(f) {
                    *p = Some(t);
                }
            }
        }
        o
    }

    pub(crate) fn flip_h(&self) -> Img {
        let mut o = Img::new(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                if let Some(c) = self.get(x, y) {
                    o.set(self.w - 1 - x, y, c);
                }
            }
        }
        o
    }

    /// Draw `s` in the 5x7 font; returns the x after the last glyph.
    pub(crate) fn text(&mut self, x: i32, y: i32, s: &str, ch: char) -> i32 {
        let mut cx = x;
        for g in s.chars() {
            for (ry, bits) in glyph(g).iter().enumerate() {
                for bx in 0..5 {
                    if bits & (1 << (4 - bx)) != 0 {
                        self.put(cx + bx, y + ry as i32, ch);
                    }
                }
            }
            cx += GLYPH_ADVANCE;
        }
        cx
    }

    /// Row-major RGB bytes; transparent pixels are black paper.
    pub(crate) fn rgb_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 3);
        for p in &self.px {
            out.extend_from_slice(&p.unwrap_or(BLACK));
        }
        out
    }

    /// Row-major opaque RGBA bytes, each pixel blown up to a `k`x`k` block.
    pub(crate) fn rgba_scaled(&self, k: u32) -> Vec<u8> {
        let k = k.max(1) as usize;
        let (w, h) = (self.w.max(0) as usize, self.h.max(0) as usize);
        let mut out = Vec::with_capacity(w * h * k * k * 4);
        let mut row = Vec::with_capacity(w * k * 4);
        for y in 0..h {
            row.clear();
            for x in 0..w {
                let [r, g, b] = self.px[y * w + x].unwrap_or(BLACK);
                for _ in 0..k {
                    row.extend_from_slice(&[r, g, b, 255]);
                }
            }
            for _ in 0..k {
                out.extend_from_slice(&row);
            }
        }
        out
    }

    /// Row-major opaque RGBA bytes for image protocols.
    pub(crate) fn rgba_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px {
            out.extend_from_slice(&p.unwrap_or(BLACK));
            out.push(255);
        }
        out
    }

    pub(crate) fn pixels(&self) -> impl Iterator<Item = Rgb> + '_ {
        self.px.iter().map(|p| p.unwrap_or(BLACK))
    }
}

// ─── 5x7 font ────────────────────────────────────────────────────────────────

pub(crate) const GLYPH_ADVANCE: i32 = 6;

pub(crate) fn text_width(s: &str) -> i32 {
    s.chars().count() as i32 * GLYPH_ADVANCE - 1
}

fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        '-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0c, 0x04, 0x08],
        ':' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00],
        '·' => [0x00, 0x00, 0x00, 0x0c, 0x0c, 0x00, 0x00],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '%' => [0x19, 0x1a, 0x02, 0x04, 0x08, 0x0b, 0x13],
        '+' => [0x00, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x00],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '?' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        '\'' => [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '#' => [0x0a, 0x0a, 0x1f, 0x0a, 0x1f, 0x0a, 0x0a],
        '×' => [0x00, 0x00, 0x11, 0x0a, 0x04, 0x0a, 0x11],
        _ => [0; 7],
    }
}
