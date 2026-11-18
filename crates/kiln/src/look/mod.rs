//! Kiln's eyes: everything a model (or a person) needs to see what a recipe
//! made, as pictures and as text.
//!
//! - [`shot`]: sheets. The grain ladder at one on-screen size (grain `g`
//!   shown at `4 / g`), each grain at its true size on a floor, a zoom, a
//!   reference beside them; frame strips and onion skins for clips.
//! - [`describe`]: the recipe as text: parts, their boxes and inks at each
//!   grain, depth order, sockets, clips.
//! - [`check`]: lints, each with its grain, clip and frame: lone pixels,
//!   jaggies, the silhouette at grain 1, floor contact, sockets in bounds,
//!   frame-to-frame jitter.
//! - [`compare`]: a picture against a reference: size, silhouette overlap,
//!   inks, and a picture of where they differ.
//!
//! Pictures here are [`Rgba`]; `encode` (feature `cli`) writes them.

pub mod check;
pub mod compare;
pub mod describe;
pub mod shot;

use crate::palette::Palette;
use crate::raster::Raster;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Rgba {
    pub fn filled(w: u32, h: u32, c: [u8; 4]) -> Rgba {
        Rgba { w, h, px: c.iter().copied().cycle().take((w * h * 4) as usize).collect() }
    }

    pub fn of(ras: &Raster) -> Rgba {
        Rgba { w: ras.w, h: ras.h, px: ras.to_rgba(Palette::realm()) }
    }

    pub fn get(&self, x: i32, y: i32) -> [u8; 4] {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return [0; 4];
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    pub fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h || c[3] == 0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        if c[3] == 255 {
            self.px[i..i + 4].copy_from_slice(&c);
        } else {
            let a = c[3] as u32;
            for k in 0..3 {
                self.px[i + k] = ((c[k] as u32 * a + self.px[i + k] as u32 * (255 - a)) / 255) as u8;
            }
            self.px[i + 3] = self.px[i + 3].max(c[3]);
        }
    }

    pub fn blit(&mut self, src: &Rgba, x: i32, y: i32, scale: u32) {
        for sy in 0..src.h {
            for sx in 0..src.w {
                let c = src.get(sx as i32, sy as i32);
                if c[3] == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        self.set(x + (sx * scale + dx) as i32, y + (sy * scale + dy) as i32, c);
                    }
                }
            }
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, c: [u8; 4]) {
        for j in 0..h as i32 {
            for i in 0..w as i32 {
                self.set(x + i, y + j, c);
            }
        }
    }

    pub fn border(&mut self, x: i32, y: i32, w: u32, h: u32, c: [u8; 4]) {
        for i in 0..w as i32 {
            self.set(x + i, y, c);
            self.set(x + i, y + h as i32 - 1, c);
        }
        for j in 0..h as i32 {
            self.set(x, y + j, c);
            self.set(x + w as i32 - 1, y + j, c);
        }
    }

    pub fn tiled(src: &Rgba, w: u32, h: u32, scale: u32) -> Rgba {
        let mut out = Rgba::filled(w, h, [0, 0, 0, 255]);
        let (tw, th) = ((src.w * scale).max(1), (src.h * scale).max(1));
        for y in 0..h {
            for x in 0..w {
                let c = src.get(((x % tw) / scale) as i32, ((y % th) / scale) as i32);
                out.set(x as i32, y as i32, [c[0], c[1], c[2], 255]);
            }
        }
        out
    }

    pub fn text(&mut self, x: i32, y: i32, words: &str, c: [u8; 4], scale: u32) -> u32 {
        let s = scale as i32;
        let mut at = x;
        for ch in words.chars() {
            let rows = glyph(ch);
            for (j, row) in rows.iter().enumerate() {
                for (i, dot) in row.bytes().enumerate() {
                    if dot == b'#' {
                        self.fill(at + i as i32 * s, y + j as i32 * s, scale, scale, c);
                    }
                }
            }
            at += 4 * s;
        }
        (at - x) as u32
    }
}

pub fn text_width(words: &str, scale: u32) -> u32 {
    words.chars().count() as u32 * 4 * scale
}

fn glyph(ch: char) -> [&'static str; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [".#.", "#.#", "###", "#.#", "#.#"],
        'B' => ["##.", "#.#", "##.", "#.#", "##."],
        'C' => [".##", "#..", "#..", "#..", ".##"],
        'D' => ["##.", "#.#", "#.#", "#.#", "##."],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'F' => ["###", "#..", "##.", "#..", "#.."],
        'G' => [".##", "#..", "#.#", "#.#", ".##"],
        'H' => ["#.#", "#.#", "###", "#.#", "#.#"],
        'I' => ["###", ".#.", ".#.", ".#.", "###"],
        'J' => ["..#", "..#", "..#", "#.#", ".#."],
        'K' => ["#.#", "#.#", "##.", "#.#", "#.#"],
        'L' => ["#..", "#..", "#..", "#..", "###"],
        'M' => ["#.#", "###", "###", "#.#", "#.#"],
        'N' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'O' => [".#.", "#.#", "#.#", "#.#", ".#."],
        'P' => ["##.", "#.#", "##.", "#..", "#.."],
        'Q' => [".#.", "#.#", "#.#", "##.", ".##"],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'S' => [".##", "#..", ".#.", "..#", "##."],
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'V' => ["#.#", "#.#", "#.#", "#.#", ".#."],
        'W' => ["#.#", "#.#", "###", "###", "#.#"],
        'X' => ["#.#", "#.#", ".#.", "#.#", "#.#"],
        'Y' => ["#.#", "#.#", ".#.", ".#.", ".#."],
        'Z' => ["###", "..#", ".#.", "#..", "###"],
        '0' => ["###", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["##.", "..#", ".#.", "#..", "###"],
        '3' => ["##.", "..#", ".#.", "..#", "##."],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "##.", "..#", "##."],
        '6' => [".##", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", ".#.", ".#.", ".#."],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "##."],
        '.' => ["...", "...", "...", "...", ".#."],
        ',' => ["...", "...", "...", ".#.", "#.."],
        ':' => ["...", ".#.", "...", ".#.", "..."],
        '-' => ["...", "...", "###", "...", "..."],
        '/' => ["..#", "..#", ".#.", "#..", "#.."],
        '=' => ["...", "###", "...", "###", "..."],
        '+' => ["...", ".#.", "###", ".#.", "..."],
        '(' => [".#.", "#..", "#..", "#..", ".#."],
        ')' => [".#.", "..#", "..#", "..#", ".#."],
        '%' => ["#..", "..#", ".#.", "#..", "..#"],
        '#' => ["#.#", "###", "#.#", "###", "#.#"],
        '_' => ["...", "...", "...", "...", "###"],
        '<' => ["..#", ".#.", "#..", ".#.", "..#"],
        '>' => ["#..", ".#.", "..#", ".#.", "#.."],
        _ => ["...", "...", "...", "...", "..."],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_are_three_by_five() {
        let mut img = Rgba::filled(40, 7, [0, 0, 0, 255]);
        let w = img.text(1, 1, "g4 x1", [255, 255, 255, 255], 1);
        assert_eq!(w, text_width("g4 x1", 1));
        assert!(img.px.chunks(4).any(|c| c[0] == 255));
    }
}
