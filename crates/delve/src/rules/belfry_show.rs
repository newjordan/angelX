//! The Belfry Shoot's show: what lately happened in the belfry, kept so the
//! picture can make a fuss of it (`render::overworld::arena::belfry_shoot`
//! draws it).
//!
//! The rules (`folly`) tell it what happened: a flock dropped from the
//! rafters, a bat struck, a bat felled, a knight bitten, the lead changing
//! hands, the clock passing half and the last ten seconds. It answers with
//! the calls to shout (`Shout`), keeps each knight's run of bats (the combo)
//! and the table's count (the bell tolls every ten). Nothing here changes a
//! score or a move: it is only memory, with the juice kit's own weights
//! (`juice::Weight`) saying how hard each moment hits.
//!
//! Time is the run's own tick count, so the host, a friend's mirror and a
//! browser guest draw the same moment. Every list is capped (`valid`).

use super::folly::{BAT_TIME, BatShoot};
use super::juice::Weight;
use super::*;

/// Ticks each kind of moment stays on the books for its effects.
pub const HIT_LIFE: u32 = 26;
pub const BITE_LIFE: u32 = 30;
pub const SPAWN_LIFE: u32 = 20;
pub const TOLL_LIFE: u32 = 64;
pub const SHOUT_LIFE: u32 = super::juice::CALL_LIFE;
/// Bats felled this near together (ticks) run on as one combo.
pub const CHAIN_GAP: u32 = 2 * HZ;
/// The table's bats between tolls of the bell.
pub const TOLL_EVERY: u16 = 10;

const MOST_HITS: usize = 18;
const MOST_BITES: usize = 8;
const MOST_SPAWNS: usize = 14;
const MOST_SHOUTS: usize = 6;
const MOST_TOLLS: usize = 4;
const MOST_FLOCKS: usize = 8;

/// A bat struck or felled: by whom (`who` 0: nobody), where it was and where
/// the blow came from (room pixels), what it took, how it weighed, and for a
/// kill its place in the knight's run and whether it was the flock's last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub who: u8,
    pub bat: u32,
    pub at: (i32, i32),
    pub from: (i32, i32),
    pub lost: u16,
    pub weight: Weight,
    pub kill: bool,
    pub chain: u8,
    pub last: bool,
    pub born: u32,
}

impl Hit {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

/// A knight bitten: where they stood (room pixels) and what it took.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bite {
    pub who: u8,
    pub at: (i32, i32),
    pub lost: u8,
    pub born: u32,
}

/// A bat dropped from the rafters: where (room pixels) and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Swoop {
    pub at: (i32, i32),
    pub born: u32,
}

/// What a banner calls. Words are the picture's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// The first bat of the night.
    First,
    /// A knight felled n bats within `CHAIN_GAP` of each other.
    Chain(u8),
    /// A whole flock down.
    Flock,
    /// A knight took the lead.
    Lead,
    /// The bell tolled for the n-th ten bats of the table.
    Toll(u8),
    /// Half the minute gone.
    Half,
    /// The last ten seconds.
    Last,
}

impl Cry {
    /// How loudly it is shown: the banner with the higher rank stays on top.
    pub fn rank(self) -> u8 {
        match self {
            Cry::First => 2,
            Cry::Chain(n) => 3 + n.min(6),
            Cry::Flock => 4,
            Cry::Lead => 3,
            Cry::Toll(_) => 7,
            Cry::Half => 6,
            Cry::Last => 10,
        }
    }
}

/// A banner called for a knight (`who` 0: for the whole room) at a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shout {
    pub cry: Cry,
    pub who: u8,
    pub born: u32,
}

/// How the shoot ended: when (run tick), who took it, the pot and the
/// winner's harm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ended {
    pub at: u32,
    pub winner: u8,
    pub pot: u32,
    pub most: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatShow {
    pub hits: Vec<Hit>,
    pub bites: Vec<Bite>,
    pub swoops: Vec<Swoop>,
    pub shouts: Vec<Shout>,
    /// The ticks the bell tolled, newest last.
    pub tolls: Vec<u32>,
    /// Each knight's run of bats: the tick of the last and the count.
    pub combo: BTreeMap<u32, (u32, u16)>,
    /// The longest run each knight made.
    pub best: BTreeMap<u32, u16>,
    /// Bats each knight felled.
    pub felled: BTreeMap<u32, u16>,
    /// Bats the whole table felled.
    pub team: u16,
    /// The knight now ahead on their own.
    pub lead: Option<u8>,
    /// The bats of each recent flock still on the wing: the flock's number
    /// and their ids.
    pub flocks: Vec<(u16, Vec<u32>)>,
    /// Each knight's health at the last look, to tell a bite.
    pub hp: BTreeMap<u32, u32>,
    /// Which of the clock's calls have been made.
    pub called: u8,
    pub ended: Option<Ended>,
}

const HALF: u8 = 1;
const LAST: u8 = 2;

impl BatShow {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.hits.len() <= MOST_HITS
            && self.bites.len() <= MOST_BITES
            && self.swoops.len() <= MOST_SPAWNS
            && self.shouts.len() <= MOST_SHOUTS
            && self.tolls.len() <= MOST_TOLLS
            && self.flocks.len() <= MOST_FLOCKS
            && self.flocks.iter().all(|(_, ids)| ids.len() <= 12)
            && self.combo.len() <= MAX_PLAYERS as usize
            && self.best.len() <= MAX_PLAYERS as usize
            && self.felled.len() <= MAX_PLAYERS as usize
            && self.hp.len() <= MAX_PLAYERS as usize
            && self.combo.keys().chain(self.best.keys()).chain(self.felled.keys()).chain(self.hp.keys()).all(seat)
    }

    /// Let go what is old at `now`.
    pub fn tick(&mut self, now: u32) {
        self.hits.retain(|h| h.age(now) < HIT_LIFE.max(48));
        self.bites.retain(|b| now.saturating_sub(b.born) < BITE_LIFE);
        self.swoops.retain(|d| now.saturating_sub(d.born) < SPAWN_LIFE);
        self.shouts.retain(|s| now.saturating_sub(s.born) < SHOUT_LIFE);
        self.tolls.retain(|&t| now.saturating_sub(t) < TOLL_LIFE);
    }

    /// Call a banner (the loudest stays on top when they come together).
    pub fn shout(&mut self, now: u32, cry: Cry, who: u32) {
        if self.shouts.len() >= MOST_SHOUTS {
            self.shouts.remove(0);
        }
        self.shouts.push(Shout { cry, who: who as u8, born: now });
    }

    /// A flock dropped from the rafters: `bats` are their ids and places.
    pub fn flock(&mut self, now: u32, number: u16, bats: &[(u32, (i32, i32))]) {
        if bats.is_empty() {
            return;
        }
        for &(_, at) in bats {
            if self.swoops.len() >= MOST_SPAWNS {
                self.swoops.remove(0);
            }
            self.swoops.push(Swoop { at, born: now });
        }
        if self.flocks.len() >= MOST_FLOCKS {
            self.flocks.remove(0);
        }
        self.flocks.push((number, bats.iter().map(|&(id, _)| id).collect()));
    }

    fn push_hit(&mut self, hit: Hit) {
        if self.hits.len() >= MOST_HITS {
            self.hits.remove(0);
        }
        self.hits.push(hit);
    }

    /// A bat struck but still on the wing: how hard it weighed.
    pub fn struck(&mut self, now: u32, who: u32, bat: u32, at: (i32, i32), from: (i32, i32), lost: u32, max: u32) -> Weight {
        // A bat is small: a solid blow is as hard as it weighs.
        let weight = Weight::of(lost, max, false, false).min(Weight::Solid);
        self.push_hit(Hit {
            who: who as u8,
            bat,
            at,
            from,
            lost: lost.min(u32::from(u16::MAX)) as u16,
            weight,
            kill: false,
            chain: 0,
            last: false,
            born: now,
        });
        weight
    }

    /// A bat felled by `who` (0: nobody's). Counts the knight's run, the
    /// flock and the table, makes the calls, and says how hard it weighed:
    /// a kill lands solid; a third in a row or a flock's last holds the room.
    pub fn felled(&mut self, now: u32, who: u32, bat: u32, at: (i32, i32), from: (i32, i32), lost: u32) -> Weight {
        let mut chain = 0;
        if who != 0 {
            let (last, n) = self.combo.get(&who).copied().unwrap_or((0, 0));
            let n = if n > 0 && now.saturating_sub(last) <= CHAIN_GAP { n + 1 } else { 1 };
            self.combo.insert(who, (now, n));
            let best = self.best.entry(who).or_default();
            *best = (*best).max(n);
            chain = n.min(255) as u8;
            *self.felled.entry(who).or_default() += 1;
            if matches!(n, 2 | 3 | 4 | 5 | 8 | 12 | 16) {
                self.shout(now, Cry::Chain(n.min(255) as u8), who);
            }
        }
        if self.team == 0 {
            self.shout(now, Cry::First, who);
        }
        self.team = self.team.saturating_add(1);
        if self.team.is_multiple_of(TOLL_EVERY) {
            self.toll(now);
            self.shout(now, Cry::Toll((self.team / TOLL_EVERY).min(255) as u8), 0);
        }
        // Its flock's last bat, felled by anyone, ends the flock.
        let mut last = false;
        if let Some(i) = self.flocks.iter().position(|(_, ids)| ids.contains(&bat)) {
            self.flocks[i].1.retain(|&id| id != bat);
            if self.flocks[i].1.is_empty() {
                self.flocks.remove(i);
                last = true;
                self.shout(now, Cry::Flock, who);
            }
        }
        let weight = if last || chain >= 3 { Weight::Kill } else { Weight::Solid };
        self.push_hit(Hit {
            who: who as u8,
            bat,
            at,
            from,
            lost: lost.min(u32::from(u16::MAX)) as u16,
            weight,
            kill: true,
            chain,
            last,
            born: now,
        });
        weight
    }

    /// A knight bitten.
    pub fn bitten(&mut self, now: u32, who: u32, at: (i32, i32), lost: u32) {
        if self.bites.len() >= MOST_BITES {
            self.bites.remove(0);
        }
        self.bites.push(Bite { who: who as u8, at, lost: lost.min(255) as u8, born: now });
        // A bite breaks the run.
        self.combo.remove(&who);
    }

    /// The tally changed: call a change of lead (a few bats in, out of a
    /// tie, taken from another's hands), and the clock's two calls.
    pub fn standings(&mut self, now: u32, scores: &BTreeMap<u32, u32>, left: u32) {
        let top = scores.values().copied().max().unwrap_or(0);
        let leaders: Vec<u32> = scores.iter().filter(|&(_, &n)| n == top && top > 0).map(|(&id, _)| id).collect();
        let lead = (leaders.len() == 1 && scores.len() > 1 && top >= 4 * 30).then(|| leaders[0] as u8);
        if let Some(one) = lead
            && self.lead != Some(one)
        {
            if self.lead.is_some() {
                self.shout(now, Cry::Lead, u32::from(one));
            }
            self.lead = Some(one);
        }
        if left <= BAT_TIME / HZ / 2 && self.called & HALF == 0 {
            self.called |= HALF;
            self.shout(now, Cry::Half, 0);
        }
        if left <= 10 && self.called & LAST == 0 {
            self.called |= LAST;
            self.shout(now, Cry::Last, 0);
        }
    }

    /// The bell tolls (the start and the end of the minute, each ten bats).
    pub fn toll(&mut self, now: u32) {
        if self.tolls.len() >= MOST_TOLLS {
            self.tolls.remove(0);
        }
        self.tolls.push(now);
    }

    /// The bats still flying when the minute ends scatter: nobody's.
    pub fn scatter(&mut self, now: u32, bats: &[(u32, (i32, i32))]) {
        for &(bat, at) in bats {
            self.push_hit(Hit { who: 0, bat, at, from: at, lost: 0, weight: Weight::Graze, kill: true, chain: 0, last: false, born: now });
        }
    }

    /// The shoot is over: who took it, the pot, the most harm.
    pub fn finish(&mut self, now: u32, winner: u32, pot: u32, most: u32) {
        self.ended = Some(Ended { at: now, winner: winner as u8, pot, most });
    }

    /// Ticks since the shoot ended at `now`, while it is the show.
    pub fn since_end(&self, now: u32) -> Option<u32> {
        self.ended.map(|e| now.saturating_sub(e.at))
    }

    /// The knight's run of bats now: none once it has gone cold.
    pub fn run_of(&self, who: u32, now: u32) -> u16 {
        self.combo
            .get(&who)
            .filter(|&&(last, _)| now.saturating_sub(last) <= CHAIN_GAP)
            .map_or(0, |&(_, n)| n)
    }
}

impl Run {
    /// Read the tick's blows off the bats' health (a drop since last tick
    /// is a hit, a bat at nothing is felled; the last hurt is whose it was),
    /// and the bites off the knights': record them in the shoot's show, and
    /// hold the room for the heaviest. Called from the shoot's own tick, so
    /// before the dead are cleared (`settle_kills`).
    pub(super) fn bats_watch(&mut self, shoot: &mut BatShoot) {
        let now = self.tick as u32;
        let px = |x: f32, y: f32| ((x / TILE_UNITS * 16.0) as i32, (y / TILE_UNITS * 16.0) as i32);
        let mut heaviest: Option<Weight> = None;
        let mut bats: Vec<(u32, f32, f32, u32, u32, u32, u32)> = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Bat && e.seen_hp > e.hp)
            .map(|e| (e.id, e.x, e.y, e.hp, e.seen_hp, e.max_hp, e.wits.foe))
            .collect();
        bats.sort_by_key(|b| b.0);
        // Only a kill shakes the picture: four knights wound a bat a dozen
        // times a second.
        let mut shaken = Weight::Graze;
        for (id, x, y, hp, seen, max, foe) in bats {
            let who = if shoot.scores.contains_key(&foe) { foe } else { 0 };
            let from = self.players.get(&who).map_or((x, y), |h| (h.x, h.y));
            let (at, from) = (px(x, y), px(from.0, from.1));
            if hp == 0 {
                let weight = shoot.show.felled(now, who, id, at, from, seen);
                shaken = shaken.max(weight);
                heaviest = heaviest.max(Some(weight));
            } else {
                shoot.show.struck(now, who, id, at, from, seen - hp, max);
            }
        }
        // Bites: a knight's health lower than it was.
        let looks: Vec<(u32, u32, f32, f32)> = self.players.iter().map(|(&id, h)| (id, h.hp, h.x, h.y)).collect();
        for (id, hp, x, y) in looks {
            let was = shoot.show.hp.insert(id, hp).unwrap_or(hp);
            if hp < was && shoot.scores.contains_key(&id) && !shoot.titled() {
                shoot.show.bitten(now, id, px(x, y), was - hp);
            }
        }
        shoot.show.standings(now, &shoot.scores, shoot.left());
        // The bell: a toll of ten rocks the room, a held breath for the
        // flock's last bat or a third in a row.
        let tolled = shoot.show.tolls.last() == Some(&now);
        if tolled {
            self.sounds.push("circle_toll");
            self.fx.push(Fx::Shake(8));
        } else if shaken >= Weight::Kill {
            self.fx.push(Fx::Shake(5));
        } else if shaken >= Weight::Solid {
            self.fx.push(Fx::Shake(2));
        }
        if let Some(weight) = heaviest {
            self.hold_for(weight);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/belfry_show__tests.rs"]
mod tests;
