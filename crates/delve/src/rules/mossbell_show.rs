//! Mossbell's show: what lately happened at the tables, kept so the picture
//! can make a fuss of it (`render::overworld::arena::mossbell` draws it).
//!
//! The rules (`mossbell`) tell it what happened: a lamp lit, the keeper
//! struck or felled, the jackpot taken, the lead changing hands, the bell
//! tolling and the clock's last ten seconds. It answers with the calls to
//! shout (`Shout`) and keeps the juice kit's blows (`juice::Juice`) that say
//! how hard each moment hits. Nothing here changes a score or a move: it is
//! only memory.
//!
//! Time is the run's own tick count, so the host, a friend's mirror and a
//! browser guest draw the same moment. Every list is capped (`valid`).

use super::juice::{Blow, Juice, Weight};
use super::*;

pub const SHOUT_LIFE: u32 = 54;
pub const MOMENT_LIFE: u32 = 78;
const MOST_SHOUTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    Lit(u8),
    KeeperLow,
    KeeperDown,
    Jackpot,
    Cleared,
    Lead,
    Bell,
    Last,
}

impl Cry {
    pub fn rank(self) -> u8 {
        match self {
            Cry::Lit(n) => 1 + n.min(3),
            Cry::KeeperLow => 4,
            Cry::Lead => 5,
            Cry::Cleared => 6,
            Cry::KeeperDown => 7,
            Cry::Bell => 8,
            Cry::Last => 9,
            Cry::Jackpot => 10,
        }
    }
    pub fn big(self) -> bool {
        matches!(self, Cry::Jackpot | Cry::KeeperDown | Cry::Bell | Cry::Cleared)
    }
}

pub const POP_LIFE: u32 = 30;
const MOST_POPS: usize = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pop {
    pub who: u8,
    pub at: u8,
    pub gain: u16,
    pub born: u32,
}

impl Pop {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shout {
    pub cry: Cry,
    pub who: u8,
    pub born: u32,
}

impl Shout {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MossShow {
    #[serde(default, skip_serializing_if = "Juice::quiet")]
    pub juice: Juice,
    pub shouts: Vec<Shout>,
    pub pops: Vec<Pop>,
    pub lead: u8,
    pub ended: Option<u32>,
    pub record: bool,
    pub was: u16,
    called: u8,
}

const LAST: u8 = 1;

impl MossShow {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let seat = |id: u32| (1..=MAX_PLAYERS).contains(&id);
        self.juice.valid()
            && self.shouts.len() <= MOST_SHOUTS
            && self.pops.len() <= MOST_POPS
            && self.pops.iter().all(|p| seat(u32::from(p.who)) && p.at < 4)
            && self.shouts.iter().all(|s| s.who == 0 || seat(u32::from(s.who)))
            && u32::from(self.lead) <= MAX_PLAYERS
            && self.was <= 1000
    }

    pub fn tick(&mut self, now: u32) {
        self.juice.tick(now);
        self.shouts.retain(|s| s.age(now) < MOMENT_LIFE);
        self.pops.retain(|p| p.age(now) < POP_LIFE);
    }

    pub fn pop(&mut self, now: u32, who: u32, at: u8, gain: u32) {
        if self.pops.len() >= MOST_POPS {
            self.pops.remove(0);
        }
        self.pops.push(Pop { who: who as u8, at, gain: gain.min(u32::from(u16::MAX)) as u16, born: now });
    }

    pub fn shout(&mut self, now: u32, cry: Cry, who: u32) {
        if self.shouts.len() >= MOST_SHOUTS {
            self.shouts.remove(0);
        }
        self.shouts.push(Shout { cry, who: who as u8, born: now });
    }

    pub fn keeper(&mut self, now: u32, who: u32, left: u8, from: (f32, f32)) -> Weight {
        let weight = match left {
            0 => Weight::Kill,
            1 | 2 => Weight::Crushing,
            _ => Weight::Solid,
        };
        self.juice.blow(Blow { owner: who, target: who, x: 80.0, y: 27.0, from, lost: 1, weight, born: now });
        match left {
            0 => self.shout(now, Cry::KeeperDown, who),
            2 => self.shout(now, Cry::KeeperLow, who),
            _ => {}
        }
        weight
    }

    pub fn cleared(&mut self, now: u32, who: u32) -> Weight {
        self.juice.blow(Blow { owner: who, target: who, x: 80.0, y: 63.0, from: (80.0, 100.0), lost: 1, weight: Weight::Crushing, born: now });
        self.shout(now, Cry::Cleared, who);
        Weight::Crushing
    }

    pub fn jackpot(&mut self, now: u32, who: u32) -> Weight {
        self.juice.blow(Blow {
            owner: who,
            target: who,
            x: 80.0,
            y: 63.0,
            from: (80.0, 100.0),
            lost: 1,
            weight: Weight::Finale,
            born: now,
        });
        self.shout(now, Cry::Jackpot, who);
        Weight::Finale
    }

    pub fn standings(&mut self, now: u32, points: &[(u32, u32)]) {
        let top = points.iter().map(|&(_, p)| p).max().unwrap_or(0);
        let ahead: Vec<u32> = points.iter().filter(|&&(_, p)| p == top).map(|&(id, _)| id).collect();
        if ahead.len() != 1 || top < 100 {
            return;
        }
        let leader = ahead[0] as u8;
        if self.lead != 0 && self.lead != leader {
            self.shout(now, Cry::Lead, u32::from(leader));
        }
        self.lead = leader;
    }

    pub fn clock(&mut self, now: u32, age: u32, length: u32) {
        if super::mossbell::BELLS.iter().any(|&(from, _)| from == age) {
            self.shout(now, Cry::Bell, 0);
        }
        if self.called & LAST == 0 && age + 10 * HZ >= length {
            self.called |= LAST;
            self.shout(now, Cry::Last, 0);
        }
    }

    pub fn end(&mut self, now: u32, record: bool, was: u16) {
        self.ended = Some(now);
        self.record = record;
        self.was = was;
    }

    pub fn since_end(&self, now: u32) -> Option<u32> {
        self.ended.map(|e| now.saturating_sub(e))
    }
}
