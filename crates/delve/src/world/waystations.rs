//! The waystations: a halt beside the kingdom's roads with a keeper, a coach
//! post and a rally pole, a day's trot apart round the realm. The coaches
//! run the stops in a ring (the order of `STOPS`), each stop onward to the next and
//! back to the one before. Fixed for the realm: where each halt stands is a
//! table (`SPOTS`) that a test keeps equal to what the search over the roads
//! and the open ground beside them finds (`search`); nothing here reads the
//! run, so the wishes can never move a halt.

use super::wishes::Lot;
use std::sync::OnceLock;

/// A halt's ground: seven tiles across, three of building and yard and a
/// fourth of apron in front, where the coach sets a party down.
pub const W: i32 = 7;
pub const H: i32 = 4;
/// How far down the road from its authored spot the search may move a halt to
/// find open ground, in road tiles.
#[cfg(any(test, feature = "testing"))]
const REACH: i32 = 24;
/// No two halts nearer than this, in tiles.
pub const SPACING: i32 = 18;

/// One halt, as authored: what it is called, who keeps it, what the keeper
/// says in greeting, and the road tile it stands nearest.
pub struct Stop {
    pub name: &'static str,
    pub keeper: &'static str,
    pub hello: &'static str,
    pub near: (i32, i32),
}

/// The ring the coaches run, in order: each stop's coach goes onward to the
/// next and back to the one before; the last runs on to the first.
pub const STOPS: [Stop; 11] = [
    Stop {
        name: "Harbour Halt",
        keeper: "Marl Gullwick",
        hello: "Salt on the wind and mud on the boots. Sit a while, knights, the road keeps.",
        near: (31, 56),
    },
    Stop {
        name: "Gatehill Halt",
        keeper: "Old Pell",
        hello: "The Delve is down the hill. Nobody asks me how deep it goes, so I tell them anyway.",
        near: (92, 37),
    },
    Stop {
        name: "Mineside Halt",
        keeper: "Dugan Flint",
        hello: "The Mines are up the road, and the dwarf king's lamps burn all night. Mind your head on the way down.",
        near: (88, 22),
    },
    Stop {
        name: "Ashroad Halt",
        keeper: "Sooty Tam",
        hello: "Ash on the kettle, ash on the bread. The Wastes keep all they are given.",
        near: (152, 23),
    },
    Stop {
        name: "Listsend Halt",
        keeper: "Bett Farthing",
        hello: "The lances splinter from dawn to dusk. I only keep the kettle and the book of names.",
        near: (160, 55),
    },
    Stop {
        name: "Coastgate Halt",
        keeper: "Hob Seawright",
        hello: "Gulls on the thatch and fish in the rain barrel. The coast road is long, knights.",
        near: (205, 70),
    },
    Stop {
        name: "Longbow Halt",
        keeper: "Ysolt Fletcher",
        hello: "Every post on this road is a bowstave in disguise. Do not lean on them.",
        near: (188, 128),
    },
    Stop {
        name: "Homecoming Halt",
        keeper: "Gaffer Mudd",
        hello: "Wind Country, where the flags flap and the hats do not stay on.",
        near: (146, 114),
    },
    Stop {
        name: "Midway Halt",
        keeper: "Tilly Spoke",
        hello: "Half the realm to the Keep, half to the sea, and I sit in the middle with the kettle.",
        near: (80, 124),
    },
    Stop {
        name: "Marchgate Halt",
        keeper: "Sergeant Bodkin",
        hello: "The great houses stand behind me, and not one of them has asked me in for tea.",
        near: (86, 157),
    },
    Stop {
        name: "Swamp Lane Halt",
        keeper: "Reed Moorcock",
        hello: "The lane west goes down to the Swamp of Horrors. The frogs there have opinions.",
        near: (40, 82),
    },
];

/// A halt as it stands: the ground it holds, and the tiles its parts keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub stop: usize,
    /// The halt's `W` x `H` tiles.
    pub rect: Lot,
    /// Where the keeper stands, before the shelter's door.
    pub keeper: (i32, i32),
    /// The coach post, where the coach stands waiting.
    pub coach: (i32, i32),
    /// The rally pole's foot.
    pub rally: (i32, i32),
    /// The road tile it stands nearest.
    pub road: (i32, i32),
}

impl Placed {
    /// Where a party set down here stands, in tiles: the apron before the
    /// coach, centred.
    pub fn apron(&self) -> (f32, f32) {
        (self.rect.tx as f32 + 3.5, self.rect.ty as f32 + 3.4)
    }
}

/// Where each halt stands, in ring order: its ground's top-left tile and the
/// road tile it stands nearest. What `search` finds on the realm's roads;
/// `the_halts_stand_where_the_search_finds_them` keeps the two equal, so a
/// change to the map that moves a halt's ground says so.
const SPOTS: [((i32, i32), (i32, i32)); STOPS.len()] = [
    ((36, 49), (38, 53)),
    ((102, 35), (100, 36)),
    ((90, 17), (92, 23)),
    ((153, 25), (152, 25)),
    ((162, 52), (164, 56)),
    ((211, 65), (211, 69)),
    ((174, 122), (181, 123)),
    ((138, 109), (140, 113)),
    ((71, 121), (77, 125)),
    ((86, 158), (88, 156)),
    ((30, 96), (36, 101)),
];

fn placed_at(stop: usize, (tx, ty): (i32, i32), road: (i32, i32)) -> Placed {
    let rect = Lot { tx, ty, tw: W, th: H };
    Placed {
        stop,
        rect,
        keeper: (tx + 2, ty + H - 2),
        coach: (tx + 5, ty + H - 2),
        rally: (tx, ty + H - 2),
        road,
    }
}

/// Every halt, in ring order.
pub fn placed() -> &'static [Placed] {
    static PLACED: OnceLock<Vec<Placed>> = OnceLock::new();
    // No halt stands on the blank-slate plan's hillside (`super::plan`).
    PLACED.get_or_init(|| {
        if super::plan::tabula() {
            return Vec::new();
        }
        SPOTS.iter().enumerate().map(|(i, &(at, road))| placed_at(i, at, road)).collect()
    })
}

/// The stop after `i` round the ring, and the one before it.
pub fn onward(i: usize) -> usize {
    (i + 1) % STOPS.len()
}

pub fn back(i: usize) -> usize {
    (i + STOPS.len() - 1) % STOPS.len()
}

/// The search for the table: for each stop, the open ground along its road
/// nearest the authored spot, kept off every door, lot, banner and thing of
/// the realm with no wishes raised, off the regions' names and the other
/// halts.
#[cfg(any(test, feature = "testing"))]
pub fn search() -> Vec<Placed> {
    use super::kingdom::name_strips;
    use super::map::{MAP_H, MAP_W, Realm};
    use crate::rules::Tile;
    let geo = super::kingdom::geography(&[]);
    let realm = Realm::get();
    let road = |x: i32, y: i32| super::kingdom::is_road(&geo, (x, y));
    let floor = |x: i32, y: i32| x > 0 && y > 0 && x < MAP_W - 1 && y < MAP_H - 1 && geo.room.tile(x, y) == Tile::Floor;
    let lotted: Vec<Lot> = geo.sites.iter().filter_map(|s| s.lot).collect();
    let stones: Vec<(i32, i32)> = crate::rules::classes::stones(&geo).into_iter().map(|(_, p)| p).collect();
    let things: Vec<(i32, i32)> = crate::rules::trinkets::things(&geo).into_iter().map(|(_, p)| p).collect();
    let strips = name_strips();
    let mut out: Vec<Placed> = Vec::new();
    for (i, stop) in STOPS.iter().enumerate() {
        // The roads' tiles within `REACH` steps of the authored spot, by
        // how far along the road they lie.
        let Some(start) = (-6..=6)
            .flat_map(|dy| (-6..=6).map(move |dx| (stop.near.0 + dx, stop.near.1 + dy)))
            .filter(|&(x, y)| road(x, y))
            .min_by_key(|&(x, y)| (x - stop.near.0).pow(2) + (y - stop.near.1).pow(2))
        else {
            continue;
        };
        let mut along = std::collections::HashMap::from([(start, 0)]);
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some((x, y)) = queue.pop_front() {
            let d = along[&(x, y)];
            if d >= REACH {
                continue;
            }
            for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if road(next.0, next.1) && !along.contains_key(&next) {
                    along.insert(next, d + 1);
                    queue.push_back(next);
                }
            }
        }
        let mut best: Option<(i32, Lot, (i32, i32))> = None;
        // Ground with room all round first, then closer quarters, and last
        // the mire's boardwalks.
        for (margin, keep_off, door, mire) in [(2, 4, 9, false), (1, 3, 7, false), (1, 2, 6, false), (1, 2, 6, true)] {
            if best.is_some() {
                break;
            }
            for y0 in stop.near.1 - REACH..=stop.near.1 + REACH {
                for x0 in stop.near.0 - REACH..=stop.near.0 + REACH {
                    let rect = Lot { tx: x0, ty: y0, tw: W, th: H };
                    // Open meadow, sand or ash under it, or ground cleared
                    // already, and no road; the ground round it walkable.
                    let fits = (y0 - margin..y0 + H + margin).all(|y| {
                        (x0 - margin..x0 + W + margin).all(|x| {
                            floor(x, y)
                                && (!rect.contains(x, y)
                                    || (!road(x, y)
                                        && !geo.roads[(y * MAP_W + x) as usize]
                                        && (matches!(realm.at(x, y), b'.' | b's' | b'a') || (mire && realm.at(x, y) == b'%') || geo.cleared_mask[(y * MAP_W + x) as usize])))
                        })
                    });
                    let grown = Lot { tx: x0 - keep_off, ty: y0 - keep_off, tw: W + 2 * keep_off, th: H + 2 * keep_off };
                    if !fits
                        || lotted.iter().any(|l| l.overlaps(&grown))
                        || geo.sites.iter().any(|s| (s.stand.0 - x0 - 3).abs() <= door && (s.stand.1 - y0 - 2).abs() <= door - 2)
                        || stones.iter().chain(&things).any(|t| Lot { tx: x0 - 2, ty: y0 - 2, tw: W + 4, th: H + 4 }.contains(t.0, t.1))
                        || strips.iter().any(|&(sx0, sx1, sy)| x0 - 2 <= sx1 && sx0 <= x0 + W + 2 && y0 - 3 <= sy && sy <= y0 + H + 2)
                        || out.iter().any(|p| (p.rect.tx - x0).abs().max((p.rect.ty - y0).abs()) < SPACING)
                    {
                        continue;
                    }
                    // The nearest road to the keeper's door, as near as the
                    // ground allows, and the nearest along the road to the
                    // authored spot.
                    let front = (x0 + 2, y0 + H - 1);
                    let Some((r, d)) = (-5..=5)
                        .flat_map(|dy| (-5..=5).map(move |dx| (front.0 + dx, front.1 + dy)))
                        .filter_map(|t| along.get(&t).map(|&d| (t, d)))
                        .min_by_key(|&((x, y), _)| (x - front.0).pow(2) + 2 * (y - front.1).pow(2))
                    else {
                        continue;
                    };
                    let toll = (r.0 - front.0).pow(2) + 2 * (r.1 - front.1).pow(2);
                    let score = toll * 6 + d;
                    if best.as_ref().is_none_or(|b| score < b.0) {
                        best = Some((score, rect, r));
                    }
                }
            }
        }
        if let Some((_, rect, road)) = best {
            out.push(placed_at(i, (rect.tx, rect.ty), road));
        }
    }
    out
}

/// The halt whose keeper a knight at `(x, y)` (units) stands within `reach`
/// units of, nearest first.
pub fn keeper_near(x: f32, y: f32, reach: f32) -> Option<usize> {
    let tile = crate::rules::TILE_UNITS;
    placed()
        .iter()
        .map(|p| {
            let (kx, ky) = ((p.keeper.0 as f32 + 0.5) * tile, (p.keeper.1 as f32 + 0.5) * tile);
            (p.stop, (x - kx).hypot(y - ky))
        })
        .filter(|&(_, d)| d < reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[cfg(test)]
#[path = "../../tests/world/waystations__tests.rs"]
mod tests;
