//! Braille dots from a picture: the structure-aware fitter (each two-by-four
//! cell takes the dot pattern that best keeps its own light/dark split), and
//! the graded converter the ride, the crawl and the art kit draw through.

/// Deliberately small palette shared by terminal-native visualizations.
pub const DMD_PALETTE: [[u8; 3]; 8] = [
    [0, 0, 0],
    [86, 232, 255],
    [91, 190, 255],
    [58, 111, 151],
    [184, 228, 255],
    [255, 207, 92],
    [255, 103, 126],
    [99, 241, 169],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColoredBrailleCell {
    pub glyph: char,
    pub fg: [u8; 3],
}

impl Default for ColoredBrailleCell {
    fn default() -> Self {
        Self {
            glyph: '\u{2800}',
            fg: DMD_PALETTE[0],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColoredBrailleImage {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<ColoredBrailleCell>,
}

impl ColoredBrailleImage {
    pub fn cell(&self, x: usize, y: usize) -> Option<ColoredBrailleCell> {
        (x < self.width && y < self.height).then(|| self.cells[y * self.width + x])
    }
}

/// One pixel as the braille fitter sees it.
#[derive(Clone, Copy, Debug, Default)]
pub struct DotSample {
    /// Brightness after the caller's tone curve, 0..1. Structure is read
    /// from this.
    pub luma: f32,
    /// How much ink a flat fill of this pixel deserves, 0..1.
    pub level: f32,
    /// Its colour.
    pub rgb: [f32; 3],
}

/// Below this a pixel is true black and never draws a dot.
const DOT_FLOOR: f32 = 0.07;
/// Dim haze falls into terminal black in the tonal 3D bridge, instead of
/// leaving a full rectangle of barely lit sky or mine fog.
const WORLD_DOT_FLOOR: f32 = 0.14;

/// A cell whose brightness spans less than this (or less than this share
/// of its brightest pixel) is a flat fill, not an edge.
const DOT_FLAT: f32 = 0.16;
const DOT_FLAT_SHARE: f32 = 0.45;

/// Within this of the threshold a pixel follows the edge beside it.
const DOT_EDGE: f32 = 0.035;

/// Structure-aware braille: for each two-by-four cell, the dot pattern that
/// best reproduces the cell's own light/dark split, so a shape keeps its
/// outline instead of becoming dot density.
///
/// - A dot is on where the pixel is brighter than a mix of the cell's mean,
///   its neighbours' mean and the midpoint of its own range.
/// - True black (below an absolute floor) is always empty.
/// - A pixel sitting on the threshold copies the decision of the pixel to its
///   left (else above), so edges run on across cells.
/// - Only a genuinely flat cell falls back to ordered dither of its level.
///
/// Returns, per cell, the dot bits and the mean colour of the on-pixels.
pub fn fit_braille(
    cells_w: usize,
    cells_h: usize,
    sample: impl Fn(usize, usize) -> DotSample,
) -> Vec<(u8, [f32; 3])> {
    fit_braille_tonal(cells_w, cells_h, sample, false)
}

/// 3D flat planes carry value in their ink instead of checker density. The
/// edge fitter still supplies the exact local silhouette and occlusion split.
pub fn fit_braille_tonal(
    cells_w: usize,
    cells_h: usize,
    sample: impl Fn(usize, usize) -> DotSample,
    tonal: bool,
) -> Vec<(u8, [f32; 3])> {
    let (pw, ph) = (cells_w * 2, cells_h * 4);
    let px: Vec<DotSample> = (0..pw * ph).map(|i| sample(i % pw, i / pw)).collect();
    let mut means = vec![0.0f32; cells_w * cells_h];
    for cy in 0..cells_h {
        for cx in 0..cells_w {
            let mut sum = 0.0;
            for ly in 0..4 {
                for lx in 0..2 {
                    sum += px[(cy * 4 + ly) * pw + cx * 2 + lx].luma;
                }
            }
            means[cy * cells_w + cx] = sum / 8.0;
        }
    }
    let mut on = vec![false; pw * ph];
    let mut out = vec![(0u8, [0.0f32; 3]); cells_w * cells_h];
    for cy in 0..cells_h {
        for cx in 0..cells_w {
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for ly in 0..4 {
                for lx in 0..2 {
                    let l = px[(cy * 4 + ly) * pw + cx * 2 + lx].luma;
                    lo = lo.min(l);
                    hi = hi.max(l);
                }
            }
            let mean = means[cy * cells_w + cx];
            let (mut nsum, mut n) = (0.0, 0.0);
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                    if (dx, dy) != (0, 0) && nx >= 0 && ny >= 0 && (nx as usize) < cells_w && (ny as usize) < cells_h {
                        nsum += means[ny as usize * cells_w + nx as usize];
                        n += 1.0;
                    }
                }
            }
            let around = if n > 0.0 { nsum / n } else { mean };
            let threshold = 0.4 * mean + 0.3 * around + 0.3 * (lo + hi) * 0.5;
            let flat = hi - lo < DOT_FLAT || hi - lo < DOT_FLAT_SHARE * hi;
            let (mut bits, mut ink, mut count) = (0u8, [0.0f32; 3], 0.0f32);
            for ly in 0..4 {
                for lx in 0..2 {
                    let (x, y) = (cx * 2 + lx, cy * 4 + ly);
                    let p = px[y * pw + x];
                    let lit = if p.luma < if tonal { WORLD_DOT_FLOOR } else { DOT_FLOOR } {
                        false
                    } else if flat && tonal {
                        true
                    } else if flat {
                        // A flat fill is drawn solid once it is more than
                        // dim, so its colour carries the value; only the
                        // darkest fills thin out, and never into a mesh.
                        let e = ((p.level - DOT_FLOOR) / 0.25).clamp(0.0, 1.0);
                        e * e * (3.0 - 2.0 * e) > dot_bayer(x, y)
                    } else if (p.luma - threshold).abs() < DOT_EDGE {
                        if x > 0 {
                            on[y * pw + x - 1]
                        } else if y > 0 {
                            on[(y - 1) * pw + x]
                        } else {
                            p.luma > threshold
                        }
                    } else {
                        p.luma > threshold
                    };
                    on[y * pw + x] = lit;
                    if lit {
                        bits |= braille_dot_bit(lx, ly);
                        for k in 0..3 {
                            ink[k] += p.rgb[k];
                        }
                        count += 1.0;
                    }
                }
            }
            if count > 0.0 {
                out[cy * cells_w + cx] = (bits, ink.map(|v| v / count));
            }
        }
    }
    out
}

/// The 8x8 ordered threshold for flat fills, in 0..1.
fn dot_bayer(x: usize, y: usize) -> f32 {
    const B: [[u8; 8]; 8] = [
        [0, 32, 8, 40, 2, 34, 10, 42],
        [48, 16, 56, 24, 50, 18, 58, 26],
        [12, 44, 4, 36, 14, 46, 6, 38],
        [60, 28, 52, 20, 62, 30, 54, 22],
        [3, 35, 11, 43, 1, 33, 9, 41],
        [51, 19, 59, 27, 49, 17, 57, 25],
        [15, 47, 7, 39, 13, 45, 5, 37],
        [63, 31, 55, 23, 61, 29, 53, 21],
    ];
    (B[y % 8][x % 8] as f32 + 0.5) / 64.0
}

pub const fn braille_dot_bit(local_x: usize, local_y: usize) -> u8 {
    match (local_x, local_y) {
        (0, 0) => 1 << 0,
        (0, 1) => 1 << 1,
        (0, 2) => 1 << 2,
        (0, 3) => 1 << 6,
        (1, 0) => 1 << 3,
        (1, 1) => 1 << 4,
        (1, 2) => 1 << 5,
        (1, 3) => 1 << 7,
        _ => 0,
    }
}

pub const fn braille_bits(glyph: char) -> u8 {
    let code = glyph as u32;
    if code >= 0x2800 && code <= 0x28ff {
        (code - 0x2800) as u8
    } else {
        0
    }
}

pub fn braille_char(bits: u8) -> char {
    char::from_u32(0x2800 + u32::from(bits)).unwrap_or('\u{2800}')
}
