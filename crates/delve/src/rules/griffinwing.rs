//! Griffinwing: an on-rails co-op flight shooter. Moonfall Pass, a canyon run
//! to relight the last beacon: three beats, about three and a half minutes,
//! flown by up to six knights on griffins in one formation, each behind their
//! own chase camera, with Round Table riders filling the wing up to four.
//!
//! The feeling (assets/griffinwing/BRIEF.md has the whole of it): the world
//! flies at you along a fixed rail; you steer your craft inside a window and
//! it banks; a free reticle leads the shot; a tap is a rapid twin bolt, and
//! holding F charges the shot while everything the reticle sweeps over is
//! locked, so letting go sends a chain of homing starbursts; Space rolls the
//! craft in a spinning shield that turns incoming Gloam bolts round; E spurs
//! on a short meter, Q brakes, X is Starfall. Kites latch onto a wingman and a
//! portrait calls it; you save them by shooting the chaser. The Wyrm's three
//! lamps open to a reflected bolt and break to a charged lock.
//!
//! How it rides the engine: race3d's flight is a hall's ride on its own
//! gorge (its `fly_griffin` is private and its track, bolts and rivals have
//! no place for shared foes), so this is its own session in the same rail
//! idiom, on the integer 30 Hz sim, 256 to the unit. Its stage projects the
//! rail into world3d's rasteriser with the engine's chase camera (the same
//! trailing eye race3d's griffin uses). Nothing here is free 3D collision:
//! every hit is a swept test in rail space (along, across, up).
//!
//! Co-op is a light layer: one set of foes, bolts and lamps for the party,
//! health scaled per seat (`Foe::seats`), never weaker foes, more of them as
//! seats fill, a knight who joins mid-run flies in at once and scales the
//! beat's foes up by its share, and a fall brings the beat back in under a
//! second with the party's progress kept.
use super::activity::{Activity, Realm, Settled};
use super::roundtable::{Adventure, Number};
use super::*;

pub mod boss;
pub mod bot;
mod combat;
mod flight;
pub mod rail;
mod script;
pub mod things;
pub mod trig;
pub mod types;
pub use types::*;

pub const NAME: &str = "Griffinwing";
/// The kingdom tile its door stands on: walk up and press E.
pub const ENTRY: (i32, i32) = (105, 9);
/// Its place in `roundtable::ADVENTURES` and in `Tune::roundtable`.
pub const SLOT: usize = 3;
/// What a finished run saves to the Home, as the Forest Road's
/// `CAMPAIGN_MARK` does. Marks are settled at the save (`settle`).
pub const CAMPAIGN_MARK: &str = "griffinwing:campaign-begun";
/// A run that flew home with the gold medal.
pub const GOLD_MARK: &str = "griffinwing:gold-medal";
/// Every mark this adventure may write: the crash-safe intent journal keeps
/// exactly these (`roundtable::intent_key`).
pub const MARKS: &[&str] = &[CAMPAIGN_MARK, GOLD_MARK];
/// The five numbers (the brief's "five numbers"): how long a hold takes to
/// charge in full, how long a roll runs, the steer box's half width (tenths
/// of a unit), how long the spur lasts from a full meter, and how long a
/// foe's tell shows before its shot lands (never under 40). The tuning table
/// is in the brief. `Tune::roundtable[SLOT]` holds them.
pub const NUMBERS: [Number; 5] = [
    Number {
        name: "griffinwing_lock",
        label: "Charge ticks",
        constant: "griffinwing::NUMBERS[0]",
        min: 20,
        max: 90,
        start: 40,
    },
    Number {
        name: "griffinwing_roll",
        label: "Roll ticks",
        constant: "griffinwing::NUMBERS[1]",
        min: 18,
        max: 60,
        start: 30,
    },
    Number {
        name: "griffinwing_window",
        label: "Steer box",
        constant: "griffinwing::NUMBERS[2]",
        min: 40,
        max: 90,
        start: 64,
    },
    Number {
        name: "griffinwing_spur",
        label: "Spur ticks",
        constant: "griffinwing::NUMBERS[3]",
        min: 30,
        max: 180,
        start: 75,
    },
    Number {
        name: "griffinwing_tell",
        label: "Tell ticks",
        constant: "griffinwing::NUMBERS[4]",
        min: 40,
        max: 120,
        start: 50,
    },
];
/// How its run is registered with the shared shell. Fixed after the prep
/// commit: the shell never needs a line changed for it.
pub const ADVENTURE: Adventure = Adventure {
    slug: "griffinwing",
    name: NAME,
    entry: ENTRY,
    slot: SLOT,
    marks: MARKS,
    numbers: &NUMBERS,
    showing: Run::griffinwing_showing,
    start: Run::start_griffinwing,
    valid: |run| run.griffinwing.as_ref().is_none_or(Session::valid),
    clear: |run| run.griffinwing = None,
    seated: |run| {
        run.griffinwing
            .as_ref()
            .map_or_else(Vec::new, |s| s.seats.keys().copied().collect())
    },
    bot: |run, id| match (run.griffinwing.as_ref(), run.players.get(&id)) {
        (Some(s), Some(_)) => s.bot_with(id, &Nums::of(&run.tune), bot::Skill::Sure),
        _ => Input::default(),
    },
};
/// Its game in the rides (`activity::RIDES`): it plays, takes the seated
/// knights' keys, and ends when a view leaves the room.
pub const ACTIVITY: Activity = Activity {
    name: NAME,
    on: Run::griffinwing_showing,
    ride: Some(|run, inputs| {
        run.tick_griffinwing(inputs);
        Some(BTreeMap::new())
    }),
    tick: |_, _| {},
    end: |run| run.griffinwing = None,
    ends_outside: true,
    settle: Some(settle),
    fields: &["griffinwing"],
};
/// A finished run's mark has been settled into `Home::roundtable[SLOT]`.
pub const BEGUN: u32 = 1;
/// And one that flew home with the gold medal.
pub const GOLD: u32 = 2;
/// What a finished run pays into the realm at the save, once: the marks
/// become flags in the Home, as the Forest Road's does.
fn settle(realm: &mut Realm<'_>, marks: &BTreeMap<String, u32>) -> Settled {
    let mut flags = 0;
    if marks.contains_key(CAMPAIGN_MARK) {
        flags |= BEGUN;
    }
    if marks.contains_key(GOLD_MARK) {
        flags |= GOLD;
    }
    let changed = flags & !realm.home.roundtable[SLOT] != 0;
    realm.home.roundtable[SLOT] |= flags;
    Settled {
        changed,
        said: if changed {
            vec![(
                "griffinwing".into(),
                format!("{NAME}: the last beacon is lit."),
            )]
        } else {
            Vec::new()
        },
    }
}

/// A run never runs past an hour; a longer one is not a run to play.
pub const LIMIT: u32 = 60 * 60 * HZ;
/// A fall puts the beat back this many ticks after the last knight goes
/// down: under a second.
pub const RETRY: u32 = 24;
/// The Wyrm's death, and the fly-through after it, in ticks.
pub const DYING: u32 = 60;
pub const VICTORY: u32 = 210;
/// The tally holds this long before a press leaves.
pub const TALLY_HOLD: u32 = 45;
/// A knight who falls rises again this many ticks later, at the formation.
pub const RISE: u32 = 75;
/// What the purse pays each seat at the finish: this and a share for each
/// rank of the medal (`Session::purse`); a party that has already lit the
/// beacon is paid the smaller replay purse, so the run is a reason to come
/// back and not a faucet. Most a seat is paid: 25 gold, the pastimes' cap.
pub const PURSE: u32 = 10;
pub const PURSE_MEDAL: u32 = 5;
pub const REPLAY: u32 = 5;
pub const REPLAY_MEDAL: u32 = 2;

/// The five numbers as the sim reads them (`Tune::roundtable[SLOT]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nums {
    pub charge: u32,
    pub roll: u32,
    pub window: u32,
    pub spur: u32,
    pub tell: u32,
}

impl Default for Nums {
    fn default() -> Self {
        let n = &NUMBERS;
        Nums {
            charge: n[0].start,
            roll: n[1].start,
            window: n[2].start,
            spur: n[3].start,
            tell: n[4].start,
        }
    }
}

impl Nums {
    pub fn of(tune: &tune::Tune) -> Nums {
        let n = |i: usize| roundtable::number(tune, SLOT, i).clamp(NUMBERS[i].min, NUMBERS[i].max);
        Nums {
            charge: n(0),
            roll: n(1),
            window: n(2),
            spur: n(3),
            tell: n(4),
        }
    }
    /// The steer box's half width and half height.
    pub fn wx(&self) -> i32 {
        dm(self.window as i32)
    }
    pub fn wz(&self) -> i32 {
        dm(self.window as i32) * 5 / 8
    }
    /// How much of a roll reflects: all but its last ten ticks.
    pub fn reflect(&self) -> u32 {
        self.roll.saturating_sub(10).max(8)
    }
}

/// What a tick asks of the host: the sounds to play and the knights who
/// pressed to leave the tally.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Out {
    pub sounds: Vec<&'static str>,
    pub leave: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seats: BTreeMap<u32, Seat>,
    /// Ticks since the run opened, falls and all.
    pub elapsed: u32,
    pub mode: Mode,
    pub mode_t: u32,
    /// The beat (0 to 2) and its clock; the rail's point follows from them.
    pub stage: u8,
    pub t: u32,
    /// Where a fall puts the beat back to, on its clock.
    pub cp: u32,
    /// The next event of the beat's script.
    pub next: u16,
    pub foes: Vec<Foe>,
    pub props: Vec<Prop>,
    pub bolts: Vec<Bolt>,
    pub wing: Vec<Wingman>,
    pub boss: Option<Beast>,
    pub calls: Vec<Call>,
    pub fx: Vec<Flare>,
    pub ids: u16,
    /// Banked beats, and the one in hand.
    pub bank: Tally,
    pub beat: Tally,
    pub retries: u8,
    pub medal: Option<Medal>,
    /// The seats the purse has been paid to (a bit each).
    pub paid: u8,
    /// The beacon's news and the Home's marks have been given: once a run,
    /// whether or not any seat was left to pay.
    #[serde(default)]
    pub settled: bool,
    /// The tick the Wyrm fell, and where on the rail.
    pub done: Option<u32>,
    pub home: i32,
    /// Whether this is the party's first run (the beacon not yet lit).
    pub first: bool,
    #[serde(skip)]
    pub snd: Vec<&'static str>,
    #[serde(skip)]
    pub leave: Vec<u32>,
}

/// A seat's bit.
pub fn bit(id: u32) -> u8 {
    1 << (id.clamp(1, 8) - 1)
}

impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Self {
        let mut s = Session {
            seats: BTreeMap::new(),
            elapsed: 0,
            mode: Mode::Play,
            mode_t: 0,
            stage: 0,
            t: 0,
            cp: 0,
            next: 0,
            foes: Vec::new(),
            props: Vec::new(),
            bolts: Vec::new(),
            wing: Vec::new(),
            boss: None,
            calls: Vec::new(),
            fx: Vec::new(),
            ids: 1,
            bank: Tally::default(),
            beat: Tally::default(),
            retries: 0,
            medal: None,
            paid: 0,
            settled: false,
            done: None,
            home: 0,
            first: true,
            snd: Vec::new(),
            leave: Vec::new(),
        };
        for (&id, _) in players.iter().filter(|(_, h)| !h.away && !h.stone) {
            s.sit(id, 0);
        }
        s.fill_wing();
        s
    }

    /// Seat a knight at the formation, unhurt for a moment.
    fn sit(&mut self, id: u32, joined: u32) {
        if !(1..=MAX_PLAYERS).contains(&id) || self.seats.contains_key(&id) {
            return;
        }
        let (x, z) = flight::slot(id);
        let mut craft = Craft::new(x, z);
        craft.hurt = 2 * HZ;
        self.seats.insert(id, Seat { joined, craft });
    }

    /// The rail's point the formation flies at now.
    pub fn anchor(&self) -> i32 {
        rail::ORIGIN[usize::from(self.stage.min(2))] + self.t as i32 * PACE
    }

    /// How many knights fly: the pressure on the foes. A table with nobody
    /// at it is still one's worth.
    pub fn pressure(&self) -> u8 {
        self.seats.len().clamp(1, MAX_PLAYERS as usize) as u8
    }

    /// A fresh id for a foe.
    pub(super) fn next_id(&mut self) -> u16 {
        let id = self.ids;
        self.ids = if self.ids >= 60_000 { 1 } else { self.ids + 1 };
        id
    }

    pub(super) fn sound(&mut self, name: &'static str) {
        if self.snd.len() < 12 && !self.snd.contains(&name) {
            self.snd.push(name);
        }
    }

    /// A rider's short shout, shown as a portrait for a moment. The same line
    /// is not said twice in a row, and four show at most.
    pub(super) fn say(&mut self, who: u8, line: Line) {
        if self.calls.iter().any(|c| c.line == line && c.ticks > 40) {
            return;
        }
        if self.calls.len() >= MAX_CALLS {
            self.calls.remove(0);
        }
        self.calls.push(Call {
            who,
            line,
            ticks: 100,
        });
    }

    pub(super) fn spark(&mut self, what: Burst, s: i32, x: i32, z: i32, n: u8) {
        if self.fx.len() >= MAX_FX {
            self.fx.remove(0);
        }
        self.fx.push(Flare {
            what,
            s,
            x,
            z,
            ticks: match what {
                Burst::Starfall => 36,
                Burst::Fall => 40,
                Burst::Lock => 8,
                _ => 14,
            },
            n,
        });
    }

    /// Who calls the hints: the first of the riders still flying, and when a
    /// full table leaves none (or all are lost), Aster, the wing's lead, who
    /// calls from the beacon's keep.
    pub(super) fn voice(&self) -> u8 {
        self.wing.iter().find(|w| !w.lost).map_or(7, |w| w.id)
    }

    /// Fill the wing up to four with riders: the seats first, riders in the
    /// seats nobody sits in.
    pub(super) fn fill_wing(&mut self) {
        let want = WING.saturating_sub(self.seats.len());
        while self.wing.len() > want {
            self.wing.pop();
        }
        while self.wing.len() < want {
            let k = self.wing.len();
            let (x, z, back) = flight::rider_slot(k);
            let mut craft = Craft::new(x, z);
            craft.back = back;
            self.wing.push(Wingman {
                id: 7 + k as u8,
                craft,
                lost: false,
                fire: 12 * k as u32,
            });
        }
    }

    /// Who is at the table now (`present`), seated and stood up, and the
    /// foes' health provisioned for the seats now flying: a knight who sits
    /// down scales the beat up by its share, and a seat already provisioned
    /// does not scale twice (`Foe::seats`).
    fn muster(&mut self, heroes: &BTreeMap<u32, Hero>) {
        self.seats
            .retain(|id, _| heroes.get(id).is_some_and(|h| !h.away && !h.stone));
        let here: Vec<u32> = heroes
            .iter()
            .filter(|(_, h)| !h.away && !h.stone)
            .map(|(&id, _)| id)
            .collect();
        for id in here {
            if !self.seats.contains_key(&id) && (1..=MAX_PLAYERS).contains(&id) {
                self.sit(id, self.elapsed);
                if self.elapsed > 0 {
                    self.say(id as u8, Line::Join);
                    self.sound("play_card");
                }
            }
        }
        let now = self.pressure();
        for f in &mut self.foes {
            if f.seats < now {
                let was = u32::from(f.seats.max(1));
                f.hp = (u64::from(f.hp) * u64::from(now) / u64::from(was)) as u32;
                f.full = f.full * u32::from(now) / was;
                f.seats = now;
            }
        }
        self.fill_wing();
    }

    /// One tick: the seats are whoever is here and awake, so a knight who
    /// leaves stands up and one who joins late sits down; then the beat.
    pub fn step(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
        nums: &Nums,
    ) -> Out {
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        self.muster(heroes);
        let live: Vec<u32> = self.seats.keys().copied().collect();
        let keys = |id: u32| {
            inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default()
        };
        match self.mode {
            Mode::Play => {
                self.t = (self.t + 1).min(LIMIT);
                for &id in &live {
                    self.fly(id, keys(id), nums);
                }
                self.wing_fly(nums);
                self.run_script(nums);
                self.step_foes(nums);
                self.step_boss(nums);
                self.step_bolts(nums);
                self.step_props(nums);
                self.step_craft(nums);
                self.tick_lists();
                self.end_beat();
            }
            Mode::Retry => {
                self.tick_lists();
                self.mode_t = self.mode_t.saturating_sub(1);
                if self.mode_t == 0 {
                    self.restart();
                }
            }
            Mode::Victory | Mode::Tally => {
                // The rail carries them home; at the tally the wing hovers.
                if self.mode == Mode::Victory {
                    self.t = (self.t + 1).min(LIMIT);
                }
                for &id in &live {
                    self.fly(id, keys(id), nums);
                }
                self.wing_fly(nums);
                self.step_bolts(nums);
                self.step_props(nums);
                self.step_craft(nums);
                self.tick_lists();
                self.mode_t = (self.mode_t + 1).min(LIMIT);
                if self.mode == Mode::Victory && self.mode_t >= VICTORY {
                    self.finish();
                } else if self.mode == Mode::Tally && self.mode_t > TALLY_HOLD {
                    for &id in &live {
                        let fresh = self
                            .seats
                            .get(&id)
                            .is_some_and(|s| s.craft.keys & flight::K_SPUR == 0);
                        if keys(id).interact && fresh {
                            self.leave.push(id);
                        }
                    }
                }
            }
        }
        // The keys of this tick stand for the next one's presses.
        for (&id, seat) in &mut self.seats {
            seat.craft.keys = flight::bits(keys(id));
        }
        Out {
            sounds: std::mem::take(&mut self.snd),
            leave: std::mem::take(&mut self.leave),
        }
    }

    /// The calls, flashes and counters wind down.
    fn tick_lists(&mut self) {
        for c in &mut self.calls {
            c.ticks = c.ticks.saturating_sub(1);
        }
        self.calls.retain(|c| c.ticks > 0);
        for f in &mut self.fx {
            f.ticks = f.ticks.saturating_sub(1);
        }
        self.fx.retain(|f| f.ticks > 0);
        for f in &mut self.foes {
            f.flash = f.flash.saturating_sub(1);
        }
    }

    /// The beat's end, a wipe, or the Wyrm's fall.
    fn end_beat(&mut self) {
        // A wipe: every knight down at once.
        if !self.seats.is_empty() && self.seats.values().all(|s| s.craft.down > 0) {
            self.mode = Mode::Retry;
            self.mode_t = RETRY;
            self.sound("knight_fall");
            return;
        }
        if self.stage < 2 && self.t >= rail::LEN[usize::from(self.stage)] {
            self.next_beat();
            return;
        }
        let dying = self
            .boss
            .as_ref()
            .is_some_and(|b| b.st == Wyrm::Dying && b.ticks == 0);
        if self.stage == 2 && dying {
            self.mode = Mode::Victory;
            self.mode_t = 0;
            self.done = Some(self.elapsed);
            self.home = self.anchor();
            self.beat.pts += Kind::Lamp.worth() * 2;
            self.beat.base += Kind::Lamp.worth() * 2;
            self.beat.offered += Kind::Lamp.worth() * 2;
            self.foes.clear();
            self.bolts.clear();
            self.say(self.voice(), Line::Home);
            self.sound("boss_death");
            self.victory_gates();
        }
    }

    /// The next beat flies on from where this one ends, the party mended a
    /// little and a bomb richer; what the beat made is banked.
    fn next_beat(&mut self) {
        self.bank.bank(&self.beat);
        self.beat = Tally::default();
        self.stage += 1;
        self.t = 0;
        self.cp = 0;
        self.next = 0;
        let line = if self.stage == 1 {
            Line::Ruins
        } else {
            Line::Roost
        };
        self.say(self.voice(), line);
        self.sound("play_card");
        for s in self.seats.values_mut() {
            s.craft.shield = (s.craft.shield + 40).min(SHIELD);
            s.craft.bombs = (s.craft.bombs + 1).min(3);
            s.craft.meter = METER;
            s.craft.spent = false;
        }
        for w in &mut self.wing {
            w.lost = false;
            w.craft.shield = (w.craft.shield + 40).min(SHIELD);
        }
    }

    /// A beat starts over from its checkpoint: the foes the beat had are
    /// gone and its script runs again from there; what was banked stays, as
    /// do the Wyrm's lamps already out. Everyone is up, mended and unhurt for
    /// a moment.
    fn restart(&mut self) {
        self.retries = self.retries.saturating_add(1);
        self.beat = Tally::default();
        self.t = self.cp;
        self.next = script::resume(self.stage, self.cp);
        self.props.clear();
        self.bolts.clear();
        self.fx.clear();
        self.calls.clear();
        self.foes.retain(|f| f.kind == Kind::Lamp);
        if let Some(b) = &mut self.boss {
            b.st = Wyrm::Idle;
            b.ticks = 2 * HZ;
            b.marks.clear();
        }
        for (&id, s) in &mut self.seats {
            let (x, z) = flight::slot(id);
            let bombs = s.craft.bombs.max(2);
            let (pts, hits, saved, gates) =
                (s.craft.pts, s.craft.hits, s.craft.saved, s.craft.gates);
            s.craft = Craft::new(x, z);
            s.craft.hurt = 2 * HZ;
            s.craft.bombs = bombs;
            (s.craft.pts, s.craft.hits, s.craft.saved, s.craft.gates) = (pts, hits, saved, gates);
        }
        for w in &mut self.wing {
            w.lost = false;
            w.craft.shield = SHIELD;
            w.craft.down = 0;
        }
        self.mode = Mode::Play;
        self.mode_t = 0;
        self.say(self.voice(), Line::Hello);
    }

    /// The Wyrm is down and the fly-through is over: the medal, and the
    /// tally.
    fn finish(&mut self) {
        self.bank.bank(&self.beat);
        self.beat = Tally::default();
        self.medal = Some(self.medal_now());
        self.mode = Mode::Tally;
        self.mode_t = 0;
        self.sound("play_card");
    }

    /// The medal the party has earned: the share of what the beats offered
    /// that they took, less a little for every fall.
    pub fn medal_now(&self) -> Medal {
        let all = self.bank.offered.max(1);
        let pct = (u64::from(self.bank.base) * 100 / u64::from(all)) as i32
            - 5 * i32::from(self.retries)
            - 4 * i32::from(self.bank.lost);
        if pct >= 85 {
            Medal::Gold
        } else if pct >= 60 {
            Medal::Silver
        } else {
            Medal::Bronze
        }
    }

    /// What the finish pays each knight: a purse and a share for the medal,
    /// less for a party that has lit the beacon before.
    pub fn purse(&self) -> u32 {
        let rank = self.medal.map_or(0, Medal::rank);
        if self.first {
            PURSE + PURSE_MEDAL * rank
        } else {
            REPLAY + REPLAY_MEDAL * rank
        }
    }

    /// Whether the Wyrm has fallen and the run is over.
    pub fn finished(&self) -> bool {
        self.mode == Mode::Tally
    }

    /// The share of what was offered that was taken, in tenths of a percent.
    pub fn permille(&self) -> u32 {
        (u64::from(self.bank.base) * 1000 / u64::from(self.bank.offered.max(1))).min(1000) as u32
    }

    pub fn valid(&self) -> bool {
        let wide = |v: i32| (-40_000_000..=40_000_000).contains(&v);
        self.elapsed <= LIMIT
            && self.seats.len() <= MAX_PLAYERS as usize
            && self.seats.iter().all(|(&id, s)| {
                (1..=MAX_PLAYERS).contains(&id) && s.joined <= self.elapsed && s.craft.valid()
            })
            && self.stage <= 2
            && self.t <= LIMIT
            && self.cp <= self.t.max(rail::BOSS_AT)
            && self.mode_t <= LIMIT
            && self.foes.len() <= MAX_FOES
            && self.props.len() <= MAX_PROPS
            && self.bolts.len() <= MAX_BOLTS
            && self.wing.len() <= WING
            && self.calls.len() <= MAX_CALLS
            && self.fx.len() <= MAX_FX
            && self.bank.valid()
            && self.beat.valid()
            && self.paid < 64
            && self.done.is_none_or(|d| d <= self.elapsed)
            && self.home >= 0
            && self
                .wing
                .iter()
                .all(|w| (7..=10).contains(&w.id) && w.craft.valid())
            && self.boss.as_ref().is_none_or(Beast::valid)
            && self.calls.iter().all(|c| c.ticks <= 200 && c.who <= 10)
            && self
                .fx
                .iter()
                .all(|f| f.ticks <= 60 && wide(f.s) && wide(f.x) && wide(f.z))
            && self.foes.iter().all(|f| {
                f.hp <= f.full
                    && f.full <= Kind::Lamp.health() * MAX_PLAYERS * 2
                    && (1..=MAX_PLAYERS as u8).contains(&f.seats)
                    && wide(f.s)
                    && wide(f.x)
                    && wide(f.z)
                    && (-(4 * Q)..=4 * Q).contains(&f.vs)
                    && (-(4 * Q)..=4 * Q).contains(&f.vx)
                    && (-(4 * Q)..=4 * Q).contains(&f.vz)
                    && f.ticks <= 100_000
                    && f.on <= 10
                    && wide(f.fx)
                    && wide(f.fz)
            })
            && self.props.iter().all(|p| {
                wide(p.s)
                    && wide(p.x)
                    && wide(p.z)
                    && (-dm(400)..=dm(400)).contains(&p.w)
                    && (-dm(400)..=dm(400)).contains(&p.h)
                    && (-dm(400)..=dm(400)).contains(&p.len)
                    && p.taken < 64
            })
            && self.bolts.iter().all(|b| {
                b.owner <= 10
                    && wide(b.s)
                    && wide(b.x)
                    && wide(b.z)
                    && (-(8 * Q)..=8 * Q).contains(&b.vs)
                    && (-(8 * Q)..=8 * Q).contains(&b.vx)
                    && (-(8 * Q)..=8 * Q).contains(&b.vz)
                    && b.life <= 600
                    && b.dmg <= 1000
            })
    }

    /// Mend what a bad save broke.
    pub fn repair(&mut self) {
        self.seats.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        self.elapsed = self.elapsed.min(LIMIT);
        let elapsed = self.elapsed;
        for (&id, seat) in &mut self.seats {
            seat.joined = seat.joined.min(elapsed);
            if !seat.craft.valid() {
                let (x, z) = flight::slot(id);
                seat.craft = Craft::new(x, z);
            }
        }
        self.stage = self.stage.min(2);
        self.t = self.t.min(LIMIT);
        self.cp = self.cp.min(self.t.max(rail::BOSS_AT));
        self.mode_t = self.mode_t.min(LIMIT);
        self.foes.truncate(MAX_FOES);
        self.props.truncate(MAX_PROPS);
        self.bolts.truncate(MAX_BOLTS);
        self.calls.truncate(MAX_CALLS);
        self.fx.truncate(MAX_FX);
        self.wing
            .retain(|w| (7..=10).contains(&w.id) && w.craft.valid());
        self.wing.truncate(WING);
        self.foes
            .retain(|f| f.hp <= f.full && (1..=MAX_PLAYERS as u8).contains(&f.seats));
        self.paid &= 63;
        if self.boss.as_ref().is_some_and(|b| !b.valid()) {
            self.boss = None;
        }
        if !self.bank.valid() {
            self.bank = Tally::default();
        }
        if !self.beat.valid() {
            self.beat = Tally::default();
        }
        if !self.valid() {
            // What is left is no beat to resume: start the one in hand over.
            self.foes.clear();
            self.props.clear();
            self.bolts.clear();
            self.fx.clear();
            self.calls.clear();
            self.boss = None;
            self.done = self.done.filter(|&d| d <= elapsed);
            self.t = 0;
            self.cp = 0;
            self.next = 0;
        }
    }
}

/// What the pictures' own checks reach for: a foe or a hunter put where the
/// beat's script would not, and no script after it.
#[cfg(any(test, feature = "testing"))]
impl Session {
    pub fn spawn_for_test(
        &mut self,
        kind: Kind,
        at: (i32, i32, i32),
        vel: (i32, i32, i32),
        st: St,
    ) -> Option<u16> {
        self.spawn_foe(kind, at, vel, st, 0, 0)
    }
    pub fn hunter_for_test(&mut self, side: i8) {
        self.act(script::Act::Hunter { side });
    }
}

impl Run {
    pub fn griffinwing_showing(&self) -> bool {
        self.room().kind == RoomKind::Forest && self.griffinwing.is_some()
    }
    pub fn start_griffinwing(&mut self) {
        if self.room().kind != RoomKind::Forest {
            return;
        }
        self.forest.leave();
        let mut session = Session::new(&self.players);
        session.first = self.home.roundtable[SLOT] & BEGUN == 0;
        self.griffinwing = Some(session);
        let record = &mut self.home.countryside.games.roundtable[SLOT];
        record.plays = record.plays.saturating_add(1);
        self.found = Some((self.tick, 0, format!("{NAME}: hold Esc to return.")));
        self.sounds.push("play_card");
    }
    pub(super) fn tick_griffinwing(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut s) = self.griffinwing.take() else {
            return;
        };
        let present = self
            .players
            .iter()
            .filter(|(id, _)| !self.exits.outside.contains_key(id))
            .map(|(&id, hero)| (id, hero.clone()))
            .collect();
        let out = s.step(inputs, &present, &Nums::of(&self.tune));
        for name in out.sounds {
            self.sounds.push(name);
        }
        if s.finished() {
            self.settle_griffinwing(&mut s);
        }
        self.griffinwing = Some(s);
        for id in out.leave {
            self.leave_activity(id);
        }
    }

    /// The Wyrm is down: the party's Home is marked and the purse is paid,
    /// once for each knight who flew it home, however often this is called.
    fn settle_griffinwing(&mut self, s: &mut Session) {
        let medal = s.medal.unwrap_or(Medal::Bronze);
        let purse = s.purse();
        let first = !s.settled;
        let flown = s.done.unwrap_or(0);
        let ids: Vec<u32> = s
            .seats
            .iter()
            .filter(|(_, seat)| seat.joined <= flown)
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            if s.paid & bit(id) != 0 {
                continue;
            }
            s.paid |= bit(id);
            if let Some(hero) = self.players.get_mut(&id) {
                hero.carried.add(Spoil::Gold, purse);
                hero.laurel = medal == Medal::Gold;
            }
        }
        if first {
            s.settled = true;
            self.marks.insert(CAMPAIGN_MARK.into(), 1);
            self.home.roundtable[SLOT] |= BEGUN;
            if medal == Medal::Gold {
                self.marks.insert(GOLD_MARK.into(), 1);
                self.home.roundtable[SLOT] |= GOLD;
            }
            let record = &mut self.home.countryside.games.roundtable[SLOT];
            record.wins = record.wins.saturating_add(1).min(record.plays);
            record.best = record.best.max(s.permille() as u16);
            self.found = Some((
                self.tick,
                0,
                format!("{NAME}: {} medal. The last beacon is lit!", medal.name()),
            ));
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
    }

    /// The party at the door, let in.
    #[cfg(any(test, feature = "testing"))]
    pub fn griffinwing_for_test() -> Self {
        roundtable::fixture(&ADVENTURE)
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_griffinwing_play__tests.rs"]
mod play_tests;
#[cfg(test)]
#[path = "../../tests/rules/together_griffinwing__tests.rs"]
mod tests;
