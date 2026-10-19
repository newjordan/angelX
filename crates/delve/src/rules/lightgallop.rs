//! Lightgallop: the Moonwell Road, a momentum ride for the whole table.
//!
//! The memory it borrows is the classic momentum platformer: speed you build
//! with the road under you, slopes that give and take it, a loop-the-loop
//! that only a fast rider can hold, sparks that scatter when you are hit,
//! springs, a boost, two ways through one level. The first ten seconds: a
//! gallop gathering pace down the gate's ramp until the horse is flying.
//!
//! A side-view stage (like the Forest Road's) with its own curved terrain:
//! the road is a set of polylines, a rider is a distance along one and a
//! ground speed, and a loop is a path that curls over itself (`terrain.rs`,
//! `rider.rs`). Fixed-point integers (Q = 256 sub-pixels) on the 30 Hz sim.
//!
//! The five numbers (rider.rs `Numbers`, the F2 knobs; `assets/lightgallop/BRIEF.md`
//! holds the tuning record):
//!  1. `lightgallop_slope`   100  slope pull and friction, percent
//!  2. `lightgallop_loop`    512  slowest speed that holds the loop's ceiling
//!  3. `lightgallop_spur`   2048  the spur's top speed
//!  4. `lightgallop_lock`     96  how far the lance-lock reaches, pixels
//!  5. `lightgallop_dimming` 1000 the Dimming's pace for one knight
//! Version 1 leaves out: rider against rider, a second road, real art.
//!
//! Keys, all existing buttons: A D run and brake, S (or the down arrow) rolls
//! while moving and crouches to charge when still, E (or W) leaps, tap E again
//! in the air to lance-lock the nearest foe, Space or F is the spur. Hold Esc
//! leaves.
use super::activity::{Activity, Realm, Settled};
use super::roundtable::{Adventure, Number};
use super::*;

pub mod bot;
pub mod level;
pub mod math;
pub mod rider;
pub mod session;
pub mod terrain;
pub mod world;

pub use rider::Numbers;
pub use session::{LIMIT, Seat, Session, Tally};

pub const NAME: &str = "Lightgallop";
/// The kingdom tile its door stands on: walk up and press E.
pub const ENTRY: (i32, i32) = (175, 71);
/// Its place in `roundtable::ADVENTURES` and in `Tune::roundtable`.
pub const SLOT: usize = 4;
/// What a finished run saves to the Home, as the Forest Road's
/// `CAMPAIGN_MARK` does. Marks are settled at the save (`settle`).
pub const CAMPAIGN_MARK: &str = "lightgallop:campaign-begun";
/// Every mark this adventure may write: the crash-safe intent journal keeps
/// exactly these (`roundtable::intent_key`).
pub const MARKS: &[&str] = &[CAMPAIGN_MARK];
/// The five numbers, bounded and named; `Tune::roundtable[SLOT]` holds them.
pub const NUMBERS: [Number; 5] = [
    Number {
        name: "lightgallop_slope",
        label: "Slope pull and friction, percent",
        constant: "lightgallop::rider::SLOPE",
        min: 20,
        max: 300,
        start: 100,
    },
    Number {
        name: "lightgallop_loop",
        label: "Loop stick speed",
        constant: "lightgallop::Numbers::loop_min",
        min: 128,
        max: 1536,
        start: 512,
    },
    Number {
        name: "lightgallop_spur",
        label: "Spur top speed",
        constant: "lightgallop::Numbers::spur_cap",
        min: 1280,
        max: 3072,
        start: 2048,
    },
    Number {
        name: "lightgallop_lock",
        label: "Lance-lock reach, pixels",
        constant: "lightgallop::Numbers::lock_reach",
        min: 32,
        max: 240,
        start: 96,
    },
    Number {
        name: "lightgallop_dimming",
        label: "Dimming pace for one knight",
        constant: "lightgallop::Numbers::dimming",
        min: 0,
        max: 2048,
        start: 1000,
    },
];
/// How its run is registered with the shared shell. Fixed after the prep
/// commit: the shell never needs a line changed for it.
pub const ADVENTURE: Adventure = Adventure {
    slug: "lightgallop",
    name: NAME,
    entry: ENTRY,
    slot: SLOT,
    marks: MARKS,
    numbers: &NUMBERS,
    showing: Run::lightgallop_showing,
    start: Run::start_lightgallop,
    valid: |run| run.lightgallop.as_ref().is_none_or(Session::valid),
    clear: |run| run.lightgallop = None,
    seated: |run| {
        run.lightgallop
            .as_ref()
            .map_or_else(Vec::new, |s| s.seats.keys().copied().collect())
    },
    bot: |run, id| match (run.lightgallop.as_ref(), run.players.get(&id)) {
        (Some(s), Some(hero)) => s.bot_input(id, hero),
        _ => Input::default(),
    },
};
/// Its game in the rides (`activity::RIDES`): it plays, takes the seated
/// knights' keys, and ends when a view leaves the room.
pub const ACTIVITY: Activity = Activity {
    name: NAME,
    on: Run::lightgallop_showing,
    ride: Some(|run, inputs| {
        run.tick_lightgallop(inputs);
        Some(BTreeMap::new())
    }),
    tick: |_, _| {},
    end: |run| run.lightgallop = None,
    ends_outside: true,
    settle: Some(settle),
    fields: &["lightgallop"],
};
/// A finished run's mark has been settled into `Home::roundtable[SLOT]`.
pub const BEGUN: u32 = 1;
/// What the first clear pays each knight, by rank (C, B, A, S): the cap is
/// 40 gold a seat, once. A later ride pays `REPLAY`, a token, so the road is
/// a place to come back to and never a faucet.
pub const PURSE: [u32; 4] = [10, 20, 30, 40];
pub const REPLAY: u32 = 5;
/// What a finished run pays into the realm at the save, once: the mark
/// becomes a flag in the Home, as the Forest Road's does.
fn settle(realm: &mut Realm<'_>, marks: &BTreeMap<String, u32>) -> Settled {
    let changed = marks.contains_key(CAMPAIGN_MARK) && realm.home.roundtable[SLOT] & BEGUN == 0;
    if changed {
        realm.home.roundtable[SLOT] |= BEGUN;
    }
    Settled {
        changed,
        said: if changed {
            vec![("lightgallop".into(), format!("{NAME} cleared. The Bell Gate is open."))]
        } else {
            Vec::new()
        },
    }
}

impl Run {
    pub fn lightgallop_showing(&self) -> bool {
        self.room().kind == RoomKind::Forest && self.lightgallop.is_some()
    }
    pub fn start_lightgallop(&mut self) {
        if self.room().kind != RoomKind::Forest {
            return;
        }
        self.forest.leave();
        self.lightgallop = Some(Session::new(&self.players));
        self.home.countryside.games.roundtable[SLOT].plays =
            self.home.countryside.games.roundtable[SLOT].plays.saturating_add(1);
        self.found = Some((self.tick, 0, format!("{NAME}: run, leap, spur. Hold Esc to return.")));
        self.sounds.push("play_card");
    }
    /// The five numbers as the rider reads them.
    pub fn lightgallop_numbers(&self) -> Numbers {
        let n = |i| super::roundtable::number(&self.tune, SLOT, i) as i32;
        Numbers { slope: n(0), loop_min: n(1), spur_cap: n(2), lock_reach: n(3), dimming: n(4) }
    }
    pub(super) fn tick_lightgallop(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut s) = self.lightgallop.take() else {
            return;
        };
        let present: BTreeMap<u32, Hero> = self
            .players
            .iter()
            .filter(|(id, _)| !self.exits.outside.contains_key(id))
            .map(|(&id, hero)| (id, hero.clone()))
            .collect();
        let numbers = self.lightgallop_numbers();
        let sounds = s.step(inputs, &present, &numbers);
        self.sounds.extend(sounds);
        if s.tally.is_some() && !s.paid {
            s.paid = true;
            self.pay_lightgallop(&s);
        }
        if s.again {
            // The ride again: a fresh road, in one tick, no menu between.
            s = Session::new(&present);
            self.home.countryside.games.roundtable[SLOT].plays =
                self.home.countryside.games.roundtable[SLOT].plays.saturating_add(1);
            self.sounds.push("play_card");
        }
        self.lightgallop = Some(s);
    }
    /// The ride is done: mark the campaign, pay each seated knight once, and
    /// keep the best.
    fn pay_lightgallop(&mut self, s: &Session) {
        let Some(tally) = &s.tally else { return };
        let first = self.home.roundtable[SLOT] & BEGUN == 0;
        let purse = if first { PURSE[usize::from(tally.rank.min(3))] } else { REPLAY };
        for (&id, h) in &mut self.players {
            if s.seats.contains_key(&id) {
                h.carried.add(Spoil::Gold, purse);
                h.laurel = id == tally.best;
                if h.laurel {
                    h.booby = None;
                }
            }
        }
        let record = &mut self.home.countryside.games.roundtable[SLOT];
        record.best = record.best.max(tally.points.min(1000) as u16);
        record.wins = record.wins.saturating_add(1).min(record.plays);
        self.home.roundtable[SLOT] |= BEGUN;
        self.marks.insert(CAMPAIGN_MARK.into(), 1);
        self.found = Some((
            self.tick,
            0,
            format!("{NAME}: {} rank. The Bell Gate rings.", ["C", "B", "A", "S"][usize::from(tally.rank.min(3))]),
        ));
        self.sounds.push("wish_grant");
    }
    /// The party at the door, let in.
    #[cfg(any(test, feature = "testing"))]
    pub fn lightgallop_for_test() -> Self {
        roundtable::fixture(&ADVENTURE)
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_lightgallop__tests.rs"]
mod tests;
#[cfg(test)]
#[path = "../../tests/rules/together_lightgallop_physics__tests.rs"]
mod physics_tests;
#[cfg(test)]
#[path = "../../tests/rules/together_lightgallop_play__tests.rs"]
mod play_tests;
#[cfg(test)]
#[path = "../../tests/rules/together_lightgallop_ride__tests.rs"]
mod ride_tests;
