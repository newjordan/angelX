//! A knight's body: the whole of the jump vocabulary on integer ground.
//!
//! Position is the feet's centre in 1/256 pixels, speed is 1/256 pixel a tick
//! (30 Hz). Everything here is fixed integer math: the same inputs on the same
//! map replay the same body. The body knows the tile map and the moving
//! planks (logs, wheel buckets, a friend's raised shield); it knows nothing of
//! foes, blocks' contents or the score. What the world should hear of (a
//! landing, a bumped block, a slam) it reports as `Ev`, and the session does
//! the rest.
//!
//! Buttons (existing ones only): A and D run, Space or W jump, S crouch (and
//! slam in the air), F or Q dive in the air, E held raises the shield.
use super::world::{Grid, TILE, Tile};
use crate::rules::rafterwake::{Plank, landing};
use serde::{Deserialize, Serialize};

pub const Q: i32 = 256;
/// A tile in fixed units.
pub const TQ: i32 = TILE * Q;

/// The five numbers a playtester moves (the F2 knobs), as the body reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Knobs {
    /// Top run speed, 1/256 px a tick.
    pub run: i32,
    /// Ticks after a landing in which the next hop of a triple jump may start.
    pub hop: u32,
    /// Jump buffer in ticks; the coyote time is two more.
    pub grace: u32,
    /// The slam's shock radius in pixels.
    pub slam: i32,
    /// The upward speed a raised shield springs a friend with.
    pub ledge: i32,
}

pub const TOP_ACCEL: i32 = 56;
pub const AIR_ACCEL: i32 = 44;
pub const AIR_DRAG: i32 = 10;
pub const FRICTION: i32 = 84;
pub const SKID_BRAKE: i32 = 184;
/// Below this speed turning round is not a skid.
pub const SKID_MIN: i32 = 520;
pub const SLIDE_FRICTION: i32 = 14;
pub const SLIDE_MIN: i32 = 420;
pub const CROUCH_FRICTION: i32 = 110;
pub const GRAV_UP: i32 = 150;
pub const GRAV_CUT: i32 = 340;
pub const GRAV_DOWN: i32 = 210;
pub const GRAV_LONG: i32 = 90;
pub const FALL_MAX: i32 = 3584;
pub const JUMP_V: i32 = 2200;
/// A run adds this fraction of its speed to the jump.
pub const JUMP_FROM_SPEED: i32 = 5;
/// A released jump rises no faster than this.
pub const CUT_V: i32 = 820;
/// The hops of a triple jump are this much (percent) of a plain one.
pub const HOP2_PERCENT: i32 = 111;
pub const HOP3_PERCENT: i32 = 133;
/// Slower than this the hops do not chain.
pub const HOP_MIN_SPEED: i32 = 560;
pub const LONG_MIN_SPEED: i32 = 760;
pub const LONG_V: i32 = 1560;
pub const LONG_PERCENT: i32 = 145;
pub const LONG_MAX: i32 = 1900;
pub const BACK_V: i32 = 2800;
pub const BACK_VX: i32 = 300;
pub const KICK_V: i32 = 1900;
pub const KICK_VX: i32 = 1150;
pub const WALL_SLIDE: i32 = 560;
pub const WIND: u32 = 6;
pub const SLAM_V: i32 = 3200;
pub const RECOVER: u32 = 9;
/// A jump pressed this soon after a slam lands springs high.
pub const SLAM_JUMP_WINDOW: u32 = 5;
pub const SLAM_JUMP_V: i32 = 2700;
pub const DIVE_VX: i32 = 1800;
pub const DIVE_VY: i32 = 700;
pub const BELLY: u32 = 16;
pub const BELLY_FRICTION: i32 = 30;
pub const BOUNCE_V: i32 = 1500;
pub const BOUNCE_HELD_V: i32 = 2350;
pub const HURT_TICKS: u32 = 14;
/// A shield needs this long raised before a friend may stand on it.
pub const BRACE_TICKS: u32 = 5;
/// How far into a ledge's top a knight still lands on it, pixels.
pub const LEDGE_LIP: i32 = 6;
/// Movers with an id from here up are raised shields (a seat's id is added).
pub const SHIELD_ID: u16 = 1000;

/// What the body is doing, apart from its pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Move {
    Ground,
    Air,
    Wall,
    /// The slam's hover before the drop.
    Wind,
    /// The slam falling.
    Pound,
    /// Stunned after a slam lands.
    Recover,
    Dive,
    /// Sliding on the belly after a dive.
    Belly,
    Hurt,
    /// Standing with the shield raised.
    Brace,
}

/// Which jump left the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Jump {
    None,
    Plain,
    Second,
    Triple,
    Long,
    Back,
    Kick,
    Bounce,
    Spring,
}

/// What a picture shows (the art slots follow these).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pose {
    Idle,
    Run,
    RunB,
    Skid,
    Crouch,
    Slide,
    Rise,
    Fall,
    Second,
    Spin,
    Long,
    Back,
    Wall,
    Wind,
    Slam,
    Land,
    Dive,
    Belly,
    Hurt,
    Brace,
    Item,
    Pole,
    Bubble,
}

/// A knight's keys for one tick, after the edges are found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cmd {
    pub x: i8,
    pub jump: bool,
    pub jump_edge: bool,
    pub down: bool,
    pub down_edge: bool,
    pub dive_edge: bool,
    pub brace: bool,
}

/// What the body reports to the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ev {
    Jump(Jump),
    Land { fell: i32 },
    /// The head struck the tile `(x, y)`.
    Bump { x: i32, y: i32 },
    /// A slam landed with its feet at `(x, y)` pixels.
    Slam { x: i32, y: i32 },
    Skid,
    /// The shield sprung this body off a friend's shield.
    Spring,
    /// Crouched on a hatch.
    Hatch,
}

/// A moving thing that can be stood on: a log, a wheel bucket, a shield.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mover {
    pub id: u16,
    pub p: Plank,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub face: i8,
    pub mode: Move,
    pub jump: Jump,
    pub big: bool,
    pub ground: bool,
    /// The mover this body stands on.
    pub plank: Option<u16>,
    pub coyote: u32,
    pub buffer: u32,
    /// The hop the next ground jump would be (2 or 3), and its window.
    pub hop_next: u8,
    pub hop_t: u32,
    /// The hop level of the jump in the air (1 to 3, 0 for others).
    pub hop: u8,
    pub crouch: bool,
    /// The side of the wall slid on or kicked off (-1 left, 1 right).
    pub wall: i8,
    pub wall_t: u32,
    pub t: u32,
    pub freeze: u32,
    pub squash: u32,
    pub cut: bool,
    pub skid: bool,
    was_skid: bool,
    pub pose: Pose,
    pub run_t: u32,
}

impl Body {
    pub fn new(x_px: i32, y_px: i32) -> Body {
        Body {
            x: x_px * Q,
            y: y_px * Q,
            vx: 0,
            vy: 0,
            face: 1,
            mode: Move::Ground,
            jump: Jump::None,
            big: false,
            ground: true,
            plank: None,
            coyote: 0,
            buffer: 0,
            hop_next: 0,
            hop_t: 0,
            hop: 0,
            crouch: false,
            wall: 0,
            wall_t: 0,
            t: 0,
            freeze: 0,
            squash: 0,
            cut: false,
            skid: false,
            was_skid: false,
            pose: Pose::Idle,
            run_t: 0,
        }
    }
    pub fn valid(&self) -> bool {
        (-(8 * TQ)..=(1200 * TQ)).contains(&self.x)
            && (-(64 * TQ)..=(80 * TQ)).contains(&self.y)
            && (-6000..=6000).contains(&self.vx)
            && (-6000..=6000).contains(&self.vy)
            && [-1, 1].contains(&self.face)
            && self.coyote <= 12
            && self.buffer <= 12
            && self.hop_next <= 3
            && self.hop_t <= 40
            && self.hop <= 3
            && [-1, 0, 1].contains(&self.wall)
            && self.wall_t <= 12
            && self.t <= 600
            && self.freeze <= 8
            && self.squash <= 8
            && self.run_t < 1 << 24
    }
    /// Width and height in pixels.
    pub fn dims(&self) -> (i32, i32) {
        let low = self.crouch || matches!(self.mode, Move::Dive | Move::Belly | Move::Recover);
        match (self.big, low) {
            (false, false) => (12, 20),
            (false, true) => (12, 12),
            (true, false) => (14, 30),
            (true, true) => (14, 18),
        }
    }
    /// The standing height, whatever the crouch.
    pub fn stand_height(&self) -> i32 {
        if self.big { 30 } else { 20 }
    }
    /// The box in fixed units: left, top, right, bottom.
    pub fn rect(&self) -> (i32, i32, i32, i32) {
        let (w, h) = self.dims();
        (
            self.x - w * Q / 2,
            self.y - h * Q,
            self.x + w * Q / 2,
            self.y,
        )
    }
    /// The feet in pixels.
    pub fn foot(&self) -> (i32, i32) {
        (self.x.div_euclid(Q), self.y.div_euclid(Q))
    }
    /// Whether a slam is under way: the body falling on whatever is below.
    pub fn slamming(&self) -> bool {
        matches!(self.mode, Move::Pound)
    }
    /// The shield, raised: a friend may stand on it.
    pub fn shield(&self, id: u16) -> Option<Mover> {
        (self.mode == Move::Brace && self.t >= BRACE_TICKS && self.ground).then(|| {
            let (_, h) = self.dims();
            Mover {
                id,
                p: Plank {
                    x: self.x.div_euclid(Q) - 9,
                    y: self.y.div_euclid(Q) - h - 1,
                    w: 18,
                    lift: false,
                },
            }
        })
    }
    /// A hit: thrown back and stunned for a moment.
    pub fn hurt(&mut self, from: i8) {
        self.mode = Move::Hurt;
        self.t = 0;
        self.vx = -i32::from(from.signum()) * 760;
        self.vy = -1500;
        self.ground = false;
        self.plank = None;
        self.crouch = false;
        self.jump = Jump::None;
    }
    /// Bounced off a foe's head.
    pub fn bounce(&mut self, held: bool) {
        self.vy = -if held { BOUNCE_HELD_V } else { BOUNCE_V };
        self.mode = Move::Air;
        self.ground = false;
        self.plank = None;
        self.jump = Jump::Bounce;
        self.hop = 0;
        self.hop_t = 0;
        self.cut = true;
    }
    /// Put somewhere, still.
    pub fn place(&mut self, x_px: i32, y_px: i32) {
        self.x = x_px * Q;
        self.y = y_px * Q;
        self.vx = 0;
        self.vy = 0;
        self.mode = Move::Ground;
        self.jump = Jump::None;
        self.ground = true;
        self.plank = None;
        self.crouch = false;
        self.hop_t = 0;
        self.hop = 0;
        self.t = 0;
        self.freeze = 0;
        self.buffer = 0;
        self.wall_t = 0;
    }

    fn solid_at(g: &dyn Grid, tx: i32, ty: i32) -> bool {
        g.tile(tx, ty).solid()
    }
    /// Whether any solid tile overlaps the fixed-unit box.
    fn blocked(g: &dyn Grid, l: i32, t: i32, r: i32, b: i32) -> bool {
        if r <= l || b <= t {
            return false;
        }
        for ty in t.div_euclid(TQ)..=(b - 1).div_euclid(TQ) {
            for tx in l.div_euclid(TQ)..=(r - 1).div_euclid(TQ) {
                if Self::solid_at(g, tx, ty) {
                    return true;
                }
            }
        }
        false
    }
    /// After a size change: a body that grew under a low roof stays crouched.
    pub fn regrow(&mut self, g: &dyn Grid) {
        if !self.can_stand(g) {
            self.crouch = true;
        }
    }
    fn can_stand(&self, g: &dyn Grid) -> bool {
        let (w, h) = self.dims();
        let top = self.y - self.stand_height() * Q;
        !Self::blocked(g, self.x - w * Q / 2, top, self.x + w * Q / 2, self.y - h * Q)
    }
    /// The side (-1 or 1) a solid wall touches the body on, 0 for none.
    pub fn wall_side(&self, g: &dyn Grid, toward: i8) -> i8 {
        let (w, h) = self.dims();
        let (t, b) = (self.y - h * Q + 2 * Q, self.y - 2 * Q);
        for side in [toward, -toward] {
            if side == 0 {
                continue;
            }
            let (l, r) = if side > 0 {
                (self.x + w * Q / 2, self.x + w * Q / 2 + Q + Q / 2)
            } else {
                (self.x - w * Q / 2 - Q - Q / 2, self.x - w * Q / 2)
            };
            if Self::blocked(g, l, t, r, b) {
                return side;
            }
        }
        0
    }
    fn tile_below(&self, g: &dyn Grid) -> Tile {
        g.tile(self.x.div_euclid(TQ), self.y.div_euclid(TQ))
    }

    /// One tick of this body.
    pub fn step(
        &mut self,
        c: Cmd,
        k: &Knobs,
        g: &dyn Grid,
        now: &[Mover],
        before: &[Mover],
        out: &mut Vec<Ev>,
    ) {
        if c.jump_edge {
            // The tick of the press counts as the first of the grace.
            self.buffer = k.grace + 1;
        }
        self.hop_t = self.hop_t.saturating_sub(1);
        self.squash = self.squash.saturating_sub(1);
        if self.freeze > 0 {
            self.freeze -= 1;
            self.buffer = self.buffer.saturating_sub(1);
            return;
        }
        // Carried by what it stands on.
        if let Some(id) = self.plank {
            let a = now.iter().find(|m| m.id == id);
            let b = before.iter().find(|m| m.id == id);
            if let (Some(a), Some(b)) = (a, b) {
                self.x += (a.p.x - b.p.x) * Q;
                self.y += (a.p.y - b.p.y) * Q;
            } else {
                self.plank = None;
                self.ground = false;
            }
        }
        self.unstick(g);
        self.was_skid = self.skid;
        self.skid = false;
        if self.mode == Move::Brace
            && (!(c.brace && self.ground) || c.jump_edge || c.down)
        {
            self.mode = Move::Ground;
            self.t = 0;
        }
        let was_ground = self.ground;
        if self.ground {
            self.coyote = k.grace + 2;
        }
        match self.mode {
            Move::Wind => {
                self.vx = 0;
                self.vy = 0;
                self.t += 1;
                if self.t >= WIND {
                    self.mode = Move::Pound;
                    self.vy = SLAM_V;
                    self.t = 0;
                }
            }
            Move::Pound => {
                self.vx = 0;
                self.vy = SLAM_V;
            }
            Move::Recover => {
                self.t += 1;
                self.vx = approach(self.vx, 0, FRICTION);
                if self.t <= SLAM_JUMP_WINDOW && self.buffer > 0 {
                    self.vy = -SLAM_JUMP_V;
                    self.mode = Move::Air;
                    self.jump = Jump::Plain;
                    self.hop = 0;
                    self.hop_t = 0;
                    self.ground = false;
                    self.plank = None;
                    self.buffer = 0;
                    self.cut = true;
                    out.push(Ev::Jump(Jump::Plain));
                } else if self.t >= RECOVER {
                    self.mode = Move::Ground;
                }
                self.gravity(c);
            }
            Move::Hurt => {
                self.t += 1;
                if self.t >= HURT_TICKS {
                    self.mode = if self.ground { Move::Ground } else { Move::Air };
                }
                self.gravity(c);
            }
            Move::Dive => {
                self.t += 1;
                self.gravity(c);
            }
            Move::Belly => {
                self.t += 1;
                self.vx = approach(self.vx, 0, BELLY_FRICTION);
                if self.t >= BELLY {
                    self.mode = Move::Ground;
                } else if self.t >= 3 && self.buffer > 0 {
                    self.vy = -(JUMP_V * 70 / 100);
                    self.mode = Move::Air;
                    self.jump = Jump::Plain;
                    self.hop = 0;
                    self.hop_t = 0;
                    self.ground = false;
                    self.plank = None;
                    self.buffer = 0;
                    out.push(Ev::Jump(Jump::Plain));
                }
                self.gravity(c);
            }
            Move::Brace => {
                // Held as long as a friend needs it; the stance only reads
                // `t >= BRACE_TICKS`, so the count stops there.
                self.t = (self.t + 1).min(BRACE_TICKS);
                self.vx = approach(self.vx, 0, FRICTION);
                self.gravity(c);
            }
            Move::Ground | Move::Air | Move::Wall => {
                self.free(c, k, g, out);
            }
        }
        self.buffer = self.buffer.saturating_sub(1);
        // Move: along, then up or down.
        let (dx, dy) = (self.vx, self.vy);
        let blocked_x = self.move_x(g, dx);
        if blocked_x && self.mode == Move::Dive {
            self.mode = Move::Air;
        }
        self.move_y(g, now, dy, k, c.x, out);
        if !self.ground && self.plank.is_none() {
            self.coyote = self.coyote.saturating_sub(1);
        }
        self.wall_t = self.wall_t.saturating_sub(1);
        if !self.ground && self.mode == Move::Ground {
            self.mode = Move::Air;
        }
        // Landing.
        if self.ground && !was_ground {
            self.landed(k, out);
        }
        if self.ground
            && self.mode == Move::Ground
            && c.down_edge
            && self.tile_below_feet(g) == Tile::Hatch
        {
            out.push(Ev::Hatch);
        }
        self.settle_pose(c);
    }

    fn tile_below_feet(&self, g: &dyn Grid) -> Tile {
        self.tile_below(g)
    }

    /// Gravity for the modes that fall without steering.
    fn gravity(&mut self, c: Cmd) {
        let g = match self.mode {
            Move::Dive | Move::Hurt => GRAV_DOWN,
            _ if self.jump == Jump::Long => GRAV_LONG,
            _ if self.vy < 0 => {
                if c.jump || !self.cuttable() {
                    GRAV_UP
                } else {
                    GRAV_CUT
                }
            }
            _ => GRAV_DOWN,
        };
        if !self.ground || self.vy < 0 {
            self.vy = (self.vy + g).min(FALL_MAX);
        } else {
            self.vy = g;
        }
    }
    fn cuttable(&self) -> bool {
        matches!(self.jump, Jump::Plain | Jump::Second)
    }

    /// The ordinary modes: run, jump, crouch, slide, kick, slam, dive, shield.
    fn free(&mut self, c: Cmd, k: &Knobs, g: &dyn Grid, out: &mut Vec<Ev>) {
        let top = k.run;
        // Snow: slow to get going and slower to stop.
        let (slip_up, slip_down) = if self.ground && self.tile_below(g).slippery() {
            (2, 4)
        } else {
            (1, 1)
        };
        let dir = i32::from(c.x);
        if self.ground {
            // Crouch, and stand again only where there is room.
            if c.down && !self.crouch {
                self.crouch = true;
            } else if !c.down && self.crouch && self.can_stand(g) {
                self.crouch = false;
            }
            if c.brace && !c.down && self.vx.abs() < 260 && !c.jump_edge && self.buffer == 0 {
                self.mode = Move::Brace;
                self.t = 0;
                self.crouch = false;
                self.vx = 0;
                self.vy = GRAV_DOWN;
                return;
            }
            if self.crouch {
                let fric = if self.vx.abs() >= SLIDE_MIN {
                    SLIDE_FRICTION
                } else {
                    CROUCH_FRICTION
                };
                self.vx = approach(self.vx, 0, fric / slip_down);
                if dir != 0 && self.vx.abs() < SLIDE_MIN {
                    self.face = c.x;
                }
            } else if dir == 0 {
                self.vx = approach(self.vx, 0, FRICTION / slip_down);
            } else if self.vx != 0 && self.vx.signum() != dir.signum() && self.vx.abs() > SKID_MIN
            {
                self.vx = approach(self.vx, 0, SKID_BRAKE / slip_down.min(3));
                self.skid = true;
                if !self.was_skid {
                    out.push(Ev::Skid);
                }
            } else {
                self.vx = approach(self.vx, dir * top, TOP_ACCEL / slip_up);
                self.face = c.x;
            }
        } else {
            self.crouch = false;
            if dir != 0 {
                self.face = c.x;
                let aligned = self.vx.signum() == dir.signum();
                if !aligned || self.vx.abs() <= top {
                    // Never slower to turn in the air than on the ground.
                    let target = dir * top;
                    self.vx = approach(self.vx, target, AIR_ACCEL);
                }
            } else if !matches!(self.jump, Jump::Long | Jump::Kick | Jump::Back) {
                self.vx = approach(self.vx, 0, AIR_DRAG);
            }
        }
        // A jump: the same tick as the press, or the next landing's.
        let mut leave = None;
        if self.buffer > 0 {
            if self.mode == Move::Wall && !self.ground {
                leave = Some(Jump::Kick);
            } else if self.ground || self.coyote > 0 {
                leave = Some(self.pick_jump(k));
            } else if self.wall_t > 0 && self.wall != 0 {
                leave = Some(Jump::Kick);
            }
        }
        if let Some(kind) = leave {
            self.launch(kind, k);
            out.push(Ev::Jump(kind));
        } else if !self.ground {
            if c.down_edge {
                self.mode = Move::Wind;
                self.t = 0;
                self.vx = 0;
                self.vy = 0;
                self.crouch = false;
                self.wall_t = 0;
                return;
            }
            if c.dive_edge {
                self.mode = Move::Dive;
                self.t = 0;
                self.vx = i32::from(self.face) * DIVE_VX;
                self.vy = self.vy.max(0) + DIVE_VY;
                self.wall_t = 0;
                return;
            }
        }
        // Rising: let go to cut the jump short.
        if self.vy < 0 && !c.jump && !self.cut && self.cuttable() {
            self.vy = self.vy.max(-CUT_V);
            self.cut = true;
        }
        self.gravity(c);
        if self.mode == Move::Wall {
            self.vy = self.vy.min(WALL_SLIDE);
        }
    }

    /// Which jump a press would make from here.
    fn pick_jump(&self, _k: &Knobs) -> Jump {
        let speed = self.vx.abs();
        if self.plank.is_some_and(|id| id >= SHIELD_ID) {
            Jump::Spring
        } else if self.crouch {
            if speed >= LONG_MIN_SPEED {
                Jump::Long
            } else {
                Jump::Back
            }
        } else if self.hop_t > 0 && self.hop_next >= 2 && speed >= HOP_MIN_SPEED {
            if self.hop_next >= 3 {
                Jump::Triple
            } else {
                Jump::Second
            }
        } else {
            Jump::Plain
        }
    }

    fn launch(&mut self, kind: Jump, k: &Knobs) {
        let speed = self.vx.abs();
        let bonus = speed / JUMP_FROM_SPEED;
        match kind {
            Jump::Plain => {
                self.vy = -(JUMP_V + bonus);
                self.hop = 1;
            }
            Jump::Second => {
                self.vy = -(JUMP_V * HOP2_PERCENT / 100 + bonus);
                self.hop = 2;
            }
            Jump::Triple => {
                self.vy = -(JUMP_V * HOP3_PERCENT / 100 + bonus);
                self.hop = 3;
            }
            Jump::Long => {
                self.vy = -LONG_V;
                let dir = if self.vx != 0 {
                    self.vx.signum()
                } else {
                    i32::from(self.face)
                };
                self.vx = dir * (speed * LONG_PERCENT / 100).min(LONG_MAX);
                self.face = dir as i8;
                self.hop = 0;
            }
            Jump::Spring => {
                self.vy = -k.ledge;
                self.hop = 0;
            }
            Jump::Back => {
                self.vy = -BACK_V;
                self.vx = -i32::from(self.face) * BACK_VX;
                self.hop = 0;
            }
            Jump::Kick => {
                self.vy = -KICK_V;
                self.vx = -i32::from(self.wall) * KICK_VX;
                self.face = -self.wall;
                self.hop = 0;
            }
            Jump::None | Jump::Bounce => {}
        }
        self.jump = kind;
        self.mode = Move::Air;
        self.ground = false;
        self.plank = None;
        self.crouch = false;
        self.coyote = 0;
        self.buffer = 0;
        self.hop_t = 0;
        self.hop_next = 0;
        self.cut = false;
        self.wall_t = 0;
        self.t = 0;
    }

    fn landed(&mut self, k: &Knobs, out: &mut Vec<Ev>) {
        let fell = self.vy.max(0);
        let was = self.mode;
        self.vy = 0;
        self.coyote = k.grace + 2;
        self.cut = false;
        let speed = self.vx.abs();
        // The hops chain only from a plain run of jumps.
        if self.jump == Jump::Plain || self.jump == Jump::Second {
            if self.hop >= 1 && self.hop <= 2 && speed >= HOP_MIN_SPEED {
                self.hop_next = self.hop + 1;
                self.hop_t = k.hop + 1;
            } else {
                self.hop_next = 0;
                self.hop_t = 0;
            }
        } else {
            self.hop_next = 0;
            self.hop_t = 0;
        }
        self.hop = 0;
        self.jump = Jump::None;
        self.wall = 0;
        match was {
            Move::Pound | Move::Wind => {
                self.mode = Move::Recover;
                self.t = 0;
                self.freeze = 3;
                self.hop_next = 0;
                self.hop_t = 0;
                out.push(Ev::Slam {
                    x: self.x.div_euclid(Q),
                    y: self.y.div_euclid(Q),
                });
            }
            Move::Dive => {
                self.mode = Move::Belly;
                self.t = 0;
            }
            Move::Hurt => {}
            _ => {
                self.mode = Move::Ground;
                self.t = 0;
            }
        }
        if fell > 1700 {
            self.squash = 4;
        }
        out.push(Ev::Land { fell });
    }

    /// Out of a tile that has closed round it, upward.
    fn unstick(&mut self, g: &dyn Grid) {
        for _ in 0..16 {
            let (l, t, r, b) = self.rect();
            if !Self::blocked(g, l, t + 1, r, b - 1) {
                break;
            }
            self.y -= Q;
        }
    }

    fn move_x(&mut self, g: &dyn Grid, dx: i32) -> bool {
        if dx == 0 {
            return false;
        }
        let (w, h) = self.dims();
        let half = w * Q / 2;
        let nx = self.x + dx;
        let edge = if dx > 0 { nx + half - 1 } else { nx - half };
        let col = edge.div_euclid(TQ);
        let (top, bottom) = (
            (self.y - h * Q).div_euclid(TQ),
            (self.y - 1).div_euclid(TQ),
        );
        for row in top..=bottom {
            if Self::solid_at(g, col, row) {
                self.x = if dx > 0 {
                    col * TQ - half
                } else {
                    (col + 1) * TQ + half
                };
                self.vx = 0;
                return true;
            }
        }
        self.x = nx;
        false
    }

    fn move_y(
        &mut self,
        g: &dyn Grid,
        now: &[Mover],
        dy: i32,
        k: &Knobs,
        toward: i8,
        out: &mut Vec<Ev>,
    ) {
        let (w, h) = self.dims();
        let half = w * Q / 2;
        let old = self.y;
        let new = old + dy;
        if dy >= 0 {
            // Falling or standing: land on the first tile top, or plank, crossed.
            let (c0, c1) = (
                (self.x - half).div_euclid(TQ),
                (self.x + half - 1).div_euclid(TQ),
            );
            let mut stop: Option<i32> = None;
            // A ledge keeps a knight whose feet sank a few pixels into it.
            let first = (old - LEDGE_LIP * Q + TQ - 1).div_euclid(TQ);
            let last = new.div_euclid(TQ);
            'rows: for row in first..=last {
                let top = row * TQ;
                for col in c0..=c1 {
                    let t = g.tile(col, row);
                    if (t.solid() && old <= top) || (t.one_way() && old <= top + LEDGE_LIP * Q) {
                        stop = Some(top);
                        break 'rows;
                    }
                }
            }
            let mut on: Option<u16> = None;
            if self.vy >= 0 {
                let planks: Vec<Plank> = now.iter().map(|m| m.p).collect();
                if let Some((i, p)) = landing(old, new, self.x.div_euclid(Q), &planks) {
                    let top = p.y * Q;
                    if stop.is_none_or(|s| top < s) {
                        stop = Some(top);
                        on = Some(now[i].id);
                    }
                }
            }
            if let Some(top) = stop {
                self.y = top;
                self.ground = true;
                self.plank = on;
                if self.mode == Move::Wall {
                    self.mode = Move::Ground;
                }
                return;
            }
            self.y = new;
            self.ground = false;
            self.plank = None;
            self.wall_check(g, k, toward);
        } else {
            // Rising: the head meets the first tile above.
            let head = old - h * Q;
            let new_head = head + dy;
            let (c0, c1) = (
                (self.x - half).div_euclid(TQ),
                (self.x + half - 1).div_euclid(TQ),
            );
            self.ground = false;
            self.plank = None;
            for row in (new_head.div_euclid(TQ)..=(head - 1).div_euclid(TQ)).rev() {
                let hit = (c0..=c1).any(|col| Self::solid_at(g, col, row));
                if hit {
                    // A rim clipped by a few pixels is slipped past, not struck.
                    let l_col = Self::solid_at(g, c0, row);
                    let r_col = Self::solid_at(g, c1, row);
                    if c0 != c1 && l_col != r_col {
                        let slip = if l_col {
                            (c0 + 1) * TQ - (self.x - half)
                        } else {
                            (self.x + half) - c1 * TQ
                        };
                        if slip <= 5 * Q {
                            self.x += if l_col { slip } else { -slip };
                            let free = !Self::blocked(
                                g,
                                self.x - half,
                                new_head,
                                self.x + half,
                                head,
                            );
                            if free {
                                self.y = new;
                                return;
                            }
                            self.x -= if l_col { slip } else { -slip };
                        }
                    }
                    self.y = (row + 1) * TQ + h * Q;
                    self.vy = 0;
                    let col = if c0 == c1 {
                        c0
                    } else {
                        let centre = self.x.div_euclid(TQ);
                        if Self::solid_at(g, centre, row) { centre } else if l_col { c0 } else { c1 }
                    };
                    out.push(Ev::Bump { x: col, y: row });
                    return;
                }
            }
            self.y = new;
            self.wall_check(g, k, toward);
        }
    }

    /// Falling against a wall with the stick toward it: slide down it.
    fn wall_check(&mut self, g: &dyn Grid, k: &Knobs, toward: i8) {
        if self.vy < 0 || matches!(self.mode, Move::Wind | Move::Pound | Move::Dive | Move::Hurt) {
            if self.mode == Move::Wall {
                self.mode = Move::Air;
            }
            return;
        }
        let side = if toward != 0 { self.wall_side(g, toward) } else { 0 };
        if side != 0 && side == toward {
            self.mode = Move::Wall;
            self.wall = side;
            self.wall_t = k.grace + 1;
            self.vy = self.vy.min(WALL_SLIDE);
        } else if self.mode == Move::Wall {
            self.mode = Move::Air;
        }
    }

    fn settle_pose(&mut self, c: Cmd) {
        self.run_t = self.run_t.wrapping_add((self.vx.abs() / 128) as u32 + 1);
        self.pose = match self.mode {
            Move::Ground => {
                if self.squash > 0 {
                    Pose::Land
                } else if self.crouch {
                    if self.vx.abs() > SLIDE_MIN / 2 {
                        Pose::Slide
                    } else {
                        Pose::Crouch
                    }
                } else if self.skid {
                    Pose::Skid
                } else if self.vx.abs() > 90 && c.x != 0 || self.vx.abs() > 200 {
                    if self.run_t / 24 % 2 == 0 {
                        Pose::Run
                    } else {
                        Pose::RunB
                    }
                } else {
                    Pose::Idle
                }
            }
            Move::Air => match self.jump {
                Jump::Triple => Pose::Spin,
                Jump::Second => Pose::Second,
                Jump::Long => Pose::Long,
                Jump::Back => Pose::Back,
                _ => {
                    if self.vy < 0 {
                        Pose::Rise
                    } else {
                        Pose::Fall
                    }
                }
            },
            Move::Wall => Pose::Wall,
            Move::Wind => Pose::Wind,
            Move::Pound => Pose::Slam,
            Move::Recover => Pose::Land,
            Move::Dive => Pose::Dive,
            Move::Belly => Pose::Belly,
            Move::Hurt => Pose::Hurt,
            Move::Brace => Pose::Brace,
        };
    }
}

/// `v` moved toward `to` by at most `by`.
pub fn approach(v: i32, to: i32, by: i32) -> i32 {
    if v < to {
        (v + by).min(to)
    } else {
        (v - by).max(to)
    }
}
