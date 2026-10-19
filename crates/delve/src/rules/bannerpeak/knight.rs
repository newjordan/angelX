//! A seated knight: the body, a pie of eight wedges of health, a size that is
//! a layer of armour (small, big, flame), gold sparks, a camera of their own
//! and what happens when they fall, are hurt, or are knocked into a bubble.
use super::body::{Body, Cmd, Q};
use super::layout::{HEIGHT, WIDTH};
use super::world::TILE;
use crate::rules::Input;
use serde::{Deserialize, Serialize};

/// The pie: most wedges.
pub const PIPS: u8 = 8;
/// Wedges a fresh knight starts with, and has when brought back by a friend or
/// by the checkpoint.
pub const START_PIPS: u8 = 6;
pub const BACK_PIPS: u8 = 3;
pub const WIPE_PIPS: u8 = 4;
/// Ticks of grace after a hit or a return.
pub const HURT_GRACE: u32 = 70;
/// Ticks a bubble floats before the checkpoint takes the knight back.
pub const BUBBLE_TICKS: u32 = 330;
/// Ticks between a fall into a pit and the return to the last safe ledge.
pub const PIT_TICKS: u32 = 14;
/// Ticks of the item-get pose.
pub const POSE_TICKS: u32 = 54;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Size {
    Small,
    Big,
    /// Big, and carrying a lantern flame.
    Flame,
}

/// The bubble a fallen knight floats in: centre in fixed units and the ticks
/// it has floated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bubble {
    pub x: i32,
    pub y: i32,
    pub t: u32,
}

/// Button bits kept between ticks.
pub const B_JUMP: u8 = 1;
pub const B_DOWN: u8 = 2;
pub const B_ACT: u8 = 4;
pub const B_BRACE: u8 = 8;
pub const B_DIVE: u8 = 16;
pub const B_USE: u8 = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knight {
    /// The tick of the run this knight sat down at (0: from the start).
    pub joined: u32,
    pub body: Body,
    pub size: Size,
    pub pips: u8,
    pub coins: u32,
    pub score: u32,
    /// Stomps since the feet last touched ground: each is worth double.
    pub chain: u8,
    /// Ticks of grace left after a hit.
    pub hurt: u32,
    /// Floating in a bubble, waiting for a friend.
    pub bubble: Option<Bubble>,
    /// Ticks until a fall into a pit is over (the knight is out of sight).
    pub pit: u32,
    /// Ticks left of the item-get pose.
    pub pose: u32,
    /// The flagpole: ticks since the grab, 0 for none.
    pub pole: u32,
    /// Tiles up the pole the grab was, for the score.
    pub grip: i32,
    /// The pole has been had: no second grab.
    pub poled: bool,
    /// The last still, safe footing, pixels.
    pub safe: (i32, i32),
    pub safe_t: u32,
    /// The camera's top-left, pixels.
    pub cam: (i32, i32),
    pub prev: u8,
    pub cooldown: u32,
    /// Carrying a cap.
    pub carry: bool,
    /// Ticks the slam's shock is drawn, and where.
    pub shock: u32,
    /// Ticks since the knight last landed, for the dust (render).
    pub landed: u32,
}

impl Knight {
    /// A knight standing at the feet position `(x, y)` pixels.
    pub fn at(joined: u32, x: i32, y: i32) -> Knight {
        Knight {
            joined,
            body: Body::new(x, y),
            size: Size::Small,
            pips: START_PIPS,
            coins: 0,
            score: 0,
            chain: 0,
            hurt: 0,
            bubble: None,
            pit: 0,
            pose: 0,
            pole: 0,
            grip: 0,
            poled: false,
            safe: (x, y),
            safe_t: 0,
            cam: (0, 0),
            prev: 0,
            cooldown: 0,
            carry: false,
            shock: 0,
            landed: 0,
        }
    }
    /// Standing and playing: not in a bubble, a pit or a pose.
    pub fn free(&self) -> bool {
        self.bubble.is_none() && self.pit == 0 && self.pose == 0 && self.pole == 0
    }
    /// Able to rescue a friend, take a hit and be hit.
    pub fn alive(&self) -> bool {
        self.bubble.is_none() && self.pit == 0
    }
    pub fn big(&self) -> bool {
        self.size != Size::Small
    }
    /// The keys of this tick as the body wants them, and the button bits to
    /// remember. A press is a key that was not down last tick.
    pub fn cmd(&self, i: Input) -> (Cmd, u8) {
        let up = i.move_y < 0;
        let jump = i.dash || up;
        let down = i.move_y > 0;
        let mut bits = 0;
        if jump {
            bits |= B_JUMP;
        }
        if down {
            bits |= B_DOWN;
        }
        if i.fire {
            bits |= B_ACT;
        }
        if i.interact {
            bits |= B_BRACE;
        }
        if i.swing {
            bits |= B_DIVE;
        }
        let edge = |b: u8| bits & b != 0 && self.prev & b == 0;
        // Fire dives in the air for a knight with no flame to throw.
        let dive = edge(B_DIVE) || (edge(B_ACT) && self.size != Size::Flame && !self.carry);
        (
            Cmd {
                x: i.move_x,
                jump,
                jump_edge: edge(B_JUMP),
                down,
                down_edge: edge(B_DOWN),
                dive_edge: dive,
                brace: i.interact,
            },
            bits,
        )
    }
    /// Whether fire was pressed this tick (for flames and for picking up).
    pub fn act_edge(&self, bits: u8) -> bool {
        bits & B_ACT != 0 && self.prev & B_ACT == 0
    }
    pub fn use_edge(&self, bits: u8) -> bool {
        bits & B_BRACE != 0 && self.prev & B_BRACE == 0
    }
    /// Take a hit from the side `from` (-1 or 1 for the way it came). Returns
    /// whether it landed; a size is lost before a wedge.
    pub fn hit(&mut self, from: i8) -> bool {
        if self.hurt > 0 || !self.free() {
            return false;
        }
        self.hurt = HURT_GRACE;
        self.chain = 0;
        self.carry = false;
        match self.size {
            Size::Flame => self.size = Size::Big,
            Size::Big => self.size = Size::Small,
            Size::Small => self.pips = self.pips.saturating_sub(1),
        }
        self.body.big = self.big();
        self.body.hurt(from);
        true
    }
    pub fn valid(&self) -> bool {
        self.body.valid()
            && self.pips <= PIPS
            && self.hurt <= HURT_GRACE
            && self.pit <= PIT_TICKS
            && self.pose <= POSE_TICKS
            && self.pole < 600
            && (0..=9).contains(&self.grip)
            && self.safe_t < 1 << 24
            && self.cooldown <= 60
            && self.shock <= 12
            && self.landed < 1 << 24
            && self.body.big == self.big()
            && self.chain <= 12
            && self.coins < 1 << 20
            && self.score < 1 << 28
            && (0..=WIDTH * TILE).contains(&self.safe.0)
            && (-64..=HEIGHT * TILE + 64).contains(&self.safe.1)
            && (-512..=WIDTH * TILE + 512).contains(&self.cam.0)
            && (-512..=HEIGHT * TILE + 512).contains(&self.cam.1)
            && self.bubble.is_none_or(|b| {
                (0..=WIDTH * TILE * Q).contains(&b.x)
                    && (-8 * TILE * Q..=(HEIGHT + 4) * TILE * Q).contains(&b.y)
                    && b.t <= BUBBLE_TICKS
            })
    }
}

/// The camera `cam` eased toward a knight at feet `(fx, fy)` pixels facing
/// `face`, moving `vx`/`vy` (fixed units): it leads the way of travel, shows
/// the drop on a fall, and never lets the knight near the edge of the view.
pub fn follow(cam: (i32, i32), fx: i32, fy: i32, face: i8, vx: i32, vy: i32) -> (i32, i32) {
    const W: i32 = 384;
    const H: i32 = 224;
    let lead = (i32::from(face) * 26 + vx / 40).clamp(-70, 70);
    let want_x = (fx + lead - W / 2).clamp(0, WIDTH * TILE - W);
    // Feet a little below the middle; a fall pulls the view down after it.
    let look = if vy > 1500 { ((vy - 1500) / 40).min(48) } else { 0 };
    let want_y = (fy - 158 + look).clamp(0, HEIGHT * TILE - H);
    let step = |c: i32, want: i32, most: i32| c + (want - c).clamp(-most, most);
    let mut x = step(cam.0, want_x, 7);
    let mut y = step(cam.1, want_y, if vy > 1500 { 14 } else { 6 });
    // Never lose the knight: keep them inside the middle of the view.
    x = x.clamp(fx - 270, fx - 114).clamp(0, WIDTH * TILE - W);
    y = y.clamp(fy - 200, fy - 70).clamp(0, HEIGHT * TILE - H);
    (x, y)
}
