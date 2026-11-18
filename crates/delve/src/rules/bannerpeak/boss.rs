//! King Cinder on the summit. He cannot be hit head on. He stomps, sweeps and
//! rains embers by a pattern that tells before it strikes; after a few of
//! them he kneels, and the crown that sits on his head comes within reach.
//! A ground pound onto the crown (any knight's) knocks a plate off it. Three
//! crowns, each heavier than the last, each a harder pattern; a plate for every
//! seat at the table, never fewer.
//!
//! The head is a platform: a knight can stand on it, and a slam that lands on
//! it while he kneels is the blow. Everything is integer pixels and ticks.
use super::layout::{SUMMIT, info};
use super::world::TILE;
use crate::rules::rafterwake::Plank;
use serde::{Deserialize, Serialize};

pub const WIDTH: i32 = 48;
pub const HEIGHT: i32 = 64;
pub const HEAD_UP: i32 = 64;
pub const HEAD_DOWN: i32 = 44;
pub const PHASES: u8 = 3;
pub const BOSS_ID: u16 = 300;
pub const REACH: i32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attack {
    Stomp,
    Sweep,
    Embers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Beat {
    Dormant,
    Rest,
    Tell,
    Strike,
    Wince,
    Rage,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hazard {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub vx: i32,
    pub tell: u32,
    pub live: u32,
}

impl Hazard {
    pub fn rect(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.x + self.w, self.y + self.h)
    }
    pub fn hurts(&self) -> bool {
        self.tell == 0 && self.live > 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boss {
    pub x: i32,
    pub beat: Beat,
    pub kind: Attack,
    pub t: u32,
    pub phase: u8,
    pub plates: u8,
    pub count: u8,
    pub head: i32,
    pub head_before: i32,
    pub x_before: i32,
    pub flinch: u32,
    pub hazards: Vec<Hazard>,
}

pub fn floor() -> i32 {
    (SUMMIT.oy + 12) * TILE
}

pub fn home() -> i32 {
    info().cinder.0 * TILE + TILE / 2
}

impl Boss {
    pub fn new(seats: u8) -> Boss {
        Boss {
            x: home(),
            beat: Beat::Dormant,
            kind: Attack::Stomp,
            t: 0,
            phase: 0,
            plates: seats.max(1),
            count: 0,
            head: HEAD_UP,
            head_before: HEAD_UP,
            x_before: home(),
            flinch: 0,
            hazards: Vec::new(),
        }
    }
    pub fn valid(&self, seats_max: u8) -> bool {
        (home() - 200..=home() + 200).contains(&self.x)
            && (home() - 200..=home() + 200).contains(&self.x_before)
            && self.t <= 400
            && self.phase <= PHASES
            && self.plates <= seats_max
            && self.count <= 8
            && (HEAD_DOWN - 1..=HEAD_UP + 1).contains(&self.head)
            && (HEAD_DOWN - 1..=HEAD_UP + 1).contains(&self.head_before)
            && self.flinch <= 20
            && self.hazards.len() <= 16
            && self.hazards.iter().all(|h| {
                h.w <= 400 && h.h <= 400 && h.tell <= 60 && h.live <= 120 && h.vx.abs() <= 8
            })
            && ((self.phase == PHASES) == (self.beat == Beat::Down))
    }
    pub fn defeated(&self) -> bool {
        self.beat == Beat::Down
    }
    pub fn body(&self) -> (i32, i32, i32, i32) {
        let f = floor();
        (self.x - WIDTH / 2 + 4, f - self.head + 4, self.x + WIDTH / 2 - 4, f)
    }
    pub fn plank(&self, before: bool) -> Plank {
        let (x, head) = if before {
            (self.x_before, self.head_before)
        } else {
            (self.x, self.head)
        };
        Plank {
            x: x - WIDTH / 2 + 2,
            y: floor() - head,
            w: WIDTH - 4,
            lift: false,
        }
    }
    pub fn crown(&self) -> Option<(i32, i32, i32, i32)> {
        (self.beat == Beat::Wince).then(|| {
            let top = floor() - self.head;
            (self.x - 18, top - 10, self.x + 18, top)
        })
    }
    pub fn seated(&mut self, was: u8, now: u8) {
        if now > was && self.phase < PHASES {
            self.plates += now - was;
        }
    }
}

#[derive(Serialize)]
struct Ask<'a> {
    op: &'static str,
    elapsed: u32,
    seats: u8,
    feet: &'a [(i32, i32)],
    boss: &'a Boss,
}

#[derive(Serialize)]
struct AskSlam<'a> {
    op: &'static str,
    x: i32,
    y: i32,
    boss: &'a Boss,
}

#[derive(Deserialize)]
struct Next {
    boss: Boss,
}

#[derive(Deserialize)]
struct Slammed {
    boss: Boss,
    hit: bool,
}

impl Boss {
    fn sound(&self) -> bool {
        self.phase <= PHASES
            && self.plates <= crate::rules::MAX_PLAYERS as u8
            && self.count <= 9
            && self.hazards.len() <= 64
    }

    pub fn step(&mut self, at: &[(i32, i32)], seats: u8, elapsed: u32) {
        #[cfg(test)]
        super::install_oracle();
        let ask = Ask {
            op: "boss",
            elapsed,
            seats,
            feet: at,
            boss: self,
        };
        match crate::scripted::step::<_, Next>("bannerpeak-rules", &ask) {
            Ok(next) if next.boss.sound() => *self = next.boss,
            other => {
                debug_assert!(other.is_ok_and(|next| next.boss.sound()), "bannerpeak-rules refused a boss tick");
            }
        }
    }

    pub fn slam(&mut self, x: i32, y: i32) -> bool {
        #[cfg(test)]
        super::install_oracle();
        let ask = AskSlam {
            op: "slam",
            x,
            y,
            boss: self,
        };
        match crate::scripted::step::<_, Slammed>("bannerpeak-rules", &ask) {
            Ok(next) if next.boss.sound() => {
                *self = next.boss;
                next.hit
            }
            other => {
                debug_assert!(other.is_ok_and(|next| next.boss.sound()), "bannerpeak-rules refused a slam");
                false
            }
        }
    }
}
