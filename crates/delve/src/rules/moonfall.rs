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
pub mod clock;
pub mod folk;
pub mod ledger;
pub mod plan;
pub mod says;
pub mod state;
pub mod story;
pub mod town;
pub mod walk;
pub mod wisps;

use super::activity::{Activity, Realm, Settled};
use super::roundtable::{Adventure, Number};
use super::*;
use folk::{CAST, Folk};
use ledger::Memory;
use says::Topic;
use state::{
    CANDLES, Call, DITCH_FILLED, INK_TAKEN, Item, Knight, Mask, Note, Nums, Panel, Phase, Ring, SPAWN, Seat, World,
};
use town::Q;
use walk::{key, keys_of, pressed};

pub const NAME: &str = "Moonfall";
/// The kingdom tile its door stands on: walk up and press E.
pub const ENTRY: (i32, i32) = (78, 92);
/// Its place in `roundtable::ADVENTURES` and in `Tune::roundtable`.
pub const SLOT: usize = 1;
/// What a finished run saves to the Home, as the Forest Road's
/// `CAMPAIGN_MARK` does. Marks are settled at the save (`settle`).
pub const CAMPAIGN_MARK: &str = "moonfall:campaign-begun";
/// The table that saved the town, by its seats: the purse is for them.
pub const PARTY_MARKS: [&str; 6] = [
    "moonfall:party-1",
    "moonfall:party-2",
    "moonfall:party-3",
    "moonfall:party-4",
    "moonfall:party-5",
    "moonfall:party-6",
];
/// Every mark this adventure may write: the crash-safe intent journal keeps
/// exactly these (`roundtable::intent_key`).
pub const MARKS: &[&str] = &[
    CAMPAIGN_MARK,
    PARTY_MARKS[0],
    PARTY_MARKS[1],
    PARTY_MARKS[2],
    PARTY_MARKS[3],
    PARTY_MARKS[4],
    PARTY_MARKS[5],
];
/// The five numbers (the brief's "five numbers"). `Tune::roundtable[SLOT]`
/// holds them; each is a live knob on the F2 panel.
pub const NUMBERS: [Number; 5] = [
    // Ticks in a day: the loop is three of them. Longer, and the folk are
    // easier to catch up with; shorter, and the favours press.
    Number { name: "moonfall_day", label: "Ticks in a day", constant: "moonfall::NUMBERS[0]", min: 1800, max: 6000, start: 3000 },
    // A knight's pace against a villager's, in percent. Higher, and
    // intercepting is easy; lower, and you must plan the walk.
    Number { name: "moonfall_walk", label: "Knight speed, % of folk", constant: "moonfall::NUMBERS[1]", min: 100, max: 300, start: 150 },
    // Wisps after dusk for one knight; two more for each knight beyond.
    Number { name: "moonfall_wisps", label: "Wisps after dusk", constant: "moonfall::NUMBERS[2]", min: 0, max: 8, start: 4 },
    // Ticks the rewind ritual takes: the clock spins back, the bell tolls,
    // the card fades in. Control returns at seven-tenths of it.
    Number { name: "moonfall_rewind", label: "Rewind ritual ticks", constant: "moonfall::NUMBERS[3]", min: 20, max: 44, start: 40 },
    // How near a knight must be to witness: the Ledger writes what is seen
    // inside this many pixels.
    Number { name: "moonfall_notice", label: "Notice radius, pixels", constant: "moonfall::NUMBERS[4]", min: 24, max: 160, start: 64 },
];
/// How its run is registered with the shared shell. Fixed after the prep
/// commit: the shell never needs a line changed for it.
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
/// Its game in the rides (`activity::RIDES`): it plays, takes the seated
/// knights' keys, and ends when a view leaves the room.
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
/// A finished run's mark has been settled into `Home::roundtable[SLOT]`.
pub const BEGUN: u32 = 1;
/// What the first deliverance pays the treasury for each knight at the
/// table: a bounded purse, once, never more than six shares.
pub const GOLD_PER_SEAT: u32 = 30;
pub const WAX_PER_SEAT: u32 = 1;

/// The first time the moon is turned back, the purse goes into the realm's
/// treasury, one share for each knight who stood at the table, and the Home
/// remembers it. Idempotent: the bit it sets is what stops a second payment.
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

/// A run never runs past an hour; a longer one is not a run to play.
pub const LIMIT: u32 = 60 * 60 * HZ;
/// Ticks the skip to dusk or dawn takes to play.
pub const SKIP_LEN: u32 = 36;
/// Ticks the moon's turning back plays before the town is free again.
pub const WON_LEN: u32 = 120;
/// Ticks a call for the bell waits for the table's hands.
pub const CALL_TICKS: u32 = 150;
/// Largest day length (the knob's bound), for what `valid` allows.
const DAY_MAX: u32 = NUMBERS[0].max;

pub fn nums_of(tune: &tune::Tune) -> Nums {
    let n = |i| roundtable::number(tune, SLOT, i);
    Nums { day: n(0), walk: n(1), wisps: n(2), rewind: n(3), notice: n(4) }
}

pub fn nums_start() -> Nums {
    let n = |i: usize| NUMBERS[i].start;
    Nums { day: n(0), walk: n(1), wisps: n(2), rewind: n(3), notice: n(4) }
}

/// The ticks into a rewind ritual when the clock stops spinning and the
/// town stands at dawn again, and the knights may move.
pub fn control(nums: &Nums) -> u32 {
    (nums.rewind * 7 / 10).max(1)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seats: BTreeMap<u32, Seat>,
    /// Knights who stepped away (a tab hidden, a link gone quiet): they keep
    /// what they carry and stand where they stood until they are back or the
    /// town is wound back.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parked: BTreeMap<u32, Seat>,
    pub elapsed: u32,
    /// Ticks into the loop: the shared clock. It is the session's own and
    /// never the Delve's.
    pub clock: u32,
    pub phase: Phase,
    /// The "Dawn of the First Day" card has this many ticks to run.
    pub card: u32,
    pub world: World,
    pub memory: Memory,
    pub call: Option<Call>,
    /// A proclamation being cried: who, what, ticks since.
    pub cry: Option<(Folk, Topic, u32)>,
    /// Words for the whole table: the Ledger wrote, an hour is near.
    pub news: Vec<(Note, u32)>,
    /// The tick of the run the three lights held.
    pub completed: Option<u32>,
    /// The most knights who have sat at once: wisps are scaled to it, and
    /// do not ease when a knight leaves.
    pub peak: u32,
    pub nums: Nums,
    /// Sounds the Ledger and the town have to make, handed out by the next
    /// step (never saved: it is empty whenever the session is at rest).
    #[serde(skip)]
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
            s.sit(id, 0);
        }
        s
    }

    fn sit(&mut self, id: u32, joined: u32) {
        let seat = self.parked.remove(&id).unwrap_or_else(|| {
            let at = SPAWN[(id as usize + 5) % SPAWN.len()];
            Seat { joined, knight: Knight::at(at) }
        });
        self.seats.insert(id, seat);
        self.peak = self.peak.max(self.seats.len() as u32);
    }

    /// The tick of this tick, in the clock's own time, shown as dawn when
    /// the ritual spins it back and as dusk when time is skipped.
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

    /// One tick: the seats are whoever is here and awake, so a knight who
    /// leaves stands up and one who joins late sits down at the well.
    /// Returns the sounds to make.
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>) -> Vec<&'static str> {
        let mut cues = Vec::new();
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        let gone: Vec<u32> = self
            .seats
            .keys()
            .copied()
            .filter(|id| !heroes.get(id).is_some_and(|h| !h.away && !h.stone))
            .collect();
        for id in gone {
            if let Some(seat) = self.seats.remove(&id) {
                // Only a knight who is still in the party is kept for later.
                if heroes.contains_key(&id) {
                    self.parked.insert(id, seat);
                }
            }
        }
        self.parked.retain(|id, _| heroes.contains_key(id));
        let late: Vec<u32> = heroes
            .iter()
            .filter(|(id, h)| !h.away && !h.stone && !self.seats.contains_key(*id))
            .map(|(&id, _)| id)
            .collect();
        for id in late {
            self.sit(id, self.elapsed);
        }
        self.age();
        self.clock = self.clock.min(clock::end(self.nums.day));
        match self.phase {
            Phase::Play => self.play(inputs, &mut cues),
            Phase::Rewind { t, from, fell } => self.rewinding(t, from, fell, &mut cues),
            Phase::Skip { t, from, to } => self.skipping(t, from, to, &mut cues),
            Phase::Won { t } => self.winning(t, &mut cues),
        }
        cues.append(&mut self.pending);
        cues
    }

    /// The timers that only run down.
    fn age(&mut self) {
        self.card = self.card.saturating_sub(1);
        for (_, n) in &mut self.news {
            *n = n.saturating_sub(1);
        }
        self.news.retain(|&(_, n)| n > 0);
        if let Some((_, _, age)) = &mut self.cry {
            *age += 1;
        }
        if self.cry.is_some_and(|(_, _, age)| age > 150) {
            self.cry = None;
        }
        for s in self.seats.values_mut() {
            let k = &mut s.knight;
            if let Some((_, left)) = &mut k.note {
                *left = left.saturating_sub(1);
            }
            if k.note.is_some_and(|(_, left)| left == 0) {
                k.note = None;
            }
            if let Some(say) = &mut k.say {
                say.age += 1;
            }
            if k.say.is_some_and(|say| say.age > 360) {
                k.say = None;
            }
        }
    }

    fn play(&mut self, inputs: &BTreeMap<u32, Input>, cues: &mut Vec<&'static str>) {
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let input = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
            self.act(id, input, cues);
        }
        self.bell_tick(cues);
        if self.completed.is_some() {
            return;
        }
        self.world_tick(cues);
        if (0..3).all(|i| self.flame(i)) {
            self.phase = Phase::Won { t: 0 };
            cues.push("circle_toll");
            cues.push("wish_grant");
        } else if self.clock >= clock::end(self.nums.day) {
            self.begin_rewind(true, cues);
        }
    }

    /// One knight's tick.
    fn act(&mut self, id: u32, input: Input, cues: &mut Vec<&'static str>) {
        let keys = keys_of(input);
        let nums = self.nums;
        let clock = self.clock;
        let before = {
            let Some(s) = self.seats.get_mut(&id) else { return };
            let before = s.knight.keys;
            s.knight.keys = keys;
            s.knight.hurt = s.knight.hurt.saturating_sub(1);
            if s.knight.down > 0 {
                s.knight.down -= 1;
                if s.knight.down == 0 {
                    let at = SPAWN[(id as usize + 5) % SPAWN.len()];
                    let fresh = Knight::at(at);
                    s.knight.x = fresh.x;
                    s.knight.y = fresh.y;
                    s.knight.candles = 1;
                    s.knight.hurt = 90;
                }
                return;
            }
            before
        };
        let press = |bit| pressed(keys, before, bit);
        let panel = self.seats[&id].knight.panel;
        match panel {
            Panel::Ledger(page) => {
                let written = self.memory.written();
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                k.read = written;
                if press(key::LEDGER) || press(key::USE) || press(key::SIDESTEP) {
                    k.panel = Panel::Shut;
                } else if press(key::RIGHT) {
                    k.panel = Panel::Ledger((page + 1) % 7);
                } else if press(key::LEFT) {
                    k.panel = Panel::Ledger((page + 6) % 7);
                }
                return;
            }
            Panel::Bell(line) => {
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                if press(key::UP) {
                    k.panel = Panel::Bell((line + 2) % 3);
                } else if press(key::DOWN) {
                    k.panel = Panel::Bell((line + 1) % 3);
                } else if press(key::LEDGER) || press(key::SIDESTEP) || press(key::MASK) {
                    k.panel = Panel::Shut;
                } else if press(key::USE) || press(key::BELL) {
                    k.panel = Panel::Shut;
                    let ring = [Ring::Rewind, Ring::Dusk, Ring::Dawn][usize::from(line)];
                    self.ask_bell(id, ring, cues);
                }
                return;
            }
            Panel::Shut => {}
        }
        if press(key::LEDGER) {
            let written = self.memory.written();
            let k = &mut self.seats.get_mut(&id).unwrap().knight;
            k.panel = Panel::Ledger(0);
            k.say = None;
            k.read = written;
            return;
        }
        // The moon has turned back: the bell has nothing left to wind.
        if press(key::BELL) && self.completed.is_none() {
            if self.call.is_some_and(|c| c.ayes & (1 << (id - 1)) == 0) {
                self.put_hand(id);
            } else if self.call.is_none() {
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                k.panel = Panel::Bell(0);
                k.say = None;
            }
            return;
        }
        if press(key::MASK) {
            self.cycle_mask(id);
        }
        if press(key::DROP) {
            let held = self.seats[&id].knight.held;
            if held != Item::Empty {
                if held == Item::Ink {
                    self.world.flags &= !INK_TAKEN;
                }
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                k.held = Item::Empty;
                k.note = Some((Note::Sign(says::Sign::Dropped), 60));
            }
        }
        // E: the next page of a talk, or what E means here.
        let talking = self.seats[&id].knight.say;
        if press(key::USE) {
            if let Some(say) = talking {
                let pages = self.say_pages(&say);
                let k = &mut self.seats.get_mut(&id).unwrap().knight;
                if usize::from(say.page) + 1 >= pages {
                    k.say = None;
                } else if let Some(s) = &mut k.say {
                    s.page += 1;
                }
            } else {
                self.press_use(id);
            }
        } else if keys & key::USE != 0 && talking.is_none() {
            if self.hold_use(id).is_some() {
                cues.push("wish_grant");
                cues.push("circle_toll");
            }
        } else if let Some(s) = self.seats.get_mut(&id) {
            s.knight.hold = 0;
        }
        // Walking. Moving away from someone ends the talk.
        let swift = self.seats[&id].knight.mask == Mask::Swift;
        let ground = self.world.ground(clock, nums.day, swift);
        let mut crates = std::mem::take(&mut self.world.crates);
        let moved = {
            let k = &mut self.seats.get_mut(&id).unwrap().knight;
            let pace = walk::speed(nums.walk, k.mask);
            let w = walk::step(k, input, pace, ground, &mut crates, press(key::SIDESTEP));
            if (input.move_x, input.move_y) != (0, 0) {
                k.say = None;
            }
            w
        };
        self.world.crates = crates;
        if moved.filled {
            self.world.set(DITCH_FILLED);
            cues.push("rock_land");
        }
        if moved.moved {
            cues.push("rock_land");
        }
        if moved.heavy {
            let k = &mut self.seats.get_mut(&id).unwrap().knight;
            if k.note.is_none() {
                k.note = Some((Note::Sign(says::Sign::TooHeavy), 60));
            }
        }
    }

    fn cycle_mask(&mut self, id: u32) {
        let owned = |m: Mask| match m {
            Mask::Bare => true,
            Mask::Swift => self.memory.has_mask(ledger::SWIFT),
            Mask::Stout => self.memory.has_mask(ledger::STOUT),
        };
        let order = [Mask::Bare, Mask::Swift, Mask::Stout];
        let now = self.seats[&id].knight.mask;
        let at = order.iter().position(|&m| m == now).unwrap_or(0);
        let next = (1..=3)
            .map(|d| order[(at + d) % 3])
            .find(|&m| owned(m))
            .unwrap_or(Mask::Bare);
        self.seats.get_mut(&id).unwrap().knight.mask = next;
    }

    /// How many hands the bell needs: more than half the table.
    pub fn bell_need(&self) -> u32 {
        self.seats.len() as u32 / 2 + 1
    }

    /// Whether a ring would do anything now.
    pub fn ring_ok(&self, ring: Ring) -> bool {
        match ring {
            Ring::Rewind => true,
            Ring::Dusk => clock::next_dusk(self.clock, self.nums.day) <= clock::final_start(self.nums.day),
            Ring::Dawn => clock::next_dawn(self.clock, self.nums.day) <= clock::final_start(self.nums.day),
        }
    }

    fn ask_bell(&mut self, id: u32, ring: Ring, cues: &mut Vec<&'static str>) {
        if !self.ring_ok(ring) {
            return;
        }
        // The same ring already asked for: this knight's hand is another.
        if let Some(c) = &mut self.call {
            if c.ring == ring {
                c.ayes |= 1 << (id - 1);
            }
            return;
        }
        let call = Call { ring, by: id, ayes: 1 << (id - 1), left: CALL_TICKS };
        self.announce(Note::Ring(id));
        if call.ayes.count_ones() >= self.bell_need() {
            self.ring(ring, cues);
        } else {
            self.call = Some(call);
        }
    }

    fn put_hand(&mut self, id: u32) {
        if let Some(c) = &mut self.call {
            c.ayes |= 1 << (id - 1);
        }
    }

    fn bell_tick(&mut self, cues: &mut Vec<&'static str>) {
        let need = self.bell_need();
        let Some(call) = &mut self.call else { return };
        // A knight who left takes their hand off the rope.
        let seated: u8 = self.seats.keys().fold(0, |m, &id| m | (1 << (id - 1)));
        call.ayes &= seated;
        call.left = call.left.saturating_sub(1);
        if call.ayes.count_ones() >= need {
            let ring = call.ring;
            self.call = None;
            self.ring(ring, cues);
        } else if call.left == 0 || call.ayes == 0 {
            self.call = None;
        }
    }

    /// Strike the bell: the ritual, or time run on.
    fn ring(&mut self, ring: Ring, cues: &mut Vec<&'static str>) {
        match ring {
            Ring::Rewind => self.begin_rewind(false, cues),
            Ring::Dusk | Ring::Dawn => {
                if !self.ring_ok(ring) {
                    return;
                }
                let day = self.nums.day;
                let to = if ring == Ring::Dusk {
                    clock::next_dusk(self.clock, day)
                } else {
                    clock::next_dawn(self.clock, day)
                };
                self.phase = Phase::Skip { t: 0, from: self.clock, to };
                self.calm();
                cues.push("circle_toll");
            }
        }
    }

    /// Menus shut and talk ended, for a change of time.
    fn calm(&mut self) {
        self.call = None;
        for s in self.seats.values_mut() {
            s.knight.panel = Panel::Shut;
            s.knight.say = None;
            s.knight.hold = 0;
        }
    }

    /// Ring the rewind from outside a knight's hands (the tests').
    #[cfg(test)]
    pub(crate) fn begin_rewind_for_test(&mut self) {
        let mut cues = Vec::new();
        self.begin_rewind(false, &mut cues);
    }

    fn begin_rewind(&mut self, fell: bool, cues: &mut Vec<&'static str>) {
        self.phase = Phase::Rewind { t: 0, from: self.clock.min(clock::end(self.nums.day)), fell };
        self.calm();
        cues.push("circle_toll");
        if fell {
            cues.push("boss_roar");
        }
    }

    fn rewinding(&mut self, t: u32, from: u32, fell: bool, cues: &mut Vec<&'static str>) {
        let t = t + 1;
        let control = control(&self.nums);
        if t == control / 2 {
            cues.push("wall_up");
        }
        if t >= control {
            self.reset();
            cues.push("play_card");
        } else {
            self.phase = Phase::Rewind { t, from, fell };
        }
    }

    /// The town returns to the first dawn. The Ledger and the masks stay;
    /// everything else, carried items and opened doors and the rest, is
    /// wound back.
    fn reset(&mut self) {
        self.memory.loops += 1;
        self.clock = 0;
        self.world = World::dawn();
        self.phase = Phase::Play;
        self.card = self.nums.rewind.saturating_sub(control(&self.nums)) + 24;
        self.call = None;
        self.cry = None;
        self.news.clear();
        for (&id, s) in self.seats.iter_mut().chain(self.parked.iter_mut()) {
            let mask = s.knight.mask;
            let at = SPAWN[(id as usize + 5) % SPAWN.len()];
            s.knight = Knight::at(at);
            s.knight.mask = mask;
        }
    }

    fn skipping(&mut self, t: u32, from: u32, to: u32, cues: &mut Vec<&'static str>) {
        let t = t + 1;
        if t >= SKIP_LEN {
            let day = self.nums.day;
            self.clock = to;
            if to / day != from / day || to % day == 0 {
                self.world.snuffed = 0;
            }
            self.world.wisps.clear();
            self.phase = Phase::Play;
            cues.push("circle_toll");
        } else {
            self.phase = Phase::Skip { t, from, to };
        }
    }

    fn winning(&mut self, t: u32, cues: &mut Vec<&'static str>) {
        let t = t + 1;
        if t >= WON_LEN {
            self.completed = Some(self.elapsed);
            self.clock = clock::end(self.nums.day);
            // The three lights burn on: the moon has turned back.
            self.world.lit_until = [self.clock * 2; 3];
            self.world.wisps.clear();
            self.phase = Phase::Play;
            cues.push("wish_grant");
        } else {
            self.phase = Phase::Won { t };
        }
    }

    /// The town's own tick: the clock, the bells, the Gloam, the Ledger.
    fn world_tick(&mut self, cues: &mut Vec<&'static str>) {
        let day = self.nums.day;
        let hour = (day / 24).max(1);
        let before_bridge = clock::bridge_up(self.clock, day);
        // The loop ends where it ends, however the day knob has moved.
        self.clock = (self.clock + 1).min(clock::end(day));
        if self.clock % hour == 0 {
            let h = clock::stamp(self.clock, day).hour;
            if h % 6 == 0 || self.final_hours() {
                cues.push("circle_toll");
            }
        }
        if clock::bridge_up(self.clock, day) != before_bridge {
            cues.push("door_open");
        }
        if self.clock % day == 0 {
            self.world.snuffed = 0;
        }
        // The moon's shadow puts the street lanterns out, one by one.
        if self.final_hours() {
            let fin = clock::final_start(day);
            for n in 0..6u32 {
                if self.clock == fin + n * hour / 2 && self.world.snuffed & (1 << n) == 0 {
                    self.world.snuffed |= 1 << n;
                    self.announce(Note::Out);
                }
            }
        }
        // Aster's oil reaches the well.
        if let Some(t) = self.world.shod {
            if !self.memory.has_keep(ledger::OIL) && self.clock >= folk::aster_at_well(t) {
                self.memory.keeps |= ledger::OIL;
                if self.memory.tick(Folk::Aster) {
                    self.announce(Note::Ticked(Folk::Aster));
                }
                cues.push("wish_grant");
            }
        }
        self.cries();
        if self.clock % 5 == 0 {
            self.witness();
        }
        for (who, id) in ledger::reminders(&self.memory, self.clock, day) {
            self.announce(Note::Remind(who, id));
        }
        cues.extend(self.tick_wisps());
    }

    /// Pella cries the hours and Mira says her verse when they reach their
    /// places.
    fn cries(&mut self) {
        let day = self.nums.day;
        let table = folk::timetable(day);
        for (who, routine) in [(Folk::Pella, 2u8), (Folk::Pella, 4), (Folk::Mira, 10)] {
            for leg in table.legs[who.index()].iter().filter(|l| l.routine == routine) {
                if leg.arrive != self.clock || leg.arrive == 0 {
                    continue;
                }
                let d = self.day_index();
                let topic = match (who, routine) {
                    (Folk::Pella, 2) => Topic::Cry(0),
                    (Folk::Pella, _) => Topic::Cry([1, 2, 3][d as usize]),
                    _ if d == 2 => Topic::Cry(0),
                    _ => continue,
                };
                self.cry = Some((who, topic, 0));
            }
        }
    }

    /// The Ledger writes what a knight is near enough to see.
    fn witness(&mut self) {
        let notice = self.nums.notice as i32;
        let knights: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|s| s.knight.up())
            .map(|s| s.knight.px())
            .collect();
        for who in CAST {
            let fig = self.figure(who);
            if !fig.arrived {
                continue;
            }
            let near = knights
                .iter()
                .any(|&(x, y)| (x - fig.x).pow(2) + (y - fig.y).pow(2) <= notice * notice);
            if !near {
                continue;
            }
            if self.memory.witness(who, fig.routine) {
                self.announce(Note::Wrote(who, fig.routine));
            }
            // What a person's doing may also be a thing to learn.
            let d = self.day_index();
            let fact = match (who, fig.routine) {
                (Folk::Pella, 4) if d == 0 => Some(ledger::Fact::AsterComing),
                (Folk::Aster, 0) => Some(ledger::Fact::AsterLame),
                (Folk::Mira, 10) if d == 2 => Some(ledger::Fact::Verse),
                _ => None,
            };
            if let Some(f) = fact {
                if self.memory.learn(f) {
                    self.announce(Note::Learned(f));
                }
            }
        }
    }

    /// What a bot presses for knight `id`.
    pub fn bot_input(&self, id: u32, _hero: &Hero) -> Input {
        plan::bot(self, id)
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

    /// Mend what a bad save broke.
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
        && k.down <= wisps::DOWN
        && k.roll <= walk::ROLL
        && k.roll_rest <= walk::ROLL_REST
        && (-1..=1).contains(&k.roll_dir.0)
        && (-1..=1).contains(&k.roll_dir.1)
        && k.hold <= story::LIGHT_HOLD
        && k.push <= walk::PUSH_TICKS
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
        self.room().kind == RoomKind::Forest && self.moonfall.is_some()
    }
    pub fn start_moonfall(&mut self) {
        if self.room().kind != RoomKind::Forest {
            return;
        }
        self.forest.leave();
        let mut s = Session::new(&self.players);
        s.nums = nums_of(&self.tune);
        s.card = s.nums.rewind;
        self.moonfall = Some(s);
        self.found = Some((self.tick, 0, format!("{NAME}: hold Esc to return.")));
        // The dawn bell: the first thing a table hears.
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
        let was_free = s.completed.is_some();
        let cues = s.step(inputs, &present);
        for cue in cues {
            self.sounds.push(cue);
        }
        if let (Some(_), false) = (s.completed, was_free) {
            self.marks.insert(CAMPAIGN_MARK.into(), 1);
            let seats = (s.seats.len() as usize).clamp(1, 6);
            self.marks.insert(PARTY_MARKS[seats - 1].into(), 1);
            self.home.roundtable[SLOT] |= BEGUN;
            let score = &mut self.home.countryside.games.roundtable[SLOT];
            let best = (1000u32.saturating_sub(150 * s.memory.loops)).clamp(100, 1000) as u16;
            score.best = score.best.max(best);
            score.wins += 1;
            score.plays = score.plays.max(score.wins);
            self.found = Some((
                self.tick,
                0,
                "The moon turns back! Moonfall stopped. E to return.".into(),
            ));
            self.sounds.push("play_card");
        }
        let freed = s.completed.is_some() && s.phase == Phase::Play;
        let leaving: Vec<u32> = if freed {
            inputs
                .iter()
                .filter(|(id, i)| i.interact && s.nearest_use(**id).is_none())
                .map(|(&id, _)| id)
                .collect()
        } else {
            Vec::new()
        };
        self.moonfall = Some(s);
        for id in leaving {
            self.leave_activity(id);
        }
    }
    /// The party at the door, let in.
    #[cfg(any(test, feature = "testing"))]
    pub fn moonfall_for_test() -> Self {
        roundtable::fixture(&ADVENTURE)
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_moonfall__tests.rs"]
mod tests;
#[cfg(test)]
#[path = "../../tests/rules/together_moonfall_play__tests.rs"]
mod play_tests;
