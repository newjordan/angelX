//! A dry coastal walking circuit, a timed beach errand and a useful harbour.
//! The tide changes interaction availability, never collision under a knight.
//!
//! The errand is a heist: a crab has made off with the beacon's lamp oil.
//! On the Strand every knight gives chase at once. The crab scuttles
//! sideways, away from whoever is nearest, and at high tide it digs into the
//! sand where nobody can grab it. Whoever corners it gets the oil (and the
//! crown, if friends were chasing too).

use super::{
    BTreeMap, DT, HZ, Input, Room, RoomKind, Run, Spoil, Spoils, TILE_UNITS, Tile, countryside::Country,
};
use serde::{Deserialize, Serialize};

pub const PLACES: [RoomKind; 3] =
    [RoomKind::Strand, RoomKind::Saltworks, RoomKind::Lighthouse];
pub const VISIT_MARKS: [&str; 3] = ["country:coast:0", "country:coast:1", "country:coast:2"];
pub const QUEST_MARKS: [&str; 3] = [
    "country:harbour:0",
    "country:harbour:1",
    "country:harbour:2",
];
pub const ERRAND: [(RoomKind, (f32, f32)); 3] = [
    (RoomKind::Lighthouse, (18.5, 8.5)),
    (RoomKind::Strand, (19.0, 9.0)),
    (RoomKind::Lighthouse, (18.5, 8.5)),
];
/// Eight dry seconds followed by four wet seconds. No real-world clock, no RNG.
pub fn low_tide(tick: u64) -> bool {
    tick % u64::from(12 * HZ) < u64::from(8 * HZ)
}
pub fn tide_left(tick: u64) -> u32 {
    let phase = (tick % u64::from(12 * HZ)) as u32;
    if phase < 8 * HZ {
        8 * HZ - phase
    } else {
        12 * HZ - phase
    }
}
/// The crab's pace (a knight walks at 13), and how near counts as caught.
pub const CRAB_PACE: f32 = 10.0;
pub const GRAB: f32 = 1.4;
/// The open sand it keeps to, between the pools (tiles, inclusive), so it
/// never hides off in a corner under a knight's tray.
const SAND: ((i32, i32), (i32, i32)) = ((2, 21), (4, 10));

/// The thief, on the Strand, in units.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Crab {
    pub x: f32,
    pub y: f32,
    /// Dug into the sand for the high tide.
    pub dug: bool,
}

impl Crab {
    pub fn valid(&self) -> bool {
        (0.0..200.0).contains(&self.x) && (0.0..200.0).contains(&self.y)
    }
}

pub fn stage(country: &Country, marks: &BTreeMap<String, u32>) -> u8 {
    let mut s = country.harbour.min(3);
    while s < 3 && marks.contains_key(QUEST_MARKS[usize::from(s)]) {
        s += 1;
    }
    s
}

pub fn shape(room: &mut Room) {
    let rects: &[((usize, usize, usize, usize), Tile)] = match room.kind {
        RoomKind::Strand => &[
            ((2, 1, 7, 3), Tile::Hazard),
            ((15, 1, 7, 3), Tile::Hazard),
            ((2, 11, 7, 2), Tile::Hazard),
            ((15, 11, 7, 2), Tile::Hazard),
        ],
        RoomKind::Saltworks => &[
            ((3, 2, 5, 2), Tile::Hazard),
            ((16, 2, 5, 2), Tile::Hazard),
            ((19, 10, 2, 1), Tile::Stairs),
        ],
        RoomKind::Lighthouse => &[
            ((4, 2, 5, 3), Tile::Block),
            ((2, 11, 7, 2), Tile::Hazard),
            ((15, 11, 7, 2), Tile::Hazard),
        ],
        _ => &[],
    };
    for &((c, r, w, h), tile) in rects {
        for y in r..r + h {
            for x in c..c + w {
                room.set(x, y, tile);
            }
        }
    }
}

impl Run {
    pub(super) fn tick_coastal(&mut self, inputs: &BTreeMap<u32, Input>) {
        if !self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && h.privy == 0)
        {
            return;
        }
        let kind = self.room().kind;
        if let Some(i) = PLACES.iter().position(|&k| k == kind) {
            if self.home.countryside.coast_visited & (1 << i) == 0 {
                self.marks.insert(VISIT_MARKS[i].into(), 1);
            }
        }
        let s = stage(&self.home.countryside, &self.marks);
        if s == 1 && kind == RoomKind::Strand {
            self.tick_crab();
            return;
        }
        self.crab = None;
        if s == 3 || self.pastimes.round.is_some() {
            return;
        }
        let (room, at) = ERRAND[usize::from(s)];
        if kind != room
            || !self.players.iter().any(|(id, h)| {
                h.hp > 0
                    && !h.stone
                    && h.privy == 0
                    && inputs.get(id).is_some_and(|i| i.valid() && i.interact)
                    && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.8
            })
        {
            return;
        }
        self.marks.insert(QUEST_MARKS[usize::from(s)].into(), 1);
        self.sounds.push("spoil_pickup");
        self.found = Some((self.tick, 0, match s {
            0 => "A crab stole the lamp oil! Catch it on the Strand.",
            _ => "Beacon lit! Ships may now argue safely.",
        }.into()));
    }

    /// The chase: the crab runs sideways from the nearest knight, digs in at
    /// high tide, and gives up the oil to whoever reaches it.
    fn tick_crab(&mut self) {
        let start = (ERRAND[1].1.0 * TILE_UNITS, ERRAND[1].1.1 * TILE_UNITS);
        let mut crab = self.crab.take().unwrap_or(Crab { x: start.0, y: start.1, dug: false });
        crab.dug = !low_tide(self.tick);
        let chasers: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let nearest = chasers
            .iter()
            .min_by(|a, b| (a.1 - crab.x).hypot(a.2 - crab.y).total_cmp(&(b.1 - crab.x).hypot(b.2 - crab.y)));
        if !crab.dug {
            if let Some(&(id, kx, ky)) = nearest {
                if (kx - crab.x).hypot(ky - crab.y) < GRAB {
                    self.crab_caught(id, chasers.len() > 1);
                    return;
                }
                // Away, and sideways: crabs mostly go left and right.
                let (ax, ay) = (crab.x - kx, (crab.y - ky) * 0.4);
                let len = ax.hypot(ay).max(0.01);
                let step = CRAB_PACE * DT;
                let (dx, dy) = (ax / len * step, ay / len * step);
                let sand = |x: f32, y: f32| {
                    let (c, r) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
                    (SAND.0.0..=SAND.0.1).contains(&c)
                        && (SAND.1.0..=SAND.1.1).contains(&r)
                        && self.room().tile(c, r) == Tile::Floor
                };
                // Blocked one way, it slides along the other.
                if sand(crab.x + dx, crab.y + dy) {
                    (crab.x, crab.y) = (crab.x + dx, crab.y + dy);
                } else if sand(crab.x + dx.signum() * step, crab.y) && dx.abs() > 0.05 {
                    crab.x += dx.signum() * step;
                } else if sand(crab.x, crab.y + dy.signum() * step) {
                    crab.y += if dy.abs() > 0.01 { dy.signum() } else { 1.0 } * step;
                }
            }
        }
        self.crab = Some(crab);
    }

    fn crab_caught(&mut self, id: u32, rivals: bool) {
        self.crab = None;
        self.marks.insert(QUEST_MARKS[1].into(), 1);
        if rivals {
            for (&k, hero) in self.players.iter_mut() {
                hero.laurel = k == id;
            }
        }
        let name = self.players.get(&id).map_or_else(String::new, |h| h.name.to_uppercase());
        self.sounds.push("spoil_pickup");
        self.sounds.push("hero_hurt");
        self.found = Some((self.tick, 0, format!("{name} GOT THE OIL! THE CRAB KEPT A FINGER.")));
    }
}

pub fn settle(
    country: &mut Country,
    treasury: &mut Spoils,
    marks: &BTreeMap<String, u32>,
    news: &mut Vec<String>,
) {
    for (i, mark) in VISIT_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) {
            country.coast_visited |= 1 << i;
        }
    }
    country.harbour = stage(country, marks);
    if country.harbour == 3 && !country.harbour_paid {
        country.harbour_paid = true;
        treasury.add(Spoil::Gold, 65);
        treasury.add(Spoil::Bone, 3);
        news.push("The Bickering Beacon: 65 gold, 3 bone.".into());
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_crab__tests.rs"]
mod tests;
