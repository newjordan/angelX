//! A frame as braille: each 2x4 block of pixels becomes one terminal cell
//! whose dots are the lighter half of the block, drawn in the lighter
//! colour over the darker. The same two-tone reduction the knight racers
//! example uses, for the terminal.

use crate::render::braille::{braille_char, braille_dot_bit};
use std::fmt::Write as _;

/// One terminal cell: its dots, the ink they are drawn in, and the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub bits: u8,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
}

/// The cells of a `w` by `h` frame of pixels (both multiples of 2 and 4),
/// row by row, into `out`.
pub fn cells(rgb: &[[u8; 3]], w: usize, h: usize, out: &mut Vec<Cell>) {
    out.clear();
    for cy in 0..h / 4 {
        for cx in 0..w / 2 {
            let (mut px, mut luma) = ([[0f32; 3]; 8], [0f32; 8]);
            for (i, (p, l)) in px.iter_mut().zip(&mut luma).enumerate() {
                *p = rgb[(cy * 4 + i / 2) * w + cx * 2 + i % 2].map(f32::from);
                *l = 0.3 * p[0] + 0.59 * p[1] + 0.11 * p[2];
            }
            let mean = luma.iter().sum::<f32>() / 8.0;
            let (mut ink, mut ground, mut bits) = (([0f32; 3], 0f32), ([0f32; 3], 0f32), 0u8);
            for (i, (p, &l)) in px.iter().zip(&luma).enumerate() {
                let lit = l > mean + 1.0;
                if lit {
                    bits |= braille_dot_bit(i % 2, i / 2);
                }
                let (sum, n) = if lit { &mut ink } else { &mut ground };
                sum.iter_mut().zip(p).for_each(|(s, c)| *s += c);
                *n += 1.0;
            }
            let avg = |(s, n): ([f32; 3], f32)| if n > 0.0 { s.map(|c| (c / n) as u8) } else { [0; 3] };
            out.push(Cell { bits, fg: avg(ink), bg: avg(ground) });
        }
    }
}

/// The cells as terminal text: truecolor only where the colours change.
pub fn ansi(cells: &[Cell], cols: usize, out: &mut String) {
    for row in cells.chunks(cols) {
        let (mut fg, mut bg) = (None, None);
        for c in row {
            let ink = c.bits != 0;
            let (new_fg, new_bg) = (ink && fg != Some(c.fg), bg != Some(c.bg));
            if new_fg || new_bg {
                out.push_str("\x1b[");
                if new_fg {
                    let _ = write!(out, "38;2;{};{};{}", c.fg[0], c.fg[1], c.fg[2]);
                    fg = Some(c.fg);
                }
                if new_bg {
                    let _ = write!(out, "{}48;2;{};{};{}", if new_fg { ";" } else { "" }, c.bg[0], c.bg[1], c.bg[2]);
                    bg = Some(c.bg);
                }
                out.push('m');
            }
            out.push(if ink { braille_char(c.bits) } else { ' ' });
        }
        out.push_str("\x1b[0m\n");
    }
}
