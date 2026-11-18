//! The show: what Bannerpeak's picture remembers so it can make a fuss.
//!
//! The sim decides what happened (a landing, a stomp, a block struck, a banner
//! raised, a plate off King Cinder's crown) and writes it here as a short mark
//! with the tick it landed on; `render::overworld::arena::bannerpeak` reads
//! the marks as ages and draws the dust, the coins, the banners and the shake.
//! Nothing here changes the run: the marks are never read back by a rule, so a
//! save that drops them plays the same game. Time is the session's own tick.
//!
//! Blows (the weight of a stomp, a slam, a plate) are the juice kit's
//! (`rules::juice`): the picture shakes and flashes by their weight, and the
//! heaviest of a tick is handed to the realm's hit-stop by `tick_bannerpeak`.
use crate::rules::MAX_PLAYERS;
use crate::rules::juice::{Blow, Juice, Weight};
use serde::{Deserialize, Serialize};

pub const MOST_MARKS: usize = 48;
pub const MOST_SHOUTS: usize = 6;
pub const SHOUT_LIFE: u32 = 66;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Land,
    Hop,
    Step,
    Skid,
    Stomp,
    Coin,
    Block,
    Banner,
    Gate,
    Spark,
    Slam,
    Spring,
    Hurt,
    Pit,
    Rescue,
    Power,
    Down,
    Plate,
    Crown,
    Cap,
    Kick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub who: u8,
    pub n: i16,
    pub born: u32,
}

impl Mark {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
    pub fn life(&self) -> u32 {
        match self.kind {
            Kind::Step | Kind::Skid | Kind::Kick => 14,
            Kind::Hop | Kind::Cap | Kind::Block => 20,
            Kind::Land | Kind::Stomp | Kind::Spark | Kind::Spring | Kind::Hurt | Kind::Pit | Kind::Power => 30,
            Kind::Coin | Kind::Slam | Kind::Down | Kind::Rescue | Kind::Gate | Kind::Plate => 40,
            Kind::Banner | Kind::Crown => 150,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    Saved,
    Lift,
    Together(u8),
    Chain(u8),
    Plate,
    Crown,
    Hundred,
}

impl Cry {
    pub fn rank(self) -> u8 {
        match self {
            Cry::Chain(n) => 2 + n.min(4),
            Cry::Lift => 2,
            Cry::Saved => 5,
            Cry::Hundred => 5,
            Cry::Together(n) => 5 + n.min(3),
            Cry::Plate => 8,
            Cry::Crown => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shout {
    pub cry: Cry,
    pub who: u8,
    pub born: u32,
}

impl Shout {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub juice: Juice,
    pub marks: Vec<Mark>,
    pub shouts: Vec<Shout>,
    #[serde(skip)]
    pub hold: Option<Weight>,
}

impl Eq for Show {}

impl Show {
    pub fn quiet(&self) -> bool {
        self.juice.quiet() && self.marks.is_empty() && self.shouts.is_empty()
    }
    pub fn valid(&self) -> bool {
        self.juice.valid()
            && self.marks.len() <= MOST_MARKS
            && self.shouts.len() <= MOST_SHOUTS
            && self.marks.iter().all(|m| m.who as u32 <= MAX_PLAYERS && (-4000..=60000).contains(&m.x) && (-4000..=60000).contains(&m.y))
            && self.shouts.iter().all(|s| s.who as u32 <= MAX_PLAYERS)
    }
    pub fn tick(&mut self, now: u32) {
        self.juice.tick(now);
        self.marks.retain(|m| m.age(now) < m.life());
        self.shouts.retain(|s| s.age(now) < SHOUT_LIFE);
        self.hold = None;
    }
    pub fn mark(&mut self, now: u32, kind: Kind, x: i32, y: i32, who: u32, n: i32) {
        if self.marks.len() >= MOST_MARKS {
            let cheap = self.marks.iter().position(|m| matches!(m.kind, Kind::Step | Kind::Hop | Kind::Skid | Kind::Kick));
            self.marks.remove(cheap.unwrap_or(0));
        }
        self.marks.push(Mark { kind, x, y, who: who.min(255) as u8, n: n.clamp(-30000, 30000) as i16, born: now });
    }
    pub fn blow(&mut self, now: u32, by: u32, x: i32, y: i32, weight: Weight) {
        self.juice.blow(Blow {
            owner: by.clamp(1, MAX_PLAYERS),
            target: by.clamp(1, MAX_PLAYERS),
            x: x as f32,
            y: y as f32,
            from: (x as f32, y as f32),
            lost: 1,
            weight,
            born: now,
        });
        if self.hold.is_none_or(|h| h < weight) {
            self.hold = Some(weight);
        }
    }
    pub fn shout(&mut self, now: u32, cry: Cry, who: u32) {
        if self.shouts.len() >= MOST_SHOUTS {
            self.shouts.remove(0);
        }
        self.shouts.push(Shout { cry, who: who.min(255) as u8, born: now });
    }
    pub fn latest(&self, kind: Kind, who: u32) -> Option<&Mark> {
        self.marks.iter().rev().find(|m| m.kind == kind && u32::from(m.who) == who)
    }
}
