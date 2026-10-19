//! Inks, paper and pen for the overworld.
//!
//! The realm master palette as one-character inks, a small RGB image with the
//! drawing primitives the kit needs, a 5x7 pixel font, and the deterministic
//! noise every mark keys on. Nothing here knows about places or state.

pub type Rgb = [u8; 3];

/// A table keyed by colour, hashed the cheap way. The palette's tables
/// (each ink's darker steps, its firelit and lighter neighbours) are read
/// for nearly every pixel a room is lit by, and a colour is three bytes
/// that need no defence against crafted keys: the default hasher's cost
/// was a share of every torch step's drawing.
pub type RgbMap<V> = std::collections::HashMap<Rgb, V, std::hash::BuildHasherDefault<RgbHasher>>;

/// `RgbMap`'s hasher: each byte folded in with a multiply.
#[derive(Default)]
pub struct RgbHasher(u64);

impl std::hash::Hasher for RgbHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(b)).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
}

pub const BLACK: Rgb = [0, 0, 0];

/// Colours per palette bank.
pub const BANK: usize = 15;
/// Bank 4 is reserved for meaning: beacons, cues, lamplight, alerts.
pub const SIGNAL_BANK: usize = 4;

/// The realm master palette (`cockpit/assets/realm/palette.json`): structure,
/// foliage, timber, stone and signal, fifteen colours each. Every overworld
/// pixel is one of these or black paper.
pub const PALETTE: [u32; 75] = [
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

pub const fn rgb(v: u32) -> Rgb {
    [
        ((v >> 16) & 255) as u8,
        ((v >> 8) & 255) as u8,
        (v & 255) as u8,
    ]
}

pub fn palette_index(c: Rgb) -> Option<usize> {
    PALETTE.iter().position(|&v| rgb(v) == c)
}

pub fn bank_of(c: Rgb) -> Option<usize> {
    palette_index(c)
        .filter(|&i| i < (SIGNAL_BANK + 1) * BANK)
        .map(|i| i / BANK)
}

pub fn is_signal(c: Rgb) -> bool {
    bank_of(c) == Some(SIGNAL_BANK)
}

/// Sprite inks: one character per palette entry. Digits and `w a @ $ c` are
/// the signal bank — game state only, never decoration.
pub fn ink(ch: char) -> Option<Rgb> {
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
pub fn nearest_plain(c: [f32; 3]) -> Rgb {
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

pub fn shade(c: Rgb, k: f32) -> Rgb {
    nearest_plain([c[0] as f32 * k, c[1] as f32 * k, c[2] as f32 * k])
}

// ─── deterministic noise ─────────────────────────────────────────────────────

pub fn hash(x: i32, y: i32, s: u32) -> u32 {
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
pub fn vnoise(x: f32, y: f32, s: u32) -> f32 {
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
pub fn bayer(x: i32, y: i32) -> f32 {
    (BAYER[(y & 3) as usize][(x & 3) as usize] + 0.5) / 16.0
}

pub fn norm3(x: f32, y: f32, z: f32) -> [f32; 3] {
    let l = (x * x + y * y + z * z).sqrt().max(1e-6);
    [x / l, y / l, z / l]
}

// ─── images ──────────────────────────────────────────────────────────────────

/// A small image of packed RGBA words (`0xAARRGGBB`): alpha 0 is
/// transparent (black paper when saved), 255 is opaque. Rows are contiguous,
/// so a copy, a compose or a conversion is a pass over plain words.
#[derive(Clone, Debug)]
pub struct Img {
    pub w: i32,
    pub h: i32,
    px: Vec<u32>,
    /// The pixels `px` keeps, `(x, y, w, h)`: the whole picture, or for a
    /// window onto a great one (`Img::window`) only that rectangle. The
    /// rest reads as transparent and takes no paint.
    span: (i32, i32, i32, i32),
    /// Where paint has landed since `untouch`, `(x0, y0, x1, y1)` in the
    /// kept pixels (`span`), the ends past the last: empty when `x0 >= x1`.
    /// `take_changes` looks only there. Not part of the picture: two
    /// pictures with the same pixels are equal however they were painted.
    touched: (i32, i32, i32, i32),
}

/// Nothing touched.
const UNTOUCHED: (i32, i32, i32, i32) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);

impl PartialEq for Img {
    fn eq(&self, other: &Self) -> bool {
        (self.w, self.h, self.span) == (other.w, other.h, other.span) && same_words(&self.px, &other.px)
    }
}

/// Whether two runs of words are the same, sixteen words a step. A slice's
/// own `==` is a `memcmp`, which in the browser's wasm goes a byte at a
/// time: a stage frame's pictures, compared every frame to say whether they
/// go again, spent a fifth of the page's time in it.
pub fn same_words(a: &[u32], b: &[u32]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let (mut ca, mut cb) = (a.chunks_exact(16), b.chunks_exact(16));
    for (x, y) in (&mut ca).zip(&mut cb) {
        let mut differ = 0u32;
        for k in 0..16 {
            differ |= x[k] ^ y[k];
        }
        if differ != 0 {
            return false;
        }
    }
    ca.remainder().iter().zip(cb.remainder()).all(|(x, y)| x == y)
}

/// `same_words` for bytes, eight a word.
pub fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let (mut ca, mut cb) = (a.chunks_exact(64), b.chunks_exact(64));
    for (x, y) in (&mut ca).zip(&mut cb) {
        let mut differ = 0u64;
        for k in 0..8 {
            let word = |c: &[u8]| u64::from_le_bytes([c[8 * k], c[8 * k + 1], c[8 * k + 2], c[8 * k + 3], c[8 * k + 4], c[8 * k + 5], c[8 * k + 6], c[8 * k + 7]]);
            differ |= word(x) ^ word(y);
        }
        if differ != 0 {
            return false;
        }
    }
    ca.remainder().iter().zip(cb.remainder()).all(|(x, y)| x == y)
}

impl Eq for Img {}

/// Transparent: no alpha.
const CLEAR: u32 = 0;
const OPAQUE: u32 = 0xff00_0000;

#[inline]
fn pack([r, g, b]: Rgb) -> u32 {
    OPAQUE | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

#[inline]
fn unpack(p: u32) -> Option<Rgb> {
    (p & OPAQUE != 0).then_some([(p >> 16) as u8, (p >> 8) as u8, p as u8])
}

#[inline]
fn paper(p: u32) -> Rgb {
    unpack(p).unwrap_or(BLACK)
}

impl Img {
    /// RGB compositors may leave the material palette after lighting.
    pub fn from_rgb(w: i32, h: i32, pixels: &[Rgb]) -> Img {
        assert_eq!(pixels.len(), (w.max(0) * h.max(0)) as usize);
        Img { w, h, px: pixels.iter().copied().map(pack).collect(), span: (0, 0, w.max(0), h.max(0)), touched: UNTOUCHED }
    }
    pub fn new(w: i32, h: i32) -> Img {
        Img {
            w,
            h,
            px: vec![CLEAR; (w.max(0) * h.max(0)) as usize],
            span: (0, 0, w.max(0), h.max(0)),
            touched: UNTOUCHED,
        }
    }

    pub fn black(w: i32, h: i32) -> Img {
        Img {
            w,
            h,
            px: vec![pack(BLACK); (w.max(0) * h.max(0)) as usize],
            span: (0, 0, w.max(0), h.max(0)),
            touched: UNTOUCHED,
        }
    }

    /// A transparent `w` by `h` picture keeping only the rectangle `(x, y,
    /// sw, sh)` of it (clipped to the picture): a window onto a great room,
    /// drawn on at the room's own pixels, costing only what lands in the
    /// window.
    pub fn window(w: i32, h: i32, (x, y, sw, sh): (i32, i32, i32, i32)) -> Img {
        let (x0, y0) = (x.clamp(0, w.max(0)), y.clamp(0, h.max(0)));
        let (x1, y1) = ((x + sw).clamp(x0, w.max(0)), (y + sh).clamp(y0, h.max(0)));
        Img { w, h, px: vec![CLEAR; ((x1 - x0) * (y1 - y0)) as usize], span: (x0, y0, x1 - x0, y1 - y0), touched: UNTOUCHED }
    }

    /// The rectangle this picture keeps (`window`): all of it, unless it is
    /// a window.
    pub fn span(&self) -> (i32, i32, i32, i32) {
        self.span
    }

    /// Whether this picture keeps all its pixels.
    pub fn whole(&self) -> bool {
        self.span == (0, 0, self.w.max(0), self.h.max(0))
    }

    /// The kept pixels as a picture of their own (all of it, for a whole
    /// picture).
    pub fn kept(&self) -> std::borrow::Cow<'_, Img> {
        if self.whole() {
            return std::borrow::Cow::Borrowed(self);
        }
        let (_, _, w, h) = self.span;
        std::borrow::Cow::Owned(Img { w, h, px: self.px.clone(), span: (0, 0, w, h), touched: UNTOUCHED })
    }

    /// Where pixel `(x, y)` is kept, if it is.
    #[inline]
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let (sx, sy, sw, sh) = self.span;
        (x >= sx && y >= sy && x < sx + sw && y < sy + sh).then(|| ((y - sy) * sw + (x - sx)) as usize)
    }

    /// Build a sprite from rows of inks; unknown characters are transparent.
    /// A sprite is read from its rows once per render thread and kept in
    /// that thread's atlas; asking again copies it back out of the sheet.
    pub fn from_rows(rows: &[&str]) -> Img {
        let key = rows_key(rows);
        if let Some(img) = ATLAS.with(|atlas| atlas.borrow().get(key)) {
            return img;
        }
        let img = Img::parse_rows(rows);
        ATLAS.with(|atlas| atlas.borrow_mut().insert(key, &img));
        img
    }

    fn parse_rows(rows: &[&str]) -> Img {
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

    pub fn get(&self, x: i32, y: i32) -> Option<Rgb> {
        self.index(x, y).and_then(|i| unpack(self.px[i]))
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.index(x, y) {
            self.px[i] = pack(c);
            self.touch(x - self.span.0, y - self.span.1, 1, 1);
        }
    }

    /// Every pixel back to transparent.
    pub fn clear_all(&mut self) {
        self.px.fill(CLEAR);
        self.touch(0, 0, self.span.2, self.span.3);
    }

    pub fn clear(&mut self, x: i32, y: i32) {
        if let Some(i) = self.index(x, y) {
            self.px[i] = CLEAR;
            self.touch(x - self.span.0, y - self.span.1, 1, 1);
        }
    }

    /// Paint landed on `w` x `h` kept pixels from `(x, y)` (in `span`).
    #[inline]
    fn touch(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let t = &mut self.touched;
        *t = (t.0.min(x), t.1.min(y), t.2.max(x + w), t.3.max(y + h));
    }

    /// Forget where paint has landed: from here `take_changes` looks only
    /// at what is painted next. Only for a picture that is, outside what
    /// it paints next, the same as the one it will be compared with.
    pub fn untouch(&mut self) {
        self.touched = UNTOUCHED;
    }

    pub fn put(&mut self, x: i32, y: i32, ch: char) {
        if let Some(c) = ink(ch) {
            self.set(x, y, c);
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, ch);
            }
        }
    }

    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char) {
        for xx in x..x + w {
            self.put(xx, y, ch);
            self.put(xx, y + h - 1, ch);
        }
        for yy in y..y + h {
            self.put(x, yy, ch);
            self.put(x + w - 1, yy, ch);
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, ch: char) {
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

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, ch: char) {
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

    /// A combat mark sampled twice, with its top-left at `(x, y)`. Walk
    /// packed pixels directly: hundreds of transparent cells need no bounds
    /// checks or colour conversion, only the occupied sprite samples do.
    pub fn stamp_twice(&mut self, src: &Img, x: i32, y: i32) {
        let (ox, oy, ow, _) = src.span;
        for (i, &p) in src.px.iter().enumerate() {
            if let Some(c) = unpack(p) {
                let px = x + (ox + i as i32 % ow.max(1)) * 2;
                let py = y + (oy + i as i32 / ow.max(1)) * 2;
                self.set(px, py, c);
                self.set(px + 1, py, c);
                self.set(px, py + 1, c);
                self.set(px + 1, py + 1, c);
            }
        }
    }

    /// `src` drawn `k` times its size, its top left at `(x, y)`.
    pub fn stamp_times(&mut self, src: &Img, x: i32, y: i32, k: i32) {
        let k = k.max(1);
        for (i, &p) in src.px.iter().enumerate() {
            if let Some(c) = unpack(p) {
                let px = x + (i as i32 % src.w) * k;
                let py = y + (i as i32 / src.w) * k;
                for dy in 0..k {
                    for dx in 0..k {
                        self.set(px + dx, py + dy, c);
                    }
                }
            }
        }
    }

    pub fn stamp(&mut self, o: &Img, x: i32, y: i32) {
        // Clipped to what both images keep, a row at a time: opaque words
        // win.
        let (sx, sy, sw, sh) = self.span;
        let (ox, oy, ow, oh) = o.span;
        let (x0, x1) = ((x + ox).max(sx), (x + ox + ow).min(sx + sw));
        let (y0, y1) = ((y + oy).max(sy), (y + oy + oh).min(sy + sh));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let n = (x1 - x0) as usize;
        for row in y0..y1 {
            let src = ((row - y - oy) * ow + (x0 - x - ox)) as usize;
            let dst = ((row - sy) * sw + (x0 - sx)) as usize;
            for (d, &s) in self.px[dst..dst + n].iter_mut().zip(&o.px[src..src + n]) {
                if s & OPAQUE != 0 {
                    *d = s;
                }
            }
        }
        self.touch(x0 - sx, y0 - sy, x1 - x0, y1 - y0);
    }

    /// Stamp `o` at a fractional pixel. Every opaque pixel lands on its
    /// integer cell, exactly as `stamp` puts it; the fraction adds a sliver
    /// past the sprite's right and bottom edges, ordered-dithered in the
    /// sprite's own inks. A move of less than one pixel changes the picture,
    /// a thin line never wears away, and the palette does not grow. An axis
    /// two pixels thick or less gets no sliver, so a dot stays one dot.
    /// `fx == 0` and `fy == 0` match `stamp`.
    pub fn stamp_sub(&mut self, o: &Img, x: f32, y: f32) {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let fx = if o.w > 2 {
            (x - x0 as f32).clamp(0.0, 0.999)
        } else {
            0.0
        };
        let fy = if o.h > 2 {
            (y - y0 as f32).clamp(0.0, 0.999)
        } else {
            0.0
        };
        for yy in 0..o.h {
            for xx in 0..o.w {
                let Some(c) = o.get(xx, yy) else {
                    continue;
                };
                let dx = x0 + xx;
                let dy = y0 + yy;
                self.set(dx, dy, c);
                // A sliver only where the sprite is clear, so it never
                // covers one of the sprite's own pixels.
                if fx > 0.0 && o.get(xx + 1, yy).is_none() && bayer(dx + 1, dy) < fx {
                    self.set(dx + 1, dy, c);
                }
                if fy > 0.0 && o.get(xx, yy + 1).is_none() && bayer(dx, dy + 1) < fy {
                    self.set(dx, dy + 1, c);
                }
            }
        }
    }

    pub fn solid(&self, x: i32, y: i32) -> bool {
        self.get(x, y).is_some()
    }

    pub fn edge(&self, x: i32, y: i32) -> bool {
        self.solid(x, y)
            && (!self.solid(x - 1, y)
                || !self.solid(x + 1, y)
                || !self.solid(x, y - 1)
                || !self.solid(x, y + 1))
    }

    /// Repaint the silhouette's own rim.
    pub fn outline_inside(&mut self, ch: char) {
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
    pub fn outline_outside(&mut self, ch: char) {
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
    pub fn recolor(&self, pairs: &[(char, char)]) -> Img {
        let mut o = self.clone();
        for &(from, to) in pairs {
            let (Some(f), Some(t)) = (ink(from), ink(to)) else {
                continue;
            };
            let (f, t) = (pack(f), pack(t));
            for p in o.px.iter_mut() {
                if *p == f {
                    *p = t;
                }
            }
        }
        o
    }

    pub fn flip_h(&self) -> Img {
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
    pub fn text(&mut self, x: i32, y: i32, s: &str, ch: char) -> i32 {
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

    /// A view of `src` at `(ox, oy)`. Same pixels as get/set, but a row copy
    /// when the rectangle sits inside the source.
    pub fn from_view(src: &Img, ox: i32, oy: i32, w: i32, h: i32) -> Img {
        let w = w.max(0);
        let h = h.max(0);
        if w == 0 || h == 0 {
            return Img::new(w, h);
        }
        let (kx, ky, kw, kh) = src.span;
        if ox >= kx && oy >= ky && ox + w <= kx + kw && oy + h <= ky + kh {
            let mut px = Vec::with_capacity((w * h) as usize);
            let sw = kw as usize;
            let ww = w as usize;
            let ox = (ox - kx) as usize;
            let oy = (oy - ky) as usize;
            for y in 0..h as usize {
                let s = (oy + y) * sw + ox;
                px.extend_from_slice(&src.px[s..s + ww]);
            }
            return Img { w, h, px, span: (0, 0, w, h), touched: UNTOUCHED };
        }
        let mut view = Img::new(w, h);
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = src.get(ox + x, oy + y) {
                    view.set(x, y, c);
                }
            }
        }
        view
    }

    /// What `self` painted over `base` (the same size) since it was last
    /// asked (or `untouch`ed): the smallest rectangle holding every pixel
    /// that differs, as `(x, y, picture)` with those pixels and nothing
    /// else. `base` takes the rectangle, so the two match again. `None` when
    /// nothing differs.
    ///
    /// Only what `self` touched is compared: outside it the two must already
    /// match (`untouch` both alike, then paint `self`), as they do between
    /// one figure and the next. A room's crowd is taken apart one figure at
    /// a time, and comparing the whole room for each cost more than all the
    /// rest of a browser's frame together (a crypt fight: 3.5 ms of 4).
    pub fn take_changes(&mut self, base: &mut Img) -> Option<(i32, i32, Img)> {
        debug_assert_eq!((self.w, self.h, self.span), (base.w, base.h, base.span));
        let (sx, sy, sw, sh) = self.span;
        let w = sw.max(0) as usize;
        let (tx0, ty0, tx1, ty1) = std::mem::replace(&mut self.touched, UNTOUCHED);
        let (tx0, ty0) = (tx0.max(0) as usize, ty0.max(0) as usize);
        let (tx1, ty1) = (tx1.min(sw).max(0) as usize, ty1.min(sh).max(0) as usize);
        if w == 0 || tx0 >= tx1 || ty0 >= ty1 {
            return None;
        }
        let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
        for y in ty0..ty1 {
            let (row, under) = (&self.px[y * w + tx0..y * w + tx1], &base.px[y * w + tx0..y * w + tx1]);
            if row == under {
                continue;
            }
            let first = row.iter().zip(under).position(|(a, b)| a != b).unwrap_or(0);
            let last = row.iter().zip(under).rposition(|(a, b)| a != b).unwrap_or(0);
            y0 = y0.min(y);
            y1 = y;
            x0 = x0.min(tx0 + first);
            x1 = x1.max(tx0 + last);
        }
        if y0 == usize::MAX {
            return None;
        }
        let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut px = vec![CLEAR; cw * ch];
        for y in 0..ch {
            let start = (y0 + y) * w + x0;
            let (row, under) = (&self.px[start..start + cw], &mut base.px[start..start + cw]);
            for (x, (a, b)) in row.iter().zip(under.iter_mut()).enumerate() {
                if a != b {
                    px[y * cw + x] = *a;
                    *b = *a;
                }
            }
        }
        Some((sx + x0 as i32, sy + y0 as i32, Img { w: cw as i32, h: ch as i32, px, span: (0, 0, cw as i32, ch as i32), touched: UNTOUCHED }))
    }

    /// What `self` painted over `base` (the same size), at full size:
    /// transparent wherever the two agree.
    pub fn changes_over(&self, base: &Img) -> Img {
        debug_assert_eq!((self.w, self.h, self.span), (base.w, base.h, base.span));
        let px = self
            .px
            .iter()
            .zip(&base.px)
            .map(|(&a, &b)| if a == b { CLEAR } else { a })
            .collect();
        Img { w: self.w, h: self.h, px, span: self.span, touched: UNTOUCHED }
    }

    /// Same bytes `rgb_bytes` would hash, without the intermediate buffer.
    pub fn content_hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for pixel in &self.px {
            let [r, g, b] = paper(*pixel);
            for byte in [r, g, b] {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash
    }

    /// Figure over scenery, 1:1, into `out`. Transparent stays black paper.
    /// Same pixels as a per-pixel get/set when the three images match.
    pub fn compose_1x(figures: &Img, scenery: &Img, out: &mut Img) {
        debug_assert_eq!((figures.w, figures.h), (scenery.w, scenery.h));
        debug_assert_eq!((out.w, out.h), (figures.w, figures.h));
        let black = pack(BLACK);
        for ((out, &f), &s) in out.px.iter_mut().zip(&figures.px).zip(&scenery.px) {
            *out = if f & OPAQUE != 0 {
                f
            } else if s & OPAQUE != 0 {
                s
            } else {
                black
            };
        }
        out.touch(0, 0, out.span.2, out.span.3);
    }

    /// The packed words, row-major, `0xAARRGGBB`: alpha 0 is transparent
    /// (black paper when shown).
    pub fn words(&self) -> &[u32] {
        &self.px
    }

    /// An image of packed words, as `words` gives them (a client's frame).
    pub fn from_words(w: i32, h: i32, px: Vec<u32>) -> Img {
        assert_eq!(px.len(), (w.max(0) * h.max(0)) as usize, "a frame of {w}x{h} words");
        Img { w, h, px, span: (0, 0, w.max(0), h.max(0)), touched: UNTOUCHED }
    }

    /// Row-major RGB bytes; transparent pixels are black paper.
    pub fn rgb_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 3);
        for p in &self.px {
            out.extend_from_slice(&paper(*p));
        }
        out
    }

    /// Row-major opaque RGBA bytes, each pixel blown up to a `k`x`k` block.
    pub fn rgba_scaled(&self, k: u32) -> Vec<u8> {
        if !self.whole() {
            let mut whole = Img::new(self.w, self.h);
            whole.stamp(self, 0, 0);
            return whole.rgba_scaled(k);
        }
        let k = k.max(1) as usize;
        let (w, h) = (self.w.max(0) as usize, self.h.max(0) as usize);
        let mut out = Vec::with_capacity(w * h * k * k * 4);
        let mut row = Vec::with_capacity(w * k * 4);
        for y in 0..h {
            row.clear();
            for x in 0..w {
                let [r, g, b] = paper(self.px[y * w + x]);
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
    pub fn rgba_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px {
            out.extend_from_slice(&paper(*p));
            out.push(255);
        }
        out
    }

    pub fn pixels(&self) -> impl Iterator<Item = Rgb> + '_ {
        self.px.iter().map(|&p| paper(p))
    }
}

// ─── the sprite atlas ────────────────────────────────────────────────────────

/// The atlas sheet's width; it grows in rows of shelves up to `ATLAS_MAX_H`,
/// then starts over (an authored set of sprites fits many times over).
const ATLAS_W: i32 = 1024;
const ATLAS_MAX_H: i32 = 4096;

/// Every sprite read from ink rows, packed on shelves into one sheet, and
/// where each one sits. One per render thread, so no thread waits on another.
struct Atlas {
    sheet: Img,
    /// The open shelf: where the next sprite goes and the shelf's height.
    shelf: (i32, i32, i32),
    index: std::collections::HashMap<u64, [i32; 4]>,
}

impl Atlas {
    fn get(&self, key: u64) -> Option<Img> {
        let [x, y, w, h] = *self.index.get(&key)?;
        Some(Img::from_view(&self.sheet, x, y, w, h))
    }

    fn insert(&mut self, key: u64, img: &Img) {
        if img.w > ATLAS_W || img.h > ATLAS_MAX_H || img.w <= 0 || img.h <= 0 {
            return;
        }
        let (mut x, mut y, mut h) = self.shelf;
        if x + img.w > ATLAS_W {
            (x, y, h) = (0, y + h, 0);
        }
        if y + img.h > ATLAS_MAX_H {
            self.index.clear();
            self.sheet = Img::new(ATLAS_W, 0);
            (x, y, h) = (0, 0, 0);
        }
        if y + img.h > self.sheet.h {
            let rows = y + img.h;
            self.sheet.px.resize((ATLAS_W * rows) as usize, CLEAR);
            self.sheet.h = rows;
            self.sheet.span = (0, 0, ATLAS_W, rows);
        }
        let n = img.w as usize;
        for row in 0..img.h {
            let dst = ((y + row) * ATLAS_W + x) as usize;
            let src = (row * img.w) as usize;
            self.sheet.px[dst..dst + n].copy_from_slice(&img.px[src..src + n]);
        }
        self.index.insert(key, [x, y, img.w, img.h]);
        self.shelf = (x + img.w, y, h.max(img.h));
    }
}

std::thread_local! {
    static ATLAS: std::cell::RefCell<Atlas> = std::cell::RefCell::new(Atlas {
        sheet: Img::new(ATLAS_W, 0),
        shelf: (0, 0, 0),
        index: std::collections::HashMap::new(),
    });
}

/// How many sprites this thread's atlas holds, and the sheet's height.
#[cfg(any(test, feature = "testing"))]
pub fn atlas_size() -> (usize, i32) {
    ATLAS.with(|atlas| {
        let atlas = atlas.borrow();
        (atlas.index.len(), atlas.sheet.h)
    })
}

/// A sprite's rows, hashed: its identity in the atlas.
fn rows_key(rows: &[&str]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for row in rows {
        for byte in row.bytes().chain([0xff]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash ^ rows.len() as u64
}

// ─── 5x7 font ────────────────────────────────────────────────────────────────

pub const GLYPH_ADVANCE: i32 = 6;

pub fn text_width(s: &str) -> i32 {
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
        '[' => [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
        ']' => [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
        '\\' => [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
        '=' => [0x00, 0x00, 0x1f, 0x00, 0x1f, 0x00, 0x00],
        ';' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x04, 0x08],
        '`' => [0x08, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
        // A hold bar's cells: filled, and the track still to fill.
        '█' => [0x1f, 0x1f, 0x1f, 0x1f, 0x1f, 0x1f, 0x1f],
        '░' => [0x15, 0x0a, 0x15, 0x0a, 0x15, 0x0a, 0x15],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod subpixel_tests {
    use super::*;

    fn sprite() -> Img {
        let mut im = Img::new(5, 7);
        for y in 1..6 {
            for x in 1..4 {
                im.set(x, y, [0xa8, 0x34, 0x1f]);
            }
        }
        im.set(2, 0, [0xe4, 0xff, 0xf8]);
        im
    }

    #[test]
    fn quarter_pixel_changes_the_edge() {
        let sprite = sprite();
        let mut still = Img::new(16, 16);
        let mut moved = Img::new(16, 16);
        still.stamp_sub(&sprite, 4.0, 4.0);
        moved.stamp_sub(&sprite, 4.4, 4.0);
        assert_ne!(still.rgb_bytes(), moved.rgb_bytes());
        // Integer placement matches a plain stamp, so snapshots stay put.
        let mut plain = Img::new(16, 16);
        plain.stamp(&sprite, 4, 4);
        assert_eq!(still.rgb_bytes(), plain.rgb_bytes());
        // No colour outside the sprite's own inks.
        let allowed = [[0xa8, 0x34, 0x1f], [0xe4, 0xff, 0xf8]];
        for px in moved.px.iter().filter_map(|&p| unpack(p)) {
            assert!(allowed.contains(&px), "{px:?}");
        }
    }

    /// Every fractional position, at every phase of the dither pattern.
    fn sweep(sprite: &Img, check: impl Fn(&Img, f32, f32)) {
        for cell in 0..4 {
            for sx in 0..32 {
                for sy in 0..32 {
                    let x = 6.0 + cell as f32 + sx as f32 / 32.0;
                    let y = 5.0 + cell as f32 + sy as f32 / 32.0;
                    let mut cv = Img::new(24, 24);
                    cv.stamp_sub(sprite, x, y);
                    check(&cv, x, y);
                }
            }
        }
    }

    #[test]
    fn every_pixel_of_a_sprite_is_drawn_at_every_fraction() {
        let ink = [0xe4, 0xff, 0xf8];
        let mut dot = Img::new(1, 1);
        dot.set(0, 0, ink);
        sweep(&dot, |cv, x, y| {
            let drawn: Vec<(i32, i32)> = (0..cv.h)
                .flat_map(|py| (0..cv.w).map(move |px| (px, py)))
                .filter(|&(px, py)| cv.get(px, py).is_some())
                .collect();
            assert_eq!(
                drawn,
                vec![(x.floor() as i32, y.floor() as i32)],
                "a dot is one pixel on its cell at ({x}, {y})"
            );
        });
        let mut post = Img::new(1, 6);
        let mut rail = Img::new(6, 1);
        for i in 0..6 {
            post.set(0, i, ink);
            rail.set(i, 0, ink);
        }
        for (line, sprite) in [("post", &post), ("rail", &rail), ("sprite", &sprite())] {
            sweep(sprite, |cv, x, y| {
                let (x0, y0) = (x.floor() as i32, y.floor() as i32);
                for yy in 0..sprite.h {
                    for xx in 0..sprite.w {
                        if let Some(c) = sprite.get(xx, yy) {
                            assert_eq!(
                                cv.get(x0 + xx, y0 + yy),
                                Some(c),
                                "{line} pixel ({xx}, {yy}) at ({x}, {y})"
                            );
                        }
                    }
                }
                for py in 0..cv.h {
                    for px in 0..cv.w {
                        if cv.get(px, py).is_some() {
                            assert!(
                                (x0..=x0 + sprite.w).contains(&px)
                                    && (y0..=y0 + sprite.h).contains(&py),
                                "{line} drew ({px}, {py}) past its sliver at ({x}, {y})"
                            );
                        }
                    }
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__ink_atlas_tests.rs"]
mod atlas_tests;
