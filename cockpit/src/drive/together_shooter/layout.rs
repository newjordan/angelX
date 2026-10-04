//! Dungeon generation: every floor is a fresh Zelda-1 dungeon of flip-screen
//! rooms, grown by a random walk from the entrance. The farthest room holds
//! the stairs (or, on the last floor, the lair), a dead end holds the
//! treasure, and every other room is a fight.
//!
//! Map packs are the reusable parts: each pack names its room templates,
//! what its obstacles and hazards are, and the monsters that live there. The
//! renderer pairs each pack with its own art style. Templates are one
//! quadrant of a room's interior, mirrored into all four, so rooms are fair
//! from every door.

use super::{EnemyKind, FLOORS, Rng};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Tiles across and down one room, walls included.
/// A screen's worth of room: the size of every ordinary room, and the
/// camera's view into a great hall.
pub(crate) const COLS: usize = 24;
pub(crate) const ROWS: usize = 14;
/// Arena units per tile.
pub(crate) const TILE_UNITS: f32 = 2.0;
/// The room grid a floor grows in.
const GRID: i32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Tile {
    Floor,
    Wall,
    /// Pillar, tomb or boulder: stops feet and shots.
    Block,
    /// Pit, water or lava: stops feet, not shots or wings.
    Hazard,
    /// A doorway; barred while the room is still being fought.
    Door,
    Stairs,
    /// A plank in a side-on room: stood on from above, jumped through from
    /// below, dropped through with down.
    Ledge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RoomKind {
    Start,
    Fight,
    Treasure,
    Stairs,
    Lair,
    /// A stone passage between chambers.
    Hall,
    /// A safe room with the reforge altar: the top of a deeper floor, and
    /// the first floor's stairs room once its guardian falls.
    Sanctuary,
    /// A side-on hall behind a marked door, off the path: gravity, planks
    /// and pits. Its one doorway is low in its west wall, whichever way
    /// the room it hangs off lies.
    Ledge,
}

/// North, east, south, west.
pub(crate) const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Pack {
    Crypt,
    Cavern,
    Hellforge,
}

impl Pack {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Pack::Crypt => "The Crypt",
            Pack::Cavern => "The Mines",
            Pack::Hellforge => "Dragon Keep",
        }
    }

    pub(crate) fn roster(self) -> &'static [(EnemyKind, u32)] {
        match self {
            Pack::Crypt => &[
                (EnemyKind::Skeleton, 4),
                (EnemyKind::Wraith, 3),
                (EnemyKind::Bat, 2),
            ],
            Pack::Cavern => &[
                (EnemyKind::Bat, 4),
                (EnemyKind::Imp, 3),
                (EnemyKind::Skeleton, 2),
            ],
            Pack::Hellforge => &[
                (EnemyKind::Imp, 4),
                (EnemyKind::Demon, 2),
                (EnemyKind::Wraith, 2),
            ],
        }
    }

    fn templates(self) -> &'static [Template] {
        match self {
            Pack::Crypt => &[PILLARS, TOMBS, COLONNADE, CROSS, POOLS],
            Pack::Cavern => &[EMPTY, PILLARS, POOLS, CROSS, COLONNADE],
            Pack::Hellforge => &[LAVA_RIVER, PILLARS, POOLS, CROSS, TOMBS],
        }
    }

    /// Caverns grow loose boulders on top of their template.
    fn scatter(self) -> usize {
        match self {
            Pack::Cavern => 4,
            _ => 0,
        }
    }
}

/// The top-left quadrant of a room's 22x12 interior: `.` floor, `#` block,
/// `~` hazard. The column next to the vertical mirror and the row next to
/// the horizontal one are the door lanes; templates keep their door ends
/// open and the flood check below guarantees every door reaches the centre.
type Template = [&'static str; 6];

const EMPTY: Template = [
    "...........",
    "...........",
    "...........",
    "...........",
    "...........",
    "...........",
];
const PILLARS: Template = [
    "...........",
    "..#....#...",
    "...........",
    "...........",
    "..#....#...",
    "...........",
];
const TOMBS: Template = [
    "...........",
    ".##..##....",
    "...........",
    ".##..##....",
    "...........",
    "...........",
];
const COLONNADE: Template = [
    "...........",
    "...........",
    "..#.#.#.#..",
    "...........",
    "...........",
    "...........",
];
const CROSS: Template = [
    "...........",
    "...........",
    "...........",
    "......#....",
    "......#....",
    "...####....",
];
const POOLS: Template = [
    "...........",
    "...........",
    "..~~~......",
    "..~~~......",
    "...........",
    "...........",
];
const LAVA_RIVER: Template = [
    "...........",
    "..~~.......",
    "...~~......",
    "....~~.....",
    "...........",
    "...........",
];
const SHRINE: Template = [
    "...........",
    "...........",
    "...........",
    "........#..",
    "...........",
    "...........",
];
const LAIR: Template = [
    "...........",
    ".#.........",
    "...~~......",
    "...~~......",
    "...........",
    "...........",
];

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Chest {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) open: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Room {
    /// Position on the floor's room grid.
    pub(crate) cell: (i32, i32),
    pub(crate) kind: RoomKind,
    /// Doorways, north/east/south/west.
    pub(crate) doors: [bool; 4],
    /// Size in tiles: a screen for most rooms, more for a great hall.
    #[serde(default = "screen_cols")]
    pub(crate) cols: usize,
    #[serde(default = "screen_rows")]
    pub(crate) rows: usize,
    tiles: Vec<Tile>,
    pub(crate) visited: bool,
    pub(crate) cleared: bool,
    /// Who waits here, spawned when the party walks in.
    pub(crate) roster: Vec<EnemyKind>,
    pub(crate) items: Vec<super::Item>,
    pub(crate) chest: Option<Chest>,
}

impl Room {
    /// What the room is built of, for a drawing kept between frames.
    pub(crate) fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (
            self.cell,
            self.kind,
            self.doors,
            self.cols,
            self.rows,
            &self.tiles,
        )
            .hash(&mut h);
        h.finish()
    }
}

fn screen_cols() -> usize {
    COLS
}

fn screen_rows() -> usize {
    ROWS
}

/// The largest great hall, in tiles.
pub(crate) const MAX_COLS: usize = 2 * COLS;
pub(crate) const MAX_ROWS: usize = 2 * ROWS;

impl Room {
    pub(super) fn valid_snapshot(&self) -> bool {
        (COLS..=MAX_COLS).contains(&self.cols)
            && (ROWS..=MAX_ROWS).contains(&self.rows)
            && self.tiles.len() == self.cols * self.rows
            && self.items.len() <= 32
            && self.roster.len() <= 64
    }

    pub(crate) fn tile(&self, col: i32, row: i32) -> Tile {
        if col < 0 || row < 0 || col >= self.cols as i32 || row >= self.rows as i32 {
            return Tile::Wall;
        }
        self.tiles[row as usize * self.cols + col as usize]
    }

    fn set(&mut self, col: usize, row: usize, tile: Tile) {
        self.tiles[row * self.cols + col] = tile;
    }

    /// Open a doorway in side `dir`, where the floor's grid had none.
    fn open_door(&mut self, dir: usize) {
        self.doors[dir] = true;
        let (cols, rows) = (self.cols, self.rows);
        let lane: [(usize, usize); 2] = match dir {
            0 => [(cols / 2 - 1, 0), (cols / 2, 0)],
            1 => [(cols - 1, rows / 2 - 1), (cols - 1, rows / 2)],
            2 => [(cols / 2 - 1, rows - 1), (cols / 2, rows - 1)],
            _ => [(0, rows / 2 - 1), (0, rows / 2)],
        };
        for (col, row) in lane {
            self.set(col, row, Tile::Door);
        }
    }

    /// The doorway out of a side-on hall: the one way back.
    pub(crate) fn way_back(&self) -> Option<usize> {
        (self.kind == RoomKind::Ledge).then(|| self.doors.iter().position(|&d| d))?
    }

    /// Where a Sanctuary's privy stands: the first clear two-by-two corner
    /// from the north-west, its door facing south. The point is its door.
    pub(crate) fn privy(&self) -> Option<(f32, f32)> {
        if self.kind != RoomKind::Sanctuary {
            return None;
        }
        let clear = |c: usize, r: usize| self.tile(c as i32, r as i32) == Tile::Floor;
        (2..self.rows / 2).find_map(|row| {
            (2..self.cols / 2 - 2).find_map(|col| {
                (clear(col, row)
                    && clear(col + 1, row)
                    && clear(col, row + 1)
                    && clear(col + 1, row + 1))
                .then(|| ((col + 1) as f32 * TILE_UNITS, (row + 2) as f32 * TILE_UNITS))
            })
        })
    }

    /// Where a Sanctuary's altar stands: the middle, or above the stairs
    /// when the room keeps them.
    pub(crate) fn altar(&self) -> (f32, f32) {
        let (x, y) = (self.width() / 2.0, self.height() / 2.0);
        if self.tiles.contains(&Tile::Stairs) {
            (x, y - 6.0)
        } else {
            (x, y)
        }
    }

    /// Size in arena units.
    pub(crate) fn width(&self) -> f32 {
        self.cols as f32 * TILE_UNITS
    }

    pub(crate) fn height(&self) -> f32 {
        self.rows as f32 * TILE_UNITS
    }

    /// Bigger than one screen: the camera follows the knights.
    pub(crate) fn great(&self) -> bool {
        self.cols > COLS || self.rows > ROWS
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Floor {
    pub(crate) depth: u32,
    pub(crate) pack: Pack,
    pub(crate) rooms: Vec<Room>,
}

impl Floor {
    /// The room through door `dir` of room `from`: none where that side
    /// has no doorway (a side-on hall may sit beside rooms it never opens
    /// into).
    pub(crate) fn neighbour(&self, from: usize, dir: usize) -> Option<usize> {
        if !self.rooms[from].doors[dir] {
            return None;
        }
        let (x, y) = self.rooms[from].cell;
        let (dx, dy) = DIRS[dir];
        self.rooms.iter().position(|r| r.cell == (x + dx, y + dy))
    }
}

/// Lay out floor `depth` of the given pack.
pub(crate) fn floor(depth: u32, pack: Pack, rng: &mut Rng) -> Floor {
    // Chambers, and the passages that join them.
    let want = 5 + depth as usize + 3;
    let mut cells = vec![(GRID / 2, GRID / 2)];
    while cells.len() < want {
        let (x, y) = cells[rng.below(cells.len())];
        let (dx, dy) = DIRS[rng.below(4)];
        let next = (x + dx, y + dy);
        if (0..GRID).contains(&next.0) && (0..GRID).contains(&next.1) && !cells.contains(&next) {
            cells.push(next);
        }
    }
    let links = |&(x, y): &(i32, i32)| DIRS.map(|(dx, dy)| cells.contains(&(x + dx, y + dy)));
    // Breadth-first distance from the entrance picks the far rooms.
    let mut dist = vec![usize::MAX; cells.len()];
    dist[0] = 0;
    let mut queue = VecDeque::from([0]);
    while let Some(i) = queue.pop_front() {
        let (x, y) = cells[i];
        for (dx, dy) in DIRS {
            if let Some(j) = cells.iter().position(|&c| c == (x + dx, y + dy))
                && dist[j] == usize::MAX
            {
                dist[j] = dist[i] + 1;
                queue.push_back(j);
            }
        }
    }
    let far = (1..cells.len()).max_by_key(|&i| (dist[i], i)).unwrap_or(0);
    let dead_end = |i: usize| links(&cells[i]).iter().filter(|&&l| l).count() == 1;
    let treasure = (1..cells.len())
        .filter(|&i| i != far)
        .max_by_key(|&i| (dead_end(i), dist[i], i));
    // Plain cells that only pass through become halls: up to three a floor,
    // never the entrance's neighbour, so a run opens on a chamber.
    let through = |i: usize| links(&cells[i]).iter().filter(|&&l| l).count() == 2;
    let mut halls: Vec<usize> = (1..cells.len())
        .filter(|&i| i != far && Some(i) != treasure && through(i) && dist[i] > 1)
        .collect();
    halls.sort_by_key(|&i| (std::cmp::Reverse(dist[i]), i));
    halls.truncate(3);
    let rooms = cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let kind = if i == 0 && depth >= 2 {
                RoomKind::Sanctuary
            } else if i == 0 {
                RoomKind::Start
            } else if i == far {
                if depth >= FLOORS {
                    RoomKind::Lair
                } else {
                    RoomKind::Stairs
                }
            } else if Some(i) == treasure {
                RoomKind::Treasure
            } else if halls.contains(&i) {
                RoomKind::Hall
            } else {
                RoomKind::Fight
            };
            room(*cell, kind, links(cell), depth, pack, rng)
        })
        .collect();
    let mut floor = Floor { depth, pack, rooms };
    if depth == 1 {
        add_ledge(&mut floor, &dist, far, treasure, &halls);
    }
    floor
}

/// The side-on hall: hung off the deepest fight room that has a free cell
/// beside it touching nothing else, so it stays a dead end off the path.
/// It draws nothing from the floor's dice, so every other room is as it
/// would be without it.
fn add_ledge(
    floor: &mut Floor,
    dist: &[usize],
    far: usize,
    treasure: Option<usize>,
    halls: &[usize],
) {
    let taken = |c: (i32, i32)| floor.rooms.iter().any(|r| r.cell == c);
    let mut parents: Vec<usize> = (1..floor.rooms.len())
        .filter(|&i| i != far && Some(i) != treasure && !halls.contains(&i))
        .filter(|&i| floor.rooms[i].kind == RoomKind::Fight)
        .collect();
    parents.sort_by_key(|&i| (std::cmp::Reverse(dist[i]), i));
    // A cell touching only its parent first; else any free cell, its
    // other sides blank walls. It may hang just past the floor's grid.
    let near = |v: i32| (-1..=GRID).contains(&v);
    let mut spots: Vec<(bool, usize, usize)> = Vec::new();
    for (rank, &i) in parents.iter().enumerate() {
        let (x, y) = floor.rooms[i].cell;
        for (d, (dx, dy)) in DIRS.into_iter().enumerate() {
            let cell = (x + dx, y + dy);
            if !near(cell.0) || !near(cell.1) || taken(cell) {
                continue;
            }
            let touching = DIRS
                .iter()
                .filter(|(ex, ey)| taken((cell.0 + ex, cell.1 + ey)))
                .count();
            spots.push((touching > 1, rank, d));
        }
    }
    spots.sort_unstable();
    for (_, rank, d) in spots {
        let i = parents[rank];
        let (x, y) = floor.rooms[i].cell;
        let cell = (x + DIRS[d].0, y + DIRS[d].1);
        let mut parent = floor.rooms[i].clone();
        parent.open_door(d);
        if !connected(&parent) {
            continue;
        }
        floor.rooms[i] = parent;
        floor.rooms.push(ledge(cell, (d + 2) % 4));
        return;
    }
}

/// The side-on hall, west to east: a low doorway, steps and planks to
/// climb, two pits to leap. `#` stone, `=` plank, `~` pit, `D` doorway.
const LEDGE: [&str; ROWS] = [
    "################################################",
    "#..............................................#",
    "#..............................................#",
    "#.........................=====................#",
    "#..............................................#",
    "#.................=====............=====.......#",
    "#..............................................#",
    "#..........=====.....................=====.....#",
    "#..............................................#",
    "D......................######..................#",
    "D......................######..............#####",
    "#########~~~###########################~~~######",
    "#########~~~###########################~~~######",
    "################################################",
];

/// The side-on hall at `cell`, its one doorway leading back through
/// `door` (the side its parent room lies on).
fn ledge(cell: (i32, i32), door: usize) -> Room {
    let cols = LEDGE[0].len();
    let mut doors = [false; 4];
    doors[door] = true;
    let mut room = Room {
        cell,
        kind: RoomKind::Ledge,
        doors,
        cols,
        rows: ROWS,
        tiles: vec![Tile::Floor; cols * ROWS],
        visited: false,
        cleared: false,
        roster: vec![
            EnemyKind::Skeleton,
            EnemyKind::Bat,
            EnemyKind::Skeleton,
            EnemyKind::Bat,
            EnemyKind::Bat,
        ],
        items: Vec::new(),
        chest: None,
    };
    for (row, line) in LEDGE.iter().enumerate() {
        for (col, b) in line.bytes().enumerate() {
            let tile = match b {
                b'#' => Tile::Block,
                b'=' => Tile::Ledge,
                b'~' => Tile::Hazard,
                b'D' => Tile::Door,
                _ => Tile::Floor,
            };
            room.set(col, row, tile);
        }
    }
    room
}

fn room(
    cell: (i32, i32),
    kind: RoomKind,
    doors: [bool; 4],
    depth: u32,
    pack: Pack,
    rng: &mut Rng,
) -> Room {
    let template = match kind {
        RoomKind::Start | RoomKind::Hall | RoomKind::Sanctuary | RoomKind::Ledge => EMPTY,
        RoomKind::Treasure => SHRINE,
        RoomKind::Lair => LAIR,
        RoomKind::Fight | RoomKind::Stairs => {
            let templates = pack.templates();
            templates[rng.below(templates.len())]
        }
    };
    // Guardians and the dragon keep great halls; some fights do too.
    let (cols, rows) = match kind {
        RoomKind::Stairs => (36, 20),
        RoomKind::Lair => (40, 24),
        RoomKind::Fight if depth >= 2 && rng.chance(12) => (MAX_COLS, MAX_ROWS),
        RoomKind::Fight if rng.chance(30) => (32, 20),
        _ => (COLS, ROWS),
    };
    let mut quadrant: Vec<Vec<u8>> = if cols == COLS && rows == ROWS {
        template.iter().map(|r| r.bytes().collect()).collect()
    } else {
        great_quadrant(cols / 2 - 1, rows / 2 - 1, kind, rng)
    };
    if matches!(kind, RoomKind::Fight | RoomKind::Stairs) && cols == COLS {
        for _ in 0..pack.scatter() {
            let (qx, qy) = (1 + rng.below(8), 1 + rng.below(4));
            quadrant[qy][qx] = b'#';
        }
    }
    let mut room = Room {
        cell,
        kind,
        doors,
        cols,
        rows,
        tiles: vec![Tile::Floor; cols * rows],
        visited: false,
        cleared: matches!(kind, RoomKind::Treasure | RoomKind::Sanctuary),
        roster: Vec::new(),
        items: Vec::new(),
        chest: None,
    };
    let build = |room: &mut Room, quadrant: &[Vec<u8>]| {
        // One quadrant, mirrored both ways: doorway lanes meet in the middle.
        let (qw, qh) = (cols / 2 - 1, rows / 2 - 1);
        for row in 0..rows {
            for col in 0..cols {
                let edge = row == 0 || col == 0 || row == rows - 1 || col == cols - 1;
                let tile = if edge {
                    let lane_x = col == cols / 2 - 1 || col == cols / 2;
                    let lane_y = row == rows / 2 - 1 || row == rows / 2;
                    let door = (row == 0 && lane_x && doors[0])
                        || (col == cols - 1 && lane_y && doors[1])
                        || (row == rows - 1 && lane_x && doors[2])
                        || (col == 0 && lane_y && doors[3]);
                    if door { Tile::Door } else { Tile::Wall }
                } else {
                    let qx = if col <= qw { col - 1 } else { cols - 2 - col };
                    let qy = if row <= qh { row - 1 } else { rows - 2 - row };
                    match quadrant[qy][qx] {
                        b'#' => Tile::Block,
                        b'~' => Tile::Hazard,
                        _ => Tile::Floor,
                    }
                };
                room.set(col, row, tile);
            }
        }
    };
    build(&mut room, &quadrant);
    if !connected(&room) {
        let open = vec![vec![b'.'; cols / 2 - 1]; rows / 2 - 1];
        build(&mut room, &open);
    }
    let centre = (cols / 2 - 1, rows / 2 - 1);
    match kind {
        RoomKind::Stairs => {
            for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                room.set(centre.0 + dc, centre.1 + dr, Tile::Stairs);
            }
        }
        RoomKind::Hall => {
            // Stone everywhere but the passages from each doorway to a hub.
            let wide = if rng.chance(50) { 2 } else { 3 };
            let (mc, mr) = (cols as i32 / 2, rows as i32 / 2);
            let hub = |c: i32, r: i32| (c - mc).abs() <= wide + 1 && (r - mr).abs() <= wide;
            let lane = |c: i32, r: i32| {
                let across = (c - mc).abs() < wide || c == mc - wide;
                let along = (r - mr).abs() < wide || r == mr - wide;
                (doors[0] && across && r <= mr)
                    || (doors[2] && across && r >= mr)
                    || (doors[1] && along && c >= mc)
                    || (doors[3] && along && c <= mc)
            };
            for row in 1..rows - 1 {
                for col in 1..cols - 1 {
                    let (c, r) = (col as i32, row as i32);
                    if !hub(c, r) && !lane(c, r) {
                        room.set(col, row, Tile::Wall);
                    }
                }
            }
        }
        RoomKind::Treasure => {
            room.chest = Some(Chest {
                x: room.width() / 2.0,
                y: room.height() / 2.0,
                open: false,
            });
        }
        _ => {}
    }
    let foes = (2 + depth as usize + rng.below(2)).min(6);
    room.roster = match kind {
        RoomKind::Start => vec![EnemyKind::Skeleton, EnemyKind::Skeleton],
        RoomKind::Treasure | RoomKind::Sanctuary | RoomKind::Ledge => Vec::new(),
        // A passage may hold a straggler or two that fly.
        RoomKind::Hall => (0..rng.below(3)).map(|_| EnemyKind::Bat).collect(),
        RoomKind::Lair => vec![EnemyKind::Dragon, EnemyKind::Imp, EnemyKind::Imp],
        // A great hall holds a bigger company.
        RoomKind::Fight => (0..foes + (cols * rows) / (COLS * ROWS * 2))
            .map(|_| rng.pick(pack.roster()))
            .collect(),
        RoomKind::Stairs => (0..foes + 1).map(|_| rng.pick(pack.roster())).collect(),
    };
    room
}

/// Every doorway and the centre reachable on foot.
fn connected(room: &Room) -> bool {
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Door | Tile::Stairs);
    let (cols, rows) = (room.cols, room.rows);
    let start = (cols as i32 / 2, rows as i32 / 2);
    let mut seen = vec![false; cols * rows];
    let mut queue = VecDeque::from([start]);
    seen[start.1 as usize * cols + start.0 as usize] = true;
    while let Some((c, r)) = queue.pop_front() {
        for (dc, dr) in DIRS {
            let (nc, nr) = (c + dc, r + dr);
            let tile = room.tile(nc, nr);
            if !walk(tile) {
                continue;
            }
            let i = nr as usize * cols + nc as usize;
            if !seen[i] {
                seen[i] = true;
                queue.push_back((nc, nr));
            }
        }
    }
    (0..rows)
        .all(|r| (0..cols).all(|c| room.tiles[r * cols + c] != Tile::Door || seen[r * cols + c]))
}

/// A great hall's quadrant (`w` by `h` tiles, mirrored into the whole room):
/// rows of pillars with gaps, a pool or two, and the lanes to the middle
/// and to each doorway kept open.
fn great_quadrant(w: usize, h: usize, kind: RoomKind, rng: &mut Rng) -> Vec<Vec<u8>> {
    let mut q = vec![vec![b'.'; w]; h];
    let (step_x, step_y) = (4 + rng.below(2), 3 + rng.below(2));
    for y in (2..h.saturating_sub(2)).step_by(step_y) {
        for x in (2..w.saturating_sub(2)).step_by(step_x) {
            if rng.chance(80) {
                q[y][x] = b'#';
                if rng.chance(35) && x + 1 < w - 2 {
                    q[y][x + 1] = b'#';
                }
            }
        }
    }
    let pools = if kind == RoomKind::Lair {
        2
    } else {
        rng.below(2)
    };
    for _ in 0..pools {
        let (px, py) = (
            2 + rng.below(w.saturating_sub(7).max(1)),
            2 + rng.below(h.saturating_sub(6).max(1)),
        );
        for row in q.iter_mut().take((py + 2).min(h - 2)).skip(py) {
            for cell in row.iter_mut().take((px + 3).min(w - 2)).skip(px) {
                *cell = b'~';
            }
        }
    }
    // The middle crossing and the doorway lanes stay clear.
    for row in q.iter_mut() {
        row.iter_mut()
            .skip(w.saturating_sub(2))
            .for_each(|c| *c = b'.');
    }
    for row in q.iter_mut().skip(h.saturating_sub(2)) {
        row.iter_mut().for_each(|c| *c = b'.');
    }
    q
}
