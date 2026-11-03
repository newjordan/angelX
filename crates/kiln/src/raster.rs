//! The picture shapes draw into: an ink per pixel, with the depth and the
//! part that put it there, so nearer parts cover farther ones and the line
//! work can find where one part meets another.

use crate::palette::{Ink, Palette};

/// A part's number in a raster; 0 is none.
pub type PartId = u16;

/// An indexed picture with depth.
#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub w: u32,
    pub h: u32,
    /// The ink of each pixel, row by row; 0 is clear.
    pub ink: Vec<Ink>,
    /// How near each pixel's surface is (larger is nearer the viewer);
    /// `f32::NEG_INFINITY` where nothing is drawn.
    pub depth: Vec<f32>,
    /// The part that drew each pixel; 0 where none did.
    pub part: Vec<PartId>,
}

impl Raster {
    /// A clear `w` x `h` picture.
    pub fn new(w: u32, h: u32) -> Raster {
        let n = (w * h) as usize;
        Raster { w, h, ink: vec![Ink::CLEAR; n], depth: vec![f32::NEG_INFINITY; n], part: vec![0; n] }
    }

    fn at(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h).then(|| (y as u32 * self.w + x as u32) as usize)
    }

    /// The ink at `(x, y)`; clear off the picture.
    pub fn get(&self, x: i32, y: i32) -> Ink {
        self.at(x, y).map_or(Ink::CLEAR, |i| self.ink[i])
    }

    /// The depth at `(x, y)`.
    pub fn depth_at(&self, x: i32, y: i32) -> f32 {
        self.at(x, y).map_or(f32::NEG_INFINITY, |i| self.depth[i])
    }

    /// The part at `(x, y)`.
    pub fn part_at(&self, x: i32, y: i32) -> PartId {
        self.at(x, y).map_or(0, |i| self.part[i])
    }

    /// Lay `ink` at `(x, y)` if it is at least as near as what is there.
    /// Returns whether it was laid.
    pub fn put(&mut self, x: i32, y: i32, ink: Ink, depth: f32, part: PartId) -> bool {
        match self.at(x, y) {
            Some(i) if depth >= self.depth[i] => {
                self.ink[i] = ink;
                self.depth[i] = depth;
                self.part[i] = part;
                true
            }
            _ => false,
        }
    }

    /// Lay `ink` at `(x, y)` over whatever is there, keeping its depth and
    /// part (line work, decals, recolours).
    pub fn paint(&mut self, x: i32, y: i32, ink: Ink) {
        if let Some(i) = self.at(x, y) {
            self.ink[i] = ink;
        }
    }

    /// Whether `(x, y)` holds an ink.
    pub fn is_inked(&self, x: i32, y: i32) -> bool {
        !self.get(x, y).is_clear()
    }

    /// RGBA bytes, row by row; clear is transparent.
    pub fn to_rgba(&self, palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.ink.len() * 4);
        for &ink in &self.ink {
            match palette.rgb(ink) {
                Some([r, g, b]) => out.extend_from_slice(&[r, g, b, 255]),
                None => out.extend_from_slice(&[0, 0, 0, 0]),
            }
        }
        out
    }

    /// The game's ink rows: one string a row, one key a pixel, `.` clear.
    pub fn ink_rows(&self, palette: &Palette) -> Vec<String> {
        self.ink.chunks(self.w as usize).map(|row| row.iter().map(|&i| palette.key(i)).collect()).collect()
    }

    /// The box round every inked pixel, `(x, y, w, h)`; none when clear.
    pub fn bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                if !self.ink[(y * self.w + x) as usize].is_clear() {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        (x0 != u32::MAX).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearer_covers_farther_and_rows_read_back() {
        let p = Palette::realm();
        let (k, w) = (p.ink('k').unwrap(), p.ink('W').unwrap());
        let mut r = Raster::new(3, 2);
        assert!(r.put(1, 0, k, 0.0, 1));
        assert!(!r.put(1, 0, w, -1.0, 2));
        assert!(r.put(1, 0, w, 0.5, 2));
        assert_eq!(r.part_at(1, 0), 2);
        assert!(!r.put(9, 9, k, 1.0, 1));
        assert_eq!(r.ink_rows(p), vec![".W.".to_string(), "...".to_string()]);
        assert_eq!(r.bounds(), Some((1, 0, 1, 1)));
        assert_eq!(&r.to_rgba(p)[4..8], &[0xd5, 0xde, 0xe0, 255]);
    }
}
