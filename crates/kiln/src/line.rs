//! Line work: the silhouette's line, the inner lines where one part covers
//! another, and the tidying that keeps lines one pixel and clean.
//!
//! - **The outline** at grain 2 and up is drawn outside the figure, one
//!   pixel wide. Where it faces the light it is selout: its neighbour's ink
//!   `selout` steps down that neighbour's ramp (the outline ink below the
//!   ramp's darkest); elsewhere it is the outline ink.
//! - **At grain 1** a pixel outside the figure is a quarter of its width,
//!   so the line is the figure's own edge instead, and only away from the
//!   light: there the edge takes its ramp's second-darkest tone, and the lit
//!   edge keeps its shading (the game's own 16 px art is drawn so).
//!   One-pixel gaps between parts are closed there with the same seam tone.
//! - **Inner lines**: where a part covers another and the depth breaks by
//!   more than the recipe's `outline.inner`, the covered pixel along the
//!   seam takes the nearer part's darkest tone (never black).
//! - **Tidying**: a lone pixel (no 4-neighbour of its ink, all of them its
//!   own part) takes its neighbours' ink, at grain 2 and up; a line pixel
//!   that only doubles an L-corner of a line drawn on the figure is given
//!   back its fill.

use crate::palette::{Ink, Ramp};
use crate::raster::{PartId, Raster};

/// What laid a pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mark {
    /// Nothing: the pixel is clear.
    #[default]
    Clear,
    /// A part's shaded surface.
    Fill,
    /// The silhouette's line, outside the figure.
    Outline,
    /// The silhouette's line on the figure's own edge (grain 1).
    Edge,
    /// An inner line along a seam.
    Inner,
    /// A decal.
    Decal,
}

impl Mark {
    /// Whether it is line work.
    pub fn is_line(self) -> bool {
        matches!(self, Mark::Outline | Mark::Edge | Mark::Inner)
    }
}

/// The part id outline pixels carry.
pub const LINE: PartId = PartId::MAX;

const N4: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const N8: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)];

/// The line work over one drawn raster.
pub struct Lines<'a> {
    pub ras: &'a mut Raster,
    /// Per pixel, what laid it.
    pub marks: Vec<Mark>,
    /// Per pixel, the shaded ink before any line was drawn over it.
    fill: Vec<Ink>,
    /// Each part's ramp, by part id (0 unused).
    ramps: &'a [Ramp],
    /// Toward the light in the picture's plane, of unit length.
    light: [f32; 2],
}

impl<'a> Lines<'a> {
    /// Line work over `ras`, whose inked pixels are all fill, its parts'
    /// ramps by id and the recipe's light.
    pub fn new(ras: &'a mut Raster, ramps: &'a [Ramp], light: [f32; 3]) -> Lines<'a> {
        let marks = ras.ink.iter().map(|i| if i.is_clear() { Mark::Clear } else { Mark::Fill }).collect();
        let fill = ras.ink.clone();
        let l = (light[0] * light[0] + light[1] * light[1]).sqrt();
        let light = if l > 1e-6 { [light[0] / l, light[1] / l] } else { [0.0, -1.0] };
        Lines { ras, marks, fill, ramps, light }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as u32) < self.ras.w && (y as u32) < self.ras.h).then(|| (y as u32 * self.ras.w + x as u32) as usize)
    }

    fn mark(&self, x: i32, y: i32) -> Mark {
        self.idx(x, y).map_or(Mark::Clear, |i| self.marks[i])
    }

    fn ramp(&self, part: PartId) -> Option<&Ramp> {
        self.ramps.get(part as usize)
    }

    /// How squarely the way out from `(x, y)`, judged by which neighbours
    /// are clear, faces the light (-1 to 1).
    fn facing(&self, x: i32, y: i32, out_is: impl Fn(Mark) -> bool) -> f32 {
        let (mut ox, mut oy) = (0.0f32, 0.0f32);
        for (dx, dy) in N8 {
            if out_is(self.mark(x + dx, y + dy)) {
                let l = ((dx * dx + dy * dy) as f32).sqrt();
                ox += dx as f32 / l;
                oy += dy as f32 / l;
            }
        }
        let l = (ox * ox + oy * oy).sqrt();
        if l < 1e-6 { 0.0 } else { (ox * self.light[0] + oy * self.light[1]) / l }
    }

    /// Inner lines where a nearer part covers a farther one by more than
    /// `brk` in depth. `lines(part)` says whether a part draws them.
    pub fn inner(&mut self, brk: f32, lines: impl Fn(PartId) -> bool) {
        let (w, h) = (self.ras.w as i32, self.ras.h as i32);
        let mut put = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if self.marks[i] != Mark::Fill {
                    continue;
                }
                let (part, depth) = (self.ras.part[i], self.ras.depth[i]);
                let mut best: Option<(f32, PartId)> = None;
                for (dx, dy) in N4 {
                    let Some(j) = self.idx(x + dx, y + dy) else { continue };
                    let (q, qd) = (self.ras.part[j], self.ras.depth[j]);
                    if self.marks[j] != Mark::Fill || q == part || qd - depth <= brk || !lines(q) {
                        continue;
                    }
                    if best.is_none_or(|(bd, _)| qd > bd) {
                        best = Some((qd, q));
                    }
                }
                if let Some((_, q)) = best
                    && let Some(r) = self.ramp(q)
                {
                    put.push((i, r.darkest()));
                }
            }
        }
        for (i, ink) in put {
            self.ras.ink[i] = ink;
            self.marks[i] = Mark::Inner;
        }
    }

    /// The silhouette's line outside the figure (grain 2 and up).
    pub fn outline(&mut self, ink: Ink, selout: u8) {
        let (w, h) = (self.ras.w as i32, self.ras.h as i32);
        let mut put = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if self.marks[i] != Mark::Clear {
                    continue;
                }
                // The nearest figure pixel beside it.
                let mut near: Option<usize> = None;
                for (dx, dy) in N4 {
                    if let Some(j) = self.idx(x + dx, y + dy)
                        && self.marks[j] != Mark::Clear
                        && near.is_none_or(|n| self.ras.depth[j] > self.ras.depth[n])
                    {
                        near = Some(j);
                    }
                }
                let Some(j) = near else { continue };
                let facing = self.facing(x, y, |m| m != Mark::Clear);
                // `facing` above is toward the figure; the line faces away from it.
                let lit = -facing > 0.3;
                let line = if lit && selout > 0 {
                    match self.ramp(self.ras.part[j]) {
                        Some(r) => match r.0.iter().position(|&k| k == self.ras.ink[j]) {
                            Some(p) if p >= selout as usize => r.0[p - selout as usize],
                            Some(_) => ink,
                            None => ink,
                        },
                        None => ink,
                    }
                } else {
                    ink
                };
                put.push((i, line, self.ras.depth[j]));
            }
        }
        for (i, line, depth) in put {
            self.ras.ink[i] = line;
            self.ras.depth[i] = depth;
            self.ras.part[i] = LINE;
            self.marks[i] = Mark::Outline;
        }
    }

    /// One-pixel gaps between parts of the figure closed (grain 1): a clear
    /// pixel with the figure on both sides of it, left and right or above and
    /// below, takes the second-darkest tone of the farther side's ramp, as a
    /// seam.
    pub fn close(&mut self) {
        let (w, h) = (self.ras.w as i32, self.ras.h as i32);
        let mut put = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if self.marks[i] != Mark::Clear {
                    continue;
                }
                let pair = |a: (i32, i32), b: (i32, i32)| match (self.idx(x + a.0, y + a.1), self.idx(x + b.0, y + b.1)) {
                    (Some(p), Some(q)) if self.marks[p] != Mark::Clear && self.marks[q] != Mark::Clear => {
                        Some(if self.ras.depth[p] <= self.ras.depth[q] { p } else { q })
                    }
                    _ => None,
                };
                let Some(far) = pair((-1, 0), (1, 0)).or_else(|| pair((0, -1), (0, 1))) else { continue };
                let part = self.ras.part[far];
                let Some(r) = self.ramp(part) else { continue };
                put.push((i, r.0[1.min(r.0.len() - 1)], self.ras.depth[far], part));
            }
        }
        for (i, ink, depth, part) in put {
            self.ras.ink[i] = ink;
            self.ras.depth[i] = depth;
            self.ras.part[i] = part;
            self.fill[i] = ink;
            self.marks[i] = Mark::Edge;
        }
    }

    /// The silhouette's line on the figure's own edge, away from the light
    /// (grain 1).
    pub fn edge(&mut self) {
        let (w, h) = (self.ras.w as i32, self.ras.h as i32);
        let mut put = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if self.marks[i] == Mark::Clear || !N4.iter().any(|&(dx, dy)| self.mark(x + dx, y + dy) == Mark::Clear) {
                    continue;
                }
                if self.facing(x, y, |m| m == Mark::Clear) >= -0.2 {
                    continue;
                }
                let Some(r) = self.ramp(self.ras.part[i]) else { continue };
                let dark = r.0[1.min(r.0.len() - 1)];
                let now = r.0.iter().position(|&k| k == self.ras.ink[i]);
                if now.is_none_or(|p| p > 1) {
                    put.push((i, dark));
                }
            }
        }
        for (i, ink) in put {
            self.ras.ink[i] = ink;
            self.marks[i] = Mark::Edge;
        }
    }

    /// Lone pixels and doubled L-corners.
    pub fn tidy(&mut self, lone: bool) {
        let (w, h) = (self.ras.w as i32, self.ras.h as i32);
        if lone {
            let mut put = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) as usize;
                    if self.marks[i] != Mark::Fill {
                        continue;
                    }
                    let (ink, part) = (self.ras.ink[i], self.ras.part[i]);
                    let mut votes: Vec<(Ink, u8)> = Vec::new();
                    let mut lone = true;
                    for (dx, dy) in N4 {
                        let Some(j) = self.idx(x + dx, y + dy) else {
                            lone = false;
                            break;
                        };
                        if self.marks[j] != Mark::Fill || self.ras.part[j] != part || self.ras.ink[j] == ink {
                            lone = false;
                            break;
                        }
                        match votes.iter_mut().find(|(k, _)| *k == self.ras.ink[j]) {
                            Some(v) => v.1 += 1,
                            None => votes.push((self.ras.ink[j], 1)),
                        }
                    }
                    if lone && let Some(&(k, _)) = votes.iter().max_by_key(|v| v.1) {
                        put.push((i, k));
                    }
                }
            }
            for (i, k) in put {
                self.ras.ink[i] = k;
                self.fill[i] = k;
            }
        }
        // Doubled L-corners of lines drawn on the figure.
        let mut give_back = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let m = self.marks[i];
                if !matches!(m, Mark::Inner | Mark::Edge) {
                    continue;
                }
                let same = |dx: i32, dy: i32| self.mark(x + dx, y + dy) == m;
                let around = N8.iter().filter(|&&(dx, dy)| same(dx, dy)).count();
                let horiz = same(1, 0) || same(-1, 0);
                let vert = same(0, 1) || same(0, -1);
                if around == 2 && horiz && vert {
                    give_back.push(i);
                }
            }
        }
        for i in give_back {
            self.ras.ink[i] = self.fill[i];
            self.marks[i] = Mark::Fill;
        }
    }
}
