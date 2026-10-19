//! Open floors: a whole floor as one map to walk (`RoomKind::Depths`), its
//! chambers laid on the labyrinth's cell graph and joined by passages three
//! or four tiles wide, with no door between one fight and the next. The
//! companies wait in their chambers (`rules::depths`); the guardian's
//! chamber shuts behind the party while its guardian lives; the side rooms
//! (the Sanctuary, the treasure, the vault, the Pit, the Wanted hall, the
//! ledge, a nook) stay rooms of their own behind a door in a chamber wall.
//!
//! The map is drawn and walked as any room is, only bigger: its tiles are
//! the chambers stamped from the chamber library (the packs' templates, the
//! dungeon masters' quadrants, the pillared grid: `furnish`), the passages
//! carved through the rock between them, and rock everywhere else.

use super::*;

/// The largest open map, in tiles.
pub const OPEN_MAX_COLS: usize = 168;
pub const OPEN_MAX_ROWS: usize = 108;
/// Rock between one cell's chamber and the next, at the least.
const GAP: usize = 8;
/// The rock a chamber keeps from its cell's edge.
const MARGIN: usize = 3;
/// The most chambers, and the most doors off the map.
pub const MAX_CHAMBERS: usize = 25;
pub const MAX_SIDE_DOORS: usize = 16;
/// A crossing: a small chamber where passages meet.
const CROSSING: (usize, usize) = (20, 12);
/// The most crossings a floor has.
const CROSSINGS: usize = 2;

/// An open floor's map: which room it is, its chambers, the passages
/// between them (chamber to chamber), the doors off it, the guardian's
/// chamber and where the party comes onto it down the stairs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Open {
    pub map: usize,
    pub chambers: Vec<Chamber>,
    pub links: Vec<(u8, u8)>,
    pub doors: Vec<SideDoor>,
    pub guardian: u8,
    pub arrival: (f32, f32),
    /// Its companies have been raised (`Run::populate_open`).
    #[serde(default)]
    pub populated: bool,
}

/// A chamber on the map: its grid cell, its stone (walls included) as
/// `(col, row, cols, rows)` in the map's tiles, what it is, who waits in it
/// and how far it lies from the arrival along the passages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chamber {
    pub cell: (i32, i32),
    pub rect: (u16, u16, u16, u16),
    pub kind: RoomKind,
    pub roster: Vec<EnemyKind>,
    pub dist: u8,
    /// Its company is dead (or it never had one), and was paid for.
    #[serde(default)]
    pub cleared: bool,
    /// Shut behind the party: the guardian's, while its guardian lives.
    #[serde(default)]
    pub sealed: bool,
    /// A knight has stood in it: it is on the map.
    #[serde(default)]
    pub seen: bool,
    /// Its company has woken once.
    #[serde(default)]
    pub stirred: bool,
    /// The doorway tiles a lock-in shuts (`Tile::Door`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<(u16, u16)>,
}

impl Chamber {
    /// Its stone in arena units, walls included: `(x0, y0, x1, y1)`.
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let (c, r, w, h) = self.rect;
        (
            f32::from(c) * TILE_UNITS,
            f32::from(r) * TILE_UNITS,
            f32::from(c + w) * TILE_UNITS,
            f32::from(r + h) * TILE_UNITS,
        )
    }

    /// Whether the point stands inside its walls, `inset` units in.
    pub fn holds(&self, x: f32, y: f32, inset: f32) -> bool {
        let (x0, y0, x1, y1) = self.bounds();
        x >= x0 + TILE_UNITS + inset
            && x < x1 - TILE_UNITS - inset
            && y >= y0 + TILE_UNITS + inset
            && y < y1 - TILE_UNITS - inset
    }

    /// Its middle, in arena units.
    pub fn centre(&self) -> (f32, f32) {
        let (x0, y0, x1, y1) = self.bounds();
        ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
    }

    /// The guardian's: its stairs, or the dragon's lair.
    pub fn guarded(&self) -> bool {
        matches!(self.kind, RoomKind::Stairs | RoomKind::Lair)
    }
}

/// A door off the map: the room behind it, the chamber whose wall it is
/// in, that wall's side, and the first of its two tiles (the second lies
/// along the wall).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideDoor {
    pub room: usize,
    pub chamber: u8,
    pub dir: u8,
    pub at: (u16, u16),
}

impl SideDoor {
    /// Its two tiles.
    pub fn tiles(&self) -> [(i32, i32); 2] {
        let (c, r) = (i32::from(self.at.0), i32::from(self.at.1));
        if self.dir.is_multiple_of(2) { [(c, r), (c + 1, r)] } else { [(c, r), (c, r + 1)] }
    }

    /// Its middle, in arena units.
    pub fn middle(&self) -> (f32, f32) {
        let (c, r) = (f32::from(self.at.0), f32::from(self.at.1));
        if self.dir.is_multiple_of(2) {
            ((c + 1.0) * TILE_UNITS, (r + 0.5) * TILE_UNITS)
        } else {
            ((c + 0.5) * TILE_UNITS, (r + 1.0) * TILE_UNITS)
        }
    }

    /// Where a knight comes out of it onto the map: a step in from the
    /// wall, into the chamber.
    pub fn landing(&self) -> (f32, f32) {
        let (x, y) = self.middle();
        let (dx, dy) = DIRS[usize::from(self.dir)];
        (x - dx as f32 * 3.3, y - dy as f32 * 3.3)
    }
}

impl Open {
    /// Every chamber's stone is inside the map, none overlaps another, the
    /// doors and gates are in the map, and the counts are bounded.
    pub fn valid(&self, rooms: &[Room]) -> bool {
        let Some(map) = rooms.get(self.map) else {
            return false;
        };
        let inside = |(c, r): (i32, i32)| c >= 0 && r >= 0 && (c as usize) < map.cols && (r as usize) < map.rows;
        map.kind == RoomKind::Depths
            && !self.chambers.is_empty()
            && self.chambers.len() <= MAX_CHAMBERS
            && self.doors.len() <= MAX_SIDE_DOORS
            && self.links.len() <= 4 * MAX_CHAMBERS
            && usize::from(self.guardian) < self.chambers.len()
            && self.arrival.0.is_finite()
            && self.arrival.1.is_finite()
            && self.links.iter().all(|&(a, b)| {
                usize::from(a) < self.chambers.len() && usize::from(b) < self.chambers.len() && a != b
            })
            && self.chambers.iter().all(|ch| {
                let (c, r, w, h) = ch.rect;
                w >= 6
                    && h >= 6
                    && usize::from(c) + usize::from(w) <= map.cols
                    && usize::from(r) + usize::from(h) <= map.rows
                    && ch.roster.len() <= 64
                    && ch.gates.len() <= 16
                    && ch.gates.iter().all(|&(c, r)| inside((i32::from(c), i32::from(r))))
            })
            && self.doors.iter().all(|d| {
                d.room < rooms.len()
                    && d.room != self.map
                    && d.dir < 4
                    && usize::from(d.chamber) < self.chambers.len()
                    && d.tiles().into_iter().all(inside)
            })
            && (0..self.chambers.len()).all(|i| {
                (i + 1..self.chambers.len()).all(|j| !overlap(self.chambers[i].rect, self.chambers[j].rect))
            })
    }

    /// The chamber whose stone holds the point, if one does.
    pub fn chamber_at(&self, x: f32, y: f32) -> Option<usize> {
        self.chambers.iter().position(|ch| {
            let (x0, y0, x1, y1) = ch.bounds();
            x >= x0 && x < x1 && y >= y0 && y < y1
        })
    }

    /// The chamber nearest the point (its middle).
    pub fn nearest(&self, x: f32, y: f32) -> usize {
        self.chamber_at(x, y).unwrap_or_else(|| {
            (0..self.chambers.len())
                .min_by(|&a, &b| {
                    let d = |i: usize| {
                        let (cx, cy) = self.chambers[i].centre();
                        (cx - x).hypot(cy - y)
                    };
                    d(a).total_cmp(&d(b))
                })
                .unwrap_or(0)
        })
    }

    /// The door off the map whose tile this is.
    pub fn door_on(&self, col: i32, row: i32) -> Option<&SideDoor> {
        self.doors.iter().find(|d| d.tiles().contains(&(col, row)))
    }
}

/// Whether two rects of tiles share a tile.
fn overlap(a: (u16, u16, u16, u16), b: (u16, u16, u16, u16)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// The cells of a `gx` by `gy` grid an open floor grows into, the first
/// the middle: by turns at random (`Warrens`), along one long gallery, or
/// in compact pockets (`Karst`), as the lower tunnels grow
/// (`world::routing`).
fn grow(gx: i32, gy: i32, want: usize, variant: crate::world::routing::TunnelVariant, rng: &mut Rng) -> Vec<(i32, i32)> {
    use crate::world::routing::TunnelVariant;
    let want = want.min((gx * gy) as usize).max(2);
    let mut cells = vec![(gx / 2, gy / 2)];
    while cells.len() < want {
        let degree = |&(x, y): &(i32, i32), cells: &[(i32, i32)]| DIRS.iter().filter(|&&(dx, dy)| cells.contains(&(x + dx, y + dy))).count();
        let mut frontier: Vec<(i32, i32)> = Vec::new();
        for &(x, y) in &cells {
            for (dx, dy) in DIRS {
                let next = (x + dx, y + dy);
                if (0..gx).contains(&next.0) && (0..gy).contains(&next.1) && !cells.contains(&next) && !frontier.contains(&next) {
                    frontier.push(next);
                }
            }
        }
        if frontier.is_empty() {
            break;
        }
        match variant {
            TunnelVariant::Gallery => {
                let last = *cells.last().unwrap();
                let key = |c: &(i32, i32)| (degree(c, &cells), (c.0 - last.0).abs() + (c.1 - last.1).abs());
                let best = frontier.iter().map(key).min().unwrap();
                frontier.retain(|c| key(c) == best);
            }
            TunnelVariant::Warrens => {}
            TunnelVariant::Karst => {
                let best = frontier.iter().map(|c| degree(c, &cells)).max().unwrap();
                frontier.retain(|c| degree(c, &cells) == best);
            }
        }
        cells.push(frontier[rng.below(frontier.len())]);
    }
    cells
}

/// The passages between the cells: a maze through all of them (each cell
/// reached once, depth first, so the way winds), then a few loops more, as
/// a dungeon of the old kind has.
fn passages(cells: &[(i32, i32)], start: usize, loops: u32, rng: &mut Rng) -> Vec<(usize, usize)> {
    let at = |c: (i32, i32)| cells.iter().position(|&d| d == c);
    let mut seen = vec![false; cells.len()];
    let mut links = Vec::new();
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(&i) = stack.last() {
        let (x, y) = cells[i];
        let fresh: Vec<usize> = DIRS
            .iter()
            .filter_map(|&(dx, dy)| at((x + dx, y + dy)))
            .filter(|&j| !seen[j])
            .collect();
        if fresh.is_empty() {
            stack.pop();
            continue;
        }
        let j = fresh[rng.below(fresh.len())];
        seen[j] = true;
        links.push((i.min(j), i.max(j)));
        stack.push(j);
    }
    let mut extra: Vec<(usize, usize)> = Vec::new();
    for (i, &(x, y)) in cells.iter().enumerate() {
        for (dx, dy) in [(1, 0), (0, 1)] {
            if let Some(j) = at((x + dx, y + dy))
                && !links.contains(&(i.min(j), i.max(j)))
            {
                extra.push((i.min(j), i.max(j)));
            }
        }
    }
    let most = cells.len() / 4;
    for pair in extra {
        if links.len() >= cells.len() - 1 + most {
            break;
        }
        if rng.chance(loops) {
            links.push(pair);
        }
    }
    links
}

/// Lay out open floor `depth` of `pack` for a party of `party` knights.
/// Room 0 is where the stairs come down: the Sanctuary below the first
/// floor, else the map itself.
pub fn open_floor(depth: u32, pack: Pack, party: usize, rng: &mut Rng) -> Floor {
    use crate::world::routing::TunnelVariant;
    let party = party.clamp(1, 4);
    let fight = party_room(party);
    let guard = (fight.0.max(32), fight.1.max(20));
    let pitch = (guard.0 + GAP, guard.1 + GAP);
    let gx = (OPEN_MAX_COLS / pitch.0).clamp(2, 5) as i32;
    let gy = (OPEN_MAX_ROWS / pitch.1).clamp(2, 5) as i32;
    let variant = if depth >= 3 { TunnelVariant::for_depth(depth) } else { TunnelVariant::Warrens };
    let mut cells = grow(gx, gy, 8 + depth as usize, variant, rng);
    // The party comes in from the south: the arrival is the cell of the
    // bottom row nearest the middle (an edge cell, so its outer wall is
    // free for the stair's Sanctuary).
    let bottom = cells.iter().map(|c| c.1).max().unwrap_or(0);
    let arrival = (0..cells.len())
        .filter(|&i| cells[i].1 == bottom)
        .min_by_key(|&i| ((cells[i].0 - gx / 2).abs(), i))
        .unwrap_or(0);
    cells.swap(0, arrival);
    let loops = match variant {
        TunnelVariant::Gallery => 10,
        TunnelVariant::Warrens => 25,
        TunnelVariant::Karst => 45,
    };
    let links = passages(&cells, 0, loops, rng);
    let linked = |i: usize, j: usize| links.contains(&(i.min(j), i.max(j)));
    let at = |c: (i32, i32)| cells.iter().position(|&d| d == c);
    // Steps from the arrival along the passages.
    let mut dist = vec![usize::MAX; cells.len()];
    dist[0] = 0;
    let mut queue = VecDeque::from([0usize]);
    while let Some(i) = queue.pop_front() {
        for &(a, b) in &links {
            let j = if a == i { b } else if b == i { a } else { continue };
            if dist[j] == usize::MAX {
                dist[j] = dist[i] + 1;
                queue.push_back(j);
            }
        }
    }
    let degree = |i: usize| links.iter().filter(|&&(a, b)| a == i || b == i).count();
    let far = (1..cells.len()).max_by_key(|&i| (dist[i], degree(i) == 1, i)).unwrap_or(0);
    // Where passages only cross, now and then a crossing.
    let mut crossings: Vec<usize> = (1..cells.len())
        .filter(|&i| i != far && degree(i) >= 3 && dist[i] > 1)
        .collect();
    crossings.sort_by_key(|&i| (std::cmp::Reverse(degree(i)), i));
    crossings.truncate(CROSSINGS);
    let (cols, rows) = (gx as usize * pitch.0, gy as usize * pitch.1);
    let mut map = Room {
        cell: (-8, -8),
        kind: RoomKind::Depths,
        doors: [false; 4],
        cols,
        rows,
        tiles: vec![Tile::Wall; cols * rows],
        visited: false,
        cleared: false,
        roster: Vec::new(),
        items: Vec::new(),
        chest: None,
        laid: None,
    };
    let mut chambers: Vec<Chamber> = Vec::with_capacity(cells.len());
    let mut width = vec![0usize; links.len()];
    for w in &mut width {
        *w = if rng.chance(50) { 3 } else { 4 };
    }
    for (i, &cell) in cells.iter().enumerate() {
        let kind = if i == 0 {
            RoomKind::Start
        } else if i == far && depth < DEEPEST {
            if depth == FLOORS { RoomKind::Lair } else { RoomKind::Stairs }
        } else if crossings.contains(&i) {
            RoomKind::Hall
        } else {
            RoomKind::Fight
        };
        let (w, h) = match kind {
            RoomKind::Stairs | RoomKind::Lair => guard,
            RoomKind::Hall => CROSSING,
            _ => fight,
        };
        let spare = (pitch.0 - w, pitch.1 - h);
        let jitter = |spare: usize, rng: &mut Rng| {
            let r = (spare / 2).saturating_sub(MARGIN);
            spare / 2 - r + rng.below(2 * r + 1)
        };
        let (jx, jy) = (jitter(spare.0, rng), jitter(spare.1, rng));
        let (ox, oy) = (cell.0 as usize * pitch.0 + jx, cell.1 as usize * pitch.1 + jy);
        let doors = DIRS.map(|(dx, dy)| at((cell.0 + dx, cell.1 + dy)).is_some_and(|j| linked(i, j)));
        let template = template_for(kind, pack, rng);
        // A fight laid for one knight's room, grown for the party: its
        // company is the one room's (the party brings its own).
        let laid = match kind {
            RoomKind::Fight | RoomKind::Start => (COLS, ROWS),
            RoomKind::Stairs => (36, 20),
            RoomKind::Lair => (40, 24),
            _ => (w, h),
        };
        let mut room = furnish(cell, kind, doors, depth, pack, template, (w, h), laid, 0, rng);
        if kind == RoomKind::Start {
            room.roster.clear();
        }
        for r in 0..h {
            for c in 0..w {
                // The room's own doorways become the passages' mouths.
                let tile = match room.tile(c as i32, r as i32) {
                    Tile::Door => Tile::Wall,
                    t => t,
                };
                map.set(ox + c, oy + r, tile);
            }
        }
        chambers.push(Chamber {
            cell,
            rect: (ox as u16, oy as u16, w as u16, h as u16),
            kind,
            roster: std::mem::take(&mut room.roster),
            dist: dist[i].min(255) as u8,
            cleared: false,
            sealed: false,
            seen: false,
            stirred: false,
            gates: Vec::new(),
        });
    }
    // Rock: inside no chamber's stone.
    let rects: Vec<(u16, u16, u16, u16)> = chambers.iter().map(|c| c.rect).collect();
    let rock = |c: usize, r: usize| {
        !rects.iter().any(|&(x, y, w, h)| {
            c >= usize::from(x) && c < usize::from(x + w) && r >= usize::from(y) && r < usize::from(y + h)
        })
    };
    for (k, &(a, b)) in links.iter().enumerate() {
        let wide = width[k];
        // Always carve from the west or north one to the other.
        let (a, b) = if (cells[a].0, cells[a].1) < (cells[b].0, cells[b].1) { (a, b) } else { (b, a) };
        let across = cells[a].1 == cells[b].1;
        let (ra, rb) = (chambers[a].rect, chambers[b].rect);
        let lane = |rect: (u16, u16, u16, u16), across: bool| {
            // The first of the passage's `wide` rows (across) or columns.
            let (x, y, w, h) = (usize::from(rect.0), usize::from(rect.1), usize::from(rect.2), usize::from(rect.3));
            if across { y + h / 2 - 1 - (wide - 2) / 2 } else { x + w / 2 - 1 - (wide - 2) / 2 }
        };
        let (la, lb) = (lane(ra, across), lane(rb, across));
        let carve = |c: usize, r: usize, map: &mut Room| {
            if rock(c, r) {
                map.set(c, r, Tile::Floor);
            }
        };
        if across {
            let from = usize::from(ra.0 + ra.2);
            let to = usize::from(rb.0);
            let mid = (from + to) / 2 - wide / 2;
            for c in from..mid + wide {
                for r in la..la + wide {
                    carve(c, r, &mut map);
                }
            }
            for r in la.min(lb)..la.max(lb) + wide {
                for c in mid..mid + wide {
                    carve(c, r, &mut map);
                }
            }
            for c in mid..to {
                for r in lb..lb + wide {
                    carve(c, r, &mut map);
                }
            }
        } else {
            let from = usize::from(ra.1 + ra.3);
            let to = usize::from(rb.1);
            let mid = (from + to) / 2 - wide / 2;
            for r in from..mid + wide {
                for c in la..la + wide {
                    carve(c, r, &mut map);
                }
            }
            for c in la.min(lb)..la.max(lb) + wide {
                for r in mid..mid + wide {
                    carve(c, r, &mut map);
                }
            }
            for r in mid..to {
                for c in lb..lb + wide {
                    carve(c, r, &mut map);
                }
            }
        }
        // The mouths in both chambers' walls, the floor before them clear.
        for (ch, side, first) in [(a, if across { 1 } else { 2 }, la), (b, if across { 3 } else { 0 }, lb)] {
            let gate = chambers[ch].guarded();
            let (x, y, w, h) = chambers[ch].rect;
            let (x, y, w, h) = (usize::from(x), usize::from(y), usize::from(w), usize::from(h));
            for k in 0..wide {
                for depth_in in 0..3 {
                    let (c, r) = match side {
                        0 => (first + k, y + depth_in),
                        1 => (x + w - 1 - depth_in, first + k),
                        2 => (first + k, y + h - 1 - depth_in),
                        _ => (x + depth_in, first + k),
                    };
                    if depth_in == 0 {
                        map.set(c, r, if gate { Tile::Door } else { Tile::Floor });
                        if gate {
                            chambers[ch].gates.push((c as u16, r as u16));
                        }
                    } else if matches!(map.tile(c as i32, r as i32), Tile::Block | Tile::Hazard | Tile::Wall) {
                        map.set(c, r, Tile::Floor);
                    }
                }
            }
        }
    }
    let guardian = far;
    let mut floor = Floor {
        world_revision: 0,
        depth,
        pack,
        rooms: Vec::new(),
        secret: None,
        open: None,
    };
    let mut open = Open {
        map: 0,
        chambers,
        links: links.iter().map(|&(a, b)| (a as u8, b as u8)).collect(),
        doors: Vec::new(),
        guardian: guardian as u8,
        arrival: (0.0, 0.0),
        populated: false,
    };
    // The stair's Sanctuary below the first floor: room 0, the map after it.
    let mut taken: Vec<(i32, i32)> = cells.clone();
    if depth >= 2 {
        let side = outward(&open, 0, gx, gy, &taken).unwrap_or(2);
        let cell = step(cells[0], side);
        let mut sanctuary = room(cell, RoomKind::Sanctuary, [false; 4], depth, pack, 1, rng);
        sanctuary.open_door((side + 2) % 4);
        floor.rooms.push(sanctuary);
        taken.push(cell);
        open.map = 1;
        floor.rooms.push(map);
        cut_door(&mut floor, &mut open, 0, side, 0);
    } else {
        floor.rooms.push(map);
    }
    // The arrival: the foot of the arrival chamber, a little in.
    {
        let ch = &open.chambers[0];
        let (cx, _) = ch.centre();
        let (_, _, _, y1) = ch.bounds();
        open.arrival = (cx, y1 - TILE_UNITS - 4.5);
    }
    // The side rooms, each behind a door in a chamber's free wall: the
    // treasure off the farthest dead end, the ledge (floor one), the Pit
    // (about half the floors two to five), the Wanted hall (every floor),
    // the vault behind its crack (now and then), the Threshold (the last).
    let order: Vec<usize> = {
        let mut o: Vec<usize> = (1..open.chambers.len())
            .filter(|&i| i != guardian && open.chambers[i].kind == RoomKind::Fight)
            .collect();
        o.sort_by_key(|&i| (std::cmp::Reverse(degree(i) == 1), std::cmp::Reverse(dist[i]), i));
        o
    };
    let shape = cells.iter().fold(u64::from(depth) ^ 0x0be9, |h, c| h.wrapping_mul(37).wrapping_add((c.0 * 19 + c.1) as u64));
    let hang = |floor: &mut Floor, open: &mut Open, taken: &mut Vec<(i32, i32)>, hosts: &[usize], build: &dyn Fn((i32, i32), usize, &mut Rng) -> Room, rng: &mut Rng| -> Option<usize> {
        for &i in hosts {
            if let Some(side) = outward(open, i, gx, gy, taken) {
                let cell = step(open.chambers[i].cell, side);
                floor.rooms.push(build(cell, (side + 2) % 4, rng));
                taken.push(cell);
                let index = floor.rooms.len() - 1;
                cut_door(floor, open, index, side, i);
                return Some(index);
            }
        }
        None
    };
    let treasure_room = |cell: (i32, i32), door: usize, rng: &mut Rng| {
        let mut doors = [false; 4];
        doors[door] = true;
        room(cell, RoomKind::Treasure, doors, depth, pack, 1, rng)
    };
    hang(&mut floor, &mut open, &mut taken, &order, &treasure_room, rng);
    let deep_first: Vec<usize> = {
        let mut o: Vec<usize> = (1..open.chambers.len())
            .filter(|&i| i != guardian && open.chambers[i].kind == RoomKind::Fight)
            .collect();
        o.sort_by_key(|&i| (std::cmp::Reverse(dist[i]), i));
        o
    };
    if depth == 1 {
        hang(&mut floor, &mut open, &mut taken, &deep_first, &|c, d, _| ledge(c, d), rng);
    } else if depth < DEEPEST && super::super::mix(shape).is_multiple_of(2) {
        hang(&mut floor, &mut open, &mut taken, &deep_first, &|c, d, _| pit(c, d), rng);
    }
    hang(&mut floor, &mut open, &mut taken, &deep_first, &|c, d, _| wanted(c, d), rng);
    if depth >= DEEPEST {
        hang(&mut floor, &mut open, &mut taken, &[guardian], &|c, d, rng| {
            let mut doors = [false; 4];
            doors[d] = true;
            room(c, RoomKind::Threshold, doors, depth, pack, 1, rng)
        }, rng);
    } else if super::super::mix(shape ^ 0x5ec2e7) % 100 < SECRET_CHANCE {
        let near_first: Vec<usize> = {
            let mut o = deep_first.clone();
            o.reverse();
            o
        };
        let snibbet = super::super::mix(shape ^ 0x5a1bbe7).is_multiple_of(3);
        if let Some(vault_at) = hang(&mut floor, &mut open, &mut taken, &near_first, &|c, d, _| vault(c, d, snibbet), rng) {
            // The vault's door stays a cracked wall until a bomb finds it.
            let door = *open.doors.last().unwrap();
            for (c, r) in door.tiles() {
                floor.rooms[open.map].set(c as usize, r as usize, Tile::Wall);
            }
            floor.secret = Some(Secret {
                host: open.map,
                side: usize::from(door.dir),
                vault: vault_at,
                found: false,
                snibbet,
                paid: false,
            });
        }
    }
    seal_pockets(&mut floor.rooms[open.map], open.arrival);
    floor.open = Some(open);
    floor
}

/// The cell one step `side` of `cell`.
fn step(cell: (i32, i32), side: usize) -> (i32, i32) {
    (cell.0 + DIRS[side].0, cell.1 + DIRS[side].1)
}

/// A side of chamber `i` with no passage and no chamber or room beyond
/// it: south first, then west, east, north.
fn outward(open: &Open, i: usize, _gx: i32, _gy: i32, taken: &[(i32, i32)]) -> Option<usize> {
    let ch = &open.chambers[i];
    let used = |side: usize| {
        open.doors.iter().any(|d| usize::from(d.chamber) == i && usize::from(d.dir) == side)
            || ch.gates.iter().any(|_| false)
    };
    [2usize, 3, 1, 0].into_iter().find(|&side| {
        let beyond = step(ch.cell, side);
        !taken.contains(&beyond) && !used(side) && (-16..=16).contains(&beyond.0) && (-16..=16).contains(&beyond.1)
    })
}

/// Cut the door to room `room` in chamber `chamber`'s wall on `side`: two
/// door tiles in the wall's middle, the floor before them clear.
fn cut_door(floor: &mut Floor, open: &mut Open, room: usize, side: usize, chamber: usize) {
    let (x, y, w, h) = open.chambers[chamber].rect;
    let (x, y, w, h) = (usize::from(x), usize::from(y), usize::from(w), usize::from(h));
    let map = &mut floor.rooms[open.map];
    let at = match side {
        0 => (x + w / 2 - 1, y),
        1 => (x + w - 1, y + h / 2 - 1),
        2 => (x + w / 2 - 1, y + h - 1),
        _ => (x, y + h / 2 - 1),
    };
    let door = SideDoor {
        room,
        chamber: chamber as u8,
        dir: side as u8,
        at: (at.0 as u16, at.1 as u16),
    };
    let (dx, dy) = DIRS[side];
    for (c, r) in door.tiles() {
        map.set(c as usize, r as usize, Tile::Door);
        for k in 1..3 {
            let (ic, ir) = (c - dx * k, r - dy * k);
            if matches!(map.tile(ic, ir), Tile::Block | Tile::Hazard) {
                map.set(ic as usize, ir as usize, Tile::Floor);
            }
        }
    }
    open.doors.push(door);
}

/// Floor no knight can reach from the arrival is stone: every floor tile
/// of the map is walkable from where the party comes in.
fn seal_pockets(map: &mut Room, (ax, ay): (f32, f32)) {
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Door | Tile::Stairs);
    let (cols, rows) = (map.cols, map.rows);
    let start = ((ax / TILE_UNITS) as i32, (ay / TILE_UNITS) as i32);
    let mut seen = vec![false; cols * rows];
    if !walk(map.tile(start.0, start.1)) {
        return;
    }
    seen[start.1 as usize * cols + start.0 as usize] = true;
    let mut queue = VecDeque::from([start]);
    while let Some((c, r)) = queue.pop_front() {
        for (dc, dr) in DIRS {
            let (nc, nr) = (c + dc, r + dr);
            if !walk(map.tile(nc, nr)) {
                continue;
            }
            let i = nr as usize * cols + nc as usize;
            if !seen[i] {
                seen[i] = true;
                queue.push_back((nc, nr));
            }
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            if matches!(map.tiles[r * cols + c], Tile::Floor | Tile::Stairs) && !seen[r * cols + c] {
                map.tiles[r * cols + c] = Tile::Block;
            }
        }
    }
}
