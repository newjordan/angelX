//! Bounded floor economy: terrain-backed support, one deployment per camp and
//! one succession per real leader death. No timers, world simulation or NPC war.
use super::{Pack, boss_gates::Leader};
use serde::{Deserialize, Serialize};

pub(super) const ENTITY_CAP: usize = 96;
pub(super) const CAMP_CAP: usize = 64;
pub(super) const ROSTER_CAP: usize = 24;
pub(super) const SPAWN_CAP: u16 = 128;
pub(super) const EVENT_CAP: usize = 25 * CAMP_CAP + 4 + 2 * SPAWN_CAP as usize;

/// Shared floor-local provision for extra bodies, independent of producer lifetimes.
pub(super) fn spawn_allowance(support: u16) -> u16 {
    (8 + support * 4).min(SPAWN_CAP)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Structure {
    Hive,
    Clans,
    Stronghold,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Resource {
    Brood,
    Ore,
    Relics,
    Fuel,
    Salvage,
    EchoBrood,
}
impl Resource {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Brood => "brood beds",
            Self::Ore => "ore seams",
            Self::Relics => "tomb tribute",
            Self::Fuel => "forge fuel",
            Self::Salvage => "salvage stacks",
            Self::EchoBrood => "echo brood",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Site {
    pub room: usize,
    pub wet: u16,
    pub works: u16,
    pub open: u16,
    pub capacity: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
    pub structure: Structure,
    pub resource: Resource,
    pub pack: Pack,
    pub sites: Vec<Site>,
    pub support: u16,
    pub salt: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Faction {
    pub name: String,
    pub controller: u8,
    pub initial_reserve: u8,
    pub reserve: u8,
    pub controlled: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Camp {
    pub room: usize,
    pub faction: u8,
    pub rival: Option<u8>,
    pub capacity: u8,
    pub base_budget: u8,
    pub budget: u8,
    pub deployed: bool,
    pub roster: Vec<super::EnemyKind>,
    pub remaining: u32,
    pub rival_present: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Event {
    Deploy(usize),
    Loss(u8),
    Emit {
        room: usize,
        id: u32,
        kind: super::EnemyKind,
        stage: u8,
    },
    Retire(u32),
    Casualty {
        camp: usize,
        slot: u8,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Economy {
    pub profile: Profile,
    pub factions: Vec<Faction>,
    pub camps: Vec<Camp>,
    pub events: Vec<Event>,
    pub spawn_allowance: u16,
    pub spawn_spent: u16,
}

impl Economy {
    pub fn camp(&self, room: usize) -> Option<&Camp> {
        self.camps.iter().find(|c| c.room == room)
    }
    pub fn same_identity(&self, other: &Self) -> bool {
        self.spawn_allowance == other.spawn_allowance
            && self.profile == other.profile
            && self.factions.len() == other.factions.len()
            && self.camps.len() == other.camps.len()
            && self.factions.iter().zip(&other.factions).all(|(a, b)| {
                (a.name.as_str(), a.controller, a.initial_reserve)
                    == (b.name.as_str(), b.controller, b.initial_reserve)
            })
            && self.camps.iter().zip(&other.camps).all(|(a, b)| {
                (a.room, a.faction, a.rival, a.capacity, a.base_budget)
                    == (b.room, b.faction, b.rival, b.capacity, b.base_budget)
            })
    }
    fn transition(&mut self, event: &Event) -> bool {
        match *event {
            Event::Emit {
                room,
                id,
                kind,
                stage,
            } => {
                if room >= CAMP_CAP
                    || id == 0
                    || self.spawn_spent >= self.spawn_allowance
                    || !Self::emission_kind(kind, stage)
                    || self
                        .events
                        .iter()
                        .any(|e| matches!(e, Event::Emit { id: prior, .. } if *prior >= id))
                {
                    return false;
                }
                self.spawn_spent += 1;
            }
            Event::Retire(id) => {
                if !self.emission(id).is_some_and(|(_, _, _, alive)| alive) {
                    return false;
                }
            }
            Event::Deploy(i) => {
                let Some(c) = self.camps.get_mut(i).filter(|c| !c.deployed) else {
                    return false;
                };
                let f = &mut self.factions[usize::from(c.faction)];
                let bonus = if f.controlled { f.reserve.min(2) } else { 0 };
                f.reserve -= bonus;
                c.budget = (c.budget + bonus).min(24);
                c.deployed = true;
                self.camps[i].rival_present = self.camps[i]
                    .rival
                    .is_some_and(|r| self.factions[usize::from(r)].controlled);
                let roster = self.make_roster(i);
                self.camps[i].remaining = (1u32 << roster.len()) - 1;
                self.camps[i].roster = roster;
            }
            Event::Casualty { camp, slot } => {
                let Some(c) = self
                    .camps
                    .get_mut(camp)
                    .filter(|c| c.deployed && slot < 24 && usize::from(slot) < c.roster.len())
                else {
                    return false;
                };
                if c.remaining & (1u32 << slot) == 0 {
                    return false;
                }
                c.remaining &= !(1u32 << slot);
            }
            Event::Loss(leader) => {
                let Some(i) = self
                    .factions
                    .iter()
                    .position(|f| f.controller == leader && f.controlled)
                else {
                    return false;
                };
                self.factions[i].controlled = false;
                let reserve = std::mem::take(&mut self.factions[i].reserve);
                // No living controller, no provisioning of future camps. Never touch an active camp.
                for c in self
                    .camps
                    .iter_mut()
                    .filter(|c| usize::from(c.faction) == i && !c.deployed)
                {
                    c.budget = c
                        .budget
                        .saturating_sub(if self.profile.structure == Structure::Hive {
                            6
                        } else {
                            2
                        })
                        .max(2);
                }
                if self.profile.structure == Structure::Clans {
                    if let Some(j) = (1..self.factions.len())
                        .map(|n| (i + n) % self.factions.len())
                        .find(|&j| self.factions[j].controlled)
                    {
                        self.factions[j].reserve =
                            (self.factions[j].reserve + reserve.min(3)).min(9);
                    }
                } else {
                    // Tribute supply loss weakens the future apex escort as well.
                    if let Some(c) = self
                        .camps
                        .last_mut()
                        .filter(|c| !c.deployed && usize::from(c.faction) != i)
                    {
                        c.budget = c.budget.saturating_sub(2).max(2);
                    }
                }
            }
        }
        true
    }
    pub fn deploy(&mut self, room: usize) -> Option<Vec<(u8, super::EnemyKind, u8)>> {
        let i = self.camps.iter().position(|c| c.room == room)?;
        if !self.camps[i].deployed {
            let event = Event::Deploy(i);
            if self.transition(&event) {
                self.events.push(event);
            }
        }
        Some(
            self.camps[i]
                .roster
                .iter()
                .enumerate()
                .filter(|(slot, _)| self.camps[i].remaining & (1u32 << slot) != 0)
                .map(|(slot, &kind)| {
                    (
                        slot as u8,
                        kind,
                        self.slot_faction(room, slot as u8).unwrap(),
                    )
                })
                .collect(),
        )
    }
    pub fn slot_faction(&self, room: usize, slot: u8) -> Option<u8> {
        let c = self.camp(room)?;
        Some(if slot == 1 && c.rival_present {
            c.rival?
        } else {
            c.faction
        })
    }
    pub fn alive_slot(&self, room: usize, slot: u8, kind: super::EnemyKind) -> bool {
        self.camp(room).is_some_and(|c| {
            slot < 24
                && c.roster.get(usize::from(slot)) == Some(&kind)
                && c.remaining & (1u32 << slot) != 0
        })
    }
    pub fn casualty(&mut self, room: usize, slot: u8) {
        if let Some(camp) = self.camps.iter().position(|c| c.room == room) {
            let event = Event::Casualty { camp, slot };
            if self.transition(&event) {
                self.events.push(event);
            }
        }
    }
    pub fn lose(&mut self, leader: u8) {
        let event = Event::Loss(leader);
        if self.transition(&event) {
            self.events.push(event);
        }
    }
    fn make_roster(&self, i: usize) -> Vec<super::EnemyKind> {
        use super::EnemyKind::*;
        let camp = &self.camps[i];
        let pool: &[super::EnemyKind] = match self.profile.pack {
            Pack::Fungal => &[Bat, Imp, Bat, Slime],
            Pack::Unknown => &[Imp, Wraith, Bat],
            Pack::Cavern => &[Imp, Bat, Skeleton],
            Pack::Archive => &[Skeleton, Wraith, Bat],
            Pack::Crypt => &[Skeleton, Wraith, Bat],
            Pack::Hellforge => &[Imp, Wraith, Demon],
        };
        let mut roster: Vec<_> = (0..camp.budget)
            .map(|n| pool[(usize::from(n) + usize::from(camp.faction)) % pool.len()])
            .collect();
        // Rival presence is composition, not NPC-on-NPC warfare. Dangerous kinds stay bounded.
        if self.profile.structure == Structure::Clans {
            roster[0] = if camp.faction.is_multiple_of(2) {
                super::EnemyKind::Sapper
            } else {
                super::EnemyKind::Hob
            };
            if let Some(rival) = camp.rival.filter(|_| camp.rival_present) {
                roster[1] = if rival % 2 == 0 { Sapper } else { Hob };
            }
        }
        // A hive has only one splitting slime per deployment, not a multiplication swarm.
        let mut slime = false;
        for k in &mut roster {
            if *k == Slime {
                if slime {
                    *k = Bat;
                }
                slime = true;
            }
        }
        roster
    }
    pub(super) fn emission_kind(kind: super::EnemyKind, stage: u8) -> bool {
        use super::EnemyKind::*;
        !matches!(kind, Boss | Dragon | PitTyrant | Dummy)
            && if kind == Slime {
                stage <= 2
            } else {
                stage == 0
            }
    }
    /// All mutable containers are bounded before cloning, replay or indexing.
    pub fn within_bounds(&self) -> bool {
        self.profile.sites.len() <= CAMP_CAP
            && !self.factions.is_empty()
            && self.factions.len() <= 4
            && self.camps.len() <= CAMP_CAP
            && self.events.len() <= EVENT_CAP
            && self.events.len() <= 25 * self.camps.len() + 4 + 2 * SPAWN_CAP as usize
            && self.spawn_allowance <= SPAWN_CAP
            && self.spawn_spent <= self.spawn_allowance
            && self
                .profile
                .sites
                .iter()
                .all(|s| s.room < CAMP_CAP && s.capacity <= 6)
            && self.factions.iter().all(|f| {
                f.name.len() <= 64 && f.controller < 4 && f.initial_reserve <= 6 && f.reserve <= 9
            })
            && self.camps.iter().all(|c| {
                c.room < CAMP_CAP
                    && c.roster.len() <= ROSTER_CAP
                    && usize::from(c.faction) < self.factions.len()
                    && c.rival.is_none_or(|r| usize::from(r) < self.factions.len())
                    && c.capacity <= 6
                    && (2..=24).contains(&c.base_budget)
                    && (2..=24).contains(&c.budget)
            })
    }
    pub fn can_emit(&self) -> bool {
        self.spawn_spent < self.spawn_allowance && self.events.len() < EVENT_CAP
    }
    pub fn emit(&mut self, room: usize, id: u32, kind: super::EnemyKind, stage: u8) {
        let event = Event::Emit {
            room,
            id,
            kind,
            stage,
        };
        assert!(
            self.transition(&event),
            "successful emission must have provision"
        );
        self.events.push(event);
    }
    pub fn emission(&self, id: u32) -> Option<(usize, super::EnemyKind, u8, bool)> {
        self.events.iter().find_map(|event| match *event {
            Event::Emit {
                room,
                id: which,
                kind,
                stage,
            } if which == id => {
                Some((room, kind, stage, !self.events.contains(&Event::Retire(id))))
            }
            _ => None,
        })
    }
    pub fn alive_emission(&self, room: usize, id: u32, kind: super::EnemyKind, stage: u8) -> bool {
        self.emission(id).is_some_and(|(r, k, s, alive)| {
            alive && room == r && kind == k && (kind != super::EnemyKind::Slime || stage == s)
        })
    }
    pub fn retire(&mut self, id: u32) {
        let event = Event::Retire(id);
        if self.transition(&event) {
            self.events.push(event);
        }
    }
    /// Replay from independently reconstructed, bounded immutable genesis, never
    /// clone a received mutable roster/journal only to discard it.
    pub fn valid_progress(&self, genesis: &Self, leaders: &[Leader]) -> bool {
        if !self.within_bounds()
            || !genesis.within_bounds()
            || leaders.len() > 4
            || !self.same_identity(genesis)
            || self.events.len()
                > 25 * self.camps.len() + leaders.len() + 2 * usize::from(self.spawn_allowance)
        {
            return false;
        }
        let mut fresh = genesis.clone();
        for event in &self.events {
            if let Event::Loss(which) = *event {
                let Some(leader) = leaders.get(usize::from(which)) else {
                    return false;
                };
                if !fresh.camp(leader.room).is_some_and(|c| c.deployed) {
                    return false;
                }
            }
            if !fresh.transition(event) {
                return false;
            }
            fresh.events.push(event.clone());
        }
        fresh == *self
            && leaders
                .iter()
                .enumerate()
                .all(|(i, l)| l.defeated == self.events.contains(&Event::Loss(i as u8)))
    }
    pub fn line(&self, at: usize) -> String {
        let lost = self.factions.iter().filter(|f| !f.controlled).count();
        let location = self
            .camp(at)
            .map(|c| {
                let f = &self.factions[usize::from(c.faction)];
                let rival = c
                    .rival
                    .filter(|&r| self.factions[usize::from(r)].controlled)
                    .map(|r| format!("; rival {}", self.factions[usize::from(r)].name))
                    .unwrap_or_default();
                format!(
                    " | {}{}: support {} reserve {}{}",
                    f.name,
                    if f.controlled { "" } else { " cut" },
                    c.budget,
                    f.reserve,
                    rival
                )
            })
            .unwrap_or_default();
        format!(
            "{} {} | control lost {}/{}{} | adds {}/{}",
            self.profile.resource.name(),
            self.profile.support,
            lost,
            self.factions.len(),
            location,
            self.spawn_allowance - self.spawn_spent,
            self.spawn_allowance
        )
    }
}
