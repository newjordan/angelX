//! What stands on the road: the level's fixed things (springs, pads, posts,
//! crates, the bell, where the sparks lie, where each foe is posted) and the
//! tiny pieces of live state they need. The level itself is built once in
//! `level.rs`; none of this is saved, only the session's own numbers are.
use super::math::{self, ONE};
use super::terrain::Terrain;

/// A hay-bale spring's kind: straight up, or up and forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpringKind {
    Up,
    Diag,
}

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub x: i32,
    pub y: i32,
    pub kind: SpringKind,
}

/// A boost pad lying on the road, pointing the way he runs.
#[derive(Clone, Copy, Debug)]
pub struct Pad {
    pub x: i32,
    pub y: i32,
}

/// A lantern post: where it stands and where a fallen rider is set down.
#[derive(Clone, Copy, Debug)]
pub struct Post {
    pub x: i32,
    pub y: i32,
    pub path: usize,
    pub s: i32,
    /// How far behind it the Dimming is held when it is lit, pixels, for one
    /// knight.
    pub hold: i32,
}

/// What a lantern crate holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrateKind {
    Shield,
    Sparks,
    Spur,
}

#[derive(Clone, Copy, Debug)]
pub struct Crate {
    pub x: i32,
    pub y: i32,
    pub kind: CrateKind,
}

/// What a foe is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoeKind {
    /// A snuffer walking a short beat on the road.
    Snuffer,
    /// A snuffer drifting in the air, there to be locked and chained.
    Floater,
    /// A bramble: it hurts whatever touches it and cannot be killed.
    Bramble,
    /// The gate's warden: it takes several hits, and holds the bell.
    Warden,
}

/// A foe's post: home, beat, how many seats it takes to wake it.
#[derive(Clone, Copy, Debug)]
pub struct FoeSpot {
    pub kind: FoeKind,
    pub x: i32,
    pub y: i32,
    /// The path whose surface a walker follows (a floater ignores it).
    pub path: usize,
    /// How far it ranges either side of home, pixels.
    pub range: i32,
    /// It joins the road when this many knights have sat down.
    pub min_seats: u32,
    /// Where in its beat it starts, ticks.
    pub phase: u32,
}

/// A sign on the roadside: a few plain words.
#[derive(Clone, Copy, Debug)]
pub struct Sign {
    pub x: i32,
    pub y: i32,
    pub words: &'static str,
}

/// The whole road.
#[derive(Debug, Default)]
pub struct Level {
    pub terrain: Terrain,
    /// The world's width and the line below which a body is lost, pixels.
    pub width: i32,
    pub dead_y: i32,
    /// Where each act begins: the Downhill Gate, the Wheel Arch, the Reed
    /// Fen and the Bell Gate.
    pub acts: [i32; 4],
    /// The main road's index in `terrain.paths`, and the rails.
    pub main: usize,
    /// Where a rider starts (path, distance) before the gate.
    pub start: (usize, i32),
    pub sparks: Vec<(i32, i32)>,
    pub springs: Vec<Spring>,
    pub pads: Vec<Pad>,
    pub posts: Vec<Post>,
    pub crates: Vec<Crate>,
    pub foes: Vec<FoeSpot>,
    pub signs: Vec<Sign>,
    /// The bell: its post's foot, pixels.
    pub bell: (i32, i32),
    /// The time a good run beats, ticks.
    pub par: u32,
}

impl Level {
    /// The indices of things within `reach` pixels of column `x`, for a
    /// list sorted by its x.
    pub fn near<T>(items: &[T], x: i32, reach: i32, at: impl Fn(&T) -> i32) -> std::ops::Range<usize> {
        let lo = items.partition_point(|i| at(i) < x - reach);
        let hi = items.partition_point(|i| at(i) <= x + reach);
        lo..hi.max(lo)
    }

    /// The column of the gate, pixels.
    pub fn start_x(&self) -> i32 {
        self.terrain.paths[self.start.0].sample(self.start.1).x / super::math::Q
    }

    /// Where on the road a rider is set down for post `n` (or the start).
    pub fn spawn(&self, post: Option<usize>) -> (usize, i32) {
        match post {
            Some(n) => (self.posts[n].path, self.posts[n].s),
            None => self.start,
        }
    }
}

/// How a foe stands this tick: its centre-bottom (pixels) and which way it
/// faces. A pure function of the post and the clock, so no foe position is
/// ever saved and every client agrees.
pub fn foe_at(level: &Level, spot: &FoeSpot, clock: u32) -> (i32, i32, i32) {
    let t = clock + spot.phase;
    match spot.kind {
        FoeKind::Bramble | FoeKind::Warden => (spot.x, spot.y, -1),
        FoeKind::Snuffer => {
            // A triangle wave along the beat, 0.6 px a tick.
            let beat = (spot.range * 2 * 5 / 3).max(2) as u32;
            let phase = (t % (beat * 2)) as i32;
            let b = beat as i32;
            let along = if phase < b { phase } else { 2 * b - phase };
            let x = spot.x - spot.range + along * 3 / 5;
            let facing = if phase < b { 1 } else { -1 };
            let y = level.terrain.paths[spot.path].y_at_x(x) / 256;
            (x, y, facing)
        }
        FoeKind::Floater => {
            // A slow bob and a slower drift about home.
            let bob = (i64::from(math::sin((t as i32 * 3) % 256)) * 5 / i64::from(ONE)) as i32;
            let drift = (i64::from(math::sin((t as i32 * 3 / 2) % 256)) * i64::from(spot.range) / i64::from(ONE)) as i32;
            (spot.x + drift, spot.y + bob, -1)
        }
    }
}

/// A foe's body, half-width and height in pixels, bottom-centred.
pub fn foe_body(kind: FoeKind) -> (i32, i32) {
    match kind {
        FoeKind::Snuffer => (9, 20),
        FoeKind::Floater => (9, 18),
        FoeKind::Bramble => (12, 14),
        FoeKind::Warden => (22, 52),
    }
}

/// A foe's live state: hits left, ticks of smoke since it was defeated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FoeState {
    pub hp: u8,
    pub pop: u16,
    pub flinch: u8,
}
