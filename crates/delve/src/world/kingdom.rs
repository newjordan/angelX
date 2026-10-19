//! One geography for the walking surface and its overview.
use super::doors::{self, Door};
use super::biome::{self, Biome};
use super::map::{self, MAP_H, MAP_W, Place, Realm, STRUCTURES, TILE, place_tile};
use crate::realm::Wish;
use super::college;
use super::wishes::{self, Lot};
use crate::rules::{Room, RoomKind, Tile, countryside};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

pub struct Site {
    pub name: String,
    pub stand: (i32, i32),
    pub room: RoomKind,
    pub lot: Option<Lot>,
}
pub struct Geography {
    /// Where each lot's building tops out, worked out once (by the
    /// drawing, `render::overworld::kingdom::roof`).
    pub roofs: OnceLock<Vec<Option<i32>>>,
    pub sites: Vec<Site>,
    pub roads: Vec<bool>,
    pub cleared_mask: Vec<bool>,
    pub room: Room,
}

fn destination(door: Door) -> RoomKind {
    match door {
        Door::Delve => RoomKind::Gate,
        Door::College => RoomKind::CollegeGrounds,
        Door::Place(p) => match p {
            Place::Mines => RoomKind::MineHead,
            Place::Lists => RoomKind::Lists,
            Place::Tournament => RoomKind::Stables,
            Place::DarkForest => RoomKind::Forest,
            Place::Swamp => RoomKind::Marsh,
            Place::Fields => RoomKind::Meadow,
            Place::Village => RoomKind::Village,
            Place::Rookery => RoomKind::Rookery,
            Place::Observatory => RoomKind::Observatory,
            // The Smithy's door opens on the smith's own forge, and the
            // Artisan Quarter's on the market square behind it.
            Place::Smithy => RoomKind::Smithy,
            Place::ArtisanQuarter => RoomKind::Market,
            // The Colosseum's door opens on its entrance hall, and its
            // gates on the arena games (`rules::colosseum`).
            Place::Colosseum => RoomKind::ColosseumHall,
            _ => RoomKind::RealmHall,
        },
        Door::Castle(_) => RoomKind::RealmHall,
    }
}
pub fn outdoor(kind: RoomKind) -> bool {
    kind.in_world()
        && kind != RoomKind::KingsHall
        && (!crate::rules::college::is_room(kind)
            || kind == RoomKind::CollegeGrounds)
        && !crate::rules::colosseum::is_arena(kind)
}
fn short(name: &str) -> String {
    let words = name.replace(['/', '-'], " ");
    words
        .split_whitespace()
        .filter(|s| !matches!(s.to_ascii_lowercase().as_str(), "the" | "of" | "a"))
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
}
/// A region of the kingdom: one biome (`super::biome`), the countryside
/// places built in it, and where its name is lettered.
pub struct Region {
    pub name: &'static str,
    pub biome: Biome,
    /// The tile its name is centred on: a row of open green.
    label: (i32, i32),
    /// Its countryside places, each with the tile it is built nearest.
    pub places: &'static [(RoomKind, (i32, i32))],
}

impl Region {
    /// Whether tile `(x, y)` lies in the region's ground.
    pub fn holds(&self, x: i32, y: i32) -> bool {
        biome::at(x, y) == self.biome
    }

    /// The tiles `(x0, y0, x1, y1)` its ground spans, end exclusive.
    fn bounds(&self) -> (i32, i32, i32, i32) {
        self.biome.chunks().fold((MAP_W, MAP_H, 0, 0), |(x0, y0, x1, y1), (cx, cy)| {
            (
                x0.min(cx * map::CHUNK_W),
                y0.min(cy * map::CHUNK_H),
                x1.max((cx + 1) * map::CHUNK_W),
                y1.max((cy + 1) * map::CHUNK_H),
            )
        })
    }
}

const fn region(biome: Biome, places: &'static [(RoomKind, (i32, i32))]) -> Region {
    Region {
        name: match biome {
            Biome::CastleTown => "Castle Town",
            Biome::Greenwood => "Greenwood",
            Biome::Highlands => "The Highlands",
            Biome::Wastes => "The Wastes",
            Biome::Coast => "The Coast",
            Biome::Fen => "The Fen",
            Biome::TiltYards => "The Tilt-Yards",
            Biome::Fairground => "The Fairground",
            Biome::WindCountry => "Wind Country",
            Biome::Longbow => "Longbow Country",
            Biome::March => "The March",
        },
        biome,
        label: match biome {
            Biome::CastleTown => (60, 72),
            Biome::Greenwood => (60, 12),
            Biome::Highlands => (112, 42),
            Biome::Wastes => (180, 38),
            Biome::Coast => (221, 100),
            Biome::Fen => (16, 100),
            Biome::TiltYards => (176, 77),
            Biome::Fairground => (40, 134),
            Biome::WindCountry => (150, 116),
            Biome::Longbow => (202, 136),
            // On the open green between the castles where the South Road
            // comes through to the far lane.
            Biome::March => (87, 165),
        },
        places,
    }
}

/// The kingdom's regions, one to a biome. The authored places (the Keep,
/// the Mines, the Village, the Colosseum...) stand in their set pieces;
/// every countryside place is built in its own biome, on the free lot
/// nearest its spot, so a biome reads as one country with its name on an
/// open green among it (`docs/KINGDOM-OPEN.md`).
pub const REGIONS: [Region; 11] = [
    region(Biome::CastleTown, &[(RoomKind::Bathhouse, (113, 70))]),
    region(
        Biome::Greenwood,
        &[
            (RoomKind::HedgeMaze, (12, 22)),
            (RoomKind::Bothy, (36, 8)),
            (RoomKind::Timberyard, (60, 28)),
            (RoomKind::Orchard, (36, 40)),
            (RoomKind::Ruins, (12, 38)),
        ],
    ),
    region(
        Biome::Highlands,
        // The griffins' eyrie on the peak above the Mines.
        &[
            (RoomKind::Eyrie, (108, 8)),
            (RoomKind::Crag, (84, 8)),
            (RoomKind::Aviary, (132, 8)),
            (RoomKind::Belfry, (156, 8)),
            (RoomKind::Aqueduct, (60, 40)),
            (RoomKind::Priory, (130, 40)),
        ],
    ),
    region(Biome::Wastes, &[]),
    region(
        Biome::Coast,
        &[
            (RoomKind::Lighthouse, (220, 8)),
            (RoomKind::Cliffs, (200, 51)),
            (RoomKind::Saltworks, (200, 88)),
            (RoomKind::Strand, (220, 118)),
        ],
    ),
    region(
        Biome::Fen,
        &[
            (RoomKind::SunkenAbbey, (14, 70)),
            (RoomKind::Millpond, (34, 70)),
            (RoomKind::Sluice, (34, 104)),
            (RoomKind::Riverbank, (58, 104)),
            (RoomKind::Causeway, (14, 118)),
        ],
    ),
    region(
        Biome::TiltYards,
        &[
            (RoomKind::TollBridge, (134, 50)),
            (RoomKind::Wheelwright, (180, 50)),
            (RoomKind::Caravan, (130, 68)),
            (RoomKind::Hippodrome, (160, 70)),
            (RoomKind::Ranch, (182, 68)),
        ],
    ),
    region(
        Biome::Fairground,
        &[
            (RoomKind::Fairground, (90, 120)),
            (RoomKind::Mummers, (58, 120)),
            (RoomKind::Laundry, (90, 102)),
        ],
    ),
    region(
        Biome::WindCountry,
        &[
            (RoomKind::GooseGreen, (108, 102)),
            (RoomKind::Windmill, (158, 104)),
            (RoomKind::Pasture, (104, 122)),
            (RoomKind::Downs, (84, 136)),
            (RoomKind::Kitewright, (108, 136)),
            (RoomKind::SignalHill, (134, 136)),
        ],
    ),
    region(
        Biome::Longbow,
        &[
            (RoomKind::Scarecrow, (182, 104)),
            (RoomKind::LongbowGreen, (204, 120)),
            (RoomKind::Fletchery, (182, 136)),
        ],
    ),
    region(Biome::March, &[]),
];

/// Open ground on Castle Town's green, between the Keep and the Village
/// beside the high street, kept clear for the King's court and its land
/// office.
pub const OPEN_PLOT: Lot = Lot {
    tx: 74,
    ty: 68,
    tw: 5,
    th: 5,
};

/// Each region's name and the tile its name is centred on.
pub fn regions() -> Vec<(&'static str, (i32, i32))> {
    REGIONS.iter().map(|r| (r.name, r.label)).collect()
}

/// The tiles a region's name is lettered across: `(x0, x1, y)`, inclusive.
fn name_strip(region: &Region) -> (i32, i32, i32) {
    let half = (crate::render::overworld::ink::text_width(&region.name.to_uppercase()) + 24) / TILE / 2 + 1;
    (region.label.0 - half, region.label.0 + half, region.label.1)
}

fn idx((x, y): (i32, i32)) -> usize {
    (y * MAP_W + x) as usize
}
fn bounds((x, y): (i32, i32)) -> bool {
    x > 0 && y > 0 && x < MAP_W - 1 && y < MAP_H - 1
}

pub fn geography(wishes: &[Wish]) -> Arc<Geography> {
    type Cache = Mutex<Option<(Vec<Wish>, Arc<Geography>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some((old, geo)) = &*cache
        && old == wishes
    {
        return geo.clone();
    }
    let geo = Arc::new(build(wishes));
    *cache = Some((wishes.to_vec(), geo.clone()));
    geo
}
fn build(wish_list: &[Wish]) -> Geography {
    let mut sites: Vec<Site> = doors::doors()
        .iter()
        .map(|&(stand, door)| Site {
            name: if door == Door::College {
                "Labyrinth College".into()
            } else {
                short(&door.name())
            },
            stand,
            room: destination(door),
            lot: None,
        })
        .collect();
    // The actual gate door, with a dry approach, replaces the old across-stream stand.
    if let Some(s) = sites.iter_mut().find(|s| s.room == RoomKind::Gate) {
        s.stand = wishes::delve_stand();
    }
    let mut taken: Vec<Lot> = map::STRUCTURES
        .iter()
        .map(|&(x, y, tw, th)| {
            let (tx, ty) = map::place_tile(x, y);
            Lot { tx, ty, tw, th }
        })
        .collect();
    taken.extend(super::castles::footprints().map(|(tx, ty, tw, th)| Lot { tx, ty, tw, th }));
    taken.extend([college::lot(), wishes::delve_gate()]);
    let wish_lots = wishes::lots(wish_list);
    taken.extend(wish_lots.iter().flatten().copied());
    let mut cleared = Vec::new();
    // The King's court, the Land Office, stands on the open plot in the
    // town core, between the Keep and the Village.
    if outdoor(RoomKind::LandOffice) && !sites.iter().any(|s| s.room == RoomKind::LandOffice) {
        let lot = Lot {
            tx: OPEN_PLOT.tx,
            ty: OPEN_PLOT.ty,
            tw: 4,
            th: 3,
        };
        cleared.push(lot);
        taken.push(lot);
        sites.push(Site {
            name: "Land Office".into(),
            stand: (lot.tx + 1, lot.ty + lot.th),
            room: RoomKind::LandOffice,
            lot: Some(lot),
        });
    } else {
        // Without the court, the open plot stays clear ground.
        taken.push(OPEN_PLOT);
    }
    // Each retained countryside pocket has a real free door in its region,
    // built on the free lot nearest its spot there.
    for region in &REGIONS {
        for &(kind, goal) in region.places {
            if !outdoor(kind) || sites.iter().any(|s| s.room == kind) {
                continue;
            }
            let (x0, y0, x1, y1) = region.bounds();
            let free = |x: i32, y: i32, taken: &[Lot]| {
                // Three tiles of ground between neighbours, a tile off every
                // set piece.
                !taken.iter().any(|l| {
                    x < l.tx + l.tw + 3 && x + 4 + 3 > l.tx && y < l.ty + l.th + 3 && y + 3 + 3 > l.ty
                }) && (y - 1..y + 5).all(|ty| (x - 1..x + 5).all(|tx| !map::in_set_piece(tx, ty)))
                    && REGIONS.iter().map(name_strip).all(|(x0, x1, sy)| {
                    // A lot and its door keep off a region's name, the row
                    // above it, and the two below (where a plaque goes).
                    x + 3 < x0 - 1 || x > x1 + 1 || y + 3 < sy - 1 || y > sy + 2
                }) && (y..y + 4).all(|ty| {
                    (x..x + 4).all(|tx| {
                        matches!(Realm::get().at(tx, ty), b'.' | b'T' | b'^' | b'd' | b'a' | b's' | b'%')
                    })
                })
            };
            // In its region; failing that (a wish took the ground), the free
            // lot nearest its spot anywhere.
            let nearest = |(x0, y0, x1, y1): (i32, i32, i32, i32), within: &dyn Fn(&(i32, i32)) -> bool| {
                (y0.max(3)..y1.min(MAP_H - 4))
                    .flat_map(|y| (x0.max(3)..x1.min(MAP_W - 5)).map(move |x| (x, y)))
                    .filter(|p| within(p))
                    .filter(|&(x, y)| free(x, y, &taken))
                    .min_by_key(|&(x, y)| ((x - goal.0).pow(2) + (y - goal.1).pow(2), y, x))
            };
            let home = |(x, y): &(i32, i32)| {
                [(*x, *y), (x + 3, *y), (*x, y + 3), (x + 3, y + 3)].iter().all(|&(cx, cy)| region.holds(cx, cy))
            };
            let best = nearest((x0, y0, x1, y1), &home).or_else(|| nearest((0, 0, MAP_W, MAP_H), &|_| true));
            let Some((x, y)) = best else {
                continue;
            };
            let lot = Lot {
                tx: x,
                ty: y,
                tw: 4,
                th: 3,
            };
            taken.push(lot);
            cleared.push(lot);
            sites.push(Site {
                name: match kind {
                    RoomKind::Ranch => "Horse Ranch".into(),
                    RoomKind::Pasture => "Flower Fields".into(),
                    RoomKind::SignalHill => "Loud Flags".into(),
                    RoomKind::Caravan => "Caravan Camp".into(),
                    RoomKind::TollBridge => "Toll Bridge".into(),
                    RoomKind::GooseGreen => "Picnic Green".into(),
                    RoomKind::LandOffice => "Land Office".into(),
                    RoomKind::LongbowGreen => "Longbow Green".into(),
                    RoomKind::Fletchery => "Fletchery".into(),
                    RoomKind::Scarecrow => "Scarecrow Field".into(),
                    RoomKind::Bathhouse => "Bathhouse".into(),
                    RoomKind::Eyrie => "Griffin Eyrie".into(),
                    _ => short(countryside::name(kind)),
                },
                stand: (lot.tx + 1, lot.ty + lot.th),
                room: kind,
                lot: Some(lot),
            });
        }
    }
    for (wish, lot) in wish_list.iter().zip(wish_lots.iter()) {
        if let Some(lot) = lot {
            sites.push(Site {
                name: short(&wish.name),
                stand: (lot.tx + lot.tw / 2, lot.ty + lot.th),
                room: RoomKind::RealmHall,
                lot: None,
            });
        }
    }
    let mut tiles: Vec<Tile> = (0..MAP_H)
        .flat_map(|y| {
            (0..MAP_W).map(move |x| {
                if step_cost(x, y).is_some() {
                    Tile::Floor
                } else {
                    Tile::Block
                }
            })
        })
        .collect();
    for lot in &cleared {
        for y in lot.ty..lot.ty + lot.th {
            for x in lot.tx..lot.tx + lot.tw {
                tiles[idx((x, y))] = Tile::Block;
            }
        }
    }
    // The rest of the open plot is cleared ground anyone can cross.
    for y in OPEN_PLOT.ty..OPEN_PLOT.ty + OPEN_PLOT.th {
        for x in OPEN_PLOT.tx..OPEN_PLOT.tx + OPEN_PLOT.tw {
            if cleared.iter().any(|l| x >= l.tx && x < l.tx + l.tw && y >= l.ty && y < l.ty + l.th) {
                continue;
            }
            tiles[idx((x, y))] = Tile::Floor;
        }
    }
    // Each region's name is lettered on cleared ground.
    let mut named = Vec::new();
    for (x0, x1, y) in REGIONS.iter().map(name_strip) {
        for x in x0..=x1 {
            if bounds((x, y))
                && matches!(Realm::get().at(x, y), b'T' | b'^' | b'd' | b'.' | b'a')
                && !taken.iter().any(|l| l.contains(x, y))
            {
                tiles[idx((x, y))] = Tile::Floor;
                named.push((x, y));
            }
        }
    }
    // Wish footprints are solid; their door row stays open.
    for lot in wish_lots.iter().flatten() {
        for y in lot.ty..lot.ty + lot.th - 1 {
            for x in lot.tx..lot.tx + lot.tw {
                tiles[idx((x, y))] = Tile::Block;
            }
        }
    }
    let mut reached = vec![false; tiles.len()];
    let start = Place::Keep.stand_world();
    let mut queue = VecDeque::from([start]);
    reached[idx(start)] = true;
    while let Some((x, y)) = queue.pop_front() {
        for next in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if bounds(next) && tiles[idx(next)] == Tile::Floor && !reached[idx(next)] {
                reached[idx(next)] = true;
                queue.push_back(next);
            }
        }
    }
    let mut roads = vec![false; tiles.len()];
    let strips: Vec<(i32, i32, i32)> = REGIONS.iter().map(name_strip).collect();
    for site in &sites {
        // Find a short approach to the connected kingdom. Rock and forest can
        // be cleared, streams bridged; structures and lava are never crossed.
        let mut parents = vec![None; tiles.len()];
        let mut queue = VecDeque::from([site.stand]);
        parents[idx(site.stand)] = Some(site.stand);
        let mut end = None;
        while let Some((x, y)) = queue.pop_front() {
            if reached[idx((x, y))] {
                end = Some((x, y));
                break;
            }
            for next in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if !bounds(next)
                    || parents[idx(next)].is_some()
                    || taken.iter().any(|l| l.contains(next.0, next.1))
                    || matches!(Realm::get().at(next.0, next.1), b'L' | b'F')
                {
                    continue;
                }
                parents[idx(next)] = Some((x, y));
                queue.push_back(next);
            }
        }
        if let Some(mut at) = end {
            while at != site.stand {
                roads[idx(at)] = true;
                tiles[idx(at)] = Tile::Floor;
                reached[idx(at)] = true;
                at = parents[idx(at)].unwrap();
            }
        }
        roads[idx(site.stand)] = true;
        tiles[idx(site.stand)] = Tile::Floor;
        // And a lane on to the nearest road, so every place hangs off the
        // road network: through open ground where it can, through woods and
        // over streams where it must, never through a building, a region's
        // name or lava.
        let mut best = vec![u32::MAX; tiles.len()];
        let mut parents = vec![None; tiles.len()];
        let mut heap = BinaryHeap::from([Reverse((0u32, site.stand))]);
        best[idx(site.stand)] = 0;
        let mut end = None;
        while let Some(Reverse((cost, at))) = heap.pop() {
            if cost > best[idx(at)] {
                continue;
            }
            let on_road = (at != site.stand && roads[idx(at)])
                || (matches!(Realm::get().at(at.0, at.1), b'=' | b':') && reached[idx(at)]);
            if on_road {
                end = Some(at);
                break;
            }
            for next in [(at.0 - 1, at.1), (at.0 + 1, at.1), (at.0, at.1 - 1), (at.0, at.1 + 1)] {
                if !bounds(next)
                    || taken.iter().any(|l| l.contains(next.0, next.1))
                    || strips.iter().any(|&(x0, x1, y)| next.1 == y && (x0..=x1).contains(&next.0))
                {
                    continue;
                }
                let step = match Realm::get().at(next.0, next.1) {
                    b'=' | b':' | b'H' => 1,
                    b'.' | b's' | b'c' | b'a' => 2,
                    b'%' => 3,
                    b'T' | b'^' | b'd' => 4,
                    b'L' | b'F' => continue,
                    _ => 6,
                };
                let cost = cost + step;
                if cost < best[idx(next)] {
                    best[idx(next)] = cost;
                    parents[idx(next)] = Some(at);
                    heap.push(Reverse((cost, next)));
                }
            }
        }
        if let Some(mut at) = end {
            while at != site.stand {
                let next = parents[idx(at)].unwrap();
                roads[idx(at)] = true;
                tiles[idx(at)] = Tile::Floor;
                reached[idx(at)] = true;
                // Lanes are two tiles wide: the tile beside each step too,
                // where nothing stands.
                let side = if next.0 == at.0 { (at.0 + 1, at.1) } else { (at.0, at.1 + 1) };
                if bounds(side)
                    && !closed_tiles()[idx(side)]
                    && !taken.iter().any(|l| l.contains(side.0, side.1))
                    && !cleared.iter().any(|l| l.contains(side.0, side.1))
                    && !wish_lots.iter().flatten().any(|l| l.contains(side.0, side.1))
                    && !strips.iter().any(|&(x0, x1, sy)| side.1 == sy && (x0..=x1).contains(&side.0))
                    && !matches!(Realm::get().at(side.0, side.1), b'L' | b'F')
                {
                    roads[idx(side)] = true;
                    tiles[idx(side)] = Tile::Floor;
                    reached[idx(side)] = true;
                }
                at = next;
            }
        }
    }
    // A verge either side of every road and lane: woods, dead trees and
    // open ground beside a road are cleared to meadow, so two knights walk
    // abreast and nobody brushes a trunk. Never a building, a lot, a
    // region's name, rock, water or lava.
    let mut verge = Vec::new();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if !roads[idx((x, y))] {
                continue;
            }
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let next = (x + dx, y + dy);
                    if !bounds(next)
                        || roads[idx(next)]
                        || closed_tiles()[idx(next)]
                        || taken.iter().any(|l| l.contains(next.0, next.1))
                        || cleared.iter().any(|l| l.contains(next.0, next.1))
                        || wish_lots.iter().flatten().any(|l| l.contains(next.0, next.1))
                        || strips.iter().any(|&(x0, x1, sy)| next.1 == sy && (x0..=x1).contains(&next.0))
                    {
                        continue;
                    }
                    if matches!(Realm::get().at(next.0, next.1), b'T' | b'd' | b'.' | b'c' | b'a' | b's') {
                        tiles[idx(next)] = Tile::Floor;
                        verge.push(next);
                    }
                }
            }
        }
    }
    // A solid outer rim bounds all four independent knights.
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if !bounds((x, y)) {
                tiles[idx((x, y))] = Tile::Wall;
            }
        }
    }
    let mut cleared_mask = vec![false; tiles.len()];
    for lot in cleared.iter().chain([&OPEN_PLOT]) {
        for y in lot.ty..lot.ty + lot.th {
            for x in lot.tx..lot.tx + lot.tw {
                cleared_mask[idx((x, y))] = true;
            }
        }
    }
    for at in named.into_iter().chain(verge) {
        cleared_mask[idx(at)] = true;
    }
    Geography {
        roofs: OnceLock::new(),
        cleared_mask,
        sites,
        roads,
        room: Room::kingdom(tiles, MAP_W as usize, MAP_H as usize),
    }
}

pub fn gate_stand(wishes: &[Wish]) -> (i32, i32) {
    geography(wishes)
        .sites
        .iter()
        .find(|s| s.room == RoomKind::Gate)
        .unwrap()
        .stand
}
/// Whether tile `(x, y)` of the walking surface is road, drawn or cut to a
/// place's door: quick going.
pub fn is_road(geo: &Geography, (x, y): (i32, i32)) -> bool {
    bounds((x, y))
        && (geo.roads[idx((x, y))] || matches!(Realm::get().at(x, y), b'=' | b':' | b'H'))
}
pub fn keep_stand() -> (i32, i32) {
    Place::Keep.stand_world()
}
/// The realm's terrain byte at a tile, for charts of the layout.
#[cfg(any(test, feature = "testing"))]
pub fn terrain(x: i32, y: i32) -> u8 {
    Realm::get().at(x, y)
}

fn closed_tiles() -> &'static [bool] {
    static CLOSED: OnceLock<Vec<bool>> = OnceLock::new();
    CLOSED.get_or_init(|| {
        let realm = Realm::get();
        let mut closed = vec![false; (MAP_W * MAP_H) as usize];
        let placed = STRUCTURES
            .into_iter()
            .map(|(ax, ay, w, h)| {
                let (x, y) = place_tile(ax, ay);
                (x, y, w, h)
            })
            .chain(super::castles::footprints())
            .chain({
                // The College and the Delve's gate stand on open ground:
                // their walls are as solid as any set piece's.
                let l = college::lot();
                let g = wishes::delve_gate();
                [(l.tx, l.ty, l.tw, l.th), (g.tx, g.ty, g.tw, g.th)]
            });
        for (x0, y0, w, h) in placed {
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    if !matches!(realm.at(x, y), b'=' | b':' | b'H') {
                        closed[(y * MAP_W + x) as usize] = true;
                    }
                }
            }
        }
        closed
    })
}

/// What a step onto `(x, y)` costs, or `None` where no one walks.
pub fn step_cost(x: i32, y: i32) -> Option<u32> {
    if x < 0 || y < 0 || x >= MAP_W || y >= MAP_H || closed_tiles()[(y * MAP_W + x) as usize] {
        return None;
    }
    if college::approach().contains(&(x, y)) {
        return Some(2);
    }
    match Realm::get().at(x, y) {
        b'=' | b':' | b'H' => Some(2),
        b'.' | b's' | b'a' => Some(5),
        b'c' => Some(7),
        b'%' => Some(9),
        _ => None,
    }
}
