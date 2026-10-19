//! Where each section of the map sits in the one world, and what the map does
//! not say itself: the moving logs, the waterwheel, the hatches, and the marks
//! read into lists (`Info`). Everything here is fixed data; nothing is saved.
//!
//! Mark glyphs (the tile glyphs are in `world.rs`):
//!
//! ```text
//! S start     s snuffer    c capped snuffer   w wisp      e extra snuffer (3+ seats)
//! f extra capped (5+ seats)   $ coin    * ring spark   K checkpoint gate   G flagpole
//! 0-7 banner  E arch zone   Y course board   C King Cinder   P Great Pole   F finale spot
//! j bell chute   r u v x y q o m signs
//! & | / " < >  hints for the scripted bots (never drawn)
//! ```
use super::body::{Mover, Q};
use super::world::{Mark, TILE, World};
use crate::rules::rafterwake::Plank;
use std::sync::OnceLock;

/// A section's place in the world: its top-left cell and its size in cells.
#[derive(Clone, Copy, Debug)]
pub struct Sec {
    pub ox: i32,
    pub oy: i32,
    pub w: i32,
    pub h: i32,
}

impl Sec {
    /// The world cell of the section's cell `(c, r)`.
    pub const fn at(&self, c: i32, r: i32) -> (i32, i32) {
        (self.ox + c, self.oy + r)
    }
    pub const fn contains_x(&self, x: i32) -> bool {
        x >= self.ox && x < self.ox + self.w
    }
}

pub const HUB: Sec = Sec { ox: 0, oy: 32, w: 80, h: 30 };
pub const MARSH: Sec = Sec { ox: 80, oy: 36, w: 105, h: 28 };
pub const MILL: Sec = Sec { ox: 185, oy: 36, w: 88, h: 18 };
pub const SHAFT: Sec = Sec { ox: 273, oy: 16, w: 14, h: 38 };
pub const SNOW: Sec = Sec { ox: 287, oy: 12, w: 105, h: 14 };
pub const SUMMIT: Sec = Sec { ox: 392, oy: 10, w: 42, h: 14 };
pub const WIDTH: i32 = SUMMIT.ox + SUMMIT.w;
/// The snow ledge's floor row: the bell boulders roll along its top.
pub const SNOW_FLOOR: i32 = SNOW.oy + 10;
pub const HEIGHT: i32 = 64;
/// The sections in order along the way.
pub const SECTIONS: [Sec; 6] = [HUB, MARSH, MILL, SHAFT, SNOW, SUMMIT];

/// The whole map, composed once.
pub fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        let rows = |text: &'static str| -> Vec<&'static str> { text.lines().collect() };
        let maps = [
            rows(include_str!("../../../assets/bannerpeak/hub.map")),
            rows(include_str!("../../../assets/bannerpeak/marsh.map")),
            rows(include_str!("../../../assets/bannerpeak/mill.map")),
            rows(include_str!("../../../assets/bannerpeak/shaft.map")),
            rows(include_str!("../../../assets/bannerpeak/snow.map")),
            rows(include_str!("../../../assets/bannerpeak/summit.map")),
        ];
        let sections: Vec<(&[&str], i32, i32)> = maps
            .iter()
            .zip(SECTIONS.iter())
            .map(|(m, s)| (m.as_slice(), s.ox, s.oy))
            .collect();
        World::compose(WIDTH, HEIGHT, &sections)
    })
}

/// The section a world column is in.
pub fn section_of(x: i32) -> usize {
    SECTIONS
        .iter()
        .rposition(|s| x >= s.ox)
        .unwrap_or(0)
}

/// Words on a signboard, by its glyph.
pub fn sign_text(glyph: char) -> &'static str {
    match glyph {
        'r' => "RUN. JUMP.",
        'u' => "HOP HOP HOP",
        'v' => "SLAM THE FLOOR",
        'm' => "SHIELD UP: E",
        'x' => "DUCK + JUMP: FAR",
        'y' => "WHEEL LIFTS",
        'q' => "KICK OFF WALLS",
        'o' => "MIND THE BELL",
        _ => "",
    }
}

/// What the marks say, as lists in the order the game counts them.
pub struct Info {
    pub start: (i32, i32),
    /// Checkpoint gates in the order they are met.
    pub gates: Vec<(i32, i32)>,
    pub sparks: Vec<(i32, i32)>,
    pub coins: Vec<(i32, i32)>,
    /// Banner pickups by banner number; `None` where a banner is won another way.
    pub banners: [Option<(i32, i32)>; 8],
    pub pole: (i32, i32),
    pub great_pole: (i32, i32),
    pub finale: (i32, i32),
    pub arch: (i32, i32),
    pub board: (i32, i32),
    pub cinder: (i32, i32),
    pub signs: Vec<(i32, i32, char)>,
    /// `s`, `c`, `w`, `e`, `f` in reading order.
    pub foes: Vec<Mark>,
    /// Hint cells for the scripted bots.
    pub hints: Vec<Mark>,
}

pub fn info() -> &'static Info {
    static INFO: OnceLock<Info> = OnceLock::new();
    INFO.get_or_init(|| {
        let w = world();
        let one = |k: char| {
            w.marks_of(k)
                .next()
                .map(|m| (m.x, m.y))
                .unwrap_or_else(|| panic!("the map has no {k}"))
        };
        let mut gates: Vec<(i32, i32)> = w.marks_of('K').map(|m| (m.x, m.y)).collect();
        // Met in order: along the map, but up the shaft.
        gates.sort_by_key(|&(x, y)| {
            let s = section_of(x);
            (s, if SECTIONS[s].ox == SHAFT.ox { -y } else { x })
        });
        let mut sparks: Vec<(i32, i32)> = w.marks_of('*').map(|m| (m.x, m.y)).collect();
        sparks.sort_by_key(|&(x, y)| {
            let s = section_of(x);
            (s, if SECTIONS[s].ox == SHAFT.ox { -y } else { x })
        });
        let mut banners = [None; 8];
        for m in &w.marks {
            if let Some(d) = m.k.to_digit(10).filter(|d| *d < 8) {
                banners[d as usize] = Some((m.x, m.y));
            }
        }
        let signs = w
            .marks
            .iter()
            .filter(|m| !sign_text(m.k).is_empty())
            .map(|m| (m.x, m.y, m.k))
            .collect();
        Info {
            start: one('S'),
            gates,
            sparks,
            coins: w.marks_of('$').map(|m| (m.x, m.y)).collect(),
            banners,
            pole: one('G'),
            great_pole: one('P'),
            finale: one('F'),
            arch: one('E'),
            board: one('Y'),
            cinder: one('C'),
            signs,
            foes: w
                .marks
                .iter()
                .filter(|m| matches!(m.k, 's' | 'c' | 'w' | 'e' | 'f'))
                .copied()
                .collect(),
            hints: w
                .marks
                .iter()
                .filter(|m| matches!(m.k, '&' | '|' | '/' | '"' | '<' | '>'))
                .copied()
                .collect(),
        }
    })
}

/// Pixels of a cell's centre, and the feet-line of a cell standing on its floor.
pub fn cell_centre((cx, cy): (i32, i32)) -> (i32, i32) {
    (cx * TILE + TILE / 2, cy * TILE + TILE / 2)
}
pub fn cell_feet((cx, cy): (i32, i32)) -> (i32, i32) {
    (cx * TILE + TILE / 2, (cy + 1) * TILE)
}

/// The cellar and the bog bonus room: a hatch tile and where crouching on it
/// puts a knight (the air cell above the floor it comes out on).
pub fn hatch_to(tile: (i32, i32)) -> Option<(i32, i32)> {
    let up = MARSH.at(44, 12);
    let down = MARSH.at(56, 26);
    if tile == up {
        Some(MARSH.at(50, 25))
    } else if tile == down {
        Some(MARSH.at(45, 11))
    } else {
        None
    }
}

// ---------------------------------------------------------------- movers

/// A sine in 1/1024, for whole degrees.
pub fn sine(deg: i32) -> i32 {
    const TABLE: [i32; 91] = [
        0, 18, 36, 54, 71, 89, 107, 125, 143, 160, 178, 195, 213, 230, 248, 265, 282, 299, 316,
        333, 350, 367, 384, 400, 416, 433, 449, 465, 481, 496, 512, 527, 543, 558, 573, 587, 602,
        616, 630, 644, 658, 672, 685, 698, 711, 724, 737, 749, 761, 773, 784, 796, 807, 818, 828,
        839, 849, 859, 868, 878, 887, 896, 904, 912, 920, 928, 935, 943, 949, 956, 962, 968, 974,
        979, 984, 989, 994, 998, 1002, 1005, 1008, 1011, 1014, 1016, 1018, 1020, 1022, 1023, 1023,
        1024, 1024,
    ];
    let d = deg.rem_euclid(360);
    match d {
        0..=90 => TABLE[d as usize],
        91..=180 => TABLE[(180 - d) as usize],
        181..=270 => -TABLE[(d - 180) as usize],
        _ => -TABLE[(360 - d) as usize],
    }
}
pub fn cosine(deg: i32) -> i32 {
    sine(deg + 90)
}

/// A value going back and forth between `lo` and `hi`, `period` ticks a round
/// trip, `phase` ticks along.
pub fn ping_pong(t: u32, period: u32, phase: u32, lo: i32, hi: i32) -> i32 {
    let half = period / 2;
    let k = (t + phase) % period;
    let up = if k < half { k } else { period - k } as i32;
    lo + (hi - lo) * up / half as i32
}

/// The waterwheel in the mill: centre in pixels, radius, buckets, ticks a turn.
pub const WHEEL: (i32, i32) = (
    (MILL.ox + 33) * TILE + TILE / 2,
    (MILL.oy + 11) * TILE + TILE / 2,
);
pub const WHEEL_RADIUS: i32 = 40;
pub const WHEEL_BUCKETS: i32 = 6;
pub const WHEEL_TURN: u32 = 270;
pub const WHEEL_ID: u16 = 200;
pub const LOG_ID: u16 = 100;

/// Every moving platform at tick `t`: the logs on the bogs and the wheel's
/// buckets. Pure in `t`, so a body can be carried by the difference between two
/// ticks.
pub fn movers(t: u32) -> Vec<Mover> {
    let mut out = Vec::with_capacity(12);
    // Two logs in the marsh's wide gap, the banks' height.
    let bank = (MARSH.oy + 12) * TILE;
    let gap = (MARSH.ox + 62) * TILE;
    out.push(log(LOG_ID, bank, ping_pong(t, 150, 0, gap + 4, gap + 52)));
    out.push(log(LOG_ID + 1, bank, ping_pong(t, 150, 75, gap + 68, gap + 116)));
    // One in the mill-race between the first piers.
    let water = (MILL.oy + 14) * TILE;
    let race = (MILL.ox + 19) * TILE;
    out.push(log(LOG_ID + 2, water, ping_pong(t, 120, 0, race, race + 32)));
    // The wheel: clockwise, so the near side climbs.
    let deg = (t % WHEEL_TURN) as i32 * 360 / WHEEL_TURN as i32;
    for i in 0..WHEEL_BUCKETS {
        let phi = deg + i * 360 / WHEEL_BUCKETS;
        let x = WHEEL.0 + WHEEL_RADIUS * cosine(phi) / 1024;
        let y = WHEEL.1 + WHEEL_RADIUS * sine(phi) / 1024;
        out.push(Mover {
            id: WHEEL_ID + i as u16,
            p: Plank {
                x: x - 14,
                y,
                w: 28,
                lift: false,
            },
        });
    }
    out
}

fn log(id: u16, top: i32, left: i32) -> Mover {
    Mover {
        id,
        p: Plank {
            x: left,
            y: top,
            w: 48,
            lift: false,
        },
    }
}

/// The fixed-point foot position of a world cell's floor.
pub fn feet_q(cell: (i32, i32)) -> (i32, i32) {
    let (x, y) = cell_feet(cell);
    (x * Q, y * Q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sections_tile_the_world_and_meet_at_one_floor() {
        for pair in SECTIONS.windows(2) {
            assert_eq!(pair[0].ox + pair[0].w, pair[1].ox, "no gap, no overlap");
        }
        let w = world();
        assert_eq!((w.w, w.h), (WIDTH, HEIGHT));
        // The floor is row 50 at the hub, the marsh, the mill and the shaft's foot.
        for x in [10, 85, 190, 275] {
            assert!(
                w.at(x, 50).solid() && !w.at(x, 49).solid(),
                "floor at column {x}"
            );
        }
        // The snow ledge and the arena floor are row 22.
        for x in [300, 380, 400, 420] {
            assert!(w.at(x, 22).solid() && !w.at(x, 21).solid(), "column {x}");
        }
    }

    #[test]
    fn the_marks_are_read_into_lists_the_game_counts() {
        let i = info();
        assert_eq!(i.gates.len(), 8, "eight gates: {:?}", i.gates);
        assert_eq!(i.sparks.len(), 8, "a ring of eight sparks");
        assert!(i.banners.iter().all(Option::is_some) || i.banners[2].is_none());
        for (n, b) in i.banners.iter().enumerate() {
            if n == 2 || n == 6 {
                assert!(b.is_none(), "banner {n} is won, not picked up");
            } else {
                assert!(b.is_some(), "banner {n} has a place");
            }
        }
        assert!(i.coins.len() >= 60, "{} coins", i.coins.len());
        // Gates run along the way; the shaft's two climb.
        let xs: Vec<i32> = i.gates.iter().map(|g| g.0).collect();
        assert!(xs.windows(2).all(|p| p[0] <= p[1] || SHAFT.contains_x(p[0])));
        assert!(i.gates[3].1 > i.gates[4].1, "the foot, then the alcove above");
    }

    #[test]
    fn the_wheel_climbs_on_the_near_side_and_the_logs_ride_their_rails() {
        let a = movers(0);
        let b = movers(30);
        let bucket = |m: &[Mover], i: u16| m.iter().find(|x| x.id == WHEEL_ID + i).unwrap().p;
        // Whichever bucket starts on the west, it rises as the wheel turns.
        let west = (0..WHEEL_BUCKETS as u16)
            .find(|&i| {
                let p = bucket(&a, i);
                p.x + 14 < WHEEL.0 - 30 && (p.y - WHEEL.1).abs() < 10
            })
            .expect("a bucket on the west at the start");
        assert!(bucket(&b, west).y < bucket(&a, west).y, "up on the west side");
        for t in [0u32, 40, 120, 149] {
            let m = movers(t);
            let l = m.iter().find(|x| x.id == LOG_ID).unwrap().p;
            let gap = (MARSH.ox + 62) * TILE;
            assert!(l.x >= gap && l.x + l.w <= gap + 100, "log stays over its gap");
        }
        assert_eq!(movers(5).len(), movers(500).len());
        assert_eq!(ping_pong(0, 100, 0, 10, 20), 10);
        assert_eq!(ping_pong(50, 100, 0, 10, 20), 20);
    }

    #[test]
    fn hatches_pair_up_and_open_onto_floor() {
        let w = world();
        for tile in [MARSH.at(44, 12), MARSH.at(56, 26)] {
            assert_eq!(w.at(tile.0, tile.1), super::super::world::Tile::Hatch);
            let to = hatch_to(tile).expect("a way through");
            assert!(w.at(to.0, to.1 + 1).solid(), "lands on floor");
            assert!(!w.at(to.0, to.1).solid());
        }
    }
}
