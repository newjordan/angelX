//! The realm's inks as the pit's only colours: every dot the stone, sand and
//! sky are painted in is one of the master palette's seventy-five (or black
//! paper), reached through an ordered dither so a gradient is a gradient.
use crate::render::overworld::ink::{PALETTE, Rgb, bayer};
use std::sync::OnceLock;

const BITS: usize = 6;
const SIDE: usize = 1 << BITS;

/// The master palette less the greens of the foliage bank and the cyans and
/// bright blues of the stone and signal banks: what stone, sand, cloth and
/// fire are ever painted in. (Sky-blue and sea-green would read as moss
/// and glass in a dithered dust-brown pit.)
fn inks() -> &'static [Rgb] {
    static INKS: OnceLock<Vec<Rgb>> = OnceLock::new();
    INKS.get_or_init(|| {
        let keep = |i: usize| match i {
            // structure and timber: all of them
            0..=14 | 30..=44 => true,
            // stone: the cold greys for shadow, not the blues
            45 | 47 | 48 | 49 | 50 | 53 | 56 | 57 => true,
            // signal: dark gold, the flame golds, the pale creams, the two reds
            65 | 66 | 67 | 68 | 69 | 70 | 71 | 72 | 73 | 74 => true,
            _ => false,
        };
        let mut v: Vec<Rgb> = PALETTE.iter().enumerate().filter(|(i, _)| keep(*i)).map(|(_, &c)| [(c >> 16) as u8, (c >> 8) as u8, c as u8]).collect();
        v.push([0, 0, 0]);
        v
    })
}

/// Every colour of a 64 by 64 by 64 cube, as the index of the nearest ink.
fn cube() -> &'static [u8] {
    static CUBE: OnceLock<Vec<u8>> = OnceLock::new();
    CUBE.get_or_init(|| {
        let inks = inks();
        let step = 256.0 / SIDE as f32;
        let mut out = Vec::with_capacity(SIDE * SIDE * SIDE);
        for r in 0..SIDE {
            for g in 0..SIDE {
                for b in 0..SIDE {
                    let c = [(r as f32 + 0.5) * step, (g as f32 + 0.5) * step, (b as f32 + 0.5) * step];
                    let mut best = (f32::MAX, 0u8);
                    for (i, p) in inks.iter().enumerate() {
                        // Redmean-ish weights: the eye is hardest on green, easiest on blue.
                        let (dr, dg, db) = (c[0] - p[0] as f32, c[1] - p[1] as f32, c[2] - p[2] as f32);
                        let d = 2.0 * dr * dr + 4.0 * dg * dg + 3.0 * db * db;
                        if d < best.0 {
                            best = (d, i as u8);
                        }
                    }
                    out.push(best.1);
                }
            }
        }
        out
    })
}

/// The inks and their cube, fetched once a frame.
#[derive(Clone, Copy)]
pub(in super::super) struct Quant {
    cube: &'static [u8],
    inks: &'static [Rgb],
}

pub(in super::super) fn quant() -> Quant {
    Quant { cube: cube(), inks: inks() }
}

impl Quant {
    /// `c` (each channel `0..=1`) dithered onto the inks at dot `(x, y)`.
    #[inline]
    pub(in super::super) fn dot(&self, c: [f32; 3], x: i32, y: i32, amp: f32) -> Rgb {
        let o = (bayer(x, y) - 0.5) * amp;
        let q = |v: f32| ((v * 255.0 + o).clamp(0.0, 255.0) as usize) >> (8 - BITS);
        self.inks[usize::from(self.cube[(q(c[0]) * SIDE + q(c[1])) * SIDE + q(c[2])])]
    }
}

/// Builds the cube ahead of the first frame.
pub(in super::super) fn warm() {
    let _ = cube();
}
