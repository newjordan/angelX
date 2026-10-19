//! The rail Griffinwing flies: three beats laid end to end, and the pose of
//! the canyon at any point along them. The sim never reads the bend (it flies
//! in rail space); the stage lays its canyon and points its camera by it, so
//! the numbers here are floats and the rest of the game's are not.
use super::trig::Det;
use super::types::{PACE, Q};
use std::sync::OnceLock;

/// The beats, in order.
pub const NAMES: [&str; 3] = ["THUNDERHEAD RUN", "THE RUINED CAUSEWAY", "THE WYRM ROOST"];
/// Two or three plain words under each name.
pub const WORDS: [&str; 3] = ["AIM, HOLD, RELEASE", "ROLL TO REFLECT", "BREAK THREE LAMPS"];
/// How long the first two beats fly, in ticks: they run to the clock.
pub const LEN: [u32; 2] = [1800, 2400];
/// Where the Wyrm rises in the third beat: its checkpoint.
pub const BOSS_AT: u32 = 480;
/// A beat's first point on the rail, as fixed point.
pub const ORIGIN: [i32; 3] = [0, 1800 * PACE, (1800 + 2400) * PACE];

/// One stretch of the canyon, as authored: its length in units, the turn it
/// makes in degrees (positive to the right), the height it climbs, half its
/// width, and the look it wears.
#[derive(Clone, Copy)]
struct Piece {
    len: u16,
    turn: f32,
    climb: f32,
    half: f32,
    look: u8,
}

const fn piece(len: u16, turn: f32, climb: f32, half: f32, look: u8) -> Piece {
    Piece {
        len,
        turn,
        climb,
        half,
        look,
    }
}

/// The looks: open storm gorge, ruined causeway, the roost's approach, the
/// roost itself, the way out.
pub const GORGE: u8 = 0;
pub const CAUSEWAY: u8 = 1;
pub const APPROACH: u8 = 2;
pub const ROOST: u8 = 3;
pub const EXIT: u8 = 4;

const PIECES: [Piece; 17] = [
    // Thunderhead Run
    piece(60, 0.0, 0.0, 14.0, GORGE),
    piece(90, 24.0, 0.0, 14.0, GORGE),
    piece(90, -34.0, -3.0, 15.0, GORGE),
    piece(80, 0.0, 0.0, 13.0, GORGE),
    piece(110, 28.0, 2.0, 13.0, GORGE),
    piece(90, -20.0, 0.0, 14.0, GORGE),
    piece(78, 10.0, 0.0, 14.0, GORGE),
    // The Ruined Causeway
    piece(80, 0.0, 0.0, 11.0, CAUSEWAY),
    piece(110, -34.0, 0.0, 10.0, CAUSEWAY),
    piece(110, 38.0, 0.0, 10.0, CAUSEWAY),
    piece(90, 0.0, 2.0, 11.0, CAUSEWAY),
    piece(140, -30.0, -2.0, 10.0, CAUSEWAY),
    piece(100, 26.0, 0.0, 11.0, CAUSEWAY),
    piece(166, 0.0, 0.0, 10.0, CAUSEWAY),
    // The Wyrm's Roost
    piece(160, 18.0, 0.0, 12.0, APPROACH),
    piece(700, -10.0, 0.0, 21.0, ROOST),
    piece(1100, 14.0, 0.0, 23.0, EXIT),
];

/// One point of the canyon's line.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub p: [f32; 3],
    pub yaw: f32,
    pub half: f32,
    pub look: u8,
}

/// The canyon's line, a sample a unit.
pub struct Rail {
    samples: Vec<Pose>,
}

impl Rail {
    pub fn len(&self) -> f32 {
        (self.samples.len() - 1) as f32
    }
    pub fn is_empty(&self) -> bool {
        self.samples.len() < 2
    }
    /// The pose at `s` units along, between its samples; past either end the
    /// line carries on along the end's heading.
    pub fn at(&self, s: f32) -> Pose {
        let end = self.len();
        let (c, beyond) = if s < 0.0 {
            (0.0, s)
        } else if s > end {
            (end, s - end)
        } else {
            (s, 0.0)
        };
        let i = (c.floor() as usize).min(self.samples.len() - 2);
        let f = c - i as f32;
        let (a, b) = (&self.samples[i], &self.samples[i + 1]);
        let mut out = *a;
        for k in 0..3 {
            out.p[k] = a.p[k] + (b.p[k] - a.p[k]) * f;
        }
        out.yaw = a.yaw + (b.yaw - a.yaw) * f;
        out.half = a.half + (b.half - a.half) * f;
        out.p[0] += out.yaw.dcos() * beyond;
        out.p[1] += out.yaw.dsin() * beyond;
        out
    }
    /// A point in rail space in the world: `x` to the right of the line and
    /// `z` up from it, `s` in units.
    pub fn world(&self, s: f32, x: f32, z: f32) -> [f32; 3] {
        let at = self.at(s);
        let (sin, cos) = at.yaw.dsin_cos();
        [at.p[0] - sin * x, at.p[1] + cos * x, at.p[2] + z]
    }
}

/// The rail, laid once.
pub fn rail() -> &'static Rail {
    static RAIL: OnceLock<Rail> = OnceLock::new();
    RAIL.get_or_init(|| {
        let mut samples = Vec::new();
        let (mut p, mut yaw) = ([0.0f32; 3], 0.0f32);
        for pc in PIECES {
            let len = f32::from(pc.len);
            let bend = pc.turn.to_radians() / len;
            for _ in 0..pc.len {
                samples.push(Pose {
                    p,
                    yaw,
                    half: pc.half,
                    look: pc.look,
                });
                let mid = yaw + bend * 0.5;
                p[0] += mid.dcos();
                p[1] += mid.dsin();
                p[2] += pc.climb / len;
                yaw += bend;
            }
        }
        let last = *samples.last().expect("pieces");
        samples.push(Pose { p, yaw, ..last });
        Rail { samples }
    })
}

/// A fixed-point `s` in units.
pub fn units(s: i32) -> f32 {
    s as f32 / Q as f32
}

/// The beat a point on the rail falls in.
pub fn beat_of(s: i32) -> usize {
    ORIGIN.iter().rposition(|&o| s >= o).unwrap_or(0)
}

/// The third beat's canyon is long enough that its fight never runs out of
/// it; the rail's end carries on along its heading regardless.
pub const FIGHT_LEN: u32 = 3 * 60 * 30;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rail_is_laid_long_enough_for_every_beat_and_a_fight() {
        let r = rail();
        let need = units(ORIGIN[2] + (BOSS_AT + FIGHT_LEN) as i32 * PACE);
        assert!(r.len() >= need, "{} < {need}", r.len());
        assert!(!r.is_empty());
        for (b, o) in ORIGIN.iter().enumerate() {
            assert_eq!(beat_of(*o), b);
        }
    }

    #[test]
    fn a_point_beyond_the_end_carries_on_along_its_heading() {
        let r = rail();
        let end = r.len();
        let (a, b) = (r.at(end), r.at(end + 10.0));
        let d = ((b.p[0] - a.p[0]).powi(2) + (b.p[1] - a.p[1]).powi(2)).sqrt();
        assert!((d - 10.0).abs() < 1e-3);
    }
}
