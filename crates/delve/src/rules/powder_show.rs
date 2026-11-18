//! The Powder Room's show: what lately went off in the store, kept so the
//! picture can make a fuss of it (`render::overworld::arena::powder_room`
//! draws it).
//!
//! The rules (`powder`) tell it what happened: a keg set down, a keg gone
//! off with its arms of fire and the crates it broke, a knight sooted, the
//! table's crates passing a mark. It answers with the calls to shout
//! (`Cry`), keeps each knight's run of crates, the table's count, the
//! chain a blast set going, and the tiles the fire blackened. Nothing here
//! changes a score or a move: it is memory, with the juice kit's weights
//! (`juice::Weight`) saying how hard each moment hits.
//!
//! Time is the run's own tick count, so the host, a friend's mirror and a
//! browser guest draw the same moment. Every list is capped (`valid`).

use super::juice::Weight;
use super::*;
use std::collections::BTreeSet;

pub const BLAST_LIFE: u32 = 110;
pub const DROP_LIFE: u32 = 18;
pub const SOOT_LIFE: u32 = 52;
pub const SHOUT_LIFE: u32 = super::juice::CALL_LIFE;
pub const CHAIN_GAP: u32 = 3 * HZ;
pub const LINK: u32 = 3;

const MOST_BLASTS: usize = 36;
const MOST_DROPS: usize = 12;
const MOST_SOOTS: usize = 8;
const MOST_SHOUTS: usize = 6;
const MOST_TILES: usize = 1344;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blast {
    pub at: (i32, i32),
    pub who: u8,
    pub arms: [u8; 4],
    pub crates: Vec<(i32, i32)>,
    pub depth: u8,
    pub born: u32,
    pub weight: Weight,
    pub sooted: u8,
    pub last: bool,
}

impl Blast {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
    pub fn arm_tile(&self, d: usize, i: i32) -> (i32, i32) {
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][d & 3];
        (self.at.0 + dx * i, self.at.1 + dy * i)
    }
    pub fn covers(&self, tile: (i32, i32)) -> bool {
        tile == self.at || (0..4).any(|d| (1..=i32::from(self.arms[d])).any(|i| self.arm_tile(d, i) == tile))
    }
    pub fn tiles(&self) -> Vec<(i32, i32)> {
        let mut out = vec![self.at];
        for d in 0..4 {
            out.extend((1..=i32::from(self.arms[d])).map(|i| self.arm_tile(d, i)));
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drop {
    pub at: (i32, i32),
    pub who: u8,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Soot {
    pub who: u8,
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub lost: u8,
    pub by: u8,
    pub born: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    First,
    Haul(u8),
    Chain(u8),
    Soot,
    Payback,
    OwnKeg,
    Lead,
    Store(u8),
    Roll(u8),
    Half,
    Last,
}

impl Cry {
    pub fn rank(self) -> u8 {
        match self {
            Cry::First => 2,
            Cry::Haul(n) => 3 + n.min(4),
            Cry::Chain(n) => 5 + n.min(8),
            Cry::Soot => 6,
            Cry::Payback => 8,
            Cry::OwnKeg => 7,
            Cry::Lead => 3,
            Cry::Store(_) => 6,
            Cry::Roll(_) => 4,
            Cry::Half => 6,
            Cry::Last => 14,
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
    pub most: u32,
    pub cleared: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowderShow {
    pub blasts: Vec<Blast>,
    pub drops: Vec<Drop>,
    pub soots: Vec<Soot>,
    pub shouts: Vec<Shout>,
    pub scorch: BTreeSet<u16>,
    pub combo: BTreeMap<u32, (u32, u16)>,
    pub best: BTreeMap<u32, u16>,
    pub got: BTreeMap<u32, u16>,
    pub sooted: BTreeMap<u32, u16>,
    pub hurt_by: BTreeMap<u32, u8>,
    pub team: u16,
    pub chain: u8,
    pub lead: Option<u8>,
    pub called: u8,
    pub ended: Option<Ended>,
}

const HALF: u8 = 1;
const LAST: u8 = 2;
const QUARTER: u8 = 4;
const HALFWAY: u8 = 8;
const MOST: u8 = 16;
const FIRST: u8 = 32;

impl PowderShow {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.blasts.len() <= MOST_BLASTS
            && self.blasts.iter().all(|b| b.crates.len() <= 4 && b.arms.iter().all(|&a| a <= 8))
            && self.drops.len() <= MOST_DROPS
            && self.soots.len() <= MOST_SOOTS
            && self.shouts.len() <= MOST_SHOUTS
            && self.scorch.len() <= MOST_TILES
            && self.combo.len() <= MAX_PLAYERS as usize
            && self.best.len() <= MAX_PLAYERS as usize
            && self.got.len() <= MAX_PLAYERS as usize
            && self.sooted.len() <= MAX_PLAYERS as usize
            && self.hurt_by.len() <= MAX_PLAYERS as usize
            && self.combo.keys().chain(self.best.keys()).chain(self.got.keys()).chain(self.sooted.keys()).chain(self.hurt_by.keys()).all(seat)
    }

    pub fn tick(&mut self, now: u32) {
        self.blasts.retain(|b| b.age(now) < BLAST_LIFE);
        self.drops.retain(|d| now.saturating_sub(d.born) < DROP_LIFE);
        self.soots.retain(|s| now.saturating_sub(s.born) < SOOT_LIFE);
        self.shouts.retain(|s| now.saturating_sub(s.born) < SHOUT_LIFE);
    }

    pub fn shout(&mut self, now: u32, cry: Cry, who: u32) {
        if self.shouts.len() >= MOST_SHOUTS {
            self.shouts.remove(0);
        }
        self.shouts.push(Shout { cry, who: who as u8, born: now });
    }

    pub fn dropped(&mut self, now: u32, who: u32, at: (i32, i32)) {
        if self.drops.len() >= MOST_DROPS {
            self.drops.remove(0);
        }
        self.drops.push(Drop { at, who: who as u8, born: now });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn blast(&mut self, now: u32, who: u32, at: (i32, i32), arms: [u8; 4], crates: Vec<(i32, i32)>, depth: u8, cols: i32) -> Weight {
        if self.called & FIRST == 0 {
            self.called |= FIRST;
            self.shout(now, Cry::First, who);
        }
        let n = crates.len();
        let weight = weigh(n, depth, 0, false);
        if n >= 3 {
            self.shout(now, Cry::Haul(n.min(4) as u8), who);
        }
        let blast = Blast { at, who: who as u8, arms, crates, depth, born: now, weight, sooted: 0, last: false };
        for (x, y) in blast.tiles() {
            if x >= 0 && y >= 0 && cols > 0 && self.scorch.len() < MOST_TILES {
                self.scorch.insert((y * cols + x) as u16);
            }
        }
        if self.blasts.len() >= MOST_BLASTS {
            self.blasts.remove(0);
        }
        self.blasts.push(blast);
        weight
    }

    pub fn broke(&mut self, now: u32, who: u32, n: u16, left: usize) {
        if n == 0 {
            return;
        }
        let (last, run) = self.combo.get(&who).copied().unwrap_or((0, 0));
        let run = if run > 0 && now.saturating_sub(last) <= CHAIN_GAP { run + n } else { n };
        self.combo.insert(who, (now, run));
        let best = self.best.entry(who).or_default();
        let before = *best;
        *best = (*best).max(run);
        for mark in [5, 10, 20] {
            if before < mark && run >= mark {
                self.shout(now, Cry::Roll(mark as u8), who);
            }
        }
        *self.got.entry(who).or_default() += n;
        let was = self.team;
        self.team = self.team.saturating_add(n);
        let total = u32::from(self.team) + left as u32;
        let (a, b) = (u32::from(was) * 100 / total.max(1), u32::from(self.team) * 100 / total.max(1));
        if a < 25 && b >= 25 && self.called & QUARTER == 0 {
            self.called |= QUARTER;
            self.shout(now, Cry::Store(25), 0);
        }
        if a < 50 && b >= 50 && self.called & HALFWAY == 0 {
            self.called |= HALFWAY;
            self.shout(now, Cry::Store(50), 0);
        }
        if a < 80 && b >= 80 && self.called & MOST == 0 {
            self.called |= MOST;
            self.shout(now, Cry::Store(80), 0);
        }
    }

    pub fn chained(&mut self, now: u32, who: u32, links: u8) {
        if links < 2 {
            return;
        }
        self.chain = self.chain.max(links);
        self.shout(now, Cry::Chain(links), who);
        if links >= 5
            && let Some(deepest) = self.blasts.iter_mut().filter(|b| b.born == now).max_by_key(|b| b.depth)
        {
            deepest.weight = deepest.weight.max(Weight::Finale);
        }
    }

    pub fn cleared_by_last(&mut self, now: u32) {
        if let Some(b) = self.blasts.iter_mut().rev().find(|b| b.born == now) {
            b.last = true;
            b.weight = b.weight.max(Weight::Finale);
        }
    }

    pub fn blame(&self, tile: (i32, i32), now: u32) -> Option<&Blast> {
        self.blasts
            .iter()
            .rev()
            .find(|b| b.age(now) < super::powder::FIRE + LINK * 12 && b.covers(tile))
    }

    pub fn sooted(&mut self, now: u32, who: u32, from: (i32, i32), to: (i32, i32), lost: u32) -> Weight {
        let by = self.blame(from, now).map_or(0, |b| u32::from(b.who));
        let mut weight = Weight::Kill;
        if let Some(b) = self.blasts.iter_mut().rev().find(|b| b.covers(from) && b.born + LINK * 12 >= now) {
            b.sooted = b.sooted.saturating_add(1);
            b.weight = b.weight.max(Weight::Kill);
            weight = b.weight;
        }
        if self.soots.len() >= MOST_SOOTS {
            self.soots.remove(0);
        }
        self.soots.push(Soot { who: who as u8, from, to, lost: lost.min(255) as u8, by: by as u8, born: now });
        *self.sooted.entry(who).or_default() += 1;
        self.combo.remove(&who);
        let revenge = by != 0 && by as u32 != who && self.hurt_by.get(&(by as u32)).copied() == Some(who as u8);
        if by as u32 == who {
            self.shout(now, Cry::OwnKeg, who);
        } else if revenge {
            self.shout(now, Cry::Payback, by as u32);
        } else {
            self.shout(now, Cry::Soot, who);
        }
        if by != 0 && by as u32 != who {
            self.hurt_by.insert(who, by as u8);
        }
        weight
    }

    pub fn standings(&mut self, now: u32, scores: &BTreeMap<u32, u16>, left: u32) {
        let top = scores.values().copied().max().unwrap_or(0);
        let leaders: Vec<u32> = scores.iter().filter(|&(_, &n)| n == top && top > 0).map(|(&id, _)| id).collect();
        let lead = (leaders.len() == 1 && scores.len() > 1 && top >= 4).then(|| leaders[0] as u8);
        if let Some(one) = lead
            && self.lead != Some(one)
        {
            if self.lead.is_some() {
                self.shout(now, Cry::Lead, u32::from(one));
            }
            self.lead = Some(one);
        }
        if left <= powder::POWDER_TIME / HZ / 2 && self.called & HALF == 0 {
            self.called |= HALF;
            self.shout(now, Cry::Half, 0);
        }
        if left <= 10 && self.called & LAST == 0 {
            self.called |= LAST;
            self.shout(now, Cry::Last, 0);
        }
    }

    pub fn finish(&mut self, now: u32, winner: u32, pot: u32, most: u32, cleared: bool) {
        self.ended = Some(Ended { at: now, winner: winner as u8, pot, most, cleared });
    }

    pub fn since_end(&self, now: u32) -> Option<u32> {
        self.ended.map(|e| now.saturating_sub(e.at))
    }

    pub fn run_of(&self, who: u32, now: u32) -> u16 {
        self.combo
            .get(&who)
            .filter(|&&(last, _)| now.saturating_sub(last) <= CHAIN_GAP)
            .map_or(0, |&(_, n)| n)
    }
}

pub fn weigh(crates: usize, depth: u8, sooted: u8, last: bool) -> Weight {
    if last {
        Weight::Finale
    } else if sooted > 0 || depth >= 2 || crates >= 4 {
        Weight::Kill
    } else if crates >= 2 || depth >= 1 {
        Weight::Crushing
    } else if crates == 1 {
        Weight::Solid
    } else {
        Weight::Graze
    }
}

pub fn cross(room: &Room, crates: &BTreeSet<u16>, at: (i32, i32)) -> ([u8; 4], Vec<(i32, i32)>) {
    let cols = room.cols as i32;
    let mut arms = [0u8; 4];
    let mut hit = Vec::new();
    for (d, (dx, dy)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
        for r in 1..=powder::REACH {
            let (x, y) = (at.0 + dx * r, at.1 + dy * r);
            if x < 0 || y < 0 || x >= cols || y >= room.rows as i32 {
                break;
            }
            if crates.contains(&((y * cols + x) as u16)) {
                arms[d] += 1;
                hit.push((x, y));
                break;
            }
            if room.tile(x, y) != Tile::Floor {
                break;
            }
            arms[d] += 1;
        }
    }
    (arms, hit)
}

#[cfg(test)]
#[path = "../../tests/rules/powder_show__tests.rs"]
mod tests;
