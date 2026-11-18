//! The default realm: an open kingdom of eleven biomes (`super::biome`) on
//! ten by eleven chunks of 24x16 tiles, with the authored Zelda screens set
//! in it as set pieces, wide roads between them (`super::roads`) and grown
//! ground everywhere else (`super::wild`): groves, outcrops, meres and the
//! rivers and ridges along the biome borders. The March, along the south,
//! holds the model houses' castles (`super::houses`). Every castle, the
//! Keep and the Dragon Keep among them, is a great walled exterior of
//! closed ground (`super::castles`). The design is `docs/KINGDOM-OPEN.md`.
//!
//! Each authored screen sits centred in its own chunk. Place footprints and
//! stand tiles are written in the authored screens' own coordinates and
//! moved into the realm by [`place_tile`] / [`place_px`].
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
//!
//! All of the above is the **full plan**. Under the blank-slate plan
//! (`super::plan`, the one switch) the realm is open meadow from edge to
//! edge: no set pieces stand, no authored place is on the map, and the
//! tables here are kept, not read.

use std::sync::OnceLock;

pub const TILE: i32 = 16;
pub const SCREEN_W: i32 = 16;
pub const SCREEN_H: i32 = 11;
pub const AUTHORED_X: i32 = 4;
pub const AUTHORED_Y: i32 = 4;
pub const CHUNK_W: i32 = 24;
pub const CHUNK_H: i32 = 16;
pub const CHUNKS_X: i32 = 10;
pub const CHUNKS_Y: i32 = 11;
pub const MAP_W: i32 = CHUNK_W * CHUNKS_X;
pub const MAP_H: i32 = CHUNK_H * CHUNKS_Y;
pub const LEGACY_MAP: (i32, i32) = (112, 121);

const SET_PIECES: [Option<(i32, i32)>; (AUTHORED_X * AUTHORED_Y) as usize] = [
    Some((1, 1)),
    Some((4, 1)),
    Some((7, 1)),
    None,
    Some((0, 3)),
    Some((3, 3)),
    Some((6, 3)),
    Some((9, 4)),
    Some((0, 5)),
    Some((3, 5)),
    Some((5, 7)),
    None,
    Some((2, 5)),
    Some((4, 5)),
    Some((5, 5)),
    Some((7, 5)),
];

const SEA_CHUNKS: [(i32, i32); 2] = [(9, 0), (9, 7)];

pub fn screen_origin(ax: i32, ay: i32) -> (i32, i32) {
    let (ax, ay) = (ax.clamp(0, AUTHORED_X - 1), ay.clamp(0, AUTHORED_Y - 1));
    let (cx, cy) = SET_PIECES[(ay * AUTHORED_X + ax) as usize]
        .unwrap_or(SEA_CHUNKS[usize::from(ay >= 2)]);
    (
        cx * CHUNK_W + (CHUNK_W - SCREEN_W) / 2,
        cy * CHUNK_H + (CHUNK_H - SCREEN_H) / 2,
    )
}

pub fn set_pieces() -> impl Iterator<Item = ((i32, i32), (i32, i32))> {
    let stands = super::plan::full();
    (0..AUTHORED_Y).flat_map(move |ay| {
        (0..AUTHORED_X)
            .filter(move |&ax| stands && SET_PIECES[(ay * AUTHORED_X + ax) as usize].is_some())
            .map(move |ax| ((ax, ay), screen_origin(ax, ay)))
    })
}

pub fn structures() -> &'static [(i32, i32, i32, i32)] {
    static IN_REALM: OnceLock<Vec<(i32, i32, i32, i32)>> = OnceLock::new();
    IN_REALM
        .get_or_init(|| {
            if !super::plan::full() {
                return Vec::new();
            }
            STRUCTURES
                .iter()
                .map(|&(ax, ay, w, h)| {
                    let (x, y) = place_tile(ax, ay);
                    (x, y, w, h)
                })
                .collect()
        })
        .as_slice()
}

pub fn in_set_piece(x: i32, y: i32) -> bool {
    set_pieces().any(|(_, (ox, oy))| {
        (ox..ox + SCREEN_W).contains(&x) && (oy..oy + SCREEN_H).contains(&y)
    })
}

pub fn place_tile(tx: i32, ty: i32) -> (i32, i32) {
    let (ax, ay) = (
        tx.div_euclid(SCREEN_W).clamp(0, AUTHORED_X - 1),
        ty.div_euclid(SCREEN_H).clamp(0, AUTHORED_Y - 1),
    );
    let (ox, oy) = screen_origin(ax, ay);
    (ox + tx - ax * SCREEN_W, oy + ty - ay * SCREEN_H)
}

pub fn place_px(x: i32, y: i32) -> (i32, i32) {
    let (sw, sh) = (SCREEN_W * TILE, SCREEN_H * TILE);
    let (ax, ay) = (
        x.div_euclid(sw).clamp(0, AUTHORED_X - 1),
        y.div_euclid(sh).clamp(0, AUTHORED_Y - 1),
    );
    let (ox, oy) = screen_origin(ax, ay);
    (ox * TILE + x - ax * sw, oy * TILE + y - ay * sh)
}

const AUTHORED: [[&str; SCREEN_H as usize]; (AUTHORED_X * AUTHORED_Y) as usize] = [
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
        "TTTTTTT=TTTTTTTT",
    ],
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
    [
        "TTTTTTTTTTTTTTTT",
        "TT...cccc....TTT",
        "T....cccc.....TT",
        "T....cccc......T",
        "T..............T",
        "T.....==========",
        "T.cccc=........T",
        "T.cccc=..cccc..T",
        "T.....=..cccc..T",
        "TT....=.......TT",
        "TTTTTT=TTTTTTTTT",
    ],
    [
        "TTTTTTT=TTTTTTTT",
        "T......=.......T",
        "T......=.......T",
        "T.::::.=.::::..T",
        "T.:::::=::::::.T",
        "================",
        "T.::::::::::::.T",
        "T......=.......T",
        "T......=.......T",
        "T.cccc.=.cccc..T",
        "TTTTTTT=TTTTTTTT",
    ],
    [
        "TTTTTTTTTTTTTTTT",
        "T..............T",
        "T..............T",
        "T..............T",
        "T..............T",
        "===..........===",
        "T.=..........=.T",
        "T.=..........=.T",
        "T.============.T",
        "T...........=..T",
        "TTTTTTTTTTTT=TTT",
    ],
    [
        "TTTTTTTTTTTTTTTT",
        "T..............T",
        "T..............T",
        "T..............T",
        "T..............T",
        "===............T",
        "T.=............T",
        "T.=............T",
        "T.============.T",
        "T..=...........T",
        "TTT=TTTTTTTTTTTT",
    ],
];

pub struct Realm {
    tiles: Vec<u8>,
}

impl Realm {
    pub fn get() -> &'static Realm {
        static REALM: OnceLock<Realm> = OnceLock::new();
        REALM.get_or_init(Realm::build)
    }

    fn build() -> Realm {
        if super::plan::tabula() {
            return Realm::tabula();
        }
        let roads = super::roads::raster();
        let mut tiles = super::wild::grow(&roads);
        for ((ax, ay), (ox, oy)) in set_pieces() {
            for (ly, row) in Realm::authored_rows(ax, ay).iter().enumerate() {
                for (lx, b) in row.bytes().enumerate() {
                    let rim = lx == 0 || ly == 0 || lx == SCREEN_W as usize - 1 || ly == SCREEN_H as usize - 1;
                    if rim && b == b'T' {
                        continue;
                    }
                    let (x, y) = (ox + lx as i32, oy + ly as i32);
                    tiles[(y * MAP_W + x) as usize] = b;
                }
            }
        }
        super::wild::finish(&mut tiles);
        for (foot, ground) in [(super::castles::KEEP, b'.'), (super::castles::DRAGON_KEEP, b'a')] {
            let (tx, ty, tw, th) = foot;
            for y in ty..ty + th {
                for x in tx..tx + tw {
                    tiles[(y * MAP_W + x) as usize] = ground;
                }
            }
        }
        super::roads::lay(&mut tiles, &roads);
        Realm { tiles }
    }

    pub fn tabula() -> Realm {
        Realm { tiles: super::plan::tabula::terrain() }
    }

    pub fn at(&self, x: i32, y: i32) -> u8 {
        let (x, y) = (x.clamp(0, MAP_W - 1), y.clamp(0, MAP_H - 1));
        self.tiles[(y * MAP_W + x) as usize]
    }

    pub fn at_px(&self, wx: i32, wy: i32) -> u8 {
        self.at(wx.div_euclid(TILE), wy.div_euclid(TILE))
    }

    pub fn authored_rows(ax: i32, ay: i32) -> &'static [&'static str; SCREEN_H as usize] {
        &AUTHORED[(ay.clamp(0, AUTHORED_Y - 1) * AUTHORED_X + ax.clamp(0, AUTHORED_X - 1)) as usize]
    }
}

pub fn walkable(t: u8) -> bool {
    matches!(t, b'=' | b':' | b'H' | b'.' | b's' | b'c' | b'a' | b'%')
}

pub const STRUCTURES: [(i32, i32, i32, i32); 44] = [
    (22, 12, 3, 3),
    (21, 12, 1, 3),
    (25, 12, 1, 3),
    (26, 12, 1, 2),
    (28, 12, 2, 2),
    (17, 12, 2, 2),
    (17, 17, 2, 2),
    (28, 17, 2, 2),
    (25, 17, 1, 1),
    (20, 17, 1, 2),
    (26, 20, 2, 1),
    (5, 14, 2, 2),
    (9, 2, 2, 2),
    (23, 1, 2, 2),
    (39, 2, 3, 3),
    (36, 19, 1, 1),
    (41, 19, 3, 1),
    (42, 26, 2, 2),
    (18, 24, 2, 2),
    (21, 24, 2, 2),
    (25, 24, 2, 2),
    (28, 24, 2, 2),
    (18, 29, 2, 2),
    (21, 29, 2, 2),
    (24, 29, 2, 2),
    (27, 29, 2, 2),
    (30, 29, 1, 1),
    (51, 13, 2, 2),
    (56, 13, 2, 2),
    (58, 19, 2, 2),
    (17, 34, 2, 2),
    (20, 34, 2, 2),
    (25, 34, 2, 2),
    (28, 34, 2, 2),
    (17, 40, 3, 2),
    (21, 40, 2, 2),
    (24, 40, 2, 2),
    (27, 40, 3, 2),
    (30, 36, 1, 1),
    (35, 34, 10, 6),
    (50, 34, 2, 2),
    (60, 34, 2, 2),
    (60, 40, 2, 1),
    (9, 34, 5, 4),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Place {
    Keep,
    Gatehouse,
    Rookery,
    Scriptorium,
    Smithy,
    Chapel,
    RoundTable,
    Observatory,
    Lists,
    Mines,
    DragonKeep,
    DarkForest,
    Swamp,
    Fields,
    Village,
    Wards,
    ArtisanQuarter,
    Colosseum,
    Tournament,
    School,
}

impl Place {
    pub const ALL: [Place; 20] = [
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
        Place::ArtisanQuarter,
        Place::Colosseum,
        Place::Tournament,
        Place::School,
    ];

    pub fn on_map(self) -> bool {
        super::plan::full() || self == Place::Keep
    }

    pub fn label(self) -> &'static str {
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
            Place::ArtisanQuarter => "ARTISAN QUARTER",
            Place::Colosseum => "COLOSSEUM",
            Place::Tournament => "KNIGHTS TOURNAMENT",
            Place::School => "SLOPTOMIZER SCHOOL",
        }
    }

    pub fn footprint(self) -> (i32, i32, i32, i32) {
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
            Place::ArtisanQuarter => (17, 34, 13, 8),
            Place::Colosseum => (35, 34, 10, 6),
            Place::Tournament => (50, 34, 12, 7),
            Place::School => (9, 34, 5, 4),
            Place::DarkForest | Place::Swamp | Place::Fields => {
                let (x, y) = self.stand();
                (x, y, 1, 1)
            }
        }
    }

    pub fn stand(self) -> (i32, i32) {
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
            Place::ArtisanQuarter => (23, 38),
            Place::Colosseum => (40, 41),
            Place::Tournament => (55, 41),
            Place::School => (11, 38),
        }
    }

    pub fn stand_world(self) -> (i32, i32) {
        if super::plan::tabula() {
            return super::plan::tabula::KEEP_STAND;
        }
        let (tx, ty) = self.stand();
        place_tile(tx, ty)
    }

    pub fn footprint_world(self) -> (i32, i32, i32, i32) {
        if super::plan::tabula() {
            return super::plan::tabula::KEEP;
        }
        let (tx, ty, tw, th) = self.footprint();
        let (x, y) = place_tile(tx, ty);
        (x, y, tw, th)
    }

    pub fn screen(self) -> (i32, i32) {
        let (tx, ty) = self.stand_world();
        (tx / SCREEN_W, ty / SCREEN_H)
    }
}
