//! Book Hop's show: what lately happened on the flooded hall, kept so the
//! picture can make a fuss of it (`render::overworld::arena::book_hop`
//! draws it).
//!
//! The rules (`hop`) tell it what happened: a knight landed on a book or a
//! bank, a knight sank (and how much of the crossing it cost them), a
//! knight reached the far bank. It answers with the calls to shout
//! (`Cry`), keeps each knight's run of clean crossings (the streak), the
//! table's lanterns (one for each seat that has crossed since the last time
//! the archive was lit) and the table's tally. Nothing here changes a move:
//! it is only memory, with the juice kit's weights (`juice::Weight`) saying
//! how hard each moment hits. The one thing it adds to the score is the
//! table's own: `ARCHIVE_GOLD` for each knight each time the whole table
//! has crossed (`waves`).
//!
//! Time is the run's own tick count, so the host, a friend's mirror and a
//! browser guest draw the same moment. Every list is capped (`valid`).

use super::juice::Weight;
use super::*;

pub const LAND_LIFE: u32 = 20;
pub const SINK_LIFE: u32 = 44;
pub const CROSS_LIFE: u32 = 44;
pub const SHOUT_LIFE: u32 = super::juice::CALL_LIFE;
pub const LIT_LIFE: u32 = 60;
pub const ARCHIVE_GOLD: u32 = 2;
pub const SOLO_LANTERNS: u8 = 3;

const MOST_LANDS: usize = 14;
const MOST_SINKS: usize = 10;
const MOST_CROSSES: usize = 10;
const MOST_SHOUTS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Land {
    pub who: u8,
    pub at: (i32, i32),
    pub lane: u8,
    pub edge: bool,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sink {
    pub who: u8,
    pub at: (i32, i32),
    pub lost: u8,
    pub wall: bool,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cross {
    pub who: u8,
    pub at: (i32, i32),
    pub n: u16,
    pub clean: u16,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    First,
    Clean(u8),
    Lead,
    Lit(u8),
    Half,
    Last,
}

impl Cry {
    pub fn rank(self) -> u8 {
        match self {
            Cry::First => 2,
            Cry::Clean(n) => 3 + (n / 3).min(4) as u8,
            Cry::Lead => 3,
            Cry::Lit(_) => 9,
            Cry::Half => 6,
            Cry::Last => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shout {
    pub cry: Cry,
    pub who: u8,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ended {
    pub at: u32,
    pub winner: u8,
    pub pot: u32,
    pub most: u16,
    pub was: u16,
    pub waves: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HopShow {
    pub lands: Vec<Land>,
    pub sinks: Vec<Sink>,
    pub crosses: Vec<Cross>,
    pub shouts: Vec<Shout>,
    pub streak: BTreeMap<u32, u16>,
    pub best: BTreeMap<u32, u16>,
    pub sunk: BTreeMap<u32, u16>,
    pub rows: BTreeMap<u32, i16>,
    pub lit: BTreeMap<u32, u8>,
    pub waves: u8,
    pub flared: Option<u32>,
    pub team: u16,
    pub lead: Option<u8>,
    pub called: u8,
    pub ended: Option<Ended>,
}

const HALF: u8 = 1;
const LAST: u8 = 2;

impl HopShow {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.lands.len() <= MOST_LANDS
            && self.sinks.len() <= MOST_SINKS
            && self.crosses.len() <= MOST_CROSSES
            && self.shouts.len() <= MOST_SHOUTS
            && self.streak.len() <= MAX_PLAYERS as usize
            && self.best.len() <= MAX_PLAYERS as usize
            && self.sunk.len() <= MAX_PLAYERS as usize
            && self.rows.len() <= MAX_PLAYERS as usize
            && self.lit.len() <= MAX_PLAYERS as usize
            && self.streak.keys().chain(self.best.keys()).chain(self.sunk.keys()).chain(self.rows.keys()).chain(self.lit.keys()).all(seat)
    }

    pub fn tick(&mut self, now: u32) {
        self.lands.retain(|l| now.saturating_sub(l.born) < LAND_LIFE);
        self.sinks.retain(|s| now.saturating_sub(s.born) < SINK_LIFE);
        self.crosses.retain(|c| now.saturating_sub(c.born) < CROSS_LIFE);
        self.shouts.retain(|s| now.saturating_sub(s.born) < SHOUT_LIFE);
        if self.flared.is_some_and(|f| now.saturating_sub(f) >= LIT_LIFE) {
            self.flared = None;
        }
    }

    pub fn shout(&mut self, now: u32, cry: Cry, who: u32) {
        if self.shouts.len() >= MOST_SHOUTS {
            self.shouts.remove(0);
        }
        self.shouts.push(Shout { cry, who: who as u8, born: now });
    }

    pub fn stood(&mut self, who: u32, row: i32) -> Option<i32> {
        let was = self.rows.insert(who, row as i16);
        was.map(i32::from).filter(|&r| r != row)
    }

    pub fn landed(&mut self, now: u32, who: u32, at: (i32, i32), lane: u8, edge: bool) {
        if self.lands.len() >= MOST_LANDS {
            self.lands.remove(0);
        }
        self.lands.push(Land { who: who as u8, at, lane, edge, born: now });
    }

    pub fn sank(&mut self, now: u32, who: u32, at: (i32, i32), lost: u32, wall: bool) -> Weight {
        if self.sinks.len() >= MOST_SINKS {
            self.sinks.remove(0);
        }
        self.sinks.push(Sink { who: who as u8, at, lost: lost.min(255) as u8, wall, born: now });
        self.streak.remove(&who);
        *self.sunk.entry(who).or_default() += 1;
        Weight::Graze
    }

    pub fn crossed(&mut self, now: u32, who: u32, at: (i32, i32), n: u16, scores: &BTreeMap<u32, u16>) -> Weight {
        let run = self.streak.entry(who).or_default();
        *run += 1;
        let clean = *run;
        let best = self.best.entry(who).or_default();
        *best = (*best).max(clean);
        if self.crosses.len() >= MOST_CROSSES {
            self.crosses.remove(0);
        }
        self.crosses.push(Cross { who: who as u8, at, n, clean, born: now });
        let first = self.team == 0;
        self.team = self.team.saturating_add(1);
        if first {
            self.shout(now, Cry::First, who);
        }
        if matches!(clean, 2 | 3 | 5 | 8 | 12) {
            self.shout(now, Cry::Clean(clean.min(255) as u8), who);
        }
        let mut weight = if clean >= 3 { Weight::Crushing } else { Weight::Solid };
        let need = if scores.len() > 1 { 1 } else { SOLO_LANTERNS };
        let lit = self.lit.entry(who).or_default();
        *lit = (*lit + 1).min(need);
        if scores.keys().all(|id| self.lit.get(id).copied().unwrap_or(0) >= need) {
            self.lit.clear();
            self.waves = self.waves.saturating_add(1);
            self.flared = Some(now);
            self.shout(now, Cry::Lit(self.waves), 0);
            weight = Weight::Kill;
        }
        weight
    }

    pub fn standings(&mut self, now: u32, scores: &BTreeMap<u32, u16>, left: u32) {
        let top = scores.values().copied().max().unwrap_or(0);
        let leaders: Vec<u32> = scores.iter().filter(|&(_, &n)| n == top && top > 0).map(|(&id, _)| id).collect();
        let lead = (leaders.len() == 1 && scores.len() > 1 && top >= 2).then(|| leaders[0] as u8);
        if let Some(one) = lead
            && self.lead != Some(one)
        {
            if self.lead.is_some() {
                self.shout(now, Cry::Lead, u32::from(one));
            }
            self.lead = Some(one);
        }
        if left <= super::hop::HOP_TIME / HZ / 2 && self.called & HALF == 0 {
            self.called |= HALF;
            self.shout(now, Cry::Half, 0);
        }
        if left <= 10 && self.called & LAST == 0 {
            self.called |= LAST;
            self.shout(now, Cry::Last, 0);
        }
    }

    pub fn finish(&mut self, now: u32, winner: u32, pot: u32, most: u16, was: u16) {
        self.ended = Some(Ended { at: now, winner: winner as u8, pot, most, was, waves: self.waves });
    }

    pub fn since_end(&self, now: u32) -> Option<u32> {
        self.ended.map(|e| now.saturating_sub(e.at))
    }

    pub fn run_of(&self, who: u32) -> u16 {
        self.streak.get(&who).copied().unwrap_or(0)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/hop_show__tests.rs"]
mod tests;
