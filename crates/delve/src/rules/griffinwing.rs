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
pub mod rail;
pub mod show;
pub mod things;
pub mod trig;
pub mod types;
pub use types::*;

/// The rules themselves are the Bend module `cockpit/live/griffinwing-rules`,
/// which the host's engine runs (`scripted::step`): the session, the keys,
/// who is at the table and the five numbers go in as the JSON the Delve
/// already saves and sends, and the next session comes back with the tick's
/// sounds and the knights who left the tally. `Session::step` and
/// `Session::bot_with` are the two asks; the session's types, saves and wire,
/// the pictures' reads and the door stay here. The tick as it ran in Rust is
/// the module's oracle, in the cockpit's rules tests; this crate's own tests
/// tick through it.
pub const MODULE: &str = "griffinwing-rules";
/// The sounds the rules cue, by the index the module answers with.
pub const SOUNDS: &str = "play_card knight_fall boss_death roll shot_bolt spoil_pickup shot_spark vent_fire \
    shot_crossbow clang hero_hurt knight_hit_big bomb_toss hit_magic kill boss_rage bomb shield_up shield_block \
    gold rock_land boss_rise boss_roar";

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::symbols"
    )]
    pub snd: Vec<&'static str>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub leave: Vec<u32>,
    /// What the picture remembers of the moments just flown (`show.rs`). The
    /// module never sees it: `bare` leaves it out of the request.
    #[serde(default, skip_serializing_if = "show::Show::quiet")]
    pub show: show::Show,
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
            show: show::Show::default(),
        };
        for (&id, _) in players.iter().filter(|(_, h)| !h.away && !h.stone) {
            s.sit(id, 0);
        }
        s.fill_wing();
        s
    }

    /// Seat a knight at the formation, unhurt for a moment.
    pub fn sit(&mut self, id: u32, joined: u32) {
        if !(1..=MAX_PLAYERS).contains(&id) || self.seats.contains_key(&id) {
            return;
        }
        let (x, z) = slot(id);
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

    /// Fill the wing up to four with riders: the seats first, riders in the
    /// seats nobody sits in.
    pub fn fill_wing(&mut self) {
        let want = WING.saturating_sub(self.seats.len());
        while self.wing.len() > want {
            self.wing.pop();
        }
        while self.wing.len() < want {
            let k = self.wing.len();
            let (x, z, back) = rider_slot(k);
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
            && self.show.valid(self.elapsed)
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
                let (x, z) = slot(id);
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

    /// The rail point a craft is at.
    pub fn craft_s(&self, c: &Craft) -> i32 {
        self.anchor() + c.back
    }

    /// Whether some kite hangs on this craft.
    pub fn latched(&self, id: u8) -> bool {
        self.foes.iter().any(|f| f.st == St::Latched && f.on == id)
    }

    /// The marks a turret's shot will land on, across and up: where it
    /// locked, and a pair more to each side for every three knights beyond
    /// the first.
    pub fn marks(f: &Foe, seats: u8) -> Vec<(i32, i32)> {
        let extra = i32::from(seats.saturating_sub(1)) / 3;
        let mut out = vec![(f.fx - dm(8), f.fz), (f.fx + dm(8), f.fz)];
        for k in 1..=extra {
            out.push((f.fx - dm(8) - k * dm(20), f.fz));
            out.push((f.fx + dm(8) + k * dm(20), f.fz));
        }
        out
    }

    /// One tick: the module reads the session, what the knights pressed
    /// (an invalid press is no press), who is at the table and awake, and the
    /// five numbers, and answers with the next session and what the host is
    /// asked: the sounds and the knights who pressed to leave the tally. A
    /// missing engine or an answer that is not a session a run may hold
    /// leaves the session as it was.
    pub fn step(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        heroes: &BTreeMap<u32, Hero>,
        nums: &Nums,
    ) -> Out {
        let bare = self.bare();
        let mut ask = Ask::of("tick", &*bare, nums);
        ask.keys = inputs
            .iter()
            .filter(|(_, i)| i.valid())
            .map(|(&id, i)| (id, i))
            .collect();
        ask.here = heroes
            .iter()
            .filter(|(_, h)| !h.away && !h.stone)
            .map(|(&id, _)| id)
            .collect();
        let answer = ask_module::<Answer>(&ask).and_then(|a| match a.session.valid() {
            true => Ok(a),
            false => Err("an answer that is not a session a run may hold".to_string()),
        });
        drop(ask);
        drop(bare);
        match answer {
            Ok(a) => {
                // What the picture remembers is the session's, not the module's.
                let mut show = std::mem::take(&mut self.show);
                let old = std::mem::replace(self, a.session);
                show.observe(&old, self);
                self.show = if show.valid(self.elapsed) {
                    show
                } else {
                    show::Show::default()
                };
                let sounds = a.sounds.iter().filter_map(|&i| SOUNDS.split(' ').nth(i));
                Out {
                    sounds: sounds.collect(),
                    leave: a.leave,
                }
            }
            Err(error) => {
                // said once, not thirty times a second
                static SAID: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    tracing::warn!(%error, "griffinwing: a tick was not taken");
                }
                Out::default()
            }
        }
    }

    /// The session as the module is to see it: without the picture's memory.
    fn bare(&self) -> std::borrow::Cow<'_, Session> {
        if self.show.quiet() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut bare = self.clone();
        bare.show = show::Show::default();
        std::borrow::Cow::Owned(bare)
    }

    /// What knight `id` presses this tick, for the numbers `n` stand at.
    pub fn bot_with(&self, id: u32, n: &Nums, skill: bot::Skill) -> Input {
        let bare = self.bare();
        let mut ask = Ask::of("bot", &*bare, n);
        (ask.id, ask.novice) = (id, skill == bot::Skill::Novice);
        ask_module(&ask).unwrap_or_default()
    }

    /// What knight `id` presses this tick, with a sure hand.
    pub fn bot_input(&self, id: u32, _hero: &Hero) -> Input {
        self.bot_with(id, &Nums::default(), bot::Skill::Sure)
    }
}

/// What the module is asked.
#[derive(Serialize)]
struct Ask<'a> {
    op: &'a str,
    session: &'a Session,
    nums: &'a Nums,
    keys: BTreeMap<u32, &'a Input>,
    here: Vec<u32>,
    id: u32,
    novice: bool,
}

impl<'a> Ask<'a> {
    fn of(op: &'a str, session: &'a Session, nums: &'a Nums) -> Self {
        Ask {
            op,
            session,
            nums,
            keys: BTreeMap::new(),
            here: Vec::new(),
            id: 0,
            novice: false,
        }
    }
}

/// What it answers to a tick.
#[derive(Deserialize)]
struct Answer {
    session: Session,
    sounds: Vec<usize>,
    leave: Vec<u32>,
}

fn ask_module<T: serde::de::DeserializeOwned>(ask: &Ask<'_>) -> Result<T, String> {
    #[cfg(test)]
    crate::scripted::install_for_test(MODULE, oracle::answer);
    crate::scripted::step(MODULE, ask)
}

/// A seat's place in the formation: across and up, tenths of a unit.
pub fn slot(id: u32) -> (i32, i32) {
    match id {
        1 => (0, 0),
        2 => (dm(-22), dm(-4)),
        3 => (dm(22), dm(-4)),
        4 => (dm(-40), dm(-8)),
        5 => (dm(40), dm(-8)),
        _ => (0, dm(12)),
    }
}

/// A rider's place: across, up and ahead of the knights, so that what hangs
/// on a rider's tail is in front of a knight behind.
pub fn rider_slot(k: usize) -> (i32, i32, i32) {
    match k {
        0 => (dm(-30), dm(8), dm(60)),
        1 => (dm(30), dm(8), dm(60)),
        2 => (0, dm(18), dm(78)),
        _ => (dm(-62), dm(2), dm(50)),
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
        self.place_for_test(kind, at, vel, st, 0)
    }
    pub fn hunter_for_test(&mut self, side: i8) {
        let at = (
            self.anchor() + LEAD - dm(40),
            i32::from(side) * dm(90),
            dm(40),
        );
        self.place_for_test(Kind::Kite, at, (0, 0, 0), St::Hunt, 1);
    }
    fn place_for_test(
        &mut self,
        kind: Kind,
        at: (i32, i32, i32),
        vel: (i32, i32, i32),
        st: St,
        mode: u8,
    ) -> Option<u16> {
        if self.foes.len() >= MAX_FOES {
            return None;
        }
        let seats = self.pressure();
        let full = kind.health() * u32::from(seats);
        let id = self.ids;
        self.ids = if id >= 60_000 { 1 } else { id + 1 };
        self.foes.push(Foe {
            id,
            kind,
            s: at.0,
            x: at.1,
            z: at.2,
            vs: vel.0,
            vx: vel.1,
            vz: vel.2,
            hp: full,
            full,
            seats,
            st,
            ticks: 0,
            fx: 0,
            fz: 0,
            on: 0,
            flash: 0,
            mode,
            phase: 0,
        });
        self.beat.offered += kind.worth();
        Some(id)
    }
}

impl Run {
    pub fn griffinwing_showing(&self) -> bool {
        self.on_stage() && self.griffinwing.is_some()
    }
    pub fn start_griffinwing(&mut self) {
        if !self.on_stage() {
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
        // The heaviest blow of the tick stops the room a moment: a turret's
        // fall, a lamp, the Wyrm. The cooldown keeps a crowd of pops moving.
        let held = s.show.held(s.elapsed);
        self.griffinwing = Some(s);
        if let Some(weight) = held {
            self.hold_for(weight);
            self.finish_hit_hold();
        }
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
            // The tally says whether this run beat the best the party had.
            s.show.know(record.best);
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
/// The rules as they ran in Rust before they moved into Bend: the engine this
/// crate's own tests tick against, and the oracle of the cockpit's parity tests.
#[cfg(test)]
#[path = "../../../../tests/cockpit/drive/griffinwing_oracle.rs"]
pub mod oracle;
#[cfg(test)]
#[path = "../../tests/rules/together_griffinwing_play__tests.rs"]
mod play_tests;
#[cfg(test)]
#[path = "../../tests/rules/together_griffinwing__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "../../tests/rules/griffinwing_bend_kit__tests.rs"]
mod bend_kit;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "griffinwing/recorder/bend.rs"]
mod bend_cases_tests;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "griffinwing/recorder/run_bend.rs"]
mod bend_run_tests;
#[cfg(test)]
use oracle::Rules;
