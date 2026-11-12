//! Villagers: the named inhabitants of a holding.
//!
//! Where `citizens` paints whoever would be about, folk are somebody:
//! a stored roster per holding — a name, a trade, a mood, a belly —
//! who come as migrants when there is a steward to receive them, a
//! standing room to lodge in and grain in the granary, and who leave
//! when the realm lets them go hungry, houseless or afraid.
//!
//! Villagers work the economy the holdings already run, and no other:
//!
//! - every four mouths eat a sack a rent day from the same granary
//!   cache hits fill and token wages empty, so a realm that spends
//!   tokens it does not cache starves its own villages first;
//! - labourers lay courses on the room going up in their holding;
//! - a fed guard holds the walls the way a retainer does;
//! - misery is remembered: hunger, no bed, raiders and a restless
//!   realm each sour a mood, and at the bottom of it folk walk away.
//!
//! Villagers never mint gold and never pay rent: rent still comes only
//! from rooms, as `holdings` has it. Everything here is a pure
//! function of the realm's state at a rent day (`keep`, called from
//! `holdings::rent_day`), so it is tested without a cockpit and a
//! mirror that sees the same holdings names the same folk.

use super::holdings::{room_def, Deed, Family, Holding, Holdings, RESTLESS, ROOM_LABOUR, STORES_CAP};
use super::occasions::{self, Cause, Dying};
use serde::{Deserialize, Serialize};

/// The most folk one holding keeps (the painted crowd's own cap).
pub const MAX_VILLAGERS: usize = 12;
/// Mouths one sack from the granary feeds for a rent day.
pub const MOUTHS_PER_SACK: usize = 4;
/// A mood runs this far either way; at the bottom, folk leave.
pub const MOOD_CAP: i8 = 5;
pub const MOOD_LEAVES: i8 = -4;
/// Rent days unfed a belly remembers.
pub const HUNGER_CAP: u8 = 3;
/// Raiders who have held a holding this many rent days start killing.
pub const RAID_FALLS: u8 = 2;

/// A trade, worked in a standing room of its family — or, for a
/// labourer, on whichever room is going up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trade {
    Brewer,
    Smith,
    Acolyte,
    Trader,
    Guard,
    Labourer,
    /// Not yet of age to work a trade; still a mouth at dinner.
    Child,
}

impl Trade {
    pub fn word(self) -> &'static str {
        match self {
            Trade::Brewer => "brewer",
            Trade::Smith => "smith",
            Trade::Acolyte => "acolyte",
            Trade::Trader => "trader",
            Trade::Guard => "guard",
            Trade::Labourer => "labourer",
            Trade::Child => "child",
        }
    }

    /// The room family this trade staffs, if it staffs one.
    pub fn family(self) -> Option<Family> {
        match self {
            Trade::Brewer => Some(Family::Tavern),
            Trade::Smith => Some(Family::Forge),
            Trade::Acolyte => Some(Family::Chapel),
            Trade::Trader => Some(Family::Trade),
            Trade::Guard => Some(Family::Barracks),
            Trade::Labourer | Trade::Child => None,
        }
    }

    pub fn of_family(family: Family) -> Trade {
        match family {
            Family::Tavern => Trade::Brewer,
            Family::Forge => Trade::Smith,
            Family::Chapel => Trade::Acolyte,
            Family::Trade => Trade::Trader,
            Family::Barracks => Trade::Guard,
        }
    }
}

/// One inhabitant of a holding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Villager {
    pub name: String,
    pub trade: Trade,
    /// How this realm is treating them, `-MOOD_CAP..=MOOD_CAP`.
    #[serde(default)]
    pub mood: i8,
    /// Rent days since they were last fed, to `HUNGER_CAP`.
    #[serde(default)]
    pub hunger: u8,
    /// A practiced hand (0..=3): a labourer of skill 2+ lays double.
    #[serde(default)]
    pub skill: u8,
    /// Rent days worked fed at the trade: every `SEASONS_PER_SKILL`
    /// of them, the hand is practiced one grade more.
    #[serde(default)]
    pub practiced: u8,
    /// Years lived. Folk under `COMING_OF_AGE` work no trade; from
    /// `ELDER` they keep the fireside instead of the walls. A soul
    /// from an older save (no age written) reads as thirty.
    #[serde(default = "adult_age")]
    pub age: u8,
    /// Rent days into the current year (`SEASONS_PER_YEAR` to a year).
    #[serde(default)]
    pub seasons: u8,
}

fn adult_age() -> u8 {
    30
}

/// Fed, working rent days per grade of skill.
pub const SEASONS_PER_SKILL: u8 = 6;
/// Rent days to a year of a life.
pub const SEASONS_PER_YEAR: u8 = 12;
/// When a child takes up a trade.
pub const COMING_OF_AGE: u8 = 14;
/// When hands retire from labour and the wall-watch.
pub const ELDER: u8 = 65;
/// Stars a holding needs before folk start families in it: a
/// chartered town, not a camp with a coffer.
pub const FAMILY_STARS: u8 = 3;

impl Villager {
    pub fn valid(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= 24
            && self.mood.abs() <= MOOD_CAP
            && self.hunger <= HUNGER_CAP
            && self.skill <= 3
            && self.practiced <= SEASONS_PER_SKILL * 3
            && self.age <= 120
            && self.seasons < SEASONS_PER_YEAR
            && (self.trade != Trade::Child || self.age < COMING_OF_AGE)
    }

    fn cheer(&mut self, n: i8) {
        self.mood = (self.mood + n).clamp(-MOOD_CAP, MOOD_CAP);
    }
}

pub(super) const NAMES: [&str; 32] = [
    "Berta", "Col", "Dunstan", "Edith", "Fenwick", "Goda", "Harold", "Isolde",
    "Jory", "Katell", "Leofric", "Mabel", "Nyle", "Odilia", "Piers", "Ragnild",
    "Sabel", "Tostig", "Uda", "Warin", "Ysolde", "Alfric", "Blythe", "Cedric",
    "Drusa", "Emeric", "Falco", "Gisela", "Hamo", "Ivo", "Lutgard", "Osbern",
];

pub(super) fn mix(seed: u64, n: u32) -> u64 {
    let mut z = seed ^ (u64::from(n).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub(super) fn seed_of(key: &str) -> u64 {
    key.bytes().fold(0x2545_F491_4F6C_DD1Du64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3))
}

/// The keeper of a countryside plot, by its id: the same soul keeps
/// the same bakery in every realm, the way the stewards are a fixed
/// cast at the Court. The countryside stores the name once given.
pub fn keeper_name(plot: &str) -> String {
    let h = mix(seed_of(plot) ^ 0x4B45_4550, 7);
    NAMES[(h % NAMES.len() as u64) as usize].to_string()
}

/// The `n`th migrant ever to come to `key`: always the same soul, so
/// equal realms name equal folk.
fn migrant(key: &str, n: u32, trade: Trade) -> Villager {
    let h = mix(seed_of(key), n);
    Villager {
        name: NAMES[(h % NAMES.len() as u64) as usize].to_string(),
        trade,
        mood: 1,
        hunger: 0,
        skill: ((h >> 8) % 3) as u8,
        practiced: 0,
        age: 20 + ((h >> 16) % 25) as u8,
        seasons: 0,
    }
}

/// The years allotted to a soul, fixed by their name: nobody rolls
/// dice with death, and equal realms mourn on the same day.
fn lifespan(name: &str) -> u8 {
    72 + (mix(seed_of(name), 3) % 18) as u8
}

/// The trade a migrant to this holding works: the family of the first
/// standing room nobody staffs yet, else a labourer's hands.
fn wanted(w: &Holding) -> Trade {
    for room in w.rooms.iter().filter(|r| r.standing()) {
        let Some(def) = room_def(&room.id) else { continue };
        if !w.folk.iter().any(|f| f.trade.family() == Some(def.family)) {
            return Trade::of_family(def.family);
        }
    }
    Trade::Labourer
}

/// The most guests one holding lodges at once.
pub const MAX_VISITORS: usize = 3;
/// The longest a guest stays, in rent days.
pub const VISIT_DAYS: u8 = 3;

/// Who comes visiting, drawn by what stands: a troubadour to a
/// taproom, a pilgrim to a shrine, a merchant wherever goods or
/// coin move, a hunter where there are kennels and ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisitorKind {
    Merchant,
    Pilgrim,
    Troubadour,
    Hunter,
}

impl VisitorKind {
    pub fn word(self) -> &'static str {
        match self {
            VisitorKind::Merchant => "merchant",
            VisitorKind::Pilgrim => "pilgrim",
            VisitorKind::Troubadour => "troubadour",
            VisitorKind::Hunter => "hunter",
        }
    }

    fn drawn_by(family: Family) -> VisitorKind {
        match family {
            Family::Tavern => VisitorKind::Troubadour,
            Family::Forge | Family::Trade => VisitorKind::Merchant,
            Family::Chapel => VisitorKind::Pilgrim,
            Family::Barracks => VisitorKind::Hunter,
        }
    }
}

/// A guest: here for a few rent days, paying their way in kind —
/// a merchant's grain, a troubadour's songs, a pilgrim's calming
/// word, a hunter's watch on the walls — and never in gold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visitor {
    pub name: String,
    pub kind: VisitorKind,
    /// Rent days left of the stay, today's included.
    #[serde(default)]
    pub days: u8,
}

impl Visitor {
    pub fn valid(&self) -> bool {
        !self.name.is_empty() && self.name.len() <= 24 && (1..=VISIT_DAYS).contains(&self.days)
    }
}

/// One rent day of village life in every holding: folk eat, work,
/// take the realm's measure, arrive and depart. The lines it says,
/// for the ledger. Called by `holdings::rent_day`, once a rent day.
pub fn keep(h: &mut Holdings) -> Vec<String> {
    let mut said = Vec::new();
    let mut told: Vec<String> = Vec::new();
    for i in 0..h.owned.len() {
        let unrest = h.unrest;
        let w = &mut h.owned[i];
        let name = w.deed().map_or_else(|| w.key.clone(), Deed::name);
        let beds = w.standing();
        // The last rent day's occasion is over; this one may make its own.
        w.occasion = None;
        let mut short = false;
        let mut dying: Vec<(usize, Dying)> = Vec::new();

        // Dinner first: a sack feeds four mouths. A short granary is
        // felt by everybody at the table.
        let mouths = w.folk.len();
        if mouths > 0 {
            let sacks = mouths.div_ceil(MOUTHS_PER_SACK) as u32;
            if h.stores >= sacks {
                h.stores -= sacks;
                for f in &mut w.folk {
                    f.hunger = 0;
                    f.cheer(1);
                }
            } else {
                h.stores = 0;
                short = true;
                // The frail, already at the end of their belly, starve.
                for (n, f) in w.folk.iter().enumerate() {
                    if f.hunger >= HUNGER_CAP && occasions::frail(f) {
                        dying.push((n, Dying::Hunger));
                    }
                }
                for f in &mut w.folk {
                    f.hunger = (f.hunger + 1).min(HUNGER_CAP);
                    f.cheer(-1);
                }
                said.push(format!("The folk of {name} go hungry: the granary is bare"));
            }
        }

        // Beds, raiders and the realm's temper.
        for (n, f) in w.folk.iter_mut().enumerate() {
            if n >= beds {
                f.cheer(-1);
            }
            if w.raid > 0 || unrest >= RESTLESS {
                f.cheer(-1);
            }
        }

        // Raiders who have held the place a while kill: a guard first,
        // else the first grown soul.
        if w.raid >= RAID_FALLS {
            let victim = w
                .folk
                .iter()
                .position(|f| f.trade == Trade::Guard && f.age < ELDER)
                .or_else(|| w.folk.iter().position(|f| f.trade != Trade::Child));
            if let Some(n) = victim.filter(|n| !dying.iter().any(|(d, _)| d == n)) {
                dying.push((n, Dying::Raiders));
            }
        }

        // Fed labourers lay courses on the room going up here.
        // Elders keep the fireside; their hands have done enough.
        let hands: u32 = w
            .folk
            .iter()
            .filter(|f| f.trade == Trade::Labourer && f.hunger == 0 && f.age < ELDER)
            .map(|f| if f.skill >= 2 { 2 } else { 1 })
            .sum();
        if hands > 0 {
            if let Some(r) = w.rooms.iter_mut().find(|r| !r.standing()) {
                r.labour = (r.labour + hands).min(ROOM_LABOUR);
                if r.standing() {
                    let room = room_def(&r.id).map_or("A room", |d| d.name);
                    said.push(format!("{room} stands at {name}, raised by its folk"));
                }
            }
        }

        // Fed hands at a standing trade grow practiced: seasons of
        // work, a grade at a time, and the ledger marks the day.
        for f in &mut w.folk {
            if f.hunger > 0 || f.trade == Trade::Child || f.age >= ELDER {
                continue;
            }
            let worked = match f.trade.family() {
                Some(family) => w.rooms.iter().any(|r| {
                    r.standing() && room_def(&r.id).is_some_and(|d| d.family == family)
                }),
                None => w.rooms.iter().any(|r| !r.standing()),
            };
            if !worked {
                continue;
            }
            f.practiced = (f.practiced + 1).min(SEASONS_PER_SKILL * 3);
            let grade = f.practiced / SEASONS_PER_SKILL;
            if grade > f.skill {
                f.skill = grade;
                said.push(format!("{} is now a practiced {} of {}", f.name, f.trade.word(), name));
            }
        }

        // A fey mood may take a practiced craftsman's workshop.
        for line in super::fey::tend(w, &mut h.stores, &name) {
            said.push(line.clone());
            told.push(line);
        }

        // A year turns for everybody, twelve rent days at a time.
        // Children come of age into the trade the holding wants;
        // the old die in their season, and the village grieves.
        for f in &mut w.folk {
            f.seasons += 1;
            if f.seasons >= SEASONS_PER_YEAR {
                f.seasons = 0;
                f.age = (f.age + 1).min(120);
            }
        }
        let mut mourned = Vec::new();
        let mut last = None;
        let mut at = 0;
        w.folk.retain(|f| {
            let n = at;
            at += 1;
            let why = dying
                .iter()
                .find(|(d, _)| *d == n)
                .map(|&(_, why)| why)
                .or_else(|| (f.age > lifespan(&f.name)).then_some(Dying::Years));
            let Some(why) = why else { return true };
            mourned.push(match why {
                Dying::Years => format!("{} the {} has died at {}, mourned at {}", f.name, f.trade.word(), f.age, name),
                Dying::Hunger => format!("{} the {} has starved at {}, mourned at {}", f.name, f.trade.word(), f.age, name),
                Dying::Raiders => format!("{} the {} has fallen to the raiders at {}, mourned at {}", f.name, f.trade.word(), f.age, name),
            });
            last = Some((f.name.clone(), f.trade, why));
            false
        });
        let grief = last.is_some();
        if !mourned.is_empty() {
            for f in &mut w.folk {
                f.cheer(-1);
            }
            told.extend(mourned.iter().cloned());
            said.extend(mourned);
        }
        if let Some((who, trade, why)) = last {
            occasions::mourn(w, &who, trade, why);
        }
        for i in 0..w.folk.len() {
            if w.folk[i].trade == Trade::Child && w.folk[i].age >= COMING_OF_AGE {
                let trade = wanted(w);
                w.folk[i].trade = trade;
                said.push(format!("{} comes of age at {} and takes up the {}", w.folk[i].name, name, trade.word()));
            }
        }

        // The miserable walk away, and are missed by name.
        let mut gone = Vec::new();
        w.folk.retain(|f| {
            let stay = f.mood > MOOD_LEAVES;
            if !stay {
                gone.push(format!("{} the {} has left {}", f.name, f.trade.word(), name));
            }
            stay
        });
        said.extend(gone);

        // A soul one step from walking away throws a tantrum; failing
        // that, a full granary is a feast. A mourning keeps the day.
        let afraid = w.raid > 0 || unrest >= RESTLESS;
        let cause = |n: usize, f: &Villager| {
            if short || f.hunger > 0 {
                Cause::Hunger
            } else if n >= beds {
                Cause::NoBed
            } else if afraid {
                Cause::Fear
            } else if grief {
                Cause::Grief
            } else {
                Cause::Restless
            }
        };
        let line = occasions::tantrum(w, &mut h.stores, &name, cause).or_else(|| occasions::feast(w, &mut h.stores, &name));
        if let Some(line) = line {
            said.push(line.clone());
            told.push(line);
        }

        // Word travels: one migrant a rent day where there is a
        // steward to receive them, a bed unclaimed, grain left after
        // dinner, and no raiders at the gate.
        if w.steward.is_some()
            && w.raid == 0
            && unrest < RESTLESS
            && h.stores > 0
            && w.folk.len() < beds.min(MAX_VILLAGERS)
        {
            let trade = wanted(w);
            let soul = migrant(&w.key, h.folk_next, trade);
            h.folk_next += 1;
            said.push(format!("{} the {} comes to {}", soul.name, soul.trade.word(), name));
            w.folk.push(soul);
        }

        // Guests do the day's good they came with, count down their
        // stay, and take the road when it runs out.
        for guest in &w.visitors {
            match guest.kind {
                VisitorKind::Merchant => h.stores = (h.stores + 1).min(STORES_CAP),
                VisitorKind::Troubadour => {
                    for f in &mut w.folk {
                        f.cheer(1);
                    }
                }
                VisitorKind::Pilgrim | VisitorKind::Hunter => {}
            }
        }
        // A guest whose stay runs out in a fed village with a bed
        // to spare petitions to settle, and the steward grants it:
        // a traveler becomes a neighbour, trade and all. The rest
        // take the road.
        let mut left = Vec::new();
        let guests = std::mem::take(&mut w.visitors);
        for mut guest in guests {
            guest.days = guest.days.saturating_sub(1);
            if guest.days > 0 {
                w.visitors.push(guest);
                continue;
            }
            let fed = w.folk.iter().all(|f| f.hunger == 0);
            let settle = w.steward.is_some()
                && w.raid == 0
                && fed
                && h.stores > 0
                && w.folk.len() < beds.min(MAX_VILLAGERS);
            if settle {
                let trade = match guest.kind {
                    VisitorKind::Merchant => Trade::Trader,
                    VisitorKind::Pilgrim => Trade::Acolyte,
                    VisitorKind::Troubadour => Trade::Brewer,
                    VisitorKind::Hunter => Trade::Guard,
                };
                let skill = 1 + (mix(seed_of(&w.key), h.folk_next) % 2) as u8;
                h.folk_next += 1;
                said.push(format!(
                    "{} the {} asks to stay at {} — and does, as its {}",
                    guest.name,
                    guest.kind.word(),
                    name,
                    trade.word()
                ));
                let age = 20 + (mix(seed_of(&guest.name), 5) % 25) as u8;
                w.folk.push(Villager { name: guest.name, trade, mood: 2, hunger: 0, skill, practiced: 0, age, seasons: 0 });
            } else {
                left.push(format!("{} the {} takes the road from {}", guest.name, guest.kind.word(), name));
            }
        }
        said.extend(left);

        // And the road brings one more, now and then: drawn by a
        // room that stands, received by the steward, kept out by
        // raiders like everybody else. The attempt counter turns
        // whether or not the road provides, so the gate is a fixed
        // arithmetic of the holding's history, never dice.
        let drawn: Vec<VisitorKind> = {
            let mut kinds: Vec<VisitorKind> = Vec::new();
            for room in w.rooms.iter().filter(|r| r.standing()) {
                if let Some(def) = room_def(&room.id) {
                    let kind = VisitorKind::drawn_by(def.family);
                    if !kinds.contains(&kind) {
                        kinds.push(kind);
                    }
                }
            }
            kinds
        };
        if w.steward.is_some() && w.raid == 0 && w.visitors.len() < MAX_VISITORS && !drawn.is_empty() {
            let n = h.visitor_next;
            h.visitor_next += 1;
            let seed = seed_of(&w.key) ^ 0x71_5175_ED;
            let roll = mix(seed, n);
            if roll % 2 == 0 {
                let kind = drawn[(roll >> 4) as usize % drawn.len()];
                let guest = Visitor {
                    name: NAMES[(roll >> 8) as usize % NAMES.len()].to_string(),
                    kind,
                    days: 2 + ((roll >> 3) % 2) as u8,
                };
                if kind == VisitorKind::Pilgrim {
                    h.unrest = h.unrest.saturating_sub(1);
                }
                said.push(format!("{} the {} lodges at {}", guest.name, guest.kind.word(), name));
                w.visitors.push(guest);
            }
        }

        // And where the charter makes it a town, contented folk
        // start families: now and then, when two grown souls are
        // settled and fed, a child is born to the holding — one
        // more mouth, and joy all round. A cradle needs no spare
        // bed the way a migrant does; if the house is full, the
        // crowding sours tempers until more rooms stand.
        let parents = w
            .folk
            .iter()
            .filter(|f| f.trade != Trade::Child && (18..=50).contains(&f.age) && f.mood >= 2 && f.hunger == 0)
            .count();
        if w.stars >= FAMILY_STARS
            && w.raid == 0
            && parents >= 2
            && h.stores > 0
            && w.folk.len() < MAX_VILLAGERS
            && mix(seed_of(&w.key) ^ 0xB1275, h.visitor_next) % 4 == 0
        {
            let n = h.folk_next;
            h.folk_next += 1;
            let child = Villager {
                name: NAMES[(mix(seed_of(&w.key) ^ 0xB1275, n) % NAMES.len() as u64) as usize].to_string(),
                trade: Trade::Child,
                mood: 2,
                hunger: 0,
                skill: 0,
                practiced: 0,
                age: 0,
                seasons: 0,
            };
            said.push(format!("A child is born at {}: {}", name, child.name));
            for f in &mut w.folk {
                f.cheer(1);
            }
            w.folk.push(child);
        }
    }
    for line in &said {
        h.say(line.clone());
    }
    for line in &told {
        h.tell(line);
    }
    said
}

#[cfg(test)]
#[path = "../../tests/rules/together_villagers__tests.rs"]
mod tests;
