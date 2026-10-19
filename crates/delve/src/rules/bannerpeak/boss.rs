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
const REACH: i32 = 100;

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

/// The attacks of a crown, in order, before he kneels.
fn pattern(phase: u8) -> &'static [Attack] {
    match phase {
        0 => &[Attack::Stomp, Attack::Embers],
        1 => &[Attack::Sweep, Attack::Stomp, Attack::Embers],
        _ => &[Attack::Embers, Attack::Sweep, Attack::Stomp, Attack::Embers],
    }
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
    /// A slam landed with its feet at `(x, y)` pixels: whether a plate fell.
    pub fn slam(&mut self, x: i32, y: i32) -> bool {
        let Some(c) = self.crown() else {
            return false;
        };
        if self.plates > 0 && (x - self.x).abs() <= 24 && (y - c.3).abs() <= 3 {
            self.plates -= 1;
            self.flinch = 14;
            if self.plates == 0 {
                self.fall();
            }
            return true;
        }
        false
    }
    /// A crown falls: the next, or the last of them.
    fn fall(&mut self) {
        self.phase += 1;
        self.hazards.clear();
        if self.phase >= PHASES {
            self.beat = Beat::Down;
            self.t = 0;
        } else {
            self.beat = Beat::Rage;
            self.t = 54;
        }
    }
    /// A seat more at the table: its plate comes with it, and the share of the
    /// crown already knocked off stays knocked off.
    pub fn seated(&mut self, was: u8, now: u8) {
        if now > was && self.phase < PHASES {
            self.plates += now - was;
        }
    }
    /// One tick. `at` are the feet of the knights standing; `seats` how many sit.
    pub fn step(&mut self, at: &[(i32, i32)], seats: u8, elapsed: u32) {
        self.head_before = self.head;
        self.x_before = self.x;
        self.flinch = self.flinch.saturating_sub(1);
        for h in &mut self.hazards {
            if h.tell > 0 {
                h.tell -= 1;
            } else {
                h.x += h.vx;
                h.live = h.live.saturating_sub(1);
            }
        }
        self.hazards.retain(|h| h.live > 0 || h.tell > 0);
        let arena = |x: i32, y: i32| {
            x > (SUMMIT.ox + 3) * TILE && x < (SUMMIT.ox + 38) * TILE && y > (SUMMIT.oy) * TILE
        };
        let near = at.iter().copied().filter(|&(x, y)| arena(x, y)).collect::<Vec<_>>();
        let crowd = i32::from(seats.max(1)) - 1;
        match self.beat {
            Beat::Dormant => {
                if !near.is_empty() {
                    self.beat = Beat::Rest;
                    self.t = 60;
                }
            }
            Beat::Down => {
                self.head = (self.head - 1).max(HEAD_DOWN - 1);
            }
            Beat::Rage => {
                self.t = self.t.saturating_sub(1);
                self.head = (self.head + 1).min(HEAD_UP);
                if self.t == 0 {
                    self.plates = seats.max(1);
                    self.count = 0;
                    self.beat = Beat::Rest;
                    self.t = 40;
                }
            }
            Beat::Rest => {
                self.t = self.t.saturating_sub(1);
                self.head = (self.head + 1).min(HEAD_UP);
                // Step toward the nearest knight, slowly.
                if let Some(&(tx, _)) = near.iter().min_by_key(|(x, _)| (x - self.x).abs()) {
                    if elapsed % 2 == 0 {
                        self.x += (tx - self.x).clamp(-1, 1);
                    }
                    self.x = self.x.clamp(home() - 110, home() + 110);
                }
                if self.t == 0 {
                    let seq = pattern(self.phase.min(2));
                    if usize::from(self.count) >= seq.len() {
                        self.beat = Beat::Wince;
                        self.t = (150 - 20 * i32::from(self.phase) - 3 * crowd).max(70) as u32;
                    } else {
                        self.kind = seq[usize::from(self.count)];
                        self.beat = Beat::Tell;
                        self.t = match self.kind {
                            Attack::Stomp => 40,
                            Attack::Sweep => 32,
                            Attack::Embers => 30,
                        } - 3 * u32::from(self.phase);
                        self.warn(&near, seats, elapsed);
                    }
                }
            }
            Beat::Tell => {
                self.t = self.t.saturating_sub(1);
                if self.t == 0 {
                    self.strike();
                    self.beat = Beat::Strike;
                    self.t = match self.kind {
                        Attack::Stomp => 22,
                        Attack::Sweep => 14,
                        Attack::Embers => 12,
                    };
                }
            }
            Beat::Strike => {
                self.t = self.t.saturating_sub(1);
                if self.t == 0 {
                    self.count += 1;
                    self.beat = Beat::Rest;
                    self.t = (44 - 6 * u32::from(self.phase) - 3 * crowd as u32).max(16);
                }
            }
            Beat::Wince => {
                self.t = self.t.saturating_sub(1);
                self.head = (self.head - 2).max(HEAD_DOWN);
                if self.t == 0 {
                    self.count = 0;
                    self.beat = Beat::Rest;
                    self.t = 30;
                }
            }
        }
    }
    /// The warning an attack gives: where it will be.
    fn warn(&mut self, near: &[(i32, i32)], seats: u8, elapsed: u32) {
        let f = floor();
        let tell = self.t;
        match self.kind {
            Attack::Stomp => {}
            Attack::Sweep => {
                let toward = near
                    .iter()
                    .min_by_key(|(x, _)| (x - self.x).abs())
                    .map_or(1, |&(x, _)| if x >= self.x { 1 } else { -1 });
                let (l, w) = if toward > 0 {
                    (self.x + 6, REACH)
                } else {
                    (self.x - 6 - REACH, REACH)
                };
                self.hazards.push(Hazard {
                    x: l,
                    y: f - 22,
                    w,
                    h: 22,
                    vx: 0,
                    tell,
                    live: 14,
                });
            }
            Attack::Embers => {
                // A column on a knight, more of them for more seats and later crowns.
                let columns = 1 + i32::from(self.phase) + i32::from(seats.saturating_sub(1)) / 2;
                let mut aims: Vec<i32> = near.iter().map(|&(x, _)| x).collect();
                aims.sort_unstable();
                let top = (SUMMIT.oy + 1) * TILE;
                for c in 0..columns {
                    let pick = if aims.is_empty() {
                        home()
                    } else {
                        aims[((elapsed as i32 + c * 3) as usize) % aims.len()]
                    };
                    let x = (pick + (c % 3 - 1) * 26).clamp(
                        (SUMMIT.ox + 4) * TILE,
                        (SUMMIT.ox + 36) * TILE,
                    );
                    self.hazards.push(Hazard {
                        x: x - 10,
                        y: top,
                        w: 20,
                        h: f - top,
                        vx: 0,
                        tell,
                        live: 12,
                    });
                }
            }
        }
    }
    /// The blow lands.
    fn strike(&mut self) {
        if self.kind == Attack::Stomp {
            let f = floor();
            let n = 1 + i32::from(self.phase >= 1);
            for k in 0..n {
                for dir in [-1, 1] {
                    // The second wave follows the first.
                    self.hazards.push(Hazard {
                        x: self.x - 8 + dir * k * 40,
                        y: f - 14,
                        w: 16,
                        h: 14,
                        vx: dir * 3,
                        tell: (k * 14) as u32,
                        live: 70,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crown_needs_a_plate_for_every_seat_and_only_a_kneeling_head_takes_a_blow() {
        let mut b = Boss::new(3);
        assert_eq!(b.plates, 3);
        let head_y = floor() - HEAD_UP;
        assert!(!b.slam(b.x, head_y), "not while he stands");
        b.beat = Beat::Wince;
        b.head = HEAD_DOWN;
        let y = floor() - HEAD_DOWN;
        assert!(!b.slam(b.x + 40, y), "off to the side");
        assert!(!b.slam(b.x, y - 30), "not high above");
        assert!(b.slam(b.x, y) && b.slam(b.x + 10, y));
        assert_eq!(b.plates, 1);
        assert!(b.slam(b.x, y));
        assert_eq!((b.phase, b.beat), (1, Beat::Rage), "the second crown");
        assert!(b.valid(6));
    }

    #[test]
    fn three_crowns_fall_in_turn_and_a_late_seat_brings_a_plate() {
        let mut b = Boss::new(1);
        for crown in 0..3u8 {
            assert_eq!(b.phase, crown);
            b.beat = Beat::Wince;
            b.head = HEAD_DOWN;
            if crown == 1 {
                // Rage mended the crown for two seats; a third sits down.
                assert_eq!(b.plates, 2);
                b.seated(2, 3);
                assert_eq!(b.plates, 3, "a seat that sits down adds a plate");
            }
            let mut blows = 0;
            while b.phase == crown {
                assert!(b.slam(b.x, floor() - HEAD_DOWN));
                blows += 1;
            }
            assert_eq!(blows, [1, 3, 2][usize::from(crown)]);
            if b.beat == Beat::Rage {
                b.t = 1;
                let seats = if crown == 0 { 2 } else { 2 };
                b.step(&[], seats, 0);
            }
        }
        assert!(b.defeated() && b.phase == PHASES);
        assert!(b.valid(6));
    }

    #[test]
    fn the_pattern_tells_before_it_strikes_and_ends_kneeling() {
        let mut b = Boss::new(1);
        let at = [(home() - 60, floor())];
        let mut told = Vec::new();
        let mut struck = false;
        let mut knelt = false;
        for t in 0..900 {
            let before = b.hazards.iter().filter(|h| h.hurts()).count();
            b.step(&at, 1, t);
            if b.beat == Beat::Tell && b.t > 8 {
                told.push(b.kind);
            }
            struck |= b.hazards.iter().filter(|h| h.hurts()).count() > before;
            knelt |= b.beat == Beat::Wince;
            if knelt {
                break;
            }
        }
        assert!(struck && knelt);
        assert!(told.contains(&Attack::Stomp) && told.contains(&Attack::Embers));
        assert!(b.valid(6));
    }

    #[test]
    fn more_seats_mean_more_embers_and_less_time_never_less_boss() {
        let embers = |seats: u8| {
            let mut b = Boss::new(seats);
            b.beat = Beat::Rest;
            b.t = 0;
            b.count = 1;
            b.step(&[(home(), floor()), (home() + 50, floor())], seats, 3);
            b.hazards.len()
        };
        assert!(embers(6) > embers(1), "{} against {}", embers(6), embers(1));
        let wince = |seats: u8| {
            let mut b = Boss::new(seats);
            b.beat = Beat::Rest;
            b.t = 0;
            b.count = 9;
            b.step(&[(home(), floor())], seats, 1);
            b.t
        };
        assert!(wince(6) < wince(1));
        assert_eq!(Boss::new(6).plates, 6);
    }
}
