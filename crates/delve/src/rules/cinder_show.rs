//! Cinder Stack's show: what just happened on a board and at the table, kept so the picture can make
//! a fuss of it. Nothing here decides anything. A board writes a mark when a pair comes to rest,
//! pots pop, slag comes down, the kiln cracks a stage or coughs, and the session calls what the
//! whole table should hear (a new leader, a big chain, match point). Marks are born on the board's
//! own tick count and calls on the round's, so the host, a friend's mirror and a browser guest
//! draw the same pixels.
use super::MAX_PLAYERS;
use super::cinder::{COLS, CRACKS, ROWS};
use super::juice::Weight;
use serde::{Deserialize, Serialize};

pub const MARK_LIFE: u32 = 64;
pub const CALL_LIFE: u32 = 66;
pub const LEAD_MIN: u16 = 6;
pub const POINT: u16 = 30;
pub const CLOSE_AT: u16 = 27;
pub const CLOSE_BY: u16 = 3;
const MOST_MARKS: usize = 12;
const MOST_CALLS: usize = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cells {
    pub lo: u32,
    pub hi: u32,
}

impl Cells {
    pub fn set(&mut self, x: usize, y: usize) {
        let i = y * COLS + x;
        if i < 30 {
            self.lo |= 1 << i;
        } else {
            self.hi |= 1 << (i - 30);
        }
    }
    pub fn has(&self, x: usize, y: usize) -> bool {
        let i = y * COLS + x;
        if i < 30 { self.lo >> i & 1 == 1 } else { self.hi >> (i - 30) & 1 == 1 }
    }
    pub fn count(&self) -> u32 {
        self.lo.count_ones() + self.hi.count_ones()
    }
    fn valid(&self) -> bool {
        self.lo < 1 << 30 && self.hi < 1 << 30
    }
    pub fn each(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..ROWS).flat_map(|y| (0..COLS).map(move |x| (x, y))).filter(|&(x, y)| self.has(x, y))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Land { a: (u8, u8), b: (u8, u8), hard: bool, from: u8 },
    Pop { by: [Cells; 4], chain: u8, gain: u8 },
    Slag { col: u8, row: u8, hit: bool, near: bool },
    Stage { n: u8 },
    Cough,
}

impl Kind {
    pub fn life(self) -> u32 {
        match self {
            Kind::Land { .. } => 16,
            Kind::Pop { .. } => 44,
            Kind::Slag { .. } => 30,
            Kind::Stage { .. } => MARK_LIFE,
            Kind::Cough => 40,
        }
    }
    pub fn weight(self) -> Weight {
        match self {
            Kind::Land { .. } => Weight::Graze,
            Kind::Pop { chain, gain, .. } => match (gain, chain) {
                (_, 4..) | (24.., _) => Weight::Kill,
                (_, 3) | (12.., _) => Weight::Crushing,
                (6.., _) | (_, 2) => Weight::Solid,
                _ => Weight::Graze,
            },
            Kind::Slag { hit: true, .. } => Weight::Crushing,
            Kind::Slag { .. } => Weight::Solid,
            Kind::Stage { n: 3 } => Weight::Finale,
            Kind::Stage { .. } => Weight::Kill,
            Kind::Cough => Weight::Crushing,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub kind: Kind,
    pub born: u32,
}

impl Mark {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    pub marks: Vec<Mark>,
}

impl Show {
    pub fn quiet(&self) -> bool {
        self.marks.is_empty()
    }
    pub fn valid(&self) -> bool {
        let cell = |&(x, y): &(u8, u8)| usize::from(x) < COLS && usize::from(y) < ROWS;
        self.marks.len() <= MOST_MARKS
            && self.marks.iter().all(|m| match m.kind {
                Kind::Land { a, b, from, .. } => cell(&a) && cell(&b) && usize::from(from) < ROWS,
                Kind::Pop { by, chain, gain } => by.iter().all(Cells::valid) && (1..=20).contains(&chain) && gain <= CRACKS as u8,
                Kind::Slag { col, row, .. } => usize::from(col) < COLS && usize::from(row) < ROWS,
                Kind::Stage { n } => (1..=3).contains(&n),
                Kind::Cough => true,
            })
    }
    pub fn tick(&mut self, now: u32) {
        self.marks.retain(|m| m.age(now) < m.kind.life());
    }
    pub fn mark(&mut self, kind: Kind, now: u32) {
        if self.marks.len() >= MOST_MARKS {
            self.marks.remove(0);
        }
        self.marks.push(Mark { kind, born: now });
    }
    pub fn fresh(&self, now: u32) -> impl Iterator<Item = &Mark> {
        self.marks.iter().filter(move |m| m.born == now)
    }
    pub fn heaviest(&self, now: u32) -> Option<Weight> {
        self.fresh(now).map(|m| m.kind.weight()).max()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    Lead,
    Chain(u8),
    Stage(u8),
    Point,
    Cough,
    Cracked,
    Close,
}

impl Cry {
    pub fn rank(self) -> u8 {
        match self {
            Cry::Cracked => 9,
            Cry::Point => 7,
            Cry::Stage(n) => 5 + n,
            Cry::Chain(n) => 3 + n.min(6),
            Cry::Lead => 4,
            Cry::Close => 6,
            Cry::Cough => 2,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calls {
    pub list: Vec<Call>,
    pub leader: Option<u32>,
    pub pointed: Vec<u32>,
    #[serde(default)]
    pub close: bool,
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
            && self.pointed.len() <= MAX_PLAYERS as usize
            && self.pointed.iter().all(seat)
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
    pub fn report<'a>(&mut self, who: u32, fresh: impl Iterator<Item = &'a Mark>, now: u32) {
        for m in fresh {
            match m.kind {
                Kind::Pop { chain, .. } if chain >= 3 => self.call(Cry::Chain(chain), who, now),
                Kind::Stage { n: 3 } => {}
                Kind::Stage { n } => self.call(Cry::Stage(n), who, now),
                Kind::Cough => self.call(Cry::Cough, who, now),
                _ => {}
            }
        }
    }
    pub fn standings(&mut self, now: u32, scores: &[(u32, u16)]) {
        let top = scores.iter().map(|&(_, c)| c).max().unwrap_or(0);
        let ahead: Vec<u32> = scores.iter().filter(|&&(_, c)| c == top && top >= LEAD_MIN).map(|&(id, _)| id).collect();
        if scores.len() > 1 && ahead.len() == 1 {
            if self.leader != Some(ahead[0]) {
                self.call(Cry::Lead, ahead[0], now);
            }
            self.leader = Some(ahead[0]);
        }
        for &(id, cracks) in scores {
            if cracks >= POINT && cracks < CRACKS && !self.pointed.contains(&id) {
                self.pointed.push(id);
                self.call(Cry::Point, id, now);
            }
        }
        if scores.len() > 1 && !self.close {
            let mut order: Vec<(u16, u32)> = scores.iter().map(|&(id, c)| (c, id)).collect();
            order.sort_by(|a, b| b.cmp(a));
            let (first, second) = (order[0], order[1]);
            if first.0 >= CLOSE_AT && first.0 < CRACKS && first.0 - second.0 <= CLOSE_BY {
                self.close = true;
                self.call(Cry::Close, first.1, now);
            }
        }
    }
}
