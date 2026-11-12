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

/// Ticks a call stays up on the marquee.
pub const SHOUT_LIFE: u32 = 54;
/// Ticks a big moment (the jackpot, the keeper's fall, the bell) holds the field.
pub const MOMENT_LIFE: u32 = 78;
const MOST_SHOUTS: usize = 8;

/// What a banner calls. Words are the picture's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// Two or three of the table's lamps are lit.
    Lit(u8),
    /// The keeper has two pods left.
    KeeperLow,
    /// The keeper has fallen.
    KeeperDown,
    /// The jackpot was taken.
    Jackpot,
    /// A table cleared (keeper down, lamps lit) after the jackpot was already taken.
    Cleared,
    /// A knight took the lead from another.
    Lead,
    /// The bell tolled: bumpers and keeper pay double.
    Bell,
    /// The last ten seconds.
    Last,
}

impl Cry {
    /// How loudly it is shown: the banner with the higher rank stays on top.
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
    /// Whether the picture gives it the whole field rather than the marquee.
    pub fn big(self) -> bool {
        matches!(self, Cry::Jackpot | Cry::KeeperDown | Cry::Bell | Cry::Cleared)
    }
}

/// Ticks a score pop stays up.
pub const POP_LIFE: u32 = 30;
const MOST_POPS: usize = 18;

/// A score that popped off a bumper (`at` 0 to 2) or the keeper (3).
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

/// A banner called for a knight (`who` 0: for the whole room) at a tick.
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
    /// The keeper's blows and the jackpot, as the kit weighs them.
    #[serde(default, skip_serializing_if = "Juice::quiet")]
    pub juice: Juice,
    pub shouts: Vec<Shout>,
    pub pops: Vec<Pop>,
    /// The knight now ahead on their own (0 when level).
    pub lead: u8,
    /// The run tick the clock ran out.
    pub ended: Option<u32>,
    /// Whether the round beat the best score the hall held.
    pub record: bool,
    /// The best score the hall held when the round began.
    pub was: u16,
    /// Which of the clock's calls have been made.
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

    /// Let go what is old at `now`.
    pub fn tick(&mut self, now: u32) {
        self.juice.tick(now);
        self.shouts.retain(|s| s.age(now) < MOMENT_LIFE);
        self.pops.retain(|p| p.age(now) < POP_LIFE);
    }

    /// A score popped for `who` off a bumper (`at` 0 to 2) or the keeper (3).
    pub fn pop(&mut self, now: u32, who: u32, at: u8, gain: u32) {
        if self.pops.len() >= MOST_POPS {
            self.pops.remove(0);
        }
        self.pops.push(Pop { who: who as u8, at, gain: gain.min(u32::from(u16::MAX)) as u16, born: now });
    }

    /// Call a banner (the loudest stays on top when they come together).
    pub fn shout(&mut self, now: u32, cry: Cry, who: u32) {
        if self.shouts.len() >= MOST_SHOUTS {
            self.shouts.remove(0);
        }
        self.shouts.push(Shout { cry, who: who as u8, born: now });
    }

    /// The keeper took a hit at `at` from `from` (table pixels): how it
    /// weighed, and the calls it made. `left` is the pods now.
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

    /// `who` cleared their table, too late for the jackpot.
    pub fn cleared(&mut self, now: u32, who: u32) -> Weight {
        self.juice.blow(Blow { owner: who, target: who, x: 80.0, y: 63.0, from: (80.0, 100.0), lost: 1, weight: Weight::Crushing, born: now });
        self.shout(now, Cry::Cleared, who);
        Weight::Crushing
    }

    /// The jackpot fell to `who`.
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

    /// The points of every seat after a tick: a knight alone ahead (and past
    /// a hundred) who was not before has taken the lead from the last one.
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

    /// The clock's own calls: the bell tolls at the start of each window
    /// (`age`, the round's ticks gone) and the last ten seconds begin.
    pub fn clock(&mut self, now: u32, age: u32, length: u32) {
        if super::mossbell::BELLS.iter().any(|&(from, _)| from == age) {
            self.shout(now, Cry::Bell, 0);
        }
        if self.called & LAST == 0 && age + 10 * HZ >= length {
            self.called |= LAST;
            self.shout(now, Cry::Last, 0);
        }
    }

    /// The clock ran out at `now`.
    pub fn end(&mut self, now: u32, record: bool, was: u16) {
        self.ended = Some(now);
        self.record = record;
        self.was = was;
    }

    /// Ticks since the clock ran out, if it has.
    pub fn since_end(&self, now: u32) -> Option<u32> {
        self.ended.map(|e| now.saturating_sub(e))
    }
}
