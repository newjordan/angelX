//! The knight: a body, a lock-on, a sword, a shield, a lantern and a hop.
//! Every seat has all of it. The verbs are the classic lock-on adventure's,
//! changed only for the camera: the view is from above, so a hop is a hop
//! along the ground and a jump attack lands where the knight comes down.
//!
//! Buttons (the Delve's own, nothing new): WASD walk, the arrows face, R
//! (a pad's left shoulder) holds the lock, Space holds the shield, Q is the
//! sword, E is the one context button (and the roll or the hop when there is
//! nothing to do with it), F is the lantern, X lays a bomb.
use super::world::*;
use super::*;

pub const B_LOCK: u8 = 1;
pub const B_GUARD: u8 = 2;
pub const B_SWORD: u8 = 4;
pub const B_CTX: u8 = 8;
pub const B_LAMP: u8 = 16;
pub const B_BOMB: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    Free,
    /// A slash: `Seat::n` is which of the three.
    Slash,
    Lunge,
    Spin,
    Roll,
    /// A hop: `Seat::n` is 0 side, 1 back, 2 forward.
    Hop,
    /// The jump attack coming down.
    Plunge,
    Hurt,
    /// Holding an item overhead.
    Item,
    Down,
    /// Over a pit and going down.
    Fall,
}

/// What a lock holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lock {
    Foe(u8),
    Prop(u8),
    /// The Warden's lamp-heart.
    Boss,
}

/// What the context button would do just now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ctx {
    None,
    Open(u8),
    Read(u8),
    Lift(u8),
    Throw,
    Flame,
    Light(u8),
    Unlock(u8),
    Revive(u8),
    Relight,
    Talk(u8),
    Pull(u8),
}

impl Ctx {
    pub fn label(self) -> &'static str {
        match self {
            Ctx::None => "",
            Ctx::Open(_) => "OPEN",
            Ctx::Read(_) => "READ",
            Ctx::Lift(_) => "LIFT",
            Ctx::Throw => "THROW",
            Ctx::Flame => "TAKE FLAME",
            Ctx::Light(_) => "LIGHT",
            Ctx::Unlock(_) => "UNLOCK",
            Ctx::Revive(_) => "REVIVE",
            Ctx::Relight => "RELIGHT",
            Ctx::Talk(_) => "TALK",
            Ctx::Pull(_) => "PULL",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Carry {
    Nothing,
    Pot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    /// Which seat this is.
    pub id: u8,
    /// The tick of the run this knight sat down at (0: from the start).
    pub joined: u32,
    pub x: i32,
    pub y: i32,
    /// Height off the ground while hopping, in `Q` pixels.
    pub z: i32,
    pub face: (i32, i32),
    /// Quarter hearts.
    pub hp: u32,
    pub max_hp: u32,
    pub gems: u32,
    pub bombs: u32,
    /// Ticks of flare left in the lantern.
    pub flame: u32,
    /// Ticks left of an ember's flame carried to a brazier.
    pub taper: u32,
    pub act: Act,
    pub t: u32,
    pub n: u8,
    /// Where the act carries the body each tick, in `Q` pixels.
    pub mv: (i32, i32),
    /// A blow's push, a few ticks.
    pub push: (i32, i32),
    pub pusht: u8,
    /// The next slash in the chain, and ticks left to start it.
    pub step: u8,
    pub chain: u32,
    pub queued: bool,
    /// Ticks the sword button has been held.
    pub charge: u32,
    /// Ticks the shield has been up (1 on the first), 0 when down.
    pub shield: u32,
    pub guard_cd: u32,
    pub lock: Option<Lock>,
    pub lock_t: u32,
    pub unlock_t: u32,
    pub relock: u32,
    pub inv: u32,
    /// Ticks left of the blink a hit leaves.
    pub ward: u32,
    /// The knight's own hit-stop: a blade that landed holds its owner.
    pub freeze: u32,
    pub swing: u32,
    pub lamp: u32,
    pub lamp_cd: u32,
    pub carry: Carry,
    pub ctx: Ctx,
    /// Sign being read, and how long it stays up.
    pub read: Option<(u8, u32)>,
    /// The aim keys last tick, to catch a fresh press (cycling a lock).
    pub aim: (i8, i8),
    pub held: u8,
    pub safe: (i32, i32),
    pub cam: (i32, i32),
    /// Revive progress while down.
    pub rev: u32,
    /// Ticks the knight has been in no one's way: for the beep.
    pub beep: u32,
    /// The item held up, while `Act::Item`.
    pub item: Option<Item>,
}

impl Seat {
    /// Whether every number is one a save or the wire may carry.
    pub fn valid(&self, id: u32, elapsed: u32, w: i32, h: i32) -> bool {
        u32::from(self.id) == id
            && self.joined <= elapsed
            && (0..=w).contains(&self.x)
            && (0..=h).contains(&self.y)
            && (0..=(HOP_HEIGHT + 2) * Q).contains(&self.z)
            && dist(self.face.0, self.face.1) <= Q + 8
            && self.max_hp <= 12 * HEART
            && self.hp <= self.max_hp
            && self.gems <= 100_000
            && self.bombs <= BOMB_CAP
            && self.flame <= 300
            && self.taper <= TAPER
            && self.t <= 4096
            && self.n <= 3
            && self.step <= 2
            && self.chain <= 64
            && self.charge <= 255
            && self.shield <= 255
            && self.guard_cd <= 64
            && self.lock_t <= 255
            && self.unlock_t <= 64
            && self.relock <= 16
            && self.inv <= 128
            && self.ward <= HURT_INV
            && self.freeze <= 16
            && self.lamp <= 255
            && self.lamp_cd <= 64
            && self.rev <= 128
            && self.pusht <= 16
            && (-(8 * Q)..=8 * Q).contains(&self.mv.0)
            && (-(8 * Q)..=8 * Q).contains(&self.mv.1)
            && (-(8 * Q)..=8 * Q).contains(&self.push.0)
            && (-(8 * Q)..=8 * Q).contains(&self.push.1)
            && (0..=w).contains(&self.cam.0)
            && (0..=h).contains(&self.cam.1)
            && (0..=w).contains(&self.safe.0)
            && (0..=h).contains(&self.safe.1)
            && self.read.is_none_or(|(_, t)| t <= READ_TICKS)
    }

    /// Standing and awake: can be hurt, can hurt, can be a foe's target.
    pub fn up(&self) -> bool {
        self.hp > 0 && self.act != Act::Down
    }

    pub fn flaring(&self) -> bool {
        self.lamp >= FLARE_AFTER && self.flame > 0 && self.act != Act::Down
    }

    /// Fully wound for the spin.
    pub fn charged(&self) -> bool {
        self.charge >= CHARGE_TICKS
    }

    pub fn airborne(&self) -> bool {
        self.z > 0
    }
}
