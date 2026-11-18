//! One mounted knight's body as the save and the picture hold it: momentum
//! along a curve, the tuck roll, the charge dash, the spur, the leap and the
//! lance-lock. How it moves is in `cockpit/live/lightgallop-rules`; this is
//! the state it hands back each tick, and the bounds a saved one must keep.
//!
//! Numbers are fixed point: positions in 1/256 pixel, speeds in 1/256 pixel
//! a tick, accelerations in 1/256 pixel a tick squared, on the 30 Hz sim.
use super::math::{self, ONE};
use super::terrain::Terrain;
use serde::{Deserialize, Serialize};

pub const SPEED_MAX: i32 = 4096;
pub const GAUGE_MAX: i32 = 1000;
pub const CHARGE_MAX: i32 = 1280;
pub const COYOTE: u32 = 4;
pub const BUFFER: u32 = 5;
pub const LOCK_TICKS: u8 = 14;
pub const LUNGE_TICKS: u8 = 8;
pub const SCATTER_MAX: u32 = 16;
pub const LOOSE_LIFE: u16 = 120;
pub const HURT_TICKS: u32 = 90;
pub const DOWN_TICKS: u32 = 24;
pub const SPARK_WORDS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Numbers {
    pub slope: i32,
    pub loop_min: i32,
    pub spur_cap: i32,
    pub lock_reach: i32,
    pub dimming: i32,
}

impl Default for Numbers {
    fn default() -> Self {
        Numbers { slope: 100, loop_min: 512, spur_cap: 2048, lock_reach: 96, dimming: 1000 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ground {
    pub path: u8,
    pub s: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    pub target: u16,
    pub ticks: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loose {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub age: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rider {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub ground: Option<Ground>,
    pub gsp: i32,
    pub dir: i32,
    pub roll: bool,
    pub ball: bool,
    pub crouch: bool,
    pub skid: bool,
    pub charge: i32,
    pub spur: bool,
    pub gauge: i32,
    pub lock: Option<Lock>,
    pub lock_cd: u8,
    pub spring_cd: u8,
    pub freeze: u8,
    pub shake: u8,
    pub lunge: u8,
    pub lunged: bool,
    pub chain: u32,
    pub best_chain: u32,
    pub sparks: u32,
    pub shield: bool,
    pub hurt: u32,
    pub ctrl: u32,
    pub down: u32,
    pub coyote: u32,
    pub buffer: u32,
    pub jump_key: bool,
    pub spur_key: bool,
    pub jumping: bool,
    pub air: u32,
    pub draft: bool,
    pub ang: i32,
    pub cam_x: i32,
    pub cam_y: i32,
    pub zoom: i32,
    pub looping: bool,
    pub want_lock: bool,
    pub loose: Vec<Loose>,
    pub taken: [u64; SPARK_WORDS],
    pub crates: u32,
    pub collected: u32,
    pub falls: u32,
    pub hits: u32,
    pub kills: u32,
    pub finish: Option<u32>,
    pub score: u32,
}

impl Rider {
    pub fn grounded(&self) -> bool {
        self.ground.is_some()
    }

    pub fn speed(&self) -> i32 {
        if self.grounded() {
            self.gsp.abs()
        } else {
            math::isqrt((i64::from(self.vx).pow(2) + i64::from(self.vy).pow(2)) as u64) as i32
        }
    }

    pub fn body(&self) -> (i32, i32) {
        if self.roll || self.ball || self.crouch || self.lock.is_some() {
            (7, 14)
        } else {
            (7, 28)
        }
    }

    pub fn centre(&self, t: &Terrain) -> (i32, i32) {
        let h = self.body().1 * 128;
        match self.ground {
            Some(g) => {
                let s = t.paths[g.path as usize].sample(g.s);
                (self.x + (i64::from(s.ty) * i64::from(h) / i64::from(ONE)) as i32,
                 self.y - (i64::from(s.tx) * i64::from(h) / i64::from(ONE)) as i32)
            }
            None => (self.x, self.y - h),
        }
    }

    pub fn valid(&self, t: &Terrain) -> bool {
        let bound = |v: i32, m: i32| (-m..=m).contains(&v);
        self.ground.is_none_or(|g| {
            (g.path as usize) < t.paths.len() && (0..=t.paths[g.path as usize].len()).contains(&g.s)
        }) && bound(self.vx, SPEED_MAX)
            && bound(self.vy, SPEED_MAX)
            && bound(self.gsp, SPEED_MAX)
            && [-1, 1].contains(&self.dir)
            && (0..=20000 * 256).contains(&self.x)
            && (-200 * 256..=1400 * 256).contains(&self.y)
            && (0..=GAUGE_MAX).contains(&self.gauge)
            && (0..=CHARGE_MAX).contains(&self.charge)
            && self.chain <= 1000
            && self.best_chain <= 1000
            && self.sparks <= 9999
            && self.hurt <= HURT_TICKS * 2
            && self.ctrl <= 60
            && self.down <= DOWN_TICKS
            && self.coyote <= COYOTE
            && self.buffer <= BUFFER
            && (0..1024).contains(&self.ang)
            && (128..=256).contains(&self.zoom)
            && self.loose.len() <= SCATTER_MAX as usize
            && self.loose.iter().all(|l| l.age <= LOOSE_LIFE)
            && self.lock.is_none_or(|l| l.ticks <= LOCK_TICKS)
            && self.lunge <= LUNGE_TICKS
            && self.freeze <= 8
            && self.shake <= 16
    }
}
