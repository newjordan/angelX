//! The juice kit, state half: what a scoring game remembers so its picture
//! can make a fuss (`render::overworld::arena::juice` draws it).
//!
//! A game that scores blows and kills owns one `Juice`, serialized beside its
//! round, and tells it three things:
//!
//! - `blow`: a blow landed (who, on whom, where, how much of the target's
//!   health it took). The picture reads `Blow::weight` for the freeze, the
//!   shake, the white impact frame and the flying numbers.
//! - `kill`: a kill scored (killer, victim, everyone's score). It answers
//!   the calls to banner: first blood, double and triple kills, sprees, a
//!   streak ended, revenge, a change of lead, match point.
//! - `tick`: once a tick, to let old blows and calls go.
//!
//! Time is the caller's own tick count, so the kit is deterministic and a
//! friend's mirror or a browser guest sees what the host does. A hit-stop is
//! the realm's (`hitstop`): `Run::hold_for` asks for the weight's pause.

use super::*;

/// Ticks a blow stays on the books for its effects (the longest is the
/// flying number and the squash).
pub const BLOW_LIFE: u32 = 36;
/// Ticks a call stays up.
pub const CALL_LIFE: u32 = 78;
/// A kill within this many ticks of the last chains into a double, a triple.
pub const CHAIN_TICKS: u32 = 4 * HZ;
const MOST_BLOWS: usize = 24;
const MOST_CALLS: usize = 8;

/// How hard a blow landed: the one number the effects are scaled by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Weight {
    /// A scratch: a spark and a number, nothing held.
    Graze,
    /// A solid hit: a short freeze, a flash, a kick.
    Solid,
    /// A crushing hit, a big share of the target's health.
    Crushing,
    /// A kill.
    Kill,
    /// The kill that wins the round.
    Finale,
}

impl Weight {
    /// The weight of a blow that took `lost` of a target's `max` health.
    pub fn of(lost: u32, max: u32, kill: bool, decisive: bool) -> Weight {
        let share = lost * 100 / max.max(1);
        match (kill, decisive) {
            (true, true) => Weight::Finale,
            (true, false) => Weight::Kill,
            _ if share >= 45 => Weight::Crushing,
            _ if share >= 18 => Weight::Solid,
            _ => Weight::Graze,
        }
    }
    /// Pixels of screen shake at the canvas's own size (384 by 224) at the
    /// moment of impact; the picture decays it over `shake_ticks`.
    pub fn shake(self) -> f32 {
        match self {
            Weight::Graze => 0.0,
            Weight::Solid => 2.0,
            Weight::Crushing => 4.0,
            Weight::Kill => 5.0,
            Weight::Finale => 8.0,
        }
    }
    /// Ticks the shake lasts.
    pub fn shake_ticks(self) -> u32 {
        match self {
            Weight::Graze => 0,
            Weight::Solid => 6,
            Weight::Crushing => 9,
            Weight::Kill => 12,
            Weight::Finale => 24,
        }
    }
    /// Ticks the struck sprite stands in its white impact frame.
    pub fn flash(self) -> u32 {
        match self {
            Weight::Graze => 1,
            Weight::Solid => 2,
            Weight::Crushing => 3,
            Weight::Kill | Weight::Finale => 4,
        }
    }
    /// How hard a struck body is squashed and stretched (`0` none).
    pub fn squash(self) -> f32 {
        match self {
            Weight::Graze => 0.08,
            Weight::Solid => 0.2,
            Weight::Crushing => 0.36,
            Weight::Kill | Weight::Finale => 0.5,
        }
    }
    fn impact(self) -> Option<hitstop::Impact> {
        match self {
            // A solid blow is felt (flash, kick, shake) but never stops the room: six knights trade
            // blows every second, and only the crushing, the kill and the win hold time still.
            Weight::Graze | Weight::Solid => None,
            Weight::Crushing => Some(hitstop::Impact::Heavy),
            Weight::Kill => Some(hitstop::Impact::EliteKill),
            Weight::Finale => Some(hitstop::Impact::BossKill),
        }
    }
}

/// A blow that landed: where, from where, and how it weighed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blow {
    pub owner: u32,
    pub target: u32,
    /// Where the target stood.
    pub x: f32,
    pub y: f32,
    /// Where the owner stood.
    pub from: (f32, f32),
    /// Health and armour the blow took.
    pub lost: u16,
    pub weight: Weight,
    /// The tick it landed on (the game's own count).
    pub born: u32,
}

impl Blow {
    /// Ticks since it landed at `now`.
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
    /// The way the blow went, a unit vector from owner to target.
    pub fn way(&self) -> (f32, f32) {
        unit(self.x - self.from.0, self.y - self.from.1)
    }
}

/// What a banner says. Words are the picture's; this is only what happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    FirstBlood,
    /// A chain of kills in a row: 2 double, 3 triple, 4 and up a rampage.
    Chain(u8),
    /// A streak without dying: 3, 5 or 7.
    Spree(u8),
    /// The knight ended another's streak of this many.
    Shutdown(u8),
    Revenge,
    /// The knight took the lead.
    Lead,
    /// One kill from the goal.
    Point,
}

impl Cry {
    /// How loudly it is shown: the banner with the higher rank stays on top.
    pub fn rank(self) -> u8 {
        match self {
            Cry::FirstBlood => 4,
            Cry::Chain(n) => 5 + n.min(4),
            Cry::Spree(n) => 4 + n.min(7) / 2,
            Cry::Shutdown(_) => 5,
            Cry::Revenge => 4,
            Cry::Lead => 3,
            Cry::Point => 8,
        }
    }
}

/// A banner called for a knight at a tick.
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

/// The running tallies a kill is judged against.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Tally {
    /// Kills in a row without dying.
    run: BTreeMap<u32, u16>,
    /// The most in a row.
    best: BTreeMap<u32, u16>,
    /// The last kill's tick and the chain's length.
    chain: BTreeMap<u32, (u32, u8)>,
    /// Who last killed each knight (revenge is owed them).
    nemesis: BTreeMap<u32, u32>,
    /// Knights who have already had their match point called.
    pointed: Vec<u32>,
    opened: bool,
    lead: Option<u32>,
}

/// The kit's whole state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Juice {
    pub blows: Vec<Blow>,
    pub calls: Vec<Call>,
    tally: Tally,
}

impl Juice {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.blows.len() <= MOST_BLOWS
            && self.calls.len() <= MOST_CALLS
            && self.blows.iter().all(|b| seat(&b.owner) && seat(&b.target) && [b.x, b.y, b.from.0, b.from.1].iter().all(|v| v.is_finite()))
            && self.calls.iter().all(|c| seat(&c.who))
            && self.tally.run.len() <= MAX_PLAYERS as usize
            && self.tally.best.len() <= MAX_PLAYERS as usize
            && self.tally.chain.len() <= MAX_PLAYERS as usize
            && self.tally.nemesis.len() <= MAX_PLAYERS as usize
            && self.tally.pointed.len() <= MAX_PLAYERS as usize
    }
    /// Let go what is old at `now`.
    pub fn tick(&mut self, now: u32) {
        self.blows.retain(|b| b.age(now) < BLOW_LIFE);
        self.calls.retain(|c| c.age(now) < CALL_LIFE);
    }
    /// A blow landed.
    pub fn blow(&mut self, blow: Blow) {
        if self.blows.len() >= MOST_BLOWS {
            self.blows.remove(0);
        }
        self.blows.push(blow);
    }
    /// The knight's streak now, and the best they ran.
    pub fn streak(&self, id: u32) -> u16 {
        self.tally.run.get(&id).copied().unwrap_or(0)
    }
    pub fn best(&self, id: u32) -> u16 {
        self.tally.best.get(&id).copied().unwrap_or(0)
    }
    /// A kill scored: `killer` took `victim` at `now`; `scores` are every
    /// knight's kills after it and `goal` the number that wins. The calls are
    /// pushed and returned, loudest last.
    pub fn kill(&mut self, now: u32, killer: u32, victim: u32, scores: &[(u32, u16)], goal: u16) -> Vec<Call> {
        let t = &mut self.tally;
        let mut cries = Vec::new();
        if !t.opened {
            t.opened = true;
            cries.push((Cry::FirstBlood, killer));
        }
        let (last, n) = t.chain.get(&killer).copied().unwrap_or((0, 0));
        let n = if n > 0 && now.saturating_sub(last) <= CHAIN_TICKS { n + 1 } else { 1 };
        t.chain.insert(killer, (now, n));
        if n >= 2 {
            cries.push((Cry::Chain(n), killer));
        }
        let ended = t.run.insert(victim, 0).unwrap_or(0);
        if ended >= 3 {
            cries.push((Cry::Shutdown(ended.min(255) as u8), killer));
        }
        let run = t.run.entry(killer).or_default();
        *run += 1;
        let run = *run;
        let best = t.best.entry(killer).or_default();
        *best = (*best).max(run);
        if matches!(run, 3 | 5 | 7) {
            cries.push((Cry::Spree(run as u8), killer));
        }
        if t.nemesis.get(&killer) == Some(&victim) {
            t.nemesis.remove(&killer);
            cries.push((Cry::Revenge, killer));
        }
        t.nemesis.insert(victim, killer);
        let top = scores.iter().map(|&(_, k)| k).max().unwrap_or(0);
        let leaders: Vec<u32> = scores.iter().filter(|&&(_, k)| k == top && top > 0).map(|&(id, _)| id).collect();
        let lead = (leaders.len() == 1).then(|| leaders[0]);
        if lead == Some(killer) && t.lead != lead && top >= 2 {
            cries.push((Cry::Lead, killer));
        }
        if lead.is_some() {
            t.lead = lead;
        }
        if goal > 1 && scores.iter().any(|&(id, k)| id == killer && k + 1 == goal) && !t.pointed.contains(&killer) {
            t.pointed.push(killer);
            cries.push((Cry::Point, killer));
        }
        cries.sort_by_key(|(c, _)| c.rank());
        let calls: Vec<Call> = cries.into_iter().map(|(cry, who)| Call { cry, who, born: now }).collect();
        for c in &calls {
            if self.calls.len() >= MOST_CALLS {
                self.calls.remove(0);
            }
            self.calls.push(*c);
        }
        calls
    }
}

impl Run {
    /// Hold the room's time for a blow of `weight`: the realm's own hit-stop
    /// (`hitstop`), so a cooldown keeps a crowded room moving and a kill or a
    /// win always lands. The pause begins when `finish_hit_hold` runs at the
    /// end of the game's tick.
    pub fn hold_for(&mut self, weight: Weight) {
        if let Some(impact) = weight.impact() {
            self.hitstop.request(&self.tune, impact);
        }
    }
    /// Start the hold `hold_for` asked for: call once at the end of the tick.
    pub fn finish_hit_hold(&mut self) {
        self.finish_hitstop();
    }
}

#[cfg(test)]
#[path = "../../tests/rules/juice__tests.rs"]
mod tests;
