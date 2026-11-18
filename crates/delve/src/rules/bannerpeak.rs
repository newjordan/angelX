//! Bannerpeak: a side-view run-and-jump adventure. A courtyard below a
//! mountain, eight banners to raise at the Great Pole, King Cinder on the
//! summit. The classic platformer's verbs for every knight at the table: run
//! with a skid, jump from speed, stomp, bump rune blocks for coins, a hearth
//! loaf and a lantern flame, kick or carry a cap, the checkpoint gate and the
//! flagpole, then the three-dimensional cousin's vocabulary in side view: the
//! triple jump, long jump, backflip, wall kick, ground pound, dive and crouch.
//! The camera cannot be that cousin's; see assets/bannerpeak/BRIEF.md for what
//! the side view costs.
//!
//! Keys (existing buttons only): A and D run; Space or W jump; S crouch, and in
//! the air slam; F (or Q) dives in the air, throws a flame, or picks up and
//! carries a cap; E held raises the shield (a friend can stand on it and be
//! sprung from it), and E at the courtyard door or a gate opens the course
//! board. Hold Esc to leave.
//!
//! The five numbers (F2 knobs, `NUMBERS`): `run` the top speed, `hop` the
//! window of the triple jump, `coyote` the grace of a late or early jump,
//! `slam` the shock's radius, `ledge` the spring off a friend's shield.
//!
//! One shared world: foes, blocks, gates and banners belong to the party; each
//! seat has its own body, pie of wedges, size and camera. More seats, more
//! foes and a faster bell boulder, a heavier crown; never weaker ones.
use super::activity::{Activity, Realm, Settled};
use super::roundtable::{Adventure, Number};
use super::*;

pub mod body;
pub mod boss;
pub mod bot;
pub mod foes;
pub mod knight;
pub mod layout;
pub mod session;
pub mod show;
pub mod world;

pub use knight::Knight;
pub use session::{LIMIT, Session};

pub const NAME: &str = "Bannerpeak";
pub const ENTRY: (i32, i32) = (174, 23);
pub const SLOT: usize = 2;
pub const CAMPAIGN_MARK: &str = "bannerpeak:campaign-begun";
pub const MARKS: &[&str] = &[CAMPAIGN_MARK];

pub const RUN: u32 = 1152;
pub const HOP: u32 = 10;
pub const COYOTE: u32 = 5;
pub const SLAM: u32 = 36;
pub const LEDGE: u32 = 3000;

pub const NUMBERS: [Number; 5] = [
    Number {
        name: "bannerpeak_run",
        label: "Top run speed",
        constant: "bannerpeak::RUN",
        min: 960,
        max: 1664,
        start: RUN,
    },
    Number {
        name: "bannerpeak_hop",
        label: "Triple-jump window",
        constant: "bannerpeak::HOP",
        min: 4,
        max: 30,
        start: HOP,
    },
    Number {
        name: "bannerpeak_coyote",
        label: "Coyote and buffer",
        constant: "bannerpeak::COYOTE",
        min: 2,
        max: 9,
        start: COYOTE,
    },
    Number {
        name: "bannerpeak_slam",
        label: "Slam shock radius",
        constant: "bannerpeak::SLAM",
        min: 24,
        max: 96,
        start: SLAM,
    },
    Number {
        name: "bannerpeak_ledge",
        label: "Shield spring",
        constant: "bannerpeak::LEDGE",
        min: 1200,
        max: 4000,
        start: LEDGE,
    },
];

pub const ADVENTURE: Adventure = Adventure {
    slug: "bannerpeak",
    name: NAME,
    entry: ENTRY,
    slot: SLOT,
    marks: MARKS,
    numbers: &NUMBERS,
    showing: Run::bannerpeak_showing,
    start: Run::start_bannerpeak,
    valid: |run| run.bannerpeak.as_ref().is_none_or(Session::valid),
    clear: |run| run.bannerpeak = None,
    seated: |run| {
        run.bannerpeak
            .as_ref()
            .map_or_else(Vec::new, |s| s.seats.keys().copied().collect())
    },
    bot: |run, id| match (run.bannerpeak.as_ref(), run.players.get(&id)) {
        (Some(s), Some(hero)) => s.bot_input(id, hero),
        _ => Input::default(),
    },
};
pub const ACTIVITY: Activity = Activity {
    name: NAME,
    on: Run::bannerpeak_showing,
    ride: Some(|run, inputs| {
        run.tick_bannerpeak(inputs);
        Some(BTreeMap::new())
    }),
    tick: |_, _| {},
    end: |run| run.bannerpeak = None,
    ends_outside: true,
    settle: Some(settle),
    fields: &["bannerpeak"],
};
pub const BEGUN: u32 = 1;
fn settle(realm: &mut Realm<'_>, marks: &BTreeMap<String, u32>) -> Settled {
    let changed = marks.contains_key(CAMPAIGN_MARK) && realm.home.roundtable[SLOT] & BEGUN == 0;
    if changed {
        realm.home.roundtable[SLOT] |= BEGUN;
    }
    Settled {
        changed,
        said: if changed {
            vec![(
                "bannerpeak".into(),
                format!("{NAME}: all eight banners are raised."),
            )]
        } else {
            Vec::new()
        },
    }
}

pub const FIRST_PURSE: u32 = 60;
pub const REPEAT_PURSE: u32 = 12;

impl Run {
    pub fn bannerpeak_showing(&self) -> bool {
        self.on_stage() && self.bannerpeak.is_some()
    }
    pub fn start_bannerpeak(&mut self) {
        if !self.on_stage() {
            return;
        }
        self.forest.leave();
        self.bannerpeak = Some(Session::new(&self.players));
        let record = &mut self.home.countryside.games.roundtable[SLOT];
        record.plays = record.plays.saturating_add(1);
        self.found = Some((
            self.tick,
            0,
            format!("{NAME}: raise eight banners. Hold Esc to return."),
        ));
        self.sounds.push("play_card");
    }
    fn bannerpeak_knobs(&self) -> body::Knobs {
        let n = |i| roundtable::number(&self.tune, SLOT, i);
        body::Knobs {
            run: n(0) as i32,
            hop: n(1),
            grace: n(2),
            slam: n(3) as i32,
            ledge: n(4) as i32,
        }
    }
    pub(super) fn tick_bannerpeak(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut s) = self.bannerpeak.take() else {
            return;
        };
        let present: BTreeMap<u32, Hero> = self
            .players
            .iter()
            .filter(|(id, _)| !self.exits.outside.contains_key(id))
            .map(|(&id, hero)| (id, hero.clone()))
            .collect();
        let knobs = self.bannerpeak_knobs();
        let was_done = s.completed.is_some();
        let mut say = Vec::new();
        s.step(inputs, &present, &knobs, &mut say);
        self.sounds.extend(say);
        let held = s.show.hold.take();
        if !was_done && s.completed.is_some() {
            self.bannerpeak_pays(&s);
        }
        let done = s.completed.is_some();
        self.bannerpeak = Some(s);
        if let Some(weight) = held {
            self.hold_for(weight);
        }
        self.finish_hit_hold();
        if done {
            let returning: Vec<u32> = inputs
                .iter()
                .filter(|(id, i)| i.interact && i.valid() && !self.exits.outside.contains_key(id))
                .map(|(&id, _)| id)
                .collect();
            for id in returning {
                self.leave_activity(id);
            }
        }
    }
    fn bannerpeak_pays(&mut self, s: &Session) {
        let first = self.home.roundtable[SLOT] & BEGUN == 0;
        let purse = if first { FIRST_PURSE } else { REPEAT_PURSE };
        for id in s.seats.keys() {
            if let Some(h) = self.players.get_mut(id) {
                h.carried.add(Spoil::Gold, purse);
            }
        }
        let seconds = s.elapsed / HZ;
        let score = 1000u32.saturating_sub(seconds.saturating_sub(120)).max(100) as u16;
        let record = &mut self.home.countryside.games.roundtable[SLOT];
        record.best = record.best.max(score.min(1000));
        record.wins = record.wins.saturating_add(1).min(record.plays);
        self.home.roundtable[SLOT] |= BEGUN;
        self.marks.insert(CAMPAIGN_MARK.into(), 1);
        self.found = Some((
            self.tick,
            0,
            format!("{NAME}: eight banners! {purse} gold each. E to return."),
        ));
        self.sounds.push("spoil_pickup");
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn bannerpeak_for_test() -> Self {
        roundtable::fixture(&ADVENTURE)
    }
}
#[cfg(test)]
#[path = "bannerpeak/oracle.rs"]
pub mod oracle;

#[cfg(test)]
pub fn install_oracle() {
    crate::scripted::install_for_test("bannerpeak-rules", oracle::answer);
}

#[cfg(test)]
#[path = "../../tests/rules/together_bannerpeak__tests.rs"]
mod tests;
#[cfg(test)]
#[path = "../../tests/rules/together_bannerpeak_feel__tests.rs"]
mod feel;
#[cfg(test)]
#[path = "../../tests/rules/together_bannerpeak_play__tests.rs"]
mod play;

#[cfg(all(test, feature = "bend-cases-games-a-bannerpeak"))]
#[path = "../../tests/rules/bannerpeak_bend_cases__tests.rs"]
mod bend_cases;
