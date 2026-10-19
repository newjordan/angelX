//! Dungeon generation: every floor is a fresh Zelda-1 dungeon of flip-screen
//! rooms. The first two floors retain their approach passages; below those,
//! a bounded labyrinth frontier grows gallery, warren and karst variants.
//! The farthest room holds
//! the stairs (or, on the last floor, the lair), a dead end holds the
//! treasure, and every other room is a fight.
//!
//! Map packs are the reusable parts: each pack names its room templates,
//! what its obstacles and hazards are, and the monsters that live there. The
//! renderer pairs each pack with its own art style. Templates are one
//! quadrant of a room's interior, mirrored into all four, so rooms are fair
//! from every door.

use super::{DEEPEST, EnemyKind, FLOORS, Rng};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub mod open;
pub use open::{Chamber, OPEN_MAX_COLS, OPEN_MAX_ROWS, Open, SideDoor, open_floor};

/// Tiles across and down one room, walls included.
/// A screen's worth of room: the size of every ordinary room, and the
/// camera's view into a great hall.
pub const COLS: usize = 24;
pub const ROWS: usize = 14;
/// Arena units per tile.
pub const TILE_UNITS: f32 = 2.0;
/// The room grid a floor grows in.
const GRID: i32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tile {
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
    /// Mire: walkable, but it holds the feet. A knight or a walking monster
    /// wades it at `swamp::BOG_PACE`; shots and wings pass over it.
    Bog,
}

impl Tile {
    /// Ground a body can be set down on: dry floor, or the swamp's bog.
    pub fn ground(self) -> bool {
        matches!(self, Tile::Floor | Tile::Bog)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
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
    /// The Undercroft: the company's home, where the Winding Stair begins.
    Home,
    /// Dame Fortune's hall beside it, where the wheel picks the delve.
    Fortune,
    /// The bottom of the Unknown: the Shoggoth's threshold, and the Grail.
    Threshold,
    /// The Training Yard north of the Undercroft: quintains, and a stall.
    Yard,
    /// The Trophy Hall south of the Undercroft: what the realm has slain.
    Trophies,
    /// The Pit, a dead end on the floors below the first: the Pit Tyrant's.
    Pit,
    /// A vault behind a cracked wall, off no map until a bomb finds it.
    Secret,
    /// Maud's tavern, the Siege Perilous, dug out west of the Trophy Hall.
    Tavern,
    /// Loop miners' stores and furnished workshops in the connected home wing.
    Stockpile,
    Workshop,
    Quarters,
    /// The world above the Delve (see `world`): the Delve's gate courtyard,
    /// the stables, the lists, and the Mines' mouth.
    Gate,
    Stables,
    Lists,
    MineHead,
    /// King Brannoc's hall, inside: reached by its great door at the
    /// mine-head once it is rebuilt.
    KingsHall,
    /// A woodland clearing south of the gate, with co-operative games.
    Forest,
    /// The walkable surface circuit: courier street, festival green, river,
    /// working countryside, old ruins and a place the party can build.
    Village,
    Fairground,
    Riverbank,
    Windmill,
    Ruins,
    Meadow,
    /// The lamplighter's lower road: an orchard and a marsh ferry landing.
    Orchard,
    Marsh,
    /// Goat pastures and a safely railed western headland.
    Downs,
    Cliffs,
    /// The eastern picnic road and a priory that breeds postal pigeons.
    Causeway,
    Priory,
    /// The northern ridge, postal rookery and improbable weather observatory.
    Crag,
    Rookery,
    Observatory,
    /// The coastal footpath, crabby salt works and a lighthouse peninsula.
    Strand,
    Saltworks,
    Lighthouse,
    Timberyard,
    HedgeMaze,
    Bothy,
    Millpond,
    Sluice,
    SunkenAbbey,
    Belfry,
    Aqueduct,
    Aviary,
    Mummers,
    Laundry,
    Caravan,
    /// The free northern race circuit, added without renumbering save masks.
    Hippodrome,
    Wheelwright,
    TollBridge,
    GooseGreen,
    Kitewright,
    SignalHill,
    /// The College of the Labyrinth and Bartosz's tower.
    CollegeGrounds,
    CollegeHall,
    CollegeClass,
    CollegeLibrary,
    CollegeLab,
    TowerEntry,
    TowerStairs,
    TowerStars,
    WizardStudy,
    /// The horse ranch beside the stables, and the flower fields beyond.
    Ranch,
    Pasture,
    /// A forge-hall of Caer Dwfn, off the first hall of a reclaimed floor.
    Forge,
    /// The Wanted hall: a side-on showdown with the floor's outlaw, a dead
    /// end off the floor like the Pit (see `outlaw`).
    Wanted,
    /// The continuous surface shared with the realm overview.
    Kingdom,
    /// A landmark or model castle's separate hall.
    RealmHall,
    /// The Court of Deeds by the Keep, where holdings are bought and run.
    LandOffice,
    /// A held floor's steward's hall, its ten bays as built (`holdings`).
    Holding,
    /// Longbow country (`bowcountry`): the archery green, the fletcher's
    /// yard and the scarecrow's field.
    LongbowGreen,
    Fletchery,
    Scarecrow,
    /// The town's rooms (`town`): the market square behind the Artisan
    /// Quarter's door, the bathhouse, and the smithy behind the Smithy's.
    Market,
    Bathhouse,
    Smithy,
    /// Nooks below ground (`nooks`): lived-in dead ends off a fight room.
    Kitchen,
    Reading,
    Bellows,
    Ossuary,
    Still,
    Loft,
    /// The Griffin Eyrie on the Mountain: free practice flights of the
    /// Griffin Run (`race3d`).
    Eyrie,
    /// An open floor's one map (`open`): its chambers and the passages
    /// between them, explored on foot, no door between one fight and the
    /// next.
    Depths,
    /// Siege muster house: personal rampart practice inside the north court.
    Muster,
    /// The Colosseum (`colosseum`): its entrance hall with two arched
    /// gates, the Powder Keg yard through one and the Gander Joust hall
    /// through the other. Appended, so saved names and masks keep.
    ColosseumHall,
    KegYard,
    GanderHall,
}

impl RoomKind {
    /// Shared labels for the connected loop wing in both Delve views.
    pub fn settlement_label(self) -> &'static str {
        match self {
            Self::Home => "PLAYER HALL",
            Self::Stockpile => "STOCKPILE",
            Self::Workshop => "WORKSHOP",
            Self::Muster => "SIEGE MUSTER",
            Self::Quarters => "QUARTERS",
            _ => "EXCAVATION",
        }
    }
}

/// North, east, south, west.
pub const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pack {
    #[default]
    Crypt,
    Cavern,
    Hellforge,
    /// Floor four, below Dragon Keep: a library the water got into.
    Archive,
    /// Floor five: a forest of mushrooms grown in the dark.
    Fungal,
    /// Floor six: the Unknown, where the Shoggoth waits.
    Unknown,
    /// The Swamp of Horrors: a delve of its own, three floors of mire under
    /// Sir Damp's Marsh (`swamp`), the Mire King and the white horse at the
    /// bottom.
    Swamp,
}

impl Pack {
    pub fn name(self) -> &'static str {
        match self {
            Pack::Crypt => "The Crypt",
            Pack::Cavern => "The Mines",
            Pack::Hellforge => "Dragon Keep",
            Pack::Archive => "The Drowned Archive",
            Pack::Fungal => "The Fungal Deep",
            Pack::Unknown => "The Unknown",
            Pack::Swamp => "The Swamp of Horrors",
        }
    }

    /// The pack of floor `depth` of a delve whose first floor is `first`:
    /// the Crypt and the Mines, Dragon Keep, then the deep.
    pub fn at(depth: u32, first: Pack) -> Pack {
        // The swamp is a delve of its own: mire all the way down.
        if first == Pack::Swamp {
            return Pack::Swamp;
        }
        let other = if first == Pack::Crypt {
            Pack::Cavern
        } else {
            Pack::Crypt
        };
        match depth {
            0 | 1 => first,
            2 => other,
            3 => Pack::Hellforge,
            4 => Pack::Archive,
            5 => Pack::Fungal,
            _ => Pack::Unknown,
        }
    }

    /// The old delve a deep one shares its materials and music with.
    pub fn kin(self) -> Pack {
        match self {
            Pack::Archive => Pack::Crypt,
            Pack::Fungal => Pack::Cavern,
            Pack::Unknown => Pack::Hellforge,
            pack => pack,
        }
    }

    /// Who lives on a floor of this pack. The second company comes in by
    /// depth: the first floor meets only its gentlest, so it can teach.
    pub fn roster_at(self, depth: u32) -> &'static [(EnemyKind, u32)] {
        use EnemyKind::*;
        match (self, depth) {
            (Pack::Crypt, 0 | 1) => &[(Skeleton, 4), (Wraith, 3), (Bat, 2), (Slime, 1), (Biter, 2)],
            (Pack::Crypt, _) => &[
                (Skeleton, 4),
                (Wraith, 3),
                (Bat, 2),
                (Slime, 2),
                (Necromancer, 1),
                (Hexer, 1),
                (Biter, 1),
                (Pavise, 2),
                (Marksman, 1),
                (Mirrorling, 1),
                (Ringer, 1),
            ],
            (Pack::Cavern, 0 | 1) => &[
                (Bat, 4),
                (Imp, 3),
                (Skeleton, 2),
                (Sapper, 1),
                (Biter, 2),
                (Mole, 1),
            ],
            (Pack::Cavern, _) => &[
                (Bat, 3),
                (Imp, 2),
                (Skeleton, 2),
                (Sapper, 2),
                (Hob, 2),
                (Warboar, 1),
                (Mole, 2),
                (Geode, 1),
                (Whirlgrave, 1),
                (Leech, 1),
                (Biter, 1),
                (Spitter, 1),
            ],
            (Pack::Hellforge, _) => &[
                (Imp, 3),
                (Demon, 2),
                (Wraith, 2),
                (Warboar, 2),
                (Shaman, 1),
                (Hob, 1),
                (Hound, 2),
                (Pavise, 1),
                (Spitter, 1),
                (Marksman, 1),
            ],
            // The Archive's stacks have spiders in them.
            (Pack::Archive, _) => &[
                (Wraith, 3),
                (Skeleton, 2),
                (Slime, 2),
                (Bat, 2),
                (Sapper, 1),
                (Necromancer, 1),
                (Silkmother, 1),
                (Lich, 1),
                (Mirrorling, 2),
                (Ringer, 1),
                (Marksman, 1),
                (Leech, 1),
                (Biter, 1),
            ],
            (Pack::Fungal, _) => &[
                (Slime, 3),
                (Bat, 3),
                (Hob, 1),
                (Warboar, 2),
                (Imp, 1),
                (Shaman, 1),
                (Silkmother, 1),
                (Flesher, 1),
                (Broodmarrow, 1),
                (Whirlgrave, 1),
                (Hexer, 1),
                (Leech, 2),
                (Spitter, 1),
                (Mole, 1),
                (Geode, 1),
                (Biter, 1),
            ],
            // The Unknown: echoes of everything met on the way down.
            // The swamp: the first floor meets the gentle things of the mire
            // (leeches, the hands under the bog, a wraith over the water); the
            // drowned knights, the wisps' lure and the casters come lower.
            (Pack::Swamp, 0 | 1) => &[
                (Leech, 4),
                (BogHand, 3),
                (Wraith, 2),
                (Wisp, 1),
                (Slime, 1),
            ],
            (Pack::Swamp, 2) => &[
                (Leech, 3),
                (BogHand, 3),
                (Wraith, 2),
                (Wisp, 2),
                (Slime, 1),
                (Spitter, 1),
                (Drowned, 1),
                (Hexer, 1),
            ],
            (Pack::Swamp, _) => &[
                (Leech, 2),
                (BogHand, 3),
                (Wraith, 2),
                (Wisp, 2),
                (Drowned, 2),
                (Slime, 1),
                (Spitter, 1),
                (Hexer, 1),
                (Whirlgrave, 1),
            ],
            (Pack::Unknown, _) => &[
                (Wraith, 2),
                (Imp, 2),
                (Demon, 2),
                (Slime, 2),
                (Warboar, 1),
                (Necromancer, 1),
                (Shaman, 1),
                (Flesher, 1),
                (Broodmarrow, 1),
                (Oathbound, 2),
                (Whirlgrave, 1),
                (Hexer, 1),
                (Lich, 1),
                (Hollow, 1),
                (Mirrorling, 1),
                (Marksman, 1),
                (Hound, 1),
                (Pavise, 1),
                (Geode, 1),
            ],
        }
    }

    /// How many dangerous kinds a room holds at once: two, three in the
    /// deep.
    pub fn dangers_at(depth: u32) -> usize {
        if depth > FLOORS { 3 } else { 2 }
    }

    /// What a room or a wave brings instead of a second of a dangerous
    /// kind: the floor's first plain monster.
    pub fn plain_at(self, depth: u32) -> EnemyKind {
        self.roster_at(depth)
            .iter()
            .map(|&(kind, _)| kind)
            .find(|kind| !kind.elite())
            .unwrap_or(EnemyKind::Bat)
    }

    fn templates(self) -> &'static [Template] {
        match self {
            Pack::Crypt => &[PILLARS, TOMBS, COLONNADE, CROSS, POOLS],
            Pack::Cavern => &[EMPTY, PILLARS, POOLS, CROSS, COLONNADE],
            Pack::Hellforge => &[LAVA_RIVER, PILLARS, POOLS, CROSS, TOMBS],
            Pack::Archive => &[STACKS, COLONNADE, POOLS, STACKS, CROSS],
            Pack::Fungal => &[EMPTY, POOLS, PILLARS, LAVA_RIVER, COLONNADE],
            Pack::Unknown => &[EMPTY, PILLARS, CROSS, POOLS, LAVA_RIVER],
            // Reed beds and black channels; the mire itself is laid over
            // whichever is drawn (`swamp::mire`).
            Pack::Swamp => &[REEDBEDS, POOLS, LAVA_RIVER, REEDBEDS, PILLARS],
        }
    }

    /// Caverns grow loose boulders on top of their template.
    pub(crate) fn scatter(self) -> usize {
        match self {
            Pack::Cavern | Pack::Fungal => 4,
            // Tussocks and stumps.
            Pack::Swamp => 2,
            _ => 0,
        }
    }
}

/// The top-left quadrant of a room's 22x12 interior: `.` floor, `#` block,
/// `~` hazard. The column next to the vertical mirror and the row next to
/// the horizontal one are the door lanes; templates keep their door ends
/// open and the flood check below guarantees every door reaches the centre.
pub(crate) type Template = [&'static str; 6];

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
/// The Archive's rows of shelves, with gaps to slip between.
const STACKS: Template = [
    "...........",
    ".####.####.",
    "...........",
    ".####.####.",
    "...........",
    "...........",
];
/// The swamp's reed beds: clumps of reeds standing on dry tussocks, and a
/// black pool.
const REEDBEDS: Template = [
    "...........",
    "..#........",
    "......#....",
    "..~~.......",
    "..~~....#..",
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
pub struct Chest {
    pub x: f32,
    pub y: f32,
    pub open: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Room {
    /// Position on the floor's room grid.
    pub cell: (i32, i32),
    pub kind: RoomKind,
    /// Doorways, north/east/south/west.
    pub doors: [bool; 4],
    /// Size in tiles: a screen for most rooms, more for a great hall.
    #[serde(default = "screen_cols")]
    pub cols: usize,
    #[serde(default = "screen_rows")]
    pub rows: usize,
    tiles: Vec<Tile>,
    pub visited: bool,
    pub cleared: bool,
    /// Who waits here, spawned when the party walks in.
    pub roster: Vec<EnemyKind>,
    pub items: Vec<super::Item>,
    pub chest: Option<Chest>,
    /// The size its kind laid it at, when it was grown for the party
    /// (`party_room`): what lives in it is reckoned for that room.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laid: Option<(usize, usize)>,
}

impl Room {
    /// Undug room-sized rock for a connected home extension. Excavation uses
    /// the ordinary map setters and doors, just like playable floors.
    pub fn solid_rock(cell: (i32, i32)) -> Self {
        Self {
            cell,
            kind: RoomKind::Hall,
            doors: [false; 4],
            cols: COLS,
            rows: ROWS,
            tiles: vec![Tile::Wall; COLS * ROWS],
            visited: true,
            cleared: true,
            roster: Vec::new(),
            items: Vec::new(),
            chest: None,
            laid: None,
        }
    }

    pub fn kingdom(tiles: Vec<Tile>, cols: usize, rows: usize) -> Self {
        let mut room = Self::solid_rock((20, -20));
        room.kind = RoomKind::Kingdom;
        room.cols = cols;
        room.rows = rows;
        room.tiles = tiles;
        room
    }

    /// What the room is built of, for a drawing kept between frames.
    pub fn fingerprint(&self) -> u64 {
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
pub const MAX_COLS: usize = 2 * COLS;
pub const MAX_ROWS: usize = 2 * ROWS;

impl Room {
    pub(super) fn valid_snapshot(&self) -> bool {
        (if self.kind == RoomKind::Kingdom {
            // The map as it is, or as an old save carries it (the kingdom
            // before it opened), which the migration then replaces.
            let (w, h) = crate::world::map::LEGACY_MAP;
            (self.cols == super::map::MAP_W as usize && self.rows == super::map::MAP_H as usize)
                || (self.cols == w as usize && self.rows == h as usize)
        } else if self.kind == RoomKind::Depths {
            // An open floor's map: as big as `open` lays one.
            (COLS..=OPEN_MAX_COLS).contains(&self.cols) && (ROWS..=OPEN_MAX_ROWS).contains(&self.rows)
        } else {
            (COLS..=MAX_COLS).contains(&self.cols) && (ROWS..=MAX_ROWS).contains(&self.rows)
        }) && self.tiles.len() == self.cols * self.rows
            && self.laid.is_none_or(|(cols, rows)| {
                (COLS..=MAX_COLS).contains(&cols) && (ROWS..=MAX_ROWS).contains(&rows)
            })
            && self.items.len() <= if self.kind == RoomKind::Depths { super::depths::OPEN_ITEMS } else { 32 }
            && self.roster.len() <= 64
    }

    pub fn tile(&self, col: i32, row: i32) -> Tile {
        if col < 0 || row < 0 || col >= self.cols as i32 || row >= self.rows as i32 {
            return Tile::Wall;
        }
        self.tiles[row as usize * self.cols + col as usize]
    }

    pub fn set(&mut self, col: usize, row: usize, tile: Tile) {
        self.tiles[row * self.cols + col] = tile;
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn set_for_test(&mut self, col: usize, row: usize, tile: Tile) {
        self.set(col, row, tile);
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn tiles_for_test(&self) -> &[Tile] {
        &self.tiles
    }

    /// Open a doorway in side `dir`, where the floor's grid had none.
    pub fn open_door(&mut self, dir: usize) {
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

    /// The room laid afresh at `cols` by `rows`: open floor inside stone
    /// walls, its doorways where they were (`open_door`). What it measured
    /// before is kept as `laid`, as a room grown for the party keeps it.
    pub(super) fn regrow(&mut self, cols: usize, rows: usize) {
        if (cols, rows) == (self.cols, self.rows) {
            return;
        }
        self.laid.get_or_insert((self.cols, self.rows));
        (self.cols, self.rows) = (cols, rows);
        self.tiles = vec![Tile::Floor; cols * rows];
        for row in 0..rows {
            for col in 0..cols {
                if row == 0 || col == 0 || row == rows - 1 || col == cols - 1 {
                    self.set(col, row, Tile::Wall);
                }
            }
        }
        for dir in 0..4 {
            if self.doors[dir] {
                self.open_door(dir);
            }
        }
    }

    /// Stairs down in the middle of the room (a slain dragon's lair).
    pub fn open_stairs(&mut self) {
        let (c, r) = (self.cols / 2 - 1, self.rows / 2 - 1);
        for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            self.set(c + dc, r + 2 + dr, Tile::Stairs);
        }
    }

    /// The doorway out of a side-on hall: the one way back.
    pub fn way_back(&self) -> Option<usize> {
        matches!(self.kind, RoomKind::Ledge | RoomKind::Wanted)
            .then(|| self.doors.iter().position(|&d| d))?
    }

    /// Where a Sanctuary's privy stands: the first clear two-by-two corner
    /// from the north-west, its door facing south. The point is its door.
    pub fn privy(&self) -> Option<(f32, f32)> {
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

    /// Where a Sanctuary's field banner stands (the class hall below
    /// ground): the first clear tile down the east side from the middle,
    /// clear of the floor's banners along the top.
    pub fn field_banner(&self) -> Option<(f32, f32)> {
        if self.kind != RoomKind::Sanctuary {
            return None;
        }
        let clear = |c: usize, r: usize| self.tile(c as i32, r as i32) == Tile::Floor;
        (self.rows / 2 - 1..self.rows - 3).find_map(|row| {
            (self.cols / 2 + 2..self.cols - 2).rev().find_map(|col| {
                (clear(col, row) && clear(col, row + 1))
                    .then(|| ((col as f32 + 0.5) * TILE_UNITS, (row as f32 + 1.0) * TILE_UNITS))
            })
        })
    }

    /// Where a Sanctuary's altar stands: the middle, or above the stairs
    /// when the room keeps them.
    pub fn altar(&self) -> (f32, f32) {
        let (x, y) = (self.width() / 2.0, self.height() / 2.0);
        if self.tiles.contains(&Tile::Stairs) {
            (x, y - 6.0)
        } else {
            (x, y)
        }
    }

    /// Size in arena units.
    pub fn width(&self) -> f32 {
        self.cols as f32 * TILE_UNITS
    }

    pub fn height(&self) -> f32 {
        self.rows as f32 * TILE_UNITS
    }

    /// Bigger than one screen: the camera follows the knights.
    pub fn great(&self) -> bool {
        self.cols > COLS || self.rows > ROWS
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Floor {
    pub depth: u32,
    /// Changes to authored surface topology invalidate whole guest snapshots.
    #[serde(default)]
    pub world_revision: u32,
    pub pack: Pack,
    pub rooms: Vec<Room>,
    /// The floor's secret room, if it has one.
    #[serde(default)]
    pub secret: Option<Secret>,
    /// An open floor's map, its chambers and the doors off it (`open`):
    /// none for a floor of rooms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<open::Open>,
}

/// A vault hung off a fight room behind a cracked wall: the room that hides
/// it and the side its crack is on, the vault's own index, and whether the
/// wall has been blown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret {
    pub host: usize,
    pub side: usize,
    pub vault: usize,
    pub found: bool,
    /// Some vaults keep Snibbet instead of a chest: a retired loot goblin
    /// who pays the party to forget him, once.
    #[serde(default)]
    pub snibbet: bool,
    #[serde(default)]
    pub paid: bool,
}

impl Floor {
    /// A secret is one declared vault adjacent to a fight-room wall. Check both
    /// endpoints and dimensions before anyone can index/open its doorway.
    pub(super) fn valid_secret(&self) -> bool {
        let Some(secret) = self.secret else {
            return !self.rooms.iter().any(|r| r.kind == RoomKind::Secret);
        };
        if let Some(open) = &self.open {
            return self.valid_open_secret(open, secret);
        }
        let Some(host) = self.rooms.get(secret.host) else {
            return false;
        };
        let Some(vault) = self.rooms.get(secret.vault) else {
            return false;
        };
        if secret.side >= 4
            || secret.host == secret.vault
            || !host.valid_snapshot()
            || !vault.valid_snapshot()
            || host.kind != RoomKind::Fight
            || vault.kind != RoomKind::Secret
            || self
                .rooms
                .iter()
                .any(|r| !(-16..=16).contains(&r.cell.0) || !(-16..=16).contains(&r.cell.1))
            || self
                .rooms
                .iter()
                .filter(|r| r.kind == RoomKind::Secret)
                .count()
                != 1
            || (secret.paid && (!secret.found || !secret.snibbet))
        {
            return false;
        }
        // The immutable vault flavor was chosen from pre-vault geography,
        // not mutable found/paid/chest state. Recompute without cloning rooms
        // (a reclaimed floor's forge-hall and a held floor's steward's hall came after it, too).
        let shape = self
            .rooms
            .iter()
            .enumerate()
            .filter(|(i, r)| *i != secret.vault && !matches!(r.kind, RoomKind::Forge | RoomKind::Holding) && !r.kind.nook())
            .fold(u64::from(self.depth) ^ 0x5ec2e7, |h, (_, r)| {
                h.wrapping_mul(37)
                    .wrapping_add((r.cell.0 * 19 + r.cell.1) as u64)
            });
        if secret.snibbet != (super::mix(shape ^ 0x5a1bbe7).is_multiple_of(3)) {
            return false;
        }
        let (dx, dy) = DIRS[secret.side];
        host.cell.0.checked_add(dx).zip(host.cell.1.checked_add(dy)) == Some(vault.cell)
            && host.doors[secret.side] == secret.found
            && vault
                .doors
                .iter()
                .enumerate()
                .all(|(side, &open)| open == (side == (secret.side + 2) % 4))
    }

    /// The crack, while it stands: its room, and the middle of the doorway
    /// it hides (arena units).
    pub fn crack(&self) -> Option<(usize, f32, f32)> {
        if !self.valid_secret() {
            return None;
        }
        let secret = self.secret.filter(|s| !s.found)?;
        if let Some(open) = &self.open {
            let door = open.doors.iter().find(|d| d.room == secret.vault)?;
            let (x, y) = door.middle();
            return Some((open.map, x, y));
        }
        let room = &self.rooms[secret.host];
        let (w, h) = (room.width(), room.height());
        let (x, y) = match secret.side {
            0 => (w / 2.0, TILE_UNITS / 2.0),
            1 => (w - TILE_UNITS / 2.0, h / 2.0),
            2 => (w / 2.0, h - TILE_UNITS / 2.0),
            _ => (TILE_UNITS / 2.0, h / 2.0),
        };
        Some((secret.host, x, y))
    }

    /// Blow the cracked wall: its doorway opens on the vault. False if
    /// there was nothing to blow.
    pub fn open_secret(&mut self) -> bool {
        if !self.valid_secret() {
            return false;
        }
        let Some(secret) = self.secret.as_mut().filter(|s| !s.found) else {
            return false;
        };
        secret.found = true;
        let (host, side, vault) = (secret.host, secret.side, secret.vault);
        if let Some(open) = &self.open {
            // On an open map the crack is a door in a chamber's wall.
            if let Some(door) = open.doors.iter().find(|d| d.room == vault) {
                for (c, r) in door.tiles() {
                    self.rooms[host].set(c as usize, r as usize, Tile::Door);
                }
            }
            return true;
        }
        self.rooms[host].open_door(side);
        true
    }

    /// An open floor's vault: behind a door of the map's (a wall until it
    /// is found), its one doorway facing back, the only vault.
    fn valid_open_secret(&self, open: &Open, secret: Secret) -> bool {
        let (Some(host), Some(vault)) = (self.rooms.get(secret.host), self.rooms.get(secret.vault)) else {
            return false;
        };
        let Some(door) = open.doors.iter().find(|d| d.room == secret.vault) else {
            return false;
        };
        let shut = door.tiles().iter().all(|&(c, r)| host.tile(c, r) == Tile::Wall);
        let opened = door.tiles().iter().all(|&(c, r)| host.tile(c, r) == Tile::Door);
        secret.host == open.map
            && secret.side < 4
            && secret.side == usize::from(door.dir)
            && host.kind == RoomKind::Depths
            && vault.kind == RoomKind::Secret
            && self.rooms.iter().filter(|r| r.kind == RoomKind::Secret).count() == 1
            && !(secret.paid && (!secret.found || !secret.snibbet))
            && if secret.found { opened } else { shut }
            && vault
                .doors
                .iter()
                .enumerate()
                .all(|(side, &open)| open == (side == (secret.side + 2) % 4))
    }

    /// The room the floor's guardian holds: its stairs, lair or threshold.
    pub fn guardian(&self) -> Option<usize> {
        self.rooms.iter().position(|r| {
            matches!(
                r.kind,
                RoomKind::Stairs | RoomKind::Lair | RoomKind::Threshold
            )
        })
    }

    /// Doors from the entrance to the guardian's room.
    pub fn stairs_doors(&self) -> usize {
        self.guardian()
            .and_then(|g| self.steps().get(g).copied())
            .filter(|&d| d != usize::MAX)
            .unwrap_or(0)
    }

    /// Whether fight room `room` may hold the floor's chief, and how well:
    /// lower is better. Beside the guardian's door or a showdown's (the Pit,
    /// the Wanted hall) is worst, then the entrance's ring (`pacing`); after
    /// that, the nearer the middle of the floor's depth the better.
    pub(super) fn chief_rank(&self, room: usize, steps: &[usize]) -> (bool, bool, bool, usize) {
        let guardian = self.guardian();
        let beside = |showdown: bool| {
            (0..4).any(|dir| {
                self.neighbour(room, dir).is_some_and(|n| {
                    if showdown {
                        matches!(self.rooms[n].kind, RoomKind::Wanted | RoomKind::Pit)
                    } else {
                        Some(n) == guardian
                    }
                })
            })
        };
        let at = steps[room];
        let deep = guardian.map_or(0, |g| steps[g]);
        (
            beside(false),
            beside(true),
            at < super::pacing::CHIEF_FROM,
            at.saturating_mul(2).abs_diff(deep),
        )
    }

    /// Whether the floor can ramp (`pacing`): its stairs lie far enough in,
    /// and a fight room lies mid-floor for its chief.
    pub fn ramps(&self) -> bool {
        let steps = self.steps();
        let showdowns_late = (0..self.rooms.len())
            .filter(|&i| matches!(self.rooms[i].kind, RoomKind::Wanted | RoomKind::Pit))
            .all(|i| steps[i] != usize::MAX && steps[i] > super::pacing::CHIEF_FROM);
        self.stairs_doors() >= super::pacing::STAIRS_FROM
            && showdowns_late
            && (0..self.rooms.len()).any(|i| {
                self.rooms[i].kind == RoomKind::Fight
                    && steps[i] != usize::MAX
                    && matches!(self.chief_rank(i, &steps), (false, false, false, _))
            })
    }

    /// Doors from the entrance to every room (`usize::MAX` where no door
    /// leads): how deep into the floor each room lies.
    pub fn steps(&self) -> Vec<usize> {
        let mut dist = vec![usize::MAX; self.rooms.len()];
        if dist.is_empty() {
            return dist;
        }
        dist[0] = 0;
        let mut queue = VecDeque::from([0]);
        while let Some(at) = queue.pop_front() {
            for dir in 0..4 {
                if let Some(next) = self.neighbour(at, dir)
                    && dist[next] == usize::MAX
                {
                    dist[next] = dist[at] + 1;
                    queue.push_back(next);
                }
            }
        }
        dist
    }

    /// The room through door `dir` of room `from`: none where that side
    /// has no doorway (a side-on hall may sit beside rooms it never opens
    /// into).
    pub fn neighbour(&self, from: usize, dir: usize) -> Option<usize> {
        if !self.rooms[from].doors[dir] {
            return None;
        }
        // An open floor: a side room's door leads back onto the map, and
        // the map's own doors are in its chambers' walls (`open::SideDoor`).
        if let Some(open) = &self.open {
            if from == open.map {
                return None;
            }
            if open.doors.iter().any(|d| d.room == from && (usize::from(d.dir) + 2) % 4 == dir) {
                return Some(open.map);
            }
        }
        let (x, y) = self.rooms[from].cell;
        let (dx, dy) = DIRS[dir];
        self.rooms.iter().position(|r| r.cell == (x + dx, y + dy))
    }
}

/// Lay out floor `depth` of the given pack.
pub fn floor(depth: u32, pack: Pack, rng: &mut Rng) -> Floor {
    floor_party(depth, pack, 1, rng)
}

/// The least a fight room measures for a party of this many knights: the
/// room's floor grows by one screen's worth for each knight, so a party of
/// four fights in a hall four times a lone knight's.
pub fn party_room(party: usize) -> (usize, usize) {
    match party {
        0 | 1 => (COLS, ROWS),
        2 => (34, 20),
        3 => (42, 24),
        _ => (MAX_COLS, MAX_ROWS),
    }
}

/// Lay out floor `depth` of the given pack for a party of `party` knights:
/// its fight rooms and stairs rooms at least [`party_room`] in size.
pub fn floor_party(depth: u32, pack: Pack, party: usize, rng: &mut Rng) -> Floor {
    // Chambers, and the passages that join them.
    let want = 8usize.saturating_add(depth as usize);
    let grow = |rng: &mut Rng| {
        if depth >= 3 {
            crate::world::routing::tunnel_cells(
                crate::world::routing::TunnelVariant::for_depth(depth),
                rng.next(),
                GRID,
                want,
            )
        } else {
            approach_cells(want, rng)
        }
    };
    // A floor that cannot ramp is grown again (`pacing`): its stairs come
    // too soon, or no fight room lies mid-floor for its chief. The first
    // that ramps stands, else the deepest of the tries.
    let cells = grow(rng);
    let mut best = build(depth, pack, party, rng, cells);
    for _ in 0..super::pacing::GROW_TRIES {
        if best.ramps() {
            break;
        }
        let cells = grow(rng);
        let again = build(depth, pack, party, rng, cells);
        if again.ramps() || again.stairs_doors() > best.stairs_doors() {
            best = again;
        }
    }
    best
}

/// Floor `depth` of `pack` for a party of `party` on the given cells, the
/// entrance first.
fn build(depth: u32, pack: Pack, party: usize, rng: &mut Rng, cells: Vec<(i32, i32)>) -> Floor {
    let plan = Plan::of(&cells);
    let links = |&(x, y): &(i32, i32)| DIRS.map(|(dx, dy)| cells.contains(&(x + dx, y + dy)));
    let Plan {
        dist,
        far,
        treasure,
        halls,
    } = plan;
    let rooms = cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let kind = if i == 0 && depth >= 2 {
                RoomKind::Sanctuary
            } else if i == 0 {
                RoomKind::Start
            } else if i == far {
                if depth >= DEEPEST {
                    RoomKind::Threshold
                } else if depth == FLOORS && pack != Pack::Swamp {
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
            room(*cell, kind, links(cell), depth, pack, party, rng)
        })
        .collect();
    let mut floor = Floor {
        world_revision: 0,
        open: None,
        depth,
        pack,
        rooms,
        secret: None,
    };
    if depth == 1 {
        add_ledge(&mut floor, &dist, far, treasure, &halls);
    } else if depth < DEEPEST {
        add_pit(&mut floor, &dist, far, treasure, &halls);
    }
    // Every floor has its showdown (it draws nothing from the dice).
    hang(&mut floor, &dist, far, treasure, &halls, wanted);
    if depth < DEEPEST {
        add_secret(&mut floor, &dist, far, treasure, &halls);
    }
    floor
}

/// What a floor's cells become: doors from the entrance to each, the far
/// room (the stairs), the treasure's dead end, and the halls.
struct Plan {
    dist: Vec<usize>,
    far: usize,
    treasure: Option<usize>,
    halls: Vec<usize>,
}

impl Plan {
    fn of(cells: &[(i32, i32)]) -> Self {
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
        Self {
            dist,
            far,
            treasure,
            halls,
        }
    }
}

/// Keep the approach floors' established seed stream for room furnishings and
/// encounters. A finite rejection budget falls back to connected expansion.
fn approach_cells(requested: usize, rng: &mut Rng) -> Vec<(i32, i32)> {
    let want = requested.min((GRID * GRID) as usize);
    let mut cells = vec![(GRID / 2, GRID / 2)];
    for _ in 0..4096 {
        if cells.len() >= want {
            return cells;
        }
        let (x, y) = cells[rng.below(cells.len())];
        let (dx, dy) = DIRS[rng.below(4)];
        let next = (x + dx, y + dy);
        if (0..GRID).contains(&next.0) && (0..GRID).contains(&next.1) && !cells.contains(&next) {
            cells.push(next);
        }
    }
    for _ in cells.len()..want {
        let next = cells.iter().find_map(|&(x, y)| {
            DIRS.into_iter()
                .map(|(dx, dy)| (x + dx, y + dy))
                .find(|next| {
                    (0..GRID).contains(&next.0)
                        && (0..GRID).contains(&next.1)
                        && !cells.contains(next)
                })
        });
        let Some(next) = next else { break };
        cells.push(next);
    }
    cells
}

/// Now and then, a vault behind a cracked wall: hung off a fight room the
/// way the Pit is, but the wall is left standing for a bomb to find. Like
/// the Pit, whether a floor has one is down to the floor's shape.
fn add_secret(
    floor: &mut Floor,
    dist: &[usize],
    far: usize,
    treasure: Option<usize>,
    halls: &[usize],
) {
    let shape = secret_shape(floor);
    if super::mix(shape) % 100 >= SECRET_CHANCE {
        return;
    }
    // The nearest fight rooms first: a secret is for passing by.
    let mut hosts: Vec<usize> = (1..floor.rooms.len())
        .filter(|&i| i != far && Some(i) != treasure && !halls.contains(&i))
        .filter(|&i| floor.rooms[i].kind == RoomKind::Fight)
        .collect();
    hosts.sort_by_key(|&i| (dist.get(i).copied().unwrap_or(usize::MAX), i));
    hang_vault(floor, &hosts, shape);
}

/// Hollow Walls: a vault on this floor, whatever its shape says (none on
/// the Unknown's last, nor where no fight room has a free side).
pub fn force_secret(floor: &mut Floor) {
    if floor.secret.is_some() || floor.depth >= DEEPEST {
        return;
    }
    let shape = secret_shape(floor);
    let hosts: Vec<usize> = (1..floor.rooms.len())
        .filter(|&i| floor.rooms[i].kind == RoomKind::Fight)
        .collect();
    hang_vault(floor, &hosts, shape);
}

/// What decides a floor's vault: its shape, not its dice.
fn secret_shape(floor: &Floor) -> u64 {
    floor
        .rooms
        .iter()
        .fold(u64::from(floor.depth) ^ 0x5ec2e7, |h, r| {
            h.wrapping_mul(37)
                .wrapping_add((r.cell.0 * 19 + r.cell.1) as u64)
        })
}

/// Hang a vault behind a wall of the first of `hosts` with a free side the
/// room can open without cutting itself off.
fn hang_vault(floor: &mut Floor, hosts: &[usize], shape: u64) {
    let taken = |c: (i32, i32)| floor.rooms.iter().any(|r| r.cell == c);
    let near = |v: i32| (-1..=GRID).contains(&v);
    for &i in hosts {
        let (x, y) = floor.rooms[i].cell;
        for (side, (dx, dy)) in DIRS.into_iter().enumerate() {
            let cell = (x + dx, y + dy);
            if floor.rooms[i].doors[side] || !near(cell.0) || !near(cell.1) || taken(cell) {
                continue;
            }
            let mut opened = floor.rooms[i].clone();
            opened.open_door(side);
            if !connected(&opened) {
                continue;
            }
            let snibbet = super::mix(shape ^ 0x5a1bbe7).is_multiple_of(3);
            floor.rooms.push(vault(cell, (side + 2) % 4, snibbet));
            floor.secret = Some(Secret {
                host: i,
                side,
                vault: floor.rooms.len() - 1,
                found: false,
                snibbet,
                paid: false,
            });
            return;
        }
    }
}

/// The chance in a hundred that a floor hides a vault.
const SECRET_CHANCE: u64 = 45;

/// The vault at `cell`, its one doorway on `door`: a small treasure room,
/// pillars at its corners, and a chest in its middle with gold heaped round
/// it, or (`snibbet`) only Snibbet, sitting on his sack.
fn vault(cell: (i32, i32), door: usize, snibbet: bool) -> Room {
    let mut room = Room {
        cell,
        kind: RoomKind::Secret,
        doors: [false; 4],
        cols: COLS,
        rows: ROWS,
        tiles: vec![Tile::Floor; COLS * ROWS],
        visited: false,
        cleared: true,
        roster: Vec::new(),
        items: Vec::new(),
        chest: None,
        laid: None,
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            if row == 0 || col == 0 || row == ROWS - 1 || col == COLS - 1 {
                room.set(col, row, Tile::Wall);
            }
        }
    }
    for (col, row) in [(4, 3), (COLS - 5, 3), (4, ROWS - 4), (COLS - 5, ROWS - 4)] {
        room.set(col, row, Tile::Block);
    }
    room.open_door(door);
    if snibbet {
        return room;
    }
    let (cx, cy) = (room.width() / 2.0, room.height() / 2.0);
    room.chest = Some(Chest {
        x: cx,
        y: cy,
        open: false,
    });
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        room.items.push(super::Item {
            card: "gold".into(),
            x: cx + a.cos() * 6.0,
            y: cy + a.sin() * 4.0,
            held_off: None, took: Vec::new(),
        });
    }
    room
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
    hang(floor, dist, far, treasure, halls, ledge);
}

/// The Pit: on floors two to five, about half of them,
/// a dead end hung off a fight room as the ledge is, the Pit Tyrant asleep in
/// it. Whether a floor has one is down to the floor's own shape, not its
/// dice.
fn add_pit(
    floor: &mut Floor,
    dist: &[usize],
    far: usize,
    treasure: Option<usize>,
    halls: &[usize],
) {
    let shape = floor.rooms.iter().fold(u64::from(floor.depth), |h, r| {
        h.wrapping_mul(31)
            .wrapping_add((r.cell.0 * 17 + r.cell.1) as u64)
    });
    if super::mix(shape).is_multiple_of(2) {
        hang(floor, dist, far, treasure, halls, pit);
    }
}

/// Hang a room off the deepest fight room with a free cell beside it,
/// touching nothing else if it can: a dead end off the path. `build` makes
/// the room at a cell, its one doorway on the given side.
fn hang(
    floor: &mut Floor,
    dist: &[usize],
    far: usize,
    treasure: Option<usize>,
    halls: &[usize],
    build: fn((i32, i32), usize) -> Room,
) {
    let taken = |c: (i32, i32)| floor.rooms.iter().any(|r| r.cell == c);
    let mut parents: Vec<usize> = (1..floor.rooms.len())
        .filter(|&i| i != far && Some(i) != treasure && !halls.contains(&i))
        .filter(|&i| floor.rooms[i].kind == RoomKind::Fight)
        .collect();
    parents.sort_by_key(|&i| (std::cmp::Reverse(dist[i]), i));
    // Off the deepest fight room that has a free cell (`pacing`: the
    // floor's showdowns wait at its end, not by its entrance); a cell
    // touching only its parent first, else any free cell, its other sides
    // blank walls. It may hang just past the floor's grid.
    let near = |v: i32| (-1..=GRID).contains(&v);
    let mut spots: Vec<(usize, bool, usize)> = Vec::new();
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
            spots.push((rank, touching > 1, d));
        }
    }
    spots.sort_unstable();
    for (rank, _, d) in spots {
        let i = parents[rank];
        let (x, y) = floor.rooms[i].cell;
        let cell = (x + DIRS[d].0, y + DIRS[d].1);
        let mut parent = floor.rooms[i].clone();
        parent.open_door(d);
        if !connected(&parent) {
            continue;
        }
        floor.rooms[i] = parent;
        floor.rooms.push(build(cell, (d + 2) % 4));
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
        laid: None,
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

/// The Wanted hall at `cell`, its one doorway leading back through `door`:
/// the stage in `outlaw::STAGE`, side-on, with nobody in it until the
/// outlaw is raised as the party walks in.
fn wanted(cell: (i32, i32), door: usize) -> Room {
    let cols = super::outlaw::STAGE[0].len();
    let mut doors = [false; 4];
    doors[door] = true;
    let mut room = Room {
        cell,
        kind: RoomKind::Wanted,
        doors,
        cols,
        rows: ROWS,
        tiles: vec![Tile::Floor; cols * ROWS],
        visited: false,
        cleared: false,
        roster: Vec::new(),
        items: Vec::new(),
        chest: None,
        laid: None,
    };
    for (row, line) in super::outlaw::STAGE.iter().enumerate() {
        for (col, b) in line.bytes().enumerate() {
            let tile = match b {
                b'#' => Tile::Block,
                b'=' => Tile::Ledge,
                b'D' => Tile::Door,
                _ => Tile::Floor,
            };
            room.set(col, row, tile);
        }
    }
    room
}

/// The Pit at `cell`, its one doorway on `door`: a plain chamber, bones
/// piled at its four corners, the Pit Tyrant in the middle.
fn pit(cell: (i32, i32), door: usize) -> Room {
    let mut room = Room {
        cell,
        kind: RoomKind::Pit,
        doors: [false; 4],
        cols: COLS,
        rows: ROWS,
        tiles: vec![Tile::Floor; COLS * ROWS],
        visited: false,
        cleared: false,
        roster: vec![EnemyKind::PitTyrant],
        items: Vec::new(),
        chest: None,
        laid: None,
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            if row == 0 || col == 0 || row == ROWS - 1 || col == COLS - 1 {
                room.set(col, row, Tile::Wall);
            }
        }
    }
    for (col, row) in [(3, 2), (COLS - 4, 2), (3, ROWS - 3), (COLS - 4, ROWS - 3)] {
        room.set(col, row, Tile::Block);
    }
    room.open_door(door);
    room
}

fn room(
    cell: (i32, i32),
    kind: RoomKind,
    doors: [bool; 4],
    depth: u32,
    pack: Pack,
    party: usize,
    rng: &mut Rng,
) -> Room {
    let template = template_for(kind, pack, rng);
    // Guardians and the dragon keep great halls; some fights do too.
    let (cols, rows) = match kind {
        RoomKind::Stairs => (36, 20),
        RoomKind::Lair => (40, 24),
        RoomKind::Threshold => (MAX_COLS, MAX_ROWS),
        RoomKind::Fight if depth >= 2 && rng.chance(12) => (MAX_COLS, MAX_ROWS),
        RoomKind::Fight if rng.chance(30) => (32, 20),
        _ => (COLS, ROWS),
    };
    // A great hall holds a bigger company; a room grown for the party does
    // not (the party brings its own, `stakes::EXTRA_PER_KNIGHT`).
    let company = (cols * rows) / (COLS * ROWS * 2);
    let laid = (cols, rows);
    // A bigger party needs more floor to fight on.
    let (cols, rows) = if matches!(kind, RoomKind::Fight | RoomKind::Stairs) {
        let (least_cols, least_rows) = party_room(party);
        (cols.max(least_cols), rows.max(least_rows))
    } else {
        (cols, rows)
    };
    furnish(cell, kind, doors, depth, pack, template, (cols, rows), laid, company, rng)
}

/// The quadrant a room of `kind` is drawn from: the pack's own and the
/// dungeon masters' for a fight, a fixed one for every other kind.
fn template_for(kind: RoomKind, pack: Pack, rng: &mut Rng) -> Template {
    match kind {
        RoomKind::Kingdom | RoomKind::RealmHall | RoomKind::LandOffice | RoomKind::Holding => EMPTY,
        RoomKind::Kitchen | RoomKind::Reading | RoomKind::Bellows | RoomKind::Ossuary | RoomKind::Still | RoomKind::Loft => EMPTY,
        RoomKind::Start
        | RoomKind::Hall
        | RoomKind::Sanctuary
        | RoomKind::Ledge
        | RoomKind::Home
        | RoomKind::Fortune
        | RoomKind::Yard
        | RoomKind::Trophies
        | RoomKind::Pit
        | RoomKind::Secret
        | RoomKind::Tavern
        | RoomKind::Stockpile
        | RoomKind::Muster | RoomKind::Workshop
        | RoomKind::Quarters
        | RoomKind::Gate
        | RoomKind::Stables
        | RoomKind::Lists
        | RoomKind::MineHead
        | RoomKind::KingsHall
        | RoomKind::Wanted
        | RoomKind::Forge => EMPTY,
        RoomKind::Forest
        | RoomKind::Village
        | RoomKind::Fairground
        | RoomKind::Riverbank
        | RoomKind::Windmill
        | RoomKind::Ruins
        | RoomKind::Meadow
        | RoomKind::Orchard
        | RoomKind::Marsh
        | RoomKind::Downs
        | RoomKind::Cliffs
        | RoomKind::Causeway
        | RoomKind::Priory
        | RoomKind::Crag
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
        | RoomKind::SignalHill
        | RoomKind::CollegeGrounds
        | RoomKind::CollegeHall
        | RoomKind::CollegeClass
        | RoomKind::CollegeLibrary
        | RoomKind::CollegeLab
        | RoomKind::TowerEntry
        | RoomKind::TowerStairs
        | RoomKind::TowerStars
        | RoomKind::WizardStudy
        | RoomKind::Ranch
        | RoomKind::Pasture
        | RoomKind::LongbowGreen
        | RoomKind::Fletchery
        | RoomKind::Scarecrow
        | RoomKind::Market
        | RoomKind::Bathhouse
        | RoomKind::Smithy
        | RoomKind::Eyrie
        | RoomKind::ColosseumHall
        | RoomKind::KegYard
        | RoomKind::GanderHall => EMPTY,
        RoomKind::Threshold | RoomKind::Depths => EMPTY,
        RoomKind::Treasure => SHRINE,
        RoomKind::Lair => LAIR,
        RoomKind::Fight | RoomKind::Stairs => {
            // The pack's own, then the dungeon masters' (`rooms`).
            let templates = pack.templates();
            let authored = super::rooms::small(pack);
            let i = rng.below(templates.len() + authored.len());
            if i < templates.len() { templates[i] } else { authored[i - templates.len()] }
        }
    }
}

/// A room of `kind` at its size `(cols, rows)` (laid at `laid`, holding
/// `company` more for a great hall): its stone from `template` (or a
/// great quadrant), its fixtures and its company.
#[allow(clippy::too_many_arguments)]
pub(super) fn furnish(
    cell: (i32, i32),
    kind: RoomKind,
    doors: [bool; 4],
    depth: u32,
    pack: Pack,
    template: Template,
    (cols, rows): (usize, usize),
    laid: (usize, usize),
    company: usize,
    rng: &mut Rng,
) -> Room {
    let mut quadrant: Vec<Vec<u8>> = if cols == COLS && rows == ROWS {
        template.iter().map(|r| r.bytes().collect()).collect()
    } else {
        // A bigger room is a dungeon master's hall when one is drawn for
        // its size (`rooms`), most of the time; else the pillared grid.
        let authored = if matches!(kind, RoomKind::Fight | RoomKind::Stairs) {
            super::rooms::great(pack, cols / 2 - 1, rows / 2 - 1)
        } else {
            Vec::new()
        };
        if !authored.is_empty() && rng.chance(super::rooms::AUTHORED_SHARE) {
            authored[rng.below(authored.len())].iter().map(|r| r.bytes().collect()).collect()
        } else {
            great_quadrant(cols / 2 - 1, rows / 2 - 1, kind, rng)
        }
    };
    if matches!(kind, RoomKind::Fight | RoomKind::Stairs) && cols == COLS {
        for _ in 0..pack.scatter() {
            let (qx, qy) = (1 + rng.below(8), 1 + rng.below(4));
            quadrant[qy][qx] = b'#';
        }
    }
    // The swamp's mire over its rooms: drawn from the room's place, not the
    // dice, so no other floor's stream moves.
    if pack == Pack::Swamp && matches!(kind, RoomKind::Fight | RoomKind::Stairs) {
        super::swamp::mire(&mut quadrant, depth, super::swamp::salt(cell, depth));
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
        laid: ((cols, rows) != laid).then_some(laid),
    };
    let build = |room: &mut Room, quadrant: &[Vec<u8>]| mirror(room, doors, quadrant);
    build(&mut room, &quadrant);
    if !connected(&room) {
        let open = vec![vec![b'.'; cols / 2 - 1]; rows / 2 - 1];
        build(&mut room, &open);
    }
    let centre = (cols / 2 - 1, rows / 2 - 1);
    match kind {
        // The swamp's last hall has no way down: the Mire King's fall lights
        // the way home instead.
        RoomKind::Stairs if pack == Pack::Swamp && depth >= super::swamp::FLOORS => {}
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
        RoomKind::Threshold => {
            // The Shoggoth fills the north of its hall: stone to feet and
            // shots alike, but for a doorway's lane.
            for row in 1..=3 {
                for col in 1..cols - 1 {
                    let lane = doors[0] && (col == cols / 2 - 1 || col == cols / 2);
                    if !lane {
                        room.set(col, row, Tile::Block);
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
    // The deep brings worse company, not more of it: Dragon Keep's numbers.
    let foes = (2 + depth.min(FLOORS) as usize + rng.below(2)).min(6);
    room.roster = match kind {
        RoomKind::Kingdom | RoomKind::RealmHall | RoomKind::LandOffice | RoomKind::Holding => Vec::new(),
        RoomKind::Kitchen | RoomKind::Reading | RoomKind::Bellows | RoomKind::Ossuary | RoomKind::Still | RoomKind::Loft => Vec::new(),
        RoomKind::Start if pack == Pack::Swamp => vec![EnemyKind::Leech, EnemyKind::Leech],
        RoomKind::Start => vec![EnemyKind::Skeleton, EnemyKind::Skeleton],
        RoomKind::Treasure
        | RoomKind::Sanctuary
        | RoomKind::Ledge
        | RoomKind::Home
        | RoomKind::Fortune
        | RoomKind::Yard
        | RoomKind::Trophies
        | RoomKind::Secret
        | RoomKind::Tavern
        | RoomKind::Stockpile
        | RoomKind::Muster | RoomKind::Workshop
        | RoomKind::Quarters
        | RoomKind::Gate
        | RoomKind::Stables
        | RoomKind::Lists
        | RoomKind::MineHead
        | RoomKind::KingsHall
        | RoomKind::Wanted
        | RoomKind::Forge => Vec::new(),
        RoomKind::Forest
        | RoomKind::Village
        | RoomKind::Fairground
        | RoomKind::Riverbank
        | RoomKind::Windmill
        | RoomKind::Ruins
        | RoomKind::Meadow
        | RoomKind::Orchard
        | RoomKind::Marsh
        | RoomKind::Downs
        | RoomKind::Cliffs
        | RoomKind::Causeway
        | RoomKind::Priory
        | RoomKind::Crag
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
        | RoomKind::SignalHill
        | RoomKind::CollegeGrounds
        | RoomKind::CollegeHall
        | RoomKind::CollegeClass
        | RoomKind::CollegeLibrary
        | RoomKind::CollegeLab
        | RoomKind::TowerEntry
        | RoomKind::TowerStairs
        | RoomKind::TowerStars
        | RoomKind::WizardStudy
        | RoomKind::Ranch
        | RoomKind::Pasture
        | RoomKind::LongbowGreen
        | RoomKind::Fletchery
        | RoomKind::Scarecrow
        | RoomKind::Market
        | RoomKind::Bathhouse
        | RoomKind::Smithy
        | RoomKind::Eyrie
        | RoomKind::Depths
        | RoomKind::ColosseumHall
        | RoomKind::KegYard
        | RoomKind::GanderHall => Vec::new(),
        RoomKind::Pit => vec![EnemyKind::PitTyrant],
        // A passage may hold a straggler or two that fly.
        RoomKind::Hall => (0..rng.below(3)).map(|_| EnemyKind::Bat).collect(),
        RoomKind::Lair => vec![EnemyKind::Dragon, EnemyKind::Imp, EnemyKind::Imp],
        RoomKind::Threshold => vec![EnemyKind::Wraith, EnemyKind::Wraith, EnemyKind::Slime],
        // A great hall holds a bigger company.
        RoomKind::Fight => (0..foes + company)
            .map(|_| rng.pick(pack.roster_at(depth)))
            .collect(),
        RoomKind::Stairs => (0..foes + 1)
            .map(|_| rng.pick(pack.roster_at(depth)))
            .collect(),
    };
    // One of each dangerous kind a room, and only so many kinds.
    let plain = pack.plain_at(depth);
    let mut seen: Vec<EnemyKind> = Vec::new();
    for kind in &mut room.roster {
        if kind.elite() {
            if seen.contains(kind) || seen.len() >= Pack::dangers_at(depth) {
                *kind = plain;
            } else {
                seen.push(*kind);
            }
        }
    }
    // A fight room holds one more of the third company than it used to, a
    // plain one, chosen by where the room is so the same floor is the same.
    if kind == RoomKind::Fight {
        let plain: Vec<EnemyKind> = pack
            .roster_at(depth)
            .iter()
            .map(|&(kind, _)| kind)
            .filter(|k| k.critter() && !k.elite())
            .collect();
        if !plain.is_empty() {
            let h = super::mix(
                u64::from(depth) * 131 + room.cell.0 as u64 * 17 + room.cell.1 as u64 * 5 + 7,
            );
            room.roster.push(plain[(h % plain.len() as u64) as usize]);
        }
    }
    room
}

/// The Undercroft: one screen of cellar, the Winding Stair in its middle
/// behind a parapet open to the south, the stations' furniture along its
/// walls. Depth 0: the stair below it goes to the first floor of `pack`.
pub fn undercroft(pack: Pack) -> Floor {
    use super::home::{BOUNTY_BOARD, SPOTS, STAIR_MOUTH, STAIRWELL};
    let mut room = Room {
        cell: (GRID / 2, GRID / 2),
        kind: RoomKind::Home,
        doors: [false; 4],
        cols: COLS,
        rows: ROWS,
        tiles: vec![Tile::Floor; COLS * ROWS],
        visited: true,
        cleared: true,
        roster: Vec::new(),
        items: Vec::new(),
        chest: None,
        laid: None,
    };
    let fill = |room: &mut Room, (col, row, w, h): (i32, i32, i32, i32), tile: Tile| {
        for r in row..row + h {
            for c in col..col + w {
                room.set(c as usize, r as usize, tile);
            }
        }
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            if row == 0 || col == 0 || row == ROWS - 1 || col == COLS - 1 {
                room.set(col, row, Tile::Wall);
            }
        }
    }
    for spot in &SPOTS {
        fill(&mut room, spot.furniture, Tile::Block);
    }
    fill(&mut room, BOUNTY_BOARD, Tile::Block);
    fill(&mut room, STAIRWELL, Tile::Block);
    let (c, r, w, h) = STAIRWELL;
    fill(&mut room, (c + 1, r + 1, w - 2, h - 2), Tile::Stairs);
    fill(&mut room, STAIR_MOUTH, Tile::Floor);
    room.open_door(1);
    room.open_door(0);
    room.open_door(2);
    // Dame Fortune's hall, east through the door: the wheel on its stand
    // against the north wall, the audience's benches across the south.
    let mut hall = Room {
        cell: (GRID / 2 + 1, GRID / 2),
        kind: RoomKind::Fortune,
        doors: [false; 4],
        tiles: vec![Tile::Floor; COLS * ROWS],
        ..room.clone()
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            if row == 0 || col == 0 || row == ROWS - 1 || col == COLS - 1 {
                hall.set(col, row, Tile::Wall);
            }
        }
    }
    fill(&mut hall, super::fortune::WHEEL_STAND, Tile::Block);
    fill(&mut hall, super::fortune::COFFER, Tile::Block);
    for bench in super::fortune::BENCHES {
        fill(&mut hall, bench, Tile::Block);
    }
    hall.open_door(3);
    // The Training Yard, north through the door: quintains, and the stall
    // a goblin will one day keep.
    let mut yard = Room {
        cell: (GRID / 2, GRID / 2 - 1),
        kind: RoomKind::Yard,
        doors: [false; 4],
        tiles: vec![Tile::Floor; COLS * ROWS],
        ..hall.clone()
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            if row == 0 || col == 0 || row == ROWS - 1 || col == COLS - 1 {
                yard.set(col, row, Tile::Wall);
            }
        }
    }
    fill(&mut yard, super::yard::STALL, Tile::Block);
    yard.open_door(2);
    // The Trophy Hall, south down the runner: plinths along its walls, the
    // dragon's in the middle, the Grail's dais at the far end.
    let mut trophies = Room {
        cell: (GRID / 2, GRID / 2 + 1),
        kind: RoomKind::Trophies,
        doors: [false; 4],
        tiles: vec![Tile::Floor; COLS * ROWS],
        ..yard.clone()
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            if row == 0 || col == 0 || row == ROWS - 1 || col == COLS - 1 {
                trophies.set(col, row, Tile::Wall);
            }
        }
    }
    for plinth in &super::trophies::PLINTHS {
        fill(&mut trophies, plinth.at, Tile::Block);
    }
    trophies.open_door(0);
    Floor {
        world_revision: 0,
        open: None,
        depth: 0,
        pack,
        rooms: vec![room, hall, yard, trophies],
        secret: None,
    }
}

/// Tobbin's crew digs out the west wing: the Trophy Hall's west wall
/// opens on Maud's tavern, the Siege Perilous. False if there is no Trophy
/// Hall here, or the tavern is already dug.
pub fn dig_tavern(floor: &mut Floor) -> bool {
    if !dig_tavern_reserved(floor) {
        return false;
    }
    floor.rooms.last_mut().unwrap().open_door(3);
    // The bellwright's loft is a separate furnished room beyond the west wing.
    let mut loft = floor.rooms.last().unwrap().clone();
    loft.kind = RoomKind::Workshop;
    loft.cell.0 -= 1;
    loft.doors = [false; 4];
    loft.tiles.fill(Tile::Floor);
    for row in 0..ROWS { for col in 0..COLS {
        if row == 0 || col == 0 || row == ROWS-1 || col == COLS-1 { loft.set(col,row,Tile::Wall); }
    }}
    loft.open_door(1);
    loft.open_door(0);
    floor.rooms.push(loft.clone());
    let mut muster = loft;
    muster.kind = RoomKind::Muster;
    muster.cell.1 -= 1;
    muster.doors = [false;4];
    muster.tiles.fill(Tile::Floor);
    for row in 0..ROWS {for col in 0..COLS {if row==0 || col==0 || row==ROWS-1 || col==COLS-1 {muster.set(col,row,Tile::Wall);}}}
    muster.open_door(2);
    floor.rooms.push(muster);
    true
}

/// Dig only the paid tavern footprint. Durable research settlements reserve this
/// cell separately; the ordinary home's loft and muster cells belong to their
/// recorded excavation wing and must not alter existing plans or saved maps.
pub fn dig_tavern_reserved(floor: &mut Floor) -> bool {
    use super::tavern::{BAR, BOARD, STAGE, TABLES};
    if floor.depth != 0 || floor.rooms.iter().any(|r| r.kind == RoomKind::Tavern) {
        return false;
    }
    let Some(hall) = floor
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Trophies)
    else {
        return false;
    };
    floor.rooms[hall].open_door(3);
    let (x, y) = floor.rooms[hall].cell;
    let mut room = Room {
        cell: (x - 1, y),
        kind: RoomKind::Tavern,
        doors: [false; 4],
        cols: COLS,
        rows: ROWS,
        tiles: vec![Tile::Floor; COLS * ROWS],
        visited: true,
        cleared: true,
        roster: Vec::new(),
        items: Vec::new(),
        chest: None,
        laid: None,
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            if row == 0 || col == 0 || row == ROWS - 1 || col == COLS - 1 {
                room.set(col, row, Tile::Wall);
            }
        }
    }
    let mut fill = |(col, row, w, h): (i32, i32, i32, i32)| {
        for r in row..row + h {
            for c in col..col + w {
                room.set(c as usize, r as usize, Tile::Block);
            }
        }
    };
    fill(BAR);
    fill(BOARD);
    fill(STAGE);
    for table in TABLES {
        fill(table);
    }
    room.open_door(1);
    floor.rooms.push(room);
    true
}

/// Floor `depth` as the wheel's mode lays it out: a horde's great hall, a
/// gauntlet of guardians, or the delve as it always was.
pub fn floor_for(
    mode: super::fortune::Mode,
    depth: u32,
    pack: Pack,
    rng: &mut Rng,
) -> Floor {
    floor_for_party(mode, depth, pack, 1, rng)
}

/// [`floor_for`] for a party of `party` knights (see [`floor_party`]).
pub fn floor_for_party(
    mode: super::fortune::Mode,
    depth: u32,
    pack: Pack,
    party: usize,
    rng: &mut Rng,
) -> Floor {
    use super::fortune::Mode;
    match mode {
        // The swamp keeps its own shape whatever the wheel says: its last
        // hall is the Mire King's, never a horde's or a gauntlet's lair.
        Mode::HoldTheStair | Mode::Gauntlet if pack == Pack::Swamp => {
            floor_party(depth, pack, party, rng)
        }
        Mode::HoldTheStair => horde(depth, pack, rng),
        Mode::Gauntlet => gauntlet(depth, pack, rng),
        Mode::HollowWalls => {
            // The wheel's promise is a vault: a floor with no fight room
            // free to hide one is grown again.
            let mut hollow = floor_party(depth, pack, party, rng);
            force_secret(&mut hollow);
            for _ in 0..super::pacing::GROW_TRIES {
                if hollow.secret.is_some() || depth >= DEEPEST {
                    break;
                }
                hollow = floor_party(depth, pack, party, rng);
                force_secret(&mut hollow);
            }
            hollow
        }
        _ => floor_party(depth, pack, party, rng),
    }
}

/// Hold the Stair: a landing, and north of it one great hall with the
/// stairs in its middle (the lair, on the last floor). The hall's waves
/// are the floor.
fn horde(depth: u32, pack: Pack, rng: &mut Rng) -> Floor {
    let mid = GRID / 2;
    let first = if depth >= 2 {
        RoomKind::Sanctuary
    } else {
        RoomKind::Start
    };
    let mut start = room(
        (mid, mid + 1),
        first,
        [true, false, false, false],
        depth,
        pack,
        1,
        rng,
    );
    start.roster.clear();
    start.cleared = true;
    let hall = if depth >= FLOORS {
        room(
            (mid, mid),
            RoomKind::Lair,
            [false, false, true, false],
            depth,
            pack,
            1,
            rng,
        )
    } else {
        // The guardian's great hall, its stairs in the middle, held as a
        // fight: the horde comes in waves, the guardian stays below.
        let mut hall = room(
            (mid, mid),
            RoomKind::Stairs,
            [false, false, true, false],
            depth,
            pack,
            1,
            rng,
        );
        hall.kind = RoomKind::Fight;
        hall.roster.truncate(3);
        hall
    };
    Floor {
        world_revision: 0,
        open: None,
        depth,
        pack,
        rooms: vec![start, hall],
        secret: None,
    }
}

/// Gauntlet of Guardians: the entrance, one passage, then the guardian's
/// stairs (the dragon's lair on the last floor), straight north.
fn gauntlet(depth: u32, pack: Pack, rng: &mut Rng) -> Floor {
    let mid = GRID / 2;
    let first = if depth >= 2 {
        RoomKind::Sanctuary
    } else {
        RoomKind::Start
    };
    let end = if depth >= FLOORS {
        RoomKind::Lair
    } else {
        RoomKind::Stairs
    };
    let mut start = room(
        (mid, mid + 1),
        first,
        [true, false, false, false],
        depth,
        pack,
        1,
        rng,
    );
    start.roster.clear();
    start.cleared = true;
    let hall = room(
        (mid, mid),
        RoomKind::Hall,
        [true, false, true, false],
        depth,
        pack,
        1,
        rng,
    );
    let guardian = room(
        (mid, mid - 1),
        end,
        [false, false, true, false],
        depth,
        pack,
        1,
        rng,
    );
    Floor {
        world_revision: 0,
        open: None,
        depth,
        pack,
        rooms: vec![start, hall, guardian],
        secret: None,
    }
}

/// Lay `quadrant` (the top-left of the interior: `.` floor, `#` block, `~`
/// hazard, `,` bog) into `room`, mirrored both ways, walls round it and doorways
/// where `doors` says: the doorway lanes meet in the middle.
pub(crate) fn mirror(room: &mut Room, doors: [bool; 4], quadrant: &[Vec<u8>]) {
    let (cols, rows) = (room.cols, room.rows);
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
                    b',' => Tile::Bog,
                    _ => Tile::Floor,
                }
            };
            room.set(col, row, tile);
        }
    }
}

/// Every doorway and the centre reachable on foot.
pub fn connected(room: &Room) -> bool {
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Door | Tile::Stairs | Tile::Bog);
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
