//! The monolith: a castle's outer wall and the approach to it over a river,
//! in a look that lights its own scene (`Look::lights`): the proof of the
//! world's new direction (2026-10-18). Big plain geometry, one continuous
//! terrain, solid colours; the sun, its shadows, a few warm lamps, the
//! palette of each place and the dither do the work, not textures.
//!
//! - **One terrain.** A single height field, not chunk cells: a meadow
//!   rolling down to a river that meanders across it, the far bank rising
//!   to a plateau the castle stands on. The road is the brightest ground,
//!   a ribbon to follow.
//! - **Monolithic.** The castle is its south face only, and it is too big to
//!   see whole: a curtain wall 34 m high and 220 m long, battered at the
//!   foot, ribbed with full-height fins, a heavy cornice, square towers of
//!   52 m, and a deep slot of a gate with a lamp in it. A keep stands
//!   behind. Standing slabs line the approach; trees are dark masses.
//! - **Palettes by place.** The meadow is a sage ramp, the river a teal one,
//!   the castle a cold slate; each passes into the next across a wide
//!   feather. Lamplight moves what it lights toward an ember ramp.
//!
//! Units are metres: +Y up, the river runs west (-X) to east, the castle is
//! north (-Z), the walk starts in the south. Everything is a pure function of
//! where the knight is and the clock, so it can be shot without a window.

use std::f32::consts::PI;

use angel_stage::look::{Dither, Fog, Lamp, Look, Reveal, Shadow, Sun, Zone, smoothstep};
use angel_stage::scene::{
    AlphaMode, Camera, Material, MeshData, MeshRef, Node, Projection, Role, Sampling, Scene, TextureSource,
    Transform, quat_axis_angle, quat_mul,
};

use crate::layer::LAYER_PICTURES;
use crate::shore::look_at;

/// The picture the scene is drawn into.
pub const SCENE_ID: u32 = LAYER_PICTURES + 0x90;
/// The knight's sprites: walking away from the eye, and across.
pub const KNIGHT_BACK_ID: u32 = LAYER_PICTURES + 0x91;
pub const KNIGHT_SIDE_ID: u32 = LAYER_PICTURES + 0x92;
pub const KNIGHT_FRONT_ID: u32 = LAYER_PICTURES + 0x93;

/// The meshes the place is made of, uploaded once (`meshes`).
pub const TERRAIN_MESH: u32 = 0x4D_0001;
pub const WATER_MESH: u32 = 0x4D_0002;
pub const CASTLE_MESH: u32 = 0x4D_0003;
pub const STONES_MESH: u32 = 0x4D_0004;
pub const TREES_MESH: u32 = 0x4D_0005;
pub const BRIDGE_MESH: u32 = 0x4D_0006;
pub const WINDOWS_MESH: u32 = 0x4D_0007;
pub const FIRES_MESH: u32 = 0x4D_0008;
pub const SKY_MESH: u32 = 0x4D_0009;
pub const QUAY_MESH: u32 = 0x4D_000A;

/// The internal height, in pixels: the knight's sprite lands near one to one.
pub const ROWS: u32 = 540;

const KNIGHT_BACK_PNG: &[u8] = include_bytes!("../../../cockpit/assets/dungeon/art-v2/knight-back.png");
const KNIGHT_SIDE_PNG: &[u8] = include_bytes!("../../../cockpit/assets/dungeon/art-v2/knight-run.png");
const KNIGHT_FRONT_PNG: &[u8] = include_bytes!("../../../cockpit/assets/dungeon/art-v2/knight-red.png");

// ------------------------------------------------------------- palettes ----

/// The meadow, dark to light: sage greys.
pub const MEADOW: [[u8; 3]; 8] = [
    [0x0e, 0x12, 0x10],
    [0x17, 0x1f, 0x1b],
    [0x22, 0x2d, 0x27],
    [0x2f, 0x3c, 0x33],
    [0x40, 0x4f, 0x42],
    [0x55, 0x65, 0x52],
    [0x70, 0x7e, 0x67],
    [0x96, 0x9f, 0x84],
];

/// The river: teal greys.
pub const RIVER: [[u8; 3]; 8] = [
    [0x0b, 0x12, 0x16],
    [0x12, 0x1e, 0x25],
    [0x1a, 0x2b, 0x35],
    [0x24, 0x3a, 0x46],
    [0x31, 0x4c, 0x59],
    [0x42, 0x61, 0x6d],
    [0x5b, 0x7c, 0x85],
    [0x82, 0x9f, 0xa3],
];

/// The castle: deep indigo.
pub const SLATE: [[u8; 3]; 8] = [
    [0x0e, 0x0f, 0x19],
    [0x14, 0x17, 0x27],
    [0x1b, 0x1f, 0x36],
    [0x24, 0x2a, 0x47],
    [0x2f, 0x36, 0x58],
    [0x3e, 0x46, 0x6b],
    [0x54, 0x5c, 0x82],
    [0x77, 0x7f, 0xa2],
];

/// Where lamps light: embers.
pub const EMBER: [[u8; 3]; 8] = [
    [0x17, 0x10, 0x0e],
    [0x29, 0x19, 0x12],
    [0x41, 0x25, 0x17],
    [0x5d, 0x33, 0x1d],
    [0x7f, 0x45, 0x23],
    [0xa3, 0x5e, 0x2b],
    [0xc6, 0x83, 0x3f],
    [0xe6, 0xb2, 0x66],
];

/// The light's glint on the water, and a lit window.
pub const GLINT: [u8; 3] = [0xd9, 0xd6, 0xb8];
pub const WINDOW: [u8; 3] = [0xee, 0xae, 0x55];

// --------------------------------------------------------------- layout ----

/// The river's middle line: its z at x.
pub fn river_z(x: f32) -> f32 {
    -20.0 + 10.0 * (x / 55.0).sin() + 4.0 * (x / 23.0 + 0.6).sin() - 4.0 * 0.6f32.sin()
}

/// Half the river's width of water.
pub const RIVER_HALF: f32 = 13.0;
/// The water's surface.
pub const WATER_Y: f32 = -1.6;

/// The plateau north of the river, which the castle stands on.
pub const PLATEAU_Y: f32 = 7.0;

/// The quay: a battered wall along the river's north edge holding the
/// plateau up, its face at `quay_z`, its top `QUAY_DEPTH` deep.
pub fn quay_z(x: f32) -> f32 {
    river_z(x) - RIVER_HALF + 1.0
}
pub const QUAY_DEPTH: f32 = 7.0;

/// The castle's south face, and the dry moat at its foot (z from the face
/// out to `MOAT_EDGE`, its floor at `MOAT_Y`).
pub const WALL_Z: f32 = -96.0;
pub const MOAT_EDGE: f32 = WALL_Z + 24.0;
pub const MOAT_Y: f32 = -1.0;
/// From the moat's floor.
pub const WALL_HEIGHT: f32 = 58.0;
pub const TOWER_HEIGHT: f32 = 86.0;
/// The causeway over the moat into the gate.
pub const CAUSEWAY_HALF: f32 = 7.0;

/// The curtain wall's stepped plinth on its south face, foot first:
/// `(rise, out)` in metres.
const WALL_TIERS: [(f32, f32); 3] = [(6.0, 6.0), (5.0, 4.0), (5.0, 2.0)];
/// The towers' stepped plinth, all round.
const TOWER_TIERS: [(f32, f32); 3] = [(8.0, 7.0), (7.0, 4.5), (7.0, 2.0)];

/// The slits high in the curtain wall: how high their foot is, how tall.
const SLIT_Y: f32 = MOAT_Y - 0.5 + 41.0;
const SLIT_H: f32 = 5.0;

/// The gate lamp's foot, hanging in the slot.
const LAMP_Y: f32 = PLATEAU_Y + 11.0;

/// The middles of the slits along the curtain wall, one a bay between the
/// fins, and whether each is lit.
fn wall_slits() -> Vec<(f32, bool)> {
    let tower_half = 13.0;
    let mut out = Vec::new();
    for [x0, x1] in [[-260.0f32, -130.0], [-130.0, -26.0], [26.0, 130.0], [130.0, 260.0]] {
        let (x0, x1) = (x0 + tower_half, x1 - tower_half);
        let count = ((x1 - x0) / 12.0).floor() as i32;
        let start = x0 + ((x1 - x0) - count as f32 * 12.0) / 2.0;
        for k in 0..count {
            let x = start + (k as f32 + 0.5) * 12.0;
            out.push((x, hash2(x as i32, 77) > 0.72));
        }
    }
    out
}

/// The bridge: a high deck level with the plateau, from the quay to the
/// south bank, then a ramp down to the meadow.
pub const DECK_Y: f32 = PLATEAU_Y;
pub const BRIDGE_HALF: f32 = 5.0;
pub const BRIDGE_SOUTH: f32 = 2.0;
pub const RAMP_SOUTH: f32 = 38.0;
pub fn bridge_north() -> f32 {
    quay_z(0.0)
}

/// The road, south to the gate: x at each z, a line through these points.
const ROAD: [[f32; 2]; 6] = [
    [10.0, 150.0],
    [7.0, 110.0],
    [-3.0, 76.0],
    [0.0, RAMP_SOUTH + 8.0],
    [0.0, BRIDGE_SOUTH],
    [0.0, WALL_Z - 14.0],
];

/// The road's half width.
pub const ROAD_HALF: f32 = 3.6;

/// The road's x at z (the line's, clamped to its ends).
pub fn road_x(z: f32) -> f32 {
    for w in ROAD.windows(2) {
        let (a, b) = (w[0], w[1]);
        if z <= a[1] && z >= b[1] {
            let t = (a[1] - z) / (a[1] - b[1]);
            // A smooth step between the points, so the road bends softly.
            let s = t * t * (3.0 - 2.0 * t);
            return a[0] + (b[0] - a[0]) * s;
        }
    }
    if z > ROAD[0][1] { ROAD[0][0] } else { ROAD[ROAD.len() - 1][0] }
}

/// A number in 0..1 that looks arbitrary and is the same every time.
fn hash2(x: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (z as u32).wrapping_mul(0xD816_3841) ^ 0x9E37_79B9;
    h ^= h >> 13;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth value noise, about -1..1, features a unit apart.
fn noise(x: f32, z: f32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let (ix, iz) = (x0 as i32, z0 as i32);
    let a = hash2(ix, iz);
    let b = hash2(ix + 1, iz);
    let c = hash2(ix, iz + 1);
    let d = hash2(ix + 1, iz + 1);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    (top + (bottom - top) * sz) * 2.0 - 1.0
}

/// The meadow's own height at (x, z): long low swells, rising to hills at
/// the world's edge.
fn meadow(x: f32, z: f32) -> f32 {
    let swell = 1.0 + 1.6 * noise(x / 70.0 + 3.1, z / 70.0 - 1.7) + 0.7 * noise(x / 29.0 - 5.0, z / 29.0 + 2.0);
    let edge = smoothstep(150.0, 230.0, (x * x * 0.6 + (z - 30.0).powi(2)).sqrt());
    swell + edge * (10.0 + 6.0 * noise(x / 40.0, z / 40.0))
}

/// The ground's height at (x, z): the meadow down to the river, the river's
/// bed up to the quay, the plateau behind it, the moat cut in front of the
/// wall. Under the quay, the moat's walls and the castle it is hidden.
pub fn ground(x: f32, z: f32) -> f32 {
    let bed = WATER_Y - 2.6;
    let q = quay_z(x);
    if z < q - 1.5 {
        // The plateau; the moat; the castle's own ground.
        if (WALL_Z - 16.0..=MOAT_EDGE + 0.5).contains(&z) {
            return MOAT_Y;
        }
        return PLATEAU_Y + 0.15 * noise(x / 40.0, z / 40.0);
    }
    let mut h = meadow(x, z);
    // The road: levelled across, a little into the swells.
    let off = (x - road_x(z)).abs();
    let on_road = 1.0 - smoothstep(ROAD_HALF, ROAD_HALF + 5.0, off);
    h += (meadow(road_x(z), z) * 0.85 - h) * on_road * 0.7;
    // The south bank down to the bed; north of the middle it is all bed,
    // up to the quay.
    let r = river_z(x);
    let bank = if z > r { smoothstep(RIVER_HALF - 4.0, RIVER_HALF + 10.0, z - r) } else { 0.0 };
    bed + (h - bed) * bank
}

/// How bright the ground's own colour is at (x, z): grass, the road, the
/// wet shore, the moat.
fn ground_tone(x: f32, z: f32, h: f32) -> f32 {
    let mut t = 0.22 + 0.02 * noise(x / 18.0 + 11.0, z / 18.0);
    if z < quay_z(x) - 1.5 {
        t = if (WALL_Z - 16.0..=MOAT_EDGE + 0.5).contains(&z) { 0.15 } else { 0.25 };
    }
    let off = (x - road_x(z)).abs();
    let road = 1.0 - smoothstep(ROAD_HALF - 0.6, ROAD_HALF + 0.6, off);
    t += (0.48 - t) * road;
    // The shore: a pale wet band right at the water's edge, the bed dark.
    let shore = smoothstep(WATER_Y - 0.4, WATER_Y + 0.1, h) * (1.0 - smoothstep(WATER_Y + 0.5, WATER_Y + 1.1, h));
    t += (0.5 - t) * shore;
    let under = 1.0 - smoothstep(WATER_Y - 0.6, WATER_Y - 0.2, h);
    t += (0.1 - t) * under;
    t
}

// --------------------------------------------------------- mesh building ----

/// Triangles with a colour each corner and flat normals, built up piece by
/// piece and handed over as one mesh.
#[derive(Default)]
struct Builder {
    mesh: MeshData,
}

/// How far a wall's foot darkens toward the ground, and over how many
/// metres: the ground's shade on it, which the eye reads as weight.
const FOOT_DARK: f32 = 0.72;
const FOOT_RISE: f32 = 7.0;

impl Builder {
    fn vertex(&mut self, p: [f32; 3], n: [f32; 3], tone: f32) -> u32 {
        let i = self.mesh.positions.len() as u32;
        self.mesh.positions.push(p);
        self.mesh.normals.push(n);
        self.mesh.uvs.push([0.0, 0.0]);
        self.mesh.colors.push([tone, tone, tone, 1.0]);
        i
    }

    /// A flat four-sided face, counter-clockwise from the front, each corner
    /// its own tone.
    fn quad(&mut self, p: [[f32; 3]; 4], tones: [f32; 4]) {
        let n = normal(p[0], p[1], p[2]);
        let base: Vec<u32> = (0..4).map(|k| self.vertex(p[k], n, tones[k])).collect();
        self.mesh.indices.extend([base[0], base[1], base[2], base[0], base[2], base[3]]);
    }

    fn tri(&mut self, p: [[f32; 3]; 3], tone: f32) {
        let n = normal(p[0], p[1], p[2]);
        let base: Vec<u32> = (0..3).map(|k| self.vertex(p[k], n, tone)).collect();
        self.mesh.indices.extend(base);
    }

    /// An upright face from `a` to `b` (seen from the side its normal points,
    /// `a` on the left), from `y0` to `y1`, darkening toward its foot.
    fn wall_face(&mut self, a: [f32; 2], b: [f32; 2], y0: f32, y1: f32, tone: f32, foot: bool) {
        let dark = if foot { tone * FOOT_DARK } else { tone };
        let split = (y0 + FOOT_RISE).min(y1);
        if foot && split < y1 {
            self.quad([[a[0], y0, a[1]], [b[0], y0, b[1]], [b[0], split, b[1]], [a[0], split, a[1]]], [dark, dark, tone, tone]);
            self.quad([[a[0], split, a[1]], [b[0], split, b[1]], [b[0], y1, b[1]], [a[0], y1, a[1]]], [tone; 4]);
        } else {
            self.quad([[a[0], y0, a[1]], [b[0], y0, b[1]], [b[0], y1, b[1]], [a[0], y1, a[1]]], [dark, dark, tone, tone]);
        }
    }

    /// A block over the rectangle `x0..x1` by `z0..z1`, `y0` to `y1`: its
    /// four sides (their feet darkened when `foot`) and its top.
    fn block(&mut self, x: [f32; 2], y: [f32; 2], z: [f32; 2], tone: f32, foot: bool) {
        let ([x0, x1], [y0, y1], [z0, z1]) = (x, y, z);
        // South (+Z), east (+X), north (-Z), west (-X), each seen from outside.
        self.wall_face([x0, z1], [x1, z1], y0, y1, tone, foot);
        self.wall_face([x1, z1], [x1, z0], y0, y1, tone, foot);
        self.wall_face([x1, z0], [x0, z0], y0, y1, tone, foot);
        self.wall_face([x0, z0], [x0, z1], y0, y1, tone, foot);
        self.quad([[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]], [tone; 4]);
    }

    /// A block turned `yaw` radians about its own middle `c` (x, z), `half`
    /// its half width and depth, `y0` to `y1`.
    fn turned_block(&mut self, c: [f32; 2], half: [f32; 2], yaw: f32, y: [f32; 2], tone: f32) {
        let (s, co) = yaw.sin_cos();
        let at = |dx: f32, dz: f32| [c[0] + dx * co + dz * s, c[1] - dx * s + dz * co];
        let corners = [at(-half[0], half[1]), at(half[0], half[1]), at(half[0], -half[1]), at(-half[0], -half[1])];
        for k in 0..4 {
            self.wall_face(corners[k], corners[(k + 1) % 4], y[0], y[1], tone, true);
        }
        let [p0, p1, p2, p3] = corners;
        self.quad([[p0[0], y[1], p0[1]], [p1[0], y[1], p1[1]], [p2[0], y[1], p2[1]], [p3[0], y[1], p3[1]]], [tone; 4]);
    }

    /// A block over `x0..x1` by `z0..z1` from `y0` to `y1` whose foot is
    /// battered: up to `yb` each side slopes out to the ground by its own
    /// outset (`[south, east, north, west]`, 0 for a straight side), and the
    /// slopes meet at the corners, closed.
    fn battered(&mut self, x: [f32; 2], z: [f32; 2], y: [f32; 3], out: [f32; 4], tone: f32) {
        let ([x0, x1], [z0, z1], [y0, yb, y1]) = (x, z, y);
        let [os, oe, on, ow] = out;
        // The footprint's corners at the batter's top, and at the ground.
        let top = [[x0, z1], [x1, z1], [x1, z0], [x0, z0]];
        let foot = [[x0 - ow, z1 + os], [x1 + oe, z1 + os], [x1 + oe, z0 - on], [x0 - ow, z0 - on]];
        let dark = tone * FOOT_DARK;
        for k in 0..4 {
            let (a, b) = (k, (k + 1) % 4);
            self.quad(
                [[foot[a][0], y0, foot[a][1]], [foot[b][0], y0, foot[b][1]], [top[b][0], yb, top[b][1]], [top[a][0], yb, top[a][1]]],
                [dark, dark, tone, tone],
            );
            self.wall_face(top[a], top[b], yb, y1, tone, false);
        }
        self.quad([[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]], [tone; 4]);
    }

    /// A stepped plinth under a body over `x` by `z` from `y0`: each tier
    /// `(rise, out)` is a block standing out `out` past the body on the
    /// sides `sides` says (`[south, east, north, west]`), its top a lit
    /// ledge and its face a band of shadow. Returns the height reached.
    fn stepped(&mut self, x: [f32; 2], z: [f32; 2], y0: f32, tiers: &[(f32, f32)], sides: [bool; 4], tone: f32) -> f32 {
        let mut y = y0;
        for (k, &(rise, out)) in tiers.iter().enumerate() {
            let o = |on: bool| if on { out } else { 0.0 };
            let [s, e, n, w] = sides;
            self.block([x[0] - o(w), x[1] + o(e)], [y, y + rise], [z[0] - o(n), z[1] + o(s)], tone * (1.0 - 0.03 * k as f32), k == 0);
            y += rise;
        }
        y
    }

    /// A cypress: a tall six-sided column swelling a little and then
    /// tapering to a blunt point, `r` across at its widest.
    fn spire(&mut self, at: [f32; 3], r: f32, h: f32, turn: f32, tone: f32) {
        let [x, y, z] = at;
        let sides = 6;
        // Height and radius at each ring, bottom to top.
        let rings = [(0.0f32, 0.55f32), (0.12, 0.95), (0.45, 1.0), (0.78, 0.7), (0.94, 0.3), (1.0, 0.0)];
        for k in 0..sides {
            let a0 = turn + 2.0 * PI * k as f32 / sides as f32;
            let a1 = turn + 2.0 * PI * (k + 1) as f32 / sides as f32;
            for w in rings.windows(2) {
                let ((h0, r0), (h1, r1)) = (w[0], w[1]);
                let p = |a: f32, hh: f32, rr: f32| [x + r * rr * a.cos(), y + h * hh, z - r * rr * a.sin()];
                let shade = tone * (0.8 + 0.2 * h1);
                if r1 > 0.0 {
                    self.quad([p(a0, h0, r0), p(a1, h0, r0), p(a1, h1, r1), p(a0, h1, r1)], [shade; 4]);
                } else {
                    self.tri([p(a0, h0, r0), p(a1, h0, r0), p(a0, h1, 0.0)], shade);
                }
            }
        }
    }

    fn done(self) -> MeshData {
        self.mesh
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    v.map(|c| c / l)
}

/// The face normal of a counter-clockwise triangle.
fn normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    normalize(cross(sub(b, a), sub(c, a)))
}

// --------------------------------------------------------------- meshes ----

/// The terrain's extent and spacing.
const TERRAIN_X: [f32; 2] = [-260.0, 260.0];
const TERRAIN_Z: [f32; 2] = [-300.0, 220.0];
const TERRAIN_STEP: f32 = 2.0;

/// The ground: one height field over the whole place, smooth normals.
pub fn terrain() -> MeshData {
    let nx = ((TERRAIN_X[1] - TERRAIN_X[0]) / TERRAIN_STEP) as usize + 1;
    let nz = ((TERRAIN_Z[1] - TERRAIN_Z[0]) / TERRAIN_STEP) as usize + 1;
    let mut mesh = MeshData::default();
    let e = 0.5;
    for iz in 0..nz {
        for ix in 0..nx {
            let x = TERRAIN_X[0] + ix as f32 * TERRAIN_STEP;
            let z = TERRAIN_Z[0] + iz as f32 * TERRAIN_STEP;
            let h = ground(x, z);
            let dx = ground(x + e, z) - ground(x - e, z);
            let dz = ground(x, z + e) - ground(x, z - e);
            mesh.positions.push([x, h, z]);
            mesh.normals.push(normalize([-dx, 2.0 * e, -dz]));
            mesh.uvs.push([0.0, 0.0]);
            let t = ground_tone(x, z, h);
            mesh.colors.push([t, t, t, 1.0]);
        }
    }
    for iz in 0..nz - 1 {
        for ix in 0..nx - 1 {
            let a = (iz * nx + ix) as u32;
            let b = a + 1;
            let c = a + nx as u32;
            let d = c + 1;
            // Counter-clockwise seen from above (+Y): z grows toward the eye.
            mesh.indices.extend([a, c, d, a, d, b]);
        }
    }
    mesh
}

/// The river's surface: a ribbon along its middle line, u the distance
/// downstream in metres, v across it 0..1.
pub fn water() -> MeshData {
    let mut mesh = MeshData::default();
    let step = 2.0;
    let n = ((TERRAIN_X[1] - TERRAIN_X[0]) / step) as usize + 1;
    let half = RIVER_HALF + 3.0;
    let mut along = 0.0;
    let mut last = [TERRAIN_X[0], river_z(TERRAIN_X[0])];
    for i in 0..n {
        let x = TERRAIN_X[0] + i as f32 * step;
        let z = river_z(x);
        along += ((x - last[0]).powi(2) + (z - last[1]).powi(2)).sqrt();
        last = [x, z];
        for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            mesh.positions.push([x, WATER_Y, z + side * half]);
            mesh.normals.push([0.0, 1.0, 0.0]);
            mesh.uvs.push([along, k as f32]);
            mesh.colors.push([0.5, 0.5, 0.5, 1.0]);
        }
    }
    for i in 0..n - 1 {
        let a = (2 * i) as u32;
        // a: north edge (z - half), a + 1: south.
        mesh.indices.extend([a, a + 1, a + 3, a, a + 3, a + 2]);
    }
    mesh
}

/// The castle's south face: the curtain wall, its towers, the gate.
pub fn castle() -> MeshData {
    let mut b = Builder::default();
    let base = MOAT_Y - 0.5;
    let top = base + WALL_HEIGHT;
    let tone = 0.27;
    let deep = 14.0;
    let (wz0, wz1) = (WALL_Z - deep, WALL_Z);
    // Tower middles along the face, and their half size.
    let towers = [-130.0f32, -26.0, 26.0, 130.0];
    let tower_half = 13.0;
    let tower_z = [WALL_Z - deep - 6.0, WALL_Z + 8.0];
    // Curtain runs between the towers.
    let runs = [[-260.0, -130.0], [-130.0, -26.0], [26.0, 130.0], [130.0, 260.0]];
    for [x0, x1] in runs {
        let (x0, x1) = (x0 + tower_half, x1 - tower_half);
        if x1 <= x0 {
            continue;
        }
        b.block([x0, x1], [base, top], [wz0, wz1], tone, true);
        b.stepped([x0, x1], [wz0, wz1], base, &WALL_TIERS, [true, false, false, false], tone);
        // The cornice: a heavy band standing out at the top.
        b.block([x0, x1], [top - 4.0, top + 0.8], [wz0, wz1 + 2.0], tone * 1.04, false);
        // String courses: shallow bands across the face every 13 m, each a
        // thin line of shadow under it.
        for k in 1..3 {
            let y = base + 16.0 + k as f32 * 13.0;
            b.block([x0, x1], [y - 0.9, y], [wz0, wz1 + 0.6], tone, false);
        }
        // Fins up the face, from the batter to the cornice, every 12 m.
        let count = ((x1 - x0) / 12.0).floor() as i32;
        let start = x0 + ((x1 - x0) - count as f32 * 12.0) / 2.0;
        for k in 1..count {
            let fx = start + k as f32 * 12.0;
            b.block([fx - 1.3, fx + 1.3], [base + 16.0, top - 4.0], [wz1, wz1 + 2.4], tone, false);
        }
    }
    // A row of deep slits high in every bay: dark, the lit ones lit.
    for (x, _) in wall_slits() {
        let y = SLIT_Y;
        b.block([x - 0.7, x + 0.7], [y, y + SLIT_H], [wz1 - 0.1, wz1 + 0.02], 0.05, false);
    }
    // The gate between the middle towers: two jambs, a lintel, and a slot
    // 12 m wide, 26 m high and 14 m deep with the doors at its back.
    let gate_half = 6.0;
    let opening = PLATEAU_Y + 26.0;
    let inner = 26.0 - tower_half;
    for [x0, x1] in [[-inner, -gate_half], [gate_half, inner]] {
        b.block([x0, x1], [base, top + 3.0], [wz0, wz1], tone * 0.96, true);
    }
    // The lamp's chain from the slot's ceiling, and its cage.
    let (lx, lz, ly) = (0.0, WALL_Z - 7.0, LAMP_Y);
    b.block([lx - 0.12, lx + 0.12], [ly + 1.9, opening], [lz - 0.12, lz + 0.12], 0.08, false);
    b.block([lx - 0.9, lx + 0.9], [ly + 1.6, ly + 1.9], [lz - 0.9, lz + 0.9], 0.08, false);
    b.block([lx - 0.7, lx + 0.7], [ly - 0.3, ly], [lz - 0.7, lz + 0.7], 0.08, false);
    for (dx, dz) in [(-0.8f32, -0.8f32), (0.8, -0.8), (-0.8, 0.8), (0.8, 0.8)] {
        b.block([lx + dx - 0.1, lx + dx + 0.1], [ly, ly + 1.6], [lz + dz - 0.1, lz + dz + 0.1], 0.08, false);
    }
    b.block([-gate_half, gate_half], [opening, top + 3.0], [wz0, wz1], tone * 0.96, false);
    b.block([-inner, inner], [top - 2.0, top + 5.0], [wz0, wz1 + 2.6], tone * 1.02, false);
    // The doors: dark, at the back of the slot.
    b.block([-gate_half, gate_half], [PLATEAU_Y, opening], [wz0 - 1.0, wz0 + 0.4], 0.12, false);
    // The causeway: solid over the moat from its edge into the slot, walled
    // low each side, its top the road.
    b.block([-CAUSEWAY_HALF, CAUSEWAY_HALF], [MOAT_Y - 0.5, PLATEAU_Y], [wz1, MOAT_EDGE], 0.34, true);
    b.quad(
        [
            [-CAUSEWAY_HALF + 1.0, PLATEAU_Y + 0.02, MOAT_EDGE],
            [CAUSEWAY_HALF - 1.0, PLATEAU_Y + 0.02, MOAT_EDGE],
            [CAUSEWAY_HALF - 1.0, PLATEAU_Y + 0.02, wz1],
            [-CAUSEWAY_HALF + 1.0, PLATEAU_Y + 0.02, wz1],
        ],
        [0.46; 4],
    );
    b.block([-gate_half, gate_half], [MOAT_Y - 0.5, PLATEAU_Y], [wz0, wz1], 0.34, false);
    b.quad(
        [[-gate_half, PLATEAU_Y + 0.02, wz1], [gate_half, PLATEAU_Y + 0.02, wz1], [gate_half, PLATEAU_Y + 0.02, wz0], [-gate_half, PLATEAU_Y + 0.02, wz0]],
        [0.4; 4],
    );
    for side in [-1.0f32, 1.0] {
        let x = side * (CAUSEWAY_HALF - 0.5);
        b.block([x - 0.5, x + 0.5], [PLATEAU_Y, PLATEAU_Y + 1.4], [wz1 + 4.0, MOAT_EDGE], 0.34, false);
    }
    // The moat's outer wall, its top a low kerb along the edge, broken by
    // the causeway.
    for [x0, x1] in [[-260.0f32, -CAUSEWAY_HALF], [CAUSEWAY_HALF, 260.0]] {
        b.block([x0, x1], [MOAT_Y - 0.5, PLATEAU_Y + 0.5], [MOAT_EDGE - 1.0, MOAT_EDGE + 2.0], 0.3, true);
    }
    // The towers: square, battered all round, a band at the crown.
    for tx in towers {
        let (x0, x1) = (tx - tower_half, tx + tower_half);
        b.block([x0, x1], [base, base + TOWER_HEIGHT], tower_z, tone * 1.02, true);
        b.stepped([x0, x1], tower_z, base, &TOWER_TIERS, [true; 4], tone * 1.02);
        // Great panels standing proud on the east and west faces, each a
        // lit edge and a line of shadow in the raking light.
        let (pz0, pz1) = (tower_z[0] + 1.0, tower_z[1] - 1.0);
        let cols = 3;
        let rows = 4;
        let (py0, py1) = (base + 24.0, base + TOWER_HEIGHT - 7.0);
        for c in 0..cols {
            let za = pz0 + (pz1 - pz0) * c as f32 / cols as f32 + 0.6;
            let zb = pz0 + (pz1 - pz0) * (c + 1) as f32 / cols as f32 - 0.6;
            for r in 0..rows {
                let ya = py0 + (py1 - py0) * r as f32 / rows as f32 + 0.6;
                let yb = py0 + (py1 - py0) * (r + 1) as f32 / rows as f32 - 0.6;
                b.block([x0 - 0.7, x0], [ya, yb], [za, zb], tone * 1.02, false);
                b.block([x1, x1 + 0.7], [ya, yb], [za, zb], tone * 1.02, false);
            }
        }
        let crown = base + TOWER_HEIGHT;
        b.block([x0 - 1.6, x1 + 1.6], [crown - 5.0, crown + 1.0], [tower_z[0] - 1.6, tower_z[1] + 1.6], tone * 1.06, false);
        // Two deep vertical slots up the south face: dark lines its whole
        // height, with a pier standing proud between them.
        let face = tower_z[1];
        for sx in [-6.0f32, 6.0] {
            b.block([tx + sx - 1.0, tx + sx + 1.0], [base + 22.0, crown - 5.0], [face - 0.2, face + 0.01], 0.06, false);
        }
        b.block([tx - 3.0, tx + 3.0], [base + 22.0, crown - 5.0], [face, face + 1.2], tone * 1.02, false);
    }
    // The keep, a monolith behind the wall, and lesser blocks round it.
    let inside = PLATEAU_Y - 0.5;
    b.battered([-40.0, 40.0], [-228.0, -168.0], [inside, inside + 20.0, inside + 124.0], [6.0; 4], 0.3);
    b.block([-42.0, 42.0], [inside + 116.0, inside + 122.0], [-230.0, -166.0], 0.33, false);
    b.battered([60.0, 98.0], [-194.0, -156.0], [inside, inside + 14.0, inside + 84.0], [4.0; 4], 0.3);
    b.battered([-104.0, -68.0], [-182.0, -146.0], [inside, inside + 14.0, inside + 66.0], [4.0; 4], 0.3);
    // The side walls, going north from the end towers.
    for x in [-130.0f32, 130.0] {
        b.block([x - 6.0, x + 6.0], [inside, top], [-340.0, tower_z[0]], tone, false);
    }
    b.done()
}

/// Slits of lit windows high on the towers and the keep: accents.
pub fn windows() -> MeshData {
    let mut b = Builder::default();
    let base = MOAT_Y - 0.5;
    // In the towers' dark slots, a few lit.
    let face = WALL_Z + 8.0 + 0.03;
    for (k, tx) in [-130.0f32, -26.0, 26.0, 130.0].into_iter().enumerate() {
        for (j, sx) in [-6.0f32, 6.0].into_iter().enumerate() {
            for row in 0..4 {
                if (k + j + row) % 3 != 0 {
                    continue;
                }
                let (x, y) = (tx + sx, base + 40.0 + row as f32 * 8.0);
                b.quad([[x - 0.55, y, face], [x + 0.55, y, face], [x + 0.55, y + 3.6, face], [x - 0.55, y + 3.6, face]], [1.0; 4]);
            }
        }
    }
    // The lit slits in the curtain wall.
    for (x, lit) in wall_slits() {
        if lit {
            let (y, z) = (SLIT_Y + 0.6, WALL_Z + 0.04);
            b.quad([[x - 0.45, y, z], [x + 0.45, y, z], [x + 0.45, y + SLIT_H - 1.2, z], [x - 0.45, y + SLIT_H - 1.2, z]], [1.0; 4]);
        }
    }
    // A few on the keep's face, far up.
    for (x, y) in [(-14.0f32, 92.0f32), (10.0, 92.0), (-2.0, 104.0)] {
        let (y, z) = (PLATEAU_Y + y, -168.0 + 0.05);
        b.quad([[x - 0.7, y, z], [x + 0.7, y, z], [x + 0.7, y + 5.0, z], [x - 0.7, y + 5.0, z]], [1.0; 4]);
    }
    b.done()
}

/// The standing slabs on the approach and the boulders by the water.
pub fn stones() -> MeshData {
    let mut b = Builder::default();
    for (k, z) in [124.0f32, 100.0, 76.0, 54.0].into_iter().enumerate() {
        for side in [-1.0f32, 1.0] {
            let x = road_x(z) + side * 10.5;
            let h = 13.0 + 3.0 * hash2(k as i32, side as i32);
            let y = ground(x, z) - 1.0;
            let lean = 0.06 * (hash2(k as i32 + 9, side as i32) - 0.5);
            b.turned_block([x, z], [2.3, 0.75], lean, [y, y + h], 0.26);
        }
    }
    // Two great plinths at the causeway's foot, a fire bowl on each.
    for side in [-1.0f32, 1.0] {
        let x = side * (CAUSEWAY_HALF + 7.0);
        let z = MOAT_EDGE + 9.0;
        b.block([x - 4.0, x + 4.0], [PLATEAU_Y - 1.0, PLATEAU_Y + 3.4], [z - 4.0, z + 4.0], 0.3, true);
        b.block([x - 1.5, x + 1.5], [PLATEAU_Y + 3.4, PLATEAU_Y + 4.4], [z - 1.5, z + 1.5], 0.12, false);
    }
    // Bollards along the moat's edge, every 12 m.
    for k in -10..=10 {
        let x = k as f32 * 12.0 + 6.0;
        if x.abs() < CAUSEWAY_HALF + 12.0 {
            continue;
        }
        let z = MOAT_EDGE + 4.0;
        b.block([x - 0.7, x + 0.7], [PLATEAU_Y - 0.5, PLATEAU_Y + 1.6], [z - 0.7, z + 0.7], 0.3, false);
    }
    // Low blocks along the river's south bank, half sunk.
    for k in 0..9 {
        let x = -78.0 + k as f32 * 9.5 + 4.0 * hash2(k, 3);
        if x.abs() < 16.0 {
            continue;
        }
        let z = river_z(x) + RIVER_HALF + 4.0 + 3.0 * hash2(k, 4);
        let y = ground(x, z) - 1.2;
        let w = 2.0 + 2.0 * hash2(k, 5);
        b.turned_block([x, z], [w, 1.4 + hash2(k, 6)], 0.5 * hash2(k, 7), [y, y + 2.0 + 2.5 * hash2(k, 8)], 0.36);
    }
    b.done()
}

/// The quay: a battered wall along the river's north edge, from below the
/// water up to the plateau, its top a broad shelf with a low kerb.
pub fn quay() -> MeshData {
    let mut b = Builder::default();
    let tone = 0.3;
    let (y0, yb, y1) = (WATER_Y - 3.0, WATER_Y + 3.0, PLATEAU_Y);
    let step = 4.0;
    let n = ((TERRAIN_X[1] - TERRAIN_X[0]) / step) as usize;
    for i in 0..n {
        let (xa, xb) = (TERRAIN_X[0] + i as f32 * step, TERRAIN_X[0] + (i + 1) as f32 * step);
        let (za, zb) = (quay_z(xa), quay_z(xb));
        let dark = tone * FOOT_DARK;
        // The batter out into the water, then straight up.
        b.quad([[xa, y0, za + 2.5], [xb, y0, zb + 2.5], [xb, yb, zb], [xa, yb, za]], [dark, dark, tone, tone]);
        b.quad([[xa, yb, za], [xb, yb, zb], [xb, y1, zb], [xa, y1, za]], [tone; 4]);
        // The top, back to the plateau.
        let back = QUAY_DEPTH;
        b.quad([[xa, y1, za], [xb, y1, zb], [xb, y1, zb - back], [xa, y1, za - back]], [0.27; 4]);
        // The kerb, but not where the bridge lands.
        if xb.max(xa) < -BRIDGE_HALF - 0.5 || xa.min(xb) > BRIDGE_HALF + 0.5 {
            let k = 0.8;
            b.quad([[xa, y1, za + 0.4], [xb, y1, zb + 0.4], [xb, y1 + k, zb + 0.4], [xa, y1 + k, za + 0.4]], [tone * 1.05; 4]);
            b.quad([[xa, y1 + k, za + 0.4], [xb, y1 + k, zb + 0.4], [xb, y1 + k, zb - 0.6], [xa, y1 + k, za - 0.6]], [tone * 1.1; 4]);
            b.quad([[xb, y1, zb - 0.6], [xa, y1, za - 0.6], [xa, y1 + k, za - 0.6], [xb, y1 + k, zb - 0.6]], [tone * 0.9; 4]);
        }
    }
    b.done()
}

/// The ramp's top at z, from the bridge's south end down to the meadow.
pub fn ramp_y(z: f32) -> f32 {
    let t = ((z - BRIDGE_SOUTH) / (RAMP_SOUTH - BRIDGE_SOUTH)).clamp(0.0, 1.0);
    DECK_Y + (ground(0.0, RAMP_SOUTH + 2.0) - DECK_Y) * t
}

/// The bridge: a high deck of one slab level with the plateau, on two tall
/// piers, solid parapets; south of the water a ramp down to the meadow.
pub fn bridge() -> MeshData {
    let mut b = Builder::default();
    let tone = 0.36;
    let (z0, z1) = (bridge_north() - 1.0, BRIDGE_SOUTH);
    let slab = 2.8;
    b.block([-BRIDGE_HALF, BRIDGE_HALF], [DECK_Y - slab, DECK_Y], [z0, z1], tone, false);
    // The deck's walking surface, lighter, like the road.
    b.quad(
        [[-BRIDGE_HALF + 0.9, DECK_Y + 0.02, z1], [BRIDGE_HALF - 0.9, DECK_Y + 0.02, z1], [BRIDGE_HALF - 0.9, DECK_Y + 0.02, z0], [-BRIDGE_HALF + 0.9, DECK_Y + 0.02, z0]],
        [0.46; 4],
    );
    for side in [-1.0f32, 1.0] {
        let x = side * (BRIDGE_HALF - 0.45);
        b.block([x - 0.45, x + 0.45], [DECK_Y, DECK_Y + 1.3], [z0, z1], tone * 1.04, false);
    }
    let middle = river_z(0.0);
    for pz in [middle - 6.5, middle + 6.5] {
        let (y0, y1) = (WATER_Y - 3.0, DECK_Y - slab);
        b.block([-BRIDGE_HALF - 1.2, BRIDGE_HALF + 1.2], [y0, y1], [pz - 2.4, pz + 2.4], tone * 0.9, true);
        // Cutwaters: a wedge upstream and down, the full height.
        for (dir, xs) in [(-1.0f32, -BRIDGE_HALF - 1.2), (1.0, BRIDGE_HALF + 1.2)] {
            let tip = xs + dir * 4.0;
            let c = tone * 0.8;
            if dir > 0.0 {
                b.quad([[xs, y0, pz + 2.4], [tip, y0, pz], [tip, y1, pz], [xs, y1, pz + 2.4]], [c; 4]);
                b.quad([[tip, y0, pz], [xs, y0, pz - 2.4], [xs, y1, pz - 2.4], [tip, y1, pz]], [c; 4]);
                b.tri([[xs, y1, pz + 2.4], [xs, y1, pz - 2.4], [tip, y1, pz]], tone * 0.9);
            } else {
                b.quad([[tip, y0, pz], [xs, y0, pz + 2.4], [xs, y1, pz + 2.4], [tip, y1, pz]], [c; 4]);
                b.quad([[xs, y0, pz - 2.4], [tip, y0, pz], [tip, y1, pz], [xs, y1, pz - 2.4]], [c; 4]);
                b.tri([[xs, y1, pz - 2.4], [xs, y1, pz + 2.4], [tip, y1, pz]], tone * 0.9);
            }
        }
    }
    // The ramp: an embankment walled each side, sloping down to the meadow.
    let rh = BRIDGE_HALF + 0.5;
    let steps = 12;
    for i in 0..steps {
        let za = BRIDGE_SOUTH + (RAMP_SOUTH - BRIDGE_SOUTH) * i as f32 / steps as f32;
        let zb = BRIDGE_SOUTH + (RAMP_SOUTH - BRIDGE_SOUTH) * (i + 1) as f32 / steps as f32;
        let (ya, yb) = (ramp_y(za), ramp_y(zb));
        let g = |z: f32| ground(0.0, z).min(ramp_y(z)) - 2.0;
        // The top, the road.
        b.quad([[-rh + 1.0, ya + 0.02, za], [-rh + 1.0, yb + 0.02, zb], [rh - 1.0, yb + 0.02, zb], [rh - 1.0, ya + 0.02, za]].map(|p| p), [0.46; 4]);
        b.quad([[-rh, ya, za], [-rh, yb, zb], [rh, yb, zb], [rh, ya, za]], [tone; 4]);
        // Its walls, down to the ground, and the parapets on them.
        for side in [-1.0f32, 1.0] {
            let x = side * rh;
            let dark = tone * FOOT_DARK;
            if side > 0.0 {
                b.quad([[x, g(za), za], [x, g(zb), zb], [x, yb, zb], [x, ya, za]].map(|p| p), [dark, dark, tone, tone]);
            } else {
                b.quad([[x, g(zb), zb], [x, g(za), za], [x, ya, za], [x, yb, zb]], [dark, dark, tone, tone]);
            }
            let (xi, xo) = (side * (rh - 0.9), side * rh);
            let (xl, xr) = if side > 0.0 { (xi, xo) } else { (xo, xi) };
            // The parapet: top and both faces.
            b.quad([[xl, ya + 1.3, za], [xl, yb + 1.3, zb], [xr, yb + 1.3, zb], [xr, ya + 1.3, za]], [tone * 1.1; 4]);
            b.quad([[xo, ya, za], [xo, yb, zb], [xo, yb + 1.3, zb], [xo, ya + 1.3, za]].map(|p| p), [tone * 1.04; 4]);
            b.quad([[xi, yb, zb], [xi, ya, za], [xi, ya + 1.3, za], [xi, yb + 1.3, zb]], [tone * 0.95; 4]);
        }
    }
    // The south abutment under the bridge's end.
    b.block([-BRIDGE_HALF - 1.5, BRIDGE_HALF + 1.5], [ground(0.0, BRIDGE_SOUTH) - 4.0, DECK_Y], [BRIDGE_SOUTH - 6.0, BRIDGE_SOUTH], tone, true);
    // Brazier posts at the ramp's foot and on the quay where the bridge lands.
    for (z, y) in [(RAMP_SOUTH + 1.5, ramp_y(RAMP_SOUTH)), (bridge_north() - 3.0, DECK_Y)] {
        for side in [-1.0f32, 1.0] {
            let x = side * (BRIDGE_HALF + 1.2);
            b.block([x - 0.6, x + 0.6], [y - 0.5, y + 2.4], [z - 0.6, z + 0.6], 0.24, false);
        }
    }
    b.done()
}

/// Where the braziers burn: the ramp's foot, the quay where the bridge lands.
fn braziers() -> [[f32; 3]; 4] {
    let (zs, ys) = (RAMP_SOUTH + 1.5, ramp_y(RAMP_SOUTH));
    let zn = bridge_north() - 3.0;
    let x = BRIDGE_HALF + 1.2;
    [[-x, ys + 2.4, zs], [x, ys + 2.4, zs], [-x, DECK_Y + 2.4, zn], [x, DECK_Y + 2.4, zn]]
}

/// Where the fires on the great plinths burn.
fn plinth_fires() -> [[f32; 3]; 2] {
    let (x, y, z) = (CAUSEWAY_HALF + 7.0, PLATEAU_Y + 4.4, MOAT_EDGE + 9.0);
    [[-x, y, z], [x, y, z]]
}

/// Fires in the braziers and at the gate: accents.
pub fn fires() -> MeshData {
    let mut b = Builder::default();
    for [x, y, z] in braziers() {
        b.block([x - 0.45, x + 0.45], [y, y + 0.7], [z - 0.45, z + 0.45], 1.0, false);
    }
    // The hanging lamp's flame, in its cage.
    b.block([-0.55, 0.55], [LAMP_Y + 0.1, LAMP_Y + 1.4], [WALL_Z - 7.55, WALL_Z - 6.45], 1.0, false);
    // The fires on the great plinths at the causeway's foot.
    for [x, y, z] in plinth_fires() {
        b.block([x - 1.0, x + 1.0], [y, y + 0.9], [z - 1.0, z + 1.0], 1.0, false);
    }
    b.done()
}

/// The trees: great dark cones packed into masses, so a wood reads as one
/// shape with a ragged crown, never as trees one by one.
pub fn trees() -> MeshData {
    let mut b = Builder::default();
    // (middle x, z, radius of the mass, how many)
    let masses = [
        (-70.0f32, 96.0f32, 26.0f32, 30),
        (-52.0, 160.0, 34.0, 36),
        (64.0, 124.0, 30.0, 34),
        (84.0, 56.0, 18.0, 14),
        (-104.0, 30.0, 24.0, 22),
        (112.0, 6.0, 20.0, 16),
        (-190.0, -40.0, 50.0, 40),
        (200.0, -50.0, 50.0, 40),
    ];
    let mut seed = 0i32;
    for (mx, mz, r, count) in masses {
        for _ in 0..count {
            seed += 1;
            let a = 2.0 * PI * hash2(seed, 1);
            let d = r * hash2(seed, 2).sqrt();
            let x = mx + d * a.cos();
            let z = mz + d * a.sin();
            // Never on the road or in the water.
            if (x - road_x(z)).abs() < ROAD_HALF + 12.0 || (z - river_z(x)).abs() < RIVER_HALF + 6.0 {
                continue;
            }
            let y = ground(x, z) - 1.0;
            // Taller toward the mass's middle, so the crown swells.
            let middle = 1.0 - d / r;
            let h = 26.0 + 16.0 * middle + 8.0 * hash2(seed, 3);
            let w = 8.5 + 4.0 * hash2(seed, 4);
            b.spire([x, y, z], w * 0.42, h, hash2(seed, 5) * PI, 0.12);
        }
    }
    b.done()
}

/// The sky: a dome over everything, dark overhead and paler at the
/// horizon, which the dither turns to bands.
pub fn sky() -> MeshData {
    let mut mesh = MeshData::default();
    let (rings, sides) = (12usize, 32usize);
    let r = 1400.0;
    for i in 0..=rings {
        // From a little under the horizon up to the top.
        let lat = -0.15 + (PI / 2.0 + 0.15) * i as f32 / rings as f32;
        let tone = 0.30 - 0.24 * smoothstep(0.0, 1.1, lat.max(0.0));
        for k in 0..=sides {
            let lon = 2.0 * PI * k as f32 / sides as f32;
            let p = [r * lat.cos() * lon.cos(), r * lat.sin(), r * lat.cos() * lon.sin()];
            mesh.positions.push(p);
            mesh.normals.push(normalize(p).map(|c| -c));
            mesh.uvs.push([0.0, 0.0]);
            mesh.colors.push([tone, tone, tone, 1.0]);
        }
    }
    let w = (sides + 1) as u32;
    for i in 0..rings as u32 {
        for k in 0..sides as u32 {
            let a = i * w + k;
            let (b, c, d) = (a + 1, a + w, a + w + 1);
            // Seen from inside.
            mesh.indices.extend([a, b, d, a, d, c]);
        }
    }
    mesh
}

/// Every mesh of the place, by id, to upload once.
pub fn meshes() -> Vec<(u32, MeshData)> {
    vec![
        (TERRAIN_MESH, terrain()),
        (WATER_MESH, water()),
        (CASTLE_MESH, castle()),
        (STONES_MESH, stones()),
        (TREES_MESH, trees()),
        (BRIDGE_MESH, bridge()),
        (WINDOWS_MESH, windows()),
        (FIRES_MESH, fires()),
        (SKY_MESH, sky()),
        (QUAY_MESH, quay()),
    ]
}

/// A sprite's PNG as straight RGBA: `(width, height, pixels)`.
fn decode(png_bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    buffer.truncate(info.buffer_size());
    Some((info.width, info.height, buffer))
}

/// The knight's sprites as `(id, width, height, rgba)`.
pub fn sprites() -> Vec<(u32, u32, u32, Vec<u8>)> {
    [(KNIGHT_BACK_ID, KNIGHT_BACK_PNG), (KNIGHT_SIDE_ID, KNIGHT_SIDE_PNG), (KNIGHT_FRONT_ID, KNIGHT_FRONT_PNG)]
        .into_iter()
        .filter_map(|(id, bytes)| decode(bytes).map(|(w, h, rgba)| (id, w, h, rgba)))
        .collect()
}

// ---------------------------------------------------------------- scene ----

/// Where the knight stands and which way it faces (radians from -Z, the
/// way north, turning toward +X).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    pub x: f32,
    pub z: f32,
    pub heading: f32,
}

/// Whether (x, z) is in the river's channel, between the quay and the
/// south bank's top.
pub fn in_channel(x: f32, z: f32) -> bool {
    z > quay_z(x) - 1.0 && z < river_z(x) + RIVER_HALF + 10.0
}

/// The height the knight's feet are at: the deck on the bridge, the ramp,
/// the causeway over the moat, else the ground.
pub fn feet_y(x: f32, z: f32) -> f32 {
    let on = |half: f32| x.abs() < half;
    if on(BRIDGE_HALF) && (bridge_north() - 1.0..=BRIDGE_SOUTH).contains(&z) {
        DECK_Y
    } else if on(BRIDGE_HALF + 0.5) && (BRIDGE_SOUTH..=RAMP_SOUTH).contains(&z) {
        ramp_y(z)
    } else if on(CAUSEWAY_HALF) && (WALL_Z - 14.0..=MOAT_EDGE + 0.5).contains(&z) {
        PLATEAU_Y
    } else {
        ground(x, z)
    }
}

/// How the eye follows the knight: behind and above, this far back and up,
/// a narrow lens so the big things stay big.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    pub back: f32,
    pub up: f32,
    pub fov_deg: f32,
    /// Turned about the knight from straight behind (radians).
    pub orbit: f32,
}

impl Default for Follow {
    fn default() -> Self {
        Self { back: 32.0, up: 18.0, fov_deg: 40.0, orbit: 0.0 }
    }
}

/// The eye over the knight.
pub fn camera(walker: Walker, follow: Follow) -> Camera {
    let y = feet_y(walker.x, walker.z);
    let (s, c) = follow.orbit.sin_cos();
    // Behind is +Z when the orbit is 0; the orbit turns about the knight.
    let eye = [walker.x + follow.back * s, y + follow.up, walker.z + follow.back * c];
    let target = [walker.x - 6.0 * s, y + 3.0, walker.z - 6.0 * c];
    let [r, g, b] = SLATE[2].map(|v| f32::from(v) / 255.0);
    Camera {
        transform: look_at(eye, target),
        projection: Projection::Perspective { fov_y: follow.fov_deg.to_radians(), near: 1.0, far: 3000.0 },
        clear: Some([r, g, b, 1.0]),
    }
}

/// Toward the sun: low in the west, a little behind the wall, so its fins
/// rake the face. `MONOLITH_SUN=x,y,z` tries another.
pub fn sun_toward() -> [f32; 3] {
    std::env::var("MONOLITH_SUN")
        .ok()
        .and_then(|v| {
            let p: Vec<f32> = v.split(',').filter_map(|c| c.trim().parse().ok()).collect();
            (p.len() == 3).then(|| [p[0], p[1], p[2]])
        })
        .unwrap_or([-0.66, 0.46, 0.46])
}

/// The look at time `t`, the shadows round `focus`.
pub fn look(rows: u32, t: f32, focus: [f32; 3]) -> Look {
    let mut look = Look::new(MEADOW.to_vec());
    look.accent = vec![GLINT, WINDOW];
    look.dither = Dither::Bayer4;
    look.fog = Some(Fog { color: 2, start: 110.0, end: 700.0 });
    look.pixel_rows = rows;
    look.sun = Some(Sun {
        // Low in the west-south-west: long shadows across the approach, the
        // wall's fins raking the face.
        toward: sun_toward(),
        strength: 1.15,
        ambient: 0.26,
        shadow: Some(Shadow { focus, reach: 230.0 }),
        rise: 70.0,
    });
    let gate = LAMP_Y + 0.7;
    let flicker = |k: f32| 1.0 + 0.06 * (t * 7.3 + k).sin() + 0.04 * (t * 12.1 + 2.0 * k).sin();
    look.lamps = vec![Lamp { at: [0.0, gate, WALL_Z - 7.0], reach: 30.0, strength: 0.95 * flicker(0.0) }];
    for (k, [x, y, z]) in braziers().into_iter().enumerate() {
        look.lamps.push(Lamp { at: [x, y + 1.0, z], reach: 12.0, strength: 0.75 * flicker(1.3 + k as f32 * 1.7) });
    }
    for (k, [x, y, z]) in plinth_fires().into_iter().enumerate() {
        look.lamps.push(Lamp { at: [x, y + 1.5, z], reach: 16.0, strength: 0.8 * flicker(4.1 + k as f32 * 2.3) });
    }
    look.warm = Some(EMBER.to_vec());
    look.water = Some(RIVER.to_vec());
    look.zones = vec![
        // The river's banks and the quay, a band along its course.
        Zone { ramp: RIVER.to_vec(), at: [0.0, -20.0], half: [400.0, 8.0], radius: 0.0, feather: 22.0 },
        // The castle: everything past the moat's edge.
        Zone { ramp: SLATE.to_vec(), at: [0.0, MOAT_EDGE - 300.0], half: [600.0, 300.0], radius: 0.0, feather: 34.0 },
    ];
    look.time = t;
    look
}

fn placed(id: u64, mesh: u32, material: Material) -> Node {
    Node::new(id, MeshRef::Uploaded(mesh), material)
}

/// The place with the knight at `walker`, at `t` seconds, seen by `follow`.
pub fn scene(walker: Walker, follow: Follow, t: f32, rows: u32) -> Scene {
    scene_stepping(walker, follow, t, rows, true)
}

/// Whether the knight may step from `from` to `to`: not into the water,
/// off the bridge or the ramp, into the moat, or into the castle (but
/// along the causeway into the gate), never up or down more than a stair.
pub fn may_step(from: Walker, to: Walker) -> bool {
    let (x, z) = (to.x, to.z);
    let y = feet_y(x, z);
    let in_river = in_channel(x, z) && y < WATER_Y + 0.3;
    if in_river || (y - feet_y(from.x, from.z)).abs() > 1.2 {
        return false;
    }
    if z < MOAT_EDGE + 2.0 && x.abs() > CAUSEWAY_HALF - 1.0 {
        return false;
    }
    if z < WALL_Z - 12.0 {
        return false;
    }
    x.abs() < 240.0 && z < 200.0
}

/// [`scene`], with the knight's step bobbing only while `walking`.
pub fn scene_stepping(walker: Walker, follow: Follow, t: f32, rows: u32, walking: bool) -> Scene {
    let grey = |l: f32| Material::lit([l, l, l, 1.0], 0.0);
    let mut nodes = vec![
        placed(1, TERRAIN_MESH, grey(1.0)),
        placed(2, WATER_MESH, Material { role: Role::Water, ..grey(0.5) }),
        placed(3, CASTLE_MESH, grey(1.0)),
        placed(4, STONES_MESH, grey(1.0)),
        placed(5, TREES_MESH, grey(1.0)),
        placed(6, BRIDGE_MESH, grey(1.0)),
        placed(7, WINDOWS_MESH, Material { accent: true, ..Material::color([0.93, 0.68, 0.33, 1.0]) }),
        placed(8, FIRES_MESH, Material { accent: true, ..Material::color([0.93, 0.68, 0.33, 1.0]) }),
        placed(10, QUAY_MESH, grey(1.0)),
        placed(9, SKY_MESH, Material { role: Role::Sky, double_sided: true, ..Material::color([1.0; 4]) }),
    ];
    // The knight: an upright card turned to the eye, the back sprite when
    // walking away, the side one (mirrored for the left) when crossing.
    let cam = camera(walker, follow);
    let y = feet_y(walker.x, walker.z);
    let eye = cam.transform.translation;
    let face = (eye[0] - walker.x).atan2(eye[2] - walker.z);
    // The heading relative to the eye's way: 0 away from it. The stride is
    // the live game's (art_v2 `figures::knight`): the back sprite walking
    // away, else the standing sprite swapped for the running one at the
    // stride's ends, a lift and a sway on the gait, flipped to face left.
    let relative = walker.heading - follow.orbit;
    let (sideways, away) = (relative.sin(), relative.cos());
    let gait = if walking { (t * 9.0).sin() } else { 0.0 };
    let sprite = if away > 0.6 {
        KNIGHT_BACK_ID
    } else if gait.abs() > 0.55 {
        KNIGHT_SIDE_ID
    } else {
        KNIGHT_FRONT_ID
    };
    // Seen from behind there is no left or right to turn to.
    let mirror = sprite != KNIGHT_BACK_ID && sideways < -0.2;
    let height = 2.1;
    let width = height
        * match sprite {
            KNIGHT_BACK_ID => 38.0 / 44.0,
            KNIGHT_FRONT_ID => 34.0 / 44.0,
            _ => 39.0 / 44.0,
        };
    let lift = gait.abs() * 0.09;
    let sway = (gait * 3.0).to_radians();
    let knight = Material {
        source: TextureSource::Picture(sprite),
        sampling: Sampling::Nearest,
        alpha: AlphaMode::Blend,
        double_sided: true,
        role: Role::Figure,
        ..Material::default()
    };
    let mut card = Node::new(20, MeshRef::Quad, knight);
    card.transform = Transform {
        translation: [walker.x, y + height / 2.0 + lift, walker.z],
        rotation: quat_mul(
            quat_axis_angle([0.0, 1.0, 0.0], face + if mirror { PI } else { 0.0 }),
            quat_axis_angle([0.0, 0.0, 1.0], sway),
        ),
        scale: [width, height, 1.0],
    };
    nodes.push(card);
    // Its shadow on the ground: a dark oval.
    let mut shadow = Node::new(21, MeshRef::Sphere { segments: 16 }, Material::lit([0.08, 0.08, 0.08, 1.0], 0.0));
    shadow.transform = Transform {
        translation: [walker.x + 0.35, y + 0.03, walker.z - 0.1],
        scale: [1.6, 0.04, 0.8],
        ..Transform::IDENTITY
    };
    nodes.push(shadow);
    let focus = [walker.x, y, walker.z - 40.0];
    let mut look = look(rows, t, focus);
    look.reveal = Some(Reveal { at: [walker.x, y + 1.2, walker.z], radius: 6.0 });
    Scene { camera: cam, nodes, look: Some(look) }
}

/// The knight's pace on the walk, metres a second.
pub const WALK_SPEED: f32 = 4.0;

/// The walk the proof follows: up the road from the south, over the
/// bridge, up to the gate, at a walk, `t` seconds in.
pub fn walk(t: f32) -> Walker {
    let speed = WALK_SPEED;
    let start = 140.0;
    let end = WALL_Z + 4.0;
    let z = (start - speed * t).max(end);
    let x = road_x(z);
    let ahead = road_x(z - 2.0);
    Walker { x, z, heading: (ahead - x).atan2(2.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_lights_its_scene_and_is_valid() {
        let l = look(ROWS, 3.0, [0.0, 0.0, 0.0]);
        assert_eq!(l.validate(), Ok(()));
        assert!(l.lights());
        assert_eq!(l.zones.len(), 2);
        for ramp in [MEADOW, RIVER, SLATE, EMBER] {
            let levels: Vec<f32> = ramp.iter().map(|&c| angel_stage::look::luminance(angel_stage::look::unit_rgb(c))).collect();
            assert!(levels.windows(2).all(|w| w[0] < w[1]), "{levels:?}");
        }
    }

    #[test]
    fn every_mesh_is_valid_and_in_budget() {
        for (id, mesh) in meshes() {
            assert_eq!(mesh.validate(), Ok(()), "mesh {id:#x}");
            println!("mesh {id:#x}: {} vertices, {} triangles", mesh.positions.len(), mesh.indices.len() / 3);
        }
    }

    #[test]
    fn the_road_runs_over_the_bridge_and_stays_out_of_the_water() {
        for k in 0..200 {
            let z = 140.0 - k as f32;
            let x = road_x(z);
            let y = feet_y(x, z);
            assert!(!in_channel(x, z) || y > WATER_Y + 0.5, "feet in the water at z {z}: {y}");
        }
        assert_eq!(feet_y(0.0, river_z(0.0)), DECK_Y);
        assert_eq!(feet_y(0.0, MOAT_EDGE - 5.0), PLATEAU_Y);
    }

    #[test]
    fn the_knight_cannot_wade_or_climb_into_the_moat() {
        let on = |x: f32, z: f32| Walker { x, z, heading: 0.0 };
        let r = river_z(-40.0);
        assert!(!may_step(on(-40.0, r + RIVER_HALF + 12.0), on(-40.0, r)), "into the river");
        assert!(may_step(on(0.0, river_z(0.0) + 1.0), on(0.0, river_z(0.0))), "along the bridge");
        assert!(!may_step(on(20.0, MOAT_EDGE + 3.0), on(20.0, MOAT_EDGE - 3.0)), "into the moat");
        assert!(may_step(on(0.0, MOAT_EDGE + 3.0), on(0.0, MOAT_EDGE - 3.0)), "onto the causeway");
        assert!(!may_step(on(0.0, WALL_Z - 11.0), on(0.0, WALL_Z - 13.0)), "through the doors");
    }

    #[test]
    fn the_walk_ends_at_the_gate() {
        let w = walk(1000.0);
        assert!((w.z - (WALL_Z + 4.0)).abs() < 1e-3);
        assert!(w.x.abs() < 0.5);
    }
}
