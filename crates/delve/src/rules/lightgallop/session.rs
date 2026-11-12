//! The party's ride: every seated knight on the Moonwell Road at once, one
//! Dimming behind them, one set of foes and lanterns ahead.
//!
//! Each knight has the whole set of moves and their own camera and spark
//! count; the road is shared: a foe popped is popped for everyone (and
//! re-forms a while later, so a pack behind has something to chain), a lantern
//! post any knight lights is lit for all and holds the Dimming back, and a
//! fallen knight is carried to the last lit post. More knights at the table
//! means more foes on the road and a quicker Dimming, never weaker foes.
//!
//! The rules run in `cockpit/live/lightgallop-rules`, one call a tick: the
//! session goes in with the keys and comes back as the next session and the
//! sounds the tick made. This file holds the session as the save and the
//! picture know it, asks the module, and refuses an answer a save would refuse.
use super::level::level;
use super::math::Q;
use super::rider::*;
use super::show::Show;
use super::world::*;
use super::{SOUNDS, ask};
use crate::rules::{BTreeMap, HZ, Hero, Input, MAX_PLAYERS};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// A run never runs past an hour; a longer one is not a run to play.
pub const LIMIT: u32 = 60 * 60 * HZ;
/// A popped foe re-forms after this many ticks if no knight is near.
pub const REFORM: u16 = 180;
/// The warden's hits for one knight; each more knight adds one.
pub const WARDEN_BASE: u8 = 3;
/// Ticks the warden shrugs off blows after one lands (shared by all).
pub const WARDEN_INV: u8 = 22;
/// Ticks the warden's fall is shown for.
pub const WARDEN_FALL: u16 = 90;

/// One knight at the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    /// The tick of the run this knight sat down at (0: from the start).
    pub joined: u32,
    pub rider: Rider,
}

/// The end of the ride: what the party scored and how it ranks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// The tick the tally began, for the numbers to tick up from.
    pub at: u32,
    /// The pack's time (the last bell), in ticks.
    pub time: u32,
    /// Sparks gathered, the pack's average.
    pub sparks: u32,
    /// The longest lance-lock chain any knight made.
    pub chain: u32,
    /// Lantern posts lit by the pack.
    pub posts: u32,
    pub falls: u32,
    pub points: u32,
    /// 0 C, 1 B, 2 A, 3 S.
    pub rank: u8,
    /// The knight who rode best (the laurel).
    pub best: u32,
    pub scores: BTreeMap<u32, u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seats: BTreeMap<u32, Seat>,
    pub elapsed: u32,
    /// The front of the Dimming, 1/256 pixel along the road.
    pub dim: i32,
    /// Lantern posts lit, a bit each; and when each was (tick + 1).
    pub posts: u32,
    pub post_t: Vec<u32>,
    /// The most knights who have sat down; foes are woken for this many.
    pub provisioned: u32,
    pub foes: Vec<FoeState>,
    /// When each spring and pad last fired (tick + 1), for the picture.
    pub spring_t: Vec<u32>,
    pub pad_t: Vec<u32>,
    pub warden_inv: u8,
    /// The tick the first bell rang, if it has.
    pub bell_t: Option<u32>,
    pub tally: Option<Tally>,
    /// The purse has been paid for this ride.
    pub paid: bool,
    /// A knight asked to ride again at the tally.
    pub again: bool,
    /// What the picture remembers of the moments just ridden (`show.rs`). The
    /// module never sees it.
    #[serde(default, skip_serializing_if = "Show::quiet")]
    pub show: Show,
}

/// The knights who can ride: here, awake and not turned to stone.
fn seated(heroes: &BTreeMap<u32, Hero>) -> Vec<u32> {
    heroes.iter().filter(|(_, h)| !h.away && !h.stone).map(|(&id, _)| id).collect()
}

impl Session {
    /// A fresh ride for the knights at the table, as the module lays it out.
    pub fn new(players: &BTreeMap<u32, Hero>) -> Option<Session> {
        #[derive(Deserialize)]
        struct Fresh {
            session: Session,
        }
        ask::<Fresh>(json!({ "op": "start", "present": seated(players) })).map(|f| f.session).filter(Session::valid)
    }

    /// Which act the road is in at column `x` (pixels).
    pub fn act_at(x: i32) -> usize {
        let a = level().acts;
        (0..4).rev().find(|&n| x >= a[n]).unwrap_or(0)
    }

    /// One tick: the module seats whoever is here and awake, runs every
    /// knight, the foes and the Dimming, and answers the next session and the
    /// sounds the tick made. An answer the save would refuse is not taken.
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>, n: &Numbers) -> Vec<&'static str> {
        #[derive(Deserialize)]
        struct Tick {
            session: Session,
            sounds: Vec<usize>,
        }
        let keys: BTreeMap<u32, Input> = inputs.iter().filter(|(_, i)| i.valid()).map(|(&id, &i)| (id, i)).collect();
        let request = json!({ "op": "tick", "session": &*self, "inputs": keys, "present": seated(heroes), "numbers": n });
        match ask::<Tick>(request) {
            Some(tick) if tick.session.valid() => {
                let mut show = std::mem::take(&mut self.show);
                let old = std::mem::replace(self, tick.session);
                show.observe(&old, self);
                // The picture's own notes are never worth refusing a ride over.
                self.show = if show.valid(self.elapsed) { show } else { Show::default() };
                tick.sounds.iter().filter_map(|&i| SOUNDS.get(i).copied()).collect()
            }
            _ => Vec::new(),
        }
    }

    /// What a bot presses for knight `id`: the high rails for the odd seats,
    /// the valley for the even, riding well.
    pub fn bot_input(&self, id: u32, _hero: &Hero) -> Input {
        #[derive(Deserialize)]
        struct Pressed {
            input: Input,
        }
        ask::<Pressed>(json!({ "op": "bot", "session": self, "id": id })).map_or_else(Input::default, |p| p.input)
    }

    pub fn valid(&self) -> bool {
        let lvl = level();
        let t = &lvl.terrain;
        self.elapsed <= LIMIT
            && self.seats.len() <= MAX_PLAYERS as usize
            && self.seats.iter().all(|(&id, s)| {
                (1..=MAX_PLAYERS).contains(&id) && s.joined <= self.elapsed && s.rider.valid(t)
            })
            && (-4000 * Q..=30000 * Q).contains(&self.dim)
            && self.posts < 1 << lvl.posts.len()
            && self.post_t.len() == lvl.posts.len()
            && self.post_t.iter().all(|&p| p <= self.elapsed + 1)
            && (1..=MAX_PLAYERS).contains(&self.provisioned)
            && self.foes.len() == lvl.foes.len()
            && self.foes.iter().zip(&lvl.foes).all(|(f, spot)| {
                f.hp <= if spot.kind == FoeKind::Warden { WARDEN_BASE + MAX_PLAYERS as u8 - 1 } else { 1 }
                    && f.pop <= REFORM
                    && f.flinch <= 16
            })
            && self.spring_t.len() == lvl.springs.len()
            && self.pad_t.len() == lvl.pads.len()
            && self.spring_t.iter().chain(&self.pad_t).all(|&p| p <= self.elapsed + 1)
            && self.warden_inv <= WARDEN_INV
            && self.bell_t.is_none_or(|b| b <= self.elapsed)
            && self.tally.as_ref().is_none_or(|y| {
                y.at <= self.elapsed && y.points <= 1000 && y.rank <= 3 && y.scores.len() <= MAX_PLAYERS as usize
                    && y.scores.values().all(|&v| v <= 1000)
            })
            && (self.tally.is_none() || self.bell_t.is_some())
            && self.show.valid(self.elapsed)
    }
}
