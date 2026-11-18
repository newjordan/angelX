//! Moonfall: three days in a fen town before the Gloam Moon falls, lived
//! over and over. The party shares one clock and one town; ringing the
//! Moonwell Bell rewinds all of it to the first dawn, and the Ledger, the
//! masks and everything learned stay.
//!
//! This is the brief, kept at the top (assets/moonfall/BRIEF.md says the
//! rest and is re-read before every change):
//!
//! 1. What a knight feels in the first ten seconds: the dawn bell, a sky
//!    with a great sleeping face in it, a baker already at her counter, a
//!    clock bar showing the first of seventy-two hours.
//! 2. The five numbers: `moonfall_day` 3000 ticks to a day, `moonfall_walk`
//!    150 percent (a knight against a villager), `moonfall_wisps` 4 after
//!    dusk, `moonfall_rewind` 40 ticks, `moonfall_notice` 64 pixels.
//! 3. What version 1 leaves out: no combat, no interiors, no music, no
//!    painted art (labelled blocks only, `slots.json` names every slot).
//!
//! The classic loop it honours: the town runs on a clock that does not wait;
//! people have routines you can learn, follow and cut off; a notebook that
//! writes itself is the real inventory; the bell winds time back and keeps
//! what you learned. Co-op is a layer: every knight has the whole verb set
//! (walk, talk, Ledger, bell, masks, sidestep), the clock and town are
//! shared, favours run in parallel, wisps grow with the table, never fewer.
//!
//! Integer only: positions are pixels times 256 on a 16-pixel tile map, the
//! clock is ticks, folk are pure functions of the clock.
//!
//! The rules (the tick, the favours, the bell, the wisps, the bots) are the Bend
//! module `cockpit/live/moonfall-rules`; this folder keeps the session and its
//! saves, the words, the town's map for the drawing, and `port.rs`, which asks
//! the module.
pub mod clock;
pub mod folk;
pub mod ledger;
pub mod port;
pub mod says;
pub mod state;
pub mod story;
pub mod town;

use super::activity::{Activity, Realm, Settled};
use super::roundtable::{Adventure, Number};
use super::*;
use folk::Folk;
use ledger::Memory;
use says::Topic;
use state::{
    CANDLES, Call, DOWN, Knight, Note, Nums, PUSH_TICKS, Panel, Phase, ROLL, ROLL_REST, Ring, SPAWN, Seat, World,
};
use town::Q;

pub const NAME: &str = "Moonfall";
pub const ENTRY: (i32, i32) = (78, 92);
pub const SLOT: usize = 1;
pub const CAMPAIGN_MARK: &str = "moonfall:campaign-begun";
pub const PARTY_MARKS: [&str; 6] = [
    "moonfall:party-1",
    "moonfall:party-2",
    "moonfall:party-3",
    "moonfall:party-4",
    "moonfall:party-5",
    "moonfall:party-6",
];
pub const MARKS: &[&str] = &[
    CAMPAIGN_MARK,
    PARTY_MARKS[0],
    PARTY_MARKS[1],
    PARTY_MARKS[2],
    PARTY_MARKS[3],
    PARTY_MARKS[4],
    PARTY_MARKS[5],
];
pub const NUMBERS: [Number; 5] = [
    Number { name: "moonfall_day", label: "Ticks in a day", constant: "moonfall::NUMBERS[0]", min: 1800, max: 6000, start: 3000 },
    Number { name: "moonfall_walk", label: "Knight speed, % of folk", constant: "moonfall::NUMBERS[1]", min: 100, max: 300, start: 150 },
    Number { name: "moonfall_wisps", label: "Wisps after dusk", constant: "moonfall::NUMBERS[2]", min: 0, max: 8, start: 4 },
    Number { name: "moonfall_rewind", label: "Rewind ritual ticks", constant: "moonfall::NUMBERS[3]", min: 20, max: 44, start: 40 },
    Number { name: "moonfall_notice", label: "Notice radius, pixels", constant: "moonfall::NUMBERS[4]", min: 24, max: 160, start: 64 },
];
pub const ADVENTURE: Adventure = Adventure {
    slug: "moonfall",
    name: NAME,
    entry: ENTRY,
    slot: SLOT,
    marks: MARKS,
    numbers: &NUMBERS,
    showing: Run::moonfall_showing,
    start: Run::start_moonfall,
    valid: |run| run.moonfall.as_ref().is_none_or(Session::valid),
    clear: |run| run.moonfall = None,
    seated: |run| {
        run.moonfall
            .as_ref()
            .map_or_else(Vec::new, |s| s.seats.keys().copied().collect())
    },
    bot: |run, id| match (run.moonfall.as_ref(), run.players.get(&id)) {
        (Some(s), Some(hero)) => s.bot_input(id, hero),
        _ => Input::default(),
    },
};
pub const ACTIVITY: Activity = Activity {
    name: NAME,
    on: Run::moonfall_showing,
    ride: Some(|run, inputs| {
        run.tick_moonfall(inputs);
        Some(BTreeMap::new())
    }),
    tick: |_, _| {},
    end: |run| run.moonfall = None,
    ends_outside: true,
    settle: Some(settle),
    fields: &["moonfall"],
};
pub const BEGUN: u32 = 1;
pub const GOLD_PER_SEAT: u32 = 30;
pub const WAX_PER_SEAT: u32 = 1;

fn settle(realm: &mut Realm<'_>, marks: &BTreeMap<String, u32>) -> Settled {
    let changed = marks.contains_key(CAMPAIGN_MARK) && realm.home.roundtable[SLOT] & BEGUN == 0;
    let mut said = Vec::new();
    if changed {
        realm.home.roundtable[SLOT] |= BEGUN;
        let seats = PARTY_MARKS
            .iter()
            .rposition(|m| marks.contains_key(*m))
            .map_or(1, |i| i as u32 + 1);
        realm.treasury.add(Spoil::Gold, GOLD_PER_SEAT * seats);
        realm.treasury.add(Spoil::Wax, WAX_PER_SEAT * seats);
        said.push((
            "moonfall".into(),
            format!(
                "{NAME}: the moon turns back. {} gold and {} wax for {seats} at the table.",
                GOLD_PER_SEAT * seats,
                WAX_PER_SEAT * seats
            ),
        ));
    }
    Settled { changed, said }
}

pub const LIMIT: u32 = 60 * 60 * HZ;
pub const SKIP_LEN: u32 = 36;
pub const WON_LEN: u32 = 120;
pub const CALL_TICKS: u32 = 150;
const DAY_MAX: u32 = NUMBERS[0].max;

pub fn nums_of(tune: &tune::Tune) -> Nums {
    let n = |i| roundtable::number(tune, SLOT, i);
    Nums { day: n(0), walk: n(1), wisps: n(2), rewind: n(3), notice: n(4) }
}

pub fn nums_start() -> Nums {
    let n = |i: usize| NUMBERS[i].start;
    Nums { day: n(0), walk: n(1), wisps: n(2), rewind: n(3), notice: n(4) }
}

pub fn control(nums: &Nums) -> u32 {
    (nums.rewind * 7 / 10).max(1)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seats: BTreeMap<u32, Seat>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parked: BTreeMap<u32, Seat>,
    pub elapsed: u32,
    pub clock: u32,
    pub phase: Phase,
    pub card: u32,
    pub world: World,
    pub memory: Memory,
    pub call: Option<Call>,
    pub cry: Option<(Folk, Topic, u32)>,
    pub news: Vec<(Note, u32)>,
    pub completed: Option<u32>,
    pub peak: u32,
    pub nums: Nums,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::symbols"
    )]
    pub pending: Vec<&'static str>,
}

impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Self {
        let mut s = Self {
            seats: BTreeMap::new(),
            parked: BTreeMap::new(),
            elapsed: 0,
            clock: 0,
            phase: Phase::Play,
            card: 0,
            world: World::dawn(),
            memory: Memory::default(),
            call: None,
            cry: None,
            news: Vec::new(),
            completed: None,
            peak: 0,
            nums: nums_start(),
            pending: Vec::new(),
        };
        s.card = s.nums.rewind;
        for (&id, _) in players.iter().filter(|(_, h)| !h.away && !h.stone) {
            let at = SPAWN[(id as usize + 5) % SPAWN.len()];
            s.seats.insert(id, Seat { joined: 0, knight: Knight::at(at) });
        }
        s.peak = s.seats.len() as u32;
        s
    }

    pub fn shown_clock(&self) -> u32 {
        let ease = |t: u32, len: u32| -> i64 {
            let x = i64::from(t.min(len)) * 256 / i64::from(len.max(1));
            x * x * (768 - 2 * x) / (256 * 256)
        };
        match self.phase {
            Phase::Rewind { t, from, .. } => {
                let e = ease(t, control(&self.nums));
                (i64::from(from) * (256 - e) / 256) as u32
            }
            Phase::Skip { t, from, to } => {
                let e = ease(t, SKIP_LEN);
                (i64::from(from) + (i64::from(to) - i64::from(from)) * e / 256) as u32
            }
            _ => self.clock,
        }
    }

    pub fn bell_need(&self) -> u32 {
        self.seats.len() as u32 / 2 + 1
    }

    pub fn ring_ok(&self, ring: Ring) -> bool {
        match ring {
            Ring::Rewind => true,
            Ring::Dusk => clock::next_dusk(self.clock, self.nums.day) <= clock::final_start(self.nums.day),
            Ring::Dawn => clock::next_dawn(self.clock, self.nums.day) <= clock::final_start(self.nums.day),
        }
    }

    pub fn valid(&self) -> bool {
        let n = &self.nums;
        let end = clock::end(n.day);
        NUMBERS.iter().zip([n.day, n.walk, n.wisps, n.rewind, n.notice]).all(|(num, v)| (num.min..=num.max).contains(&v))
            && self.elapsed <= LIMIT
            && self.clock <= end.max(clock::end(DAY_MAX))
            && self.clock <= end
            && self.card <= 120
            && self.peak <= MAX_PLAYERS
            && self.seats.len() <= MAX_PLAYERS as usize
            && self.peak as usize >= self.seats.len()
            && self.news.len() <= 3
            && self.completed.is_none_or(|t| t <= self.elapsed)
            && self.world.ink_given.is_none_or(|t| t <= end)
            && self.world.shod.is_none_or(|t| t <= end)
            && self.world.lit_until.iter().all(|&t| t <= end * 2)
            && match self.phase {
                Phase::Play => true,
                Phase::Rewind { t, from, .. } => t <= control(n) && from <= end,
                Phase::Skip { t, from, to } => t <= SKIP_LEN && from <= end && to <= end && from <= to,
                Phase::Won { t } => t <= WON_LEN,
            }
            && self.call.is_none_or(|c| c.ayes < 64 && c.by <= MAX_PLAYERS && c.left <= CALL_TICKS)
            && self.cry.is_none_or(|(_, _, age)| age <= 151)
            && self.world.valid()
            && self.memory.valid()
            && self.parked.len() + self.seats.len() <= MAX_PLAYERS as usize
            && self.parked.keys().all(|id| !self.seats.contains_key(id))
            && self.seats.iter().chain(self.parked.iter()).all(|(&id, s)| {
                (1..=MAX_PLAYERS).contains(&id) && s.joined <= self.elapsed && knight_valid(&s.knight)
            })
    }

    pub fn repair(&mut self) {
        self.seats.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        self.parked.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id) && !self.seats.contains_key(&id));
        self.elapsed = self.elapsed.min(LIMIT);
        for (field, num) in [
            (&mut self.nums.day, &NUMBERS[0]),
            (&mut self.nums.walk, &NUMBERS[1]),
            (&mut self.nums.wisps, &NUMBERS[2]),
            (&mut self.nums.rewind, &NUMBERS[3]),
            (&mut self.nums.notice, &NUMBERS[4]),
        ] {
            *field = (*field).clamp(num.min, num.max);
        }
        let end = clock::end(self.nums.day);
        self.clock = self.clock.min(end);
        self.card = self.card.min(120);
        self.peak = self.peak.clamp(self.seats.len() as u32, MAX_PLAYERS);
        self.news.truncate(3);
        if !matches!(self.phase, Phase::Play) {
            self.phase = Phase::Play;
        }
        if self.call.is_some_and(|c| c.ayes >= 64 || c.by > MAX_PLAYERS || c.left > CALL_TICKS) {
            self.call = None;
        }
        if self.cry.is_some_and(|(_, _, age)| age > 151) {
            self.cry = None;
        }
        if self.completed.is_some_and(|t| t > self.elapsed) {
            self.completed = None;
        }
        if !self.world.valid() {
            self.world = World::dawn();
        }
        self.memory.repair();
        let elapsed = self.elapsed;
        for (&id, seat) in self.seats.iter_mut().chain(self.parked.iter_mut()) {
            seat.joined = seat.joined.min(elapsed);
            if !knight_valid(&seat.knight) {
                seat.knight = Knight::at(SPAWN[(id as usize + 5) % SPAWN.len()]);
            }
        }
    }
}

fn knight_valid(k: &Knight) -> bool {
    (0..=town::COLS * town::TILE * Q).contains(&k.x)
        && (0..=town::ROWS * town::TILE * Q).contains(&k.y)
        && k.face < 4
        && k.candles <= CANDLES
        && k.hurt <= 120
        && k.down <= DOWN
        && k.roll <= ROLL
        && k.roll_rest <= ROLL_REST
        && (-1..=1).contains(&k.roll_dir.0)
        && (-1..=1).contains(&k.roll_dir.1)
        && k.hold <= story::LIGHT_HOLD
        && k.push <= PUSH_TICKS
        && k.stride <= 60_000
        && k.read <= 400
        && k.held_at <= clock::end(DAY_MAX)
        && match k.panel {
            Panel::Shut => true,
            Panel::Ledger(p) => p < 7,
            Panel::Bell(l) => l < 3,
        }
        && k.say.is_none_or(|s| s.page < 8 && s.age <= 361)
        && k.note.is_none_or(|(_, left)| left <= 120)
}

impl Run {
    pub fn moonfall_showing(&self) -> bool {
        self.on_stage() && self.moonfall.is_some()
    }
    pub fn start_moonfall(&mut self) {
        if !self.on_stage() {
            return;
        }
        self.forest.leave();
        let mut s = Session::new(&self.players);
        s.nums = nums_of(&self.tune);
        s.card = s.nums.rewind;
        self.moonfall = Some(s);
        self.found = Some((self.tick, 0, format!("{NAME}: hold Esc to return.")));
        self.sounds.push("circle_toll");
    }
    pub(super) fn tick_moonfall(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut s) = self.moonfall.take() else {
            return;
        };
        s.nums = nums_of(&self.tune);
        let present = self
            .players
            .iter()
            .filter(|(id, _)| !self.exits.outside.contains_key(id))
            .map(|(&id, hero)| (id, hero.clone()))
            .collect();
        let ticked = s.advance(inputs, &present).unwrap_or_default();
        for cue in ticked.cues {
            self.sounds.push(cue);
        }
        if let Some((seats, best)) = ticked.won {
            self.marks.insert(CAMPAIGN_MARK.into(), 1);
            self.marks.insert(PARTY_MARKS[seats.clamp(1, 6) as usize - 1].into(), 1);
            self.home.roundtable[SLOT] |= BEGUN;
            let score = &mut self.home.countryside.games.roundtable[SLOT];
            score.best = score.best.max(best);
            score.wins += 1;
            score.plays = score.plays.max(score.wins);
            self.found = Some((self.tick, 0, "The moon turns back! Moonfall stopped. E to return.".into()));
            self.sounds.push("play_card");
        }
        self.moonfall = Some(s);
        for id in ticked.leaving {
            self.leave_activity(id);
        }
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn moonfall_for_test() -> Self {
        roundtable::fixture(&ADVENTURE)
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_moonfall__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-games-b"))]
#[path = "../../tests/rules/moonfall_bend_cases__tests.rs"]
mod moonfall_bend_cases_tests;
