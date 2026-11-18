//! The Deep (the operator, 2026-10-28: "every 10 levels you visit a
//! subterranean village that gives you missions and conversation/message
//! opportunities to define the world and objectives for the proceeding 10
//! floors so its a revolving wheel, not the same cities, we are going down
//! into the dungeon world but we bring hope and the ability to make a
//! difference, like comets or shooting stars to the underground villages").
//!
//! Below the Unknown the stair goes on. The six floors of a delve are as
//! they always were; the Deep is taken on purpose, from the Winding Stair:
//! once the realm has stood on the Unknown's floor, Wren's stair has a
//! landing below it, floor 7, the Deep Road, and every village the party
//! has lit is a landing too. Nothing below runs unless a delve went down
//! that way (`Run::deep`).
//!
//! The Deep is told in chapters of ten floors. Floors 7 to 9 are the road
//! down to the first village; then every tenth floor (10, 20, 30...) is a
//! village of the deep people, a different people each time from a
//! turning wheel (`cultures`), and the nine floors after it are that
//! village's chapter: its errands are laid on them (`missions`), and the
//! next village waits at the bottom.
//!
//! The road between villages is an adventure: every other floor of it
//! holds a meeting, a patrol, a caravan, a hermit, captives, a shrine or an
//! envoy, whose answers feed each people's favour, the errands, the dread
//! at the next stair and what the next village gives (`road`).
//!
//! A floor of the Deep plays as the deep floor above it does (its `ECHO`,
//! the Fungal Deep's depth), so every rule written for the six floors
//! holds on it unchanged: its layout, its guardian, its nook, its stair.
//! The Deep keeps its own count (`Descent::floor`), turns the packs over
//! floor by floor, and raises the stakes on top as it goes (`health`,
//! `hit`). The realm keeps what the Deep remembers (`Chronicle`, on
//! `Home::deep`): the villages met and lit, the errands they asked, the
//! wishes visitors made for them, how each people stands after the road.

use super::*;
use std::collections::BTreeSet;

pub mod city;
pub mod cultures;
pub mod dread;
pub mod missions;
pub mod road;
pub mod village;

pub use cultures::Culture;
pub use missions::{Errand, Mission, Sign};
pub use village::Village;

pub const ROAD: u32 = DEEPEST + 1;
pub const CHAPTER: u32 = 10;
pub const ECHO: u32 = DEEPEST - 1;
pub const PACKS: [Pack; 6] = [Pack::Archive, Pack::Fungal, Pack::Cavern, Pack::Crypt, Pack::Hellforge, Pack::Unknown];
pub const MOST_TOWNS: usize = 64;
pub const MOST_MISSIONS: usize = 24;

pub fn is_village(floor: u32) -> bool {
    floor >= CHAPTER && floor.is_multiple_of(CHAPTER)
}

pub fn chapter(floor: u32) -> u32 {
    floor / CHAPTER
}

pub fn errands_from(floor: u32) -> Option<u32> {
    (floor > CHAPTER && !is_village(floor)).then(|| chapter(floor) * CHAPTER)
}

pub fn pack_at(floor: u32, wheel: u32) -> Pack {
    let turn = floor as usize + chapter(floor) as usize * 5 + wheel as usize;
    PACKS[turn % PACKS.len()]
}

pub fn health(floor: u32) -> u32 {
    160 + 10 * floor.saturating_sub(DEEPEST)
}

pub fn hit(floor: u32) -> u32 {
    240 + 6 * floor.saturating_sub(DEEPEST)
}

pub fn boss_health(floor: u32) -> u32 {
    390 + 20 * floor.saturating_sub(DEEPEST)
}

pub fn purse(floor: u32) -> u32 {
    60 + 40 * chapter(floor)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Descent {
    pub floor: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub village: Option<Village>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub signs: Vec<Sign>,
    #[serde(skip_serializing_if = "is_zero")]
    pub held: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meeting: Option<road::Meeting>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cargo: Vec<road::Cargo>,
    #[serde(skip_serializing_if = "is_zero")]
    pub stir: u8,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl Descent {
    pub fn valid(&self, rooms: usize) -> bool {
        self.floor >= ROAD
            && self.floor < 10_000
            && self.signs.len() <= missions::MOST_SIGNS
            && self.signs.iter().all(|s| s.room < rooms)
            && self.village.as_ref().is_none_or(|v| v.valid() && is_village(self.floor))
            && self.meeting.as_ref().is_none_or(|m| m.valid(rooms) && self.village.is_none())
            && self.cargo.len() <= road::MOST_CARGO
            && self.stir <= road::MOST_STIR
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Town {
    pub floor: u32,
    pub culture: Culture,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lit: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deeds: Vec<Errand>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wishes: Vec<city::CityWish>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dread: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reunited: bool,
}

impl Town {
    pub fn new(floor: u32, culture: Culture) -> Town {
        Town { floor, culture, lit: false, deeds: Vec::new(), wishes: Vec::new(), dread: false, reunited: false }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chronicle {
    #[serde(skip_serializing_if = "is_zero32")]
    pub wheel: u32,
    #[serde(skip_serializing_if = "is_zero32")]
    pub deepest: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub towns: Vec<Town>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missions: Vec<Mission>,
    #[serde(skip_serializing_if = "road::Ledger::is_empty")]
    pub road: road::Ledger,
}

fn is_zero32(n: &u32) -> bool {
    *n == 0
}

impl Chronicle {
    pub fn is_empty(&self) -> bool {
        *self == Chronicle::default()
    }

    pub fn valid(&self) -> bool {
        self.towns.len() <= MOST_TOWNS
            && self.road.valid()
            && self.missions.len() <= MOST_MISSIONS
            && self.towns.iter().all(|t| is_village(t.floor) && t.wishes.len() <= city::SLOTS && t.deeds.len() <= 16)
            && self.towns.iter().map(|t| t.floor).collect::<BTreeSet<_>>().len() == self.towns.len()
            && self.towns.iter().all(|t| t.wishes.iter().all(city::CityWish::valid))
    }

    pub fn town(&self, floor: u32) -> Option<&Town> {
        self.towns.iter().find(|t| t.floor == floor)
    }

    pub fn town_mut(&mut self, floor: u32) -> Option<&mut Town> {
        self.towns.iter_mut().find(|t| t.floor == floor)
    }

    pub fn culture_at(&self, floor: u32) -> Culture {
        self.town(floor).map_or_else(|| cultures::turn(self.wheel, chapter(floor)), |t| t.culture)
    }

    pub fn lit(&self) -> impl Iterator<Item = u32> + '_ {
        self.towns.iter().filter(|t| t.lit).map(|t| t.floor)
    }

    pub fn open_on(&self, floor: u32) -> impl Iterator<Item = (usize, &Mission)> + '_ {
        let from = errands_from(floor);
        self.missions
            .iter()
            .enumerate()
            .filter(move |(_, m)| Some(m.village) == from && !m.done())
    }
}

impl home::Home {
    pub fn deep_road(&self) -> bool {
        self.deepest >= DEEPEST
    }

    pub fn deep_landings(&self) -> Vec<u32> {
        if !self.deep_road() {
            return Vec::new();
        }
        let mut floors: Vec<u32> = std::iter::once(ROAD).chain(self.deep.lit()).collect();
        floors.sort_unstable();
        floors.dedup();
        floors
    }
}

impl Run {
    pub fn deep_floor(&self) -> Option<u32> {
        self.deep.as_ref().map(|d| d.floor)
    }

    pub fn in_deep_village(&self) -> bool {
        self.deep.as_ref().is_some_and(|d| d.village.is_some()) && self.room().kind == RoomKind::DeepVillage
    }

    pub fn star_falling(&self) -> bool {
        self.in_deep_village() && self.deep.as_ref().and_then(|d| d.village.as_ref()).is_some_and(|v| v.falling(self.tick))
    }

    pub fn star_camera(&self) -> Option<(f32, f32)> {
        let v = self.deep.as_ref()?.village.as_ref().filter(|_| self.star_falling())?;
        let t = (self.tick.saturating_sub(v.arrived) as f32 / village::FALL as f32 - 0.7).max(0.0) / 0.3;
        let ease = t * t * (3.0 - 2.0 * t);
        Some(village::units((village::LANDING.0, 4.0 + (village::LANDING.1 - 4.0) * ease)))
    }

    pub(super) fn deep_health(&self) -> Option<u32> {
        self.deep_floor().map(health)
    }

    pub(super) fn deep_hit(&self) -> Option<u32> {
        self.deep_floor().map(hit)
    }

    pub(super) fn deep_boss_health(&self) -> Option<u32> {
        self.deep_floor().map(boss_health)
    }

    pub(super) fn enter_the_deep(&mut self, floor: u32) {
        self.first = self.dungeon.pack;
        if self.home.deep.wheel == 0 {
            self.home.deep.wheel = (mix(self.seed ^ self.raid_id.rotate_left(29)) % 997) as u32 + 1;
        }
        self.deep = Some(Descent { floor, ..Descent::default() });
        let pack = self.deep_pack(floor);
        self.finish_descent(ECHO, pack, true);
    }

    pub(super) fn descend_deep(&mut self) {
        let Some(floor) = self.deep_floor() else {
            return;
        };
        self.close_floor(floor_stats::End::Cleared);
        self.settle_dare();
        self.unwashed_toll();
        self.lift_gifts();
        if !is_village(floor) {
            self.reclaimed.push(self.dungeon.pack);
        }
        self.bank_all(true, &format!("floor {floor} of the Deep cleared"));
        let next = floor + 1;
        if let Some(d) = &mut self.deep {
            d.floor = next;
            d.village = None;
            d.signs.clear();
            d.meeting = None;
        }
        let pack = self.deep_pack(next);
        self.finish_descent(ECHO, pack, false);
    }

    fn deep_pack(&self, floor: u32) -> Pack {
        if is_village(floor) {
            self.home.deep.culture_at(floor).kin()
        } else {
            pack_at(floor, self.home.deep.wheel)
        }
    }

    pub(super) fn deep_village_floor(&mut self) -> Option<Floor> {
        let floor = self.deep_floor().filter(|&f| is_village(f))?;
        let culture = self.home.deep.culture_at(floor);
        Some(village::floor(culture, ECHO))
    }

    pub(super) fn ready_deep_floor(&mut self) {
        let Some(floor) = self.deep_floor() else {
            return;
        };
        if floor > self.home.deep.deepest {
            self.home.deep.deepest = floor;
        }
        if is_village(floor) {
            self.arrive_at_village(floor);
        } else {
            self.lay_signs(floor);
            self.lay_meeting(floor);
        }
    }

    pub(super) fn tick_deep(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut deep) = self.deep.take() else {
            return;
        };
        let held = self
            .players
            .keys()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .fold(0u8, |bits, &id| bits | 1 << (id & 7));
        let pressed = held & !deep.held;
        deep.held = held;
        if let Some(mut v) = deep.village.take() {
            if self.room().kind == RoomKind::DeepVillage {
                self.tick_village(&mut v, pressed);
            }
            deep.village = Some(v);
        } else {
            self.tick_signs(&mut deep, pressed);
            self.tick_road(&mut deep, pressed);
        }
        self.deep = Some(deep);
    }

    pub(super) fn deep_inputs(&mut self, inputs: &BTreeMap<u32, Input>) -> Option<BTreeMap<u32, Input>> {
        let village = self.in_deep_village();
        let falling = village && self.deep.as_ref().and_then(|d| d.village.as_ref()).is_some_and(|v| v.falling(self.tick));
        let talking: Vec<u32> = if village {
            self.deep
                .as_ref()
                .and_then(|d| d.village.as_ref())
                .map(|v| v.talks.keys().copied().collect())
                .unwrap_or_default()
        } else {
            self.road_talking()
        };
        if !falling && talking.is_empty() {
            return None;
        }
        let mut out = inputs.clone();
        for (&id, input) in out.iter_mut() {
            if !input.valid() {
                continue;
            }
            if falling {
                *input = Input::default();
                continue;
            }
            if talking.contains(&id) && input.cast != 0 {
                let pick = input.cast;
                input.cast = 0;
                if village {
                    self.answer(id, pick);
                } else {
                    self.answer_road(id, pick);
                }
            }
        }
        Some(out)
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    pub fn deep_floor_for_test(&mut self, floor: u32) {
        if self.home.deep.wheel == 0 {
            self.home.deep.wheel = 1;
        }
        self.deep = Some(Descent { floor, ..Descent::default() });
        let pack = self.deep_pack(floor);
        self.finish_descent(ECHO, pack, false);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/deep_villages__tests.rs"]
mod tests;
