//! Rescues. Now and then a floor's treasure room holds a cage, and someone
//! in it: a cook the goblins took, a monk the Crypt kept, a scribe lost in
//! the Archive, an alewife the mushrooms grew round. Hold F at the cage to
//! set them free; they go home up the stair and stay. The Undercroft fills,
//! a rescue at a time, and each who lives there does something for the
//! party.

use super::*;

/// Someone who can be rescued: who, where from, what they do at home.
pub(crate) struct Resident {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    /// The delve whose cage they are found in.
    pub(crate) pack: Pack,
    /// What they do for the party, for the notice and the docs.
    pub(crate) gives: &'static str,
    /// Where they stand in the Undercroft (tiles).
    pub(crate) home_at: (f32, f32),
}

pub(crate) const RESIDENTS: [Resident; 4] = [
    Resident {
        id: "mabel",
        name: "Mabel",
        pack: Pack::Cavern,
        gives: "her stew: 15 more health for every knight",
        home_at: (9.0, 11.6),
    },
    Resident {
        id: "anselm",
        name: "Brother Anselm",
        pack: Pack::Crypt,
        gives: "his blessing: one more second wind a delve",
        home_at: (3.9, 8.7),
    },
    Resident {
        id: "pip",
        name: "Pip",
        pack: Pack::Archive,
        gives: "his maps: every floor drawn whole on arrival",
        home_at: (15.4, 2.8),
    },
    Resident {
        id: "maud",
        name: "Maud",
        pack: Pack::Fungal,
        gives: "her ale: a potion for every knight on the way down",
        home_at: (16.2, 11.4),
    },
];

pub(crate) fn resident(id: &str) -> Option<&'static Resident> {
    RESIDENTS.iter().find(|r| r.id == id)
}

/// Who waits in a cage on this floor: who, in which room, where the cage
/// stands, how long they've been held at (for the F hold), and whether
/// they're free.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Captive {
    pub(crate) who: String,
    pub(crate) room: usize,
    pub(crate) x: f32,
    pub(crate) y: f32,
    #[serde(default)]
    pub(crate) freed: Option<u64>,
}

/// A knight this close to the cage can work its lock.
pub(crate) const CAGE_REACH: f32 = 3.0;
/// The chance a floor holds a cage, if someone is still to be found there.
const CAGE_CHANCE: u64 = 50;

impl Run {
    /// A new floor: perhaps a cage in its treasure room, with whoever of
    /// this delve's people is still to be found.
    pub(super) fn cage_someone(&mut self) {
        self.captive = None;
        let pack = self.dungeon.pack;
        let Some(who) = RESIDENTS
            .iter()
            .find(|r| r.pack == pack && !self.home.residents.contains(r.id))
        else {
            return;
        };
        let Some(room) = self
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Treasure)
        else {
            return;
        };
        if mix(self.seed ^ u64::from(self.dungeon.depth).rotate_left(29)) % 100 >= CAGE_CHANCE {
            return;
        }
        let r = &self.dungeon.rooms[room];
        self.captive = Some(Captive {
            who: who.id.into(),
            room,
            x: r.width() / 2.0,
            y: r.height() / 2.0 - 6.0,
            freed: None,
        });
    }

    /// The cage each tick: holding F beside it frees whoever is inside.
    pub(super) fn tick_cage(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(captive) = self.captive.clone().filter(|c| c.room == self.at) else {
            return;
        };
        if captive.freed.is_some() {
            return;
        }
        let mut freed = false;
        for (&id, hero) in self.players.iter_mut() {
            let near = hero.hp > 0
                && !hero.stone
                && (hero.x - captive.x).hypot(hero.y - captive.y) < CAGE_REACH;
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            if !near || !fire {
                if near {
                    hero.buying = 0;
                }
                continue;
            }
            hero.buying = hero.buying.saturating_add(1);
            if hero.buying >= super::home::BUY_HOLD {
                hero.buying = 0;
                freed = true;
            }
        }
        if freed {
            if let Some(c) = self.captive.as_mut() {
                c.freed = Some(self.tick);
            }
            *self
                .marks
                .entry(format!("rescue:{}", captive.who))
                .or_default() += 1;
            self.cues.push(format!("rescued:{}", captive.who));
            self.sounds.push("door_open");
            self.notice("rescuer");
            self.thrill(60);
        }
    }

    /// What the people at home do for a delve, at the top of the stair:
    /// Anselm's blessing and Maud's ale (Mabel's stew is a home card; Pip
    /// draws each floor as it comes).
    pub(super) fn residents_help(&mut self) {
        let blessed = self.home.residents.contains("anselm");
        let ale = self.home.residents.contains("maud");
        for hero in self.players.values_mut() {
            if blessed {
                hero.winds = hero.winds.saturating_add(1);
            }
            if ale && hero.hand.len() < cards::HAND {
                hero.hand.push("potion".into());
            }
        }
    }

    /// The people at home speak as a knight walks up to them, once per
    /// approach.
    pub(super) fn greet_residents(&mut self) {
        let mut near = 0u8;
        let dug = self
            .dungeon
            .rooms
            .iter()
            .any(|r| r.kind == RoomKind::Tavern);
        for (bit, resident) in RESIDENTS.iter().enumerate() {
            // Once the tavern is dug, Maud keeps its bar instead.
            if !self.home.residents.contains(resident.id) || (dug && resident.id == "maud") {
                continue;
            }
            let (rx, ry) = (
                resident.home_at.0 * TILE_UNITS,
                resident.home_at.1 * TILE_UNITS,
            );
            let close = self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - rx).hypot(h.y - ry) < 4.0);
            if close {
                near |= 1 << bit;
                if self.met & (1 << bit) == 0 {
                    self.cues.push(format!("npc:{}", resident.id));
                }
            }
        }
        self.met = near;
    }

    /// Pip's maps: a floor drawn whole as the party arrives.
    pub(super) fn pips_map(&mut self) {
        if self.home.residents.contains("pip") {
            for room in &mut self.dungeon.rooms {
                room.visited = true;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_rescues__tests.rs"]
mod tests;
