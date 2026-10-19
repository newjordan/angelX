//! Latch Duel: read the hook, catch the closing beat, buckle the bellows.
//! First ten seconds: a high hook and a low rake make the first counter physical.
//! Five live numbers: latch_tell=36 ticks, latch_window=7 ticks,
//! latch_rest=24 ticks, latch_recover=18 ticks, latch_stop=4 ticks.
//! Version one leaves out bullets, free movement, weapons and a second room.
//! Full feel brief: assets/latch/BRIEF.md. Every seat gets the same attack deck.
use super::{BTreeMap, HZ, Input, MAX_PLAYERS, RoomKind, Run, Spoil};
use serde::{Deserialize, Serialize};

/// Longer makes the commitment easier to read; even the minimum is 0.77s.
pub const TELL: u32 = 36;
/// Larger forgives more early timing inside the final wind-up beat.
pub const WINDOW: u32 = 7;
/// Longer gives a breath before the next commitment.
pub const REST: u32 = 24;
/// Longer lets the fail or clean contact settle before retry.
pub const RECOVER: u32 = 18;
/// A local freeze, never a pause of another knight's bout.
pub const STOP: u32 = 4;
pub const LENGTH: u32 = 45 * HZ;
pub const GOLD_PER_SEAT: u32 = 10;
pub const BAY: (f32, f32) = (31.0, 18.0);

/// Summed timing error: one tick remains visible as +33MS on the podium.
pub fn timing(ticks: u32) -> String {
    format!("+{}MS", (u64::from(ticks) * 1000 + u64::from(HZ) / 2) / u64::from(HZ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Beat {
    Rest,
    Tell,
    Clean,
    Miss,
    Defeat,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duel {
    pub seed: u64,
    pub attack: u32,
    /// -1 overhead hook, +1 floor rake. The guard uses the same convention.
    pub height: i8,
    pub guard: i8,
    pub beat: Beat,
    pub ticks: u32,
    pub windup: u32,
    pub freeze: u32,
    pub counters: u8,
    pub misses: u32,
    /// Sum of ticks before contact, so a smaller error wins a close race.
    pub error: u32,
    pub chain: u8,
    pub fire_key: bool,
    /// Absolute run tick at the start of this tell, retained through its miss.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tell_at: Option<u64>,
    /// A fresh attempt consumes this tell, including an early/wrong-height one.
    #[serde(default)]
    pub attempted: bool,
    /// Chain before an automatic miss; restored if its on-time press arrives late.
    #[serde(default)]
    pub miss_chain: u8,
}
impl Duel {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            attack: 0,
            height: -1,
            guard: -1,
            beat: Beat::Rest,
            ticks: REST,
            windup: TELL,
            freeze: 0,
            counters: 0,
            misses: 0,
            error: 0,
            chain: 0,
            fire_key: false,
            tell_at: None,
            attempted: false,
            miss_chain: 0,
        }
    }
    fn next_height(&self) -> i8 {
        // Each two-attack phrase contains both heights; its order is seeded.
        let phrase = (self.attack / 2) as u64;
        let bit = ((self.seed.rotate_left((phrase % 64) as u32) ^ phrase) & 1) as u32;
        if (self.attack % 2) ^ bit == 0 { -1 } else { 1 }
    }
    pub fn valid(&self) -> bool {
        [-1, 1].contains(&self.height)
            && [-1, 1].contains(&self.guard)
            && self.counters <= 3
            && self.chain <= self.counters
            && self.miss_chain <= self.counters
            && self.ticks <= 90
            && (23..=60).contains(&self.windup)
            && self.freeze <= 8
            && self.attack <= LENGTH
            && self.misses <= LENGTH
            && self.error <= 3 * 15
            && match self.beat {
                Beat::Tell => self.ticks > 0 && self.ticks <= self.windup,
                Beat::Rest | Beat::Miss => self.ticks > 0,
                Beat::Clean => self.ticks > 0 && self.counters > 0,
                Beat::Defeat => self.counters == 3,
            }
    }
    fn fail(&mut self, tune: &super::tune::Tune) -> Option<&'static str> {
        self.misses += 1;
        self.miss_chain = self.chain;
        self.chain = 0;
        self.beat = Beat::Miss;
        self.ticks = tune.latch_recover;
        Some("hit")
    }
    pub fn step(&mut self, input: Input, tune: &super::tune::Tune) -> Option<&'static str> {
        self.step_at(input, tune, None, 0)
    }
    fn clean(&mut self, remaining: u32, tune: &super::tune::Tune) -> Option<&'static str> {
        self.attempted = true;
        self.counters += 1;
        self.chain += 1;
        self.error += remaining - 1;
        self.beat = Beat::Clean;
        self.ticks = tune.latch_recover;
        self.freeze = tune.latch_stop;
        Some("clang")
    }
    /// Sample ticks refer to the state the player saw, rather than packet arrival.
    /// The existing miss recovery retains the tell, so a delayed valid press can
    /// replace its automatic miss without extending any window or recovery.
    pub fn step_at(&mut self, input: Input, tune: &super::tune::Tune, now: Option<u64>, allowance: u64) -> Option<&'static str> {
        let input = if input.valid() {
            input
        } else {
            Input::default()
        };
        let mut counter = input.fire && !self.fire_key;
        self.fire_key = input.fire;
        if input.move_y != 0 {
            self.guard = input.move_y;
        }
        if counter && input.latch_tick.is_some() {
            counter = false; // A stamped press is never rejudged at arrival time.
            if let (Some(sample), Some(start), Some(now)) = (input.latch_tick, self.tell_at, now)
                && sample >= start && sample <= now && now - sample <= allowance
                && !self.attempted && matches!(self.beat, Beat::Tell | Beat::Miss)
            {
                let elapsed = sample - start;
                let remaining = u64::from(self.windup).saturating_sub(elapsed);
                self.attempted = true;
                if remaining > 0 && remaining <= u64::from(tune.latch_window) && self.guard == self.height {
                    if self.beat == Beat::Miss {
                        self.chain = self.miss_chain;
                        self.misses -= 1; // Undo only the automatic expiry of this tell.
                    }
                    return self.clean(remaining as u32, tune);
                }
                if self.beat == Beat::Tell {
                    return self.fail(tune);
                }
            }
        }
        if self.freeze > 0 {
            self.freeze -= 1;
            return None;
        }
        match self.beat {
            Beat::Defeat => return None,
            Beat::Tell => {
                if counter {
                    if self.ticks <= tune.latch_window && self.guard == self.height {
                        return self.clean(self.ticks, tune);
                    }
                    self.attempted = true;
                    return self.fail(tune);
                }
                self.ticks = self.ticks.saturating_sub(1);
                if self.ticks == 0 {
                    return self.fail(tune);
                }
            }
            Beat::Rest | Beat::Clean | Beat::Miss => {
                self.ticks = self.ticks.saturating_sub(1);
                if self.ticks == 0 {
                    if self.counters == 3 {
                        self.beat = Beat::Defeat;
                        return Some("spoil_pickup");
                    }
                    if self.beat == Beat::Rest || self.beat == Beat::Miss {
                        self.height = self.next_height();
                        self.attack += 1;
                        self.beat = Beat::Tell;
                        self.tell_at = now.map(|tick| tick + 1);
                        self.attempted = false;
                        self.windup = tune.latch_tell;
                        self.ticks = self.windup;
                        return Some("play_card");
                    }
                    self.beat = Beat::Rest;
                    self.ticks = tune.latch_rest.saturating_sub(u32::from(self.counters) * 3);
                }
            }
        }
        None
    }
    /// A late practice bot waits until the published sweep has already landed.
    pub fn late_bot_input(&self, recover: u32) -> Input {
        Input {
            move_y: self.height,
            fire: self.beat == Beat::Miss && self.ticks == recover && !self.fire_key,
            ..Default::default()
        }
    }
    /// A bot reads only the published tell; different seats can choose timing.
    pub fn bot_input(&self, early: u32) -> Input {
        Input {
            move_y: self.height,
            fire: self.beat == Beat::Tell && self.ticks <= early.max(1) && !self.fire_key,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub duels: BTreeMap<u32, Duel>,
    pub seed: u64,
    pub left: u32,
    pub order: Vec<u32>,
    pub key: bool,
    pub won: bool,
    /// Keep the final contact and freeze on screen before the podium.
    pub finish: Option<u32>,
    /// The common winning input tick, independent of subsequent live knob edits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_at: Option<u64>,
}
impl Session {
    pub fn new(seats: impl Iterator<Item = u32>, seed: u64) -> Self {
        Self {
            duels: seats.map(|id| (id, Duel::new(seed))).collect(),
            seed,
            left: LENGTH,
            order: Vec::new(),
            key: true,
            won: false,
            finish: None,
            finish_at: None,
        }
    }
    pub fn score(&self, id: u32) -> (std::cmp::Reverse<u8>, u32, u32) {
        let d = &self.duels[&id];
        (std::cmp::Reverse(d.counters), d.misses, d.error)
    }
    pub fn place(&self, id: u32) -> usize {
        1 + self
            .duels
            .keys()
            .filter(|&&r| self.score(r) < self.score(id))
            .count()
    }
    pub fn payout(&self, id: u32) -> u32 {
        let Some(&first) = self.order.first() else {
            return 0;
        };
        if self.left > 0 || self.score(id) != self.score(first) {
            return 0;
        }
        let tied = self
            .duels
            .keys()
            .filter(|&&r| self.score(r) == self.score(first))
            .count() as u32;
        let before = self
            .duels
            .range(..id)
            .filter(|&(r, _)| self.score(*r) == self.score(first))
            .count() as u32;
        let total = GOLD_PER_SEAT * self.duels.len() as u32;
        total / tied + u32::from(before < total % tied)
    }
    pub fn valid(&self) -> bool {
        !self.duels.is_empty()
            && self.duels.len() <= MAX_PLAYERS as usize
            && self
                .duels
                .iter()
                .all(|(&id, d)| (1..=MAX_PLAYERS).contains(&id) && d.valid() && d.seed == self.seed)
            && self.left <= LENGTH
            && self.finish.is_none_or(|t| t <= 35)
            && self.order.len() <= self.duels.len()
            && self
                .order
                .iter()
                .enumerate()
                .all(|(n, id)| self.duels.contains_key(id) && !self.order[..n].contains(id))
            && (self.left > 0 || self.order.len() == self.duels.len())
    }
    pub fn repair(&mut self) {
        self.duels.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        for d in self.duels.values_mut() {
            if !d.valid() || d.seed != self.seed {
                *d = Duel::new(self.seed);
            }
        }
        if self.duels.is_empty() {
            self.duels.insert(1, Duel::new(self.seed));
        }
        self.left = LENGTH;
        self.order.clear();
        self.key = true;
        self.won = false;
        self.finish = None;
        self.finish_at = None;
    }
}
impl Run {
    pub fn latch_showing(&self) -> bool {
        self.room().kind == RoomKind::Smithy && self.latch.is_some()
    }
    pub(super) fn tick_latch(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Smithy {
            self.latch = None;
            return;
        }
        let pressed = inputs.iter().any(|(id, i)| {
            i.valid()
                && i.interact
                && self.players.get(id).is_some_and(|h| {
                    h.hp > 0
                        && !h.stone
                        && !h.away
                        && !self.exits.outside.contains_key(id)
                        && (h.x - BAY.0).hypot(h.y - BAY.1) < 3.0
                })
        });
        if self.latch.is_none() {
            if pressed {
                self.start_latch();
            }
            return;
        }
        let mut s = self.latch.take().unwrap();
        s.duels
            .retain(|id, _| self.players.contains_key(id) && !self.exits.outside.contains_key(id));
        s.order.retain(|id| s.duels.contains_key(id));
        if s.duels.is_empty() {
            return;
        }
        if s.left == 0 {
            let take = inputs
                .iter()
                .any(|(id, i)| s.duels.contains_key(id) && i.valid() && i.interact);
            if take && !s.key {
                self.start_latch();
            } else {
                s.key = take;
                self.latch = Some(s);
            }
            return;
        }
        s.left -= 1;
        let finishing = s.finish.is_some();
        for (&id, d) in &mut s.duels {
            if self.players.get(&id).is_none_or(|h| h.away || h.stone) {
                continue;
            }
            let mut input = inputs.get(&id).copied().unwrap_or_default();
            // The winning tick is common to all seats. The last contact is visual only.
            let on_winning_tick = s.finish_at.is_some_and(|winning_tick| {
                input.latch_tick.is_some_and(|sample| sample <= winning_tick)
            });
            if finishing && !on_winning_tick && !matches!(d.beat, Beat::Clean | Beat::Defeat) {
                continue;
            }
            if finishing && !on_winning_tick {
                input.fire = false;
            }
            if let Some(sound) = d.step_at(input, &self.tune, Some(self.tick), self.latch_input_age.get(&id).copied().unwrap_or(0)) {
                self.sounds.push(sound);
            }
        }
        if let Some(t) = s.finish.as_mut() {
            *t = t.saturating_sub(1);
        }
        if s.finish.is_none() && s.duels.values().any(|d| d.counters == 3) {
            s.finish = Some(self.tune.latch_stop + self.tune.latch_recover);
            s.finish_at = Some(self.tick);
        }
        if s.finish == Some(0) || s.left == 0 {
            s.order = s.duels.keys().copied().collect();
            s.order.sort_by_key(|id| {
                let d = &s.duels[id];
                (std::cmp::Reverse(d.counters), d.misses, d.error, *id)
            });
            s.left = 0;
            s.key = true;
            s.won = s.duels.values().any(|d| d.counters == 3);
            let record = &mut self.home.countryside.games.latch;
            record.best = record.best.max(u16::from(s.duels[&s.order[0]].counters));
            if s.won {
                record.wins = record.wins.saturating_add(1).min(record.plays);
            }
            for (&id, h) in &mut self.players {
                if s.duels.contains_key(&id) {
                    h.laurel = s.place(id) == 1;
                    h.carried.add(Spoil::Gold, s.payout(id));
                    if h.laurel {
                        h.booby = None;
                    }
                }
            }
            self.sounds.push("spoil_pickup");
        }
        self.latch = Some(s);
    }
    fn start_latch(&mut self) {
        let seed = self.rng.next();
        let seats = self
            .players
            .iter()
            .filter(|(id, h)| {
                h.hp > 0 && !h.stone && !h.away && !self.exits.outside.contains_key(id)
            })
            .map(|(&id, _)| id);
        let session = Session::new(seats, seed);
        if session.duels.is_empty() {
            return;
        }
        self.latch = Some(session);
        self.home.countryside.games.latch.plays =
            self.home.countryside.games.latch.plays.saturating_add(1);
        self.sounds.push("play_card");
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn latch_for_test() -> Run {
        let mut run = Run::at_home(
            41,
            1,
            Some("Gawain"),
            Default::default(),
            Default::default(),
        );
        let at = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Smithy)
            .unwrap();
        run.arrive(at, (BAY.0 / 2.0, BAY.1 / 2.0));
        run
    }
}
