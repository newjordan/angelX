//! Wilderness: the screens between the authored places.
//!
//! The realm spreads its twelve authored screens across a wider grid, and
//! every screen between them is grown here. Terrain comes from noise in
//! world coordinates, so neighbouring wild screens meet seamlessly. Along
//! an authored neighbour a wild screen continues that neighbour's edge — a
//! coastline, a cliff, a tree line — a few tiles in, and a road runs from
//! each of the neighbour's exits to a junction, carried over water on
//! planks. Pure: the same screen always grows the same way.

use super::ink::{hash, vnoise};
use super::map::{SCREEN_H, SCREEN_W};

const W: usize = SCREEN_W as usize;
const H: usize = SCREEN_H as usize;

pub(crate) type Rows = [[u8; W]; H];

/// An authored neighbour's tiles along the shared border, if there is one.
#[derive(Default)]
pub(crate) struct Edges {
    pub(crate) north: Option<[u8; W]>,
    pub(crate) south: Option<[u8; W]>,
    pub(crate) west: Option<[u8; H]>,
    pub(crate) east: Option<[u8; H]>,
}

/// What a wild screen leans toward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lean {
    Forest,
    Meadow,
    Hills,
    Ash,
    Marsh,
}

fn road_like(t: u8) -> bool {
    matches!(t, b'=' | b':' | b'H')
}

/// Tiles that carry across a border into the wild: land marks strong
/// enough to read as the same coast, cliff or wood.
fn carries(t: u8) -> bool {
    matches!(t, b'~' | b's' | b'^' | b'T' | b'%' | b'a' | b'd')
}

/// Grow wild screen `(sx, sy)`.
pub(crate) fn grow(sx: i32, sy: i32, edges: &Edges, lean: Lean) -> Rows {
    let mut rows: Rows = [[b'.'; W]; H];
    for (y, row) in rows.iter_mut().enumerate() {
        for (x, t) in row.iter_mut().enumerate() {
            let (wx, wy) = (sx * SCREEN_W + x as i32, sy * SCREEN_H + y as i32);
            *t = terrain(wx, wy, lean);
        }
    }
    shore(&mut rows);
    bleed(&mut rows, edges, sx, sy);
    watercourse(&mut rows, sx, sy, lean);
    shore(&mut rows);
    roads(&mut rows, edges, sx, sy);
    rows
}

/// A stream from a spring high in the screen down to a pool: a fall where
/// it leaves the rock, a run through the open, a pool where it settles. In
/// the ash wastes the same course runs with lava.
fn watercourse(rows: &mut Rows, sx: i32, sy: i32, lean: Lean) {
    let h = hash(sx, sy, 431);
    let molten = lean == Lean::Ash;
    let flows = match lean {
        Lean::Hills | Lean::Ash => true,
        Lean::Forest => h % 2 == 0,
        Lean::Meadow | Lean::Marsh => false,
    };
    if !flows {
        return;
    }
    let (run, drop, pool) = if molten {
        (b'L', b'F', b'L')
    } else {
        (b'w', b'f', b'~')
    };
    // Rise among the rocks near the top when there are any.
    let mut x = (3 + (h % 10) as usize).min(W - 4);
    for dx in 0..10 {
        let cand = 2 + ((h as usize >> 4) + dx) % (W - 4);
        if rows[1][cand] == b'^' || rows[2][cand] == b'^' {
            x = cand;
            break;
        }
    }
    let end = 6 + ((h >> 9) % 2) as usize;
    for y in 1..=end {
        rows[y][x] = if rows[y][x] == b'^' { drop } else { run };
        if y < end && y >= 3 {
            let step = hash(sx * 31 + x as i32, sy * 17 + y as i32, 432) % 4;
            let nx = match step {
                0 if x > 2 => x - 1,
                1 if x < W - 3 => x + 1,
                _ => x,
            };
            if nx != x {
                rows[y][nx] = if rows[y][nx] == b'^' { drop } else { run };
                x = nx;
            }
        }
    }
    // A round, ragged pool where the course settles.
    let (cx, cy) = (x as f32, end as f32 + 2.0);
    for py in end + 1..H - 1 {
        for px in 1..W - 1 {
            let (dx, dy) = ((px as f32 - cx) / 2.3, (py as f32 - cy) / 1.6);
            let ragged = (hash(px as i32 + sx * 16, py as i32 + sy * 11, 433) % 100) as f32 / 250.0;
            if dx * dx + dy * dy <= 1.0 + ragged {
                rows[py][px] = pool;
            }
        }
    }
}

fn terrain(wx: i32, wy: i32, lean: Lean) -> u8 {
    let wood = vnoise(wx as f32 / 4.5, wy as f32 / 3.5, 401);
    let hill = vnoise(wx as f32 / 6.0, wy as f32 / 4.5, 402);
    let lake = vnoise(wx as f32 / 8.0, wy as f32 / 6.0, 403);
    let h = hash(wx, wy, 404);
    match lean {
        Lean::Forest => {
            if lake > 0.8 {
                b'~'
            } else if wood > 0.5 {
                b'T'
            } else {
                b'.'
            }
        }
        Lean::Meadow => {
            if lake > 0.82 {
                b'~'
            } else if wood > 0.7 {
                b'T'
            } else if hill > 0.84 {
                b'^'
            } else {
                b'.'
            }
        }
        Lean::Hills => {
            if hill > 0.48 {
                b'^'
            } else if wood > 0.74 {
                b'T'
            } else {
                b'.'
            }
        }
        Lean::Ash => {
            if lake > 0.8 {
                b'L'
            } else if hill > 0.55 {
                b'^'
            } else if h % 17 == 0 {
                b'd'
            } else {
                b'a'
            }
        }
        Lean::Marsh => {
            if lake > 0.64 {
                b'~'
            } else if wood > 0.78 {
                b'T'
            } else if h % 23 == 0 {
                b'd'
            } else {
                b'%'
            }
        }
    }
}

/// Meadow touching open water becomes shore.
fn shore(rows: &mut Rows) {
    let src = *rows;
    for y in 0..H {
        for x in 0..W {
            if src[y][x] != b'.' && src[y][x] != b'a' {
                continue;
            }
            let wet = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    nx >= 0
                        && ny >= 0
                        && (nx as usize) < W
                        && (ny as usize) < H
                        && matches!(src[ny as usize][nx as usize], b'~' | b'w' | b'L')
                });
            if wet && src[y][x] == b'.' {
                rows[y][x] = b's';
            }
        }
    }
}

/// Continue each authored neighbour's border a few tiles in: the facing
/// row or column copies it, and strong marks reach further with fading odds.
fn bleed(rows: &mut Rows, edges: &Edges, sx: i32, sy: i32) {
    let reach = |depth: usize, a: i32, b: i32| -> bool {
        match depth {
            0 => true,
            1 => hash(a, b, 411) % 10 < 7,
            2 => hash(a, b, 412) % 10 < 3,
            _ => false,
        }
    };
    let mut put = |x: usize, y: usize, t: u8, depth: usize| {
        if depth == 0 {
            rows[y][x] = if road_like(t) { b'=' } else { calm(t) };
        } else if carries(t) && reach(depth, sx * 97 + x as i32, sy * 89 + y as i32) {
            rows[y][x] = t;
        }
    };
    for depth in 0..3 {
        if let Some(edge) = edges.north {
            for (x, &t) in edge.iter().enumerate() {
                put(x, depth, t, depth);
            }
        }
        if let Some(edge) = edges.south {
            for (x, &t) in edge.iter().enumerate() {
                put(x, H - 1 - depth, t, depth);
            }
        }
        if let Some(edge) = edges.west {
            for (y, &t) in edge.iter().enumerate() {
                put(depth, y, t, depth);
            }
        }
        if let Some(edge) = edges.east {
            for (y, &t) in edge.iter().enumerate() {
                put(W - 1 - depth, y, t, depth);
            }
        }
    }
}

/// A neighbour's building-ground tiles read as open meadow on this side.
fn calm(t: u8) -> u8 {
    match t {
        b'c' => b'.',
        other => other,
    }
}

/// Roads from every authored exit to one junction.
fn roads(rows: &mut Rows, edges: &Edges, sx: i32, sy: i32) {
    let h = hash(sx, sy, 421);
    let (jx, jy) = (6 + (h % 4) as usize, 3 + ((h >> 4) % 4) as usize);
    let mut lay = |x: usize, y: usize| {
        rows[y][x] = if matches!(rows[y][x], b'~' | b'w' | b'f' | b'L' | b'F') {
            b'H'
        } else {
            b'='
        };
    };
    let exits = |edge: Option<&[u8]>| -> Vec<usize> {
        edge.map_or_else(Vec::new, |e| {
            (0..e.len()).filter(|&i| road_like(e[i])).collect()
        })
    };
    for x in exits(edges.north.as_ref().map(|e| &e[..])) {
        for y in 0..=jy {
            lay(x, y);
        }
        for xx in x.min(jx)..=x.max(jx) {
            lay(xx, jy);
        }
    }
    for x in exits(edges.south.as_ref().map(|e| &e[..])) {
        for y in jy..H {
            lay(x, y);
        }
        for xx in x.min(jx)..=x.max(jx) {
            lay(xx, jy);
        }
    }
    for y in exits(edges.west.as_ref().map(|e| &e[..])) {
        for x in 0..=jx {
            lay(x, y);
        }
        for yy in y.min(jy)..=y.max(jy) {
            lay(jx, yy);
        }
    }
    for y in exits(edges.east.as_ref().map(|e| &e[..])) {
        for x in jx..W {
            lay(x, y);
        }
        for yy in y.min(jy)..=y.max(jy) {
            lay(jx, yy);
        }
    }
}
