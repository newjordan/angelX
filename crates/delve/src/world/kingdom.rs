//! One geography for the walking surface and its overview.
use super::doors::{self, Door};
use super::map::{self, MAP_H, MAP_W, Place, Realm, STRUCTURES, TILE, place_tile};
use crate::realm::Wish;
use super::college;
use super::houses;
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
/// A region of the kingdom: the countryside places that belong together,
/// the ground they are built within, and where its name is lettered.
pub struct Region {
    pub name: &'static str,
    /// Tiles `(x, y, w, h)` its places are built within.
    area: (i32, i32, i32, i32),
    /// The tile its name is centred on: a row cleared for it, or water.
    label: (i32, i32),
    /// Its countryside places, each with the tile it is built nearest.
    pub places: &'static [(RoomKind, (i32, i32))],
}

impl Region {
    /// Whether tile `(x, y)` lies in the region's ground.
    pub fn holds(&self, x: i32, y: i32) -> bool {
        let (ax, ay, aw, ah) = self.area;
        (ax..ax + aw).contains(&x) && (ay..ay + ah).contains(&y)
    }
}

/// The kingdom's regions, north to south. The authored places (the Keep,
/// the Mines, the Village, the Colosseum...) stand where the realm puts
/// them; every countryside place is built in its own region, so a region
/// reads as one cluster with its name in the open ground among it.
pub const REGIONS: [Region; 12] = [
    Region {
        name: "The Woods",
        area: (9, 0, 23, 22),
        label: (19, 21),
        places: &[
            (RoomKind::HedgeMaze, (11, 11)),
            (RoomKind::Bothy, (17, 11)),
            (RoomKind::Timberyard, (23, 11)),
            (RoomKind::Orchard, (14, 16)),
            (RoomKind::Ruins, (21, 16)),
        ],
    },
    Region {
        name: "The Mountain",
        area: (32, 0, 32, 22),
        label: (43, 8),
        // The griffins' eyrie on its slopes, for a practice flight.
        places: &[(RoomKind::Eyrie, (47, 13))],
    },
    Region {
        name: "The Wastes",
        area: (64, 0, 16, 22),
        label: (71, 9),
        places: &[],
    },
    Region {
        name: "The High Roads",
        area: (76, 11, 18, 11),
        label: (85, 21),
        places: &[
            (RoomKind::Crag, (78, 11)),
            (RoomKind::Aviary, (84, 11)),
            (RoomKind::Belfry, (90, 11)),
            (RoomKind::Aqueduct, (78, 16)),
            (RoomKind::Priory, (84, 16)),
        ],
    },
    Region {
        name: "The Coast",
        area: (94, 11, 16, 12),
        // On the sea.
        label: (104, 11),
        places: &[
            (RoomKind::Lighthouse, (104, 14)),
            (RoomKind::Cliffs, (98, 14)),
            (RoomKind::Saltworks, (98, 19)),
            (RoomKind::Strand, (104, 19)),
        ],
    },
    Region {
        name: "Castle Town",
        area: (32, 22, 16, 22),
        label: (35, 32),
        places: &[(RoomKind::Bathhouse, (42, 38))],
    },
    Region {
        name: "The Wetland",
        area: (0, 33, 32, 16),
        // On the mere.
        label: (12, 37),
        places: &[
            (RoomKind::Millpond, (21, 34)),
            (RoomKind::Sluice, (27, 34)),
            (RoomKind::SunkenAbbey, (21, 39)),
            (RoomKind::Riverbank, (27, 39)),
            (RoomKind::Causeway, (9, 44)),
        ],
    },
    Region {
        name: "The Fair",
        area: (8, 55, 31, 10),
        label: (28, 61),
        places: &[
            (RoomKind::Fairground, (22, 56)),
            (RoomKind::Mummers, (28, 56)),
            (RoomKind::Laundry, (34, 56)),
        ],
    },
    Region {
        name: "Wind Country",
        area: (46, 55, 50, 10),
        label: (64, 61),
        places: &[
            (RoomKind::GooseGreen, (48, 56)),
            (RoomKind::Windmill, (54, 56)),
            (RoomKind::Pasture, (60, 56)),
            (RoomKind::Downs, (66, 56)),
            (RoomKind::Kitewright, (72, 56)),
            (RoomKind::SignalHill, (80, 57)),
        ],
    },
    Region {
        name: "Longbow Country",
        area: (70, 40, 24, 14),
        label: (82, 51),
        places: &[
            (RoomKind::Scarecrow, (73, 42)),
            (RoomKind::LongbowGreen, (80, 42)),
            (RoomKind::Fletchery, (87, 42)),
        ],
    },
    Region {
        name: "The Tilt-Yards",
        area: (78, 66, 18, 22),
        label: (86, 77),
        places: &[
            (RoomKind::Hippodrome, (79, 67)),
            (RoomKind::Ranch, (85, 67)),
            (RoomKind::TollBridge, (91, 67)),
            (RoomKind::Caravan, (79, 72)),
            (RoomKind::Wheelwright, (85, 72)),
        ],
    },
    Region {
        name: "The March",
        area: (0, 88, 112, 33),
        label: (57, 90),
        places: &[],
    },
];

/// Open ground in the town core, between the Keep and the Village and on
/// the north road, kept clear for the King's court and its land office.
pub const OPEN_PLOT: Lot = Lot {
    tx: 33,
    ty: 35,
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
    taken.extend(houses::footprints().map(|(tx, ty, tw, th)| Lot { tx, ty, tw, th }));
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
            let (ax, ay, aw, ah) = region.area;
            let free = |x: i32, y: i32, taken: &[Lot]| {
                !taken.iter().any(|l| {
                    x < l.tx + l.tw + 2 && x + 4 + 2 > l.tx && y < l.ty + l.th + 2 && y + 3 + 2 > l.ty
                }) && REGIONS.iter().map(name_strip).all(|(x0, x1, sy)| {
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
            let nearest = |(x0, y0, x1, y1): (i32, i32, i32, i32)| {
                (y0.max(3)..y1.min(MAP_H - 4))
                    .flat_map(|y| (x0.max(3)..x1.min(MAP_W - 5)).map(move |x| (x, y)))
                    .filter(|&(x, y)| free(x, y, &taken))
                    .min_by_key(|&(x, y)| ((x - goal.0).pow(2) + (y - goal.1).pow(2), y, x))
            };
            let best = nearest((ax, ay, ax + aw, ay + ah)).or_else(|| nearest((0, 0, MAP_W, MAP_H)));
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
                roads[idx(at)] = true;
                tiles[idx(at)] = Tile::Floor;
                reached[idx(at)] = true;
                at = parents[idx(at)].unwrap();
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
    for at in named {
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
            .chain(houses::footprints())
            .chain({
                let l = college::lot();
                [(l.tx, l.ty, l.tw, l.th)]
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
