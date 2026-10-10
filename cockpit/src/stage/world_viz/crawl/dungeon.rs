//! A floor of the Delve, laid out for walking first-person. Each of the
//! floor's rooms is set in the middle of its own block of rock, and a narrow
//! corridor is cut from each doorway to the next room's, so the halls the
//! arena knows become the halls of an old-school crawl: big rooms, long dark
//! passages between. Pillars and shelves stay walls; tombs, boulders,
//! mushrooms and eye-pillars stand up as pictures.

use super::render::{Cell, Grid, Ground};
use crate::drive::together_shooter::{EnemyKind, Floor, Pack, RoomKind, Tile};

/// Rock left round each room, so every doorway opens on a passage.
const MARGIN: usize = 4;

/// What a blocking tile is on this floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prop {
    Tomb,
    Boulder,
    Mushroom,
    EyePillar,
    Crate,
    Workbench,
    Bed,
    Exhibit,
}

/// A room of the floor, in the crawl's cells.
#[derive(Clone, Debug)]
pub(crate) struct Hall {
    pub(crate) kind: RoomKind,
    /// Its floor's first and last open cells.
    pub(crate) min: (i32, i32),
    pub(crate) max: (i32, i32),
    pub(crate) roster: Vec<EnemyKind>,
    /// Neighbours through its doorways, by hall index.
    pub(crate) ways: Vec<usize>,
}

impl Hall {
    pub(crate) fn centre(&self) -> (i32, i32) {
        ((self.min.0 + self.max.0) / 2, (self.min.1 + self.max.1) / 2)
    }
}

pub(crate) struct Dungeon {
    pub(crate) grid: Grid,
    pub(crate) halls: Vec<Hall>,
    pub(crate) props: Vec<(f32, f32, Prop)>,
    // Billboard furniture stays visually open to the raycaster, but is solid
    // to feet, just like the playable Floor's Tile::Block.
    blocked: Vec<bool>,
    pub(crate) pack: Pack,
    /// Where the wall sconces burn: each one's face, just off the wall.
    pub(crate) sconces: Vec<(f32, f32)>,
}

/// What hangs on a wall face: a sconce, a banner (the kit's decal order).
pub(crate) const SCONCE: u8 = 0;
pub(crate) const BANNER: u8 = 1;

/// The sconces and banners of a dungeon: on some wall faces that look onto
/// open floor, picked by the place itself so they're always the same.
fn fixtures(
    w: usize,
    h: usize,
    cells: &[Cell],
) -> (
    std::collections::HashMap<(i32, i32, i8, i8), u8>,
    Vec<(f32, f32)>,
) {
    let open = |x: i32, y: i32| {
        x >= 0
            && y >= 0
            && (x as usize) < w
            && (y as usize) < h
            && matches!(cells[y as usize * w + x as usize], Cell::Open(_))
    };
    let mut decals = std::collections::HashMap::new();
    let mut sconces = Vec::new();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if !matches!(cells[y as usize * w + x as usize], Cell::Wall(_)) {
                continue;
            }
            let faces: Vec<(i32, i32)> = [(1, 0), (0, 1), (-1, 0), (0, -1)]
                .into_iter()
                .filter(|&(dx, dy)| open(x + dx, y + dy))
                .collect();
            let [(dx, dy)] = faces[..] else {
                continue;
            };
            let pick = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
            let pick = pick.wrapping_mul(0x9e37_79b1) >> 24;
            let kind = match pick % 100 {
                0..9 => SCONCE,
                9..12 => BANNER,
                _ => continue,
            };
            decals.insert((x, y, dx as i8, dy as i8), kind);
            if kind == SCONCE {
                sconces.push((
                    x as f32 + 0.5 + dx as f32 * 0.6,
                    y as f32 + 0.5 + dy as f32 * 0.6,
                ));
            }
        }
    }
    (decals, sconces)
}

/// Which wall texture a pack's rooms are faced with, and what its blocking
/// tiles become: a wall face (pillars, shelves) or a standing prop.
fn faces(pack: Pack) -> (u8, Result<u8, Prop>) {
    use super::textures::Wall;
    match pack {
        Pack::Crypt => (Wall::Crypt as u8, Err(Prop::Tomb)),
        Pack::Cavern => (Wall::Mine as u8, Err(Prop::Boulder)),
        Pack::Hellforge => (Wall::Keep as u8, Ok(Wall::Pillar as u8)),
        Pack::Archive => (Wall::Archive as u8, Ok(Wall::Shelf as u8)),
        Pack::Fungal => (Wall::Fungal as u8, Err(Prop::Mushroom)),
        Pack::Unknown => (Wall::Unknown as u8, Err(Prop::EyePillar)),
    }
}

impl Dungeon {
    /// The crawl's floor from a floor of the Delve.
    pub(crate) fn of(floor: &Floor) -> Dungeon {
        use super::textures::Wall;
        let (face, block) = faces(floor.pack);
        let (bw, bh) = floor
            .rooms
            .iter()
            .fold((0, 0), |(w, h), r| (w.max(r.cols), h.max(r.rows)));
        let (bw, bh) = (bw + 2 * MARGIN, bh + 2 * MARGIN);
        let (gx0, gy0) = floor.rooms.iter().fold((i32::MAX, i32::MAX), |(x, y), r| {
            (x.min(r.cell.0), y.min(r.cell.1))
        });
        let (gx1, gy1) = floor.rooms.iter().fold((i32::MIN, i32::MIN), |(x, y), r| {
            (x.max(r.cell.0), y.max(r.cell.1))
        });
        let w = (gx1 - gx0 + 1).max(1) as usize * bw;
        let h = (gy1 - gy0 + 1).max(1) as usize * bh;
        let mut cells = vec![Cell::Wall(Wall::Stone as u8); w * h];
        let mut halls = Vec::new();
        let mut props = Vec::new();
        let mut blocked = vec![false; w * h];
        // Where each room sits, and its doorways' middles.
        let mut origins = Vec::new();
        for room in &floor.rooms {
            let ox = (room.cell.0 - gx0) as usize * bw + (bw - room.cols) / 2;
            let oy = (room.cell.1 - gy0) as usize * bh + (bh - room.rows) / 2;
            origins.push((ox as i32, oy as i32));
            for row in 0..room.rows {
                for col in 0..room.cols {
                    let tile = room.tile(col as i32, row as i32);
                    blocked[(oy + row) * w + ox + col] = tile == Tile::Block;
                    let cell = match tile {
                        Tile::Wall => Cell::Wall(if room.kind == RoomKind::Home {
                            Wall::Stone as u8
                        } else {
                            face
                        }),
                        Tile::Floor | Tile::Door | Tile::Ledge => Cell::Open(Ground::Flags),
                        Tile::Stairs => Cell::Open(Ground::Stairs),
                        Tile::Hazard => Cell::Open(match floor.pack {
                            Pack::Hellforge | Pack::Unknown => Ground::Lava,
                            _ => Ground::Water,
                        }),
                        Tile::Block => match room.kind {
                            RoomKind::Stockpile | RoomKind::Workshop | RoomKind::Quarters => {
                                let prop = match room.kind {
                                    RoomKind::Stockpile => Prop::Crate,
                                    RoomKind::Workshop => Prop::Workbench,
                                    _ => Prop::Bed,
                                };
                                props.push((
                                    (ox + col) as f32 + 0.5,
                                    (oy + row) as f32 + 0.5,
                                    prop,
                                ));
                                Cell::Open(Ground::Flags)
                            }
                            RoomKind::Home => Cell::Wall(Wall::Stone as u8),
                            _ => match block {
                                Ok(face) => Cell::Wall(face),
                                Err(prop) => {
                                    props.push((
                                        (ox + col) as f32 + 0.5,
                                        (oy + row) as f32 + 0.5,
                                        prop,
                                    ));
                                    Cell::Open(Ground::Flags)
                                }
                            },
                        },
                    };
                    cells[(oy + row) * w + ox + col] = cell;
                }
            }
            halls.push(Hall {
                kind: room.kind,
                min: (ox as i32 + 1, oy as i32 + 1),
                max: ((ox + room.cols) as i32 - 2, (oy + room.rows) as i32 - 2),
                roster: room.roster.clone(),
                ways: Vec::new(),
            });
        }
        // Corridors: from each doorway's middle, straight out to the edge of
        // the room's block, then along the edge to meet the neighbour's.
        let cut = |cells: &mut Vec<Cell>, (x, y): (i32, i32)| {
            if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                let c = &mut cells[y as usize * w + x as usize];
                if matches!(c, Cell::Wall(_)) {
                    *c = Cell::Open(Ground::Flags);
                }
            }
        };
        for (i, room) in floor.rooms.iter().enumerate() {
            for dir in 0..4 {
                let Some(j) = floor.neighbour(i, dir) else {
                    continue;
                };
                halls[i].ways.push(j);
                if j < i {
                    continue;
                }
                let door = |k: usize, d: usize| {
                    let r = &floor.rooms[k];
                    let (ox, oy) = origins[k];
                    let (c, rw) = match d {
                        0 => (r.cols as i32 / 2, 0),
                        1 => (r.cols as i32 - 1, r.rows as i32 / 2),
                        2 => (r.cols as i32 / 2, r.rows as i32 - 1),
                        _ => (0, r.rows as i32 / 2),
                    };
                    (ox + c, oy + rw)
                };
                let (a, b) = (door(i, dir), door(j, (dir + 2) % 4));
                // The seam between the two blocks.
                let (ax, ay) = (
                    (room.cell.0 - gx0) * bw as i32,
                    (room.cell.1 - gy0) * bh as i32,
                );
                let seam = match dir {
                    0 => ay,
                    1 => ax + bw as i32,
                    2 => ay + bh as i32,
                    _ => ax,
                };
                // Out of one doorway to the seam, along it, and in at the
                // other: straight where they line up, a dogleg where not.
                let via = |(x, y): (i32, i32)| if dir % 2 == 1 { (seam, y) } else { (x, seam) };
                for (from, to) in [(a, via(a)), (via(a), via(b)), (via(b), b)] {
                    let (mut x, mut y) = from;
                    cut(&mut cells, (x, y));
                    while (x, y) != to {
                        if x != to.0 {
                            x += (to.0 - x).signum();
                        } else {
                            y += (to.1 - y).signum();
                        }
                        cut(&mut cells, (x, y));
                    }
                }
            }
        }
        let (decals, sconces) = fixtures(w, h, &cells);
        Dungeon {
            grid: Grid {
                w,
                h,
                cells,
                decals,
            },
            halls,
            props,
            blocked,
            pack: floor.pack,
            sconces,
        }
    }

    /// The hall a party starts in.
    pub(crate) fn start(&self) -> usize {
        self.halls
            .iter()
            .position(|h| h.kind == RoomKind::Start)
            .unwrap_or(0)
    }

    /// Whether a party can stand in a cell: open, dry, and no prop in it.
    pub(crate) fn walkable(&self, (x, y): (i32, i32)) -> bool {
        x >= 0
            && y >= 0
            && x < self.grid.w as i32
            && y < self.grid.h as i32
            && !self.blocked[y as usize * self.grid.w + x as usize]
            && matches!(
                self.grid.at(x, y),
                Cell::Open(Ground::Flags | Ground::Stairs)
            )
    }

    /// The shortest walk from `a` to `b`, cell by cell (both ends kept),
    /// along walkable cells; none if there's no way.
    pub(crate) fn path(&self, a: (i32, i32), b: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        if !self.walkable(a) || !self.walkable(b) {
            return None;
        }
        let (w, h) = (self.grid.w as i32, self.grid.h as i32);
        let index = |(x, y): (i32, i32)| (y * w + x) as usize;
        crate::drive::labyrinth::routing::shortest_path(
            (w * h) as usize,
            index(a),
            index(b),
            |at| {
                let at = (at as i32 % w, at as i32 / w);
                [(1, 0), (0, 1), (-1, 0), (0, -1)]
                    .into_iter()
                    .map(|(dx, dy)| (at.0 + dx, at.1 + dy))
                    .filter(|&next| self.walkable(next))
                    .map(|next| (index(next), 1))
                    .collect()
            },
        )
        .map(|path| {
            path.into_iter()
                .map(|at| (at as i32 % w, at as i32 / w))
                .collect()
        })
    }
}
