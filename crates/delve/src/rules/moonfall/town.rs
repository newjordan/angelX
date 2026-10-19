//! Moonfall's fen town: one tile map, its named spots and its walking rules.
//!
//! Everything is on the integer grid: 16-pixel tiles, positions in pixels
//! times `Q` (256). The map is plain text so it can be read at a glance;
//! the stage draws from the same text and art never decides what is solid.
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, OnceLock};

pub const TILE: i32 = 16;
pub const COLS: i32 = 38;
pub const ROWS: i32 = 28;
pub const Q: i32 = 256;

/// The town, north at the top. `.` grass, `,` cobbles, `~` water, `=` the
/// north bridge, `p` creaking planks, `r` reeds, `f` hedge, `g` the ditch,
/// `d` a doorstep, `G` the east gate, `L` a lantern post, `c` a crate,
/// capitals are buildings (N north tower, Z south tower, S scriptorium,
/// U bridge hut, A archive, B bakery, F forge, H herald house, W the
/// Moonwell, C and I other houses).
pub const MAP: [&str; ROWS as usize] = [
    "......................................",
    "...SSSSS.rrrrrrrNNNNN.................",
    "...SSSSS.rfrrfrrNNNNN.......CCCCC.....",
    "...SSSSS.rrrrrrrNNNNN..UUU..CCCCC.....",
    "...SSSSS.rrrrrrr.,d,...UUU..CCCCC.....",
    "...,,d,,,frrrrfr.,,,...UUU..CCCCC.....",
    "...,,,,,,rrrrrrr.,,,.,,,d,............",
    "...,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,....",
    "~~~~p~~~~~~~~~~~~===~~~~~~~~~~~~~~~~~~",
    "~~~~p~~~~~~~~~~~~===~~~~~~~~~~~~~~~~~~",
    ".CCC,..AAAAA..,,,,,,,,.BBBBFFFHHHHH.ff",
    ".CCC,..AAAAA..,,,,,,,,.BBBBFFFHHHHH.ff",
    ".CCC,..AAAAA..,,,WW,,,.BBBBFFFHHHHH.ff",
    ".CCC,..AAAAA..,,,WW,,,.BBBBFFFHHHHH.ff",
    ".,,,,,,,,d,,,,,,,,,,,,,,d,,,d,,,d,,G,,",
    ".,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,G,,",
    ".,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,G,,",
    ".........L...L,,L,,,L,..L...L.......ff",
    ".CCCCCC.......,,,,,,,,..IIIIIIIII...ff",
    ".CCCCCC.......,,,,,,,,..IIIIIIIII...ff",
    ".CCCCCC..........c......IIIIIIIII.....",
    "~~~~~~~~~~~~~~~~~,~~~~~~~~~~~~~~~~~~~~",
    "~~~~~~~~~~~~~~~~~,~~~~~~~~~~~~~~~~~~~~",
    "~~~~~~~~~~~~~~~~~g~~~~~~~~~~~~~~~~~~~~",
    ".................d....................",
    "........~~~~~~.ZZZZZ..~~~~~...~~~~~~~~",
    "........~~~~~~.ZZZZZ..~~~~~...~~~~~~~~",
    "........~~~~~~.ZZZZZ..~~~~~...~~~~~~~~",
];

/// What a tile is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Grass,
    Cobble,
    Water,
    /// The north bridge: down by day, raised at night.
    Bridge,
    /// Creaking planks across the canal: only a light knight crosses.
    Plank,
    Reeds,
    Hedge,
    /// The ditch before the south tower: solid until a crate fills it.
    Ditch,
    Door,
    Gate,
    /// A building or tower wall, by its letter.
    Wall(char),
}

/// A tile by its column and row; beyond the map is water.
pub fn tile(tx: i32, ty: i32) -> Tile {
    if !(0..COLS).contains(&tx) || !(0..ROWS).contains(&ty) {
        return Tile::Water;
    }
    match MAP[ty as usize].as_bytes()[tx as usize] as char {
        '~' => Tile::Water,
        '=' => Tile::Bridge,
        'p' => Tile::Plank,
        'r' => Tile::Reeds,
        'f' => Tile::Hedge,
        'g' => Tile::Ditch,
        'd' => Tile::Door,
        'G' => Tile::Gate,
        ',' | 'L' | 'c' => Tile::Cobble,
        '.' => Tile::Grass,
        c => Tile::Wall(c),
    }
}

/// What is true of the ground right now: the rules read it, the stage
/// draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ground {
    pub bridge_up: bool,
    pub swift: bool,
    pub ditch_filled: bool,
}

impl Ground {
    pub fn open(self, t: Tile) -> bool {
        match t {
            Tile::Grass | Tile::Cobble | Tile::Reeds | Tile::Door => true,
            Tile::Bridge => !self.bridge_up,
            Tile::Plank => self.swift,
            Tile::Ditch => self.ditch_filled,
            Tile::Water | Tile::Hedge | Tile::Gate | Tile::Wall(_) => false,
        }
    }
}

/// The tile a pixel is in.
pub fn tile_of(px: i32) -> i32 {
    px.div_euclid(TILE)
}

/// Whether a knight's feet (an 8 by 6 box, `x` and `y` in pixels times `Q`)
/// stand clear of every solid tile and crate.
pub fn clear(x: i32, y: i32, ground: Ground, crates: &[(i32, i32)]) -> bool {
    let (px, py) = (x.div_euclid(Q), y.div_euclid(Q));
    for (cx, cy) in [(-4, -3), (3, -3), (-4, 2), (3, 2)] {
        let (tx, ty) = (tile_of(px + cx), tile_of(py + cy));
        if !ground.open(tile(tx, ty)) || crates.contains(&(tx, ty)) {
            return false;
        }
    }
    true
}

/// A named place folk and knights go to, by tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Spot {
    ArchDoor,
    ArchPorch,
    BakeryDoor,
    BakeryAt,
    ForgeAt,
    HeraldDoor,
    HeraldAt,
    Podium,
    WellSide,
    WellEast,
    ScriptDoor,
    ScriptDesk,
    ReedBank,
    HutDoor,
    BridgePost,
    EastGate,
    EastOut,
    Lamp1,
    Lamp2,
    Lamp3,
    Lamp4,
    Lamp5,
    Lamp6,
    NorthYard,
    SouthYard,
    Plaza,
}

impl Spot {
    pub fn tile(self) -> (i32, i32) {
        match self {
            Self::ArchDoor => (9, 14),
            Self::ArchPorch => (10, 15),
            Self::BakeryDoor => (24, 14),
            Self::BakeryAt => (24, 15),
            Self::ForgeAt => (28, 15),
            Self::HeraldDoor => (32, 14),
            Self::HeraldAt => (32, 15),
            Self::Podium => (21, 16),
            Self::WellSide => (15, 15),
            Self::WellEast => (20, 15),
            Self::ScriptDoor => (5, 5),
            Self::ScriptDesk => (6, 6),
            Self::ReedBank => (12, 7),
            Self::HutDoor => (24, 6),
            Self::BridgePost => (21, 7),
            Self::EastGate => (34, 15),
            Self::EastOut => (37, 15),
            Self::Lamp1 => LAMPS[0],
            Self::Lamp2 => LAMPS[1],
            Self::Lamp3 => LAMPS[2],
            Self::Lamp4 => LAMPS[3],
            Self::Lamp5 => LAMPS[4],
            Self::Lamp6 => LAMPS[5],
            Self::NorthYard => (18, 5),
            Self::SouthYard => (17, 24),
            Self::Plaza => (17, 15),
        }
    }
    /// The middle of its tile, in pixels.
    pub fn px(self) -> (i32, i32) {
        let (tx, ty) = self.tile();
        (tx * TILE + TILE / 2, ty * TILE + TILE / 2)
    }
}

/// The street lanterns, west to east, by the tile of their post.
pub const LAMPS: [(i32, i32); 6] = [(9, 17), (13, 17), (16, 17), (20, 17), (24, 17), (28, 17)];

/// Where the one crate begins: on the plaza's south edge, above the ditch.
pub const CRATE_START: (i32, i32) = (17, 20);
/// The ditch it fills.
pub const DITCH: (i32, i32) = (17, 23);
/// The Moonwell Bell's post, west of the well, in pixels (its foot).
pub const BELL_POST: (i32, i32) = (13 * TILE + 8, 12 * TILE + 12);
/// The Moonwell's middle, in pixels.
pub const WELL: (i32, i32) = (18 * TILE, 13 * TILE);
/// The two tower doors, in pixels, and the flame's place above each.
pub const NORTH_DOOR: (i32, i32) = (18 * TILE + 8, 4 * TILE + 8);
pub const SOUTH_DOOR: (i32, i32) = (17 * TILE + 8, 24 * TILE + 8);
/// Where the ink pot lies and the reed bundles stand, in the reed bank.
pub const INK_POT: (i32, i32) = (11 * TILE + 8, 3 * TILE + 8);
pub const REED_STACK: (i32, i32) = (13 * TILE + 8, 5 * TILE + 8);

/// A house on the stage: letter, name, first tile and size in tiles.
pub struct House {
    pub ch: char,
    pub name: &'static str,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub const HOUSES: [House; 10] = [
    House { ch: 'N', name: "NORTH BELL TOWER", x: 16, y: 1, w: 5, h: 3 },
    House { ch: 'Z', name: "SOUTH BELL TOWER", x: 15, y: 25, w: 5, h: 3 },
    House { ch: 'S', name: "SCRIPTORIUM", x: 3, y: 1, w: 5, h: 4 },
    House { ch: 'U', name: "BRIDGE HUT", x: 23, y: 3, w: 3, h: 3 },
    House { ch: 'A', name: "ARCHIVE", x: 7, y: 10, w: 5, h: 4 },
    House { ch: 'B', name: "BAKERY", x: 23, y: 10, w: 4, h: 4 },
    House { ch: 'F', name: "FORGE", x: 27, y: 10, w: 3, h: 4 },
    House { ch: 'H', name: "HERALD HOUSE", x: 30, y: 10, w: 5, h: 4 },
    House { ch: 'C', name: "", x: 1, y: 10, w: 3, h: 4 },
    House { ch: 'I', name: "INN", x: 24, y: 18, w: 9, h: 3 },
];

// ---- paths -----------------------------------------------------------------

/// A flood of walking distances from a goal tile over the open ground in
/// `ground`, one entry per tile (`u16::MAX`: not reachable).
pub type Field = Vec<u16>;

fn index(tx: i32, ty: i32) -> usize {
    (ty * COLS + tx) as usize
}

fn flood(goal: (i32, i32), ground: Ground, crate_at: Option<(i32, i32)>, folk: bool) -> Field {
    let mut field = vec![u16::MAX; (COLS * ROWS) as usize];
    let free = |tx: i32, ty: i32| {
        (0..COLS).contains(&tx)
            && (0..ROWS).contains(&ty)
            && (ground.open(tile(tx, ty)) || (folk && tile(tx, ty) == Tile::Gate))
            && crate_at != Some((tx, ty))
    };
    if !free(goal.0, goal.1) {
        return field;
    }
    field[index(goal.0, goal.1)] = 0;
    let mut queue = VecDeque::from([goal]);
    while let Some((x, y)) = queue.pop_front() {
        let here = field[index(x, y)];
        for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let (nx, ny) = (x + dx, y + dy);
            if free(nx, ny) && field[index(nx, ny)] == u16::MAX {
                field[index(nx, ny)] = here + 1;
                queue.push_back((nx, ny));
            }
        }
    }
    field
}

type FieldKey = ((i32, i32), u8, Option<(i32, i32)>, bool);

/// The flood from `goal` (cached: a pure function of its arguments).
pub fn field(goal: (i32, i32), ground: Ground, crate_at: Option<(i32, i32)>) -> std::sync::Arc<Field> {
    static CACHE: OnceLock<Mutex<BTreeMap<FieldKey, std::sync::Arc<Field>>>> = OnceLock::new();
    let key = (
        goal,
        u8::from(ground.bridge_up) | u8::from(ground.swift) << 1 | u8::from(ground.ditch_filled) << 2,
        crate_at,
        false,
    );
    if let Some(f) = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return f.clone();
    }
    let made = std::sync::Arc::new(flood(goal, ground, crate_at, false));
    CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, made.clone());
    made
}

/// The way from one spot to another a villager walks, as pixel points
/// through tile middles (by day: the bridge is down, the planks are shut;
/// the east gate is theirs to pass).
pub fn route(from: Spot, to: Spot) -> std::sync::Arc<Vec<(i32, i32)>> {
    type Routes = BTreeMap<(Spot, Spot), std::sync::Arc<Vec<(i32, i32)>>>;
    static CACHE: OnceLock<Mutex<Routes>> = OnceLock::new();
    if let Some(r) = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).get(&(from, to)) {
        return r.clone();
    }
    let made = {
        {
            let day = Ground::default();
            let flood = flood(to.tile(), day, None, true);
            let (mut x, mut y) = from.tile();
            let mut out = vec![(x * TILE + TILE / 2, y * TILE + TILE / 2)];
            while (x, y) != to.tile() && flood[index(x, y)] != u16::MAX {
                let here = flood[index(x, y)];
                let step = [(0, -1), (0, 1), (-1, 0), (1, 0)].into_iter().find(|&(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..COLS).contains(&nx) && (0..ROWS).contains(&ny) && flood[index(nx, ny)].saturating_add(1) == here
                });
                let Some((dx, dy)) = step else { break };
                x += dx;
                y += dy;
                out.push((x * TILE + TILE / 2, y * TILE + TILE / 2));
            }
            std::sync::Arc::new(out)
        }
    };
    CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert((from, to), made.clone());
    made
}

/// The length of a route in pixels.
pub fn length(route: &[(i32, i32)]) -> i32 {
    route
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_is_a_full_rectangle_and_every_building_stands_where_its_table_says() {
        assert!(MAP.iter().all(|row| row.len() == COLS as usize));
        for h in &HOUSES {
            for ty in h.y..h.y + h.h {
                for tx in h.x..h.x + h.w {
                    assert_eq!(tile(tx, ty), Tile::Wall(h.ch), "{} at {tx},{ty}", h.name);
                }
            }
        }
        assert_eq!(tile(CRATE_START.0, CRATE_START.1), Tile::Cobble);
        assert_eq!(tile(DITCH.0, DITCH.1), Tile::Ditch);
    }

    #[test]
    fn by_day_the_bridge_carries_and_by_night_only_a_light_knight_crosses() {
        let reach = |ground: Ground| {
            let f = flood(Spot::Plaza.tile(), ground, None, false);
            move |s: Spot| f[index(s.tile().0, s.tile().1)] != u16::MAX
        };
        let day = reach(Ground::default());
        for spot in [Spot::ScriptDoor, Spot::ReedBank, Spot::HutDoor, Spot::NorthYard, Spot::EastGate] {
            assert!(day(spot), "{spot:?} by day");
        }
        assert!(!day(Spot::SouthYard), "the south tower waits behind the ditch");
        let night = reach(Ground { bridge_up: true, ..Ground::default() });
        assert!(!night(Spot::NorthYard) && !night(Spot::ReedBank));
        let light = reach(Ground { bridge_up: true, swift: true, ..Ground::default() });
        assert!(light(Spot::NorthYard) && light(Spot::ReedBank));
        let filled = reach(Ground { ditch_filled: true, ..Ground::default() });
        assert!(filled(Spot::SouthYard));
    }

    #[test]
    fn a_villager_route_runs_between_any_two_daytime_spots() {
        let r = route(Spot::ScriptDesk, Spot::BakeryAt);
        assert!(r.len() > 10 && length(&r) > 200);
        assert_eq!(r.first().copied(), Some(Spot::ScriptDesk.px()));
        assert_eq!(r.last().copied(), Some(Spot::BakeryAt.px()));
        let out = route(Spot::EastOut, Spot::EastGate);
        assert!(out.len() >= 3, "through the gate");
    }

    #[test]
    fn a_knights_feet_are_stopped_by_walls_and_water() {
        let at = |t: i32| (t * TILE + 8) * Q;
        assert!(clear(at(17), at(15), Ground::default(), &[]));
        assert!(!clear(at(17), at(12), Ground::default(), &[]), "the well");
        assert!(!clear(at(17), at(8), Ground { bridge_up: true, ..Ground::default() }, &[]));
        assert!(!clear(at(20), at(17), Ground::default(), &[(20, 17)]), "a crate");
    }
}
