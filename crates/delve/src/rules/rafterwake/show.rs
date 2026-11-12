//! Rafterwake's show: what lately happened on a knight's tower and at the table, kept so the
//! picture can make a fuss of it. Nothing here decides a move or a score. A lane writes a mark
//! when boots leave a plank, land, catch a rung or fall, and when the keeper is struck or slams;
//! the session writes the table's calls: a new leader, the first knight at the keeper, the crew
//! bell, a knight who fell. Marks and calls are born on the run's own tick, so the host, a
//! friend's mirror and a browser guest draw the same pixels.
use crate::rules::MAX_PLAYERS;
use crate::rules::juice::Weight;
use serde::{Deserialize, Serialize};

/// Ticks a mark stays on the books for its effects.
pub const MARK_LIFE: u32 = 44;
/// Ticks a call stays up.
pub const CALL_LIFE: u32 = 72;
/// Calls of one kind from one knight wait this long.
pub const CALL_WAIT: u32 = 90;
const MOST_MARKS: usize = 16;
const MOST_CALLS: usize = 4;

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// Boots left a plank or a rung.
    Hop,
    /// Boots came down; `hard` after a fall of more than a deck and a half.
    Land { hard: bool },
    /// A hand took a rung.
    Catch,
    /// The knight's best deck rose to `deck`.
    Top { deck: u8 },
    /// The knight went down: `cause` 0 off the scaffold, 1 the keeper, 2 a brick.
    Fall { cause: u8 },
    /// The knight came back on its deck.
    Rise,
    /// The keeper took a hit; `left` is what it has left (0 when it is felled).
    Strike { left: u8 },
    /// The keeper's attack came down: a sweep or a drop.
    Slam { sweep: bool },
    /// A brick burst on the planks.
    Brick,
    /// The landing chain reached a round number.
    Chain { n: u16 },
}

impl Kind {
    /// Ticks a mark of this kind stays on the books.
    pub fn life(self) -> u32 {
        match self {
            Kind::Hop | Kind::Catch | Kind::Land { .. } | Kind::Chain { .. } => 26,
            _ => MARK_LIFE,
        }
    }
    /// How a mark weighs, for the kit's shake, freeze and flash.
    pub fn weight(self) -> Weight {
        match self {
            Kind::Strike { left: 0 } => Weight::Finale,
            Kind::Strike { left: 1 } => Weight::Kill,
            Kind::Strike { .. } => Weight::Crushing,
            Kind::Fall { cause: 1 } | Kind::Fall { cause: 2 } => Weight::Crushing,
            Kind::Fall { .. } | Kind::Slam { .. } | Kind::Land { hard: true } => Weight::Solid,
            _ => Weight::Graze,
        }
    }
}

/// A thing that happened, where and when (tower pixels, the feet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    /// The way the knight faced: 1 right, -1 left.
    pub dir: i8,
    pub born: u32,
}

impl Mark {
    /// Ticks since it happened at `now`.
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

/// One tower's marks.
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
            && self.marks.iter().all(|m| {
                (-64..=320).contains(&m.x) && (-128..=700).contains(&m.y) && [-1, 1].contains(&m.dir)
            })
    }
    /// Let go what is old at `now`.
    pub fn tick(&mut self, now: u32) {
        self.marks.retain(|m| m.age(now) < m.kind.life());
    }
    pub fn mark(&mut self, kind: Kind, at: (i32, i32), dir: i32, now: u32) {
        if self.marks.len() >= MOST_MARKS {
            self.marks.remove(0);
        }
        self.marks.push(Mark {
            kind,
            x: at.0,
            y: at.1,
            dir: if dir < 0 { -1 } else { 1 },
            born: now,
        });
    }
    /// The newest mark that `pick` takes.
    pub fn latest<T>(&self, pick: impl Fn(&Mark) -> Option<T>) -> Option<T> {
        self.marks.iter().rev().find_map(pick)
    }
    /// The heaviest weight of the marks born at `now`.
    pub fn heaviest(&self, now: u32) -> Option<Weight> {
        self.marks.iter().filter(|m| m.born == now).map(|m| m.kind.weight()).max()
    }
}

/// What the whole table hears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// A knight took the lead.
    Lead,
    /// A knight was first to the keeper's deck.
    Keeper,
    /// A knight brought the keeper down.
    Slain,
    /// The crew bell rang for the `n`th time.
    Bell(u8),
    /// A knight went down.
    Fell,
}

impl Cry {
    /// How loud: the call with the higher rank stays on top.
    pub fn rank(self) -> u8 {
        match self {
            Cry::Slain => 9,
            Cry::Bell(_) => 8,
            Cry::Keeper => 6,
            Cry::Lead => 5,
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
    /// Who is ahead now, if one knight is alone in front (and has climbed).
    pub leader: Option<u32>,
    /// The first knight on the keeper's deck, and the first to bring it down.
    pub first_keeper: Option<u32>,
    pub first_slain: Option<u32>,
    /// The tick of the last lead call.
    pub lead_at: Option<u32>,
    /// The last tick each knight's fall was called.
    pub fells: Vec<(u32, u32)>,
}

impl Calls {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.list.len() <= MOST_CALLS
            && self.list.iter().all(|c| seat(&c.who) || matches!(c.cry, Cry::Bell(_)))
            && self.list.iter().all(|c| !matches!(c.cry, Cry::Bell(n) if n == 0 || n > 3))
            && self.leader.iter().all(seat)
            && self.first_keeper.iter().all(seat)
            && self.first_slain.iter().all(seat)
            && self.fells.len() <= MAX_PLAYERS as usize
            && self.fells.iter().all(|(id, _)| seat(id))
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
    /// A fall of `id` at `now`, called if it has been a while since the last.
    pub fn fell(&mut self, id: u32, now: u32) {
        let due = match self.fells.iter_mut().find(|(who, _)| *who == id) {
            Some(at) => {
                let rested = now.saturating_sub(at.1) >= CALL_WAIT * 2;
                if rested {
                    at.1 = now;
                }
                rested
            }
            None => {
                self.fells.push((id, now));
                true
            }
        };
        if due {
            self.call(Cry::Fell, id, now);
        }
    }
    /// The standings after a tick, `(id, score)`: a knight alone ahead who was not before has
    /// taken the lead, if the margin is a deck's worth and the last call has settled.
    pub fn standings(&mut self, now: u32, points: &[(u32, u32)]) {
        let mut sorted = points.to_vec();
        sorted.sort_by_key(|&(id, p)| (std::cmp::Reverse(p), id));
        let [(top, a), rest @ ..] = sorted.as_slice() else { return };
        let b = rest.first().map_or(0, |&(_, p)| p);
        if points.len() < 2 || *a < 80 || a.saturating_sub(b) < 24 {
            return;
        }
        if self.leader.is_some_and(|l| l != *top)
            && self.lead_at.is_none_or(|at| now.saturating_sub(at) >= CALL_WAIT)
        {
            self.call(Cry::Lead, *top, now);
            self.lead_at = Some(now);
        }
        self.leader = Some(*top);
    }
}
