//! The realm's one ground (docs/REALM-MONOLITH.md, "One continuous ground"):
//! a height field over the whole kingdom, a corner every half tile, made
//! once from the map and cut into each chunk's `ground` statement. Every
//! corner's height and tone is one function of the map, never of a chunk,
//! so two neighbours give their shared edge the same numbers and the ground
//! runs on with no seam.
//!
//! - **Where knights walk it is the floor.** A corner touching a tile a
//!   knight walks, or one a building stands on, is at height 0: the knights
//!   stand as cards on the floor, and what lies flat on it (walkers, signs,
//!   shots) is drawn there, so the walked ground never rises over their feet.
//! - **The rest rises** with the distance from the walked ground: gentle
//!   swells under the woods, the Highlands' ridges, the downs, the Wastes'
//!   shelves, the rim of the map. How high is each biome's own (`relief`),
//!   eased across chunk borders, with value noise for the swells.
//! - **Water and lava are lowered** into it: the shore at the floor, the bed
//!   deepening away from the bank, the surface (`WATER_Y`) between, so the
//!   banks slope down to it. A road over water is a bridge: its tiles are
//!   water under the deck.
//! - **Tones** are each tile's ground (grass, a road the brightest, a path,
//!   sand, mud, the dark floor of a wood), averaged at the corners they
//!   share, so the road is a soft-edged bright ribbon; crop fields are
//!   striped along their furrows.

use std::sync::OnceLock;

use super::chunks::{CHUNK, Cover, Land};
use crate::render::overworld::ink::{hash, vnoise};
use crate::world::biome::{self, Biome};
use angel_stage::look::smoothstep;
use angel_stage::set::Kind;

/// Corners a tile is cut into on a side.
pub const PER_TILE: i32 = 2;
/// Kingdom pixels between corners.
pub const STEP: f32 = 16.0 / PER_TILE as f32;
/// The water's surface, under the floor.
pub const WATER_Y: f32 = -5.0;
/// Lava's surface.
pub const LAVA_Y: f32 = -3.0;

/// What a tile is to the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Walked on, or built on: the floor.
    Open,
    /// Water, or a road over it (a bridge's deck stands over it).
    Water,
    Lava,
    /// Woods, rock, ash, the rim: ground that rises.
    Raised,
}

/// The kingdom's ground, made once.
pub struct Terrain {
    /// Corners across and down: `cols * PER_TILE + 1` by `rows * PER_TILE + 1`.
    pub w: usize,
    pub h: usize,
    pub heights: Vec<f32>,
    pub tones: Vec<u8>,
    /// Each tile's class, row by row.
    class: Vec<Class>,
    /// For each tile, how many tiles straight north of it are not walked on
    /// (to `NORTH_MAX`): how tall what stands on it may be before it hides
    /// the ground a knight walks behind it.
    north: Vec<u8>,
    /// For each raised corner, how far it is from ground that is not
    /// (kingdom pixels): woods and rock rise toward their middles.
    inland: Vec<f32>,
    cols: i32,
    rows: i32,
}

/// The most tiles `Terrain::north_clear` counts.
pub const NORTH_MAX: u8 = 15;

impl Terrain {
    /// The ground of `land`.
    pub fn of(land: &Land) -> Terrain {
        let (cols, rows) = (land.cols, land.rows);
        let class: Vec<Class> = (0..rows)
            .flat_map(|y| (0..cols).map(move |x| (x, y)))
            .map(|(x, y)| class_of(land, x, y))
            .collect();
        let mut north = vec![0u8; class.len()];
        for x in 0..cols {
            let mut run = 0u8;
            for y in 0..rows {
                north[(y * cols + x) as usize] = run;
                run = if land.walkable(x, y) { 0 } else { (run + 1).min(NORTH_MAX) };
            }
        }
        let tile_tone: Vec<f32> = (0..rows)
            .flat_map(|y| (0..cols).map(move |x| (x, y)))
            .map(|(x, y)| tile_tone(land, x, y))
            .collect();
        let (w, h) = ((cols * PER_TILE + 1) as usize, (rows * PER_TILE + 1) as usize);
        let tile = |x: i32, y: i32| -> Class {
            if x < 0 || y < 0 || x >= cols || y >= rows { Class::Raised } else { class[(y * cols + x) as usize] }
        };
        // The tiles round a corner: one or two across, one or two down.
        let span = |i: i32| if i % PER_TILE == 0 { (i / PER_TILE - 1, i / PER_TILE) } else { (i / PER_TILE, i / PER_TILE) };
        #[derive(Clone, Copy, PartialEq)]
        enum Corner {
            Floor,
            Shore,
            Wet(bool),
            Raised,
        }
        let mut corner = vec![Corner::Raised; w * h];
        for j in 0..h as i32 {
            let (y0, y1) = span(j);
            for i in 0..w as i32 {
                let (x0, x1) = span(i);
                let round = [tile(x0, y0), tile(x1, y0), tile(x0, y1), tile(x1, y1)];
                let wet = |c: &Class| matches!(c, Class::Water | Class::Lava);
                corner[j as usize * w + i as usize] = if round.contains(&Class::Open) {
                    Corner::Floor
                } else if round.iter().all(wet) {
                    Corner::Wet(round.contains(&Class::Lava) && !round.contains(&Class::Water))
                } else if round.iter().any(wet) {
                    Corner::Shore
                } else {
                    Corner::Raised
                };
            }
        }
        // How far each raised corner is from the floor or the shore, and each
        // wet one from its bank: a chamfer distance over the corners.
        let inland = chamfer(w, h, |k| matches!(corner[k], Corner::Raised));
        let offshore = chamfer(w, h, |k| matches!(corner[k], Corner::Wet(_)));
        let mut heights = vec![0.0f32; w * h];
        let mut tones = vec![0u8; w * h];
        for j in 0..h {
            for i in 0..w {
                let k = j * w + i;
                let (x, z) = (i as f32 * STEP, j as f32 * STEP);
                heights[k] = match corner[k] {
                    Corner::Floor | Corner::Shore => 0.0,
                    Corner::Wet(false) => -(6.0 + 10.0 * smoothstep(0.0, 24.0, offshore[k])),
                    Corner::Wet(true) => -(4.0 + 4.0 * smoothstep(0.0, 12.0, offshore[k])),
                    Corner::Raised => rise(x, z, inland[k]),
                };
                // The tone: the mean of the tiles round it, a little noise
                // on the open ground, the furrows of a crop field.
                let (x0, x1) = span(i as i32);
                let (y0, y1) = span(j as i32);
                let mut sum = 0.0;
                let mut n = 0.0;
                for (tx, ty) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                    let t = if tx < 0 || ty < 0 || tx >= cols || ty >= rows { RIM_TONE } else { tile_tone[(ty * cols + tx) as usize] };
                    sum += t;
                    n += 1.0;
                }
                let mut t = sum / n;
                if matches!(corner[k], Corner::Floor) {
                    t += 1.5 * (vnoise(x / 40.0, z / 40.0, 907) * 2.0 - 1.0);
                }
                let crop = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
                    .iter()
                    .all(|&(tx, ty)| land.ground(tx, ty).0 == Kind::Crop && land.walkable(tx, ty));
                if crop {
                    t += if j % 2 == 1 { 4.0 } else { -3.0 };
                }
                tones[k] = t.round().clamp(0.0, 100.0) as u8;
            }
        }
        Terrain { w, h, heights, tones, class, north, inland, cols, rows }
    }

    /// Corner `(i, j)`'s height (clamped into the field).
    pub fn corner(&self, i: i32, j: i32) -> f32 {
        let (i, j) = (i.clamp(0, self.w as i32 - 1) as usize, j.clamp(0, self.h as i32 - 1) as usize);
        self.heights[j * self.w + i]
    }

    /// The ground's height at kingdom point `(x, z)`, between its corners.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let (fx, fz) = (x / STEP, z / STEP);
        let (i, j) = (fx.floor() as i32, fz.floor() as i32);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let a = self.corner(i, j) + (self.corner(i + 1, j) - self.corner(i, j)) * tx;
        let b = self.corner(i, j + 1) + (self.corner(i + 1, j + 1) - self.corner(i, j + 1)) * tx;
        a + (b - a) * tz
    }

    /// The lowest the ground is over the box `[x0, z0, x1, z1]` (its
    /// corners and middle): where to start something standing on it so its
    /// foot is never in the air.
    pub fn lowest(&self, [x0, z0, x1, z1]: [f32; 4]) -> f32 {
        let mut low = f32::INFINITY;
        let n = 3;
        for b in 0..=n {
            for a in 0..=n {
                let x = x0 + (x1 - x0) * a as f32 / n as f32;
                let z = z0 + (z1 - z0) * b as f32 / n as f32;
                low = low.min(self.height_at(x, z));
            }
        }
        low
    }

    /// Tile `(x, y)`'s class (off the map: raised).
    pub fn class(&self, x: i32, y: i32) -> Class {
        if x < 0 || y < 0 || x >= self.cols || y >= self.rows { Class::Raised } else { self.class[(y * self.cols + x) as usize] }
    }

    /// How many tiles straight north of tile `(x, y)` no knight walks.
    pub fn north_clear(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.cols || y >= self.rows { 0 } else { self.north[(y * self.cols + x) as usize] }
    }

    /// How far kingdom point `(x, z)` is into its raised ground (0 off it).
    pub fn inland(&self, x: f32, z: f32) -> f32 {
        let (i, j) = ((x / STEP).round() as i32, (z / STEP).round() as i32);
        let (i, j) = (i.clamp(0, self.w as i32 - 1) as usize, j.clamp(0, self.h as i32 - 1) as usize);
        self.inland[j * self.w + i]
    }

    /// Chunk `(cx, cz)`'s `ground` statement: its corners, the shared edges
    /// included, as every chunk cuts the one field.
    pub fn statement(&self, cx: i32, cz: i32) -> String {
        use std::fmt::Write as _;
        let (ci, cj) = ((cx * CHUNK.0 * PER_TILE) as usize, (cz * CHUNK.1 * PER_TILE) as usize);
        let (cols, rows) = ((CHUNK.0 * PER_TILE) as usize, (CHUNK.1 * PER_TILE) as usize);
        let mut out = format!(
            "ground {cols} {rows} step {} at {} {} heights",
            STEP,
            ci as f32 * STEP,
            cj as f32 * STEP
        );
        for j in cj..=cj + rows {
            for i in ci..=ci + cols {
                let v = self.heights[j.min(self.h - 1) * self.w + i.min(self.w - 1)];
                let r = (v * 10.0).round() / 10.0;
                let _ = if r == r.trunc() { write!(out, " {}", r as i32) } else { write!(out, " {r:.1}") };
            }
        }
        out.push_str(" tones");
        for j in cj..=cj + rows {
            for i in ci..=ci + cols {
                let _ = write!(out, " {}", self.tones[j.min(self.h - 1) * self.w + i.min(self.w - 1)]);
            }
        }
        out
    }
}

/// The tone off the map's edge.
const RIM_TONE: f32 = 12.0;

fn class_of(land: &Land, x: i32, y: i32) -> Class {
    match land.cover(x, y) {
        Cover::Water | Cover::Bridge => Class::Water,
        Cover::Lava => Class::Lava,
        Cover::Open => Class::Open,
        _ if land.walkable(x, y) || land.built(x, y).is_some() => Class::Open,
        _ => Class::Raised,
    }
}

/// A tile's ground tone, 0 to 100: its ground's brightness before the light.
fn tile_tone(land: &Land, x: i32, y: i32) -> f32 {
    match land.cover(x, y) {
        Cover::Water | Cover::Bridge => return 9.0,
        Cover::Lava => return 8.0,
        Cover::Woods => return 12.0,
        Cover::DeadTree => return 17.0,
        Cover::Rock => return 25.0,
        Cover::Hedge => return RIM_TONE,
        Cover::Open => {}
    }
    match land.ground(x, y).0 {
        Kind::Road => 50.0,
        Kind::Path | Kind::Wood => 41.0,
        Kind::Sand => 38.0,
        Kind::Mud => 15.0,
        Kind::Crop => 28.0,
        Kind::Ground => 24.0,
        Kind::Rock => 25.0,
        _ => 22.0,
    }
}

/// How high a biome's raised ground rises, kingdom pixels.
pub fn relief(b: Biome) -> f32 {
    match b {
        Biome::Greenwood => 16.0,
        Biome::Highlands => 38.0,
        Biome::Wastes => 22.0,
        Biome::Coast => 10.0,
        Biome::Fen => 4.0,
        Biome::CastleTown => 6.0,
        Biome::TiltYards => 6.0,
        Biome::Fairground => 6.0,
        Biome::WindCountry => 18.0,
        Biome::Longbow => 10.0,
        Biome::March => 14.0,
    }
}

/// The relief at kingdom point `(x, z)`, eased between the chunks'
/// middles so a biome's border is no cliff.
fn relief_at(x: f32, z: f32) -> f32 {
    let (cw, ch) = ((CHUNK.0 * 16) as f32, (CHUNK.1 * 16) as f32);
    let (fx, fz) = (x / cw - 0.5, z / ch - 0.5);
    let (i, j) = (fx.floor() as i32, fz.floor() as i32);
    let (tx, tz) = (smoothstep(0.0, 1.0, fx - i as f32), smoothstep(0.0, 1.0, fz - j as f32));
    let r = |a: i32, b: i32| relief(biome::chunk(a, b));
    let top = r(i, j) + (r(i + 1, j) - r(i, j)) * tx;
    let bottom = r(i, j + 1) + (r(i + 1, j + 1) - r(i, j + 1)) * tx;
    top + (bottom - top) * tz
}

/// Raised ground's height at `(x, z)`, `inland` pixels from the floor.
fn rise(x: f32, z: f32, inland: f32) -> f32 {
    let r = relief_at(x, z);
    let swell = 0.55 + 0.9 * vnoise(x / 70.0, z / 70.0, 911);
    let h = r * swell * smoothstep(0.0, 44.0, inland) + 0.25 * r * smoothstep(40.0, 140.0, inland);
    // The Wastes' shelves: the rise in flat steps.
    if biome::at((x / 16.0) as i32, (z / 16.0) as i32) == Biome::Wastes && h > 4.0 {
        let step = 7.0;
        let k = (h / step).floor();
        let f = h / step - k;
        return step * (k + smoothstep(0.7, 1.0, f));
    }
    h
}

/// The chamfer distance (kingdom pixels) from every corner `inside` says to
/// the nearest corner it does not, 0 for those; two passes, a corner's
/// eight neighbours.
fn chamfer(w: usize, h: usize, inside: impl Fn(usize) -> bool) -> Vec<f32> {
    let far = 1.0e6;
    let mut d: Vec<f32> = (0..w * h).map(|k| if inside(k) { far } else { 0.0 }).collect();
    let (a, b) = (STEP, STEP * std::f32::consts::SQRT_2);
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            if d[k] == 0.0 {
                continue;
            }
            let mut v = d[k];
            if i > 0 {
                v = v.min(d[k - 1] + a);
            }
            if j > 0 {
                v = v.min(d[k - w] + a);
                if i > 0 {
                    v = v.min(d[k - w - 1] + b);
                }
                if i + 1 < w {
                    v = v.min(d[k - w + 1] + b);
                }
            }
            d[k] = v;
        }
    }
    for j in (0..h).rev() {
        for i in (0..w).rev() {
            let k = j * w + i;
            if d[k] == 0.0 {
                continue;
            }
            let mut v = d[k];
            if i + 1 < w {
                v = v.min(d[k + 1] + a);
            }
            if j + 1 < h {
                v = v.min(d[k + w] + a);
                if i + 1 < w {
                    v = v.min(d[k + w + 1] + b);
                }
                if i > 0 {
                    v = v.min(d[k + w - 1] + b);
                }
            }
            d[k] = v;
        }
    }
    d
}

/// The kingdom's ground (the map with no wishes), made once.
pub fn terrain() -> &'static Terrain {
    static TERRAIN: OnceLock<Terrain> = OnceLock::new();
    TERRAIN.get_or_init(|| Terrain::of(super::kingdom::land()))
}

/// A number in 0..1 from a tile and a salt, the same every time.
pub fn unit(x: i32, y: i32, salt: u32) -> f32 {
    (hash(x, y, salt) & 0xffff) as f32 / 65535.0
}
