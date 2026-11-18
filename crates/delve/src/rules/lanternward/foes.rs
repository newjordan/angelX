//! The Gloam's small things: the snuffer, a hooded wisp that lunges along a
//! line it shows first, and the turret that looses an iron bolt along one.
//! Every attack shows its footprint for its whole tell and strikes only
//! there.
use super::world::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fk {
    Snuffer,
    Turret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fs {
    Dormant,
    Drift,
    Tell,
    Strike,
    Recover,
    Stagger,
    Dying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Danger {
    None,
    Capsule { ax: i32, ay: i32, bx: i32, by: i32, r: i32 },
    Circle { x: i32, y: i32, r: i32 },
    Fan { x: i32, y: i32, deg: i32, half: i32, r: i32 },
    Ring { x: i32, y: i32, r: i32, w: i32 },
}

fn seg_dist2(p: (i64, i64), a: (i64, i64), b: (i64, i64)) -> i64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0 {
        0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) * 1024 / len2).clamp(0, 1024)
    };
    let (cx, cy) = (a.0 + dx * t / 1024, a.1 + dy * t / 1024);
    (p.0 - cx) * (p.0 - cx) + (p.1 - cy) * (p.1 - cy)
}

impl Danger {
    pub fn hits(self, x: i32, y: i32, rad: i32) -> bool {
        match self {
            Danger::None => false,
            Danger::Capsule { ax, ay, bx, by, r } => {
                let reach = i64::from(r + rad);
                seg_dist2(
                    (i64::from(x), i64::from(y)),
                    (i64::from(ax), i64::from(ay)),
                    (i64::from(bx), i64::from(by)),
                ) <= reach * reach
            }
            Danger::Circle { x: cx, y: cy, r } => dist(x - cx, y - cy) <= r + rad,
            Danger::Fan { x: cx, y: cy, deg, half, r } => {
                let d = dist(x - cx, y - cy);
                d <= r + rad && (d < rad || dot(unit(x - cx, y - cy), dir(deg)) >= cos_q(half))
            }
            Danger::Ring { x: cx, y: cy, r, w } => (dist(x - cx, y - cy) - r).abs() <= w + rad,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub id: u8,
    pub kind: Fk,
    pub x: i32,
    pub y: i32,
    pub hp: u32,
    pub full: u32,
    pub prov: u8,
    pub st: Fs,
    pub t: u16,
    pub fx: i32,
    pub fy: i32,
    pub danger: Danger,
    pub hooded: bool,
    pub lit: u16,
    pub flinch: u8,
    pub freeze: u8,
    pub dead: u8,
    pub open: bool,
    pub hit: [u32; 6],
    pub struck: u8,
    pub kb: (i32, i32),
    pub kbt: u8,
    pub n: u8,
}

impl Foe {
    pub fn alive(&self) -> bool {
        self.hp > 0 && self.st != Fs::Dying
    }

    pub fn radius(&self) -> i32 {
        match self.kind {
            Fk::Snuffer => 6 * Q,
            Fk::Turret => 8 * Q,
        }
    }

    pub fn vulnerable(&self) -> bool {
        match self.kind {
            Fk::Snuffer => !(self.hooded && self.lit == 0),
            Fk::Turret => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bk {
    Iron,
    Light,
    Pot,
    Bright,
    Dim,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bolt {
    pub kind: Bk,
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub ttl: u16,
    pub owner: u8,
}

impl Bolt {
    pub fn light(&self) -> bool {
        matches!(self.kind, Bk::Light | Bk::Bright)
    }
    pub fn hostile(&self) -> bool {
        self.kind == Bk::Iron
    }
}
