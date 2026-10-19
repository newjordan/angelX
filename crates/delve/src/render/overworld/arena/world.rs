//! The world above the Delve, drawn: the rooms of the realm the party walks
//! with the Delve's keys, in the overworld's own hand. Meadow tufts and
//! pebbled roads on black paper (the overworld's ground marks), trees and
//! the mountain's rock for their edges, its buildings at room scale, dusk
//! and lantern light. An entrance (a stair down, a mine's shaft) wears a
//! board when a knight comes near, and a ring that fills while one stands
//! on it.

use super::super::ink::{Img, hash};
use super::super::kit::{self, RockKind, Tiles};
use super::super::material::{self, Ground, Leaf};
use super::super::ink::vnoise;
use super::{TILE, at, fire, home};
use crate::rules::world::{
    self, ADIT, GATE_ARCH, GATE_STAIR, PADDOCK, PAVILIONS, POST, SHAFT, SPOIL, STABLE_BLOCK,
    STANDS, TILT, TROUGH, WELL,
};
use crate::rules::{RoomKind, Run, Tile};
use crate::render::overworld::light::Light;

/// A rectangle of tiles: column, row, width, height.
type Rect = (i32, i32, i32, i32);

fn inside((c, r, w, h): Rect, col: i32, row: i32) -> bool {
    col >= c && col < c + w && row >= r && row < r + h
}

/// The roads through a room of the world, by its doors and entrances.
fn roads(kind: RoomKind) -> &'static [Rect] {
    match kind {
        // North-south from the realm's road to the mine-head; east-west
        // between the stables and the lists; a spur up to the gate's stair.
        RoomKind::Gate => &[(11, 0, 2, 14), (0, 6, 24, 2), (5, 5, 2, 1)],
        RoomKind::Stables => &[
            (9, 0, 6, 5),
            (0, 6, 24, 2),
            (2, 4, 20, 2),
            (21, 6, 3, 2),
            (19, 4, 2, 4),
            (20, 7, 2, 5),
            (11, 11, 10, 1),
            (11, 11, 2, 3),
        ],
        RoomKind::Lists => &[(9, 0, 6, 3), (11, 3, 2, 4), (0, 6, 3, 2), (1, 8, 12, 2), (11, 8, 2, 6)],
        RoomKind::MineHead => &[(9, 0, 6, 3), (11, 4, 2, 10), (0, 6, 13, 2), (12, 9, 9, 2)],
        RoomKind::Forest => &[(11, 0, 2, 14), (0, 6, 24, 2), (3, 10, 16, 1)],
        RoomKind::Village | RoomKind::Fairground | RoomKind::Ruins => {
            &[(11, 0, 2, 14), (0, 6, 24, 2)]
        }
        RoomKind::Windmill => &[(11, 0, 2, 10), (0, 6, 24, 2)],
        RoomKind::Riverbank => &[(11, 0, 2, 7), (0, 6, 24, 2)],
        RoomKind::Meadow => &[(11, 0, 2, 10), (7, 9, 14, 2)],
        RoomKind::Orchard => &[(11, 0, 2, 8), (0, 6, 24, 2), (5, 7, 2, 3)],
        RoomKind::Marsh => &[(4, 6, 20, 2), (5, 7, 2, 3), (16, 5, 2, 3), (16, 8, 5, 2)],
        RoomKind::Downs => &[(11, 0, 2, 12), (11, 6, 13, 2), (3, 5, 9, 2)],
        RoomKind::Cliffs => &[(11, 0, 2, 14), (3, 6, 21, 2), (16, 7, 2, 3)],
        RoomKind::Crag
        | RoomKind::Rookery
        | RoomKind::Observatory
        | RoomKind::Strand
        | RoomKind::Saltworks
        | RoomKind::Lighthouse
        | RoomKind::Timberyard
        | RoomKind::HedgeMaze
        | RoomKind::Bothy
        | RoomKind::Millpond
        | RoomKind::Sluice
        | RoomKind::SunkenAbbey
        | RoomKind::Belfry
        | RoomKind::Aqueduct
        | RoomKind::Aviary
        | RoomKind::Mummers
        | RoomKind::Laundry
        | RoomKind::Caravan
          | RoomKind::Hippodrome
          | RoomKind::Wheelwright
          | RoomKind::TollBridge
          | RoomKind::GooseGreen
          | RoomKind::Kitewright
          | RoomKind::SignalHill => &[(11, 0, 2, 14), (0, 6, 24, 2)],
        RoomKind::Eyrie => &[(11, 6, 2, 8)],
        RoomKind::Fletchery => &[(0, 6, 21, 2), (11, 8, 2, 6), (19, 8, 2, 2)],
        RoomKind::Scarecrow => &[(11, 6, 13, 2), (11, 8, 2, 6)],
        _ => &[],
    }
}

/// A road mark at pixel `(x, y)`: the overworld's pebbled bed, its edges
/// ragged where the road meets the meadow.
pub(super) fn road_mark(kind: RoomKind, x: i32, y: i32) -> Option<char> {
    let (tx, ty) = (x.div_euclid(TILE), y.div_euclid(TILE));
    let (lx, ly) = (x.rem_euclid(TILE), y.rem_euclid(TILE));
    let on = |dx: i32, dy: i32| roads(kind).iter().any(|&r| inside(r, tx + dx, ty + dy));
    let rag = |a: i32, b: i32, s: u32| (hash(a, b, s) % 3) as i32 + 1;
    let mut edge = false;
    for (gone, dist, reach) in [
        (!on(-1, 0), lx, rag(y, tx, 11)),
        (!on(1, 0), TILE - 1 - lx, rag(y, tx, 12)),
        (!on(0, -1), ly, rag(x, ty, 13)),
        (!on(0, 1), TILE - 1 - ly, rag(x, ty, 14)),
    ] {
        if gone {
            if dist < reach {
                return field(kind, x, y);
            }
            if dist == reach {
                edge = true;
            }
        }
    }
    if edge {
        // The verge: a near-solid line of dark earth where the grass gives way.
        return (!hash(x, y, 16).is_multiple_of(5)).then_some('I');
    }
    // Trodden earth: clods lying close in a calm band, each lit along its
    // top, black paper between; the wheel ruts run through it as clear
    // lines, and here and there a few pebbles lie in it. Grouped marks,
    // never an even speckle.
    let along_x = roads(kind).iter().any(|&(c, r, w, h)| inside((c, r, w, h), tx, ty) && w > h);
    let across = if along_x { ly } else { lx };
    if across == 4 || across == 11 {
        return None;
    }
    if let Some(stone) = pebbles(x, y, 19) {
        return Some(stone);
    }
    // Cells of five by four, every other row shifted, each holding a clod
    // at its own place in the cell so no rows line up.
    let cy = y.div_euclid(4);
    let sx = x + cy.rem_euclid(2) * 2;
    let (cx, px, py) = (sx.div_euclid(5), sx.rem_euclid(5), y.rem_euclid(4));
    let h = hash(cx, cy, 18);
    if h % 10 >= 6 {
        return None;
    }
    let (ox, oy, w) = ((h >> 4) as i32 % 3, (h >> 6) as i32 % 3, 2 + (h >> 8) as i32 % 2);
    if px < ox || px >= ox + w || py < oy || py > oy + 1 {
        return None;
    }
    Some(if py == oy { if (h >> 10) % 3 == 0 { 'P' } else { 'B' } } else { 'b' })
}

/// A few pebbles in a loose cluster at `(x, y)`, each lit along its top
/// over a darker body: one cell in nine of five by four pixels holds one.
fn pebbles(x: i32, y: i32, seed: u32) -> Option<char> {
    let (cx, cy) = (x.div_euclid(5), y.div_euclid(4));
    let (px, py) = (x.rem_euclid(5), y.rem_euclid(4));
    let h = hash(cx, cy, seed);
    if h % 9 != 0 {
        return None;
    }
    let (ox, oy) = (((h >> 4) % 3) as i32, ((h >> 7) % 2) as i32);
    if !(px == ox || px == ox + 1) {
        return None;
    }
    match py - oy {
        0 => Some(if px == ox { 'o' } else { 'r' }),
        1 => Some('P'),
        _ => None,
    }
}

/// Trodden ground off the road: grit in clods of two or three pixels,
/// gathered in patches by a slow field, bare paper between.
fn clods(x: i32, y: i32) -> Option<char> {
    let (tx, ty) = (x.div_euclid(6), y.div_euclid(5));
    let (ox, oy) = (x.rem_euclid(6), y.rem_euclid(5));
    let t = hash(tx, ty, 62);
    let patch = ((vnoise(x as f32 / 30.0, y as f32 / 24.0, 63) - 0.5) / 0.3).clamp(0.0, 1.0);
    if (t & 0xff) as f32 / 255.0 > 0.04 + patch * 0.32 {
        return None;
    }
    let (bx, by) = (1 + (t >> 8) as i32 % 3, 1 + (t >> 10) as i32 % 2);
    match (ox - bx, oy - by) {
        (0, 0) => Some(if (t >> 12) % 3 == 0 { 'P' } else { 'I' }),
        (1, 0) => Some('I'),
        (0, 1) | (1, 1) if (t >> 14) % 2 == 0 => Some('b'),
        _ => None,
    }
}

/// The ground off the road: the place's own field.
fn field(kind: RoomKind, x: i32, y: i32) -> Option<char> {
    match kind {
        RoomKind::Strand | RoomKind::Saltworks | RoomKind::Lighthouse => material::ground(Ground::Sand, x, y),
        RoomKind::Crag | RoomKind::Observatory => material::ground(Ground::Snow, x, y),
        RoomKind::Rookery | RoomKind::Cliffs => {
            if vnoise(x as f32 / 40.0, y as f32 / 30.0, 71) > 0.55 {
                material::ground(Ground::Scree, x, y)
            } else {
                material::ground(Ground::Meadow, x, y)
            }
        }
        RoomKind::Marsh | RoomKind::Sluice | RoomKind::SunkenAbbey | RoomKind::Millpond => {
            if vnoise(x as f32 / 34.0, y as f32 / 26.0, 72) > 0.5 {
                material::ground(Ground::Mud, x, y)
            } else {
                material::ground(Ground::Meadow, x, y)
            }
        }
        _ => material::ground(Ground::Meadow, x, y),
    }
}

/// The room's ground: roads where they run, the place's field everywhere else.
fn ground(cv: &mut Img, run: &Run) {
    let room = run.room();
    let kind = room.kind;
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (tx, ty) = (x / TILE, y / TILE);
            if !matches!(room.tile(tx, ty), Tile::Floor | Tile::Door | Tile::Stairs) {
                continue;
            }
            let on_road = roads(kind).iter().any(|&r| inside(r, tx, ty));
            let mark = if indoor(kind) {
                // Indoors: dressed flagstones.
                super::flags(x, y)
            } else if kind == RoomKind::Crag && (4..11).contains(&ty) && (3..22).contains(&tx) {
                // Smooth trodden snow under the racing piece, its edge
                // wandering and its corners worn round where it meets the
                // loose snow. Tonal, not white: packed snow in the cool bank,
                // its drifts a step lighter in soft patches, sled tracks a
                // step darker.
                let (lx, ly) = ((x - 3 * TILE).min(22 * TILE - 1 - x), (y - 4 * TILE).min(11 * TILE - 1 - y));
                // Smooth noise keeps near its middle: stretched, so the edge
                // bends.
                let bend = ((vnoise(x as f32 / 14.0, y as f32 / 14.0, 79) - 0.5) * 3.0).clamp(-1.0, 1.0);
                let edge = 3.0 + bend * 2.5;
                let round = 14.0;
                let (cx, cy) = ((round - lx as f32).max(0.0), (round - ly as f32).max(0.0));
                let inside = (lx as f32) >= edge && (ly as f32) >= edge && cx.hypot(cy) <= round - edge * 0.5;
                if !inside {
                    field(kind, x, y)
                } else {
                    let drift = vnoise(x as f32 / 14.0, y as f32 / 9.0, 81);
                    Some(match hash(x / 3, y / 2, 78) % 96 {
                        _ if y.rem_euclid(TILE) == 8 && vnoise(x as f32 / 11.0, (y / TILE) as f32, 80) > 0.45 => 'x',
                        0 => 'v',
                        1..=3 => 'U',
                        _ if drift > 0.56 => 'u',
                        _ => 'S',
                    })
                }
            } else if on_road {
                road_mark(kind, x, y)
            } else if kind == RoomKind::MineHead {
                // Trodden ground before the mine: grit more than grass, in
                // patches of each.
                if vnoise(x as f32 / 40.0, y as f32 / 30.0, 61) > 0.42 {
                    clods(x, y)
                } else {
                    field(kind, x, y)
                }
            } else {
                field(kind, x, y)
            };
            if let Some(ch) = mark {
                cv.put(x, y, ch);
            }
        }
    }
}

/// How a room of the world is closed: woods, hedges, or the mountain.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    Wood(Leaf),
    Hedge,
    Rock,
}

fn edge_of(kind: RoomKind) -> Edge {
    match kind {
        RoomKind::MineHead | RoomKind::Crag | RoomKind::Rookery | RoomKind::Observatory | RoomKind::Cliffs | RoomKind::Eyrie => Edge::Rock,
        RoomKind::HedgeMaze => Edge::Hedge,
        RoomKind::Orchard | RoomKind::Bothy | RoomKind::Timberyard | RoomKind::Kitewright => Edge::Wood(Leaf::Copper),
        RoomKind::Forest | RoomKind::SignalHill | RoomKind::Aqueduct | RoomKind::Belfry => Edge::Wood(Leaf::Pine),
        _ => Edge::Wood(Leaf::Oak),
    }
}

/// The room's edges: a wood of overlapping trees (back row darker, front
/// row lit, south trees over north ones), clipped hedges in the maze, and
/// boulders up the mountain. A door leaves a gap for its road.
fn edges(cv: &mut Img, run: &Run) {
    // The edge depends on the room's walls alone: grow it once per layout.
    type Cache = std::sync::Mutex<std::collections::HashMap<(u32, u64), std::sync::Arc<Img>>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let room = run.room();
    let key = (room.kind as u32, room.fingerprint());
    let cache = CACHE.get_or_init(Default::default);
    let hit = cache.lock().ok().and_then(|c| c.get(&key).cloned());
    let layer = hit.unwrap_or_else(|| {
        let mut layer = Img::new(cv.w, cv.h);
        grow_edges(&mut layer, run);
        let layer = std::sync::Arc::new(layer);
        if let Ok(mut c) = cache.lock() {
            if c.len() > 96 {
                c.clear();
            }
            c.insert(key, std::sync::Arc::clone(&layer));
        }
        layer
    });
    // Its shadows darken the ground marks they fall on.
    for y in 0..cv.h {
        for x in 0..cv.w {
            if let Some(c) = layer.get(x, y) {
                if c == super::super::ink::ink('f').unwrap_or(super::super::ink::BLACK)
                    && cv.get(x, y).is_some_and(|g| g != super::super::ink::BLACK)
                {
                    cv.put(x, y, if (x + y) % 2 == 0 { 'f' } else { 'K' });
                } else {
                    cv.set(x, y, c);
                }
            }
        }
    }
}

fn grow_edges(cv: &mut Img, run: &Run) {
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let wall = |c: i32, r: i32| c >= 0 && r >= 0 && c < cols && r < rows && room.tile(c, r) == Tile::Wall;
    if indoor(room.kind) {
        for row in 0..rows {
            for col in 0..cols {
                // A hall's walls: dressed stone, braziers in the corners.
                let corner = (col == 0 || col == cols - 1) && (row == 0 || row == rows - 1);
                if wall(col, row) && !corner {
                    let floor = |c: i32, r: i32| c >= 0 && r >= 0 && c < cols && r < rows && !wall(c, r);
                    let open = [floor(col, row - 1), floor(col + 1, row), floor(col, row + 1), floor(col - 1, row)];
                    material::wall_tile(cv, (col * TILE, row * TILE), TILE, open, material::ramp::STONE, false, 310);
                }
            }
        }
        return;
    }
    // Masses may spill three pixels past their tiles, never further: the
    // canopy overhangs the path a little, never the way through a door.
    let keep = |x: i32, y: i32| {
        [(-5, -5), (5, -5), (-5, 5), (5, 5), (0, 0)]
            .iter()
            .any(|&(dx, dy)| wall((x + dx).div_euclid(TILE), (y + dy).div_euclid(TILE)))
    };
    let style = edge_of(room.kind);
    let mut spots = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            if wall(col, row) {
                spots.push((col, row, hash(col, row, 71 + room.kind as u32)));
            }
        }
    }
    match style {
        Edge::Wood(leaf) => {
            // The back row: smaller, darker, between the front trees; the
            // front row's crowns sit on the wall tiles, trunks at their foot.
            // Each tree has its own size, habit and place in its tile, and
            // now and then a crown of another kind or hue stands among them.
            let pines = leaf == Leaf::Pine;
            let mut back: Vec<_> = spots.iter().map(|&(c, r, v)| (c * TILE + (v % 11) as i32 - 1, r * TILE + 11 + (v >> 4) as i32 % 5, v)).collect();
            back.sort_by_key(|p| p.1);
            for (x, y, v) in back {
                let r = 6.0 + (v >> 8) as f32 % 3.5;
                if pines || (v >> 20) % 7 == 0 {
                    material::pine(cv, x, y, r * 0.9, material::ramp::EDGE_PINE, v, &keep);
                } else {
                    material::broadleaf(cv, x, y, r, material::ramp::EDGE_BACK, v, v >> 5, &keep);
                }
            }
            let mut front: Vec<_> = spots.iter().map(|&(c, r, v)| (c * TILE + 8 + (v >> 12) as i32 % 9 - 4, r * TILE + 19 - (v >> 16) as i32 % 4, v)).collect();
            front.sort_by_key(|p| p.1);
            for (x, y, v) in front {
                // The wood is background: dark, calm crowns, a rare copper
                // one where the region is autumnal.
                let r = 7.0 + (v >> 24) as f32 % 4.5;
                let seed = v ^ 0x9e37;
                let other = (v >> 27) % 6 == 0;
                if pines != other {
                    material::pine(cv, x, y, r * 0.92, material::ramp::EDGE_PINE, seed, &keep);
                } else {
                    let ramp = match (v >> 6) % 7 {
                        0 if leaf == Leaf::Copper => material::ramp::EDGE_COPPER,
                        1 | 4 => material::ramp::EDGE_OLIVE,
                        _ => material::ramp::EDGE,
                    };
                    material::broadleaf(cv, x, y, r, ramp, seed, v >> 9, &keep);
                }
            }
        }
        Edge::Hedge => {
            for &(c, r, v) in &spots {
                let open_s = !wall(c, r + 1);
                let open = [!wall(c, r - 1), !wall(c + 1, r), open_s, !wall(c - 1, r)];
                material::hedge_open(cv, (c * TILE, r * TILE - if wall(c, r - 1) { 0 } else { 3 }, TILE, TILE + if open_s { 0 } else { 3 }), v, open);
            }
        }
        Edge::Rock => {
            let mut stones: Vec<_> = spots
                .iter()
                .flat_map(|&(c, r, v)| {
                    [(c * TILE + 4 + (v % 5) as i32, r * TILE + 9, v), (c * TILE + 12 - (v >> 5) as i32 % 4, r * TILE + 16, v >> 3)]
                })
                .collect();
            stones.sort_by_key(|p| p.1);
            let snow = matches!(room.kind, RoomKind::Crag | RoomKind::Observatory);
            for (x, y, v) in stones {
                // The mountain is background: its boulders keep the dark
                // end of their ramps.
                let ramp: &[char] = if room.kind == RoomKind::MineHead {
                    &['n', 'b', 'I', 'B', 'P']
                } else if v % 3 == 0 {
                    &['K', 'X', 'g', 'j', 'G']
                } else {
                    &['s', 's', 'x', 'S', 'u']
                };
                material::boulder(cv, x, y, 5.5 + (v >> 9) as f32 % 3.0, ramp, v);
                if snow {
                    // Snow on the crowns.
                    for k in -3..=2 {
                        cv.put(x + k, y - 9 + (k * k) / 4, if k % 2 == 0 { 'V' } else { 'v' });
                    }
                }
            }
        }
    }
}

/// Paint a room of the world at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    let tick = run.tick as u32;
    ground(&mut cv, run);
    edges(&mut cv, run);
    waters(&mut cv, run);
    scatter(&mut cv, run);
    undergrowth(&mut cv, run);
    match room.kind {
        RoomKind::Gate => {
            gate(&mut cv, &mut lights, tick);
            super::college::gate_building(&mut cv, tick);
        },
        RoomKind::Stables => {
            stables(&mut cv, &mut lights, tick);
            super::ranch::stables_gate(&mut cv);
        }
        RoomKind::Lists => lists(&mut cv, &mut lights, tick, room.doors[0]),
        RoomKind::MineHead => mine_head(&mut cv, run, &mut lights, tick),
        RoomKind::KingsHall => super::barony::kings_hall(&mut cv, run, &mut lights, tick),
        RoomKind::LandOffice => super::holdings::office_scenery(&mut cv, &mut lights, tick),
        RoomKind::Forest => super::forest::scenery(&mut cv),
        RoomKind::Eyrie => super::race3d::eyrie_scenery(&mut cv, &mut lights, tick),
        _ => {}
    }
    super::pastimes::scenery(&mut cv, room.kind);
    super::countryside::scenery(&mut cv, run);
    super::countryside::lights(&mut lights, run);
    super::dusk_with(&mut cv, &lights, ('F', 'I'), 1.0);
    cv
}

/// The world's indoor rooms: dressed walls, no wood, no water, no weeds.
fn indoor(kind: RoomKind) -> bool {
    matches!(kind, RoomKind::KingsHall | RoomKind::LandOffice | RoomKind::Market | RoomKind::Bathhouse | RoomKind::Smithy)
}

/// Every water tile of a room of the world is one body of open water.
fn waters(cv: &mut Img, run: &Run) {
    let room = run.room();
    if indoor(room.kind) {
        return;
    }
    let wet = |x: i32, y: i32| room.tile(x.div_euclid(TILE), y.div_euclid(TILE)) == Tile::Hazard;
    material::water_field(cv, (0, 0, cv.w, cv.h), &wet, 40 + room.kind as u32, (run.tick / 6) as u32);
}

/// Small things on the open ground, never taller than a boot: pebbles,
/// clover, fallen leaves and toadstools by the wood, puddles in the wet.
fn scatter(cv: &mut Img, run: &Run) {
    let room = run.room();
    if indoor(room.kind) {
        return;
    }
    let road = |c: i32, r: i32| roads(room.kind).iter().any(|&rc| inside(rc, c, r));
    let style = edge_of(room.kind);
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            if room.tile(c, r) != Tile::Floor {
                continue;
            }
            let v = hash(c, r, 77 + room.kind as u32);
            let (x, y) = (c * TILE + 3 + (v >> 4) as i32 % 10, r * TILE + 3 + (v >> 8) as i32 % 10);
            let near_wood = [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| room.tile(c + dx, r + dy) == Tile::Wall);
            match v % 9 {
                0 | 1 if road(c, r) || matches!(style, Edge::Rock) => {
                    // Pebbles, each lit on top.
                    for k in 0..3 {
                        let (px, py) = (x + (hash(c, r + k, 78) % 7) as i32 - 3, y + (hash(c + k, r, 79) % 5) as i32 - 2);
                        cv.put(px, py, 'J');
                        cv.put(px + 1, py, 'G');
                        cv.put(px, py + 1, 'g');
                        cv.put(px + 1, py + 1, 'X');
                    }
                }
                2 if !road(c, r) => {
                    // A clover clump.
                    for k in 0..6 {
                        let h = hash(c * 7 + k, r, 80);
                        cv.put(x + (h % 5) as i32 - 2, y + (h >> 4) as i32 % 3, if k % 3 == 0 { 'm' } else { 'l' });
                    }
                }
                3 if near_wood && !road(c, r) => {
                    // Toadstools at the wood's foot.
                    for k in 0..3 {
                        let (px, py) = (x + k * 3 - 3, y + (k % 2));
                        cv.put(px, py + 1, 'h');
                        cv.put(px, py, 'R');
                        cv.put(px + 1, py, 'p');
                        cv.put(px - 1, py, 'p');
                    }
                }
                4 if near_wood && matches!(style, Edge::Wood(Leaf::Copper | Leaf::Oak)) => {
                    // Fallen leaves.
                    for k in 0..7 {
                        let h = hash(c, r * 5 + k, 81);
                        cv.put(x + (h % 11) as i32 - 5, y + (h >> 4) as i32 % 6 - 3, ['R', 'o', 'p', 'B'][(h >> 8) as usize % 4]);
                    }
                }
                5 if matches!(room.kind, RoomKind::Marsh | RoomKind::Sluice | RoomKind::SunkenAbbey | RoomKind::Millpond) => {
                    material::water(cv, (x - 5, y - 2, 11, 5), v, (run.tick / 6) as u32);
                }
                _ => {}
            }
        }
    }
}

/// Undergrowth where the wood meets the open ground, and reeds along the
/// water's edge. Fixed per room; nothing here stands in the way.
fn undergrowth(cv: &mut Img, run: &Run) {
    let room = run.room();
    if indoor(room.kind) {
        return;
    }
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let road = |c: i32, r: i32| roads(room.kind).iter().any(|&rc| inside(rc, c, r));
    let style = edge_of(room.kind);
    for r in 0..rows {
        for c in 0..cols {
            if room.tile(c, r) != Tile::Floor || road(c, r) {
                continue;
            }
            let v = hash(c, r, 75 + room.kind as u32);
            let (x, y) = (c * TILE, r * TILE);
            let water = [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| room.tile(c + dx, r + dy) == Tile::Hazard);
            let sandy = matches!(room.kind, RoomKind::Strand | RoomKind::Saltworks | RoomKind::Lighthouse);
            if water && sandy {
                if v % 3 == 0 {
                    // Shells and wrack along the tide line.
                    let (sx, sy) = (x + 3 + (v >> 3) as i32 % 9, y + 4 + (v >> 6) as i32 % 8);
                    cv.ellipse(sx as f32, sy as f32, 2.5, 1.5, 'J');
                    cv.put(sx - 1, sy - 1, 'i');
                    cv.line(sx + 4, sy + 2, sx + 8, sy + 1, 'N');
                }
                continue;
            }
            if water && v % 3 == 0 {
                material::reeds(cv, x + 4 + (v >> 3) as i32 % 8, y + 12 + (v >> 6) as i32 % 3, v);
                continue;
            }
            let wall_n = room.tile(c, r - 1) == Tile::Wall;
            let wall_s = room.tile(c, r + 1) == Tile::Wall;
            let wall_w = room.tile(c - 1, r) == Tile::Wall;
            let wall_e = room.tile(c + 1, r) == Tile::Wall;
            if !(wall_n || wall_s || wall_w || wall_e) || v % 5 > 1 {
                if v % 23 == 4 {
                    material::flowers(cv, x + 8, y + 8, v);
                }
                continue;
            }
            let (bx, by) = if wall_n {
                (x + 3 + (v >> 4) as i32 % 10, y + 4)
            } else if wall_s {
                (x + 3 + (v >> 4) as i32 % 10, y + TILE + 1)
            } else if wall_w {
                (x + 2, y + 6 + (v >> 4) as i32 % 8)
            } else {
                (x + TILE - 2, y + 6 + (v >> 4) as i32 % 8)
            };
            match style {
                Edge::Rock => material::boulder(cv, bx, by, 3.0 + (v >> 9) as f32 % 2.0, material::ramp::STONE, v),
                Edge::Wood(leaf) => material::bush(cv, bx, by, 4.0 + (v >> 9) as f32 % 2.0, if leaf == Leaf::Pine { Leaf::Hedge } else { leaf }, v),
                Edge::Hedge => {}
            }
        }
    }
}

/// A lantern on its post, lit; its light.
fn lantern(cv: &mut Img, lights: &mut Vec<Light>, x: i32, y: i32) {
    let im = kit::lantern(true);
    cv.stamp(&im, x - im.w / 2, y - im.h + 1);
    lights.push(fire(x, y - im.h + 3, 44.0, 0.34));
}

/// A torch on a timber post.
fn torch(cv: &mut Img, lights: &mut Vec<Light>, x: i32, y: i32, tick: u32, seed: u32) {
    let im = kit::torch(tick / 4, seed);
    cv.stamp(&im, x - im.w / 2, y - im.h + 1);
    lights.push(fire(x, y - im.h + 2, 50.0, 0.4));
}

/// The Delve's gate, at room scale: a slate rise, a dressed-stone arch, the
/// stair going down into the dark, a torch on either side. The overworld's
/// gate (`wishes::delve_sprite`) drawn twice the size, with its detail.
fn gate_arch(tick: u32) -> Img {
    let (w, h) = (GATE_ARCH.2 * TILE, GATE_ARCH.3 * TILE);
    let mut im = kit::blob(
        w,
        h,
        &[
            (20.0, 40.0, 20.0, 22.0),
            (60.0, 40.0, 20.0, 22.0),
            (40.0, 24.0, 30.0, 22.0),
        ],
        &['x', 'S', 'u', 'U', 'v'],
        0.5,
        29,
        2.0,
    );
    im.outline_inside('s');
    let (cx, cy) = (40.0f32, 40.0f32);
    let (ar, mr) = (17.0f32, 11.5f32);
    for y in 18..h {
        for x in 20..60 {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / ar, (y as f32 + 0.5 - cy) / ar);
            let arch = y as f32 >= cy || dx * dx + dy * dy <= 1.0;
            let (mx, my) = ((x as f32 + 0.5 - cx) / mr, (y as f32 + 0.5 - cy) / mr);
            let mouth =
                (x as f32 + 0.5 - cx).abs() <= mr && (y as f32 >= cy || mx * mx + my * my <= 1.0);
            if mouth {
                // Treads lit near the top, then only the dark going down.
                let tread = y >= 44 && (y - 44) % 5 == 0;
                let lip = y >= 44 && (y - 44) % 5 == 1;
                im.put(
                    x,
                    y,
                    if tread && y < 58 {
                        'j'
                    } else if lip && y < 58 {
                        'g'
                    } else {
                        'k'
                    },
                );
            } else if arch {
                // Dressed blocks: courses three high, joints staggered.
                let course = y / 4;
                let joint = y % 4 == 0 || (x + course * 5) % 9 == 0;
                im.put(x, y, if joint { 'J' } else { 'h' });
            }
        }
    }
    // The keystone and the voussoirs' bright edge.
    for k in 0..40 {
        let a = std::f32::consts::PI * (k as f32 / 39.0);
        let (x, y) = (cx - a.cos() * (ar - 0.5), cy - a.sin() * (ar - 0.5));
        im.put(x as i32, y as i32, 'i');
    }
    im.rect(37, 20, 6, 4, 'H');
    im.rect(38, 21, 4, 2, 'i');
    let _ = tick;
    im
}

fn gate(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (c, r, _, _) = GATE_ARCH;
    let arch = gate_arch(tick);
    cv.stamp(&arch, c * TILE, r * TILE);
    // Torches either side of the mouth, burning: the Delve is open.
    let (sc, sr, sw, _) = GATE_STAIR;
    for (k, x) in [(0u32, sc * TILE - 6), (1, (sc + sw) * TILE + 5)] {
        torch(cv, lights, x, sr * TILE + 14, tick, 31 + k);
    }
    // The well, its roof and bucket.
    let (wc, wr, _, _) = WELL;
    cv.stamp(&well(), wc * TILE, wr * TILE);
    signpost(cv, 13 * TILE + 4, 6 * TILE - 2);
    // The notice post by the road: the realm's notices, Wren's hand.
    let (pc, pr, _, _) = POST;
    let (px, py) = (pc * TILE + 8, pr * TILE + 15);
    cv.rect(px - 1, py - 14, 2, 15, 'B');
    cv.rect(px - 7, py - 15, 14, 8, 'b');
    cv.rect(px - 6, py - 14, 12, 6, 'P');
    for (dx, dy, ink) in [
        (-5, -13, 'T'),
        (-1, -12, 't'),
        (2, -13, 'T'),
        (-4, -10, 't'),
    ] {
        cv.rect(px + dx, py + dy, 3, 2, ink);
    }
    // The road home to the realm, out through the palisade's south gate.
    for x in [10 * TILE + 12, 13 * TILE + 3] {
        cv.rect(x, 13 * TILE, 2, TILE, 'B');
        cv.put(x, 13 * TILE, 'o');
    }
    lantern(cv, lights, 9 * TILE + 2, 5 * TILE + 8);
    lantern(cv, lights, 14 * TILE + 2, 9 * TILE + 2);
    lantern(cv, lights, 9 * TILE + 8, 11 * TILE);
}

/// The courtyard's well at room scale: a ring of dressed stone round dark
/// water, a little shingled roof on two posts, its bucket on the rope.
fn well() -> Img {
    let mut im = Img::new(32, 32);
    // The ring, seen from above and a little in front.
    for y in 14..30 {
        for x in 2..30 {
            let (dx, dy) = ((x as f32 - 15.5) / 13.5, (y as f32 - 22.0) / 7.5);
            let d = dx * dx + dy * dy;
            if d <= 1.0 {
                let water = d < 0.45 && y < 25;
                let joint = (x + (y / 3) * 3) % 6 == 0 || y % 3 == 0;
                im.put(
                    x,
                    y,
                    if water {
                        if (x + y) % 7 == 0 { 'Q' } else { 'q' }
                    } else if joint {
                        'G'
                    } else if y < 22 {
                        'h'
                    } else {
                        'J'
                    },
                );
            }
        }
    }
    // Two posts and the roof.
    im.rect(5, 6, 2, 18, 'B');
    im.rect(25, 6, 2, 18, 'B');
    for y in 0..7 {
        let inset = 6 - y;
        for x in 2 + inset..30 - inset {
            im.put(x, y, if (x + y) % 3 == 0 { 'x' } else { 'S' });
        }
    }
    im.line(2, 7, 29, 7, 'b');
    // The windlass and the bucket on its rope.
    im.line(7, 9, 24, 9, 'r');
    im.line(16, 10, 16, 15, 'o');
    im.rect(14, 15, 5, 4, 'B');
    im.line(14, 15, 18, 15, 'o');
    im = super::super::kit::shaded_rim(&im);
    im
}

/// A signpost at the crossroads: an arm to each way, its name on it.
fn signpost(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y - 40, 2, 41, 'r');
    cv.line(x, y - 40, x, y, 'o');
    // North on top, pointing up; west and east below, pointing their ways.
    for (i, (name, way)) in [("MINES", 0), ("STABLES", -1), ("LISTS", 1)]
        .into_iter()
        .enumerate()
    {
        let w = super::super::ink::text_width(name) + 6;
        let by = y - 40 + i as i32 * 10;
        let bx = match way {
            -1 => x - w + 1,
            1 => x + 1,
            _ => x + 1 - w / 2,
        };
        cv.rect(bx, by, w, 8, 'B');
        cv.line(bx, by, bx + w - 1, by, 'r');
        match way {
            -1 => cv.line(bx - 1, by + 3, bx - 1, by + 4, 'B'),
            1 => cv.line(bx + w, by + 3, bx + w, by + 4, 'B'),
            _ => cv.line(bx + w / 2 - 1, by - 1, bx + w / 2, by - 1, 'r'),
        }
        cv.text(bx + 3, by + 1, name, 'O');
    }
}

/// The stables, at rest: the stable block with its three stalls, the
/// trough, the paddock fence (the horses are figures).
fn stables(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (c, r, w, h) = STABLE_BLOCK;
    let (x0, y0, bw, bh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    // A long thatched roof over timber walls, three stall doors open.
    for y in y0..y0 + 22 {
        for x in x0..x0 + bw {
            // Thatch in combed bands, the ridge lit, the eaves in shadow.
            let ramp = ['o', 'r', 'P', 'p', 'B'];
            let k = ((y - y0) * 5 / 22) as usize;
            let comb = (x + (y - y0) / 3) % 5 == 0;
            let straw = hash(x / 2, y, 41).is_multiple_of(7);
            cv.put(
                x,
                y,
                if straw {
                    'O'
                } else if comb {
                    'b'
                } else {
                    ramp[k.min(4)]
                },
            );
        }
    }
    cv.line(x0, y0, x0 + bw - 1, y0, 't');
    cv.line(x0, y0 + 22, x0 + bw - 1, y0 + 22, 'b');
    for y in y0 + 23..y0 + bh {
        for x in x0..x0 + bw {
            let board = (x - x0) % 6 == 0;
            cv.put(x, y, if board { 'I' } else { 'B' });
        }
    }
    for &(dx, dw) in &stall_doors() {
        cv.rect(dx, y0 + 25, dw, bh - 25, 'k');
        cv.rect(dx - 1, y0 + 24, dw + 2, 1, 'o');
        cv.line(dx - 1, y0 + 24, dx - 1, y0 + bh - 1, 'r');
        cv.line(dx + dw, y0 + 24, dx + dw, y0 + bh - 1, 'r');
    }
    // The trough: a timber box, water in it.
    // The northern road's arch matches the migrated collision tiles.
    cv.rect(9 * TILE, TILE, 6 * TILE, 3 * TILE, 'k');
    cv.rect(9 * TILE + 3, TILE + 1, 6 * TILE - 6, 3 * TILE - 1, 'r');
    cv.line(9 * TILE, TILE, 15 * TILE - 1, TILE, 'h');
    cv.line(9 * TILE, TILE, 9 * TILE, 4 * TILE - 1, 'o');
    cv.line(15 * TILE - 1, TILE, 15 * TILE - 1, 4 * TILE - 1, 'o');
    cv.text(10 * TILE, 4 * TILE + 3, "ROOKERY ROAD", 'O');
    let (tc, tr, tw, _) = TROUGH;
    let (tx, ty) = (tc * TILE + 2, tr * TILE + 4);
    cv.rect(tx, ty, tw * TILE - 4, 9, 'B');
    cv.rect(tx + 2, ty + 2, tw * TILE - 8, 4, 'q');
    cv.line(tx + 3, ty + 3, tx + 9, ty + 3, 'Q');
    // The paddock fence: two rails on posts.
    let (fc, fr, fw, _) = PADDOCK;
    let (fx, fy) = (fc * TILE, fr * TILE + 8);
    for x in fx..fx + fw * TILE {
        cv.put(x, fy - 4, 'o');
        cv.put(x, fy, 'r');
    }
    for x in (fx..fx + fw * TILE).step_by(16) {
        cv.rect(x, fy - 7, 2, 10, 'B');
    }
    // Hay bales, a saddle on its stand.
    for (bx, by) in [(18 * TILE + 4, 6 * TILE + 2), (19 * TILE + 6, 6 * TILE + 8)] {
        cv.rect(bx, by, 12, 8, 'o');
        cv.line(bx, by + 3, bx + 11, by + 3, 'r');
        cv.line(bx, by, bx + 11, by, 'T');
    }
    lantern(cv, lights, 12 * TILE, 5 * TILE - 2);
    let _ = tick;
}

/// Where the three stall doors stand along the stable block (x, width).
pub(super) fn stall_doors() -> [(i32, i32); 3] {
    [(5 * TILE, 22), (11 * TILE + 5, 22), (17 * TILE + 10, 22)]
}

/// The lists, at rest: the stands and their crowd's benches, the tilt's
/// striped rail, the pavilions.
pub(super) fn lists(cv: &mut Img, lights: &mut Vec<Light>, tick: u32, north_road: bool) {
    // Raked sand inside the lists' fence.
    for y in 4 * TILE..12 * TILE {
        for x in 3 * TILE..21 * TILE {
            if hash(x, y, 102).is_multiple_of(11) {
                cv.put(x, y, 'I');
            }
        }
    }
    let (c, r, w, h) = STANDS;
    let (x0, y0, sw, sh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    // A canopy in the realm's red and gold over tiered benches.
    for x in x0..x0 + sw {
        // The caravan road cuts through the grandstand. Do not paint benches
        // over the full four-knight collision clearance (columns 9..15).
        if north_road && (9 * TILE..15 * TILE).contains(&x) {
            continue;
        }
        for y in y0..y0 + 6 {
            cv.put(x, y, if (x / 6) % 2 == 0 { 'T' } else { 'R' });
        }
        cv.put(x, y0 + 6, 'p');
        for y in y0 + 7..y0 + sh {
            let tier = (y - y0 - 7) % 7 == 0;
            cv.put(x, y, if tier { 'r' } else { 'b' });
        }
    }
    // The tilt: a striped rail on posts down the middle of the lists.
    let (tc, tr, tw, _) = TILT;
    let (tx, ty) = (tc * TILE, tr * TILE + 6);
    for x in tx..tx + tw * TILE {
        cv.put(x, ty, if (x / 6) % 2 == 0 { 'T' } else { 'R' });
        cv.put(x, ty + 1, if (x / 6) % 2 == 0 { 'o' } else { 'p' });
        cv.put(x, ty + 2, 'B');
    }
    for x in (tx..tx + tw * TILE).step_by(12) {
        cv.rect(x, ty - 2, 2, 8, 'b');
    }
    // The fence round the lists.
    for x in 3 * TILE..21 * TILE {
        cv.put(x, 12 * TILE + 4, 'r');
    }
    for x in (3 * TILE..21 * TILE).step_by(8) {
        cv.rect(x, 12 * TILE + 2, 2, 5, 'B');
    }
    // The pavilions: the realm's (red, west) and the rival's (blue, east).
    for (i, (pc, pr, pw, ph)) in PAVILIONS.into_iter().enumerate() {
        let tent = kit::tent(if i == 0 {
            kit::Heraldry::Red
        } else {
            kit::Heraldry::Blue
        });
        cv.stamp(
            &tent,
            pc * TILE + (pw * TILE - tent.w) / 2,
            (pr + ph) * TILE - tent.h,
        );
    }
    lantern(cv, lights, 2 * TILE + 4, 6 * TILE - 2);
    lantern(cv, lights, 22 * TILE - 2, 9 * TILE);
    let _ = tick;
}

/// The mine-head: the mountain's adit with its timbers and the shaft going
/// down, the fallen gate-hall of Caer Dwfn to the west, spoil heaps and a
/// cart to the east.
pub(super) fn mine_head(cv: &mut Img, run: &Run, lights: &mut Vec<Light>, tick: u32) {
    // The adit: a dark mouth in the rock, timbered, rails going in.
    let (c, r, w, h) = ADIT;
    let (x0, y0, ah) = (c * TILE, r * TILE, h * TILE);
    let tiles = Tiles::get();
    for row in 0..h {
        for col in 0..w {
            cv.stamp(
                tiles.rock(RockKind::Ore, hash(c + col, r + row, 73)),
                x0 + col * TILE,
                y0 + row * TILE,
            );
        }
    }
    let (sc, _, sw, _) = SHAFT;
    let (mx0, mx1) = (sc * TILE - 2, (sc + sw) * TILE + 2);
    for y in y0 + 10..y0 + ah {
        for x in mx0..mx1 {
            let tread = y > y0 + ah - 14 && (y - y0) % 4 == 0;
            cv.put(x, y, if tread { 'g' } else { 'k' });
        }
    }
    // The timber set: two posts, a lintel, a cap piece.
    cv.rect(mx0 - 4, y0 + 8, 4, ah - 8, 'B');
    cv.rect(mx1, y0 + 8, 4, ah - 8, 'B');
    cv.rect(mx0 - 6, y0 + 5, mx1 - mx0 + 12, 4, 'r');
    cv.line(mx0 - 6, y0 + 5, mx1 + 5, y0 + 5, 'o');
    cv.line(mx0 - 4, y0 + 8, mx0 - 4, y0 + ah - 1, 'p');

    // Rails out of the mouth, down the road.
    for x in [mx0 + 6, mx1 - 7] {
        cv.line(x, y0 + ah, x, 13 * TILE, 'h');
    }
    for y in ((y0 + ah)..13 * TILE).step_by(5) {
        cv.line(mx0 + 4, y, mx1 - 5, y, 'b');
    }
    torch(cv, lights, mx0 - 10, y0 + ah + 6, tick, 5);
    torch(cv, lights, mx1 + 9, y0 + ah + 6, tick, 6);
    // Spoil heaps and a cart east of the adit.
    let (pc, pr, _, ph) = SPOIL;
    let heaps = kit::blob(
        64,
        32,
        &[
            (14.0, 22.0, 12.0, 9.0),
            (34.0, 19.0, 14.0, 12.0),
            (52.0, 23.0, 11.0, 8.0),
        ],
        &['b', 'I', 'r', 'P', 'R'],
        0.5,
        83,
        2.0,
    );
    cv.stamp(&heaps, pc * TILE, (pr + ph) * TILE - heaps.h);
    let cart = kit::handcart();
    cv.stamp(&cart, 15 * TILE, 5 * TILE);
    // The gate-hall of Caer Dwfn: a ruin, until the realm pays to raise it.
    super::barony::gate_hall(cv, run, lights, tick);
    if run.room().doors[0] {
        // A timbered through-road to Mummers' Hollow, not solid mine rock.
        // Apply after the rubble and stockpile, whose sprite fringes also
        // overlap this collision-clear road. Preserve the shaft below row
        // three and all of the great hall's structural tiles to its west.
        for y in TILE..3 * TILE {
            for x in 9 * TILE..15 * TILE {
                cv.put(x, y, road_mark(RoomKind::MineHead, x, y).unwrap_or('k'));
            }
        }
        cv.line(9 * TILE - 2, TILE, 9 * TILE - 2, 3 * TILE - 1, 'o');
        cv.line(15 * TILE + 1, TILE, 15 * TILE + 1, 3 * TILE - 1, 'o');
        cv.line(9 * TILE - 2, TILE - 1, 15 * TILE + 1, TILE - 1, 'r');
    }
}

/// Over everything in a room of the world: the board of the entrance a
/// knight is near.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    // The College and the Colosseum letter their own doors.
    if crate::rules::college::is_room(kind) || crate::rules::colosseum::is_room(kind) {
        return;
    }
    super::swamp::marsh_board(cv, run);
    if kind == RoomKind::Gate {
        let label = if run.kingdom.return_to.is_some() {
            "KINGDOM"
        } else {
            "SOUTH: GROVE"
        };
        let board = home::board(&[(label.into(), '9')]);
        cv.stamp(&board, (cv.w - board.w) / 2, cv.h - board.h - TILE);
    }
    if crate::rules::countryside::PLACES.contains(&kind)
        || crate::rules::countryside::WEST_PLACES.contains(&kind)
        || crate::rules::countryside::EAST_PLACES.contains(&kind)
        || crate::rules::countryside::NORTH_PLACES.contains(&kind)
    {
        let title = home::board(&[(
            crate::rules::countryside::name(kind).into(),
            '9',
        )]);
        // Keep the north doorway visible instead of putting a title on it.
        cv.stamp(&title, 3, 3);
    }
    // Names come from real neighbouring rooms, not promised missing routes.
    for dir in 0..4 {
        if !run.room().doors[dir] {
            continue;
        }
        let Some(next) = run.dungeon.neighbour(run.at, dir) else {
            continue;
        };
        let near = run.players.values().any(|h| {
            h.hp > 0
                && !h.stone
                && match dir {
                    0 => h.y < 5.8,
                    1 => h.x > run.room().width() - 5.8,
                    2 => h.y > run.room().height() - 5.8,
                    _ => h.x < 5.8,
                }
        });
        // At the horse market the pen's own board speaks instead.
        if !near || (dir == 2 && crate::rules::joust::on_market(run)) {
            continue;
        }
        let name = if run.kingdom.return_to.is_some() {
            "KINGDOM"
        } else {
            crate::rules::countryside::name(run.dungeon.rooms[next].kind)
        };
        let board = home::board(&[(
            format!("{}: {name}", ["NORTH", "EAST", "SOUTH", "WEST"][dir]),
            'O',
        )]);
        let (x, y) = match dir {
            0 => ((cv.w - board.w) / 2, TILE * 2),
            1 => (cv.w - board.w - 3, TILE * 4),
            // Leave a clear gap above the five-line festival ledgers.
            2 => ((cv.w - board.w) / 2, cv.h - board.h - TILE * 3 - 8),
            _ => (3, TILE * 4),
        };
        cv.stamp(&board, x, y);
    }
    for e in world::ENTRANCES.iter().filter(|e| e.room == kind) {
        let (ex, ey) = world::entrance_centre(e);
        let reach = if world::open(e, &run.home) { 4.2 } else { 1.8 };
        let near = run
            .players
            .values()
            .any(|h| h.hp > 0 && (h.x - ex).hypot(h.y - ey) < reach);
        if !near {
            continue;
        }
        let board = if world::open(e, &run.home) {
            home::board(&[
                (if run.returns_to_kingdom(e.id) { "KINGDOM" } else { e.label }.into(), '9'),
                (
                    if run.returns_to_kingdom(e.id) {
                        "TAKE STAIR"
                    } else if e.id.ends_with("coach") || e.id.ends_with("ferry") || e.id.ends_with("lift") || e.id.ends_with("plate") || e.needs == Some("country:caravan")
                    {
                        "STAND ON THE TRAVEL PLATE"
                    } else {
                        "STAND ON THE STAIR"
                    }
                    .into(),
                    'O',
                ),
            ])
        } else if e.needs == Some("country:racecoach") {
            home::board(&[
                ("THE SUSPICIOUSLY FAST COACH".into(), '9'),
                ("FIRST, FIX THE COACH".into(), 'h'),
            ])
        } else if e.needs == Some("country:caravan") {
            home::board(&[
                ("THE WHEELY INN HAS NO WHEELS YET".into(), '9'),
                ("FIRST, WIN THE HALF-PIPE".into(), 'h'),
            ])
        } else if e.needs == Some("country:inn") {
            home::board(&[
                ("THE COACH HAS NO INN TO STOP AT".into(), '9'),
                ("BUILD THE BENT SPOON INN FIRST".into(), 'h'),
            ])
        } else if e.needs == Some("country:ferry") {
            home::board(&[
                ("SIR DAMP IS WAITING FOR A FERRY OFFICE".into(), '9'),
                ("FIRST, LIGHT THE ROAD".into(), 'h'),
            ])
        } else if e.needs == Some("country:lift") {
            home::board(&[
                ("THE GOAT HAS NO LIFT TO PULL".into(), '9'),
                ("FIRST, SIGN THE STAKES".into(), 'h'),
            ])
        } else {
            home::board(&[
                ("THE GATE-HALL OF CAER DWFN".to_string(), '9'),
                ("FALLEN IN: ITS DOOR IS RUBBLE".to_string(), 'h'),
            ])
        };
        if matches!(e.needs, Some("country:caravan" | "country:racecoach")) {
            cv.stamp(&board, 4, 13);
            continue;
        }
        let (x, y) = at(ex, ey);
        // Beside the entrance, never over the knights who arrive below it.
        let bx = if x + 22 + board.w <= cv.w - 2 {
            x + 22
        } else {
            x - 22 - board.w
        };
        cv.stamp(
            &board,
            bx.clamp(2, cv.w - board.w - 2),
            (y - board.h / 2).clamp(2, cv.h - board.h - 2),
        );
    }
}

/// The ring filling round the entrance a knight stands on.
pub(super) fn entrance_ring(cv: &mut Img, run: &Run) {
    if run.descending == 0 {
        return;
    }
    let Some((e, _)) = world::entrance_under(run) else {
        return;
    };
    let (ex, ey) = world::entrance_centre(e);
    let (cx, cy) = at(ex, ey);
    let k = run.descending as f32 / crate::rules::home::DESCEND_HOLD as f32;
    let n = 48;
    for i in 0..(n as f32 * k.min(1.0)) as i32 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
        for r in [16.0f32, 17.0] {
            cv.put(
                cx + (a.cos() * r) as i32,
                cy + (a.sin() * r) as i32,
                if i % 2 == 0 { '6' } else { '5' },
            );
        }
    }
}
