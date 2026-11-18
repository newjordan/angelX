//! What the picture remembers of the race: the moments a driver should *feel*. The race's own rules
//! write a short record here when a whip cracks, a team is barged or wrecked, a lap is run, the
//! lead changes hands, a line is crossed or a photograph is called. The picture reads each record
//! by its age, so it is a pure function of the race and a friend's mirror draws what the host does.
//!
//! Records name teams by their `team` number, which stays with a chariot when a friend's screen
//! swaps it to the front of the field. Time is the race's own `clock`, which never resets.
use super::*;
use crate::rules::juice::Weight;

pub const LIFE: u32 = 90;
const MOST: usize = 64;
pub const PHOTO: f32 = 15.0;
const CLEAR: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Off,
    Whip,
    Iron,
    Barge,
    Wreck,
    Lead,
    Pass,
    Lap,
    Final,
    Fastest,
    Finish,
    Photo,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fx {
    pub kind: Kind,
    pub team: u8,
    pub other: u8,
    pub n: u16,
    pub at: [f32; 2],
    pub weight: Weight,
    pub born: u32,
}

impl Fx {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub fx: Vec<Fx>,
    pub leader: Option<u8>,
    ahead: Vec<(u8, u8)>,
    pub best_lap: u32,
}

impl Show {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self, now: u32) -> bool {
        self.fx.len() <= MOST
            && self.ahead.len() <= 64
            && self.fx.iter().all(|f| f.born <= now && f.at.iter().all(|v| v.is_finite() && v.abs() < 100_000.0))
    }

    pub fn put(&mut self, kind: Kind, team: u8, other: u8, n: u32, at: [f32; 2], weight: Weight, now: u32) {
        if self.fx.len() >= MOST {
            self.fx.remove(0);
        }
        self.fx.push(Fx { kind, team, other, n: n.min(u32::from(u16::MAX)) as u16, at, weight, born: now });
    }

    pub fn tick(&mut self, now: u32) {
        self.fx.retain(|f| f.age(now) < LIFE);
    }

    pub fn of(&self, kind: Kind, now: u32) -> impl Iterator<Item = &Fx> {
        self.fx.iter().filter(move |f| f.kind == kind && f.age(now) < LIFE)
    }

    pub fn since(&self, kind: Kind, team: u8, now: u32) -> Option<u32> {
        self.of(kind, now).filter(|f| f.team == team).map(|f| f.age(now)).min()
    }

    pub fn latest(&self, kind: Kind, now: u32) -> Option<u32> {
        self.of(kind, now).map(|f| f.age(now)).min()
    }

    pub fn held(&self, now: u32, holds: impl Fn(&Fx) -> bool) -> Option<Weight> {
        self.fx.iter().filter(|f| f.born == now && holds(f)).map(|f| f.weight).max()
    }

    fn set_ahead(&mut self, a: u8, b: u8) -> bool {
        let swapped = self.ahead.contains(&(b, a));
        self.ahead.retain(|&(x, y)| !((x, y) == (a, b) || (x, y) == (b, a)));
        self.ahead.push((a, b));
        swapped
    }

    pub fn watch(&mut self, racers: &[Racer], now: u32) {
        let placed = |r: &Racer| r.place.is_some();
        let front = racers
            .iter()
            .filter(|r| !placed(r))
            .max_by(|a, b| a.dist.partial_cmp(&b.dist).unwrap_or(std::cmp::Ordering::Equal));
        let current = self.leader.and_then(|l| racers.iter().find(|r| r.team == l && !placed(r)));
        match (current, front) {
            (None, Some(f)) if self.leader.is_some() || racers.iter().all(|r| r.dist > 1.0) => self.leader = Some(f.team),
            (Some(c), Some(f)) if f.team != c.team && f.dist > c.dist + CLEAR => {
                self.put(Kind::Lead, f.team, c.team, 0, [f.dist, f.lane], Weight::Graze, now);
                self.leader = Some(f.team);
            }
            _ => {}
        }
        for a in racers.iter().filter(|r| !placed(r)) {
            for b in racers.iter().filter(|r| !placed(r) && r.team != a.team) {
                if a.dist > b.dist + CLEAR {
                    let was_behind = self.set_ahead(a.team, b.team);
                    if was_behind && self.leader != Some(a.team) {
                        let place = 1 + racers.iter().filter(|o| o.dist > a.dist).count() as u32;
                        self.put(Kind::Pass, a.team, b.team, place, [a.dist, a.lane], Weight::Graze, now);
                    }
                }
            }
        }
    }
}
