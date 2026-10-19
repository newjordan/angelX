//! Set dressing: the small stories the country tells between its places.
//! Clusters of non-interactive things, one or two to a chunk, each biome in
//! its own hand (a woodcutter's stump, a cairn, bleached bones, an upturned
//! boat, a sunken punt, hay stooks, a hitching rail, a packed-up wagon, a
//! milestone, a market barrow), so every screen of the open kingdom has
//! something to walk to and never a crowd.
//!
//! A **wild camp** is a small clearing (a bay three tiles wide where a find
//! may stand, `super::finds`) with its things round it and a footpath
//! wandering to the nearest road; a **wayside** is a thing or two beside a
//! road. Everything stands on open ground, off roads, lots, set pieces and
//! the castles, and two tiles or more from every door. Fixed for the realm
//! (placed from hashes of the map, not of a run), worked out once; the
//! drawing is `render::overworld::dressing`.

use super::biome::{self, Biome};
use super::kingdom::{self, Geography};
use super::map::{CHUNK_H, CHUNK_W, CHUNKS_X, CHUNKS_Y, MAP_H, MAP_W, Realm, SCREEN_H, SCREEN_W, in_set_piece, set_pieces};
use crate::render::overworld::ink::hash;
use crate::rules::Tile;
use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::sync::OnceLock;

/// What can stand in a cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Item {
    // Greenwood
    FallenLog,
    StumpAxe,
    MushroomRing,
    Hide,
    Woodpile,
    // The Highlands
    Cairn,
    Fold,
    Scree,
    WatchStub,
    // The Wastes
    Bones,
    BurntCart,
    AshPit,
    // The Coast
    Boat,
    Nets,
    Driftwood,
    TidePool,
    // The Fen
    Punt,
    EelTraps,
    Reeds,
    StiltHut,
    // The farm country
    Stooks,
    Scarecrow,
    Stile,
    Hives,
    // The Tilt-Yards
    Rail,
    LanceRack,
    // The Fairground
    Wagon,
    Bunting,
    Barrels,
    // The March
    BurntPost,
    Milestone,
    Cross,
    // Castle Town
    Barrow,
    Trough,
    Stocks,
    Stall,
    Sacks,
    Garden,
}

impl Item {
    /// Every kind of thing there is.
    pub const ALL: [Item; 38] = [
        Item::FallenLog,
        Item::StumpAxe,
        Item::MushroomRing,
        Item::Hide,
        Item::Woodpile,
        Item::Cairn,
        Item::Fold,
        Item::Scree,
        Item::WatchStub,
        Item::Bones,
        Item::BurntCart,
        Item::AshPit,
        Item::Boat,
        Item::Nets,
        Item::Driftwood,
        Item::TidePool,
        Item::Punt,
        Item::EelTraps,
        Item::Reeds,
        Item::StiltHut,
        Item::Stooks,
        Item::Scarecrow,
        Item::Stile,
        Item::Hives,
        Item::Rail,
        Item::LanceRack,
        Item::Wagon,
        Item::Bunting,
        Item::Barrels,
        Item::BurntPost,
        Item::Milestone,
        Item::Cross,
        Item::Barrow,
        Item::Trough,
        Item::Stocks,
        Item::Stall,
        Item::Sacks,
        Item::Garden,
    ];

    /// Whether this thing may stand on terrain `t`: open meadow, ash and
    /// shore for all; marsh for the Fen's; crop strips for the field's.
    fn stands_on(self, t: u8) -> bool {
        match t {
            b'.' | b'a' | b's' => true,
            b'%' => matches!(self, Item::Punt | Item::EelTraps | Item::Reeds | Item::StiltHut),
            b'c' => matches!(self, Item::Stooks | Item::Scarecrow | Item::Hives),
            _ => false,
        }
    }

    /// Tiles a wide thing takes either side of its foot.
    pub fn reach(self) -> i32 {
        match self {
            Item::Fold | Item::Boat | Item::Nets | Item::FallenLog | Item::Wagon | Item::Rail | Item::Punt | Item::Garden => 1,
            _ => 0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Item::FallenLog => "fallen log",
            Item::StumpAxe => "stump and axe",
            Item::MushroomRing => "mushroom ring",
            Item::Hide => "hunter's hide",
            Item::Woodpile => "woodpile",
            Item::Cairn => "cairn",
            Item::Fold => "sheepfold",
            Item::Scree => "scree",
            Item::WatchStub => "watchtower stub",
            Item::Bones => "bones",
            Item::BurntCart => "burnt cart",
            Item::AshPit => "ash pit",
            Item::Boat => "upturned boat",
            Item::Nets => "nets on poles",
            Item::Driftwood => "driftwood",
            Item::TidePool => "tide pool",
            Item::Punt => "sunken punt",
            Item::EelTraps => "eel traps",
            Item::Reeds => "reed stack",
            Item::StiltHut => "hut on stilts",
            Item::Stooks => "hay stooks",
            Item::Scarecrow => "scarecrow",
            Item::Stile => "stone stile",
            Item::Hives => "beehives",
            Item::Rail => "hitching rail",
            Item::LanceRack => "broken lance rack",
            Item::Wagon => "packed-up wagon",
            Item::Bunting => "bunting poles",
            Item::Barrels => "barrels",
            Item::BurntPost => "burnt watch post",
            Item::Milestone => "milestone",
            Item::Cross => "wayside cross",
            Item::Barrow => "market barrow",
            Item::Trough => "horse trough",
            Item::Stocks => "stocks",
            Item::Stall => "market stall",
            Item::Sacks => "sacks",
            Item::Garden => "garden plot",
        }
    }
}

/// One thing standing: what, on which tile (its foot), and whether it is
/// drawn turned about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub item: Item,
    pub at: (i32, i32),
    pub flip: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A clearing with a footpath to the road; a find may stand in it.
    Wild,
    /// A thing or two beside a road.
    Road,
}

/// One cluster: its centre (a wild camp's bay), the biome, the story told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camp {
    pub at: (i32, i32),
    pub biome: Biome,
    pub class: Class,
    pub story: &'static str,
}

/// Everything the realm is dressed with.
pub struct Dressing {
    /// Every thing, back to front (by the row its foot stands on).
    pub pieces: Vec<Piece>,
    pub camps: Vec<Camp>,
    /// The tiles of every footpath.
    pub trails: Vec<bool>,
    /// Which wild camp's clearing each tile lies in (its index + 1), or 0.
    bays: Vec<u16>,
}

impl Dressing {
    /// Whether tile `(x, y)` is a footpath.
    pub fn trail(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < MAP_W && y < MAP_H && self.trails[(y * MAP_W + x) as usize]
    }

    /// The camp whose clearing tile `(x, y)` lies in, if any.
    pub fn bay_at(&self, x: i32, y: i32) -> Option<&Camp> {
        if x < 0 || y < 0 || x >= MAP_W || y >= MAP_H {
            return None;
        }
        usize::from(self.bays[(y * MAP_W + x) as usize]).checked_sub(1).map(|i| &self.camps[i])
    }
}

/// The realm's dressing, worked out once.
pub fn get() -> &'static Dressing {
    static DRESSING: OnceLock<Dressing> = OnceLock::new();
    DRESSING.get_or_init(|| build(&kingdom::geography(&[])))
}

struct Story {
    name: &'static str,
    class: Class,
    /// Each thing and its tile, from the camp's centre.
    items: &'static [(Item, i8, i8)],
}

const fn wild(name: &'static str, items: &'static [(Item, i8, i8)]) -> Story {
    Story { name, class: Class::Wild, items }
}

const fn way(name: &'static str, items: &'static [(Item, i8, i8)]) -> Story {
    Story { name, class: Class::Road, items }
}

/// Each biome's stories, in `Biome::ALL` order.
const STORIES: [&[Story]; 11] = [
    // Castle Town
    &[
        way("market barrows", &[(Item::Barrow, 0, 0), (Item::Trough, 3, 1)]),
        way("the stocks", &[(Item::Stocks, 0, 0), (Item::Barrow, -3, 1)]),
        way("a trough and a rail", &[(Item::Trough, 0, 0), (Item::Rail, -3, 1)]),
        way("a market stall", &[(Item::Stall, 0, 0), (Item::Sacks, 3, 1), (Item::Barrow, -3, 1)]),
        way("a town garden", &[(Item::Garden, 0, 0), (Item::Barrels, 3, 1)]),
    ],
    // Greenwood
    &[
        wild("woodcutter's camp", &[(Item::StumpAxe, -3, 0), (Item::FallenLog, 3, 1), (Item::Woodpile, 3, -2)]),
        wild("mushroom glade", &[(Item::MushroomRing, -3, 1), (Item::FallenLog, 3, 0), (Item::StumpAxe, -2, -2)]),
        wild("hunter's hide", &[(Item::Hide, 3, -1), (Item::FallenLog, -3, 1), (Item::MushroomRing, 2, 2)]),
        way("a felled tree", &[(Item::StumpAxe, 0, 0), (Item::Woodpile, 3, 1)]),
    ],
    // The Highlands
    &[
        wild("cairns", &[(Item::Cairn, -3, 0), (Item::Cairn, 3, 1), (Item::Scree, 0, 2)]),
        wild("shepherd's fold", &[(Item::Fold, -4, 1), (Item::Scree, 3, -1), (Item::Cairn, 3, 2)]),
        wild("watchtower stub", &[(Item::WatchStub, -3, -1), (Item::Scree, 3, 2), (Item::Cairn, 3, 0)]),
        wild("prospector's dig", &[(Item::Barrow, -3, 0), (Item::Scree, 3, 1), (Item::Woodpile, 0, -2)]),
        way("a waymarker cairn", &[(Item::Cairn, 0, 0), (Item::Scree, 3, 1)]),
    ],
    // The Wastes
    &[
        wild("bone field", &[(Item::Bones, -3, 1), (Item::Bones, 3, 0), (Item::Scree, 0, 2)]),
        wild("burnt cart", &[(Item::BurntCart, -3, 0), (Item::AshPit, 3, 1), (Item::Bones, 2, -2)]),
        wild("ash pits", &[(Item::AshPit, -3, 1), (Item::Scree, 3, 0), (Item::BurntPost, 0, -2)]),
        wild("a knight's grave", &[(Item::Cross, -3, 0), (Item::Cairn, 3, 1), (Item::Bones, 0, 2)]),
        wild("signal pyre", &[(Item::BurntPost, -3, 0), (Item::Woodpile, 3, 1), (Item::AshPit, 0, 2)]),
        way("a roadside heap of bones", &[(Item::Bones, 0, 0), (Item::Scree, 3, 1)]),
        way("a charred post", &[(Item::BurntPost, 0, 0), (Item::AshPit, 3, 1)]),
    ],
    // The Coast
    &[
        wild("beached boat", &[(Item::Boat, -3, 1), (Item::Driftwood, 3, 0), (Item::TidePool, 1, 2)]),
        wild("net-menders", &[(Item::Nets, -3, 0), (Item::Boat, 3, 1), (Item::Driftwood, 0, -2)]),
        wild("tide pools", &[(Item::TidePool, -3, 1), (Item::TidePool, 3, 0), (Item::Driftwood, 0, 2)]),
        wild("crab pots", &[(Item::EelTraps, -3, 0), (Item::Nets, 3, 1), (Item::Barrels, 0, 2)]),
        way("a drying net", &[(Item::Nets, 0, 0), (Item::Driftwood, 3, 1)]),
    ],
    // The Fen
    &[
        wild("sunken punt", &[(Item::Punt, -3, 1), (Item::EelTraps, 3, 0), (Item::Reeds, 2, -2)]),
        wild("stilt hut", &[(Item::StiltHut, -3, -1), (Item::EelTraps, 3, 1), (Item::Reeds, 0, 2)]),
        wild("reed cutters", &[(Item::Reeds, -3, 0), (Item::Reeds, 3, 1), (Item::Punt, 0, -2)]),
        way("eel traps", &[(Item::EelTraps, 0, 0), (Item::Reeds, 3, 1)]),
    ],
    // The Tilt-Yards
    &[
        wild("the horse lines", &[(Item::Rail, -3, 1), (Item::LanceRack, 3, 0), (Item::Trough, 0, -2)]),
        wild("a paddock end", &[(Item::Rail, -3, 0), (Item::Rail, 3, 1), (Item::Barrels, 0, 2)]),
        wild("farrier's corner", &[(Item::Trough, -3, 1), (Item::Woodpile, 3, 0), (Item::Barrels, 0, 2)]),
        way("hitching rail", &[(Item::Rail, 0, 0), (Item::LanceRack, 3, 0)]),
        way("spare lances", &[(Item::LanceRack, 0, 0), (Item::Rail, 3, 1)]),
    ],
    // The Fairground
    &[
        wild("packed-up wagon", &[(Item::Wagon, -3, 0), (Item::Barrels, 3, 1), (Item::Bunting, 2, -2)]),
        wild("bunting poles", &[(Item::Bunting, -3, 0), (Item::Bunting, 3, 0), (Item::Barrels, 0, 2)]),
        wild("shuttered stalls", &[(Item::Stall, -3, 0), (Item::Stall, 3, 1), (Item::Sacks, 0, 2)]),
        way("stacked barrels", &[(Item::Barrels, 0, 0), (Item::Bunting, 3, 0)]),
        way("a fair barrow", &[(Item::Barrow, 0, 0), (Item::Sacks, 3, 1)]),
    ],
    // Wind Country
    &[
        wild("hay field", &[(Item::Stooks, -3, 0), (Item::Stooks, 3, 1), (Item::Stile, 0, 2)]),
        wild("bee garden", &[(Item::Hives, -3, 0), (Item::Stile, 3, 1), (Item::Stooks, 0, -2)]),
        wild("scarecrow", &[(Item::Scarecrow, -3, 0), (Item::Stooks, 3, 0), (Item::Stile, 0, 2)]),
        wild("sheep pen", &[(Item::Fold, -4, 1), (Item::Stile, 3, 0), (Item::Stooks, 3, 2)]),
        way("hay stooks", &[(Item::Stooks, 0, 0), (Item::Stile, 3, 1)]),
    ],
    // Longbow Country
    &[
        wild("bee garden", &[(Item::Hives, -3, 0), (Item::Stile, 3, 1), (Item::Stooks, 0, -2)]),
        wild("scarecrow", &[(Item::Scarecrow, -3, 0), (Item::Stooks, 3, 0), (Item::Stile, 0, 2)]),
        wild("hay field", &[(Item::Stooks, -3, 0), (Item::Stooks, 3, 1), (Item::Hives, 0, 2)]),
        wild("fowler's corner", &[(Item::Hide, 3, -1), (Item::Stile, -3, 1), (Item::Stooks, 0, 2)]),
        way("a stile", &[(Item::Stile, 0, 0), (Item::Stooks, 3, 1)]),
    ],
    // The March
    &[
        wild("burnt watch post", &[(Item::BurntPost, -3, 0), (Item::Cairn, 3, 1), (Item::Cross, 0, -2)]),
        wild("wayside cross", &[(Item::Cross, -3, 0), (Item::Milestone, 3, 1), (Item::Cairn, 0, 2)]),
        wild("wrecked supply cart", &[(Item::BurntCart, -3, 0), (Item::Barrels, 3, 1), (Item::Woodpile, 0, 2)]),
        wild("picket camp", &[(Item::AshPit, -3, 1), (Item::Rail, 3, 0), (Item::Woodpile, 0, -2)]),
        way("milestone", &[(Item::Milestone, 0, 0), (Item::Cross, 3, 1)]),
        way("a burnt post", &[(Item::BurntPost, 0, 0), (Item::Milestone, 3, 1)]),
        way("a boundary stile", &[(Item::Stile, 0, 0), (Item::Milestone, 3, 1)]),
        way("a dump of barrels", &[(Item::Barrels, 0, 0), (Item::Woodpile, 3, 1)]),
    ],
];

/// How many wild camps and waysides a chunk of each biome is asked for.
fn quota(b: Biome) -> (usize, usize) {
    match b {
        Biome::CastleTown => (0, 2),
        Biome::Greenwood | Biome::Fen => (3, 0),
        Biome::Highlands | Biome::Wastes | Biome::Coast => (3, 1),
        Biome::TiltYards | Biome::Fairground | Biome::WindCountry | Biome::Longbow | Biome::March => (2, 1),
    }
}

/// The tiles a thing may shift to when the story's own is taken: its own,
/// then each way round it.
const SLOTS: [(i32, i32); 13] =
    [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1), (2, 0), (-2, 0), (0, 2), (0, -2)];

/// No two wild camps nearer than this (tiles); wayside things may stand
/// closer.
const SPACING: i32 = 9;
const WAY_SPACING: i32 = 8;
/// A wild camp stands at least this far (road steps) from any road.
const WILD_OFF: u8 = 5;
/// How far (tiles, across and down) from a spot something to walk to must
/// be: about the half of a screen either way, a little less.
const GAP: (i32, i32) = (10, 6);
/// No view of 24 x 14 tiles holds more things than this, nor more of one
/// kind than [`SAME`].
const CROWD: usize = 9;
const SAME: usize = 4;
/// A trail longer than this is not laid.
const TRAIL_MAX: u32 = 34;

fn idx(x: i32, y: i32) -> usize {
    (y * MAP_W + x) as usize
}

fn inside(x: i32, y: i32) -> bool {
    x >= 2 && y >= 2 && x < MAP_W - 2 && y < MAP_H - 2
}

fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Steps from each tile to the nearest road, up to 8.
fn road_steps(geo: &Geography) -> Vec<u8> {
    let mut near = vec![8u8; (MAP_W * MAP_H) as usize];
    let mut frontier: Vec<(i32, i32)> = (0..MAP_H)
        .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
        .filter(|&t| kingdom::is_road(geo, t))
        .collect();
    for &(x, y) in &frontier {
        near[idx(x, y)] = 0;
    }
    for step in 1..8u8 {
        let mut next = Vec::new();
        for (x, y) in frontier {
            for n in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if n.0 >= 0 && n.1 >= 0 && n.0 < MAP_W && n.1 < MAP_H && near[idx(n.0, n.1)] > step {
                    near[idx(n.0, n.1)] = step;
                    next.push(n);
                }
            }
        }
        frontier = next;
    }
    near
}

/// Where nothing may stand: set pieces, lots, doors, castles, the College,
/// the Delve's gate, fingerposts, the regions' names and the roads' verges.
fn keep_out(geo: &Geography) -> Vec<bool> {
    let mut no = vec![false; (MAP_W * MAP_H) as usize];
    let mut mark = |x0: i32, y0: i32, x1: i32, y1: i32| {
        for y in y0.max(0)..y1.min(MAP_H) {
            for x in x0.max(0)..x1.min(MAP_W) {
                no[idx(x, y)] = true;
            }
        }
    };
    for (_, (ox, oy)) in set_pieces() {
        mark(ox - 2, oy - 2, ox + SCREEN_W + 2, oy + SCREEN_H + 2);
    }
    for site in &geo.sites {
        // A door keeps three tiles clear all round: well over two.
        mark(site.stand.0 - 3, site.stand.1 - 3, site.stand.0 + 4, site.stand.1 + 4);
        if let Some(l) = site.lot {
            mark(l.tx - 3, l.ty - 3, l.tx + l.tw + 3, l.ty + l.th + 4);
        }
    }
    for (tx, ty, tw, th) in super::castles::footprints() {
        mark(tx - 3, ty - 3, tx + tw + 3, ty + th + 5);
    }
    let (school, gate) = (super::college::lot(), super::wishes::delve_gate());
    for l in [school, gate] {
        mark(l.tx - 3, l.ty - 3, l.tx + l.tw + 3, l.ty + l.th + 4);
    }
    for &(x, y) in super::college::approach() {
        mark(x - 2, y - 2, x + 3, y + 3);
    }
    for post in super::signs::posts() {
        mark(post.at.0 - 2, post.at.1 - 2, post.at.0 + 3, post.at.1 + 3);
    }
    for (_, (x, y)) in kingdom::regions() {
        mark(x - 9, y - 2, x + 10, y + 4);
    }
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if kingdom::is_road(geo, (x, y)) {
                mark(x - 2, y - 2, x + 3, y + 3);
            }
        }
    }
    no
}

struct Planner<'a> {
    geo: &'a Geography,
    realm: &'static Realm,
    no: Vec<bool>,
    claimed: Vec<bool>,
    near: Vec<u8>,
    spots: Vec<bool>,
}

impl Planner<'_> {
    /// Open ground a thing of `item`'s kind may stand on at `(x, y)`.
    fn open(&self, item: Option<Item>, x: i32, y: i32) -> bool {
        inside(x, y)
            && !self.no[idx(x, y)]
            && !self.claimed[idx(x, y)]
            && self.geo.room.tile(x, y) == Tile::Floor
            && !in_set_piece(x, y)
            && match item {
                Some(item) => item.stands_on(self.realm.at(x, y)),
                None => matches!(self.realm.at(x, y), b'.' | b'a' | b's'),
            }
    }

    /// Try to raise `story` with its centre at `at`; the pieces if enough
    /// of its things find open ground.
    fn raise(&self, story: &Story, at: (i32, i32), mirror: bool, far: u8) -> Option<Vec<Piece>> {
        let wild = story.class == Class::Wild;
        if !self.open(None, at.0, at.1) {
            return None;
        }
        if wild {
            // The bay: three tiles each way, all open, and a spot a find
            // may stand on.
            if !self.spots[idx(at.0, at.1)] {
                return None;
            }
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if !self.open(None, at.0 + dx, at.1 + dy) {
                        return None;
                    }
                }
            }
        } else if !(3..=far).contains(&self.near[idx(at.0, at.1)]) {
            return None;
        }
        let mut pieces: Vec<Piece> = Vec::new();
        for (k, &(item, dx, dy)) in story.items.iter().enumerate() {
            let dx = if mirror { -i32::from(dx) } else { i32::from(dx) };
            let reach = item.reach();
            // Where the story has it, else the nearest open ground to that.
            for (ox, oy) in SLOTS {
                let (x, y) = (at.0 + dx + ox, at.1 + i32::from(dy) + oy);
                let clear = (!wild || chebyshev((x, y), at) >= 2 + reach)
                    && pieces.iter().all(|p| chebyshev(p.at, (x, y)) >= 2 + p.item.reach() + reach)
                    && (x - reach..=x + reach).all(|tx| self.open(Some(item), tx, y));
                if clear {
                    let flip = hash(x, y, 905 + k as u32) % 2 == 0;
                    pieces.push(Piece { item, at: (x, y), flip });
                    break;
                }
            }
        }
        let need = if story.items.len() >= 3 { 2 } else { 1 };
        (pieces.len() >= need).then_some(pieces)
    }

    fn claim(&mut self, camp: &Camp, pieces: &[Piece]) {
        let (cx, cy) = camp.at;
        if camp.class == Class::Wild {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    self.claimed[idx(cx + dx, cy + dy)] = true;
                }
            }
        }
        for p in pieces {
            let reach = p.item.reach();
            for x in p.at.0 - reach..=p.at.0 + reach {
                self.claimed[idx(x, p.at.1)] = true;
            }
        }
    }
}

impl Planner<'_> {
    /// Raise one cluster of `class` for biome `home` with its centre at
    /// `at` if any of the biome's stories of that class fits there, and
    /// no other camp of its class stands within the class's spacing.
    fn place(&mut self, out: &mut (Vec<Camp>, Vec<Piece>), home: Biome, at: (i32, i32), class: Class, far: u8) -> bool {
        let gap = if class == Class::Wild { SPACING } else { WAY_SPACING };
        if out.0.iter().any(|c| chebyshev(c.at, at) < gap) {
            return false;
        }

        let stories: Vec<&Story> = STORIES[usize::from(home.index())].iter().filter(|s| s.class == class).collect();
        if stories.is_empty() {
            return false;
        }
        let first = hash(at.0, at.1, 813) as usize;
        let mirror = hash(at.0, at.1, 814) % 2 == 0;
        for s in 0..stories.len() {
            let story = stories[(first + s) % stories.len()];
            if let Some(raised) = self.raise(story, at, mirror, far) {
                // Never a crowd, never one thing over and over: no view's worth
                // of country is to hold more than [`CROWD`] things, or [`SAME`]
                // of a kind, with these in it.
                // Every view that would take one of the new things.
                let (x0, x1) = (raised.iter().map(|p| p.at.0).min().unwrap_or(at.0), raised.iter().map(|p| p.at.0).max().unwrap_or(at.0));
                let (y0, y1) = (raised.iter().map(|p| p.at.1).min().unwrap_or(at.1), raised.iter().map(|p| p.at.1).max().unwrap_or(at.1));
                let near: Vec<&Piece> =
                    out.1.iter().filter(|p| p.at.0 >= x0 - 24 && p.at.0 <= x1 + 24 && p.at.1 >= y0 - 14 && p.at.1 <= y1 + 14).collect();
                let crowded = (y0 - 7..=y1 + 7).any(|wy| {
                    (x0 - 12..=x1 + 12).any(|wx| {
                        let seen = |p: &&Piece| (p.at.0 - wx).abs() <= 12 && (p.at.1 - wy).abs() <= 7;
                        let all = || near.iter().copied().chain(raised.iter()).filter(seen);
                        all().count() > CROWD
                            || raised.iter().filter(seen).any(|r| all().filter(|p| p.item == r.item).count() > SAME)
                    })
                });
                if crowded {
                    continue;
                }
                let camp = Camp { at, biome: home, class, story: story.name };
                self.claim(&camp, &raised);
                out.0.push(camp);
                out.1.extend(raised);
                return true;
            }
        }
        false
    }
}

fn build(geo: &Geography) -> Dressing {
    let realm = Realm::get();
    let mut spots = vec![false; (MAP_W * MAP_H) as usize];
    for &(x, y) in super::finds::spots() {
        spots[idx(x, y)] = true;
    }
    let mut planner = Planner {
        geo,
        realm,
        no: keep_out(geo),
        claimed: vec![false; (MAP_W * MAP_H) as usize],
        near: road_steps(geo),
        spots,
    };
    let mut out: (Vec<Camp>, Vec<Piece>) = (Vec::new(), Vec::new());
    let in_chunk = |at: (i32, i32), cx: i32, cy: i32| at.0 / CHUNK_W == cx && at.1 / CHUNK_H == cy;
    // Two rounds: what each chunk is asked for, then once more for any
    // chunk still bare, with more tries.
    for (round, tries) in [(0, 160u32), (1, 400)] {
        for cy in 0..CHUNKS_Y {
            for cx in 0..CHUNKS_X {
                let home = biome::chunk(cx, cy);
                let (x0, y0) = (cx * CHUNK_W, cy * CHUNK_H);
                // A chunk with a set piece already has its own place.
                if set_pieces().any(|(_, (ox, oy))| in_chunk((ox, oy), cx, cy)) {
                    continue;
                }
                let have = |out: &(Vec<Camp>, Vec<Piece>), class: Class| out.0.iter().filter(|c| c.class == class && in_chunk(c.at, cx, cy)).count();
                let (want_wild, want_way) = quota(home);
                let wants = if round == 0 {
                    [(Class::Wild, want_wild), (Class::Road, want_way)]
                } else if have(&out, Class::Wild) + have(&out, Class::Road) == 0 {
                    // Bare: any one cluster will do.
                    [(Class::Wild, 1), (Class::Road, 1)]
                } else {
                    continue;
                };
                for (class, want) in wants {
                    for k in 0..tries {
                        if have(&out, class) >= want || (round == 1 && !out.0.iter().all(|c| !in_chunk(c.at, cx, cy))) {
                            break;
                        }
                        let (rx, ry) = (hash(cx * 97 + k as i32, cy, 811), hash(cx, cy * 97 + k as i32, 812));
                        let at = (x0 + 3 + (rx % (CHUNK_W - 6) as u32) as i32, y0 + 3 + (ry % (CHUNK_H - 6) as u32) as i32);
                        planner.place(&mut out, home, at, class, if round == 0 { 6 } else { 8 });
                    }
                }
            }
        }
    }
    // Last, the gaps: wherever a screen's worth of walkable country round a
    // spot holds nothing to walk to (no cluster, no place, no fingerpost),
    // a cluster goes in near it, anywhere off the roads.
    let mut fixed: Vec<(i32, i32)> = geo.sites.iter().map(|s| s.stand).collect();
    fixed.extend(set_pieces().map(|(_, (ox, oy))| (ox + SCREEN_W / 2, oy + SCREEN_H / 2)));
    fixed.extend(super::castles::footprints().map(|(x, y, w, h)| (x + w / 2, y + h / 2)));
    fixed.extend(super::signs::posts().iter().map(|p| p.at));
    for wy in (8..MAP_H - 8).step_by(4) {
        for wx in (8..MAP_W - 8).step_by(4) {
            if geo.room.tile(wx, wy) != Tile::Floor {
                continue;
            }
            let near = |&(x, y): &(i32, i32)| (x - wx).abs() <= GAP.0 && (y - wy).abs() <= GAP.1;
            if fixed.iter().any(near) || out.0.iter().any(|c| near(&c.at)) || out.1.iter().any(|p| near(&p.at)) {
                continue;
            }
            let home = biome::at(wx, wy);
            'tries: for k in 0..160 {
                let at = (wx - 5 + (hash(wx, wy + k, 815) % 11) as i32, wy - 3 + (hash(wx + k, wy, 816) % 7) as i32);
                for class in [Class::Wild, Class::Road] {
                    if planner.place(&mut out, home, at, class, 255) {
                        break 'tries;
                    }
                }
            }
        }
    }
    let (camps, mut pieces) = out;
    let trails = trails(&planner, &camps);
    pieces.sort_by_key(|p| (p.at.1, p.at.0));
    let mut bays = vec![0u16; (MAP_W * MAP_H) as usize];
    for (i, c) in camps.iter().enumerate().filter(|(_, c)| c.class == Class::Wild) {
        for dy in -1..=1 {
            for dx in -1..=1 {
                bays[idx(c.at.0 + dx, c.at.1 + dy)] = i as u16 + 1;
            }
        }
    }
    Dressing { pieces, camps, trails, bays }
}

/// A footpath from each wild camp to the nearest road: one tile wide,
/// wandering over open ground, joining one already laid when it meets it.
fn trails(p: &Planner<'_>, camps: &[Camp]) -> Vec<bool> {
    let mut laid = vec![false; (MAP_W * MAP_H) as usize];
    // Where a trail may not run: a thing's tile, another camp's bay, a lot
    // and its door, a castle, the College and the gate (a tile wider than
    // where a thing may not stand, since the road's verge is crossed).
    let mut shut = vec![false; (MAP_W * MAP_H) as usize];
    let mut close = |x0: i32, y0: i32, x1: i32, y1: i32| {
        for y in y0.max(0)..y1.min(MAP_H) {
            for x in x0.max(0)..x1.min(MAP_W) {
                shut[idx(x, y)] = true;
            }
        }
    };
    for site in &p.geo.sites {
        close(site.stand.0 - 2, site.stand.1 - 2, site.stand.0 + 3, site.stand.1 + 3);
        if let Some(l) = site.lot {
            close(l.tx - 1, l.ty - 1, l.tx + l.tw + 1, l.ty + l.th + 2);
        }
    }
    for (tx, ty, tw, th) in super::castles::footprints() {
        close(tx - 1, ty - 1, tx + tw + 1, ty + th + 3);
    }
    for l in [super::college::lot(), super::wishes::delve_gate()] {
        close(l.tx - 1, l.ty - 1, l.tx + l.tw + 1, l.ty + l.th + 2);
    }
    for post in super::signs::posts() {
        close(post.at.0 - 1, post.at.1 - 1, post.at.0 + 2, post.at.1 + 2);
    }
    for (_, (x, y)) in kingdom::regions() {
        close(x - 9, y - 1, x + 10, y + 3);
    }
    for (ox, oy) in set_pieces().map(|(_, o)| o) {
        close(ox - 1, oy - 1, ox + SCREEN_W + 1, oy + SCREEN_H + 1);
    }
    let blocked = |p: &Planner<'_>, own: (i32, i32), x: i32, y: i32| {
        if !inside(x, y) || shut[idx(x, y)] || p.geo.room.tile(x, y) != Tile::Floor {
            return true;
        }
        // A thing's tile or another camp's bay, but not the camp's own.
        p.claimed[idx(x, y)] && chebyshev(own, (x, y)) > 1
    };
    for camp in camps.iter().filter(|c| c.class == Class::Wild) {
        let start = camp.at;
        let mut best = vec![u32::MAX; (MAP_W * MAP_H) as usize];
        let mut parent = vec![(0i32, 0i32); (MAP_W * MAP_H) as usize];
        let mut heap = BinaryHeap::from([Reverse((0u32, start))]);
        best[idx(start.0, start.1)] = 0;
        let mut end = None;
        while let Some(Reverse((cost, at))) = heap.pop() {
            if cost > best[idx(at.0, at.1)] {
                continue;
            }
            if cost > TRAIL_MAX * 12 {
                break;
            }
            // Arrived: beside a road, or on a trail already laid.
            let by_road = at != start
                && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| kingdom::is_road(p.geo, (at.0 + dx, at.1 + dy)));
            if by_road || (laid[idx(at.0, at.1)] && chebyshev(at, start) > 2) {
                end = Some(at);
                break;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (at.0 + dx, at.1 + dy);
                if blocked(p, start, n.0, n.1) || kingdom::is_road(p.geo, n) {
                    continue;
                }
                let ground = p.realm.at(n.0, n.1);
                if !matches!(ground, b'.' | b'a' | b's') {
                    continue;
                }
                // Wandering: each tile has its own small price, trodden
                // ground is cheap.
                let step = if laid[idx(n.0, n.1)] { 3 } else { 10 + hash(n.0, n.1, 820) % 6 };
                let c = cost + step;
                if c < best[idx(n.0, n.1)] {
                    best[idx(n.0, n.1)] = c;
                    parent[idx(n.0, n.1)] = at;
                    heap.push(Reverse((c, n)));
                }
            }
        }
        if let Some(mut at) = end {
            loop {
                laid[idx(at.0, at.1)] = true;
                if at == start {
                    break;
                }
                at = parent[idx(at.0, at.1)];
            }
        }
    }
    laid
}

#[cfg(test)]
#[path = "../../tests/world/dressing__tests.rs"]
mod tests;
