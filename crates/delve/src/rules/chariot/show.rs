//! What the picture remembers of the race: the moments a driver should *feel*. The race's own rules
//! write a short record here when a whip cracks, a team is barged or wrecked, a lap is run, the
//! lead changes hands, a line is crossed or a photograph is called. The picture reads each record
//! by its age, so it is a pure function of the race and a friend's mirror draws what the host does.
//!
//! Records name teams by their `team` number, which stays with a chariot when a friend's screen
//! swaps it to the front of the field. Time is the race's own `clock`, which never resets.
use super::*;
use crate::rules::juice::Weight;

/// The longest any record is worth showing, ticks.
pub const LIFE: u32 = 90;
/// The most records kept: the oldest go first.
const MOST: usize = 64;
/// Two teams home within this many ticks of each other ran a photo finish.
pub const PHOTO: f32 = 15.0;
/// A team must be this far clear (paces) for a lead or a pass to count, so two teams running
/// wheel to wheel never call the lead back and forth.
const CLEAR: f32 = 7.0;

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// The off: the gate drops.
    Off,
    /// A whip cracked.
    Whip,
    /// Iron dropped on the course.
    Iron,
    /// Two teams met (`other`: the one that gave the lane; `n`: how hard, tenths).
    Barge,
    /// A team wrecked (`other`: the team that did it, or 255).
    Wreck,
    /// A team took the lead (`other`: the one that had it).
    Lead,
    /// A team passed another (`n`: its place now).
    Pass,
    /// A lap run (`n`: the lap just done).
    Lap,
    /// The last lap begins.
    Final,
    /// The fastest lap of the race so far (`n`: its time, tenths of a second).
    Fastest,
    /// A team crossed the line (`n`: the place it came home in).
    Finish,
    /// The second team home was this close to the first (`n`: hundredths of a second apart).
    Photo,
}

/// One thing that happened: what, to whom, where on the course and when.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fx {
    pub kind: Kind,
    pub team: u8,
    pub other: u8,
    pub n: u16,
    /// Where it happened: paces along the course and the lane.
    pub at: [f32; 2],
    pub weight: Weight,
    /// The race's clock when it happened.
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
    /// Who leads, once the field is away.
    pub leader: Option<u8>,
    /// Who is clear ahead of whom: `(a, b)` is a ahead of b.
    ahead: Vec<(u8, u8)>,
    /// The best lap of the race so far, ticks.
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

    /// Write a record.
    pub fn put(&mut self, kind: Kind, team: u8, other: u8, n: u32, at: [f32; 2], weight: Weight, now: u32) {
        if self.fx.len() >= MOST {
            self.fx.remove(0);
        }
        self.fx.push(Fx { kind, team, other, n: n.min(u32::from(u16::MAX)) as u16, at, weight, born: now });
    }

    /// Let go of what has stood its time.
    pub fn tick(&mut self, now: u32) {
        self.fx.retain(|f| f.age(now) < LIFE);
    }

    /// The records of `kind` still showing at `now`, newest last.
    pub fn of(&self, kind: Kind, now: u32) -> impl Iterator<Item = &Fx> {
        self.fx.iter().filter(move |f| f.kind == kind && f.age(now) < LIFE)
    }

    /// How old the newest `kind` for `team` is, if it is still showing.
    pub fn since(&self, kind: Kind, team: u8, now: u32) -> Option<u32> {
        self.of(kind, now).filter(|f| f.team == team).map(|f| f.age(now)).min()
    }

    /// The age of the newest `kind` of anyone's.
    pub fn latest(&self, kind: Kind, now: u32) -> Option<u32> {
        self.of(kind, now).map(|f| f.age(now)).min()
    }

    /// The heaviest thing that happened this very tick to a team `holds` says matters: what holds
    /// the room still. A graze never does, and a rival's trouble never holds a table that is not
    /// in it.
    pub fn held(&self, now: u32, holds: impl Fn(&Fx) -> bool) -> Option<Weight> {
        self.fx.iter().filter(|f| f.born == now && holds(f)).map(|f| f.weight).max()
    }

    /// Note that `a` is clear ahead of `b`: true if `b` was clear ahead of `a` before.
    fn set_ahead(&mut self, a: u8, b: u8) -> bool {
        let swapped = self.ahead.contains(&(b, a));
        self.ahead.retain(|&(x, y)| !((x, y) == (a, b) || (x, y) == (b, a)));
        self.ahead.push((a, b));
        swapped
    }

    /// Look at the field after a tick: who leads, who passed whom.
    pub fn watch(&mut self, racers: &[Racer], now: u32) {
        let placed = |r: &Racer| r.place.is_some();
        // The lead: whoever is first on the road, held until another is clear of them.
        let front = racers
            .iter()
            .filter(|r| !placed(r))
            .max_by(|a, b| a.dist.partial_cmp(&b.dist).unwrap_or(std::cmp::Ordering::Equal));
        let current = self.leader.and_then(|l| racers.iter().find(|r| r.team == l && !placed(r)));
        match (current, front) {
            // No leader yet (the field has barely moved) or the leader is home: the front takes
            // it, quietly.
            (None, Some(f)) if self.leader.is_some() || racers.iter().all(|r| r.dist > 1.0) => self.leader = Some(f.team),
            (Some(c), Some(f)) if f.team != c.team && f.dist > c.dist + CLEAR => {
                self.put(Kind::Lead, f.team, c.team, 0, [f.dist, f.lane], Weight::Graze, now);
                self.leader = Some(f.team);
            }
            _ => {}
        }
        // Passes, pair by pair, on a clear margin.
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
