//! The 3D races: the Shaft Run, the Ore Cart and the Griffin Run. One hall
//! on each floor of the delve is a race hall; when the party walks in, every
//! knight in the room races at once, each behind their own camera.
//!
//! - **The Shaft Run** (a Marble Madness nod): each knight runs inside their
//!   own iron ball down a winding mineshaft of timbered ramps, a plank
//!   bridge over the drop, crossing ore carts, a lodestone rail, a sluice
//!   and a collapsing drift. Off an edge is down the shaft, and a rope
//!   hauls you back to the last post. Brannoc's ore-sorter at the bottom
//!   ranks the field.
//! - **The Ore Cart** (a mine-cart nod): carts on three lines through the
//!   galleries. Jump the broken rails and the fallen timbers, switch lines
//!   before a line ends at its buffer stop.
//! - **The Griffin Run** (an on-rails flier nod): griffins down the gorge
//!   under Dragon Keep. Rings give a burst, gargoyles can be shot for one,
//!   a rival shot is ruffled, and a spire or a gargoyle met head-on sends
//!   griffin and knight tumbling.
//!
//! The course is the same for everyone and fixed, so its mesh is built once.
//! Every knight antes a tenth of the gold they carry; the first home takes
//! the pot and the laurel, and everyone keeps what they picked up on the way.
//! Positions are along the course (`s`), across it (`x`) and above it (`z`),
//! one unit a tile.
use super::*;
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// The title card and its countdown, before anyone moves.
pub const TITLE: u32 = 3 * HZ;
/// The longest a race runs; whoever is not home by then is ranked by how
/// far down they got.
pub const RACE_TIME: u32 = 90 * HZ;
/// The finish board stays up this long before the hall comes back.
pub const PODIUM: u32 = 6 * HZ;
/// How long a crash keeps a knight out of it (the rope, the righting).
pub const CRASH: u32 = 45;
/// A ball's radius.
pub const BALL_R: f32 = 0.5;
/// Gold kept for each thing picked up on the way.
pub const HAUL_GOLD: u32 = 2;
/// The cart lines, across the gallery.
pub const LANES: [f32; 3] = [-1.7, 0.0, 1.7];
/// How far a griffin may stray from the gorge's line, across and up.
pub const SKY_X: f32 = 4.0;
pub const SKY_Z: f32 = 2.5;
/// A griffin's bolt: its pace past the shooter, its life, and the rest
/// between shots.
pub const BOLT_PACE: f32 = 32.0;
pub const BOLT_LIFE: u32 = 24;
pub const BOLT_REST: u32 = 10;
/// A bolt in the feathers slows a griffin this long.
pub const RUFFLE: u32 = HZ;
/// A burst from a ring or a smashed gargoyle.
pub const BURST: u32 = 45;
/// A barrel roll, and the rest before the next.
pub const ROLL: u32 = 18;
pub const ROLL_REST: u32 = 45;
/// Collapsing planks: how long after the first ball passes a rotten plank
/// it takes to go.
pub const ROT: u32 = HZ;
/// Seconds of one tick.
const DT: f32 = 1.0 / HZ as f32;
/// The mark a won race leaves (settled into the realm as `race_won`).
pub const RACE_MARK: &str = "country:race";
/// The Griffin Eyrie on the surface: the crags either side and the perch
/// (tiles), and where a knight stands to ask for a practice flight.
pub const EYRIE_ROCKS: [(i32, i32, i32, i32); 4] =
    [(1, 1, 7, 3), (16, 1, 7, 3), (10, 1, 4, 2), (2, 10, 4, 3)];
pub const PERCH: (f32, f32) = (12.0, 4.0);
/// How near the perch E asks for a flight (tiles).
pub const PERCH_REACH: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Course {
    Shaft,
    Cart,
    Griffin,
}

impl Course {
    pub fn name(self) -> &'static str {
        match self {
            Course::Shaft => "THE SHAFT RUN",
            Course::Cart => "THE ORE CART",
            Course::Griffin => "THE GRIFFIN RUN",
        }
    }
    /// The title card's words, and nothing else.
    pub fn words(self) -> &'static str {
        match self {
            Course::Shaft => "ROLL. DON'T FALL.",
            Course::Cart => "JUMP. SWITCH.",
            Course::Griffin => "FLY. SHOOT.",
        }
    }
    /// The two keys that matter, each with its one word.
    pub fn keys(self) -> [(&'static str, &'static str); 2] {
        match self {
            Course::Shaft => [("F", "HOP"), ("E", "BRAKE")],
            Course::Cart => [("F", "JUMP"), ("E", "SWITCH")],
            Course::Griffin => [("F", "SHOOT"), ("E", "ROLL")],
        }
    }
    /// Where the finish board says the field ended up.
    pub fn board(self) -> &'static str {
        match self {
            Course::Shaft => "BRANNOC'S ORE-SORTER",
            Course::Cart => "END OF THE LINE",
            Course::Griffin => "THE KEEP GATE",
        }
    }
    /// The race a floor's hall holds, in turn down the delve from the second
    /// floor: the Shaft Run, then the Griffin Run under Dragon Keep, then
    /// the Ore Cart.
    pub fn of_floor(depth: u32) -> Course {
        [Course::Griffin, Course::Cart, Course::Shaft][(depth % 3) as usize]
    }
}

/// What the course is made of underfoot at a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    /// Timbered floor between rails.
    Floor,
    /// A plank bridge over the drop: narrow, no rails.
    Bridge,
    /// Ore carts cross the way.
    Crossing,
    /// An iron rail that pulls the ball to it, and speeds it along it.
    Lodestone,
    /// A water channel that washes a ball side to side.
    Sluice,
    /// Planks over a drift; the rotten ones go once a ball has passed.
    Collapse,
    /// The ore-sorter at the bottom, and the finish.
    Sorter,
    /// Cart rails in the galleries.
    Rails,
    /// Rails on a trestle over a chasm.
    Trestle,
    /// The open gorge under the Keep.
    Gorge,
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    /// The course's middle line at this point, and its heading.
    pub p: [f32; 3],
    pub yaw: f32,
    /// Height lost per unit along (negative climbs).
    pub slope: f32,
    /// Heading turned per unit along (positive turns right).
    pub bend: f32,
    pub width: f32,
    pub walls: bool,
    pub ground: Ground,
    /// Cart lines laid here, a bit per line.
    pub lanes: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// A gem shard (Shaft) or a gold nugget (Cart), for the first to it.
    Gem,
    /// A gold ring in the air: a burst for each griffin through it.
    Ring,
    /// A gargoyle on the wing: shoot it for a burst, never fly into it.
    Gargoyle,
    /// A timber fallen across a cart line: jump it or leave the line.
    Timber,
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub s: f32,
    pub x: f32,
    pub z: f32,
    pub kind: ItemKind,
}

/// Something solid in the gorge: a spire from the floor or a rampart
/// across it, as a box in course space.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub s: f32,
    pub len: f32,
    pub x: (f32, f32),
    pub z: (f32, f32),
}

/// A broken rail: a cart on `lane` must be off the ground from `s.0` to `s.1`.
#[derive(Clone, Copy, Debug)]
pub struct Gap {
    pub lane: u8,
    pub s: (f32, f32),
}

pub struct Track {
    pub samples: Vec<Sample>,
    /// Where the line is.
    pub finish: f32,
    pub items: Vec<Item>,
    pub blocks: Vec<Block>,
    pub gaps: Vec<Gap>,
    /// Where the rope brings a fallen knight back: the start of each piece.
    pub posts: Vec<f32>,
}

impl Track {
    pub fn len(&self) -> f32 {
        (self.samples.len() - 1) as f32
    }
    /// The course at `s`, between its samples.
    pub fn at(&self, s: f32) -> Sample {
        let s = s.clamp(0.0, self.len());
        let i = (s.floor() as usize).min(self.samples.len() - 2);
        let f = s - i as f32;
        let (a, b) = (&self.samples[i], &self.samples[i + 1]);
        let mut out = *a;
        for k in 0..3 {
            out.p[k] = a.p[k] + (b.p[k] - a.p[k]) * f;
        }
        out.yaw = a.yaw + (b.yaw - a.yaw) * f;
        out
    }
    /// A point in course space, in the world: `x` to the right of the line
    /// (as the camera sees it), `z` up from it.
    pub fn world(&self, s: f32, x: f32, z: f32) -> [f32; 3] {
        let at = self.at(s);
        let (sin, cos) = at.yaw.sin_cos();
        [at.p[0] - sin * x, at.p[1] + cos * x, at.p[2] + z]
    }
    /// The last post at or behind `s`.
    pub fn post(&self, s: f32) -> f32 {
        self.posts.iter().copied().filter(|&p| p <= s).fold(0.0, f32::max)
    }
}

/// One stretch of a course, as authored.
#[derive(Clone, Copy)]
struct Piece {
    len: u16,
    turn: f32,
    drop: f32,
    width: f32,
    walls: bool,
    ground: Ground,
    lanes: u8,
}

const fn piece(len: u16, turn: f32, drop: f32, width: f32, walls: bool, ground: Ground) -> Piece {
    Piece { len, turn, drop, width, walls, ground, lanes: 0 }
}

const fn rails(len: u16, turn: f32, drop: f32, lanes: u8, ground: Ground) -> Piece {
    Piece { len, turn, drop, width: 5.6, walls: true, ground, lanes }
}

/// Walk the pieces a unit at a time into samples; the posts are where each
/// piece starts.
fn lay(pieces: &[Piece]) -> (Vec<Sample>, Vec<f32>) {
    let mut samples = Vec::new();
    let mut posts = Vec::new();
    let (mut p, mut yaw) = ([0.0f32; 3], 0.0f32);
    for pc in pieces {
        posts.push(samples.len() as f32);
        let len = f32::from(pc.len);
        let bend = pc.turn.to_radians() / len;
        for _ in 0..pc.len {
            samples.push(Sample {
                p,
                yaw,
                slope: pc.drop / len,
                bend,
                width: pc.width,
                walls: pc.walls,
                ground: pc.ground,
                lanes: pc.lanes,
            });
            let mid = yaw + bend * 0.5;
            p[0] += mid.cos();
            p[1] += mid.sin();
            p[2] -= pc.drop / len;
            yaw += bend;
        }
    }
    let last = *samples.last().unwrap();
    samples.push(Sample { p, yaw, ..last });
    (samples, posts)
}

/// The Shaft Run: down a winding mineshaft, about five hundred tiles.
const SHAFT: [Piece; 19] = [
    piece(14, 0.0, 0.0, 6.0, true, Ground::Floor),
    piece(26, 0.0, 5.0, 5.0, true, Ground::Floor),
    piece(36, 150.0, 3.0, 5.0, true, Ground::Floor),
    piece(22, 0.0, 0.0, 2.4, false, Ground::Bridge),
    piece(30, -120.0, 3.0, 5.0, true, Ground::Floor),
    piece(18, 0.0, 0.0, 5.0, true, Ground::Crossing),
    piece(28, 40.0, 2.0, 4.4, false, Ground::Lodestone),
    piece(30, 0.0, 4.0, 4.6, true, Ground::Sluice),
    piece(36, 170.0, 4.0, 5.0, true, Ground::Floor),
    piece(30, 0.0, 1.0, 4.0, false, Ground::Collapse),
    piece(24, -90.0, 5.0, 5.0, true, Ground::Floor),
    piece(20, 0.0, 0.0, 2.0, false, Ground::Bridge),
    piece(30, 120.0, 3.0, 5.0, true, Ground::Floor),
    piece(18, 0.0, 0.0, 5.0, true, Ground::Crossing),
    piece(32, -150.0, 4.0, 4.6, true, Ground::Sluice),
    piece(26, 0.0, 1.0, 4.4, false, Ground::Lodestone),
    piece(28, 0.0, 1.0, 4.0, false, Ground::Collapse),
    piece(22, -60.0, 6.0, 5.0, true, Ground::Floor),
    piece(16, 0.0, 0.0, 6.0, true, Ground::Sorter),
];

/// The Ore Cart: the old ore line through the galleries. Bit `i` of the
/// lanes is line `i` (left, middle, right).
const CART: [Piece; 18] = [
    rails(16, 0.0, 0.0, 0b111, Ground::Rails),
    rails(24, 0.0, 5.0, 0b111, Ground::Rails),
    rails(34, 120.0, 3.0, 0b111, Ground::Rails),
    rails(24, 0.0, 0.0, 0b110, Ground::Trestle),
    rails(30, -90.0, 4.0, 0b111, Ground::Rails),
    rails(26, 0.0, 2.0, 0b011, Ground::Rails),
    rails(30, 70.0, 3.0, 0b111, Ground::Rails),
    rails(28, 0.0, 0.0, 0b101, Ground::Trestle),
    rails(34, -140.0, 5.0, 0b111, Ground::Rails),
    rails(26, 0.0, 2.0, 0b110, Ground::Rails),
    rails(30, 60.0, 4.0, 0b111, Ground::Rails),
    rails(24, 0.0, 0.0, 0b011, Ground::Trestle),
    rails(32, -100.0, 4.0, 0b111, Ground::Rails),
    rails(26, 0.0, 2.0, 0b101, Ground::Rails),
    rails(30, 130.0, 4.0, 0b111, Ground::Rails),
    rails(24, 0.0, 0.0, 0b110, Ground::Trestle),
    rails(30, -50.0, 6.0, 0b111, Ground::Rails),
    rails(16, 0.0, 0.0, 0b010, Ground::Rails),
];

/// The Griffin Run: down the gorge under Dragon Keep and up to its gate.
const GORGE: [Piece; 10] = [
    piece(40, 0.0, 0.0, 14.0, true, Ground::Gorge),
    piece(60, 50.0, 4.0, 14.0, true, Ground::Gorge),
    piece(50, -70.0, 2.0, 12.0, true, Ground::Gorge),
    piece(70, 0.0, 6.0, 12.0, true, Ground::Gorge),
    piece(60, 90.0, 0.0, 14.0, true, Ground::Gorge),
    piece(60, -40.0, -3.0, 12.0, true, Ground::Gorge),
    piece(70, 0.0, -2.0, 12.0, true, Ground::Gorge),
    piece(60, -60.0, -4.0, 14.0, true, Ground::Gorge),
    piece(50, 30.0, -6.0, 14.0, true, Ground::Gorge),
    piece(40, 0.0, 0.0, 16.0, true, Ground::Gorge),
];

fn shaft() -> Track {
    let (samples, posts) = lay(&SHAFT);
    let at = |k: usize, ds: f32| posts[k] + ds;
    let gem = |s: f32, x: f32| Item { s, x, z: 0.45, kind: ItemKind::Gem };
    // Gem shards sit on the risky lines: the outside of the bends, the
    // edges of the bridges, beside the lodestone's drop, on rotten planks.
    let items = vec![
        gem(at(1, 14.0), -1.9),
        gem(at(2, 12.0), -2.0),
        gem(at(2, 24.0), -2.0),
        gem(at(3, 6.0), 0.85),
        gem(at(3, 12.0), -0.85),
        gem(at(3, 18.0), 0.85),
        gem(at(4, 15.0), 2.0),
        gem(at(5, 9.0), 0.0),
        gem(at(6, 10.0), 1.8),
        gem(at(6, 20.0), 1.8),
        gem(at(7, 15.0), -1.8),
        gem(at(8, 20.0), -2.0),
        gem(at(9, 9.0), 0.0),
        gem(at(9, 21.0), 0.0),
        gem(at(10, 12.0), 2.0),
        gem(at(11, 7.0), -0.7),
        gem(at(11, 14.0), 0.7),
        gem(at(12, 16.0), -2.0),
        gem(at(13, 9.0), 0.0),
        gem(at(14, 18.0), 1.8),
        gem(at(15, 12.0), 1.7),
        gem(at(16, 13.0), 0.0),
        gem(at(17, 11.0), 1.9),
    ];
    let finish = posts[18] + 4.0;
    Track { samples, finish, items, blocks: Vec::new(), gaps: Vec::new(), posts }
}

fn cart() -> Track {
    let (samples, posts) = lay(&CART);
    let at = |k: usize, ds: f32| posts[k] + ds;
    let nugget = |s: f32, lane: usize, high: bool| Item {
        s,
        x: LANES[lane],
        z: if high { 1.9 } else { 0.6 },
        kind: ItemKind::Gem,
    };
    let timber = |s: f32, lane: usize| Item { s, x: LANES[lane], z: 0.0, kind: ItemKind::Timber };
    let items = vec![
        nugget(at(1, 8.0), 0, false),
        nugget(at(1, 16.0), 2, true),
        timber(at(2, 10.0), 2),
        nugget(at(2, 20.0), 1, true),
        nugget(at(3, 12.0), 2, false),
        timber(at(4, 8.0), 1),
        nugget(at(4, 18.0), 0, true),
        timber(at(5, 14.0), 0),
        nugget(at(6, 10.0), 2, false),
        timber(at(6, 22.0), 1),
        nugget(at(7, 14.0), 0, true),
        nugget(at(8, 12.0), 1, false),
        timber(at(8, 24.0), 2),
        nugget(at(9, 10.0), 2, true),
        timber(at(10, 12.0), 1),
        nugget(at(10, 20.0), 0, false),
        nugget(at(11, 10.0), 1, true),
        timber(at(12, 12.0), 1),
        nugget(at(12, 22.0), 2, false),
        timber(at(13, 18.0), 0),
        nugget(at(14, 14.0), 1, true),
        timber(at(16, 10.0), 2),
        nugget(at(16, 20.0), 1, false),
    ];
    // Broken rails: jump them.
    let gap = |lane: u8, s0: f32, len: f32| Gap { lane, s: (s0, s0 + len) };
    let gaps = vec![
        gap(0, at(2, 18.0), 3.0),
        gap(1, at(3, 10.0), 3.0),
        gap(2, at(4, 20.0), 3.0),
        gap(1, at(6, 6.0), 3.0),
        gap(0, at(7, 8.0), 3.0),
        gap(2, at(7, 18.0), 3.0),
        gap(1, at(8, 16.0), 3.0),
        gap(2, at(10, 4.0), 3.0),
        gap(0, at(11, 14.0), 3.0),
        gap(2, at(13, 8.0), 3.0),
        gap(1, at(15, 10.0), 3.0),
    ];
    let finish = posts[17] + 6.0;
    Track { samples, finish, items, blocks: Vec::new(), gaps, posts }
}

fn gorge() -> Track {
    let (samples, posts) = lay(&GORGE);
    let at = |k: usize, ds: f32| posts[k] + ds;
    let ring = |s: f32, x: f32, z: f32| Item { s, x, z, kind: ItemKind::Ring };
    let goyle = |s: f32, x: f32, z: f32| Item { s, x, z, kind: ItemKind::Gargoyle };
    let items = vec![
        ring(at(0, 24.0), 0.0, 0.0),
        ring(at(1, 14.0), -2.5, 1.0),
        ring(at(1, 40.0), 2.5, -1.0),
        goyle(at(1, 28.0), 0.0, 0.5),
        ring(at(2, 20.0), 0.0, 2.0),
        goyle(at(2, 34.0), -2.0, 0.0),
        goyle(at(2, 36.0), 2.0, 0.0),
        ring(at(3, 12.0), -3.0, -1.5),
        ring(at(3, 30.0), 3.0, -1.5),
        ring(at(3, 50.0), 0.0, 2.0),
        goyle(at(3, 60.0), 0.0, 0.0),
        ring(at(4, 20.0), 2.0, 0.5),
        goyle(at(4, 38.0), -1.5, 1.5),
        ring(at(4, 50.0), -2.0, -0.5),
        ring(at(5, 18.0), 0.0, -2.0),
        goyle(at(5, 30.0), 2.5, -1.0),
        ring(at(5, 44.0), 0.0, 2.0),
        goyle(at(6, 16.0), 0.0, 1.0),
        ring(at(6, 30.0), -3.0, 0.0),
        ring(at(6, 52.0), 3.0, 0.0),
        goyle(at(7, 20.0), -2.0, -1.0),
        ring(at(7, 36.0), 0.0, 0.0),
        goyle(at(8, 16.0), 1.0, 1.0),
        ring(at(8, 30.0), 0.0, 1.5),
    ];
    // Spires up from the gorge floor (pass beside or over them), and the
    // old ramparts across it (under or over).
    let spire = |s: f32, x0: f32, x1: f32, top: f32| Block { s, len: 1.6, x: (x0, x1), z: (-9.0, top) };
    let rampart = |s: f32, z0: f32, z1: f32| Block { s, len: 1.2, x: (-8.0, 8.0), z: (z0, z1) };
    let blocks = vec![
        spire(at(1, 22.0), -4.5, -1.0, 1.0),
        spire(at(2, 10.0), 1.0, 4.5, 2.5),
        rampart(at(3, 22.0), -0.6, 0.8),
        spire(at(3, 40.0), -1.5, 1.5, -0.5),
        spire(at(4, 30.0), -4.5, -0.5, 2.5),
        spire(at(4, 32.0), 1.8, 4.5, 2.5),
        rampart(at(5, 36.0), 0.8, 2.6),
        spire(at(6, 24.0), -1.2, 1.2, 2.6),
        rampart(at(6, 44.0), -2.6, -0.4),
        spire(at(7, 28.0), 0.5, 4.5, 1.0),
        rampart(at(8, 38.0), -0.8, 0.6),
    ];
    let finish = posts[9] + 20.0;
    Track { samples, finish, items, blocks, gaps: Vec::new(), posts }
}

/// The course, laid once.
pub fn track(course: Course) -> &'static Track {
    static TRACKS: [OnceLock<Track>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    match course {
        Course::Shaft => TRACKS[0].get_or_init(shaft),
        Course::Cart => TRACKS[1].get_or_init(cart),
        Course::Griffin => TRACKS[2].get_or_init(gorge),
    }
}

/// Rotten planks in a collapsing drift: every other two-tile plank, by
/// index from the start of the course.
pub fn rotten(plank: u16) -> bool {
    plank % 2 == 1
}

/// The ore carts at the crossings: where each is across the way at race
/// tick `t` (two carts a crossing, one every four seconds each way, two
/// seconds to cross; a cart off the way waits at 9 either side).
pub fn crossing_carts(track: &Track, t: u32) -> Vec<(f32, f32)> {
    let phase = |offset: u32| ((t + offset) % (4 * HZ)) as f32 / (2 * HZ) as f32;
    let x = |p: f32| if p < 1.0 { -4.0 + 8.0 * p } else { 9.0 };
    track
        .posts
        .iter()
        .filter(|&&p| track.at(p).ground == Ground::Crossing)
        .flat_map(|&start| [(start + 6.0, x(phase(0))), (start + 13.0, -x(phase(2 * HZ)))])
        .collect()
}

/// What a crash was, for the right picture and the right joke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wreck {
    #[default]
    None,
    /// Off an edge and down the shaft.
    Fall,
    /// Hit by a crossing ore cart.
    Bonk,
    /// A cart line ended at its buffer stop with a cart still on it.
    Buffer,
    /// A cart down a broken rail.
    Gap,
    /// A cart into a fallen timber.
    Timber,
    /// A griffin into stone.
    Tumble,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rider {
    pub s: f32,
    pub x: f32,
    pub z: f32,
    pub vs: f32,
    pub vx: f32,
    pub vz: f32,
    /// The cart line it is on (or switching to).
    pub lane: u8,
    /// How far the ball has rolled (radians) or the wings have beaten.
    pub spin: f32,
    /// Ticks left of a crash, and what it was.
    pub crash: u32,
    pub wreck: Wreck,
    pub crashes: u16,
    /// Ticks left knocked silly (a cart's bonk, a bolt in the feathers).
    pub stun: u32,
    /// Ticks left of a burst.
    pub burst: u32,
    /// Ticks left of a barrel roll, and of the rest after one.
    pub rolling: u32,
    pub rest: u32,
    /// Ticks until the next bolt.
    pub reload: u32,
    /// Gems and nuggets picked up, rings flown and gargoyles smashed.
    pub haul: u16,
    /// The rings this griffin has flown through, a bit each.
    pub rings: u64,
    /// The race tick it crossed the line.
    pub finish: Option<u32>,
    /// F and E as held last tick (bits 0 and 1), for presses.
    pub keys: u8,
}

impl Rider {
    fn new(s: f32, x: f32, lane: u8) -> Rider {
        Rider {
            s,
            x,
            z: 0.0,
            vs: 0.0,
            vx: 0.0,
            vz: 0.0,
            lane,
            spin: 0.0,
            crash: 0,
            wreck: Wreck::None,
            crashes: 0,
            stun: 0,
            burst: 0,
            rolling: 0,
            rest: 0,
            reload: 0,
            haul: 0,
            rings: 0,
            finish: None,
            keys: 0,
        }
    }
    fn valid(&self) -> bool {
        [self.s, self.x, self.z, self.vs, self.vx, self.vz, self.spin]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 10_000.0)
            && usize::from(self.lane) < LANES.len()
            && self.crash <= CRASH
            && self.stun <= CRASH
            && self.burst <= BURST
            && self.rolling <= ROLL
            && self.rest <= ROLL_REST
            && self.reload <= BOLT_REST
    }
    /// Racing: not crashed and not home.
    pub fn racing(&self) -> bool {
        self.crash == 0 && self.finish.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bolt {
    pub owner: u32,
    pub s: f32,
    pub x: f32,
    pub z: f32,
    pub life: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Race3d {
    pub course: Course,
    pub room: usize,
    /// Ticks since it began (the title card first).
    pub t: u32,
    pub riders: BTreeMap<u32, Rider>,
    pub ante: BTreeMap<u32, u32>,
    /// Gems and nuggets taken, and gargoyles smashed, by item index.
    pub taken: BTreeSet<u16>,
    /// Rotten planks a ball has passed: the race tick each began to go.
    pub planks: BTreeMap<u16, u32>,
    pub bolts: Vec<Bolt>,
    pub done: bool,
    /// The race tick it ended, and the gold each knight took home.
    pub ended: u32,
    pub won: BTreeMap<u32, u32>,
    /// A free practice flight at the Eyrie: no antes, no pot, no laurel.
    #[serde(default)]
    pub practice: bool,
}

impl Race3d {
    pub fn valid(&self) -> bool {
        let track = track(self.course);
        self.t <= TITLE + RACE_TIME + PODIUM + HZ
            && self.riders.len() <= MAX_PLAYERS as usize
            && self.ante.len() <= MAX_PLAYERS as usize
            && self.won.len() <= MAX_PLAYERS as usize
            && self.riders.keys().chain(self.ante.keys()).all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.riders.values().all(Rider::valid)
            && self.taken.iter().all(|&i| usize::from(i) < track.items.len())
            && self.planks.len() <= track.samples.len()
            && self.bolts.len() <= 4 * MAX_PLAYERS as usize
            && self.bolts.iter().all(|b| b.life <= BOLT_LIFE && b.s.is_finite() && b.x.is_finite() && b.z.is_finite())
    }
    pub fn titled(&self) -> bool {
        self.t < TITLE
    }
    /// Seconds left on the race clock.
    pub fn left(&self) -> u32 {
        (TITLE + RACE_TIME).saturating_sub(self.t).min(RACE_TIME).div_ceil(HZ)
    }
    /// While this race owns the screen: the race, then its board.
    pub fn showing(&self) -> bool {
        !self.done || self.t < self.ended + PODIUM
    }
    /// The field in order: those home by when they got there, then the
    /// rest by how far down they are.
    pub fn order(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.riders.keys().copied().collect();
        ids.sort_by(|a, b| {
            let (ra, rb) = (&self.riders[a], &self.riders[b]);
            match (ra.finish, rb.finish) {
                (Some(x), Some(y)) => x.cmp(&y).then(a.cmp(b)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => rb.s.total_cmp(&ra.s).then(a.cmp(b)),
            }
        });
        ids
    }
    /// A knight's place, from one.
    pub fn place(&self, id: u32) -> usize {
        1 + self.order().iter().position(|&r| r == id).unwrap_or(0)
    }
}

/// Where knight number `k` of the field starts.
fn grid(course: Course, k: usize) -> (f32, f32, u8) {
    match course {
        Course::Shaft => (2.0 + (k / 2) as f32 * 1.4, if k % 2 == 0 { -1.0 } else { 1.0 }, 1),
        // Three abreast on the depot's three lines, the fourth behind.
        Course::Cart => {
            let (s, lane) = [(6.0, 1u8), (4.5, 0), (4.5, 2), (2.5, 1)][k % 4];
            (s, LANES[usize::from(lane)], lane)
        }
        Course::Griffin => (2.0 + (k / 2) as f32 * 2.0, if k % 2 == 0 { -1.6 } else { 1.6 }, 1),
    }
}

/// What happened in a tick, for the room's sounds.
#[derive(Default)]
struct Noise {
    sounds: Vec<&'static str>,
}

impl Run {
    /// One hall on each floor is the floor's race hall: the seed's pick of
    /// the halls the follies left.
    pub(super) fn pick_race3d(&mut self) {
        self.race3d = None;
        self.race3d_room = None;
        if self.dungeon.depth == 0 {
            return;
        }
        let camps = |i: usize| {
            self.boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(i).is_some() || g.guardian == i)
        };
        let halls: Vec<usize> = (0..self.dungeon.rooms.len())
            .filter(|&i| {
                let r = &self.dungeon.rooms[i];
                r.kind == RoomKind::Hall
                    && !r.great()
                    && !r.cleared
                    && i != self.at
                    && Some(i) != self.folly_room
                    && Some(i) != self.gantry_room
                    && !camps(i)
            })
            .collect();
        if halls.is_empty() {
            return;
        }
        let pick = (mix(self.seed ^ self.raid_id.rotate_left(17) ^ u64::from(self.dungeon.depth) << 3)
            % halls.len() as u64) as usize;
        self.race3d_room = Some(halls[pick]);
    }

    /// Into the race hall: the antes go in and the field lines up.
    pub(super) fn start_race3d(&mut self, index: usize) {
        let course = Course::of_floor(self.dungeon.depth);
        let (ante, scores) = folly::take_antes(&mut self.players);
        let riders = scores
            .keys()
            .enumerate()
            .map(|(k, &id)| {
                let (s, x, lane) = grid(course, k);
                (id, Rider::new(s, x, lane))
            })
            .collect();
        self.race3d = Some(Race3d {
            course,
            room: index,
            t: 0,
            riders,
            ante,
            taken: BTreeSet::new(),
            planks: BTreeMap::new(),
            bolts: Vec::new(),
            done: false,
            ended: 0,
            won: BTreeMap::new(),
            practice: false,
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// Whether anybody has won a race yet, in the realm or this run.
    pub fn race_won(&self) -> bool {
        self.home.countryside.race_won || self.marks.contains_key(RACE_MARK)
    }

    /// The Griffin Eyrie: E by the perch and the griffin master saddles a
    /// griffin for every knight in the yard, for a free flight down the
    /// gorge. Again as often as you like.
    pub(super) fn tick_eyrie(&mut self) {
        if !self.at_home_now() || self.room().kind != RoomKind::Eyrie || self.race3d_showing() {
            return;
        }
        let (px, py) = (PERCH.0 * TILE_UNITS, PERCH.1 * TILE_UNITS);
        let asked = self.players.values().any(|h| {
            h.hp > 0 && !h.stone && h.taking && (h.x - px).hypot(h.y - py) < PERCH_REACH * TILE_UNITS
        });
        if !asked {
            return;
        }
        let riders = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .enumerate()
            .map(|(k, (&id, _))| {
                let (s, x, lane) = grid(Course::Griffin, k);
                (id, Rider::new(s, x, lane))
            })
            .collect();
        self.race3d = Some(Race3d {
            course: Course::Griffin,
            room: self.at,
            t: 0,
            riders,
            ante: BTreeMap::new(),
            taken: BTreeSet::new(),
            planks: BTreeMap::new(),
            bolts: Vec::new(),
            done: false,
            ended: 0,
            won: BTreeMap::new(),
            practice: true,
        });
        self.sounds.push("swing");
        self.cues.push("folly".into());
    }

    /// A race is on in the room the party stands in (or its board is up).
    pub fn race3d_showing(&self) -> bool {
        self.race3d.as_ref().is_some_and(|r| r.room == self.at && r.showing())
    }

    /// The race's tick: every knight's keys drive their own ball, cart or
    /// griffin; the knights themselves stand still in the hall meanwhile.
    pub(super) fn tick_race3d(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut race) = self.race3d.take() else {
            return;
        };
        if race.room != self.at || !race.showing() {
            self.race3d = Some(race);
            return;
        }
        race.t += 1;
        if race.done {
            self.race3d = Some(race);
            return;
        }
        // A knight who sits down mid-race starts from the top.
        let standing: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, _)| id)
            .collect();
        for &id in &standing {
            if !race.riders.contains_key(&id) && race.riders.len() < MAX_PLAYERS as usize {
                let (s, x, lane) = grid(race.course, race.riders.len());
                race.riders.insert(id, Rider::new(s, x, lane));
            }
        }
        if race.titled() {
            if race.t == TITLE - HZ / 2 {
                self.sounds.push("circle_toll");
            }
            self.race3d = Some(race);
            return;
        }
        let mut noise = Noise::default();
        let ids: Vec<u32> = race.riders.keys().copied().collect();
        for &id in &ids {
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid() && standing.contains(&id))
                .unwrap_or_default();
            let mut rider = race.riders[&id].clone();
            let (f, e) = (input.fire, input.interact);
            let pressed = (f && rider.keys & 1 == 0, e && rider.keys & 2 == 0);
            rider.keys = u8::from(f) | u8::from(e) << 1;
            match race.course {
                Course::Shaft => roll_ball(&mut race, &mut rider, input, pressed, &mut noise),
                Course::Cart => ride_cart(&mut race, &mut rider, input, pressed, &mut noise),
                Course::Griffin => fly_griffin(&mut race, id, &mut rider, input, pressed.1, &mut noise),
            }
            let track = track(race.course);
            if rider.finish.is_none() && rider.crash == 0 && rider.s >= track.finish {
                rider.finish = Some(race.t);
                noise.sounds.push("wheel_land");
            }
            race.riders.insert(id, rider);
        }
        match race.course {
            Course::Shaft => bump_balls(&mut race, &mut noise),
            Course::Cart => bump_carts(&mut race, &mut noise),
            Course::Griffin => fly_bolts(&mut race, &mut noise),
        }
        self.sounds.extend(noise.sounds);
        // Done when everyone still standing is home, or the clock is out.
        let all_home = race
            .riders
            .iter()
            .filter(|(id, _)| standing.contains(id))
            .all(|(_, r)| r.finish.is_some());
        if all_home || race.t >= TITLE + RACE_TIME {
            self.finish_race3d(&mut race);
        }
        self.race3d = Some(race);
    }

    /// The board: the first home takes the pot and the laurel, and everyone
    /// keeps two gold for each thing picked up on the way.
    fn finish_race3d(&mut self, race: &mut Race3d) {
        race.done = true;
        race.ended = race.t;
        let order = race.order();
        let best = order.first().copied();
        // A race won leaves its mark on the realm.
        if best.and_then(|id| race.riders.get(&id)).is_some_and(|r| r.finish.is_some()) {
            self.marks.insert(RACE_MARK.into(), 1);
        }
        if race.practice {
            // Nothing staked, nothing paid: the flight is its own reward.
            for &id in race.riders.keys() {
                race.won.insert(id, 0);
            }
            let name = best
                .and_then(|id| self.players.get(&id))
                .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
            self.found = Some((self.tick, 0, format!("{name} FLEW THE GORGE FIRST!")));
            self.sounds.push("play_card");
            return;
        }
        let pot: u32 = race.ante.values().sum();
        let rivals = race.riders.len() > 1;
        for (&id, rider) in &race.riders {
            let mut gold = u32::from(rider.haul) * HAUL_GOLD;
            if Some(id) == best {
                gold += pot;
            }
            race.won.insert(id, gold);
            if let Some(hero) = self.players.get_mut(&id) {
                hero.carried.add(Spoil::Gold, gold);
                if rivals {
                    hero.laurel = Some(id) == best;
                }
            }
        }
        if let Some(room) = self.dungeon.rooms.get_mut(race.room) {
            room.cleared = true;
        }
        let name = best
            .and_then(|id| self.players.get(&id))
            .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
        self.found = Some((self.tick, 0, format!("{name} WON {}!", race.course.name())));
        self.sounds.push("play_card");
        self.cues.push("pastime_won".into());
    }
}

/// Knocked out of it: the crash's clock starts and the tally goes up.
fn wreck(rider: &mut Rider, what: Wreck, noise: &mut Noise) {
    rider.crash = CRASH;
    rider.wreck = what;
    rider.crashes = rider.crashes.saturating_add(1);
    rider.burst = 0;
    noise.sounds.push(match what {
        Wreck::Fall | Wreck::Gap => "knight_fall",
        Wreck::Bonk | Wreck::Timber => "helmet_pop",
        Wreck::Buffer => "clang",
        _ => "rock_land",
    });
}

/// How far before the last post the rope sets a knight down: a run-up to
/// whatever threw them off.
pub const RUN_UP: f32 = 6.0;

/// Back on the course after the rope: a run-up short of the last post,
/// still, in the middle.
fn rope_back(track: &Track, rider: &mut Rider, x: f32) {
    rider.s = (track.post(rider.s) - RUN_UP).max(1.0);
    (rider.x, rider.z, rider.vs, rider.vx, rider.vz) = (x, 0.0, 0.0, 0.0, 0.0);
    rider.wreck = Wreck::None;
}

/// Pick up any shared gem within reach.
fn pick_up(race: &mut Race3d, rider: &mut Rider, reach: f32, noise: &mut Noise) {
    let track = track(race.course);
    for (i, item) in track.items.iter().enumerate() {
        if item.kind != ItemKind::Gem || (item.s - rider.s).abs() > reach {
            continue;
        }
        let i = i as u16;
        if race.taken.contains(&i) {
            continue;
        }
        let dz = item.z - (rider.z + 0.5);
        if (item.x - rider.x).hypot(dz) <= reach + 0.2 {
            race.taken.insert(i);
            rider.haul = rider.haul.saturating_add(1);
            noise.sounds.push("gold");
        }
    }
}

// ─── the Shaft Run ──────────────────────────────────────────────────────────

/// Gravity down a ramp, a knight's push, the brake, rolling drag, and how
/// fast a ball may go.
const FALL_G: f32 = 9.0;
const PUSH: f32 = 3.2;
const BRAKE: f32 = 7.0;
const DRAG: f32 = 0.12;
const TOP: f32 = 11.0;
const STEER: f32 = 7.0;
const HOP: f32 = 4.6;
const HOP_G: f32 = 14.0;

fn roll_ball(race: &mut Race3d, rider: &mut Rider, input: Input, pressed: (bool, bool), noise: &mut Noise) {
    let track = track(race.course);
    if rider.finish.is_some() {
        // Into the sorter: the ball rolls to a stop at the bottom.
        rider.vs *= 0.9;
        rider.s = (rider.s + rider.vs * DT).min(track.len() - 1.0);
        rider.spin += rider.vs * DT / BALL_R;
        return;
    }
    if rider.crash > 0 {
        rider.crash -= 1;
        if rider.wreck == Wreck::Fall {
            // Down the shaft, tumbling, until the rope comes for it.
            rider.vz -= HOP_G * DT;
            rider.z = (rider.z + rider.vz * DT).max(-14.0);
            rider.s += rider.vs * DT * 0.5;
            rider.x += rider.vx * DT * 0.5;
            rider.spin += 0.5;
        } else {
            rider.vs *= 0.85;
            rider.s = (rider.s + rider.vs * DT).max(0.5);
            rider.x += rider.vx * DT;
            rider.vx *= 0.85;
        }
        if rider.crash == 0 {
            if rider.wreck == Wreck::Fall {
                rope_back(track, rider, 0.0);
            }
            rider.wreck = Wreck::None;
        }
        return;
    }
    let at = track.at(rider.s);
    let grounded = rider.z <= 0.0;
    // Along: the slope, the knight's feet, the brake, drag.
    let mut a = FALL_G * at.slope;
    if rider.stun == 0 {
        if input.move_y < 0 {
            a += PUSH;
        }
        if input.move_y > 0 || input.interact {
            a -= BRAKE * rider.vs.signum();
        }
    }
    if at.ground == Ground::Sluice {
        a += 2.0;
        if race.t % 20 == 0 {
            noise.sounds.push("step_wet");
        }
    }
    // The lodestone: a pull toward its rail, and a ride along it.
    let rail = 1.3;
    let mut ax = 0.0;
    if at.ground == Ground::Lodestone && grounded {
        let off = rail - rider.x;
        if off.abs() < 1.8 {
            ax += 3.2 * off.signum();
        }
        if off.abs() < 0.35 {
            a += 3.0;
        }
    }
    if at.ground == Ground::Sluice {
        ax += 4.0 * (rider.s * 0.35).sin();
    }
    a -= DRAG * rider.vs;
    rider.vs = (rider.vs + a * DT).clamp(-4.0, TOP);
    if rider.stun == 0 {
        ax += STEER * f32::from(input.move_x);
    }
    rider.stun = rider.stun.saturating_sub(1);
    // Round a bend the ball runs wide.
    ax -= rider.vs * rider.vs * at.bend;
    rider.vx = (rider.vx + ax * DT) * if grounded { 0.97 } else { 1.0 };
    // The hop: the knight kicks off the ball's wall.
    if pressed.0 && grounded {
        rider.vz = HOP;
        noise.sounds.push("roll");
    }
    if !grounded || rider.vz > 0.0 {
        rider.vz -= HOP_G * DT;
        rider.z += rider.vz * DT;
        if rider.z <= 0.0 {
            rider.z = 0.0;
            rider.vz = 0.0;
            noise.sounds.push("wheel_land");
        }
    }
    rider.s += rider.vs * DT;
    rider.x += rider.vx * DT;
    rider.spin += rider.vs * DT / BALL_R;
    if rider.s < 0.5 {
        rider.s = 0.5;
        rider.vs = rider.vs.abs() * 0.4;
    }
    let at = track.at(rider.s);
    let half = at.width * 0.5;
    if at.walls {
        if rider.x.abs() > half - BALL_R {
            rider.x = rider.x.signum() * (half - BALL_R);
            rider.vx = -rider.vx * 0.45;
            rider.vs *= 0.94;
            if rider.vx.abs() > 1.5 {
                noise.sounds.push("clang");
            }
        }
    } else if rider.x.abs() > half && rider.z <= 0.05 {
        rider.vx = rider.x.signum() * 2.0;
        rider.vz = 0.0;
        wreck(rider, Wreck::Fall, noise);
        return;
    }
    // Rotten planks: the first ball over one starts it going; a ball on a
    // plank that has gone goes with it.
    if at.ground == Ground::Collapse {
        let plank = (rider.s / 2.0) as u16;
        if rotten(plank) {
            let began = *race.planks.entry(plank).or_insert(race.t);
            if race.t >= began + ROT && rider.z <= 0.05 {
                rider.vz = 0.0;
                wreck(rider, Wreck::Fall, noise);
                return;
            }
        }
    }
    // The crossing ore carts. A bonked ball is let through the rest of
    // the crossing.
    rider.rest = rider.rest.saturating_sub(1);
    if at.ground == Ground::Crossing && rider.z < 1.0 && rider.rest == 0 {
        for (cs, cx) in crossing_carts(track, race.t) {
            if (cs - rider.s).abs() < 1.3 && (cx - rider.x).abs() < 1.6 {
                rider.vs = -3.0;
                rider.vx = if cx < rider.x { 5.0 } else { -5.0 };
                wreck(rider, Wreck::Bonk, noise);
                rider.crash = 20;
                rider.rest = ROLL_REST;
                return;
            }
        }
    }
    pick_up(race, rider, 0.9, noise);
}

/// Balls that meet bounce apart and trade their pace.
fn bump_balls(race: &mut Race3d, noise: &mut Noise) {
    let ids: Vec<u32> = race.riders.keys().copied().collect();
    for (k, &a) in ids.iter().enumerate() {
        for &b in &ids[k + 1..] {
            let (ra, rb) = (&race.riders[&a], &race.riders[&b]);
            if !ra.racing() || !rb.racing() || (ra.z - rb.z).abs() > 0.8 {
                continue;
            }
            let (ds, dx) = (rb.s - ra.s, rb.x - ra.x);
            let d = ds.hypot(dx);
            if d >= 2.0 * BALL_R || d < 1e-4 {
                continue;
            }
            let (ns, nx) = (ds / d, dx / d);
            let push = (2.0 * BALL_R - d) * 0.5;
            let along = (rb.vs - ra.vs) * ns + (rb.vx - ra.vx) * nx;
            let mut ra = ra.clone();
            let mut rb = rb.clone();
            ra.s -= ns * push;
            ra.x -= nx * push;
            rb.s += ns * push;
            rb.x += nx * push;
            if along < 0.0 {
                let j = -along * 0.9;
                ra.vs -= j * ns;
                ra.vx -= j * nx;
                rb.vs += j * ns;
                rb.vx += j * nx;
                noise.sounds.push("clang");
            }
            race.riders.insert(a, ra);
            race.riders.insert(b, rb);
        }
    }
}

// ─── the Ore Cart ───────────────────────────────────────────────────────────

/// A cart's cruising pace, pumped and braked, how fast it gets there, the
/// jump, and how quick a cart crosses to another line.
const CRUISE: f32 = 8.5;
const PUMPED: f32 = 11.0;
const BRAKED: f32 = 4.0;
const SPUR: f32 = 4.0;
const JUMP: f32 = 5.6;
const JUMP_G: f32 = 15.0;
const SWITCH: f32 = 11.0;
/// How high a fallen timber stands: a cart's wheels must clear it.
pub const TIMBER_H: f32 = 0.6;

/// Whether line `lane` is laid at `s`.
pub fn laid(track: &Track, s: f32, lane: u8) -> bool {
    track.at(s).lanes & (1 << lane) != 0
}

/// Whether line `lane` is broken at `s`.
pub fn broken(track: &Track, s: f32, lane: u8) -> bool {
    track.gaps.iter().any(|g| g.lane == lane && s >= g.s.0 && s < g.s.1)
}

fn ride_cart(race: &mut Race3d, rider: &mut Rider, input: Input, pressed: (bool, bool), noise: &mut Noise) {
    let track = track(race.course);
    if rider.finish.is_some() {
        rider.vs *= 0.92;
        rider.s = (rider.s + rider.vs * DT).min(track.len() - 1.0);
        rider.spin += rider.vs * DT;
        return;
    }
    if rider.crash > 0 {
        rider.crash -= 1;
        // The cart goes over and the knight goes up; then the rope.
        rider.vz -= JUMP_G * DT;
        rider.z = (rider.z + rider.vz * DT).max(if rider.wreck == Wreck::Gap { -12.0 } else { 0.0 });
        rider.s += rider.vs * DT;
        rider.vs *= 0.9;
        rider.spin += 0.4;
        if rider.crash == 0 {
            let s = (track.post(rider.s) - RUN_UP).max(1.0);
            // Back on a line that is laid there.
            let lane = (0..3u8)
                .min_by_key(|&l| (!laid(track, s, l), (i32::from(l) - 1).abs()))
                .unwrap_or(1);
            rider.lane = lane;
            rope_back(track, rider, LANES[usize::from(lane)]);
            // Set back on the rails rolling.
            rider.vs = BRAKED;
        }
        return;
    }
    let at = track.at(rider.s);
    let grounded = rider.z <= 0.0 && rider.vz <= 0.0;
    let goal = if input.move_y < 0 {
        PUMPED
    } else if input.move_y > 0 {
        BRAKED
    } else {
        CRUISE
    };
    let pull = (goal - rider.vs).clamp(-SPUR * DT * 2.0, SPUR * DT);
    rider.vs = (rider.vs + pull + FALL_G * at.slope * DT * 0.5).clamp(0.0, PUMPED + 2.0);
    // Switch lines: toward the arrow held, or else to the nearest other
    // line laid ahead.
    if pressed.1 {
        let ahead = rider.s + 2.0;
        let dir = i32::from(input.move_x);
        let options: Vec<i32> = if dir != 0 { vec![dir] } else { vec![1, -1] };
        for d in options {
            let to = i32::from(rider.lane) + d;
            if (0..3).contains(&to) && laid(track, ahead, to as u8) {
                rider.lane = to as u8;
                noise.sounds.push("wheel_spin");
                break;
            }
        }
    }
    let target = LANES[usize::from(rider.lane)];
    let step = SWITCH * DT;
    rider.x += (target - rider.x).clamp(-step, step);
    if pressed.0 && grounded {
        rider.vz = JUMP;
        noise.sounds.push("roll");
    }
    if !grounded || rider.vz > 0.0 {
        rider.vz -= JUMP_G * DT;
        rider.z += rider.vz * DT;
        if rider.z <= 0.0 {
            rider.z = 0.0;
            rider.vz = 0.0;
            noise.sounds.push("wheel_land");
        }
    }
    let before = rider.s;
    rider.s += rider.vs * DT;
    rider.spin += rider.vs * DT;
    let on_rail = (rider.x - target).abs() < 0.3;
    // A line that ends at a buffer stop with the cart still on it.
    if !laid(track, rider.s, rider.lane) && laid(track, before, rider.lane) {
        rider.s = before;
        rider.vz = 4.0;
        wreck(rider, Wreck::Buffer, noise);
        return;
    }
    if rider.z <= 0.0 && on_rail && broken(track, rider.s, rider.lane) {
        rider.vz = 0.0;
        wreck(rider, Wreck::Gap, noise);
        return;
    }
    // A timber across the line: over it or into it.
    for item in &track.items {
        if item.kind == ItemKind::Timber
            && (item.x - rider.x).abs() < 1.0
            && before <= item.s
            && rider.s > item.s - 0.6
            && rider.z < TIMBER_H
        {
            rider.vz = 5.0;
            rider.vs *= 0.5;
            wreck(rider, Wreck::Timber, noise);
            return;
        }
    }
    pick_up(race, rider, 0.8, noise);
}

/// A cart that runs into the back of another on its line shunts it on and
/// stops dead itself.
fn bump_carts(race: &mut Race3d, noise: &mut Noise) {
    let ids: Vec<u32> = race.riders.keys().copied().collect();
    for (k, &a) in ids.iter().enumerate() {
        for &b in &ids[k + 1..] {
            let (ra, rb) = (&race.riders[&a], &race.riders[&b]);
            if !ra.racing() || !rb.racing() || (ra.x - rb.x).abs() > 0.8 || (ra.z - rb.z).abs() > 0.8 {
                continue;
            }
            if (ra.s - rb.s).abs() >= 1.4 {
                continue;
            }
            let (back, front) = if ra.s < rb.s { (a, b) } else { (b, a) };
            let mut rear = race.riders[&back].clone();
            let mut lead = race.riders[&front].clone();
            if rear.vs <= lead.vs {
                continue;
            }
            lead.vs = rear.vs + 1.0;
            rear.vs = (lead.vs - 4.0).max(1.0);
            rear.s = lead.s - 1.4;
            noise.sounds.push("clang");
            race.riders.insert(back, rear);
            race.riders.insert(front, lead);
        }
    }
}

// ─── the Griffin Run ────────────────────────────────────────────────────────

/// The gorge's pace, a burst on top, a ruffled griffin's, how quick a
/// griffin answers the reins, and how fast a roll throws it aside.
const WING: f32 = 10.0;
const WING_BURST: f32 = 5.0;
const WING_RUFFLED: f32 = 5.0;
const REIN: f32 = 16.0;
const ROLL_THROW: f32 = 7.0;

fn fly_griffin(race: &mut Race3d, id: u32, rider: &mut Rider, input: Input, rolled: bool, noise: &mut Noise) {
    let track = track(race.course);
    rider.spin += 0.35;
    if rider.finish.is_some() {
        rider.vs = (rider.vs - 0.2).max(4.0);
        rider.s = (rider.s + rider.vs * DT).min(track.len() - 1.0);
        rider.z += (1.5 - rider.z) * 0.05;
        return;
    }
    if rider.crash > 0 {
        // Tumbling: feathers everywhere, sinking to the line, slow.
        rider.crash -= 1;
        rider.vs = (rider.vs - 0.6).max(3.0);
        rider.s += rider.vs * DT;
        rider.x *= 0.95;
        rider.z += (-1.0 - rider.z) * 0.08;
        rider.spin += 0.6;
        if rider.crash == 0 {
            rider.wreck = Wreck::None;
        }
        return;
    }
    let goal = WING
        + if rider.burst > 0 { WING_BURST } else { 0.0 }
        - if rider.stun > 0 { WING_RUFFLED } else { 0.0 };
    rider.vs += (goal - rider.vs).clamp(-0.5, 0.4);
    rider.burst = rider.burst.saturating_sub(1);
    rider.stun = rider.stun.saturating_sub(1);
    rider.rest = rider.rest.saturating_sub(1);
    rider.reload = rider.reload.saturating_sub(1);
    // The reins: across and up, with the weight of a big bird.
    let (wx, wz) = (f32::from(input.move_x), -f32::from(input.move_y));
    rider.vx += (wx * 5.0 - rider.vx) * (REIN * DT).min(1.0) * 0.5;
    rider.vz += (wz * 4.0 - rider.vz) * (REIN * DT).min(1.0) * 0.5;
    if rolled && rider.rest == 0 {
        rider.rolling = ROLL;
        rider.rest = ROLL_REST;
        rider.vx = if wx != 0.0 { wx } else { 1.0 } * ROLL_THROW;
        noise.sounds.push("swing");
    }
    rider.rolling = rider.rolling.saturating_sub(1);
    // Held, F looses a bolt every rest.
    if input.fire && rider.reload == 0 {
        rider.reload = BOLT_REST;
        race.bolts.push(Bolt { owner: id, s: rider.s + 1.0, x: rider.x, z: rider.z, life: BOLT_LIFE });
        noise.sounds.push("shot_bolt");
    }
    let before = rider.s;
    rider.s += rider.vs * DT;
    rider.x = (rider.x + rider.vx * DT).clamp(-SKY_X, SKY_X);
    rider.z = (rider.z + rider.vz * DT).clamp(-SKY_Z, SKY_Z);
    // Rings, each once for each griffin.
    for (i, item) in track.items.iter().enumerate().take(64) {
        let bit = 1u64 << i;
        if item.kind == ItemKind::Ring
            && rider.rings & bit == 0
            && before <= item.s
            && rider.s > item.s
            && (item.x - rider.x).hypot(item.z - rider.z) < 1.3
        {
            rider.rings |= bit;
            rider.haul = rider.haul.saturating_add(1);
            rider.burst = BURST;
            noise.sounds.push("gold");
        }
    }
    // Stone and gargoyles met head-on.
    let hit_block = track.blocks.iter().any(|b| {
        rider.s + 0.5 > b.s
            && before - 0.5 < b.s + b.len
            && rider.x + 0.6 > b.x.0
            && rider.x - 0.6 < b.x.1
            && rider.z + 0.4 > b.z.0
            && rider.z - 0.4 < b.z.1
    });
    let hit_goyle = track.items.iter().enumerate().any(|(i, item)| {
        item.kind == ItemKind::Gargoyle
            && !race.taken.contains(&(i as u16))
            && before - 0.5 <= item.s
            && rider.s + 0.5 > item.s
            && (item.x - rider.x).hypot(item.z - rider.z) < 1.0
    });
    if hit_block || hit_goyle {
        // Off the stone the way it came: beside a spire, under a rampart.
        if let Some(b) = track.blocks.iter().find(|b| rider.s + 0.5 > b.s && before - 0.5 < b.s + b.len) {
            if b.x.1 - b.x.0 > 2.0 * SKY_X {
                rider.z = if b.z.0 - 0.6 > -SKY_Z { b.z.0 - 0.6 } else { (b.z.1 + 0.6).min(SKY_Z) };
            } else {
                let left = b.x.0 - 0.7;
                let right = b.x.1 + 0.7;
                rider.x = if (rider.x - left).abs() < (rider.x - right).abs() && left > -SKY_X { left } else { right.min(SKY_X) };
            }
        }
        rider.vs = 4.0;
        wreck(rider, Wreck::Tumble, noise);
    }
}

/// Bolts fly on down the gorge; a gargoyle hit shatters (a burst for the
/// shooter), a rival hit is ruffled.
fn fly_bolts(race: &mut Race3d, noise: &mut Noise) {
    let track = track(race.course);
    let mut bolts = std::mem::take(&mut race.bolts);
    bolts.retain_mut(|bolt| {
        let pace = race.riders.get(&bolt.owner).map_or(WING, |r| r.vs) + BOLT_PACE;
        let before = bolt.s;
        bolt.s += pace * DT;
        bolt.life = bolt.life.saturating_sub(1);
        for (i, item) in track.items.iter().enumerate() {
            let i = i as u16;
            if item.kind == ItemKind::Gargoyle
                && !race.taken.contains(&i)
                && before <= item.s + 0.5
                && bolt.s >= item.s - 0.5
                && (item.x - bolt.x).hypot(item.z - bolt.z) < 1.3
            {
                race.taken.insert(i);
                if let Some(shooter) = race.riders.get_mut(&bolt.owner) {
                    shooter.haul = shooter.haul.saturating_add(1);
                    shooter.burst = BURST;
                }
                noise.sounds.push("rock_land");
                return false;
            }
        }
        for (&id, rival) in race.riders.iter_mut() {
            if id != bolt.owner
                && rival.racing()
                && rival.rolling == 0
                && before <= rival.s + 0.5
                && bolt.s >= rival.s - 0.5
                && (rival.x - bolt.x).hypot(rival.z - bolt.z) < 1.1
            {
                rival.stun = RUFFLE;
                noise.sounds.push("hit_arrow");
                return false;
            }
        }
        bolt.life > 0 && bolt.s < track.len()
    });
    race.bolts = bolts;
}

#[cfg(test)]
#[path = "../../tests/rules/together_race3d__tests.rs"]
mod tests;
