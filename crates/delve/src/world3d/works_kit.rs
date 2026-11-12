//! The Works' buildings, lines and loads as pieces of the monolith kit
//! (docs/REALM-MONOLITH.md; `kit.rs`, `realm_mono.rs`): big stepped masses,
//! deep slots, one honest silhouette each, light only where something is
//! being done. Each is a pure function of `(plan, state, frame)`, as the
//! flat kit's are (`render::overworld::works_kit`), keyed by the same plan
//! ids and drawn from the same footprint, so the two views of a building
//! are the same building.
//!
//! A piece is built in its own frame as `kit::build` builds its own: `w`
//! across its front and `d` deep, `x` left to right, `z` from the back (0) to
//! the front (`d`), `y` up; [`stand`] turns it to face a side and moves it
//! onto a footprint. Everything keeps within the footprint, so open ground
//! round a building stays open, and the front yard is left for the hands
//! that work it (figures stay sprites).
//!
//! - [`solids`]: a plan in a state at an animation step: under construction
//!   in four steps (the finished piece cut at the height the work has
//!   reached, scaffolded, its materials in the yard), idle, working (fire a
//!   `Kind::Fire` accent and a lamp, smoke, wheels turning), starved (cold, a
//!   board of what is missing), backed up (stock heaped outside).
//! - [`line_solids`] and [`vehicle_solids`]: the laid line per kind, and the
//!   cart, wagon, bucket, sled and barge that run on it, a load on each.
//!
//! Performance: a piece is made once per `(plan, state, frame)` and shared,
//! forty solids or so; the loop is [`FRAMES`] steps for a working piece and
//! one for the rest.

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::set::{Kind, SetLamp};

use super::blockout::{Shape, Solid};
use super::kit::Face;
use crate::render::overworld::ink::hash;
use crate::render::overworld::works_kit::{self as flat, FRAMES, Load, STEPS, State};

pub use flat::lines::{Dir, Kind as LineKind};

pub mod belts;
mod gaps;
mod plans;
pub mod rails;
mod yards;

#[cfg(test)]
#[path = "../../tests/world3d/works_kit__tests.rs"]
mod tests;

/// A piece: its solids and the lamps it lights (light = activity).
#[derive(Clone, Debug, Default)]
pub struct Piece {
    pub solids: Vec<Solid>,
    pub lamps: Vec<SetLamp>,
}

/// The footprint of plan `id` in stage pixels: width across its front and
/// depth.
pub fn extent(id: &str) -> Option<(f32, f32)> {
    flat::size(id).map(|(w, d)| (w as f32 * 16.0, d as f32 * 16.0))
}

/// Plan `id` in `state` at animation step `frame` (any count; it wraps), in
/// its own frame; `None` for a plan the kit does not draw. Made once and
/// shared.
pub fn solids(id: &str, state: State, frame: u32) -> Option<Arc<Piece>> {
    let (w, d) = extent(id)?;
    let frame = if state.frames() > 1 { frame % state.frames() } else { 0 };
    static MADE: OnceLock<Mutex<HashMap<(String, State, u32), Arc<Piece>>>> = OnceLock::new();
    let memory = MADE.get_or_init(Default::default);
    let key = (id.to_string(), state, frame);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return Some(hit);
    }
    let made = Arc::new(make(id, w, d, state, frame)?);
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    Some(made)
}

/// A works on fire, in its own frame (`w` by `d` stage pixels): flames leaping along the footprint at
/// two heights and a pillar of smoke, each flame with its lamp.
pub fn blaze(w: f32, d: f32, frame: u32) -> Piece {
    let mut b = W::new(w, d, 11, frame, true);
    let n = ((w / 14.0) as u32).max(2);
    for k in 0..n {
        let x = (k as f32 + 0.5) * w / n as f32;
        let z = d * (0.35 + 0.3 * ((k * 5 + 2) % 3) as f32 / 2.0);
        b.fire(x, 8.0 + (b.r(k) % 8) as f32, z, 6.0 + (b.r(k + 20) % 4) as f32);
    }
    b.smoke(w / 2.0, 30.0, d / 2.0, 36.0, 4);
    Piece { solids: b.solids, lamps: b.lamps }
}

/// `piece` on the footprint `foot` (`[x0, z0, x1, z1]`, stage pixels) with
/// its front toward `face`: the frame's `(x, z)` become world points and
/// every solid is turned with it.
pub fn stand(piece: &Piece, foot: [f32; 4], face: Face) -> Piece {
    let [x0, z0, x1, z1] = foot;
    let (fw, fd) = (x1 - x0, z1 - z0);
    let (w, d) = match face {
        Face::South | Face::North => (fw, fd),
        Face::East | Face::West => (fd, fw),
    };
    let turn: f32 = match face {
        Face::South => 0.0,
        Face::East => 90.0,
        Face::North => 180.0,
        Face::West => 270.0,
    };
    let (s, c) = turn.to_radians().sin_cos();
    let (s, c) = (s.round(), c.round());
    let (wx, wz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    let place = |x: f32, z: f32| {
        let (lx, lz) = (x - w / 2.0, z - d / 2.0);
        (wx + lx * c + lz * s, wz - lx * s + lz * c)
    };
    let quarter = ((turn / 90.0).round() as i32).rem_euclid(4);
    let solids = piece
        .solids
        .iter()
        .map(|solid| {
            let mut solid = solid.clone();
            let (x, z) = place(solid.at[0], solid.at[2]);
            solid.at[0] = x;
            solid.at[2] = z;
            orient(&mut solid, turn, quarter);
            solid
        })
        .collect();
    let lamps = piece
        .lamps
        .iter()
        .map(|l| {
            let (x, z) = place(l.at[0], l.at[2]);
            SetLamp { at: [x, l.at[1], z], ..*l }
        })
        .collect();
    Piece { solids, lamps }
}

/// A solid turned by `turn` more degrees, as `kit::build` turns its own: a
/// shape the same every quarter turn takes it as its sides swapped; the rest
/// keep it, less the half turns a shape the same both ways does not show.
fn orient(solid: &mut Solid, turn: f32, quarter: i32) {
    let square = matches!(solid.shape, Shape::Box | Shape::Sphere | Shape::Cone | Shape::Cylinder | Shape::Frustum | Shape::Pyramid | Shape::Gem | Shape::Rock(_) | Shape::Blob(_));
    if square && solid.turn == 0.0 {
        if quarter % 2 == 1 {
            solid.size.swap(0, 2);
        }
        return;
    }
    let half = if matches!(solid.shape, Shape::Gable | Shape::Hip | Shape::Arch | Shape::Fence(_)) { 180.0 } else { 360.0 };
    let total = (solid.turn + quarter as f32 * 90.0).rem_euclid(half);
    solid.turn = if total.abs() < 1e-3 || (half - total).abs() < 1e-3 { 0.0 } else { total };
    let _ = turn;
}

// ---------------------------------------------------------- the builder ----

/// A piece being built in its own frame.
pub(crate) struct W {
    pub w: f32,
    pub d: f32,
    pub seed: u32,
    pub t: u32,
    pub work: bool,
    pub solids: Vec<Solid>,
    pub lamps: Vec<SetLamp>,
}

impl W {
    fn new(w: f32, d: f32, seed: u32, t: u32, work: bool) -> Self {
        Self { w, d, seed, t, work, solids: Vec::new(), lamps: Vec::new() }
    }

    /// The step as an angle round a turn.
    pub fn turn(&self) -> f32 {
        (self.t % FRAMES) as f32 / FRAMES as f32 * TAU
    }

    pub fn r(&self, k: u32) -> u32 {
        hash(self.seed as i32, k as i32, 4217)
    }

    pub fn add(&mut self, s: Solid) {
        self.solids.push(s);
    }

    /// A box over `x0..x1` by `z0..z1`, from `y0` up to `y1`.
    pub fn bx(&mut self, kind: Kind, [x0, x1]: [f32; 2], [y0, y1]: [f32; 2], [z0, z1]: [f32; 2]) {
        if x1 - x0 < 0.05 || y1 - y0 < 0.05 || z1 - z0 < 0.05 {
            return;
        }
        self.add(Solid::block(kind, [x0, z0, x1, z1], y0, y1 - y0));
    }

    fn shape(&mut self, shape: Shape, kind: Kind, at: [f32; 3], size: [f32; 3], turn: f32) {
        let mut s = Solid::new(shape, kind, at, size);
        s.turn = turn;
        self.add(s);
    }

    /// A column `r` across its foot at `(cx, cz)`, from `y0`, `h` high.
    pub fn cyl(&mut self, kind: Kind, cx: f32, cz: f32, r: f32, y0: f32, h: f32) {
        self.shape(Shape::Cylinder, kind, [cx, y0, cz], [2.0 * r, h, 2.0 * r], 0.0);
    }

    /// A tapering column (to 0.6 across at its top).
    pub fn frus(&mut self, kind: Kind, cx: f32, cz: f32, r: f32, y0: f32, h: f32) {
        self.shape(Shape::Frustum, kind, [cx, y0, cz], [2.0 * r, h, 2.0 * r], 0.0);
    }

    pub fn cone(&mut self, kind: Kind, cx: f32, cz: f32, r: f32, y0: f32, h: f32) {
        self.shape(Shape::Cone, kind, [cx, y0, cz], [2.0 * r, h, 2.0 * r], 0.0);
    }

    pub fn pyr(&mut self, kind: Kind, cx: f32, cz: f32, sx: f32, sz: f32, y0: f32, h: f32) {
        self.shape(Shape::Pyramid, kind, [cx, y0, cz], [sx, h, sz], 0.0);
    }

    /// A ball `r` across its middle at `(cx, cy, cz)`.
    pub fn ball(&mut self, kind: Kind, cx: f32, cy: f32, cz: f32, rx: f32, ry: f32, rz: f32) {
        self.shape(Shape::Sphere, kind, [cx, cy - ry, cz], [2.0 * rx, 2.0 * ry, 2.0 * rz], 0.0);
    }

    pub fn gem(&mut self, kind: Kind, cx: f32, y0: f32, cz: f32, s: f32, h: f32, turn: f32) {
        self.shape(Shape::Gem, kind, [cx, y0, cz], [s, h, s], turn);
    }

    /// A pitched roof over `x0..x1` by `z0..z1`, its ridge along x (or z).
    pub fn gable(&mut self, kind: Kind, [x0, x1]: [f32; 2], [z0, z1]: [f32; 2], y0: f32, h: f32, ridge_z: bool) {
        let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
        if ridge_z {
            self.shape(Shape::Gable, kind, [cx, y0, cz], [z1 - z0, h, x1 - x0], 90.0);
        } else {
            self.shape(Shape::Gable, kind, [cx, y0, cz], [x1 - x0, h, z1 - z0], 0.0);
        }
    }

    pub fn hip(&mut self, kind: Kind, [x0, x1]: [f32; 2], [z0, z1]: [f32; 2], y0: f32, h: f32) {
        let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
        self.shape(Shape::Hip, kind, [cx, y0, cz], [x1 - x0, h, z1 - z0], 0.0);
    }

    /// A ramp, full height at the back (-z), nothing at the front.
    pub fn wedge(&mut self, kind: Kind, [x0, x1]: [f32; 2], [z0, z1]: [f32; 2], y0: f32, h: f32) {
        let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
        self.shape(Shape::Wedge, kind, [cx, y0, cz], [x1 - x0, h, z1 - z0], 0.0);
    }

    /// A wall block with a round-topped opening through it along z.
    pub fn arch(&mut self, kind: Kind, cx: f32, cz: f32, w: f32, h: f32, depth: f32, y0: f32) {
        self.shape(Shape::Arch, kind, [cx, y0, cz], [w, h, depth], 0.0);
    }

    pub fn rock(&mut self, kind: Kind, cx: f32, cz: f32, s: f32, y0: f32, h: f32, k: u32) {
        let v = self.r(k);
        self.shape(Shape::Rock((v % 64) as u16 + 1), kind, [cx, y0, cz], [s, h, s * 0.9], (v % 360) as f32);
    }

    pub fn fence(&mut self, kind: Kind, [x0, x1]: [f32; 2], z: f32, h: f32) {
        let posts = (((x1 - x0) / 8.0).round() as u16).clamp(2, 8);
        self.shape(Shape::Fence(posts), kind, [(x0 + x1) / 2.0, 0.0, z], [x1 - x0, h, 2.0], 0.0);
    }

    pub fn lamp(&mut self, at: [f32; 3], reach: f32, strength: f32) {
        self.lamps.push(SetLamp { at, reach, strength });
    }

    /// A stepped plinth under a body over `x` by `z`: each tier `(rise, out)`.
    /// Returns the height reached.
    pub fn plinth(&mut self, kind: Kind, [x0, x1]: [f32; 2], [z0, z1]: [f32; 2], tiers: &[(f32, f32)]) -> f32 {
        let mut y = 0.0;
        for &(rise, out) in tiers {
            self.bx(kind, [x0 - out, x1 + out], [y, y + rise], [z0 - out, z1 + out]);
            y += rise;
        }
        y
    }

    /// A door as a deep slot in the face at `z` (front): dark, jambs proud of it.
    pub fn door(&mut self, x: f32, z: f32, y: f32, [w, h]: [f32; 2]) {
        self.bx(Kind::Door, [x - w / 2.0, x + w / 2.0], [y, y + h], [z - 0.3, z + 0.3]);
        for side in [-1.0f32, 1.0] {
            let jx = x + side * (w / 2.0 + 1.0);
            self.bx(Kind::Stone, [jx - 1.0, jx + 1.0], [y, y + h + 2.0], [z, z + 1.5]);
        }
        self.bx(Kind::Stone, [x - w / 2.0 - 2.0, x + w / 2.0 + 2.0], [y + h, y + h + 2.0], [z, z + 1.5]);
    }

    /// A slit window in the face at `z`: lit (a window accent) while working.
    pub fn slit(&mut self, x: f32, z: f32, y: f32, [w, h]: [f32; 2]) {
        let kind = if self.work { Kind::Window } else { Kind::Door };
        self.bx(kind, [x - w / 2.0, x + w / 2.0], [y, y + h], [z - 0.2, z + 0.3]);
    }

    /// Smoke over `(x, y, z)`: `n` puffs rising `rise`, growing as they go,
    /// drifting east. A piece that is not working has none.
    pub fn smoke(&mut self, x: f32, y: f32, z: f32, rise: f32, n: u32) {
        if !self.work {
            return;
        }
        for k in 0..n {
            let age = (((self.t % FRAMES) as f32 / FRAMES as f32 * n as f32 + k as f32) % n as f32) / n as f32;
            let r = 1.6 + age * 3.4;
            self.ball(Kind::Plaster, x + age * 5.0, y + age * rise, z, r, r * 0.9, r);
        }
    }

    /// A fire standing on `(x, y, z)`, `s` across, leaping from step to
    /// step, with a lamp over it. Out when the piece is not working.
    pub fn fire(&mut self, x: f32, y: f32, z: f32, s: f32) {
        if !self.work {
            return;
        }
        let leap = 1.0 + (self.r(900 + self.t % FRAMES) % 3) as f32 * 0.25;
        self.cone(Kind::Fire, x, z, s / 2.0, y, s * 1.5 * leap);
        self.lamp([x, y + s * 1.6, z], 38.0 + s * 3.0, 0.8);
    }

    /// A wheel standing on edge, facing the front, `r` across the middle of
    /// its disc at `(cx, cy, cz)`, turned by `angle`: a thin disc, a hub,
    /// `n` boards or lugs round its rim that travel as it turns.
    pub fn wheel(&mut self, kind: Kind, cx: f32, cy: f32, cz: f32, r: f32, angle: f32, n: u32, thick: f32) {
        self.ball(kind, cx, cy, cz, r, r, thick / 2.0);
        self.ball(Kind::Metal, cx, cy, cz, r * 0.22, r * 0.22, thick / 2.0 + 0.8);
        for k in 0..n {
            let a = angle + k as f32 / n as f32 * TAU;
            let (x, y) = (cx + a.cos() * r, cy + a.sin() * r);
            self.bx(Kind::Floor, [x - 1.3, x + 1.3], [(y - 1.3).max(0.0), (y + 1.3).max(0.6)], [cz - thick / 2.0 - 1.0, cz + thick / 2.0 + 1.0]);
            // A spoke dot half way along.
            let (sx, sy) = (cx + a.cos() * r * 0.55, cy + a.sin() * r * 0.55);
            self.bx(Kind::Trunk, [sx - 0.8, sx + 0.8], [(sy - 0.8).max(0.0), (sy + 0.8).max(0.6)], [cz - thick / 2.0 - 0.4, cz + thick / 2.0 + 0.4]);
        }
    }

    /// A heap of `load` standing on ground `y0` at `(cx, cz)`, `w` across,
    /// `h` high.
    pub fn heap(&mut self, load: Load, cx: f32, cz: f32, w: f32, y0: f32, h: f32) {
        let seed = self.seed;
        self.solids.extend(load_solids(load, cx, cz, w, y0, h, seed));
    }

    /// A board on a post: the missing or the surplus good heaped on its top.
    pub fn board(&mut self, x: f32, z: f32, load: Load, warning: bool) {
        self.bx(Kind::Floor, [x - 0.8, x + 0.8], [0.0, 14.0], [z - 0.8, z + 0.8]);
        self.bx(Kind::Floor, [x - 5.0, x + 5.0], [10.0, 17.0], [z - 0.6, z + 0.6]);
        // The board's rim: a dark frame for a shortfall, a pale one for a glut.
        let rim = if warning { Kind::Door } else { Kind::Bone };
        self.bx(rim, [x - 5.0, x + 5.0], [16.0, 17.6], [z - 1.0, z + 1.0]);
        self.solids.extend(load_solids(load, x, z + 1.5, 7.0, 17.0, 5.0, self.seed));
    }
}

fn make(id: &str, w: f32, d: f32, state: State, frame: u32) -> Option<Piece> {
    let seed = id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    let build = |work: bool, t: u32| -> Option<W> {
        let mut b = W::new(w, d, seed, t, work);
        plans::draw(id, &mut b)?;
        Some(b)
    };
    let mut b = match state {
        State::Working => build(true, frame)?,
        State::Idle => build(false, 0)?,
        State::Starved(load) => {
            let mut b = build(false, 0)?;
            b.board(4.5, d - 5.0, load, true);
            b
        }
        State::BackedUp(load) => {
            let mut b = build(false, 0)?;
            let (x, z) = (w - 9.0, d - 7.0);
            b.heap(load, x, z, 14.0, 0.0, 12.0);
            if w >= 40.0 {
                b.heap(load, w - 26.0, d - 6.0, 12.0, 0.0, 8.0);
            }
            if w >= 64.0 {
                b.heap(load, w - 40.0, d - 5.0, 12.0, 0.0, 7.0);
            }
            b.board(4.5, d - 5.0, load, false);
            b
        }
        State::Building(step) => return raise(id, w, d, step.min(STEPS - 1), seed),
    };
    Some(Piece { solids: std::mem::take(&mut b.solids), lamps: std::mem::take(&mut b.lamps) })
}

/// What is being raised of: the pile its yard keeps.
fn materials(id: &str) -> Load {
    match id {
        "bakery" | "brick-kiln" | "brick-cottage" | "brick-wall" | "brick-gate" => Load::Brick,
        "lodge" | "treadwheel-crane" | "sawpit" | "carpentry" | "wheelwright" | "ropewalk" | "warehouse" | "farmstead" | "brewery" | "weaving-shed" | "hut" | "cottage" | "woodsman-camp" | "smithy" | "depot" | "waterwheel" | "windmill" | "horse-gin" | "treadwheel" | "mortar-mill" | "intake" => Load::Planks,
        _ => Load::Ashlar,
    }
}

/// A building being raised: step 0 stakes the ground out; steps 1 to 3 are
/// the finished piece cut at the height the work has reached, the roof not
/// yet on, a scaffold up its face.
fn raise(id: &str, w: f32, d: f32, step: u8, seed: u32) -> Option<Piece> {
    let fin = solids(id, State::Idle, 0)?;
    let top = fin.solids.iter().map(|s| s.top()).fold(0.0f32, f32::max).max(8.0);
    let mut b = W::new(w, d, seed, 0, false);
    let load = materials(id);
    if step == 0 {
        // Stakes at the corners, a cord between, a low plinth of footings begun, a heap by.
        let (x0, x1, z0, z1) = (3.0, w - 3.0, d * 0.2, d - 3.0);
        for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
            b.bx(Kind::Floor, [x - 0.8, x + 0.8], [0.0, 6.0], [z - 0.8, z + 0.8]);
        }
        for (a, c) in [((x0, z0), (x1, z0)), ((x0, z1), (x1, z1)), ((x0, z0), (x0, z1)), ((x1, z0), (x1, z1))] {
            let (cx, cz) = ((a.0 + c.0) / 2.0, (a.1 + c.1) / 2.0);
            b.bx(Kind::Cloth, [cx.min(a.0).min(c.0) - 0.0, cx.max(a.0).max(c.0)], [5.0, 5.8], [cz.min(a.1).min(c.1), cz.max(a.1).max(c.1) + 0.4]);
        }
        b.bx(Kind::Stone, [x0 + 4.0, x1 - 4.0], [0.0, 1.4], [z0 + 4.0, z1 - 4.0]);
        b.heap(load, w - 9.0, d - 7.0, 13.0, 0.0, 8.0);
        return Some(Piece { solids: b.solids, lamps: Vec::new() });
    }
    let frac = [0.0, 0.28, 0.6, 0.9][step as usize];
    let cut = (top * frac).max(3.0);
    for s in &fin.solids {
        // Ragged work: each piece of the wall stops a little either side of the line.
        let ragged = cut + ((hash(s.at[0] as i32, s.at[2] as i32, seed) % 5) as f32 - 2.0);
        if s.bottom() >= ragged {
            continue;
        }
        if s.top() <= ragged {
            b.add(s.clone());
            continue;
        }
        // Over the line: a wall is cut to it; anything that roofs or caps is not yet on.
        match s.shape {
            Shape::Box | Shape::Cylinder | Shape::Frustum | Shape::Arch | Shape::Stairs | Shape::Rock(_) => {
                let mut cutting = s.clone();
                cutting.size[1] = (ragged - s.bottom()).max(0.5);
                b.add(cutting);
            }
            _ => {}
        }
    }
    // The scaffold: poles at the corners and the middle, planks across at two lifts, a ladder.
    let (sx0, sx1, sz) = (1.0, w - 1.0, d - 1.5);
    for x in [sx0, w / 2.0, sx1] {
        b.bx(Kind::Floor, [x - 0.7, x + 0.7], [0.0, cut + 9.0], [sz - 0.7, sz + 0.7]);
    }
    for y in [cut * 0.45, cut * 0.9] {
        b.bx(Kind::Floor, [sx0, sx1], [y.max(2.0), y.max(2.0) + 1.0], [sz - 2.0, sz + 1.5]);
    }
    b.bx(Kind::Floor, [w - 6.0, w - 4.8], [0.0, cut * 0.9 + 2.0], [sz + 1.0, sz + 1.8]);
    b.bx(Kind::Floor, [w - 3.6, w - 2.4], [0.0, cut * 0.9 + 2.0], [sz + 1.0, sz + 1.8]);
    if step == 3 {
        // Roofing: bare rafters standing where the roof will lie.
        for k in 0..3 {
            let x = 4.0 + k as f32 * (w - 8.0) / 2.0;
            b.bx(Kind::Floor, [x - 0.6, x + 0.6], [cut, cut + top * 0.18], [d * 0.3, d - 3.0]);
        }
        b.bx(Kind::Floor, [4.0, w - 4.0], [cut + top * 0.18 - 1.0, cut + top * 0.18], [d * 0.55 - 0.7, d * 0.55 + 0.7]);
    }
    b.heap(load, 8.0, d - 6.0, 12.0, 0.0, 8.0);
    Some(Piece { solids: b.solids, lamps: Vec::new() })
}

// -------------------------------------------------------------- the loads --

/// Solids for a heap of `load` on ground `y0` at `(cx, cz)`, `w` across and
/// `h` high: stacked blocks, logs, sacks, barrels or a mound, in the kind
/// whose tone reads as the good.
pub fn load_solids(load: Load, cx: f32, cz: f32, w: f32, y0: f32, h: f32, seed: u32) -> Vec<Solid> {
    let mut b = W::new(w, w, seed, 0, false);
    let (hw, row) = (w / 2.0, h / 3.0);
    match load {
        // Quarry stone and ore: rough lumps heaped, a pyramid of boulders (the ore darker, with glints
        // of metal in its faces), so a cart of it reads as a load and not as masonry.
        Load::Stone | Load::Ore => {
            let (kind, chip) = if load == Load::Ore { (Kind::Slate, Kind::Metal) } else { (Kind::Rock, Kind::Stone) };
            let s = w * 0.4;
            for i in 0..3 {
                let x = cx + (i as f32 - 1.0) * w * 0.3;
                b.rock(kind, x, cz, s, y0, h * 0.55, 10 + i);
            }
            for i in 0..2 {
                let x = cx + (i as f32 - 0.5) * w * 0.28;
                b.rock(kind, x, cz, s * 0.8, y0 + h * 0.42, h * 0.5, 20 + i);
            }
            for i in 0..3 {
                let x = cx + (i as f32 - 1.0) * w * 0.28;
                let top = if load == Load::Ore { y0 + h * 0.3 } else { y0 + h * 0.7 };
                b.bx(chip, [x - 0.8, x + 0.8], [top, top + 1.0], [cz - 1.4 + i as f32 * 1.3, cz - 0.4 + i as f32 * 1.3]);
            }
        }
        // Blocks: layers of boxes, each smaller than the last, a little askew.
        Load::Ashlar | Load::Brick | Load::Slate | Load::Iron | Load::Lead | Load::Crate | Load::Planks | Load::Hides | Load::Cloth => {
            let (kind, bw, bh, bd) = match load {
                Load::Ashlar => (Kind::Road, 5.0, 3.0, 3.5),
                Load::Brick => (Kind::Brick, 3.4, 1.8, 1.8),
                Load::Slate => (Kind::Slate, 5.0, 1.0, 4.0),
                Load::Iron => (Kind::Metal, 5.0, 1.4, 1.6),
                Load::Lead => (Kind::Metal, 3.4, 2.0, 2.4),
                Load::Planks => (Kind::Sand, 7.0, 1.0, 3.0),
                Load::Hides => (Kind::Path, 6.0, 0.8, 5.0),
                Load::Cloth => (Kind::Cloth, 5.0, 2.0, 3.0),
                _ => (Kind::Floor, 4.5, 4.0, 4.5),
            };
            let layers = ((h / bh).floor() as i32).clamp(1, 7);
            for l in 0..layers {
                let n = (((hw * 2.0 - l as f32 * bw * 0.5) / bw).floor() as i32).max(1);
                let left = cx - n as f32 * bw / 2.0;
                for k in 0..n {
                    let jitter = ((hash(k, l, seed) % 3) as f32 - 1.0) * 0.4;
                    let x = left + (k as f32 + 0.5) * bw + jitter;
                    b.bx(kind, [x - bw / 2.0 + 0.15, x + bw / 2.0 - 0.15], [y0 + l as f32 * bh, y0 + (l as f32 + 1.0) * bh], [cz - bd / 2.0 - jitter, cz + bd / 2.0 - jitter]);
                }
            }
        }
        // Logs: dark boxes laid long, their ends pale.
        Load::Timber => {
            let layers = ((h / 2.4).floor() as i32).clamp(1, 5);
            for l in 0..layers {
                let n = (((w - l as f32 * 2.0) / 2.6).floor() as i32).max(1);
                for k in 0..n {
                    let x = cx - n as f32 * 1.3 + (k as f32 + 0.5) * 2.6;
                    b.bx(Kind::Wood, [x - 1.2, x + 1.2], [y0 + l as f32 * 2.3, y0 + l as f32 * 2.3 + 2.3], [cz - 4.0, cz + 4.0]);
                }
            }
            b.bx(Kind::Sand, [cx - w / 2.0 + 0.8, cx + w / 2.0 - 0.8], [y0, y0 + 1.0], [cz + 3.8, cz + 4.2]);
        }
        // Barrels and casks: short columns, a second row on top.
        Load::Ale | Load::Water => {
            let kind = if load == Load::Water { Kind::Water } else { Kind::Floor };
            let n = ((w / 5.0).floor() as i32).clamp(1, 4);
            for k in 0..n {
                let x = cx - n as f32 * 2.5 + (k as f32 + 0.5) * 5.0;
                b.cyl(kind, x, cz, 2.2, y0, 5.0);
                b.cyl(Kind::Metal, x, cz, 2.3, y0 + 1.2, 0.7);
            }
            if n > 1 && h > 6.0 {
                b.cyl(kind, cx, cz, 2.2, y0 + 5.0, 5.0);
            }
        }
        // Fleece: loose soft lumps heaped together.
        Load::Wool => {
            for (k, (dx, dy, s)) in [(-0.3, 0.0, 6.0), (0.25, 0.0, 5.0), (-0.1, 0.35, 5.0), (0.2, 0.55, 3.6)].into_iter().enumerate() {
                b.add(Solid::new(Shape::Blob((k as u16) * 7 + 3), Kind::Bone, [cx + dx * w * 0.6, y0 + dy * h, cz], [s * 1.5, s, s]));
            }
        }
        // Sacks: soft pale lumps in a stack.
        Load::Flour | Load::Bread => {
            let kind = if load == Load::Bread { Kind::Thatch } else { Kind::Plaster };
            let n = ((w / 4.5).floor() as i32).clamp(1, 4);
            for l in 0..(n.min(3)) {
                for k in 0..(n - l) {
                    let x = cx - (n - l) as f32 * 2.25 + (k as f32 + 0.5) * 4.5;
                    b.ball(kind, x, y0 + 2.2 + l as f32 * 3.4, cz, 2.4, 2.0, 2.2);
                }
            }
        }
        // Loose piles: a mound of what pours, grains of its own tone.
        _ => {
            let kind = match load {
                Load::Lime => Kind::Plaster,
                Load::Sand => Kind::Sand,
                Load::Mortar => Kind::Path,
                Load::Clay => Kind::Brick,
                Load::Charcoal => Kind::Mud,
                Load::Coal => Kind::Slate,
                Load::Grain => Kind::Crop,
                Load::Hemp => Kind::Hedge,
                Load::Rope => Kind::Thatch,
                Load::Glass => Kind::Metal,
                Load::Ore => Kind::Rock,
                _ => Kind::Rock,
            };
            if load == Load::Rope {
                b.cyl(kind, cx, cz, hw * 0.55, y0, row * 0.9);
                b.cyl(kind, cx + hw * 0.3, cz, hw * 0.4, y0 + row * 0.9, row * 0.8);
            } else if load == Load::Glass {
                for k in 0..4 {
                    b.gem(kind, cx - hw + (k as f32 + 0.5) * hw / 2.0, y0, cz + (k % 2) as f32, 3.2, 4.0 + (k % 3) as f32, k as f32 * 30.0);
                }
            } else if load == Load::Lime {
                // Quicklime: a pale mound with the sacks it came in leaning at its foot.
                b.cone(kind, cx, cz, hw, y0, h);
                for (dx, dz) in [(-0.55f32, 0.5f32), (0.45, 0.6)] {
                    b.ball(Kind::Bone, cx + dx * hw, y0 + 1.7, cz + dz * hw, 2.3, 1.7, 1.9);
                }
            } else {
                b.cone(kind, cx, cz, hw, y0, h);
            }
        }
    }
    b.solids
}

// ------------------------------------------------------- lines & carriers --

/// One tile of laid line, in its own frame (16 across, `z` down the tile,
/// `x` across): the road, the rails, a cable and its pylon (a `post` tile),
/// the chute's trough on trestles, the barge's lane. `links` are the
/// neighbours it joins: `N | E | S | W` bits of
/// `render::overworld::works_kit::lines`.
pub fn line_solids(kind: LineKind, links: u8, post: bool, frame: u32) -> Arc<Vec<Solid>> {
    line_solids_at(kind, links, post, 0, frame)
}

/// [`line_solids`] for a tile `span` tiles along a ropeway's span from its
/// tower (`0` at the tower, up to [`rails::SPAN`] - 1): the cable sags between
/// towers, so each tile of it is at its own height. Any other line ignores it.
pub fn line_solids_at(kind: LineKind, links: u8, post: bool, span: u32, frame: u32) -> Arc<Vec<Solid>> {
    static MADE: OnceLock<Mutex<HashMap<(LineKind, u8, bool, u32, u32), Arc<Vec<Solid>>>>> = OnceLock::new();
    let memory = MADE.get_or_init(Default::default);
    let frame = if matches!(kind, LineKind::Barge) { frame % FRAMES } else { 0 };
    let span = if matches!(kind, LineKind::Rope) { span % rails::SPAN } else { 0 };
    let key = (kind, links & 15, post, span, frame);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let made = Arc::new(rails::line(kind, links & 15, post, span, frame));
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    made
}

/// A carrier on a line, in its own frame (its front toward `dir`, standing
/// on the tile's middle, `x` 0 to its length, `y` up): an ox cart, a wagon,
/// a bucket hung from the cable, a sled in the chute, a barge; `load` heaped
/// on it.
pub fn vehicle_solids(kind: LineKind, dir: Dir, load: Option<Load>, frame: u32) -> Arc<Vec<Solid>> {
    vehicle_solids_lod(kind, dir, load, frame, true)
}

/// [`vehicle_solids`], or, with `near` false, the same carrier in a few blocks: what a works with
/// many carriers on its roads draws, where each is a few pixels.
pub fn vehicle_solids_lod(kind: LineKind, dir: Dir, load: Option<Load>, frame: u32, near: bool) -> Arc<Vec<Solid>> {
    static MADE: OnceLock<Mutex<HashMap<(LineKind, Dir, Option<Load>, u32, bool), Arc<Vec<Solid>>>>> = OnceLock::new();
    let memory = MADE.get_or_init(Default::default);
    let key = (kind, dir, load, frame % FRAMES, near);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let made = Arc::new(rails::vehicle(kind, dir, load, frame % FRAMES, near));
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    made
}
