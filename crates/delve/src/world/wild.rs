//! Wilderness: the ground between the set pieces, grown biome by biome.
//!
//! Each biome lays its own ground (meadow, ash, marsh) and grows features
//! in clumps on a jittered grid: groves, outcrops, meres, crop strips, lava
//! pools, never a carpet, so open greens stay open; streams and falls run
//! down the Highlands, lava down the Wastes. Nothing grows within
//! two tiles of a road or a tile of a set piece. Along the chunk edges
//! where two biomes meet runs one natural border: the Kingsriver and the
//! Marchwater, ridges, streams and hedgerows, gapped where roads cross and
//! here and there for a walker. The sea lies along the east rim and the
//! estuary along the Fen's. Pure: the same realm always grows the same way.

use super::biome::{self, Biome};
use super::map::{CHUNK_H, CHUNK_W, CHUNKS_X, CHUNKS_Y, MAP_H, MAP_W, SCREEN_H, SCREEN_W, set_pieces};
use crate::render::overworld::ink::{hash, vnoise};

const N: usize = (MAP_W * MAP_H) as usize;

fn idx(x: i32, y: i32) -> usize {
    (y * MAP_W + x) as usize
}

fn inside(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < MAP_W && y < MAP_H
}

/// One kind of clump a biome grows: its tile, its chance per thousand
/// grid cells, and its half-widths across and down.
struct Clump {
    tile: u8,
    chance: u32,
    rx: (i32, i32),
    ry: (i32, i32),
}

const fn clump(tile: u8, chance: u32, rx: (i32, i32), ry: (i32, i32)) -> Clump {
    Clump { tile, chance, rx, ry }
}

/// What each biome grows, by grid cell of 8x6 tiles, in `Biome::ALL` order.
const CLUMPS: [&[Clump]; 11] = [
    // Castle Town: ornamental clumps and garden plots on the greens.
    &[clump(b'T', 420, (1, 2), (1, 1)), clump(b'c', 140, (2, 3), (1, 1))],
    // Greenwood: groves with glades between.
    &[clump(b'T', 760, (2, 4), (1, 3)), clump(b'~', 40, (1, 2), (1, 1))],
    // The Highlands: outcrops and pines, a tarn.
    &[clump(b'^', 280, (1, 3), (1, 2)), clump(b'T', 260, (1, 2), (1, 2)), clump(b'~', 30, (1, 2), (1, 1))],
    // The Wastes: scree, lava pools, dead trees.
    &[clump(b'^', 220, (1, 3), (1, 2)), clump(b'L', 80, (1, 2), (1, 1)), clump(b'd', 300, (2, 3), (1, 2))],
    // The Coast: dunes, a few pines and rocks.
    &[clump(b'T', 150, (1, 2), (1, 2)), clump(b'^', 120, (1, 2), (1, 1)), clump(b's', 220, (2, 3), (1, 2))],
    // The Fen: meres, reeds, alders.
    &[clump(b'~', 330, (1, 3), (1, 2)), clump(b'T', 120, (1, 2), (1, 1)), clump(b'd', 180, (1, 2), (1, 1))],
    // The Tilt-Yards: open paddocks.
    &[clump(b'T', 300, (1, 2), (1, 2))],
    // The Fairground: one big field.
    &[clump(b'T', 260, (1, 2), (1, 1))],
    // Wind Country: crop strips, a stone on a hilltop.
    &[clump(b'c', 330, (2, 3), (1, 1)), clump(b'T', 160, (1, 2), (1, 1)), clump(b'^', 70, (1, 1), (1, 1))],
    // Longbow Country: fields and copses.
    &[clump(b'c', 250, (2, 3), (1, 1)), clump(b'T', 280, (1, 3), (1, 2))],
    // The March: groves, fields, a few rocks.
    &[clump(b'T', 420, (1, 3), (1, 2)), clump(b'c', 150, (2, 3), (1, 1)), clump(b'^', 80, (1, 2), (1, 1))],
];

fn clumps(b: Biome) -> &'static [Clump] {
    CLUMPS[usize::from(b.index())]
}

/// The ground a biome lays before anything grows on it.
fn ground(b: Biome, x: i32, y: i32) -> u8 {
    match b {
        Biome::Wastes if vnoise(x as f32 / 6.0, y as f32 / 5.0, 476) > 0.42 => b'a',
        Biome::Fen if vnoise(x as f32 / 5.0, y as f32 / 4.0, 471) > 0.56 => b'%',
        _ => b'.',
    }
}

/// How the border between two biomes is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    River,
    Ridge,
    Stream,
    Hedge,
}

/// The border on the chunk edge above chunk `(cx, cy)` (`across`) or left
/// of it, between biomes `a` and `b`.
pub fn edge(a: Biome, b: Biome, across: bool, cx: i32, cy: i32) -> Option<Edge> {
    if a == b {
        return None;
    }
    let either = |k: Biome| a == k || b == k;
    Some(if across && ((cy == 3 && cx <= 7) || cy == 9) {
        Edge::River
    } else if either(Biome::Fen) {
        Edge::Stream
    } else if either(Biome::Highlands)
        || either(Biome::Wastes)
        || (either(Biome::Coast) && either(Biome::TiltYards))
    {
        Edge::Ridge
    } else {
        Edge::Hedge
    })
}

/// Grow the realm's ground around the set pieces and under the roads
/// (`roads` is the laid road width at each tile).
pub fn grow(roads: &[u8]) -> Vec<u8> {
    let near = road_distance(roads, 4);
    let mut keep = vec![false; N];
    // The set pieces, the fixed lots (the Delve's gate, the College) and
    // the castles keep a tile of open ground round them.
    let gate = super::wishes::DELVE_GATE;
    let school = super::college::LOT;
    // Each castle keeps its footprint and the ground before its gate.
    let fixed = set_pieces()
        .map(|(_, (ox, oy))| (ox, oy, SCREEN_W, SCREEN_H))
        .chain([(gate.tx, gate.ty, gate.tw, gate.th), (school.tx, school.ty, school.tw, school.th)])
        .chain(super::houses::footprints().map(|(tx, ty, tw, th)| (tx, ty, tw, th + 2)));
    for (ox, oy, w, h) in fixed {
        for y in oy - 1..oy + h + 1 {
            for x in ox - 1..ox + w + 1 {
                if inside(x, y) {
                    keep[idx(x, y)] = true;
                }
            }
        }
    }
    let mut t = vec![b'.'; N];
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            t[idx(x, y)] = ground(biome::at(x, y), x, y);
        }
    }
    let open = |x: i32, y: i32| inside(x, y) && near[idx(x, y)] > 2 && !keep[idx(x, y)];
    grow_clumps(&mut t, &open);
    watercourses(&mut t, &open);
    rises(&mut t, &near);
    mountains(&mut t, &open);
    borders(&mut t, &near, &keep);
    seas(&mut t);
    t
}

/// Once the set pieces stand in the grown ground: their water and rock
/// run on past their edges, and meadow by water turns to shore.
pub fn finish(t: &mut [u8]) {
    bleed(t);
    shore(t);
}

/// The steps from each tile to the nearest road tile, up to `cap`.
fn road_distance(roads: &[u8], cap: u8) -> Vec<u8> {
    let mut d = vec![cap; N];
    let mut frontier: Vec<(i32, i32)> = Vec::new();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if roads[idx(x, y)] > 0 {
                d[idx(x, y)] = 0;
                frontier.push((x, y));
            }
        }
    }
    for step in 1..cap {
        let mut next = Vec::new();
        for (x, y) in frontier {
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if inside(nx, ny) && d[idx(nx, ny)] > step {
                    d[idx(nx, ny)] = step;
                    next.push((nx, ny));
                }
            }
        }
        frontier = next;
    }
    d
}

fn grow_clumps(t: &mut [u8], open: &impl Fn(i32, i32) -> bool) {
    let (gw, gh) = (8, 6);
    for gy in 0..MAP_H / gh + 1 {
        for gx in 0..MAP_W / gw + 1 {
            let h = hash(gx, gy, 461);
            let (cx, cy) = (gx * gw + (h % gw as u32) as i32, gy * gh + ((h >> 4) % gh as u32) as i32);
            if !inside(cx, cy) {
                continue;
            }
            let home = biome::at(cx, cy);
            let mut roll = (h >> 9) % 1000;
            let Some(c) = clumps(home).iter().find(|c| {
                if roll < c.chance {
                    true
                } else {
                    roll -= c.chance;
                    false
                }
            }) else {
                continue;
            };
            let span = |(lo, hi): (i32, i32), s: u32| lo + (hash(gx, gy, s) % (hi - lo + 1) as u32) as i32;
            let (rx, ry) = (span(c.rx, 462), span(c.ry, 463));
            for dy in -ry..=ry {
                for dx in -rx..=rx {
                    let (x, y) = (cx + dx, cy + dy);
                    if !open(x, y) || biome::at(x, y) != home {
                        continue;
                    }
                    // Crop strips are fields, square-cornered; the rest
                    // are ragged rounds.
                    let fit = c.tile == b'c' || {
                        let (fx, fy) = (dx as f32 / (rx as f32 + 0.5), dy as f32 / (ry as f32 + 0.5));
                        let ragged = (hash(x, y, 464) % 100) as f32 / 300.0;
                        fx * fx + fy * fy <= 0.85 + ragged
                    };
                    // Dead trees stand apart, not in a thicket.
                    let sparse = c.tile != b'd' || hash(x, y, 465).is_multiple_of(3);
                    if fit && sparse {
                        t[idx(x, y)] = c.tile;
                    }
                }
            }
        }
    }
}

/// Streams in the Highlands and lava in the Wastes: from a spring among
/// the rocks a course runs down the chunk, a fall wherever it leaves rock,
/// and settles in a pool. About half the Highlands' chunks have one, a
/// third of the Wastes'.
fn watercourses(t: &mut [u8], open: &impl Fn(i32, i32) -> bool) {
    for cy in 0..CHUNKS_Y {
        for cx in 0..CHUNKS_X {
            let molten = match biome::chunk(cx, cy) {
                Biome::Highlands => false,
                Biome::Wastes => true,
                _ => continue,
            };
            let h = hash(cx, cy, 477);
            if h % 100 >= if molten { 35 } else { 55 } {
                continue;
            }
            let (run, drop, pool) = if molten { (b'L', b'F', b'L') } else { (b'w', b'f', b'~') };
            let mut x = cx * CHUNK_W + 4 + (h >> 8) as i32 % (CHUNK_W - 8);
            let top = cy * CHUNK_H + 2;
            let length = 6 + (h >> 16) as i32 % 4;
            if !open(x, top) || !open(x, top + 1) {
                continue;
            }
            // It springs from a rock face: a fall between two crags.
            for side in [x - 1, x + 1] {
                if open(side, top) {
                    t[idx(side, top)] = b'^';
                }
            }
            t[idx(x, top)] = b'^';
            let mut end = None;
            for y in top..top + length {
                if !open(x, y) {
                    break;
                }
                t[idx(x, y)] = if t[idx(x, y)] == b'^' { drop } else { run };
                end = Some((x, y));
                // Now and then it steps aside, staying joined.
                let side = match hash(x, y, 478) % 4 {
                    0 => -1,
                    1 => 1,
                    _ => 0,
                };
                if side != 0 && y > top + 1 && open(x + side, y) {
                    x += side;
                    t[idx(x, y)] = if t[idx(x, y)] == b'^' { drop } else { run };
                }
            }
            let Some((px, py)) = end else {
                continue;
            };
            for dy in 0..=2 {
                for dx in -2..=2 {
                    let (x, y) = (px + dx, py + 1 + dy);
                    if open(x, y) && (dx * dx) as f32 / 5.0 + ((dy - 1) * (dy - 1)) as f32 / 1.6 <= 1.0 {
                        t[idx(x, y)] = pool;
                    }
                }
            }
        }
    }
}

/// Each castle of the March stands on a rise: a crescent of rock behind it
/// and down its flanks, open toward its road.
fn rises(t: &mut [u8], near: &[u8]) {
    for (tx, ty, tw, _) in super::houses::footprints() {
        for x in tx - 2..tx + tw + 2 {
            for y in [ty - 2, ty - 1] {
                let flank = x == tx - 2 || x == tx + tw + 1;
                let solid = y == ty - 2 || flank || hash(x, y, 466) % 3 != 0;
                if inside(x, y) && near[idx(x, y)] > 0 && solid {
                    t[idx(x, y)] = b'^';
                }
            }
        }
        for x in [tx - 2, tx + tw + 1] {
            if inside(x, ty) && near[idx(x, ty)] > 0 {
                t[idx(x, ty)] = b'^';
            }
        }
    }
}

/// Mountains along the Highlands' and the Wastes' north rim.
fn mountains(t: &mut [u8], open: &impl Fn(i32, i32) -> bool) {
    for x in 0..MAP_W {
        if !matches!(biome::at(x, 0), Biome::Highlands | Biome::Wastes) {
            continue;
        }
        let depth = 1 + (vnoise(x as f32 / 4.0, 0.5, 467) * 3.0) as i32;
        for y in 0..depth {
            if open(x, y) {
                t[idx(x, y)] = b'^';
            }
        }
    }
}

fn borders(t: &mut [u8], near: &[u8], keep: &[bool]) {
    // `across`: the edge along the top of chunk (cx, cy); else its left.
    for cy in 0..CHUNKS_Y {
        for cx in 0..CHUNKS_X {
            for across in [true, false] {
                let (a, b) = if across {
                    if cy == 0 {
                        continue;
                    }
                    (biome::chunk(cx, cy - 1), biome::chunk(cx, cy))
                } else {
                    if cx == 0 {
                        continue;
                    }
                    (biome::chunk(cx - 1, cy), biome::chunk(cx, cy))
                };
                let Some(kind) = edge(a, b, across, cx, cy) else {
                    continue;
                };
                let (len, line) = if across { (CHUNK_W, cy * CHUNK_H) } else { (CHUNK_H, cx * CHUNK_W) };
                let start = if across { cx * CHUNK_W } else { cy * CHUNK_H };
                let at = |along: i32, off: i32| if across { (along, line + off) } else { (line + off, along) };
                let mut last = None;
                for along in start..start + len {
                    // The line wanders up to two tiles off the edge.
                    let wander = vnoise(along as f32 / 9.0, line as f32 / 3.0, 468);
                    let shift = match kind {
                        Edge::River => -((wander * 3.0) as i32).min(2),
                        Edge::Stream => ((wander * 4.0) as i32).min(3) - 2,
                        _ => -((wander * 2.0) as i32).min(1),
                    };
                    let from = last.map_or(shift, |prev: i32| prev.min(shift));
                    let to = last.map_or(shift, |prev: i32| prev.max(shift));
                    let put = |t: &mut [u8], off: i32, tile: u8| {
                        let (x, y) = at(along, off);
                        if inside(x, y) && !keep[idx(x, y)] {
                            t[idx(x, y)] = tile;
                        }
                    };
                    let gap = |off: i32| {
                        let (x, y) = at(along, off);
                        !inside(x, y) || near[idx(x, y)] <= 1
                    };
                    match kind {
                        Edge::River => {
                            // Two tiles of water, joined where it bends.
                            for off in from - 1..=to {
                                put(t, off, b'~');
                            }
                        }
                        Edge::Stream => {
                            for off in from..=to {
                                put(t, off, b'~');
                            }
                        }
                        Edge::Ridge => {
                            // Rock two deep, ragged; a pass at every road
                            // and a notch now and then for a climber.
                            let notch = hash(along / 3, line, 469) % 9 == 0;
                            for off in [shift - 1, shift] {
                                if !gap(off) && !notch {
                                    put(t, off, b'^');
                                }
                            }
                            let extra = hash(along, line, 470) % 3;
                            if extra == 0 && !gap(shift - 2) && !notch {
                                put(t, shift - 2, b'^');
                            } else if extra == 1 && !gap(shift + 1) && !notch {
                                put(t, shift + 1, b'^');
                            }
                        }
                        Edge::Hedge => {
                            // A hedgerow: a line of trees with field gates.
                            if !gap(shift) && hash(along / 2, line, 472) % 5 != 0 {
                                put(t, shift, b'T');
                            }
                        }
                    }
                    last = Some(shift);
                }
            }
        }
    }
    // The Kingsriver rises in a pool at its eastern end.
    let (sx, sy) = (8 * CHUNK_W - 1, 3 * CHUNK_H - 1);
    for y in sy - 2..=sy + 1 {
        for x in sx - 2..=sx + 2 {
            if inside(x, y) && !keep[idx(x, y)] && (x - sx).abs() + (y - sy).abs() <= 3 {
                t[idx(x, y)] = b'~';
            }
        }
    }
}

/// The sea along the east rim and the estuary along the Fen's west rim.
fn seas(t: &mut [u8]) {
    for y in 0..MAP_H {
        let wobble = vnoise(y as f32 / 8.0, 0.5, 473);
        // Out past the Repo Wards, whose quay runs to its own sea.
        let east = if (60..82).contains(&y) { 236 } else { 226 + (wobble * 6.0) as i32 };
        for x in east..MAP_W {
            t[idx(x, y)] = b'~';
        }
        if (44..128).contains(&y) {
            let west = 4 + (vnoise(y as f32 / 6.0, 1.5, 474) * 3.0) as i32;
            for x in 0..west {
                t[idx(x, y)] = b'~';
            }
        }
    }
}

/// A set piece's water and shore run on a tile or two past its edge, so a
/// mere it was drawn with does not stop at a straight line.
fn bleed(t: &mut [u8]) {
    for ((ax, ay), (ox, oy)) in set_pieces() {
        let rows = super::map::Realm::authored_rows(ax, ay);
        let at = |lx: i32, ly: i32| rows[ly as usize].as_bytes()[lx as usize];
        let mut reach = |lx: i32, ly: i32, dx: i32, dy: i32| {
            let tile = at(lx, ly);
            if !matches!(tile, b'~' | b's') {
                return;
            }
            for k in 1..=2 {
                let (x, y) = (ox + lx + dx * k, oy + ly + dy * k);
                if inside(x, y) && (k == 1 || hash(x, y, 475) % 2 == 0) && !matches!(t[idx(x, y)], b'~' | b'w') {
                    t[idx(x, y)] = tile;
                }
            }
        };
        for lx in 0..SCREEN_W {
            reach(lx, 0, 0, -1);
            reach(lx, SCREEN_H - 1, 0, 1);
        }
        for ly in 0..SCREEN_H {
            reach(0, ly, -1, 0);
            reach(SCREEN_W - 1, ly, 1, 0);
        }
    }
}

/// Meadow touching open water becomes shore (set pieces keep their own).
fn shore(t: &mut [u8]) {
    let src = t.to_vec();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if src[idx(x, y)] != b'.' || super::map::in_set_piece(x, y) {
                continue;
            }
            let wet = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                inside(nx, ny) && src[idx(nx, ny)] == b'~'
            });
            if wet {
                t[idx(x, y)] = b's';
            }
        }
    }
}
