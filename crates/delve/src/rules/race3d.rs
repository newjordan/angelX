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
mod drift;
pub mod leg;
pub mod show;
mod oracle;
pub use drift::*;

use super::*;
use std::collections::BTreeSet;
use std::sync::OnceLock;

pub const TITLE: u32 = 3 * HZ;
pub const RACE_TIME: u32 = 90 * HZ;
pub const PODIUM: u32 = 6 * HZ;
pub const CRASH: u32 = 45;
pub const BALL_R: f32 = 0.5;
pub const HAUL_GOLD: u32 = 2;
pub const LANES: [f32; 3] = [-1.7, 0.0, 1.7];
pub const SKY_X: f32 = 4.0;
pub const SKY_Z: f32 = 2.5;
pub const BOLT_PACE: f32 = 32.0;
pub const BOLT_LIFE: u32 = 24;
pub const BOLT_REST: u32 = 10;
pub const RUFFLE: u32 = HZ;
pub const BURST: u32 = 45;
pub const ROLL: u32 = 18;
pub const ROLL_REST: u32 = 45;
pub const ROT: u32 = HZ;
pub const REGROW: u32 = 2 * HZ;
const DT: f32 = 1.0 / HZ as f32;
pub const RACE_MARK: &str = "country:race";
pub const EYRIE_ROCKS: [(i32, i32, i32, i32); 4] =
    [(1, 1, 7, 3), (16, 1, 7, 3), (10, 1, 4, 2), (2, 10, 4, 3)];
pub const PERCH: (f32, f32) = (12.0, 4.0);
pub const PERCH_REACH: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Course {
    Shaft,
    Cart,
    Griffin,
    Touge,
}

impl Course {
    pub fn name(self) -> &'static str {
        match self {
            Course::Shaft => "THE SHAFT RUN",
            Course::Cart => "THE ORE CART",
            Course::Griffin => "THE GRIFFIN RUN",
            Course::Touge => "THE TORCHLIGHT TOUGE",
        }
    }
    pub fn words(self) -> &'static str {
        match self {
            Course::Shaft => "ROLL. DON'T FALL.",
            Course::Cart => "JUMP. SWITCH.",
            Course::Griffin => "FLY. SHOOT.",
            Course::Touge => "SLIDE. RELEASE. BOOST.",
        }
    }
    pub fn keys(self) -> [(&'static str, &'static str); 2] {
        match self {
            Course::Shaft => [("F", "HOP"), ("E", "BRAKE")],
            Course::Cart => [("F", "JUMP"), ("E", "SWITCH")],
            Course::Griffin => [("F", "SHOOT"), ("E", "ROLL")],
            Course::Touge => [("SPACE", "DRIFT"), ("S", "BRAKE")],
        }
    }
    pub fn board(self) -> &'static str {
        match self {
            Course::Shaft => "BRANNOC'S ORE-SORTER",
            Course::Cart => "END OF THE LINE",
            Course::Griffin => "THE KEEP GATE",
            Course::Touge => "MOUNTAIN LAURELS",
        }
    }
    pub fn of_floor(depth: u32) -> Course {
        if depth >= 5 && depth % 4 == 1 {
            Course::Touge
        } else {
            [Course::Griffin, Course::Cart, Course::Shaft][(depth % 3) as usize]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Ground {
    Floor,
    Bridge,
    Crossing,
    Lodestone,
    Sluice,
    Collapse,
    Sorter,
    Rails,
    Trestle,
    Gorge,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Sample {
    #[serde(skip)]
    pub p: [f32; 3],
    #[serde(skip)]
    pub yaw: f32,
    pub slope: f32,
    pub bend: f32,
    pub width: f32,
    pub walls: bool,
    pub ground: Ground,
    pub lanes: u8,
    pub funnel: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ItemKind {
    Gem,
    Ring,
    Gargoyle,
    Timber,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Item {
    pub s: f32,
    pub x: f32,
    pub z: f32,
    pub kind: ItemKind,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Block {
    pub s: f32,
    pub len: f32,
    pub x: (f32, f32),
    pub z: (f32, f32),
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Gap {
    pub lane: u8,
    pub s: (f32, f32),
}

#[derive(Serialize)]
pub struct Track {
    pub looped: bool,
    pub samples: Vec<Sample>,
    pub finish: f32,
    pub items: Vec<Item>,
    pub blocks: Vec<Block>,
    pub gaps: Vec<Gap>,
    pub posts: Vec<f32>,
}

impl Track {
    pub fn len(&self) -> f32 {
        (self.samples.len() - 1) as f32
    }
    pub fn at(&self, s: f32) -> Sample {
        let s = if self.looped { s.rem_euclid(self.len()) } else { s.clamp(0.0, self.len()) };
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
    pub fn world(&self, s: f32, x: f32, z: f32) -> [f32; 3] {
        let at = self.at(s);
        let (sin, cos) = at.yaw.sin_cos();
        [at.p[0] - sin * x, at.p[1] + cos * x, at.p[2] + z]
    }
    pub fn post(&self, s: f32) -> f32 {
        self.posts.iter().copied().filter(|&p| p <= s).fold(0.0, f32::max)
    }
}

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

pub const SEAM_FUNNEL: f32 = 5.0;

fn lay(pieces: &[Piece]) -> (Vec<Sample>, Vec<f32>) {
    let mut samples = Vec::new();
    let mut posts = Vec::new();
    let (mut p, mut yaw) = ([0.0f32; 3], 0.0f32);
    for (k, pc) in pieces.iter().enumerate() {
        posts.push(samples.len() as f32);
        let len = f32::from(pc.len);
        let bend = pc.turn.to_radians() / len;
        let taper_to = pieces
            .get(k + 1)
            .filter(|&next| next.width < pc.width)
            .map_or(pc.width, |next| next.width);
        for j in 0..pc.len {
            let from_end = f32::from(pc.len - j);
            let funnel = taper_to < pc.width && from_end < SEAM_FUNNEL;
            let width = if funnel {
                taper_to + (pc.width - taper_to) * ((from_end - 1.0) / (SEAM_FUNNEL - 1.0))
            } else {
                pc.width
            };
            samples.push(Sample {
                p,
                yaw,
                slope: pc.drop / len,
                bend,
                width,
                walls: pc.walls,
                ground: pc.ground,
                lanes: pc.lanes,
                funnel,
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
    Track { looped: false, samples, finish, items, blocks: Vec::new(), gaps: Vec::new(), posts }
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
    Track { looped: false, samples, finish, items, blocks: Vec::new(), gaps, posts }
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
    Track { looped: false, samples, finish, items, blocks, gaps: Vec::new(), posts }
}

pub fn track(course: Course) -> &'static Track {
    static TRACKS: [OnceLock<Track>; 4] = [const { OnceLock::new() }; 4];
    match course {
        Course::Shaft => TRACKS[0].get_or_init(shaft),
        Course::Cart => TRACKS[1].get_or_init(cart),
        Course::Griffin => TRACKS[2].get_or_init(gorge),
        Course::Touge => TRACKS[3].get_or_init(drift::touge),
    }
}

pub fn rotten(plank: u16) -> bool {
    plank % 2 == 1
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wreck {
    #[default]
    None,
    Fall,
    Bonk,
    Buffer,
    Gap,
    Timber,
    Tumble,
    SpinOut,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rider {
    #[serde(default, skip_serializing_if = "Drift::is_default")]
    pub drift: Drift,
    pub s: f32,
    pub x: f32,
    pub z: f32,
    pub vs: f32,
    pub vx: f32,
    pub vz: f32,
    pub lane: u8,
    pub spin: f32,
    pub crash: u32,
    pub wreck: Wreck,
    pub crashes: u16,
    pub stun: u32,
    pub burst: u32,
    pub rolling: u32,
    pub rest: u32,
    pub reload: u32,
    pub haul: u16,
    pub rings: u64,
    pub finish: Option<u32>,
    pub keys: u8,
}

impl Rider {
    fn new(s: f32, x: f32, lane: u8) -> Rider {
        Rider {
            drift: Drift::default(),
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
            && self.drift.valid()
            && usize::from(self.lane) < LANES.len()
            && self.crash <= CRASH
            && self.stun <= CRASH
            && self.burst <= BURST
            && self.rolling <= ROLL
            && self.rest <= ROLL_REST
            && self.reload <= BOLT_REST
    }
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
    pub t: u32,
    pub riders: BTreeMap<u32, Rider>,
    pub ante: BTreeMap<u32, u32>,
    pub taken: BTreeSet<u16>,
    pub planks: BTreeMap<u16, u32>,
    pub bolts: Vec<Bolt>,
    pub done: bool,
    pub ended: u32,
    pub won: BTreeMap<u32, u32>,
    #[serde(default)]
    pub practice: bool,
    #[serde(default)]
    pub crossroads: bool,
    #[serde(default, skip_serializing_if = "show::Show::quiet")]
    pub show: show::Show,
}

impl Race3d {
    pub fn valid(&self) -> bool {
        let track = track(self.course);
        self.t <= TITLE + self.time_limit() + PODIUM + HZ
            && self.riders.len() <= MAX_PLAYERS as usize
            && self.ante.len() <= MAX_PLAYERS as usize
            && self.won.len() <= MAX_PLAYERS as usize
            && self.riders.keys().chain(self.ante.keys()).all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.riders.values().all(Rider::valid)
            && self.show.valid(self.t)
            && self.taken.iter().all(|&i| usize::from(i) < track.items.len())
            && self.planks.len() <= track.samples.len()
            && self.bolts.len() <= 4 * MAX_PLAYERS as usize
            && self.bolts.iter().all(|b| b.life <= BOLT_LIFE && b.s.is_finite() && b.x.is_finite() && b.z.is_finite())
    }
    pub fn titled(&self) -> bool {
        self.t < TITLE
    }
    pub fn time_limit(&self) -> u32 {
        if self.course == Course::Touge && !self.crossroads { DRIFT_RACE_TIME } else { RACE_TIME }
    }
    pub fn finish_distance(&self) -> f32 {
        track(self.course).finish * if self.course == Course::Touge { DRIFT_LAPS as f32 } else { 1.0 }
    }
    pub fn lap(&self, id: u32) -> u32 {
        self.riders.get(&id).map_or(1, |r| (r.s / track(self.course).finish) as u32 + 1).min(DRIFT_LAPS)
    }
    pub fn left(&self) -> u32 {
        (TITLE + self.time_limit()).saturating_sub(self.t).min(self.time_limit()).div_ceil(HZ)
    }
    pub fn showing(&self) -> bool {
        !self.done || self.t < self.ended + PODIUM
    }
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
    pub fn place(&self, id: u32) -> usize {
        1 + self.order().iter().position(|&r| r == id).unwrap_or(0)
    }
}

fn grid(course: Course, k: usize) -> (f32, f32, u8) {
    match course {
        Course::Shaft => (2.0 + (k / 2) as f32 * 1.4, if k % 2 == 0 { -1.0 } else { 1.0 }, 1),
        Course::Cart => {
            let (s, lane) = [(6.0, 1u8), (4.5, 0), (4.5, 2), (2.5, 1), (1.0, 0), (1.0, 2)][k % 6];
            (s, LANES[usize::from(lane)], lane)
        }
        Course::Touge => (0.0, (k as f32 - 1.5) * 1.6, 1),
        Course::Griffin => (2.0 + (k / 2) as f32 * 2.0, if k % 2 == 0 { -1.6 } else { 1.6 }, 1),
    }
}

#[derive(Default)]
struct Noise {
    sounds: Vec<&'static str>,
}

fn ore_cart_on() -> bool {
    cfg!(test) || std::env::var_os("ANGEL_DELVE_ORE_CART").is_some_and(|v| v == "1")
}

impl Run {
    pub(super) fn pick_race3d(&mut self) {
        self.race3d = None;
        self.race3d_room = None;
        if self.dungeon.depth == 0 || !ore_cart_on() {
            return;
        }
        self.pick_race3d_hall();
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn race3d_hall_for_test(&mut self) {
        if self.dungeon.depth > 0 && self.race3d_room.is_none() {
            self.pick_race3d_hall();
        }
    }

    fn pick_race3d_hall(&mut self) {
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
                    && !self.hosts_game(i)
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

    pub(super) fn start_race3d(&mut self, index: usize) {
        self.start_race3d_course(index, Course::of_floor(self.dungeon.depth));
    }

    pub(super) fn start_race3d_course(&mut self, index: usize, course: Course) {
        let (ante, scores) = if let Some(c) = &self.crossroads {
            (BTreeMap::new(), c.seats.iter().map(|&id| (id, 0u32)).collect())
        } else {
            folly::take_antes(&mut self.players)
        };
        self.open_race3d(index, course, ante, scores, self.crossroads.is_some());
    }

    pub(super) fn open_race3d(
        &mut self,
        index: usize,
        course: Course,
        ante: BTreeMap<u32, u32>,
        scores: BTreeMap<u32, u32>,
        short: bool,
    ) {
        let riders = scores
            .keys()
            .enumerate()
            .map(|(k, &id)| {
                let (s, mut x, lane) = grid(course, k);
                if course == Course::Touge && short {
                    x = (k as f32 - (scores.len() as f32 - 1.0) / 2.0) * 1.6;
                }
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
            crossroads: short,
            show: show::Show::default(),
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    pub fn race_won(&self) -> bool {
        self.home.countryside.race_won || self.marks.contains_key(RACE_MARK)
    }

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
            crossroads: false,
            show: show::Show::default(),
        });
        self.sounds.push("swing");
        self.cues.push("folly".into());
    }

    pub fn race3d_showing(&self) -> bool {
        self.race3d.as_ref().is_some_and(|r| r.room == self.at && r.showing())
    }

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
        let standing: Vec<u32> = self
            .players
            .iter()
            .filter(|(id, h)| self.crossroads.as_ref().map_or(h.hp > 0 && !h.stone, |c| c.seats.contains(id)))
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
        let show = std::mem::take(&mut race.show);
        let before = race.clone();
        if race.course == Course::Shaft {
            if let Ok(ran) = leg::run(&mut race, inputs, &standing) {
                self.sounds.extend(ran.sounds);
                if ran.all_home || ran.timed_out {
                    self.finish_race3d(&mut race);
                }
            }
            self.keep_show(race, show, &before);
            return;
        }
        if matches!(race.course, Course::Cart | Course::Griffin) {
            let (all_home, timed_out, sounds) = oracle::tick(&mut race, inputs, &standing);
            self.sounds.extend(sounds);
            if all_home || timed_out {
                self.finish_race3d(&mut race);
            }
            self.keep_show(race, show, &before);
            return;
        }
        let mut noise = Noise::default();
        let ids: Vec<u32> = race.riders.keys().copied().collect();
        for &id in &ids {
            if !standing.contains(&id) { continue; }
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid() && standing.contains(&id))
                .unwrap_or_default();
            let mut rider = race.riders[&id].clone();
            let shift = input.fire && rider.keys & 1 == 0;
            rider.keys = u8::from(input.fire) | u8::from(input.interact) << 1;
            drift::drive(&race, &mut rider, input, shift, &mut noise);
            if rider.finish.is_none() && rider.crash == 0 && rider.s >= race.finish_distance() {
                rider.finish = Some(race.t);
                noise.sounds.push("wheel_land");
            }
            race.riders.insert(id, rider);
        }
        self.sounds.extend(noise.sounds);
        let all_home = !race.riders.is_empty() && race.riders.values().all(|r| r.finish.is_some());
        if all_home || race.t >= TITLE + race.time_limit() {
            self.finish_race3d(&mut race);
        }
        self.keep_show(race, show, &before);
    }

    fn keep_show(&mut self, mut race: Race3d, mut show: show::Show, before: &Race3d) {
        show.observe(before, &race);
        let held = show.held(race.t);
        race.show = if show.valid(race.t) { show } else { show::Show::default() };
        self.race3d = Some(race);
        if let Some(weight) = held {
            self.hold_for(weight);
            self.finish_hit_hold();
        }
    }

    fn finish_race3d(&mut self, race: &mut Race3d) {
        race.done = true;
        race.ended = race.t;
        let order = race.order();
        let best = order.first().copied();
        if best.and_then(|id| race.riders.get(&id)).is_some_and(|r| r.finish.is_some()) {
            self.marks.insert(RACE_MARK.into(), 1);
        }
        if race.practice {
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

pub const RUN_UP: f32 = 6.0;

pub const TIMBER_H: f32 = 0.6;

pub fn laid(track: &Track, s: f32, lane: u8) -> bool {
    track.at(s).lanes & (1 << lane) != 0
}

pub fn broken(track: &Track, s: f32, lane: u8) -> bool {
    track.gaps.iter().any(|g| g.lane == lane && s >= g.s.0 && s < g.s.1)
}

#[cfg(test)]
fn install_oracle() {
    crate::scripted::install_for_test(leg::LEG, oracle::answer);
}

#[cfg(test)]
#[path = "../../tests/rules/together_race3d__tests.rs"]
pub(crate) mod tests;
