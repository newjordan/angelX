//! Brasswake's show: what just happened in an aisle and at the table, kept so the picture can make
//! a fuss of it. Nothing here decides anything. The lane writes a mark when a blow lands, a slam
//! comes down, a cask breaks or a gate falls, and the session calls what the whole table should
//! hear (a new leader, a team kick, the keeper at the vat). Marks and calls are born on the run's
//! own tick, so the host, a friend's mirror and a browser guest draw the same pixels.
use crate::rules::MAX_PLAYERS;
use crate::rules::juice::Weight;
use serde::{Deserialize, Serialize};

/// Ticks a mark stays on the books for its effects (the longest is a flying score).
pub const MARK_LIFE: u32 = 48;
/// Ticks a call stays up.
pub const CALL_LIFE: u32 = 72;
/// A kick from two aisles this close together is a team kick.
pub const TEAM_TICKS: u32 = 10;
/// Calls of a kind wait this long before the next.
pub const TEAM_WAIT: u32 = 90;
const MOST_MARKS: usize = 18;
const MOST_CALLS: usize = 5;

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A blow landed on foe `foe`: swing 0 jab, 1 cross, 2 kick; `gain` is the work it paid.
    Blow { swing: u8, boss: bool, slain: bool, gain: u16, foe: u8 },
    /// A swing met nothing.
    Whiff { swing: u8 },
    /// A foe's slam or rush came down (`hit` if it found the knight).
    Quake { boss: bool, rush: bool, hit: bool },
    /// The knight took a blow, or held it off behind a guard.
    Hurt { guarded: bool, boss: bool },
    Fall,
    Rise,
    Grab,
    Throw,
    /// A thrown cask came down on the aisle; `hit` if the knight stood in it.
    Smash { hit: bool, from: u32 },
    Bread,
    /// The gate of aisle `wave` fell.
    Clear { wave: u8 },
    /// The knight's round was done: the keeper slain.
    Finish,
    /// The chain reached a round number.
    Streak { n: u16 },
    /// A step kicked up dust.
    Step,
    /// A cask this knight threw came down on aisle `to`.
    Landed { hit: bool, to: u32 },
}

impl Kind {
    /// Ticks a mark of this kind stays on the books.
    pub fn life(self) -> u32 {
        match self {
            Kind::Step => 14,
            _ => MARK_LIFE,
        }
    }
    /// How a mark weighs, for the kit's shake, freeze and flash.
    pub fn weight(self) -> Weight {
        match self {
            Kind::Blow { boss: true, slain: true, .. } => Weight::Finale,
            Kind::Blow { slain: true, .. } => Weight::Kill,
            Kind::Blow { swing: 2, .. } => Weight::Crushing,
            Kind::Blow { swing: 1, .. } => Weight::Solid,
            Kind::Blow { .. } => Weight::Graze,
            Kind::Quake { boss: true, hit: true, .. } => Weight::Crushing,
            Kind::Quake { boss: true, .. } | Kind::Quake { hit: true, .. } => Weight::Solid,
            Kind::Hurt { boss: true, guarded: false } => Weight::Crushing,
            Kind::Hurt { guarded: false, .. } => Weight::Solid,
            Kind::Smash { hit: true, .. } => Weight::Crushing,
            Kind::Smash { .. } | Kind::Fall => Weight::Solid,
            Kind::Finish => Weight::Finale,
            _ => Weight::Graze,
        }
    }
    /// Extra ticks the aisle holds still after a blow of this kind, on top of the knob.
    pub fn hold(self) -> u32 {
        match self.weight() {
            Weight::Graze | Weight::Solid => 0,
            Weight::Crushing => 1,
            Weight::Kill => 2,
            Weight::Finale => 4,
        }
    }
}

/// A thing that happened, where and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub kind: Kind,
    /// The aisle it happened in (a mark from an earlier aisle is not drawn on the next).
    pub wave: u8,
    /// The spot, in the aisle's own pixels.
    pub x: i32,
    pub y: i32,
    /// The way it went: 1 to the right, -1 to the left.
    pub dir: i8,
    pub born: u32,
}

impl Mark {
    /// Ticks since it happened at `now`.
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

/// One aisle's marks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    pub marks: Vec<Mark>,
}

impl Show {
    pub fn quiet(&self) -> bool {
        self.marks.is_empty()
    }
    pub fn valid(&self) -> bool {
        self.marks.len() <= MOST_MARKS
            && self.marks.iter().all(|m| (-512..=2048).contains(&m.x) && (-512..=512).contains(&m.y) && m.wave <= 3 && [-1, 0, 1].contains(&m.dir))
    }
    /// Let go what is old at `now`.
    pub fn tick(&mut self, now: u32) {
        self.marks.retain(|m| m.age(now) < m.kind.life());
    }
    pub fn mark(&mut self, kind: Kind, wave: u8, at: (i32, i32), dir: i32, now: u32) {
        if self.marks.len() >= MOST_MARKS {
            self.marks.remove(0);
        }
        self.marks.push(Mark { kind, wave, x: at.0, y: at.1, dir: dir.signum() as i8, born: now });
    }
    /// The newest mark that `pick` takes.
    pub fn latest<T>(&self, pick: impl Fn(&Mark) -> Option<T>) -> Option<T> {
        self.marks.iter().rev().find_map(pick)
    }
}

/// What the whole table hears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// A knight took the lead.
    Lead,
    /// A knight was first to the vat keeper.
    Keeper,
    /// Two aisles kicked together.
    Team,
    /// The keeper fell, and this knight was first to do it.
    Slain,
    /// A knight went down.
    Fell,
}

impl Cry {
    /// How loud: the call with the higher rank stays on top.
    pub fn rank(self) -> u8 {
        match self {
            Cry::Slain => 9,
            Cry::Keeper => 6,
            Cry::Lead => 5,
            Cry::Team => 4,
            Cry::Fell => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub cry: Cry,
    pub who: u32,
    pub born: u32,
}

impl Call {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

/// The table's calls and what they were judged against.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calls {
    pub list: Vec<Call>,
    /// Who is ahead now, if one knight is alone in front.
    pub leader: Option<u32>,
    /// The first to the keeper, and the first to bring it down.
    pub first_keeper: Option<u32>,
    pub first_slain: Option<u32>,
    /// The last tick each knight landed a kick, and the last team call.
    pub kicks: Vec<(u32, u32)>,
    pub team_at: Option<u32>,
}

impl Calls {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.list.len() <= MOST_CALLS
            && self.list.iter().all(|c| seat(&c.who))
            && self.leader.iter().all(seat)
            && self.first_keeper.iter().all(seat)
            && self.first_slain.iter().all(seat)
            && self.kicks.len() <= MAX_PLAYERS as usize
            && self.kicks.iter().all(|(id, _)| seat(id))
    }
    pub fn tick(&mut self, now: u32) {
        self.list.retain(|c| c.age(now) < CALL_LIFE);
    }
    pub fn call(&mut self, cry: Cry, who: u32, now: u32) {
        if self.list.len() >= MOST_CALLS {
            self.list.remove(0);
        }
        self.list.push(Call { cry, who, born: now });
    }
    /// A kick landed in `id`'s aisle at `now`; true when another aisle kicked within `TEAM_TICKS`
    /// and the table has not just heard a team kick.
    pub fn kicked(&mut self, id: u32, now: u32) -> bool {
        let together = self.kicks.iter().any(|&(who, at)| who != id && now.saturating_sub(at) <= TEAM_TICKS);
        match self.kicks.iter_mut().find(|(who, _)| *who == id) {
            Some(k) => k.1 = now,
            None => self.kicks.push((id, now)),
        }
        let rested = self.team_at.is_none_or(|at| now.saturating_sub(at) >= TEAM_WAIT);
        if together && rested {
            self.team_at = Some(now);
            self.call(Cry::Team, id, now);
            return true;
        }
        false
    }
}
