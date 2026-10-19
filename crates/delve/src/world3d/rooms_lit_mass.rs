//! A lit room's masses (docs/REALM-MONOLITH.md, "Rooms, lit"): the
//! generator's walls, pillars and props, which it makes as small closed
//! boxes, made to read as big brutalist masses, never where a knight walks
//! and never hiding more of the floor than the generator lets
//! (`blockout::HIDE_ONE`, `blockout::HIDE_ALL`).
//!
//! - **Walls are thick and stepped.** The far wall and the side walls stand
//!   on their wall tiles as a plinth the tile deep, then a great mass set a
//!   little back from the room and running on out of it ([`THICK`]), the
//!   far wall rising out of sight ([`FAR`]); full-height fins stand proud
//!   of the face every few tiles, and every other bay of the far wall
//!   between them is a deep slot over a sill. What hangs on a face (a
//!   torch, a window, a banner) goes back with it.
//! - **A delve's west wall is opened** where it stands full height, in every
//!   other bay between its fins: a window with a sill under it and a lintel over,
//!   its walling thin about it and nothing behind, so the low sun of a delve
//!   comes through as a hard-edged shaft across the floor.
//! - **Doorways are deep.** A doorway's leaf is set back into the wall's
//!   thickness, so the way through is a deep slot between the wall's
//!   returns, under a lintel as thick as the wall and as high.
//! - **Pillars and what stands on blocked tiles** step up from a plinth to
//!   a shaft set in, under a cap: a box a tile or more across is a base, a
//!   body and a slab.
//! - **The rock's cores.** Where a stretch of wall stands in the room (an
//!   open floor's rock between its chambers, a passage's walls), each tile
//!   deep in it rises as far as the floor north of it lets the camera see
//!   over it, under a cap of dark slate: a great mass stepping up away from
//!   the floor, not a low plateau.
//! - **The near wall** runs on, low, out of the room toward the eye.
//!
//! Everything a wall tile had stays over it, and nothing new stands over a
//! tile a knight walks taller than a boot, below a lintel's headroom: the
//! agreement the generator keeps (`rooms_lit__tests`).

use super::blockout::{self, Dressing, Group, Role, Setting, Shape, Solid, TILE, View};
use crate::rules::{Room, Tile};

/// How far a wall's mass runs on out of the room, past its tile.
pub const THICK: f32 = 28.0;
/// How high the far wall rises: out of the room camera's sight.
pub const FAR: f32 = 112.0;
/// A wall's plinth: its height over the floor line, the whole tile deep.
pub const PLINTH: f32 = 10.0;
/// How far a wall's face above its plinth is set back from the room.
pub const SET_BACK: f32 = 5.0;
/// A fin every this many tiles of a wall, its width.
pub const FIN_EVERY: i32 = 3;
pub const FIN: f32 = 8.0;
/// The far wall's slots: in every other bay between its fins, the face
/// cut this far back into the wall, from a sill over the plinth up to under
/// a lintel.
pub const SLOT_DEPTH: f32 = 15.0;
pub const SLOT_SILL: f32 = PLINTH + 3.0;
pub const SLOT_TOP: f32 = 42.0;
/// How far in from its bay's fins a slot stands.
pub const SLOT_INSET: f32 = 7.0;

/// Make `groups`, the blockout of `room` dressed `d`, monolithic.
pub fn monolith(groups: &mut Vec<Group>, room: &Room, d: &Dressing) {
    monolith_in(groups, room, d, room, (0, 0));
}

/// [`monolith`] for `room` cut from map `map` at tile `origin` (an open
/// floor's piece): its rock's cores rise as the whole map's floor lets them.
pub fn monolith_in(groups: &mut Vec<Group>, room: &Room, d: &Dressing, map: &Room, origin: (i32, i32)) {
    let masonry = !matches!(d.setting, Setting::Outdoor { .. });
    let sun_in = matches!(d.setting, Setting::Dungeon(_));
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let view = View::of(room);
    let open = blockout::open_tiles(room);
    // What hangs on the far wall's face (torches, windows, banners, lamps):
    // no slot is cut behind it.
    let hung: Vec<(f32, f32)> = groups
        .iter()
        .filter(|g| g.name.starts_with("torches") || g.name.starts_with("fixed to the far wall"))
        .flat_map(|g| &g.solids)
        .filter(|s| s.footprint()[1] <= TILE + 2.0)
        .map(|s| (s.footprint()[0] - 4.0, s.footprint()[2] + 4.0))
        .collect();
    let mut set_back = [false; 3];
    for g in groups.iter_mut() {
        let before = g.solids.clone();
        match g.role {
            Role::Wall if masonry => {
                g.solids = g.solids.iter().flat_map(|s| wall_mass(s, room, w, h, &hung, sun_in)).collect();
            }
            Role::Door if masonry && g.name.starts_with("the ") => {
                g.solids = g.solids.iter().flat_map(|s| door_mass(s, w, h)).collect();
            }
            Role::Prop => {
                g.solids = g.solids.iter().flat_map(|s| stepped(s, room)).collect();
            }
            _ => continue,
        }
        // What the camera sees of the floor stays as the generator kept it.
        if !blockout::worst_hidden_ok(&view, &open, &g.solids, &before) {
            g.solids = before;
        } else if g.role == Role::Wall && masonry {
            for s in &before {
                match side_of(s, w, h) {
                    Side::North => set_back[0] = true,
                    Side::West => set_back[1] = true,
                    Side::East => set_back[2] = true,
                    _ => {}
                }
            }
        }
    }
    if masonry {
        let walls: Vec<Solid> = groups.iter().filter(|g| g.role == Role::Wall).flat_map(|g| g.solids.clone()).collect();
        let cores = cores(room, map, origin, (&view, &open[..]), &walls);
        if !cores.is_empty() {
            groups.push(Group::new(Role::Wall, "the rock's cores: wall standing in the room rising away from the floor", cores));
        }
    }
    // What hangs on a wall's face above its plinth goes back with the face.
    for g in groups.iter_mut().filter(|g| g.name.starts_with("torches") || g.name.starts_with("fixed to the far wall")) {
        for s in &mut g.solids {
            if s.bottom() < PLINTH {
                continue;
            }
            let [x0, z0, x1, _] = s.footprint();
            if set_back[0] && z0 <= TILE + 2.0 && z0 >= TILE - 3.0 {
                s.at[2] -= SET_BACK;
            } else if set_back[1] && x0 <= TILE + 2.0 && x0 >= TILE - 3.0 {
                s.at[0] -= SET_BACK;
            } else if set_back[2] && x1 >= w - TILE - 2.0 && x1 <= w - TILE + 3.0 {
                s.at[0] += SET_BACK;
            }
        }
    }
}

/// Which wall of the room a wall block stands on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    North,
    West,
    East,
    South,
    Inside,
}

fn side_of(s: &Solid, w: f32, h: f32) -> Side {
    let [x0, z0, x1, z1] = s.footprint();
    if z1 <= TILE + 0.01 {
        Side::North
    } else if z0 >= h - TILE - 0.01 {
        Side::South
    } else if x1 <= TILE + 0.01 {
        Side::West
    } else if x0 >= w - TILE - 0.01 {
        Side::East
    } else {
        Side::Inside
    }
}

/// A wall block of the room's edge made a mass: its plinth over its tile,
/// its face set back above it, the mass running on out of the room, a fin
/// where one falls, a cornice along a side wall's top. A wall in the room (a
/// pillar run) steps up from a plinth. The near wall runs on low. With
/// `sun_in` (a delve's low sun, from the west) the west wall is opened.
fn wall_mass(s: &Solid, room: &Room, w: f32, h: f32, hung: &[(f32, f32)], sun_in: bool) -> Vec<Solid> {
    if s.shape != Shape::Box || s.bottom() >= 0.0 || s.turn != 0.0 {
        return vec![s.clone()];
    }
    let [x0, z0, x1, z1] = s.footprint();
    let (y0, top) = (s.bottom(), s.top());
    let kind = s.kind;
    let full = top >= 40.0;
    let mut out = Vec::new();
    match side_of(s, w, h) {
        Side::North => {
            // A plinth over the tile, then the face set back, the mass rising
            // out of sight and running on north.
            let rise = if full { FAR } else { top };
            out.push(Solid::block(kind, [x0, z0, x1, z1], y0, PLINTH.min(top) - y0));
            if !full {
                out.push(Solid::block(kind, [x0, z0 - THICK, x1, z1 - SET_BACK], y0, rise - y0));
                return out;
            }
            let fins = fins(x0, x1, room.cols);
            // The bays between the fins (and the run's ends), every other one
            // slotted, but for one beside a doorway or under what hangs there.
            let mut edges = vec![x0];
            edges.extend(fins.iter().copied());
            edges.push(x1);
            let corner = |x: f32| x <= 0.01 || x >= w - 0.01;
            let mut slots = Vec::new();
            for (k, bay) in edges.windows(2).enumerate() {
                let (lo, hi) = (bay[0] + SLOT_INSET, bay[1] - SLOT_INSET);
                let end = (k == 0 && !corner(x0)) || (k + 2 == edges.len() && !corner(x1));
                let clear = hung.iter().all(|&(a, b)| b <= lo || a >= hi);
                if k % 2 == 1 && hi - lo >= TILE && !end && clear {
                    slots.push((lo, hi));
                }
            }
            // Behind the slots, the mass full height; in front, the face
            // between them; over each slot a lintel, under it a sill.
            out.push(Solid::block(kind, [x0, z0 - THICK, x1, z1 - SLOT_DEPTH], y0, rise - y0));
            let mut from = x0;
            for &(lo, hi) in &slots {
                out.push(Solid::block(kind, [from, z1 - SLOT_DEPTH, lo, z1 - SET_BACK], y0, rise - y0));
                out.push(Solid::block(kind, [lo, z1 - SLOT_DEPTH, hi, z1 - SET_BACK], y0, SLOT_SILL - y0));
                out.push(Solid::block(kind, [lo, z1 - SLOT_DEPTH, hi, z1 - SET_BACK], SLOT_TOP, rise - SLOT_TOP));
                from = hi;
            }
            out.push(Solid::block(kind, [from, z1 - SLOT_DEPTH, x1, z1 - SET_BACK], y0, rise - y0));
            for x in fins {
                out.push(Solid::block(kind, [x - FIN / 2.0, z0 - THICK, x + FIN / 2.0, z1], y0, rise - y0));
            }
        }
        Side::West | Side::East => {
            let west = side_of(s, w, h) == Side::West;
            let (face, back) = if west { (x1, x0 - THICK) } else { (x0, x1 + THICK) };
            let inward = if west { -SET_BACK } else { SET_BACK };
            let (lo, hi) = if west { (back, face + inward) } else { (face + inward, back) };
            out.push(Solid::block(kind, [x0, z0, x1, z1], y0, PLINTH.min(top) - y0));
            let windows = if sun_in && west && full { shafts(z0, z1, room.rows) } else { Vec::new() };
            // The mass in lengths, thin about an opening (its flanks, as
            // thick as its frame, so nothing shades the way in): a sill
            // under it, a lintel over, nothing behind.
            let mut from = z0;
            for &(a, b) in &windows {
                let (ta, tb) = ((a - SHAFT_FLANK).max(z0), (b + SHAFT_FLANK).min(z1));
                out.push(Solid::block(kind, [lo, from, hi, ta], y0, top - y0));
                out.push(Solid::block(kind, [hi - SHAFT_DEPTH, ta, hi, a], y0, top - y0));
                out.push(Solid::block(kind, [hi - SHAFT_DEPTH, b, hi, tb], y0, top - y0));
                out.push(Solid::block(kind, [hi - SHAFT_DEPTH, a, hi, b], y0, SHAFT_SILL - y0));
                out.push(Solid::block(kind, [hi - SHAFT_DEPTH, a, hi, b], SHAFT_TOP, top - SHAFT_TOP));
                from = tb;
            }
            out.push(Solid::block(kind, [lo, from, hi, z1], y0, top - y0));
            if full {
                for z in fins(z0, z1, room.rows) {
                    // A fin beside an opening is only as deep as its frame.
                    let beside = windows.iter().any(|&(a, b)| z > a - SHAFT_FLANK && z < b + SHAFT_FLANK);
                    let deep = if beside { SHAFT_DEPTH } else { THICK + TILE };
                    let (flo, fhi) = if west { (face - deep, face) } else { (face, face + deep) };
                    out.push(Solid::block(kind, [flo, z - FIN / 2.0, fhi, z + FIN / 2.0], y0, top - y0));
                }
            }
        }
        Side::South => {
            // The near wall: low, running on toward the eye.
            out.push(s.clone());
            if z1 >= h - 0.01 {
                out.push(Solid::block(kind, [x0, z1, x1, z1 + THICK * 0.5], y0, top - y0));
            }
        }
        Side::Inside => out.extend(stepped(s, room)),
    }
    out
}

/// An opening in the west wall the low sun comes through: the length of
/// the wall between two fins (every other bay), a sill under it and a lintel
/// over, a frame this thick and nothing behind it.
pub const SHAFT_DEPTH: f32 = 10.0;
pub const SHAFT_SILL: f32 = PLINTH + 4.0;
pub const SHAFT_TOP: f32 = 44.0;
/// How far the thin walling runs on either side of an opening.
pub const SHAFT_FLANK: f32 = 24.0;

/// Where a wall along `[lo, hi)` is opened: the middles of every other bay
/// between its fins, a slot's inset in from each fin, and never at a run's
/// end (a doorway's, or a corner's buttress).
fn shafts(lo: f32, hi: f32, tiles: usize) -> Vec<(f32, f32)> {
    let fins = fins(lo, hi, tiles);
    let mut edges = vec![lo];
    edges.extend(fins.iter().copied());
    edges.push(hi);
    let mut out = Vec::new();
    for (k, bay) in edges.windows(2).enumerate() {
        let (a, b) = (bay[0] + SLOT_INSET, bay[1] - SLOT_INSET);
        let end = k == 0 || k + 2 == edges.len();
        if k % 2 == 1 && b - a >= TILE && !end {
            out.push((a, b));
        }
    }
    out
}

/// Where fins stand along `[lo, hi)` of a wall: at every [`FIN_EVERY`]th tile
/// edge, the room's corners not (they have their buttresses).
fn fins(lo: f32, hi: f32, tiles: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let (a, b) = ((lo / TILE).round() as i32, (hi / TILE).round() as i32);
    for k in a..=b {
        if k % FIN_EVERY == 0 && k > 1 && k < tiles as i32 - 1 {
            let x = k as f32 * TILE;
            if x - FIN / 2.0 >= lo - 0.01 && x + FIN / 2.0 <= hi + 0.01 {
                out.push(x);
            }
        }
    }
    out
}

/// A doorway's leaf set back into the wall's thickness, its lintel as deep
/// as the wall and as high, its sill as it was.
fn door_mass(s: &Solid, w: f32, _h: f32) -> Vec<Solid> {
    let [x0, z0, x1, z1] = s.footprint();
    let (y0, top) = (s.bottom(), s.top());
    if s.kind == blockout::Kind::Door {
        // North: the leaf from z -2..0 back into the slot.
        if z1 <= 0.01 && z0 < 0.0 {
            return vec![Solid::block(s.kind, [x0, -THICK, x1, -THICK + 2.0], y0, top - y0)];
        }
        if x1 <= 0.01 && x0 < 0.0 {
            return vec![Solid::block(s.kind, [-THICK, z0, -THICK + 2.0, z1], y0, top - y0)];
        }
        if x0 >= w - 0.01 {
            return vec![Solid::block(s.kind, [w + THICK - 2.0, z0, w + THICK, z1], y0, top - y0)];
        }
        return vec![s.clone()];
    }
    if s.shape == Shape::Box && y0 >= 30.0 {
        // The lintel: through the wall's whole thickness, up to its top.
        let rise = if z1 <= TILE + 0.01 { FAR } else { top };
        if z1 <= TILE + 0.01 {
            return vec![Solid::block(s.kind, [x0, -THICK, x1, z1 - SET_BACK], y0, rise - y0)];
        }
        if x1 <= TILE + 0.01 {
            return vec![Solid::block(s.kind, [-THICK, z0, x1 - SET_BACK, z1], y0, rise - y0)];
        }
        if x0 >= w - TILE - 0.01 {
            return vec![Solid::block(s.kind, [x0 + SET_BACK, z0, w + THICK, z1], y0, rise - y0)];
        }
    }
    vec![s.clone()]
}

/// A box that stands on blocked tiles, a tile or more across and more than
/// a boot high, stepped: a plinth over its whole foot, a body set in a
/// little, a slab over it as wide as the plinth. Anything else as it is.
fn stepped(s: &Solid, room: &Room) -> Vec<Solid> {
    let [x0, z0, x1, z1] = s.footprint();
    let (y0, top) = (s.bottom(), s.top());
    let big = s.shape == Shape::Box && s.turn == 0.0 && x1 - x0 >= TILE - 0.5 && z1 - z0 >= TILE - 0.5 && y0 <= 0.5 && top >= 14.0;
    // Only over tiles a knight cannot walk: what stands on open ground is
    // already boot-high, a lintel, or an arch.
    let on_blocked = {
        let (c0, r0) = ((x0 / TILE).floor() as i32, (z0 / TILE).floor() as i32);
        let (c1, r1) = (((x1 - 0.01) / TILE).floor() as i32, ((z1 - 0.01) / TILE).floor() as i32);
        (r0..=r1).all(|r| (c0..=c1).all(|c| matches!(room.tile(c, r), Tile::Block | Tile::Wall)))
    };
    if !big || !on_blocked {
        return vec![s.clone()];
    }
    let base = (top * 0.22).clamp(3.0, 7.0);
    let cap = (top * 0.14).clamp(2.0, 5.0);
    let inset = ((x1 - x0).min(z1 - z0) * 0.09).clamp(1.0, 2.5);
    vec![
        Solid::block(s.kind, [x0, z0, x1, z1], y0, base - y0),
        Solid::block(s.kind, [x0 + inset, z0 + inset, x1 - inset, z1 - inset], base, top - cap - base),
        Solid::block(s.kind, [x0, z0, x1, z1], top - cap, cap),
    ]
}

/// How high a core may rise for each pixel of the floor it stands back
/// from (north of it the camera's line falls a little more than a pixel of
/// floor for each pixel of height), and at most.
const CORE_RISE: f32 = 0.8;
const CORE_MAX: f32 = 120.0;
/// How many tiles north a core looks for the floor: past it, it rises to
/// the most.
pub const CORE_REACH: i32 = (CORE_MAX / (TILE * CORE_RISE)) as i32 + 2;

/// The rock's cores: where wall tiles stand in the room (not its edge: an
/// open floor's rock between its chambers, a passage's walls), each tile
/// rises as far as the floor north of it lets it (the camera sees over it to
/// that floor), so a wide stretch of wall stands as a great mass stepping
/// up away from the floor instead of a low plateau. Each rectangle of equal
/// height is kept only where it hides no more of the floor than the walls
/// there did.
fn cores(room: &Room, map: &Room, (ox, oy): (i32, i32), (view, open): (&View, &[(i32, i32)]), walls: &[Solid]) -> Vec<Solid> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    // Wall on the map, the room's tile `(c, r)` being the map's `(c + ox, r + oy)`.
    let wall = |c: i32, r: i32| map.tile(c + ox, r + oy) == Tile::Wall;
    let on_map = |c: i32| (0..map.cols as i32).contains(&(c + ox));
    let kind_at = |c: i32, r: i32| -> Option<blockout::Kind> {
        let (x, z) = ((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE);
        walls.iter().find(|s| {
            let [x0, z0, x1, z1] = s.footprint();
            x > x0 && x < x1 && z > z0 && z < z1 && s.bottom() <= 0.0
        }).map(|s| s.kind)
    };
    let mut heights: Vec<Option<(u32, blockout::Kind)>> = vec![None; (cols * rows).max(0) as usize];
    for r in 1..rows - 1 {
        for c in 1..cols - 1 {
            // Deep in the rock: every tile round it is wall.
            if !(-1..=1).all(|dr| (-1..=1).all(|dc| wall(c + dc, r + dr))) {
                continue;
            }
            // How far north (and to either side) the nearest floor is.
            let mut reach = None;
            for d in 1..=CORE_REACH {
                if r + oy - d < 0 {
                    break;
                }
                if (-1..=1).any(|dc| on_map(c + dc) && !wall(c + dc, r - d)) {
                    reach = Some(d);
                    break;
                }
            }
            let h = match reach {
                Some(d) => ((d - 1) as f32 * TILE * CORE_RISE).min(CORE_MAX),
                None => CORE_MAX,
            };
            // Stepped by a third of a tile, so neighbours share heights.
            let h = ((h / 16.0).floor() * 16.0) as u32;
            if h as f32 > WALL_IN_ROOM + 8.0
                && let Some(kind) = kind_at(c, r)
            {
                heights[(r * cols + c) as usize] = Some((h, kind));
            }
        }
    }
    let mut out = Vec::new();
    for ((c, r, w, h), (height, kind)) in blockout::merge(cols as usize, rows as usize, &heights) {
        let foot = blockout::px((c, r, w, h));
        let core = Solid::block(kind, foot, -blockout::SLAB, height as f32 + blockout::SLAB - CAP);
        // Its top a cap of dark slate: a mass, not a floor raised.
        let cap = Solid::block(blockout::Kind::Slate, foot, height as f32 - CAP, CAP);
        let mut with = walls.to_vec();
        with.extend([core.clone(), cap.clone()]);
        if !blockout::worst_hidden_ok(view, open, &with, walls) {
            continue;
        }
        out.extend([core, cap]);
    }
    out
}

/// A core's cap of slate, its top.
const CAP: f32 = 3.0;

/// How high the generator stands a wall in the room.
const WALL_IN_ROOM: f32 = 24.0;
