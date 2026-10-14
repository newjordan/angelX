//! Flooded abbey circuit. The dry causeways remain open before any building.
//! The sluice work is a finite party intent; the realm pays and builds once.
//!
//! The errand: Brother Bucket's bell is under the flood. The sluice has
//! three valves (Fish, Boot, Duck) on springs: each one turned stays open
//! six seconds, then snaps shut. The water only drains with all three open
//! at once, so a lone knight sprints and a party splits up. Then whoever
//! fishes the bell out of the abbey wears the crown.
use super::countryside::Country;
use super::{BTreeMap, HZ, Input, Room, RoomKind, Run, Spoil, Spoils, TILE_UNITS, Tile};

/// How long a turned valve stays open before its spring shuts it.
pub const VALVE_OPEN: u32 = 6 * HZ;
pub const VALVES: [&str; 3] = ["FISH", "BOOT", "DUCK"];

pub const PLACES: [RoomKind; 3] =
    [RoomKind::Millpond, RoomKind::Sluice, RoomKind::SunkenAbbey];
pub const VISIT_MARKS: [&str; 3] = ["country:wet:0", "country:wet:1", "country:wet:2"];
pub const QUEST_MARKS: [&str; 6] = [
    "country:sluice:0",
    "country:sluice:1",
    "country:sluice:2",
    "country:sluice:3",
    "country:sluice:4",
    "country:sluice:5",
];
pub const ERRAND: [(RoomKind, (f32, f32)); 6] = [
    (RoomKind::Millpond, (18.5, 9.5)),
    (RoomKind::Sluice, (5.0, 5.0)),
    (RoomKind::Sluice, (12.0, 9.5)),
    (RoomKind::Sluice, (19.0, 5.0)),
    (RoomKind::SunkenAbbey, (5.5, 9.0)),
    (RoomKind::Millpond, (18.5, 9.5)),
];
pub const BOAT: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const ORCHARD_BOAT: (i32, i32, i32, i32) = (19, 8, 2, 1);

pub fn stage(country: &Country, marks: &BTreeMap<String, u32>) -> u8 {
    let mut s = country.sluice.min(6);
    while s < 6 && marks.contains_key(QUEST_MARKS[usize::from(s)]) {
        s += 1;
    }
    s
}
pub fn shape(room: &mut Room) {
    let pools: &[(usize, usize, usize, usize)] = match room.kind {
        RoomKind::Millpond => &[(5, 4, 6, 5), (13, 4, 4, 5)],
        RoomKind::Sluice => &[(3, 2, 6, 2), (15, 2, 6, 2), (3, 11, 6, 2), (15, 11, 6, 2)],
        RoomKind::SunkenAbbey => &[(3, 2, 6, 3), (3, 11, 6, 2), (15, 11, 6, 2)],
        _ => &[],
    };
    for &(c, r, w, h) in pools {
        for y in r..r + h {
            for x in c..c + w {
                room.set(x, y, Tile::Hazard);
            }
        }
    }
    let stair = match room.kind {
        RoomKind::SunkenAbbey => Some(BOAT),
        RoomKind::Orchard => Some(ORCHARD_BOAT),
        _ => None,
    };
    if let Some((c, r, w, h)) = stair {
        for y in r..r + h {
            for x in c..c + w {
                room.set(x as usize, y as usize, Tile::Stairs);
            }
        }
    }
}
impl Run {
    pub(super) fn tick_wetland(&mut self, inputs: &BTreeMap<u32, Input>) {
        if !self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && h.privy == 0)
        {
            return;
        }
        let kind = self.room().kind;
        if let Some(i) = PLACES.iter().position(|&k| k == kind) {
            if self.home.countryside.wet_visited & (1 << i) == 0 {
                self.marks.insert(VISIT_MARKS[i].into(), 1);
            }
        }
        let s = stage(&self.home.countryside, &self.marks);
        if (1..4).contains(&s) && kind == RoomKind::Sluice {
            self.tick_valves(inputs, s);
            return;
        }
        self.valves = None;
        if s == 6 || ERRAND[usize::from(s)].0 != kind {
            return;
        }
        let at = ERRAND[usize::from(s)].1;
        let actor = self.players.iter().find_map(|(&id, h)| {
            (h.hp > 0
                && !h.stone
                && h.privy == 0
                && inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
                && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.4)
                .then_some(id)
        });
        let Some(actor) = actor else {
            return;
        };
        self.marks.insert(QUEST_MARKS[usize::from(s)].into(), 1);
        self.sounds.push("spoil_pickup");
        if s == 4 && self.party_len() > 1 {
            for (&id, hero) in self.players.iter_mut() {
                hero.laurel = id == actor;
            }
        }
        let name = self.players.get(&actor).map_or_else(String::new, |h| h.name.to_uppercase());
        self.found = Some((self.tick, actor, match s {
            0 => "Bell lost! Open all three valves at once.".to_string(),
            4 => format!("{name} FISHED OUT THE BELL!"),
            _ => "Bell delivered! It only rings at lunch.".to_string(),
        }));
    }

    /// The sluice: valves turned open, springs shutting them, and the
    /// drain when all three stand open together.
    fn tick_valves(&mut self, inputs: &BTreeMap<u32, Input>, s: u8) {
        // An older save may have turned some for good.
        let already = usize::from(s - 1).min(3);
        let mut open = self.valves.take().unwrap_or([0; 3]);
        for (i, left) in open.iter_mut().enumerate().skip(already) {
            if *left == 1 {
                self.sounds.push("clang");
                self.found = Some((self.tick, 0, format!("SPROING! {} SHUT.", VALVES[i])));
            }
            *left = left.saturating_sub(1);
            let at = ERRAND[1 + i].1;
            let turner = self.players.iter().find_map(|(&id, h)| {
                (h.hp > 0
                    && !h.stone
                    && h.privy == 0
                    && inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
                    && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.4 * TILE_UNITS)
                    .then_some(id)
            });
            if let Some(id) = turner {
                if *left == 0 {
                    self.sounds.push("wheel_spin");
                    self.found = Some((self.tick, id, [
                        "FISH OPEN. THE FISH COMPLAINS.",
                        "BOOT OPEN. ROYAL SOCKS SAFE.",
                        "DUCK OPEN. QUACK.",
                    ][i].into()));
                }
                *left = VALVE_OPEN;
            }
        }
        if open[already..].iter().all(|&left| left > 0) {
            for mark in &QUEST_MARKS[usize::from(s)..4] {
                self.marks.insert((*mark).into(), 1);
            }
            self.sounds.push("gas_release");
            self.found = Some((self.tick, 0, "THE SLUICE DRAINS! FETCH THE BELL.".into()));
            return;
        }
        self.valves = Some(open);
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
            country.wet_visited |= 1 << i;
        }
    }
    country.sluice = stage(country, marks);
    if country.sluice == 6 && !country.sluice_paid {
        country.sluice_paid = true;
        treasury.add(Spoil::Gold, 65);
        treasury.add(Spoil::Ore, 2);
        news.push("Brother Bucket's Bell: 65 gold, 2 ore.".into());
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_valves__tests.rs"]
mod tests;
