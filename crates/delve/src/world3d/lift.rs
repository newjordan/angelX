//! The lift: a lit room changing dimension, in three moves
//! (docs/REALM-MONOLITH.md, "Rooms, lit").
//!
//! The game starts as pixel and ends in 3D, and when something upgrades the
//! player should see it change dimension. A lit room already holds both
//! states: with every mass a card, the ground flat, the sun straight overhead
//! (where nothing throws a shadow), the lamps out and the floor not tipped it
//! is a flat plan in the room's ramp; with nothing scaled it is the room the
//! window draws. The lift is the way between: three transitions, each a pure
//! function of time and, where it has one, of the place of what moves. Every
//! number is fixed (seconds, pixels, radians); at 0 each is exactly the flat
//! state and at its own `DURATION` exactly the lit one (and [`apply`] then
//! leaves a frame exactly as it found it). To play one backwards is to ask for
//! `DURATION - t`: the flames go out first, the stones sink after a small
//! lift (the lock, run the other way), the sun climbs back overhead, the room
//! tips up a little further before it lets down to flat, and every frame
//! between is as good as one going forward, being the same frame.
//!
//! - **A prop lifts** ([`prop`]). Every mass is first a card on the floor: its
//!   footprint and its top's tone, [`CARD`] thick, on the dais where it
//!   stands on one. From the ripple's source outward each stone rises, a low
//!   one quickly and a wall slowly, on a cubic that slows into its stop, goes
//!   past it by a tenth of its height and settles: the lock of
//!   `glass::transition`'s walls, the thunk where it lands. What stands
//!   together rises together (a pillar's plinth, shaft and crown are one
//!   column: parts at their own heights fought each other at the card's
//!   thickness), and its shadow, the sun being where it is, grows with it.
//!   Last of all, when every mass in the room has seated, its flames catch,
//!   in a second ripple: a spark that is a flat orange mark on the card, a
//!   tongue that flares past its size and settles, and a beat behind it the
//!   lamp's pool, which blooms outward past its reach and draws back.
//! - **The floor rises** ([`floor`], [`pool`]). The ground's relief grows from
//!   flat on the same cubic and lock (a pit deepens, a dais lifts) while the
//!   key light comes on: the sun swings down from straight overhead to its
//!   own place at the pace that makes every shadow's length grow evenly from
//!   nothing (the angle alone would hold them back and rush them at the
//!   end); its strength follows it down so the floor facing it keeps the
//!   tone it began with, and only what stands (a wall's face, a pillar's) is
//!   lit as it comes, from [`FLAT_KEY`] of the light to all of it. The lamps'
//!   pools bloom when the sun is nearly down, door to heart.
//! - **The room tips** ([`tip`]). The stage's tilt sweeps from flat to the
//!   game's on a slow-in curve, like a slab lifted at one edge, lands a fifth
//!   past it and settles, and the lamps catch at the landing: a flare that
//!   fades. The tilt is the engine's: at 0.3 it is a 7% squash and a vanishing
//!   point, and the far wall comes into view as it goes, which is most of what
//!   the eye sees.
//!
//! [`climb`] plays the three as one: the floor and sun first, the props from
//! a third of a second in, the tip so it lands as the last flame catches.
//!
//! **Where the ripples start.** From the doors ([`Ripple::doors`]): the
//! Delve's light comes in by them (`rooms_lit`'s lanes), so the walls at the
//! entrances stand first and the room closes in on its heart. From the middle
//! ([`Ripple::centre`], the way tiles fall in `glass::transition`) the stones
//! at the rim, the walls, come last and the room reads as a hole filling.
//! Doors read better; the example takes either.
//!
//! **Rows and dither.** The look's internal rows would let the pixel art come
//! in from chunky to fine, but the GPU pins a lit world's rows to its stage's
//! own pixel grain (`gpu/src/stage.rs`: `look.pixel_rows = stage_size.1`), so
//! they are not a knob of the look and the lift leaves them. Emulated on the
//! proof's frames (the world drawn 4, 3, 2 then 1 stage pixels to a pixel)
//! the climb of resolution helps, when it goes in whole steps: a flat plan in
//! fat dithered blocks resolving into the room is the idea made visible. Run
//! continuously it hurts: the dither's grid slides over the picture and
//! crawls. It would take `look.pixel_rows` honoured below the stage's rows
//! in that line of the GPU, and [`apply`] stepping it. The dither itself is
//! the room's own and one mode of it: swapping it mid-lift would only pop.
//!
//! **What it moves.** [`apply`] edits a frame the stage maker made, in
//! place: the frame's tilt, and in its lit world the ground node's height
//! scale (the mesh is never re-sent: the node's own scale scales every
//! height, and the GPU's cofactor normals follow), each mass's height about
//! the ground it stands on, each flame's size about its foot, and the look's
//! sun and lamps. A page that makes a lit room's blocks itself from the set's
//! text (`SetRef`) has to apply the same to what it makes.

use std::f32::consts::PI;

use angel_stage::Frame;
use angel_stage::look::Sun;
use angel_stage::scene::{MeshData, MeshRef, Node, World};

// ------------------------------------------------------------ the props ----

/// A mass as a card, in the room's pixels: as thick as the dais the ground
/// lifts under what stands (`rooms_lit`'s `DAIS`, 3) and a half pixel more,
/// so a rising floor never buries one. A mass lower than this does not lift.
pub const CARD: f32 = 3.5;
/// A mass this tall (the room's pixels) is as heavy as a wall: the slowest
/// to climb.
const TALL: f32 = 48.0;

/// The first mass starts at 0; the farthest from the ripple's source
/// `PROP_SPREAD` later, each early or late by up to `PROP_JITTER` by its spot.
const PROP_SPREAD: f32 = 0.45;
const PROP_JITTER: f32 = 0.06;
/// How long a low stone and a wall take to climb to their height.
const RISE_LOW: f32 = 0.30;
const RISE_TALL: f32 = 0.45;
/// The lock: past its height by this much of it, and back, in this long.
const LOCK_OVER: f32 = 0.10;
const LOCK_TIME: f32 = 0.14;

/// Flames catch after every mass has seated, and a beat more.
const IGNITE_GAP: f32 = 0.03;
const IGNITE_AT: f32 = PROP_SPREAD + PROP_JITTER + RISE_TALL + LOCK_TIME + IGNITE_GAP;
/// The first flame catches at `IGNITE_AT`, the farthest `IGNITE_SPREAD`
/// later, each by its spot up to `IGNITE_JITTER` off.
const IGNITE_SPREAD: f32 = 0.30;
const IGNITE_JITTER: f32 = 0.05;
/// A flame takes `IGNITE_TIME` to catch: it grows from a spark over the first
/// `IGNITE_RISE` of it, flares past its size by `FLARE` of it over the rest,
/// and settles.
const IGNITE_TIME: f32 = 0.40;
const IGNITE_RISE: f32 = 0.55;
const FLARE: f32 = 0.50;
/// A spark: this much of a flame's size, and of its width.
const SPARK: f32 = 0.05;
const SPARK_WIDTH: f32 = 0.55;
/// A lamp's pool follows its flame by `POOL_LAG` and blooms in `BLOOM_TIME`,
/// as a flame catches: it comes up from `POOL_FROM` of its reach over the
/// first `BLOOM_RISE` of it, its light then flares by `BLOOM_FLARE` of itself
/// and its reach by `BLOOM_REACH`, and both draw back.
const POOL_LAG: f32 = 0.08;
const BLOOM_TIME: f32 = 0.50;
const BLOOM_RISE: f32 = 0.55;
const BLOOM_FLARE: f32 = 0.35;
const BLOOM_REACH: f32 = 0.20;
const POOL_FROM: f32 = 0.30;

/// The prop lift's whole length, in seconds.
pub const PROP_DURATION: f32 = IGNITE_AT + IGNITE_SPREAD + IGNITE_JITTER + POOL_LAG + BLOOM_TIME;

// ----------------------------------------------------------- the floor ----

/// The ground's relief is full after `RELIEF_TIME`, past it by `RELIEF_OVER`
/// of itself, settled `RELIEF_SETTLE` later.
const RELIEF_TIME: f32 = 0.70;
const RELIEF_OVER: f32 = 0.10;
const RELIEF_SETTLE: f32 = 0.20;
/// The sun starts down at `SUN_AT` and is in its place `SUN_TIME` later.
const SUN_AT: f32 = 0.20;
const SUN_TIME: f32 = 1.00;
/// What of the key light's strength is on while the sun is overhead.
pub const FLAT_KEY: f32 = 0.78;
/// The lamps' pools bloom from `POOL_AT`, the farthest from the ripple's
/// source `POOL_SPREAD` later, each in `BLOOM_TIME`.
const POOL_AT: f32 = 1.00;
const POOL_SPREAD: f32 = 0.30;

/// The floor's whole length.
pub const FLOOR_DURATION: f32 = POOL_AT + POOL_SPREAD + BLOOM_TIME;

// ------------------------------------------------------------- the tip ----

/// The tilt reaches the game's after `TIP_TIME`, past it by `TIP_OVER` of it,
/// settled `TIP_SETTLE` later.
const TIP_TIME: f32 = 0.90;
const TIP_OVER: f32 = 0.20;
const TIP_SETTLE: f32 = 0.45;
/// The lamps flare by `CATCH` of themselves at the landing, and fade as the
/// room settles.
const CATCH: f32 = 0.30;

/// The tip's whole length.
pub const TIP_DURATION: f32 = TIP_TIME + TIP_SETTLE;

// ------------------------------------------------------------ the climb ----

/// In the whole climb the floor starts at 0, the props `PROP_AT` in, and the
/// tip when it will land as the last flame catches.
const PROP_AT: f32 = 0.35;
const TIP_AT: f32 = PROP_AT + IGNITE_AT + IGNITE_SPREAD - TIP_TIME;

/// The whole climb's length: the last of the three to end.
pub const DURATION: f32 = max3(FLOOR_DURATION, PROP_AT + PROP_DURATION, TIP_AT + TIP_DURATION);

const fn max3(a: f32, b: f32, c: f32) -> f32 {
    let ab = if a > b { a } else { b };
    if ab > c { ab } else { c }
}

// ------------------------------------------------------------- motions ----

/// A lamp's pool: how strong it burns and how far it reaches, each of its
/// own (1 is as the room lights it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pool {
    pub strength: f32,
    pub reach: f32,
}

impl Pool {
    /// Out.
    pub const OUT: Self = Self { strength: 0.0, reach: POOL_FROM };
    /// As the room lights it.
    pub const LIT: Self = Self { strength: 1.0, reach: 1.0 };
}

/// Where something stands in its room, for its motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// 0 at the ripple's source, 1 at the farthest the room goes from it.
    pub order: f32,
    /// A steady number in 0..=1 for the spot, smooth across the floor so
    /// what stands together moves together ([`seed`]).
    pub seed: f32,
    /// How tall it stands when it is up, in the room's pixels.
    pub tall: f32,
}

/// How a prop stands at one moment of its lift: a mass's height, its flame
/// and its lamp's pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    /// How far up the mass is, from a card ([`CARD`]) to its height: 0 a
    /// card, 1 up, past it on the lock.
    pub height: f32,
    /// Of the flame's own size: [`SPARK`] before it catches, past 1 as it
    /// flares.
    pub flame: f32,
    pub lamp: Pool,
}

impl Prop {
    /// A card with a spark on it and its lamp out.
    pub const CARD: Self = Self { height: 0.0, flame: SPARK, lamp: Pool::OUT };
    /// Up, burning.
    pub const SOLID: Self = Self { height: 1.0, flame: 1.0, lamp: Pool::LIT };
}

/// How the floor stands at one moment of its rise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Floor {
    /// Of the ground's own heights: 0 flat, 1 as it is, past it on the lock.
    pub relief: f32,
    /// How far the sun has come down: 0 straight overhead, 1 in its place.
    pub sun: f32,
    /// Of the key light's strength at the sun's place: [`FLAT_KEY`] to 1.
    pub key: f32,
}

impl Floor {
    pub const FLAT: Self = Self { relief: 0.0, sun: 0.0, key: FLAT_KEY };
    pub const LIT: Self = Self { relief: 1.0, sun: 1.0, key: 1.0 };
}

/// How the room is tipped at one moment of its tip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tip {
    /// Of the frame's own tilt: 0 flat, 1 as the game has it, past it on the
    /// landing.
    pub tilt: f32,
    /// Of every lamp's strength: 1, flaring at the landing.
    pub catch: f32,
}

impl Tip {
    pub const FLAT: Self = Self { tilt: 0.0, catch: 1.0 };
    pub const TIPPED: Self = Self { tilt: 1.0, catch: 1.0 };
}

/// A prop's motion at time `t`: a card at 0 or before its turn, up and lit
/// from `PROP_DURATION`. A time that is not a number is 0.
pub fn prop(t: f32, place: Place) -> Prop {
    if t.is_nan() || t <= 0.0 {
        return Prop::CARD;
    }
    if t >= PROP_DURATION {
        return Prop::SOLID;
    }
    let order = place.order.clamp(0.0, 1.0);
    let seed = place.seed.clamp(0.0, 1.0);
    // The mass: it climbs, then locks.
    let weight = (place.tall / TALL).clamp(0.0, 1.0);
    let rise = RISE_LOW + (RISE_TALL - RISE_LOW) * weight;
    let start = PROP_SPREAD * order + PROP_JITTER * seed;
    let up = ease_out_cubic((t - start) / rise);
    let height = up + LOCK_OVER * lock((t - start - rise) / LOCK_TIME);
    // The flame, then the pool under it.
    let catch = IGNITE_AT + IGNITE_SPREAD * order + IGNITE_JITTER * seed;
    let k = (t - catch) / IGNITE_TIME;
    let flame = SPARK + (1.0 - SPARK) * ease_out_cubic(k / IGNITE_RISE) + FLARE * lock((k - IGNITE_RISE) / (1.0 - IGNITE_RISE));
    Prop { height, flame, lamp: bloom((t - catch - POOL_LAG) / BLOOM_TIME) }
}

/// A pool at `k` of its bloom (0 out, 1 lit): its strength comes up and
/// flares, its reach opens past and draws back.
fn bloom(k: f32) -> Pool {
    let (up, flare) = (ease_out_cubic(k / BLOOM_RISE), lock((k - BLOOM_RISE) / (1.0 - BLOOM_RISE)));
    Pool { strength: up + BLOOM_FLARE * flare, reach: POOL_FROM + (1.0 - POOL_FROM) * up + BLOOM_REACH * flare }
}

/// The floor at time `t`: flat with the sun overhead at 0 (or when `t` is
/// not a number), as the room has it from `FLOOR_DURATION`.
pub fn floor(t: f32) -> Floor {
    if t.is_nan() || t <= 0.0 {
        return Floor::FLAT;
    }
    if t >= FLOOR_DURATION {
        return Floor::LIT;
    }
    let relief = ease_out_cubic(t / RELIEF_TIME) + RELIEF_OVER * lock((t - RELIEF_TIME) / RELIEF_SETTLE);
    let sun = smooth((t - SUN_AT) / SUN_TIME);
    Floor { relief, sun, key: FLAT_KEY + (1.0 - FLAT_KEY) * sun }
}

/// The lamp's pool of a floor at time `t`, at `place`: out until its turn,
/// then it blooms.
pub fn pool(t: f32, place: Place) -> Pool {
    if t.is_nan() || t <= 0.0 {
        return Pool::OUT;
    }
    if t >= FLOOR_DURATION {
        return Pool::LIT;
    }
    bloom((t - POOL_AT - POOL_SPREAD * place.order.clamp(0.0, 1.0)) / BLOOM_TIME)
}

/// The tip at time `t`: flat at 0 (or when `t` is not a number), tipped as
/// the game has it from `TIP_DURATION`.
pub fn tip(t: f32) -> Tip {
    if t.is_nan() || t <= 0.0 {
        return Tip::FLAT;
    }
    if t >= TIP_DURATION {
        return Tip::TIPPED;
    }
    Tip {
        tilt: ease_in_out_cubic(t / TIP_TIME) + TIP_OVER * lock((t - TIP_TIME) / TIP_SETTLE),
        catch: 1.0 + CATCH * lock((t - TIP_TIME) / TIP_SETTLE),
    }
}

/// How far into each of the three a moment is, in seconds: before 0 is flat,
/// from the move's own length is done.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Times {
    pub floor: f32,
    pub prop: f32,
    pub tip: f32,
}

impl Times {
    /// All three done: the room as it is.
    pub const DONE: Self = Self { floor: FLOOR_DURATION, prop: PROP_DURATION, tip: TIP_DURATION };
    /// None begun: the flat plan.
    pub const FLAT: Self = Self { floor: 0.0, prop: 0.0, tip: 0.0 };
}

/// The whole climb at time `t`: each of the three as its start says.
pub fn climb(t: f32) -> Times {
    Times { floor: t, prop: t - PROP_AT, tip: t - TIP_AT }
}

// ------------------------------------------------------------- the room ----

/// Where the ripples start: points of the room (its own pixels) the lift
/// spreads from, and how far it is to the farthest thing from them.
#[derive(Clone, Debug, PartialEq)]
pub struct Ripple {
    sources: Vec<[f32; 2]>,
    reach: f32,
}

impl Ripple {
    /// From the middle of a room `w` by `h` pixels, out to its corners.
    pub fn centre(w: f32, h: f32) -> Self {
        Self { sources: vec![[w / 2.0, h / 2.0]], reach: (w / 2.0).hypot(h / 2.0) }
    }

    /// From the doors of a room `w` by `h` pixels (a room with none, its
    /// middle), to the corner farthest from all of them.
    pub fn doors(doors: &[[f32; 2]], w: f32, h: f32) -> Self {
        if doors.is_empty() {
            return Self::centre(w, h);
        }
        let mut ripple = Self { sources: doors.to_vec(), reach: 1.0 };
        ripple.reach = [[0.0, 0.0], [w, 0.0], [0.0, h], [w, h]].iter().map(|c| ripple.distance(c[0], c[1])).fold(1.0, f32::max);
        ripple
    }

    fn distance(&self, x: f32, z: f32) -> f32 {
        self.sources.iter().map(|s| (s[0] - x).hypot(s[1] - z)).fold(f32::INFINITY, f32::min)
    }

    /// How far `(x, z)` is along the ripple: 0 at a source, 1 at the end.
    pub fn order(&self, x: f32, z: f32) -> f32 {
        let d = self.distance(x, z) / self.reach.max(1.0);
        if d.is_finite() { d.clamp(0.0, 1.0) } else { 0.0 }
    }

    /// The place of a thing `tall` high at `(x, z)`.
    pub fn place(&self, x: f32, z: f32, tall: f32) -> Place {
        Place { order: self.order(x, z), seed: seed(x, z), tall }
    }
}

/// A steady number in 0..=1 for a spot of the room, smooth across it: two
/// slow waves, so neighbours are alike and the room is not a wave.
pub fn seed(x: f32, z: f32) -> f32 {
    let v = 0.5 + 0.30 * (x * 0.07 + 1.3).sin() + 0.20 * (z * 0.11 + x * 0.03 + 4.1).sin();
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.5 }
}

/// The least the ground's height scale is drawn at: a node scaled to nothing
/// has no normals.
const MIN_RELIEF: f32 = 1e-3;
/// What lies in the ground (water, lava, a bed: its top no higher than this,
/// in the room's pixels) goes down with it, and when the ground is flat lies
/// this far over it: a film, the pond on the plan.
const SUNK: f32 = 0.5;
const FILM: f32 = 0.25;
/// How many masses deep one standing on another is followed.
const STAND_DEPTH: usize = 6;

/// What a frame's flames are: a lit room's flame nodes have ids of this tag
/// (`id >> 32`, `rooms_lit::flame_nodes`).
const FLAME_TAG: u64 = 0xF1A;

fn is_flame(node: &Node) -> bool {
    node.id >> 32 == FLAME_TAG
}

/// Whether a node is a lit room's ground (an uploaded mesh).
fn is_ground(node: &Node) -> bool {
    matches!(node.mesh, Some(MeshRef::Uploaded(_)))
}

/// A frame of a lit room as `times` have it: its tilt, and in its lit world
/// the ground's relief, every mass's height, each flame, the sun and the
/// lamps. A frame whose three are done is left exactly as it is.
pub fn apply(frame: &mut Frame, times: Times, ripple: &Ripple) {
    if times.tip < TIP_DURATION {
        frame.tilt *= tip(times.tip).tilt;
    }
    if let Some(world) = frame.props.as_mut().and_then(|p| p.world.as_mut()) {
        apply_world(world, times, ripple);
    }
}

/// [`apply`] to a lit world alone (its floor, blocks, flames and look).
pub fn apply_world(world: &mut World, times: Times, ripple: &Ripple) {
    // Where the room's pixels are on this stage: the ground's own place.
    let (origin, scale) = world
        .floor
        .iter()
        .find(|n| is_ground(n))
        .map_or(([0.0, 0.0], 1.0), |g| ([g.transform.translation[0], g.transform.translation[2]], g.transform.scale[0].max(1e-6)));
    let room = |x: f32, z: f32| ((x - origin[0]) / scale, (z - origin[1]) / scale);
    // What lies in the ground, before it moves: the water and the lava.
    let sunk: Vec<bool> = world.extra.iter().map(|n| !is_flame(n) && n.transform.translation[1] + n.transform.scale[1] * 0.5 <= SUNK * scale).collect();

    if times.floor < FLOOR_DURATION {
        let rising = floor(times.floor);
        let relief = rising.relief.max(MIN_RELIEF);
        for node in world.floor.iter_mut().filter(|n| is_ground(n)) {
            node.transform.scale[1] *= relief;
        }
        for (node, &lies) in world.extra.iter_mut().zip(&sunk) {
            if lies {
                let t = &mut node.transform;
                t.translation[1] = t.translation[1] * relief + FILM * scale * (1.0 - relief.min(1.0));
                t.scale[1] *= relief;
            }
        }
        if let Some(sun) = world.look.as_mut().and_then(|l| l.sun.as_mut()) {
            lower(sun, rising.sun, rising.key);
        }
    }
    if times.prop < PROP_DURATION {
        // The ground the masses stand on, as the stage has it now: where its
        // surface is under a point (what stands on a dais stands on it as it
        // rises, and is a card on it).
        let stretch = world.floor.iter().find(|n| is_ground(n)).map_or(1.0, |g| g.transform.scale[1]);
        let field = world
            .floor
            .iter()
            .find_map(|n| match n.mesh {
                Some(MeshRef::Uploaded(id)) => world.meshes.iter().find(|(m, _)| *m == id),
                _ => None,
            })
            .and_then(|(_, mesh)| Field::of(mesh));
        let ground = |x: f32, z: f32| {
            let (rx, rz) = room(x, z);
            field.as_ref().map_or(0.0, |f| f.height(rx, rz) * stretch)
        };
        // What stands together rises together: a mass takes the height and
        // the place of the tallest thing standing at its middle.
        let columns: Vec<Column> = world.blocks.iter().map(Column::of).collect();
        let stand = |x: f32, z: f32| -> (f32, [f32; 2]) {
            let tallest = |at: [f32; 2]| columns.iter().filter(|c| c.holds(at[0], at[1])).max_by(|a, b| a.top.total_cmp(&b.top));
            let Some(mut column) = tallest([x, z]) else {
                return (0.0, [x, z]);
            };
            // And what stands at its middle may be taller still (a plinth
            // inside a wall): so a flame and the mass it burns on, and a
            // mass and what holds it, always agree.
            for _ in 0..STAND_DEPTH {
                match tallest(column.at) {
                    Some(next) if next.top > column.top => column = next,
                    _ => break,
                }
            }
            (column.top, column.at)
        };
        // How much of its height a mass has at `x`, `z`: from a card's
        // thickness over the ground there to the whole of it (exactly 1).
        let mass = |x: f32, z: f32, now: f32| -> (f32, f32) {
            let (top, at) = stand(x, z);
            let base = ground(x, z);
            let (rx, rz) = room(at[0], at[1]);
            let up = prop(now, ripple.place(rx, rz, top / scale)).height;
            let card = (CARD * scale / (top - base).max(1e-6)).min(1.0);
            (1.0 + (up - 1.0) * (1.0 - card), base)
        };
        for node in world.blocks.iter_mut() {
            let t = &mut node.transform;
            let (m, base) = mass(t.translation[0], t.translation[2], times.prop);
            if m != 1.0 {
                t.translation[1] = base + (t.translation[1] - base) * m;
                t.scale[1] *= m;
            }
        }
        // The flames grow on their masses, and whatever else burns or lies on
        // one (a bed of coals) rides it.
        for (node, &lies) in world.extra.iter_mut().zip(&sunk) {
            if lies {
                continue;
            }
            let (x, z) = (node.transform.translation[0], node.transform.translation[2]);
            let (rx, rz) = room(x, z);
            let size = if is_flame(node) { prop(times.prop, ripple.place(rx, rz, 0.0)).flame } else { 1.0 };
            let (m, base) = mass(x, z, times.prop);
            if m != 1.0 || size != 1.0 {
                grow(node, (m, base), size);
            }
        }
    }
    let catch = if times.tip < TIP_DURATION { tip(times.tip).catch } else { 1.0 };
    let (lit_floor, lit_props) = (times.floor >= FLOOR_DURATION, times.prop >= PROP_DURATION);
    if let Some(look) = world.look.as_mut()
        && !(lit_floor && lit_props && catch == 1.0)
    {
        for lamp in look.lamps.iter_mut() {
            let (x, z) = room(lamp.at[0], lamp.at[2]);
            let place = ripple.place(x, z, 0.0);
            let (a, b) = (prop(times.prop, place).lamp, pool(times.floor, place));
            lamp.strength *= a.strength * b.strength * catch;
            lamp.reach *= a.reach * b.reach;
        }
    }
}

/// What a mass covers on the floor (its footprint, turned about the vertical
/// as the shape is), and how high it stands (stage pixels).
struct Column {
    at: [f32; 2],
    half: [f32; 2],
    /// The sine and cosine of its turn.
    turn: [f32; 2],
    top: f32,
}

impl Column {
    fn of(node: &Node) -> Self {
        let t = &node.transform;
        let turn = 2.0 * t.rotation[1].atan2(t.rotation[3]);
        Self {
            at: [t.translation[0], t.translation[2]],
            half: [t.scale[0] * 0.5, t.scale[2] * 0.5],
            turn: [turn.sin(), turn.cos()],
            top: t.translation[1] + t.scale[1] * 0.5,
        }
    }

    /// Whether floor point `(x, z)` is under it.
    fn holds(&self, x: f32, z: f32) -> bool {
        let (dx, dz) = (x - self.at[0], z - self.at[1]);
        let (along, across) = (dx * self.turn[1] - dz * self.turn[0], dx * self.turn[0] + dz * self.turn[1]);
        along.abs() <= self.half[0] && across.abs() <= self.half[1]
    }
}

/// A flame as it rides its mass up (`mass` of its height over `base`, the
/// ground it stands on) and grows (`size` of its own, about its foot).
fn grow(node: &mut Node, (mass, base): (f32, f32), size: f32) {
    let t = &mut node.transform;
    // Where its foot is: a built-in mesh's low corner, else its middle.
    let low = node.mesh.and_then(|m| m.generate()).and_then(|d| d.positions.iter().map(|p| p[1]).reduce(f32::min)).unwrap_or(-0.5);
    let foot = t.translation[1] + t.scale[1] * low;
    let width = if size >= 1.0 { 1.0 } else { SPARK_WIDTH + (1.0 - SPARK_WIDTH) * size.max(0.0) };
    t.scale = [t.scale[0] * width, t.scale[1] * size, t.scale[2] * width];
    t.translation[1] = base + (foot - base) * mass - t.scale[1] * low;
}

/// The ground's heights as its uploaded mesh holds them: corners `step`
/// apart, a row of `cols` of them to a row, the first at `at` (the room's
/// pixels), each its height over the floor (`Ground::mesh`).
struct Field<'a> {
    corners: &'a [[f32; 3]],
    at: [f32; 2],
    step: f32,
    cols: usize,
}

impl<'a> Field<'a> {
    fn of(mesh: &'a MeshData) -> Option<Self> {
        let first = *mesh.positions.first()?;
        let cols = mesh.positions.iter().take_while(|p| p[2] == first[2]).count();
        let step = mesh.positions.get(1)?[0] - first[0];
        let field = Self { corners: &mesh.positions, at: [first[0], first[2]], step, cols };
        (cols >= 2 && step > 0.0 && step.is_finite() && mesh.positions.len() % cols == 0 && mesh.positions.len() >= cols * 2).then_some(field)
    }

    /// The height at floor point `(x, z)`, between the corners round it.
    fn height(&self, x: f32, z: f32) -> f32 {
        let rows = self.corners.len() / self.cols;
        let fx = ((x - self.at[0]) / self.step).clamp(0.0, (self.cols - 1) as f32);
        let fz = ((z - self.at[1]) / self.step).clamp(0.0, (rows - 1) as f32);
        let (i, j) = ((fx.floor() as usize).min(self.cols - 2), (fz.floor() as usize).min(rows - 2));
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let at = |i: usize, j: usize| self.corners[j * self.cols + i][1];
        let (a, b) = (at(i, j) + (at(i + 1, j) - at(i, j)) * tx, at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * tx);
        a + (b - a) * tz
    }
}

/// The lowest the sun is taken to be, in radians over the floor.
const MIN_SUN: f32 = 0.05;

/// The sun `progress` of the way down from overhead (1: where it is), `key`
/// of its light there. The way it goes is its shadows' way: their length
/// (their casters' height times the cotangent of its height) is `progress`
/// of what it is where the sun stands, so they grow from nothing at an even
/// pace, not in a rush at the end. Its strength follows it so that a face
/// square to the ground keeps its brightness while the angle changes.
fn lower(sun: &mut Sun, progress: f32, key: f32) {
    let toward = sun.toward_unit();
    let around = toward[0].hypot(toward[2]);
    if around < 1e-6 {
        return;
    }
    let end = toward[1].atan2(around).max(MIN_SUN);
    let height = 1.0f32.atan2(end.cos() / end.sin() * progress.clamp(0.0, 1.0));
    sun.toward = if progress > 0.0 {
        [toward[0] / around * height.cos(), height.sin(), toward[2] / around * height.cos()]
    } else {
        [0.0, 1.0, 0.0]
    };
    sun.strength *= end.sin() / height.sin() * key;
}

// -------------------------------------------------------------- curves ----

/// 0 up to `x` = 0, 1 from 1, a cubic that slows into its stop between.
fn ease_out_cubic(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}

fn ease_in_out_cubic(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The lock over `k` in 0..1 (0 outside it): up from 0, past, and back, as
/// `glass::transition`'s walls lock.
fn lock(k: f32) -> f32 {
    if k > 0.0 && k < 1.0 { (PI * k).sin() * (1.0 - k) } else { 0.0 }
}

#[cfg(test)]
#[path = "../../tests/world3d/lift__tests.rs"]
mod tests;
