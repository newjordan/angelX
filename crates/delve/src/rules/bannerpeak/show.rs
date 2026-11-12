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

/// The most marks held; the oldest go first.
pub const MOST_MARKS: usize = 48;
/// The most calls held.
pub const MOST_SHOUTS: usize = 6;
/// Ticks a call stays up.
pub const SHOUT_LIFE: u32 = 66;

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// Feet met ground; `n` is how hard (pixels a tick of the fall).
    Land,
    /// A jump left the ground; `n` is which (`body::Jump` as a number).
    Hop,
    /// A running step kicked dust back.
    Step,
    /// A skid on the turn.
    Skid,
    /// A foe stomped; `n` is the stomp chain.
    Stomp,
    /// A coin pinged out of a rune block, or one gathered; `n` 0 block, 1 gathered.
    Coin,
    /// A rune block struck from below, or a brick that held.
    Block,
    /// A banner raised; `n` is which.
    Banner,
    /// A checkpoint gate raised.
    Gate,
    /// A spark of the ring gathered.
    Spark,
    /// A ground pound landed; `n` is its radius.
    Slam,
    /// Sprung off a friend's raised shield.
    Spring,
    /// A knight was hurt; `n` is the way thrown (-1 left, 1 right).
    Hurt,
    /// A knight fell into a pit or the water.
    Pit,
    /// A bubble popped: a friend brought a knight back.
    Rescue,
    /// A loaf or a flame taken: the knight grows.
    Power,
    /// A knight went down into a bubble.
    Down,
    /// A plate fell off King Cinder's crown; `n` the plates left.
    Plate,
    /// King Cinder's last crown fell.
    Crown,
    /// A cap kicked, caught or struck.
    Cap,
    /// A wall kick: dust off the wall.
    Kick,
}

/// One thing that happened, where and to whom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub kind: Kind,
    /// World pixels (the feet, or the thing struck).
    pub x: i32,
    pub y: i32,
    /// The seat it happened to (0 for the world).
    pub who: u8,
    pub n: i16,
    pub born: u32,
}

impl Mark {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
    /// Ticks the picture keeps it.
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

/// A call the whole table sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// A friend brought a knight out of a bubble.
    Saved,
    /// Sprung off a friend's shield.
    Lift,
    /// A banner raised with this many knights standing together.
    Together(u8),
    /// A stomp chain of this many.
    Chain(u8),
    /// A plate off the crown.
    Plate,
    /// The crown is down.
    Crown,
    /// Gold sparks by the hundred.
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
    /// Weighty blows (the kit's): shake, flash and the realm's hit-stop.
    pub juice: Juice,
    pub marks: Vec<Mark>,
    pub shouts: Vec<Shout>,
    /// The heaviest blow of this tick, for the hit-stop; never saved.
    #[serde(skip)]
    pub hold: Option<Weight>,
}

/// The kit's blows hold floats, but `valid` keeps them finite, so equality is total.
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
    /// Let go what is old at `now`, and forget last tick's hold.
    pub fn tick(&mut self, now: u32) {
        self.juice.tick(now);
        self.marks.retain(|m| m.age(now) < m.life());
        self.shouts.retain(|s| s.age(now) < SHOUT_LIFE);
        self.hold = None;
    }
    pub fn mark(&mut self, now: u32, kind: Kind, x: i32, y: i32, who: u32, n: i32) {
        if self.marks.len() >= MOST_MARKS {
            // Drop the oldest of the short-lived kind first: dust never crowds out a banner.
            let cheap = self.marks.iter().position(|m| matches!(m.kind, Kind::Step | Kind::Hop | Kind::Skid | Kind::Kick));
            self.marks.remove(cheap.unwrap_or(0));
        }
        self.marks.push(Mark { kind, x, y, who: who.min(255) as u8, n: n.clamp(-30000, 30000) as i16, born: now });
    }
    /// A weighty blow: it shakes and flashes, and asks the room to hold.
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
    /// The latest mark of `kind` for seat `who`.
    pub fn latest(&self, kind: Kind, who: u32) -> Option<&Mark> {
        self.marks.iter().rev().find(|m| m.kind == kind && u32::from(m.who) == who)
    }
}
