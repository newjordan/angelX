//! The default realm: twelve authored Zelda screens spread across a wider
//! grid with grown wilderness between them, and where every place stands.
//!
//! Authored screens sit at even grid coordinates; every other screen is
//! wilderness (`wild.rs`) that carries the authored roads through. Place
//! footprints and stand tiles are written in the authored screens' own
//! coordinates and moved into the realm by [`place_tile`] / [`place_px`].
//!
//! Terrain legend (one byte per tile):
//!
//! | byte | tile        | byte | tile          |
//! |------|-------------|------|---------------|
//! | `.`  | meadow      | `=`  | road          |
//! | `T`  | tree        | `:`  | cobble        |
//! | `^`  | rock        | `%`  | swamp         |
//! | `~`  | water       | `c`  | crops         |
//! | `s`  | shore sand  | `a`  | ash           |
//! | `d`  | dead tree   | `H`  | planks        |
//!
//! Buildings are drawn over meadow; their footprints live in [`Place`].

use std::sync::OnceLock;

use crate::stage::world_viz::Building;

pub(crate) const TILE: i32 = 16;
pub(crate) const SCREEN_W: i32 = 16;
pub(crate) const SCREEN_H: i32 = 11;
/// The authored screens' own grid.
pub(crate) const AUTHORED_X: i32 = 4;
pub(crate) const AUTHORED_Y: i32 = 3;
/// The realm's grid: an authored screen at every even coordinate,
/// wilderness between.
pub(crate) const SCREENS_X: i32 = AUTHORED_X * 2 - 1;
pub(crate) const SCREENS_Y: i32 = AUTHORED_Y * 2 - 1;
pub(crate) const MAP_W: i32 = SCREEN_W * SCREENS_X;
pub(crate) const MAP_H: i32 = SCREEN_H * SCREENS_Y;

/// An authored tile `(tx, ty)` moved to its place in the realm.
pub(crate) fn place_tile(tx: i32, ty: i32) -> (i32, i32) {
    (
        tx + tx.div_euclid(SCREEN_W) * SCREEN_W,
        ty + ty.div_euclid(SCREEN_H) * SCREEN_H,
    )
}

/// An authored pixel point moved to its place in the realm.
pub(crate) fn place_px(x: i32, y: i32) -> (i32, i32) {
    let (sw, sh) = (SCREEN_W * TILE, SCREEN_H * TILE);
    (x + x.div_euclid(sw) * sw, y + y.div_euclid(sh) * sh)
}

/// Authored screens, row-major: `(ax, ay)` at index `ay * AUTHORED_X + ax`.
const AUTHORED: [[&str; SCREEN_H as usize]; (AUTHORED_X * AUTHORED_Y) as usize] = [
    // (0,0) Observatory hill and the Dark Forest
    [
        "TTTTTTTTTTTTTTTT",
        "TTTTTTT.......^^",
        "TTTTT..........^",
        "TTTT...........^",
        "TTTT.....=.....T",
        "TTT.....==.....T",
        "TT..T...========",
        "TTT.....=......T",
        "TTTT..T.=...T..T",
        "TTTTT...=.....TT",
        "TTTTTTTT=TTTTTTT",
    ],
    // (1,0) The Mines
    [
        "^^^^^^^^^^^^^^^^",
        "^^^^^^^..^^^^^^^",
        "^^^^^^^..^^^^^^^",
        "^^^^^^^.=^^^^^^^",
        "^^^^....=....^^^",
        "^^^.....=.....^^",
        "================",
        "^^^.=........^^^",
        "^^^^=.......^^^^",
        "^^^^=.a..a.^^^^^",
        "^^^^=^^^^^^^^^^^",
    ],
    // (2,0) Dragon Keep
    [
        "^^^^^^^^^^^^^^^^",
        "^^aaadaaaaaadaa^",
        "^aaaaaaaaaaaaaa^",
        "^adaaaaaaaaaaad^",
        "^aaaaaaaaaaaaaa^",
        "^aaaaaaa=aaaaaa^",
        "========aadaaaa^",
        "^^aadaaaaaaaaa^^",
        "^^^aaaaaaadaa^^^",
        "^^^^^aaaaaa^^^^^",
        "^^^^^^^^^^^^^^^^",
    ],
    // (3,0) North coast
    [
        "~~~~~~~~~~~~~~~~",
        "~~~~~~~~~~~~~~~~",
        "^^~~~~~~~ss~~~~~",
        "^^s~~~~~sTTs~~~~",
        "^ss~~~~~~ss~~~~~",
        "^ss~~~~~~~~~~~~~",
        "^sss~~~~~~~~~~~~",
        "^^ss~~~~~~~~~~~~",
        "^^sss~~~~~~~~~~~",
        "^^^ss~~~~~~~~~~~",
        "^^^Tss~~~~~~~~~~",
    ],
    // (0,1) Gatehouse and harbour
    [
        "~~~sTTTT=TTTTTTT",
        "~~~sT...=......T",
        "~~~s....=......T",
        "~~~s....=......T",
        "~~~s============",
        "~~~s...........T",
        "~~~s..T.....T..T",
        "~~~ss..........T",
        "~~~~s...TT.....T",
        "~~~~ss........TT",
        "~~~~~sTTTTTTTTTT",
    ],
    // (1,1) Castle town
    [
        "TTTT=TTTTTTTTTTT",
        "T...=..........T",
        "T...=..........T",
        "T...=..........T",
        "================",
        "T.....:::......T",
        "T.....:::......T",
        "T.....:::......T",
        "T.=============T",
        "T......=.......T",
        "TTTTTTT=TTTTTTTT",
    ],
    // (2,1) The Lists
    [
        "TTTTTTTTTTTTTTTT",
        "T..............T",
        "T..............T",
        "T..............T",
        "===............T",
        "T.=............T",
        "T.=............T",
        "T.==============",
        "T......=.......T",
        "T......=.......T",
        "TTTTTTT=TTTTTTTT",
    ],
    // (3,1) Repo wards
    [
        "TTTTTTTTTTTTTTTT",
        "T.............s~",
        "T.............s~",
        "T.............s~",
        "T.............s~",
        "T...=....=....s~",
        "T...=....=....s~",
        "==============s~",
        "T.......=.....s~",
        "T.......=....ss~",
        "TTTTTTTTTTTTTss~",
    ],
    // (0,2) The Swamp
    [
        "~~~~~~TTTTTTTTTT",
        "~~~~%%%%%%%%%%%T",
        "~~~%%%~~%%%%%%%T",
        "~~%%%~~~%%d%%%%T",
        "~~%%%%~%%%%%%%%T",
        "~~%%%%%%%%%%====",
        "~~~%%d%%%~~%%%%T",
        "~~~%%%%%%~~~%%%T",
        "~~~~%%%%%%%%%%TT",
        "~~~~~%%%%%%%TTTT",
        "~~~~~~~~TTTTTTTT",
    ],
    // (1,2) The Village — the fleet
    [
        "TTTTTTT=TTTTTTTT",
        "T......=.......T",
        "T......=.......T",
        "T......=.......T",
        "T......=.......T",
        "================",
        "T..............T",
        "T..............T",
        "T..............T",
        "T..............T",
        "TTTTTTTTTTTTTTTT",
    ],
    // (2,2) Homecoming road and the fields
    [
        "TTTTTTT=TTTTTTTT",
        "T......=.......T",
        "T.cccc.=.ccccc.T",
        "T.cccc.=.ccccc.T",
        "T.cccc.=.......T",
        "========.......T",
        "T......=.cccc..T",
        "T.ccc..=.cccc..~",
        "T.ccc..=.......~",
        "T......=....ss~~",
        "TTTTTTTTTTTss~~~",
    ],
    // (3,2) South sea
    [
        "TTTTTTTTTTTss~~~",
        "TTTTTTTTsss~~~~~",
        "TTTTTsss~~~~~~~~",
        "TTTss~~~~~~~~~~~",
        "Tss~~~~~~~~~~~~~",
        "Ts~~~~~~~~~~~~~~",
        "Ts~~~~~~~~~ss~~~",
        "~~~~~~~~~~sTTs~~",
        "~~~~~~~~~~~ss~~~",
        "~~~~~~~~~~~~~~~~",
        "~~~~~~~~~~~~~~~~",
    ],
];

/// The terrain grid of the default realm.
pub(crate) struct Realm {
    tiles: Vec<u8>,
}

impl Realm {
    /// The default realm, built once.
    pub(crate) fn get() -> &'static Realm {
        static REALM: OnceLock<Realm> = OnceLock::new();
        REALM.get_or_init(Realm::build)
    }

    fn build() -> Realm {
        let mut tiles = vec![b'.'; (MAP_W * MAP_H) as usize];
        for sy in 0..SCREENS_Y {
            for sx in 0..SCREENS_X {
                let rows = match authored_at(sx, sy) {
                    Some(rows) => rows,
                    None => super::wild::grow(sx, sy, &edges_of(sx, sy), lean_of(sx, sy)),
                };
                for (ly, row) in rows.iter().enumerate() {
                    for (lx, &b) in row.iter().enumerate() {
                        let (x, y) = (sx * SCREEN_W + lx as i32, sy * SCREEN_H + ly as i32);
                        tiles[(y * MAP_W + x) as usize] = b;
                    }
                }
            }
        }
        Realm { tiles }
    }

    /// Terrain at a tile; off-map reads clamp to the nearest edge tile so
    /// edges never grow seams.
    pub(crate) fn at(&self, x: i32, y: i32) -> u8 {
        let (x, y) = (x.clamp(0, MAP_W - 1), y.clamp(0, MAP_H - 1));
        self.tiles[(y * MAP_W + x) as usize]
    }

    /// Terrain under a world pixel.
    pub(crate) fn at_px(&self, wx: i32, wy: i32) -> u8 {
        self.at(wx.div_euclid(TILE), wy.div_euclid(TILE))
    }

    /// An authored screen's rows, in the authored grid.
    pub(crate) fn authored_rows(ax: i32, ay: i32) -> &'static [&'static str; SCREEN_H as usize] {
        &AUTHORED[(ay.clamp(0, AUTHORED_Y - 1) * AUTHORED_X + ax.clamp(0, AUTHORED_X - 1)) as usize]
    }
}

/// The authored screen at realm screen `(sx, sy)`, if one sits there.
fn authored_at(sx: i32, sy: i32) -> Option<super::wild::Rows> {
    if sx % 2 != 0 || sy % 2 != 0 || !(0..SCREENS_X).contains(&sx) || !(0..SCREENS_Y).contains(&sy)
    {
        return None;
    }
    let mut rows = [[b'.'; SCREEN_W as usize]; SCREEN_H as usize];
    for (ly, row) in Realm::authored_rows(sx / 2, sy / 2).iter().enumerate() {
        for (lx, b) in row.bytes().enumerate().take(SCREEN_W as usize) {
            rows[ly][lx] = b;
        }
    }
    Some(rows)
}

/// The borders a wild screen shares with authored neighbours.
fn edges_of(sx: i32, sy: i32) -> super::wild::Edges {
    let (w, h) = (SCREEN_W as usize, SCREEN_H as usize);
    let mut edges = super::wild::Edges::default();
    if let Some(n) = authored_at(sx, sy - 1) {
        edges.north = Some(n[h - 1]);
    }
    if let Some(s) = authored_at(sx, sy + 1) {
        edges.south = Some(s[0]);
    }
    if let Some(west) = authored_at(sx - 1, sy) {
        edges.west = Some(std::array::from_fn(|y| west[y][w - 1]));
    }
    if let Some(east) = authored_at(sx + 1, sy) {
        edges.east = Some(std::array::from_fn(|y| east[y][0]));
    }
    edges
}

/// What each wild screen leans toward, by where it lies: hills below the
/// mines, ash around the dragon's keep, marsh by the swamp, woods and
/// meadow elsewhere.
fn lean_of(sx: i32, sy: i32) -> super::wild::Lean {
    use super::wild::Lean;
    match (sx, sy) {
        (3, 0) | (2, 1) | (3, 1) => Lean::Hills,
        (5, 0) | (4, 1) | (5, 1) => Lean::Ash,
        (0, 3) | (1, 3) | (1, 4) => Lean::Marsh,
        (0, 1) | (1, 0) | (1, 1) | (5, 2) | (5, 3) => Lean::Forest,
        _ => Lean::Meadow,
    }
}

pub(crate) fn walkable(t: u8) -> bool {
    matches!(t, b'=' | b':' | b'H' | b'.' | b's' | b'c' | b'a' | b'%')
}

/// Every tile a structure stands on; the knight walks around them. Road
/// tiles inside a footprint (the gatehouse arch) stay open.
pub(crate) const STRUCTURES: [(i32, i32, i32, i32); 26] = [
    (22, 12, 3, 3), // keep
    (21, 12, 1, 3), // keep turrets
    (25, 12, 1, 3),
    (26, 12, 1, 2), // rookery
    (28, 12, 2, 2), // chapel
    (17, 12, 2, 2), // round table
    (17, 17, 2, 2), // smithy
    (28, 17, 2, 2), // scriptorium
    (25, 17, 1, 1), // well
    (20, 17, 1, 2), // market
    (26, 20, 2, 1), // garden
    (5, 14, 2, 2),  // gatehouse
    (9, 2, 2, 2),   // observatory
    (23, 1, 2, 2),  // mine mouth
    (39, 2, 3, 3),  // dragon keep
    (36, 19, 1, 1), // quintain
    (41, 19, 3, 1), // standings board
    (42, 26, 2, 2), // windmill
    (18, 24, 2, 2), // cottages
    (25, 24, 2, 2),
    (28, 24, 2, 2),
    (18, 29, 2, 2), // forge
    (27, 29, 2, 2), // granary
    (51, 13, 2, 2), // wards
    (56, 13, 2, 2),
    (58, 19, 2, 2),
];

/// Every place on the default realm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Place {
    Keep,
    Gatehouse,
    Rookery,
    Scriptorium,
    Smithy,
    Chapel,
    RoundTable,
    Observatory,
    /// The tournament ground and its quintain — the loop and the ladder.
    Lists,
    /// Coding and competition loops: the measured-candidates path.
    Mines,
    /// A submission in flight or an acceptance gate judging the work.
    DragonKeep,
    /// Research and exploration loops.
    DarkForest,
    /// Stalled loops, blocked verifiers, awaiting approval.
    Swamp,
    /// A finished loop walking the loot home.
    Fields,
    /// The fleet's hamlet: cottages for the heads, the forge, the granary.
    Village,
    Wards,
}

impl Place {
    pub(crate) const ALL: [Place; 16] = [
        Place::Keep,
        Place::Gatehouse,
        Place::Rookery,
        Place::Scriptorium,
        Place::Smithy,
        Place::Chapel,
        Place::RoundTable,
        Place::Observatory,
        Place::Lists,
        Place::Mines,
        Place::DragonKeep,
        Place::DarkForest,
        Place::Swamp,
        Place::Fields,
        Place::Village,
        Place::Wards,
    ];

    pub(crate) fn of_building(b: Building) -> Place {
        match b {
            Building::Keep => Place::Keep,
            Building::Gatehouse => Place::Gatehouse,
            Building::Rookery => Place::Rookery,
            Building::Scriptorium => Place::Scriptorium,
            Building::Smithy => Place::Smithy,
            Building::Chapel => Place::Chapel,
            Building::RoundTable => Place::RoundTable,
            Building::Observatory => Place::Observatory,
        }
    }

    /// HUD name.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Place::Keep => "KEEP",
            Place::Gatehouse => "GATEHOUSE",
            Place::Rookery => "ROOKERY",
            Place::Scriptorium => "SCRIPTORIUM",
            Place::Smithy => "SMITHY",
            Place::Chapel => "CHAPEL",
            Place::RoundTable => "ROUND TABLE",
            Place::Observatory => "OBSERVATORY",
            Place::Lists => "THE LISTS",
            Place::Mines => "THE MINES",
            Place::DragonKeep => "DRAGON KEEP",
            Place::DarkForest => "DARK FOREST",
            Place::Swamp => "THE SWAMP",
            Place::Fields => "HOMECOMING",
            Place::Village => "THE VILLAGE",
            Place::Wards => "REPO WARDS",
        }
    }

    /// Footprint in tiles: `(tx, ty, tw, th)`. Regions are marked by the
    /// tile the knight stands on.
    pub(crate) fn footprint(self) -> (i32, i32, i32, i32) {
        match self {
            Place::Keep => (22, 12, 3, 3),
            Place::Gatehouse => (5, 14, 2, 2),
            Place::Rookery => (26, 12, 1, 2),
            Place::Scriptorium => (28, 17, 2, 2),
            Place::Smithy => (17, 17, 2, 2),
            Place::Chapel => (28, 12, 2, 2),
            Place::RoundTable => (17, 12, 2, 2),
            Place::Observatory => (9, 2, 2, 2),
            Place::Lists => (35, 12, 12, 5),
            Place::Mines => (23, 1, 2, 2),
            Place::DragonKeep => (39, 2, 3, 3),
            Place::Village => (18, 24, 12, 7),
            Place::Wards => (51, 13, 9, 8),
            Place::DarkForest | Place::Swamp | Place::Fields => {
                let (x, y) = self.stand();
                (x, y, 1, 1)
            }
        }
    }

    /// The tile the knight stands on to work here — always on a road or open
    /// ground in front of the door.
    pub(crate) fn stand(self) -> (i32, i32) {
        match self {
            Place::Keep => (23, 15),
            Place::Gatehouse => (8, 15),
            Place::Rookery => (26, 15),
            Place::Scriptorium => (30, 19),
            Place::Smithy => (19, 19),
            Place::Chapel => (29, 15),
            Place::RoundTable => (20, 13),
            Place::Observatory => (9, 4),
            Place::Lists => (35, 19),
            Place::Mines => (24, 3),
            Place::DragonKeep => (40, 5),
            Place::DarkForest => (5, 7),
            Place::Swamp => (11, 27),
            Place::Fields => (39, 31),
            Place::Village => (23, 27),
            Place::Wards => (52, 18),
        }
    }

    /// The stand tile in the realm.
    pub(crate) fn stand_world(self) -> (i32, i32) {
        let (tx, ty) = self.stand();
        place_tile(tx, ty)
    }

    /// The footprint in the realm.
    pub(crate) fn footprint_world(self) -> (i32, i32, i32, i32) {
        let (tx, ty, tw, th) = self.footprint();
        let (x, y) = place_tile(tx, ty);
        (x, y, tw, th)
    }

    /// Which realm screen holds this place.
    pub(crate) fn screen(self) -> (i32, i32) {
        let (tx, ty) = self.stand_world();
        (tx / SCREEN_W, ty / SCREEN_H)
    }
}
