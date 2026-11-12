//! Latch Duel's show: what just happened at a bay and at the table, kept so the picture can make a
//! fuss of it. Nothing here decides anything. A counter or a miss writes a mark on the clock of the
//! round (the ticks since the bell, `Session::elapsed`), and the table's calls (a new leader, match
//! point, the whole table countering the same attack) are written beside them. Marks and calls are
//! born on that clock, so the host, a friend's mirror and a browser guest draw the same pixels.
use super::MAX_PLAYERS;
use super::juice::Weight;
use serde::{Deserialize, Serialize};

/// Ticks the longest mark stays on the books.
pub const MARK_LIFE: u32 = 64;
/// Ticks a call stays up.
pub const CALL_LIFE: u32 = 60;
/// A counter this many ticks (or fewer) before the arm lands is perfect.
pub const PERFECT: u8 = 2;
const MOST_MARKS: usize = 24;
const MOST_CALLS: usize = 6;

/// Why a guard folded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Why {
    /// F came before the closing beat.
    Early,
    /// F came at the other height.
    Wrong,
    /// The arm landed and F never came.
    Slow,
}

/// What happened at a bay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A clean counter: the link of the chain it made, the counters the knight has now, how many
    /// ticks before the arm landed it came, and which attack of the deck it answered.
    Counter { chain: u8, count: u8, error: u8, attack: u32 },
    /// A fold of the guard, and the chain of counters it cost the knight.
    Miss { why: Why, lost: u8 },
}

impl Kind {
    /// Ticks a mark of this kind stays on the books.
    pub fn life(self) -> u32 {
        match self {
            Kind::Counter { count: 3, .. } => MARK_LIFE,
            Kind::Counter { .. } => 46,
            Kind::Miss { .. } => 34,
        }
    }
    /// How a mark weighs, for the kit's shake and flash: the third counter is the finale.
    pub fn weight(self) -> Weight {
        match self {
            Kind::Counter { count: 3, .. } => Weight::Finale,
            Kind::Counter { chain, error, .. } if chain >= 2 || error <= PERFECT => Weight::Crushing,
            Kind::Counter { .. } => Weight::Solid,
            Kind::Miss { .. } => Weight::Graze,
        }
    }
}

/// A thing that happened to a knight, and when (the round's own clock).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub who: u32,
    pub kind: Kind,
    pub born: u32,
}

impl Mark {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

/// What the whole table hears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// A knight took the lead.
    Lead,
    /// A knight is one counter from Clasp's end.
    Point,
    /// The whole table has countered this many attacks in a row, together.
    Sync(u8),
}

impl Cry {
    /// How loud: the call with the higher rank stays on top.
    pub fn rank(self) -> u8 {
        match self {
            Cry::Point => 6,
            Cry::Sync(n) => 3 + n.min(3),
            Cry::Lead => 4,
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

/// The table's show: every bay's marks, the calls, and what they were judged against.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    pub marks: Vec<Mark>,
    pub calls: Vec<Call>,
    /// Who is alone in front now.
    pub leader: Option<u32>,
    /// Knights whose match point has been called.
    pub pointed: Vec<u32>,
    /// The attack the table is answering, and the knights who have countered it.
    pub attack: u32,
    pub answered: Vec<u32>,
    /// Attacks in a row the whole table countered.
    pub streak: u8,
}

impl Show {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.marks.len() <= MOST_MARKS
            && self.calls.len() <= MOST_CALLS
            && self.marks.iter().all(|m| {
                seat(&m.who)
                    && match m.kind {
                        Kind::Counter { chain, count, error, .. } => (1..=3).contains(&chain) && (1..=3).contains(&count) && chain <= count && error <= 60,
                        Kind::Miss { lost, .. } => lost <= 3,
                    }
            })
            && self.calls.iter().all(|c| seat(&c.who))
            && self.leader.iter().all(seat)
            && self.pointed.len() <= MAX_PLAYERS as usize
            && self.pointed.iter().all(seat)
            && self.answered.len() <= MAX_PLAYERS as usize
            && self.answered.iter().all(seat)
            && self.streak <= 90
    }
    /// Let go what is old at `now`.
    pub fn tick(&mut self, now: u32) {
        self.marks.retain(|m| m.age(now) < m.kind.life());
        self.calls.retain(|c| c.age(now) < CALL_LIFE);
    }
    fn mark(&mut self, who: u32, kind: Kind, now: u32) {
        if self.marks.len() >= MOST_MARKS {
            self.marks.remove(0);
        }
        self.marks.push(Mark { who, kind, born: now });
    }
    fn call(&mut self, cry: Cry, who: u32, now: u32) {
        if self.calls.len() >= MOST_CALLS {
            self.calls.remove(0);
        }
        self.calls.push(Call { cry, who, born: now });
    }
    /// Knight `who` countered attack `attack` of the deck, `error` ticks before it landed, and
    /// has `count` counters and a chain of `chain` now; `seats` are the knights at the table.
    pub fn counter(&mut self, who: u32, chain: u8, count: u8, error: u32, attack: u32, now: u32, seats: &[u32]) {
        // A late press that saves a tell the clock had already folded takes the fold back.
        self.marks.retain(|m| !(m.who == who && matches!(m.kind, Kind::Miss { .. }) && m.age(now) < 12));
        self.mark(who, Kind::Counter { chain, count, error: error.min(60) as u8, attack }, now);
        // The whole table, together: every seat answering the same attack.
        if attack != self.attack {
            if self.answered.len() < seats.len() {
                self.streak = 0;
            }
            self.attack = attack;
            self.answered.clear();
        }
        if !self.answered.contains(&who) {
            self.answered.push(who);
            if seats.len() >= 2 && seats.iter().all(|s| self.answered.contains(s)) {
                self.streak = self.streak.saturating_add(1);
                if self.streak >= 2 {
                    self.call(Cry::Sync(self.streak.min(9)), who, now);
                }
            }
        }
    }
    /// Knight `who`'s guard folded.
    pub fn miss(&mut self, who: u32, why: Why, lost: u8, now: u32) {
        self.mark(who, Kind::Miss { why, lost: lost.min(3) }, now);
    }
    /// Everyone's counters now: the lead changes hands, a knight reaches match point.
    pub fn standings(&mut self, now: u32, scores: &[(u32, u8)]) {
        let top = scores.iter().map(|&(_, c)| c).max().unwrap_or(0);
        let ahead: Vec<u32> = scores.iter().filter(|&&(_, c)| c == top && top >= 1).map(|&(id, _)| id).collect();
        if scores.len() > 1 && ahead.len() == 1 {
            if self.leader != Some(ahead[0]) {
                self.call(Cry::Lead, ahead[0], now);
            }
            self.leader = Some(ahead[0]);
        }
        for &(id, count) in scores {
            if count == 2 && !self.pointed.contains(&id) {
                self.pointed.push(id);
                self.call(Cry::Point, id, now);
            }
        }
    }
    /// The heaviest mark born on this very tick.
    pub fn heaviest(&self, now: u32) -> Option<Weight> {
        self.marks.iter().filter(|m| m.born == now).map(|m| m.kind.weight()).max()
    }
}
