//! The Gnome's Garden (the operator, 2026-10-27): a party gnome's cottage on
//! the third floor of an open delve, its garden full of little blue folk who
//! keep to the walls and fairies who fly a little crooked. Locally grown
//! herbs and spices on the drying rack, mushrooms in the glowing bed, a copper
//! kettle out back that is never for tea, and barrels nobody opens. The
//! gnome offers three things:
//!
//! - **Herbs and spices** (E at the rack): +25% fire rate this floor.
//! - **Mushrooms** (E at the bed): +25% damage this floor, and the room
//!   goes strange for a while.
//! - **Stay a while** (E at the gnome): everyone in the garden is mended
//!   and the party starts. Then the garden's other guests arrive:
//!   caterpillars, worms and beetles, four waves of them, and every one
//!   wants into the cellar, not at a knight. Each that reaches the door
//!   eats into the house. Hold the door through all four and the fairies
//!   give every knight in the garden a fairy wish: their next wish lands
//!   three steps up at once, at its third tier, Legendary. Lose the house
//!   and the party is over; what is left in the garden turns on the
//!   knights.
//!
//! Nobody says what is in the kettle.
use super::*;

pub const DEPTH: u32 = 3;
pub const FLAMINGO: &str = "lawn-flamingo";
pub const HOUSE: (i32, i32, i32, i32) = (9, 1, 6, 4);
pub const DOOR: (f32, f32) = (12.0, 5.4);
pub const KETTLE: (i32, i32, i32, i32) = (15, 1, 3, 3);
pub const BARRELS: (i32, i32, i32, i32) = (18, 1, 4, 2);
pub const BLOCKS: [(i32, i32, i32, i32); 3] = [HOUSE, KETTLE, BARRELS];
pub const HERBS: (f32, f32) = (4.5, 7.0);
pub const SHROOMS: (f32, f32) = (19.5, 7.0);
pub const GNOME: (f32, f32) = (12.0, 6.9);
pub const FOLK: [(f32, f32); 4] = [(7.6, 4.6), (17.6, 5.7), (2.6, 11.2), (21.4, 10.6)];
pub const GATES: [(f32, f32); 6] = [(1.6, 12.4), (22.4, 12.4), (1.6, 8.0), (22.4, 8.0), (6.0, 12.6), (18.0, 12.6)];

pub const HERB_RATE: u32 = 25;
pub const SHROOM_DAMAGE: u32 = 25;
pub const SHROOM_TICKS: u32 = 25 * HZ;
pub const HOUSE_HP: u32 = 100;
pub const BITE: u32 = 10;
pub const IN_REACH: f32 = 1.4;
pub const WAVES: [&[(EnemyKind, u8)]; 4] = [
    &[(EnemyKind::Leech, 6)],
    &[(EnemyKind::Leech, 4), (EnemyKind::Biter, 5)],
    &[(EnemyKind::Spiderling, 7), (EnemyKind::Leech, 4)],
    &[(EnemyKind::Biter, 6), (EnemyKind::Spiderling, 5), (EnemyKind::Silkmother, 1)],
];
pub const SPAWN_EVERY: u32 = HZ * 2 / 3;
pub const FIRST_WAVE: u32 = 3 * HZ;
pub const WAVE_REST: u32 = 4 * HZ;
pub const HELD_LINE: &str = "THE HOUSE HELD! THE FAIRIES GRANT EVERY KNIGHT A FAIRY WISH: TIER III, LEGENDARY.";
pub const LOST_LINE: &str = "THEY GOT INTO THE CELLAR. THE PARTY'S OVER.";

pub const WAVE_LINES: [&str; 4] = [
    "SOMETHING'S AT THE CELLAR DOOR!",
    "MORE OF THEM! THEY SMELL IT!",
    "BEETLES! WHO INVITED BEETLES?",
    "THE BIG ONE'S COMING FOR THE KETTLE!",
];

pub const FAIRY_STEPS: u8 = 3;
pub const MOST_GUESTS: usize = 48;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Party {
    #[default]
    Quiet,
    Holding,
    Held,
    Lost,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Garden {
    pub party: Party,
    pub house: u32,
    pub wave: u8,
    pub wave_t: u32,
    pub sent: u8,
    pub guests: Vec<u32>,
    pub took: BTreeMap<u32, u8>,
    pub trips: BTreeMap<u32, u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitten: Option<(u64, EnemyKind)>,
}

pub fn wave_size(wave: usize) -> u8 {
    WAVES.get(wave).map_or(0, |w| w.iter().map(|&(_, n)| n).sum())
}

fn guest_of(wave: usize, k: u8) -> EnemyKind {
    let mut left = k;
    for &(kind, n) in WAVES[wave] {
        if left < n {
            return kind;
        }
        left -= n;
    }
    WAVES[wave][0].0
}

impl Garden {
    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.house <= HOUSE_HP
            && usize::from(self.wave) < WAVES.len()
            && self.sent <= wave_size(usize::from(self.wave))
            && self.guests.len() <= MOST_GUESTS
            && self.took.len() <= MAX_PLAYERS as usize
            && self.took.values().all(|&t| t <= 3)
            && self.took.keys().all(seat)
            && self.trips.len() <= MAX_PLAYERS as usize
            && self.trips.values().all(|&t| t <= SHROOM_TICKS)
            && self.trips.keys().all(seat)
    }

    pub fn lure(&self) -> Option<((f32, f32), &[u32])> {
        (self.party == Party::Holding).then(|| ((DOOR.0 * TILE_UNITS, DOOR.1 * TILE_UNITS), self.guests.as_slice()))
    }
}

impl Run {
    pub(crate) fn flamingo(&mut self) -> Option<usize> {
        if let Some(i) = self.bosses.iter().position(|b| b.id == FLAMINGO) {
            return Some(i);
        }
        if self.bosses.len() >= usize::from(u8::MAX) {
            return None;
        }
        self.bosses.push(bosses::lawn_flamingo());
        Some(self.bosses.len() - 1)
    }

    pub fn tripping(&self, id: u32) -> bool {
        self.nook.as_ref().and_then(|n| n.garden.as_ref()).is_some_and(|g| {
            self.dungeon.rooms.get(self.at).is_some_and(|r| r.kind == RoomKind::GnomeParty)
                && (g.party == Party::Holding || g.trips.get(&id).is_some_and(|&t| t > 0))
        })
    }

    pub(crate) fn garden_lure(&self) -> Option<((f32, f32), Vec<u32>)> {
        let nook = self.nook.as_ref().filter(|n| n.room == self.at)?;
        nook.garden.as_ref()?.lure().map(|(at, ids)| (at, ids.to_vec()))
    }

    pub(super) fn tick_gnome(&mut self, nook: &mut Nook, pressed: u8) {
        let mut g = nook.garden.take().unwrap_or_default();
        for t in g.trips.values_mut() {
            *t = t.saturating_sub(1);
        }
        g.trips.retain(|_, t| *t > 0);
        for id in self.nook_players() {
            if !has(pressed, id) {
                continue;
            }
            let took = g.took.get(&id).copied().unwrap_or(0);
            if self.nook_near(id, HERBS, REACH) {
                if took & 1 != 0 {
                    self.found = Some((self.tick, id, "HE SAYS: PACE YOURSELF.".into()));
                    continue;
                }
                g.took.insert(id, took | 1);
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.boon.rate = hero.boon.rate.max(HERB_RATE);
                }
                *nook.score.entry(id).or_default() += 1;
                self.sounds.push("potion");
                self.found = Some((self.tick, id, "LOCALLY GROWN. +25% FIRE RATE THIS FLOOR.".into()));
            } else if self.nook_near(id, SHROOMS, REACH) {
                if took & 2 != 0 {
                    self.found = Some((self.tick, id, "HE SAYS: ONE'S PLENTY, FRIEND.".into()));
                    continue;
                }
                g.took.insert(id, took | 2);
                g.trips.insert(id, SHROOM_TICKS);
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.boon.damage = hero.boon.damage.max(SHROOM_DAMAGE);
                }
                *nook.score.entry(id).or_default() += 1;
                self.sounds.push("holy_water");
                self.found = Some((self.tick, id, "FOR THE ADVENTUROUS PALATE. +25% DAMAGE THIS FLOOR.".into()));
            } else if self.nook_near(id, GNOME, REACH + 0.6) {
                match g.party {
                    Party::Quiet => {
                        g.party = Party::Holding;
                        g.house = HOUSE_HP;
                        g.wave = 0;
                        g.wave_t = 0;
                        g.sent = 0;
                        for hero in self.players.values_mut().filter(|h| h.hp > 0) {
                            hero.hp = hero.max_hp;
                        }
                        self.sounds.push("wish_grant");
                        self.cues.push("gnome_party".into());
                        self.found = Some((self.tick, id, "STAY A WHILE. WE'RE HAVING A FEW FRIENDS OVER.".into()));
                    }
                    Party::Holding => self.found = Some((self.tick, id, "THE DOOR! MIND THE DOOR!".into())),
                    Party::Held => self.found = Some((self.tick, id, "BEST PARTY IN YEARS.".into())),
                    Party::Lost => self.found = Some((self.tick, id, "THEY GOT INTO THE CELLAR. ALL OF IT.".into())),
                }
            }
        }
        if g.party == Party::Holding {
            self.hold_the_door(nook, &mut g);
        }
        nook.garden = Some(g);
    }

    fn hold_the_door(&mut self, nook: &mut Nook, g: &mut Garden) {
        let door = (DOOR.0 * TILE_UNITS, DOOR.1 * TILE_UNITS);
        let mut bitten = 0;
        let mut last = None;
        let reach = IN_REACH * TILE_UNITS;
        let guests = g.guests.clone();
        self.enemies.retain(|e| {
            let at_door = guests.contains(&e.id) && e.hp > 0 && (e.x - door.0).hypot(e.y - door.1) < reach + e.radius();
            if at_door {
                bitten += 1;
                last = Some(e.kind);
            }
            !at_door
        });
        if let Some(kind) = last {
            g.bitten = Some((self.tick, kind));
        }
        if bitten > 0 {
            g.house = g.house.saturating_sub(BITE * bitten);
            self.sounds.push("crack_open");
            self.shake = self.shake.max(6);
        }
        let living: Vec<u32> = self.enemies.iter().filter(|e| e.hp > 0).map(|e| e.id).collect();
        g.guests.retain(|id| living.contains(id));
        if g.house == 0 {
            g.party = Party::Lost;
            g.guests.clear();
            nook.done = true;
            self.sounds.push("boss_rage");
            self.found = Some((self.tick, 0, LOST_LINE.into()));
            return;
        }
        let wave = usize::from(g.wave);
        g.wave_t = g.wave_t.saturating_add(1);
        let size = wave_size(wave);
        let start = if wave == 0 { FIRST_WAVE } else { WAVE_REST };
        if g.wave_t >= start && g.sent < size && (g.wave_t - start).is_multiple_of(SPAWN_EVERY) {
            let gate = GATES[(usize::from(g.sent) + wave * 3) % GATES.len()];
            if g.sent == 0 {
                self.sounds.push("wave_gate");
                self.found = Some((self.tick, 0, WAVE_LINES[wave].into()));
            }
            let kind = guest_of(wave, g.sent);
            g.sent += 1;
            if let Some(id) = self.spawn_at(kind, gate.0 * TILE_UNITS, gate.1 * TILE_UNITS)
                && g.guests.len() < MOST_GUESTS
            {
                g.guests.push(id);
            }
        }
        if g.sent >= size && g.guests.is_empty() {
            if wave + 1 < WAVES.len() {
                g.wave += 1;
                g.wave_t = 0;
                g.sent = 0;
            } else {
                g.party = Party::Held;
                nook.done = true;
                self.fairy_wishes_for_the_garden(nook);
            }
        }
    }

    fn fairy_wishes_for_the_garden(&mut self, nook: &Nook) {
        for id in self.nook_players() {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.fairy_wishes = hero.fairy_wishes.saturating_add(1).min(overclass::MOST_WISHES);
            }
        }
        self.nook_crown(nook);
        self.sounds.push("fairy_wish");
        self.cues.push("fairy_wish".into());
        self.found = Some((self.tick, 0, HELD_LINE.into()));
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/together_gnome__tests.rs"]
mod tests;
