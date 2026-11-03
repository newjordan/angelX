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
/// His head, from the floor, standing and kneeling.
pub const HEAD_UP: i32 = 64;
pub const HEAD_DOWN: i32 = 44;
/// A plate per phase, per seat.
pub const PHASES: u8 = 3;
pub const BOSS_ID: u16 = 300;
pub const REACH: i32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attack {
    /// He drops his weight: a wave runs out along the floor each way.
    Stomp,
    /// An arm sweeps the floor in front of him.
    Sweep,
    /// Columns of embers fall where the knights stand.
    Embers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Beat {
    /// Asleep until a knight walks in.
    Dormant,
    Rest,
    Tell,
    Strike,
    /// Kneeling: the crown is low.
    Wince,
    /// Between crowns.
    Rage,
    /// The last crown is down.
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hazard {
    /// Left, top, width, height in pixels.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// A wave's pixels a tick (0 for the others).
    pub vx: i32,
    /// Ticks of warning before it hurts.
    pub tell: u32,
    /// Ticks it hurts for.
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
    /// Centre, pixels.
    pub x: i32,
    pub beat: Beat,
    pub kind: Attack,
    pub t: u32,
    /// The crown: 0 to 2, then 3 for all three down.
    pub phase: u8,
    /// Plates of this crown still on.
    pub plates: u8,
    /// Attacks done since he last knelt.
    pub count: u8,
    /// How high the head is, pixels above the floor.
    pub head: i32,
    pub head_before: i32,
    pub x_before: i32,
    pub flinch: u32,
    pub hazards: Vec<Hazard>,
}

/// The floor of the arena, pixel row.
pub fn floor() -> i32 {
    (SUMMIT.oy + 12) * TILE
}

/// Where he stands to begin with, pixels.
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
    /// The body: what a touch from the side hurts.
    pub fn body(&self) -> (i32, i32, i32, i32) {
        let f = floor();
        (self.x - WIDTH / 2 + 4, f - self.head + 4, self.x + WIDTH / 2 - 4, f)
    }
    /// The head, as a platform to stand on.
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
    /// The crown's box while he kneels.
    pub fn crown(&self) -> Option<(i32, i32, i32, i32)> {
        (self.beat == Beat::Wince).then(|| {
            let top = floor() - self.head;
            (self.x - 18, top - 10, self.x + 18, top)
        })
    }
    /// A seat more at the table: its plate comes with it, and the share of the
    /// crown already knocked off stays knocked off.
    pub fn seated(&mut self, was: u8, now: u8) {
        if now > was && self.phase < PHASES {
            self.plates += now - was;
        }
    }
}

/// What the leg is handed: the tick number, how many sit, the feet of the
/// knights standing, and King Cinder as the save and the wire know him.
#[derive(Serialize)]
struct Ask<'a> {
    op: &'static str,
    elapsed: u32,
    seats: u8,
    feet: &'a [(i32, i32)],
    boss: &'a Boss,
}

/// A slam, with the feet it landed at.
#[derive(Serialize)]
struct AskSlam<'a> {
    op: &'static str,
    x: i32,
    y: i32,
    boss: &'a Boss,
}

/// The tick's answer: the fight as it stands now.
#[derive(Deserialize)]
struct Next {
    boss: Boss,
}

/// The slam's answer: the fight, and whether a plate came off the crown.
#[derive(Deserialize)]
struct Slammed {
    boss: Boss,
    hit: bool,
}

impl Boss {
    /// Whether an answer is a fight this session can hold: the crowns, the
    /// plates and the hazards a tick may carry.
    fn sound(&self) -> bool {
        self.phase <= PHASES
            && self.plates <= crate::rules::MAX_PLAYERS as u8
            && self.count <= 9
            && self.hazards.len() <= 64
    }

    /// One tick. `at` are the feet of the knights standing; `seats` how many
    /// sit. The beats, the warnings, the blows and the hazards are the leg's;
    /// this is the adapter that asks it and applies the answer.
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

    /// A slam landed with its feet at `(x, y)` pixels: whether a plate fell.
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
