//! The kingdom's roads: main roads three tiles wide between the set pieces'
//! gates, lanes two wide, laid as polylines of realm tiles. They are laid
//! over the grown ground (water under a road becomes a plank bridge, rock a
//! pass, a hedge a gap) and stop at a set piece's edge, where its own
//! authored street carries on. The places' own lanes to the nearest road
//! are cut later by the kingdom's geography (`super::kingdom`).

use super::map::{MAP_H, MAP_W, in_set_piece};

/// A road: its width in tiles and the realm tiles it runs through.
pub struct Road {
    pub width: u8,
    pub points: &'static [(i32, i32)],
}

const MAIN: u8 = 3;
const LANE: u8 = 2;

/// Every road and lane. The ends sit one tile outside a set piece's gate;
/// between them a road bends through its points (`course`).
pub const ROADS: &[Road] = &[
    // The King's Road, west to the harbour gate and east to the Lists.
    Road { width: MAIN, points: &[(20, 54), (30, 55), (40, 54), (52, 57), (64, 56), (75, 54)] },
    // It leaves the town a row below the Keep's wall.
    Road { width: MAIN, points: &[(92, 55), (104, 56), (118, 57), (132, 55), (140, 54), (147, 54)] },
    // On from the Lists to the Repo Wards on the coast.
    Road { width: MAIN, points: &[(164, 57), (178, 58), (192, 62), (204, 66), (212, 72), (219, 73)] },
    // The North Road: along the Keep's west wall, over the Kingsriver,
    // past the College and the Delve's gate, up to the Mines.
    Road { width: MAIN, points: &[(79, 49), (80, 43), (88, 38), (98, 35), (103, 31), (104, 29)] },
    // The High Road across the north: the Observatory, the Mines, the
    // Dragon Keep.
    Road { width: MAIN, points: &[(44, 24), (54, 26), (66, 23), (78, 26), (90, 24), (99, 24)] },
    Road { width: MAIN, points: &[(116, 24), (128, 22), (144, 25), (158, 22), (171, 24)] },
    // The Fen Road: the harbour gate north over the river to the
    // Observatory, and south past the Swamp to the March.
    Road { width: MAIN, points: &[(12, 49), (13, 44), (20, 38), (30, 34), (36, 29)] },
    Road {
        width: MAIN,
        points: &[(40, 55), (38, 70), (40, 87), (38, 100), (32, 116), (26, 136), (24, 156)],
    },
    Road { width: LANE, points: &[(39, 87), (30, 89), (20, 87)] },
    // The town's high street, Keep to Village, and the street along the
    // south of the town on to the Tilt-Yards.
    Road { width: MAIN, points: &[(83, 61), (83, 81)] },
    Road { width: MAIN, points: &[(68, 87), (75, 87)] },
    Road { width: MAIN, points: &[(92, 87), (99, 87)] },
    Road { width: MAIN, points: &[(116, 87), (123, 87)] },
    Road { width: MAIN, points: &[(140, 87), (171, 87)] },
    Road { width: LANE, points: &[(107, 81), (107, 55)] },
    // The Lists down to the Tilt-Yards' street.
    Road { width: MAIN, points: &[(155, 61), (156, 74), (155, 86)] },
    // The Toll Road, north over the river from the King's Road to the
    // High Road.
    Road { width: MAIN, points: &[(140, 53), (139, 44), (142, 34), (144, 25)] },
    // South from the town: the South Road to the March and on between its
    // castles to the far lane, the Artisans' and the Colosseum's roads to
    // the Homecoming fields, the Tournament's through Longbow Country, and
    // the School's lane to the Fen Road.
    Road { width: MAIN, points: &[(83, 93), (84, 108), (79, 124), (74, 140), (74, 156), (75, 171)] },
    Road { width: MAIN, points: &[(107, 93), (108, 106), (114, 117), (123, 120)] },
    Road { width: MAIN, points: &[(136, 93), (137, 103), (132, 109), (131, 113)] },
    Road { width: MAIN, points: &[(175, 93), (177, 110), (186, 128), (194, 144), (198, 156)] },
    Road { width: LANE, points: &[(58, 93), (57, 106), (49, 117), (32, 119)] },
    // The Coast Road, north to the lighthouse and south to the March.
    Road { width: MAIN, points: &[(204, 65), (210, 52), (216, 38), (218, 24), (218, 12)] },
    Road {
        width: MAIN,
        points: &[(205, 67), (209, 82), (213, 98), (214, 116), (212, 136), (214, 156)],
    },
    // The frontier road along the March, and the far lane south of it.
    // Eight castles stand with their backs to the Marchwater and their
    // gates on the frontier road, eight behind it with their gates on the
    // far lane (`super::houses`): each road runs level along a castle's
    // face, its first row the tile before the gate, and bends between.
    Road {
        width: MAIN,
        points: &[
            (2, 155), (5, 155), (16, 155), (32, 157), (41, 157), (52, 156), (65, 156),
            (82, 155), (93, 155), (104, 156), (113, 156), (125, 157), (140, 157),
            (151, 155), (161, 155), (172, 156), (188, 156), (200, 156), (214, 157), (226, 156),
        ],
    },
    Road {
        width: LANE,
        points: &[
            (3, 157), (3, 164), (6, 170), (10, 170), (22, 170), (34, 170), (45, 170),
            (55, 171), (65, 171), (75, 171), (90, 171), (100, 170), (115, 170),
            (125, 170), (138, 170), (149, 171), (164, 171), (174, 171), (184, 171),
            (194, 170), (207, 170), (213, 167), (214, 159),
        ],
    },
    // The Homecoming lane east into Longbow Country.
    Road { width: LANE, points: &[(133, 112), (146, 115), (160, 118), (178, 116)] },
];

fn idx(x: i32, y: i32) -> usize {
    (y * MAP_W + x) as usize
}

fn in_castle(x: i32, y: i32) -> bool {
    super::castles::footprints().any(|(tx, ty, tw, th)| x >= tx && y >= ty && x < tx + tw && y < ty + th)
}

/// The road width at every tile (0 where no road runs), as laid.
pub fn raster() -> Vec<u8> {
    let mut out = vec![0u8; (MAP_W * MAP_H) as usize];
    for road in ROADS {
        let mut prev: Option<(i32, i32)> = None;
        for (x, y) in course(road.points) {
            match prev {
                Some(p) => walk(p, (x, y), |x, y| brush(&mut out, x, y, road.width)),
                None => brush(&mut out, x, y, road.width),
            }
            prev = Some((x, y));
        }
    }
    out
}

/// The tiles a road's centre line runs through: a smooth curve through
/// its points (Catmull-Rom), sampled a half tile at a time.
pub fn course(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let p = |i: isize| {
        let (x, y) = points[i.clamp(0, points.len() as isize - 1) as usize];
        (x as f32, y as f32)
    };
    let mut out = Vec::new();
    for i in 0..points.len().saturating_sub(1) as isize {
        let (p0, p1, p2, p3) = (p(i - 1), p(i), p(i + 1), p(i + 2));
        let len = ((p2.0 - p1.0).abs() + (p2.1 - p1.1).abs()).max(1.0);
        let steps = (len * 2.0) as usize;
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let at = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3)
            };
            let tile = (at(p0.0, p1.0, p2.0, p3.0).round() as i32, at(p0.1, p1.1, p2.1, p3.1).round() as i32);
            if out.last() != Some(&tile) {
                out.push(tile);
            }
        }
    }
    out
}

/// Step from `a` to `b` one axis at a time, so a road stays joined edge to
/// edge, calling `put` on every tile but the first.
fn walk((x0, y0): (i32, i32), (x1, y1): (i32, i32), mut put: impl FnMut(i32, i32)) {
    let (dx, dy) = ((x1 - x0).abs(), (y1 - y0).abs());
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    let (mut a, mut b) = (0, 0);
    while a < dx || b < dy {
        if b >= dy || (a < dx && (2 * a + 1) * dy < (2 * b + 1) * dx) {
            a += 1;
        } else {
            b += 1;
        }
        put(x0 + sx * a, y0 + sy * b);
    }
}

/// A road's width laid round `(x, y)`: three tiles centred, two to the
/// south-east.
fn brush(out: &mut [u8], x: i32, y: i32, width: u8) {
    let (lo, hi) = if width >= MAIN { (-1, 1) } else { (0, 1) };
    for dy in lo..=hi {
        for dx in lo..=hi {
            let (px, py) = (x + dx, y + dy);
            // Roads go round a castle's wall, never through it.
            if px > 0 && py > 0 && px < MAP_W - 1 && py < MAP_H - 1 && !in_castle(px, py) {
                let at = &mut out[idx(px, py)];
                *at = (*at).max(width);
            }
        }
    }
}

/// Lay the roads into the terrain: road over ground, planks over water and
/// lava; never inside a set piece, whose own streets meet them at its gates.
pub fn lay(tiles: &mut [u8], raster: &[u8]) {
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if raster[idx(x, y)] == 0 || in_set_piece(x, y) {
                continue;
            }
            let t = &mut tiles[idx(x, y)];
            *t = if matches!(*t, b'~' | b'w' | b'f' | b'L' | b'F') {
                b'H'
            } else {
                b'='
            };
        }
    }
}
