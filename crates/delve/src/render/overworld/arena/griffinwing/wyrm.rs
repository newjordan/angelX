//! The Gloam Wyrm in the 3D scene: a shield of a skull with a brow that bars
//! the way, a blade of a snout and a jaw that drops, two great horns swept
//! out and up, a fan of spines raised behind the head like a halo, three
//! brass hooks that hold its lanterns out in front, and a neck that comes up
//! out of the roost in an S of dark scale with a pale belly toward us. Its
//! tells read in the silhouette before the footprints land: the spit
//! thrusts the head forward with the maw and throat burning, the coil winds
//! the neck and turns the head from the side the body comes in on, the roar
//! throws the head back with the frill spread and the throat cold-lit; a
//! broken lamp snaps a horn off, and the last one breaks the Wyrm in parts.
//!
//! Everything is read off the session by its tick (the beast's state, its
//! lamps, the records of a lamp out and of the fall), so a frame is a pure
//! function of the session. Art never sets a hitbox: the sim's own radii
//! judge every hit, and the lamps stay where the sim puts them (the hooks
//! reach for them).
use super::actors::{at, frame};
use crate::render::world3d::math::V3;
use crate::render::world3d::mesh::{Mesh, mat};
use crate::rules::griffinwing::show::What;
use crate::rules::griffinwing::trig::Det;
use crate::rules::griffinwing::{Beast, Kind, Mode, PACE, Session, St, VICTORY, Wyrm, boss, rail, rail::units};

const TAU: f32 = std::f32::consts::TAU;
/// Ticks the Wyrm takes dying, as the rules run them.
const DYING_TICKS: f32 = 60.0;
/// Ticks the wreck is worth drawing after the last lamp breaks.
pub(super) const WRECK_LIFE: u32 = 100;
/// When the last part of the Wyrm lets go (ticks after its last lamp).
const LAST_START: u32 = 56;
/// How far ahead of the head the lamps hang, and how high over it the crown
/// lamp sits, in units (the sim's own numbers).
const FRONT: f32 = 4.0;
const CROWN: f32 = 4.2;
/// How far the rail runs in a tick, in units.
fn pace() -> f32 {
    units(PACE)
}

// ─── the pose ──────────────────────────────────────────────────────────────

/// What a glow is lit with.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Glow {
    /// Unlit: dark stone.
    #[default]
    Dark,
    /// The cold light of the roost.
    Cold,
    /// A flame: gold.
    Fire,
    /// Coals: orange, the hottest.
    Coals,
}

impl Glow {
    fn mat(self) -> u8 {
        match self {
            Glow::Dark => mat::STONE_DARK,
            Glow::Cold => mat::MOONLIGHT,
            Glow::Fire => mat::FIRE,
            Glow::Coals => mat::EMBER,
        }
    }
}

/// How the Wyrm stands: its head's turn and thrust, its jaw, its frill, what
/// burns, and how far the body is wound.
#[derive(Clone, Copy, Default, Debug)]
pub(super) struct Pose {
    /// Up lifts the face, down drops it.
    pitch: f32,
    /// Toward the right is positive.
    yaw: f32,
    roll: f32,
    /// Units the head is thrust toward the eye (negative: drawn back).
    shove: f32,
    /// 0 shut, 1 gaping.
    jaw: f32,
    /// 0 laid flat, 1 spread, a little over is a bristle.
    flare: f32,
    mouth: Glow,
    eyes: Glow,
    /// 0 to 1: how hard the throat and the neck's belly burn.
    heat: f32,
    /// 0 to 1: a band of light climbing the neck toward the head (negative:
    /// none).
    wave: f32,
    /// 0 to 1: the body wound for a sweep.
    wind: f32,
    /// 0 to 1: the death's sag.
    droop: f32,
    /// A white frame: the blow just struck.
    flash: bool,
}

/// The pose of beast `b` at `tick`.
fn pose_of(b: &Beast, tick: u32) -> Pose {
    let t = tick as f32;
    let side = f32::from(b.side);
    let mut p = Pose {
        flare: 0.46 + 0.06 * (t * 0.07).dsin(),
        jaw: 0.1 + 0.05 * (t * 0.1).dsin(),
        eyes: Glow::Fire,
        wave: -1.0,
        ..Pose::default()
    };
    let ticks = b.ticks as f32;
    match b.st {
        Wyrm::Rise => {
            let k = 1.0 - (ticks / 90.0).clamp(0.0, 1.0);
            p.flare = 0.12 + 0.34 * k;
            p.jaw = 0.0;
            p.pitch = 0.3 * (1.0 - k);
        }
        Wyrm::Idle => {}
        Wyrm::Tell => {
            // The wind-up fills the last forty ticks of the tell.
            let k = (1.0 - ticks / 40.0).clamp(0.0, 1.0);
            let pulse = 0.5 + 0.5 * (t * (0.3 + 0.5 * k)).dsin();
            match b.atk {
                0 => {
                    p.pitch = -0.16 * k;
                    p.shove = 1.5 * k;
                    p.jaw = 0.18 + 0.72 * k;
                    p.mouth = Glow::Coals;
                    p.eyes = Glow::Coals;
                    p.heat = k * (0.6 + 0.4 * pulse);
                    p.wave = k;
                    p.flare = 0.55 + 0.25 * k;
                }
                1 => {
                    p.yaw = 0.34 * side * k;
                    p.roll = 0.24 * side * k;
                    p.pitch = -0.1 * k;
                    p.jaw = 0.3 + 0.25 * k;
                    p.eyes = Glow::Fire;
                    p.wind = k;
                    p.heat = k * 0.7 * (0.6 + 0.4 * pulse);
                    p.flare = 0.6 + 0.35 * k;
                }
                _ => {
                    p.pitch = 0.42 * k;
                    p.shove = -1.7 * k;
                    p.jaw = 0.25 + 0.7 * k;
                    p.mouth = Glow::Cold;
                    p.eyes = Glow::Cold;
                    p.heat = k * (0.6 + 0.4 * pulse);
                    p.wave = k;
                    p.flare = 0.55 + 0.7 * k;
                }
            }
        }
        Wyrm::Coil => {
            let k = (ticks / 22.0).clamp(0.0, 1.0);
            p.yaw = 0.5 * side * (0.4 + 0.6 * k);
            p.roll = 0.26 * side;
            p.shove = 1.9;
            p.jaw = 0.55;
            p.eyes = Glow::Fire;
            p.wind = k;
            p.flare = 0.95;
            p.heat = 0.5;
        }
        Wyrm::Roar => {
            p.pitch = 0.46;
            p.shove = -1.9;
            p.jaw = 1.0;
            p.mouth = Glow::Cold;
            p.eyes = Glow::Cold;
            p.heat = 1.0;
            p.flare = 1.25;
            p.roll = 0.05 * (t * 1.7).dsin();
        }
        Wyrm::Hurt => {
            // Reeling: thrown back and rolled, then hanging, the eyes out.
            let hit = ((ticks - 52.0) / 18.0).clamp(0.0, 1.0);
            p.shove = -2.8 * hit;
            p.roll = 0.36 * hit * (t * 0.9).dsin();
            p.pitch = -0.1 * hit + 0.1;
            p.jaw = 0.18 + 0.5 * hit;
            p.flare = 0.22 + 0.25 * hit;
            p.eyes = Glow::Dark;
            p.flash = b.ticks >= 66;
        }
        Wyrm::Dying => {
            let k = 1.0 - (ticks / DYING_TICKS).clamp(0.0, 1.0);
            p.droop = k;
            p.pitch = -1.0 * k;
            p.shove = 1.0 * k;
            p.jaw = 0.5 + 0.5 * k;
            p.eyes = Glow::Dark;
            p.flare = 0.3 * (1.0 - k);
            p.roll = 0.1 * (1.0 - k) * (t * 1.3).dsin();
        }
    }
    p
}

/// The pose of a Wyrm that has fallen and left only its parts.
fn wreck_pose() -> Pose {
    pose_of(
        &Beast {
            phase: 2,
            st: Wyrm::Dying,
            ticks: 0,
            x: 0,
            z: 0,
            lamps: [0; 3],
            marks: Vec::new(),
            gap: 0,
            side: 1,
            col: 0,
            atk: 0,
            seq: 0,
        },
        0,
    )
}

// ─── the head's frame ──────────────────────────────────────────────────────

/// The neck's pivot in the head's own coordinates (along, across, up): the
/// head turns about it.
/// How big the head is drawn against the numbers below.
const SIZE: f32 = 1.45;
const NA: f32 = 3.0;
const NC: f32 = -1.2;
/// The jaw's hinge.
const HINGE: (f32, f32) = (1.2, -1.3);

/// The head's frame: `a` runs back from the face (away from the eye), `b` to
/// the right and `c` up, turned by the pose.
#[derive(Clone, Copy)]
struct Fr {
    n: V3,
    a: V3,
    b: V3,
    c: V3,
}

impl Fr {
    fn new(head: V3, f: V3, r: V3, p: &Pose) -> Fr {
        let (mut a, mut b, mut c) = (f, r, V3::UP);
        let (sy, cy) = p.yaw.dsin_cos();
        (a, b) = (a * cy - b * sy, b * cy + a * sy);
        let (sp, cp) = p.pitch.dsin_cos();
        (a, c) = (a * cp - c * sp, c * cp + a * sp);
        let (sr, cr) = p.roll.dsin_cos();
        (b, c) = (b * cr + c * sr, c * cr - b * sr);
        Fr {
            n: head + f * (NA - p.shove) + V3::UP * NC,
            a,
            b,
            c,
        }
    }

    /// A point of the head, in its own coordinates (drawn `SIZE` times its
    /// measure about the head's middle, so it stands in the canyon's scale).
    fn p(&self, a: f32, b: f32, c: f32) -> V3 {
        self.n + self.a * (a * SIZE - NA) + self.b * (b * SIZE) + self.c * (c * SIZE - NC)
    }

    /// A point of the jaw, in the head's coordinates when shut, swung open
    /// by `alpha` about the hinge.
    fn jaw(&self, alpha: f32, a: f32, b: f32, c: f32) -> V3 {
        let (s, co) = alpha.dsin_cos();
        let (da, dc) = (a - HINGE.0, c - HINGE.1);
        self.p(HINGE.0 + da * co - dc * s, b, HINGE.1 + da * s + dc * co)
    }
}

// ─── the way a part breaks off ─────────────────────────────────────────────

/// A transform a part goes through once it is off: a turn about an axis
/// through a pivot, and a move.
#[derive(Clone, Copy)]
struct Xf {
    on: bool,
    pivot: V3,
    axis: V3,
    c: f32,
    s: f32,
    disp: V3,
}

impl Xf {
    const ID: Xf = Xf {
        on: false,
        pivot: V3::ZERO,
        axis: V3::ZERO,
        c: 1.0,
        s: 0.0,
        disp: V3::ZERO,
    };

    fn apply(&self, p: V3) -> V3 {
        if !self.on {
            return p;
        }
        let d = p - self.pivot;
        let k = self.axis;
        let r = d * self.c + k.cross(d) * self.s + k * (k.dot(d) * (1.0 - self.c));
        self.pivot + r + self.disp
    }
}

/// The parts of the Wyrm that come away, in the order a table lists them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
enum Part {
    Core,
    Jaw,
    HornL,
    HornR,
    Crest,
    HookL,
    HookR,
    FrillL,
    FrillR,
    Neck0,
    Neck1,
    Neck2,
    Neck3,
    Coil,
}

const PARTS: usize = 14;

/// When each part lets go (ticks after the last lamp), how it is thrown
/// (along, across, up in the head's axes, units a tick), what it turns about
/// (the same axes) and how fast (radians a tick), and where it turns.
struct Break {
    start: u32,
    vel: (f32, f32, f32),
    spin: (f32, f32, f32),
    rate: f32,
    pivot: (f32, f32, f32),
    /// Whether the throw and the pivot are in the rail's own axes (the body,
    /// which hangs from the roost, not from the head's turn) and not the head's.
    rail: bool,
}

const BREAKS: [Break; PARTS] = [
    // The skull: last, and slowly.
    Break { start: 44, vel: (-0.02, 0.0, -0.05), spin: (0.0, 1.0, 0.0), rate: 0.05, pivot: (0.0, 0.0, 0.0), rail: false },
    // The jaw drops off its hinge.
    Break { start: 30, vel: (-0.1, 0.0, -0.08), spin: (0.0, 1.0, 0.0), rate: 0.1, pivot: (1.2, 0.0, -1.3), rail: false },
    // The horns tumble out and up.
    Break { start: 8, vel: (-0.04, -0.3, 0.16), spin: (1.0, 0.0, 0.0), rate: 0.13, pivot: (1.0, -4.0, 4.0), rail: false },
    Break { start: 16, vel: (-0.04, 0.3, 0.16), spin: (1.0, 0.0, 0.0), rate: -0.13, pivot: (1.0, 4.0, 4.0), rail: false },
    Break { start: 22, vel: (-0.14, 0.0, 0.2), spin: (0.0, 1.0, 0.0), rate: -0.12, pivot: (-1.0, 0.0, 5.0), rail: false },
    // The hooks go with the lamps they held.
    Break { start: 12, vel: (-0.1, -0.2, -0.04), spin: (0.0, 0.0, 1.0), rate: 0.1, pivot: (-1.0, -4.0, 0.5), rail: false },
    Break { start: 20, vel: (-0.1, 0.2, -0.04), spin: (0.0, 0.0, 1.0), rate: -0.1, pivot: (-1.0, 4.0, 0.5), rail: false },
    // The frill folds away behind.
    Break { start: 26, vel: (0.1, -0.16, -0.02), spin: (0.0, 0.0, 1.0), rate: 0.07, pivot: (3.0, -3.0, 4.0), rail: false },
    Break { start: 34, vel: (0.1, 0.16, -0.02), spin: (0.0, 0.0, 1.0), rate: -0.07, pivot: (3.0, 3.0, 4.0), rail: false },
    // The neck lets go in lengths, head first.
    Break { start: 38, vel: (0.02, -0.06, -0.1), spin: (1.0, 0.0, 0.0), rate: 0.04, pivot: (3.4, 0.0, -4.6), rail: true },
    Break { start: 44, vel: (0.0, 0.07, -0.12), spin: (1.0, 0.0, 0.0), rate: -0.04, pivot: (4.4, 3.6, -9.0), rail: true },
    Break { start: 50, vel: (0.0, -0.06, -0.14), spin: (1.0, 0.0, 0.0), rate: 0.03, pivot: (5.6, -3.6, -13.6), rail: true },
    Break { start: 56, vel: (0.0, 0.06, -0.16), spin: (1.0, 0.0, 0.0), rate: -0.03, pivot: (6.6, 3.2, -18.4), rail: true },
    Break { start: 52, vel: (0.04, 0.12, -0.14), spin: (0.0, 0.0, 1.0), rate: 0.03, pivot: (8.0, -9.0, -9.0), rail: true },
];

/// Where the parts that still hold on at the end let go, and when: the tick
/// after the last lamp, and the place as fixed-point (along, across, up) from
/// the head's middle, for the blasts that ride the body down. The horns are
/// not here: a lamp struck each of them off before.
pub(super) fn break_points() -> Vec<(u32, [i32; 3])> {
    let q = |v: f32| (v * 256.0) as i32;
    [
        Part::HookL,
        Part::HookR,
        Part::Crest,
        Part::FrillL,
        Part::FrillR,
        Part::Jaw,
        Part::Neck0,
        Part::Core,
        Part::Neck1,
        Part::Coil,
        Part::Neck2,
        Part::Neck3,
    ]
    .into_iter()
    .map(|part| {
        let b = &BREAKS[part as usize];
        {
            let k = if b.rail { 1.0 } else { SIZE };
            (b.start, [q(b.pivot.0 * k), q(b.pivot.1 * k), q(b.pivot.2 * k)])
        }
    })
    .collect()
}

/// Gravity on a part that has let go, units a tick a tick.
const G: f32 = 0.03;
/// Ticks after it lets go that a part has fallen out of the picture.
const GONE_AFTER: u32 = 46;

/// Where the Wyrm stands for one frame: the head's place, its rail frame, its
/// pose and, once it has fallen, how long ago.
struct Scene {
    head: V3,
    f: V3,
    r: V3,
    pose: Pose,
    /// Ticks since the last lamp broke (None: not falling).
    age: Option<u32>,
    /// How many lamps are out (0 to 3), and when the last two broke.
    out: u32,
    horn_cut: [Option<u32>; 2],
    tick: u32,
    side: f32,
    phase: u8,
    atk: u8,
    st: Wyrm,
    /// The lamps' places in the world, left, right, crown.
    lamps: [V3; 3],
    /// How far the coil's slabs are in (0 to 1).
    sweep: f32,
}

impl Scene {
    /// The transform of `part` as the fall's age is.
    fn xf(&self, part: Part, fr: &Fr) -> Xf {
        match self.age {
            Some(age) => self.xf_at(part, fr, age),
            None => Xf::ID,
        }
    }

    /// The transform of `part` `age` ticks after the last lamp broke. The path
    /// a part takes is a guide of its own: a throw along the head's axes, the
    /// world's gravity and the pace the head stopped holding, which
    /// `guides` draws for the shots.
    fn xf_at(&self, part: Part, fr: &Fr, age: u32) -> Xf {
        let br = &BREAKS[part as usize];
        if age < br.start {
            return Xf::ID;
        }
        if age >= br.start + GONE_AFTER {
            // Long gone: out of the picture, far down.
            return Xf { on: true, pivot: fr.n, axis: V3::UP, c: 1.0, s: 0.0, disp: V3::UP * -200.0 };
        }
        let t = (age - br.start) as f32;
        let (ax, bx, cx) = self.axes(br, fr);
        let vel = ax * br.vel.0 + bx * br.vel.1 + cx * br.vel.2;
        // A part that has let go stays where it was in the world, not on the
        // head that goes on holding its pace ahead of the wing.
        let held = pace() * (age.min(DYING_TICKS as u32) as f32 - br.start.min(DYING_TICKS as u32) as f32);
        let disp = vel * t - self.f * held + V3::UP * (-0.5 * G * t * t);
        let axis = (ax * br.spin.0 + bx * br.spin.1 + cx * br.spin.2).normalize();
        let (s, c) = (br.rate * t).dsin_cos();
        Xf {
            on: true,
            pivot: self.pivot_of(br, fr),
            axis,
            c,
            s,
            disp,
        }
    }

    /// The axes a part's throw and turn are in: the head's own or the rail's.
    fn axes(&self, br: &Break, fr: &Fr) -> (V3, V3, V3) {
        if br.rail {
            (self.f, self.r, V3::UP)
        } else {
            (fr.a, fr.b, fr.c)
        }
    }

    /// Where a part turns, in the world.
    fn pivot_of(&self, br: &Break, fr: &Fr) -> V3 {
        let (a, b, c) = br.pivot;
        if br.rail {
            self.head + self.f * a + self.r * b + V3::UP * c
        } else {
            fr.p(a, b, c)
        }
    }
}

// ─── the emitters ──────────────────────────────────────────────────────────

/// Triangles into a mesh, through the transform of the part being built.
struct Out<'a> {
    m: &'a mut Mesh,
    xf: Xf,
}

impl Out<'_> {
    fn tri(&mut self, a: V3, b: V3, c: V3, uv: [[f32; 2]; 3], material: u8) {
        let v = [self.xf.apply(a), self.xf.apply(b), self.xf.apply(c)];
        self.m.push_tri(v, uv, material);
    }

    /// A quad a, b, c, d with its own uv from the edge lengths.
    fn quad(&mut self, a: V3, b: V3, c: V3, d: V3, material: u8) {
        let (u, v) = ((b - a).length(), (d - a).length());
        self.tri(a, b, c, [[0.0, 0.0], [u, 0.0], [u, v]], material);
        self.tri(a, c, d, [[0.0, 0.0], [u, v], [0.0, v]], material);
    }

    /// A quad with uvs given at the corners.
    fn quad_uv(&mut self, p: [V3; 4], uv: [[f32; 2]; 4], material: u8) {
        self.tri(p[0], p[1], p[2], [uv[0], uv[1], uv[2]], material);
        self.tri(p[0], p[2], p[3], [uv[0], uv[2], uv[3]], material);
    }

    /// A box between two quads of four corners each, in the same order: its
    /// four long faces, and the ends where asked.
    fn prism(&mut self, c0: [V3; 4], c1: [V3; 4], caps: (bool, bool), material: u8) {
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.quad(c0[i], c0[j], c1[j], c1[i], material);
        }
        if caps.0 {
            self.quad(c0[0], c0[1], c0[2], c0[3], material);
        }
        if caps.1 {
            self.quad(c1[0], c1[1], c1[2], c1[3], material);
        }
    }

    /// A four-sided spike from a square at `base` (half a width `w`, laid
    /// across `side`) to `tip`.
    fn spike(&mut self, base: V3, tip: V3, w: f32, side: V3, material: u8) {
        let axis = (tip - base).normalize();
        let s = (side - axis * side.dot(axis)).normalize() * w;
        let t = axis.cross(s).normalize() * w;
        let c = [base + s + t, base - s + t, base - s - t, base + s - t];
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.tri(c[i], c[j], tip, [[0.0, 0.0], [w * 2.0, 0.0], [w, (tip - base).length()]], material);
        }
    }
}

/// A Catmull-Rom curve through `pts`, `per` samples to a span.
fn spline(pts: &[V3], per: usize) -> Vec<V3> {
    let n = pts.len();
    let mut out = Vec::with_capacity((n - 1) * per + 1);
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = pts[(i + 2).min(n - 1)];
        for k in 0..per {
            let t = k as f32 / per as f32;
            let (t2, t3) = (t * t, t * t * t);
            let q = (p1 * 2.0
                + (p2 - p0) * t
                + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
                + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3)
                * 0.5;
            out.push(q);
        }
    }
    out.push(pts[n - 1]);
    out
}

/// A ring of a tube: its centre, its two axes and its radius.
#[derive(Clone, Copy)]
struct Ring {
    p: V3,
    n1: V3,
    n2: V3,
    r: f32,
}

/// Rings along `path`, the radius from `radius(t)` (t from 0 to 1 along it),
/// the first axis kept as near `across` as the path lets it.
fn rings(path: &[V3], across: V3, radius: impl Fn(f32) -> f32) -> Vec<Ring> {
    let n = path.len();
    (0..n)
        .map(|i| {
            let t = (path[(i + 1).min(n - 1)] - path[i.saturating_sub(1)]).normalize();
            let mut n1 = across - t * across.dot(t);
            if n1.length() < 1e-3 {
                n1 = V3::UP - t * V3::UP.dot(t);
            }
            let n1 = n1.normalize();
            Ring {
                p: path[i],
                n1,
                n2: t.cross(n1).normalize(),
                r: radius(i as f32 / (n - 1).max(1) as f32),
            }
        })
        .collect()
}

/// The unit circle in `sides` steps.
fn circle(sides: usize) -> Vec<(f32, f32)> {
    (0..sides)
        .map(|j| (j as f32 / sides as f32 * TAU).dsin_cos())
        .collect()
}

/// What a face of a tube is made of: by its ring, whether it looks at the
/// eye, and how it burns.
type Skin<'a> = &'a dyn Fn(usize, bool) -> u8;

/// A tube along `rs[from..=to]`: its faces are `skin(ring, belly)` where
/// belly is whether the face looks toward the eye.
#[allow(clippy::too_many_arguments)]
fn tube(o: &mut Out, rs: &[Ring], from: usize, to: usize, sides: usize, to_eye: V3, scale: f32, skin: Skin) {
    let dirs = circle(sides);
    let to = to.min(rs.len() - 1);
    if from >= to {
        return;
    }
    let mut v = 0.0f32;
    for i in from..to {
        let (a, b) = (&rs[i], &rs[i + 1]);
        let seg = (b.p - a.p).length();
        let pt = |ring: &Ring, j: usize| {
            let (s, c) = dirs[j % sides];
            ring.p + ring.n1 * (c * ring.r) + ring.n2 * (s * ring.r)
        };
        let circ = a.r.max(b.r) * TAU / sides as f32 * scale;
        for j in 0..sides {
            let mid = {
                let (s0, c0) = dirs[j];
                let (s1, c1) = dirs[(j + 1) % sides];
                let (ms, mc) = (s0 + s1, c0 + c1);
                (a.n1 * mc + a.n2 * ms).normalize()
            };
            let belly = mid.dot(to_eye) > 0.78;
            let material = skin(i, belly);
            let (u0, u1) = (j as f32 * circ, (j + 1) as f32 * circ);
            let (v0, v1) = (v * scale, (v + seg) * scale);
            o.quad_uv(
                [pt(a, j), pt(a, j + 1), pt(b, j + 1), pt(b, j)],
                [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
                material,
            );
        }
        v += seg;
    }
}

/// A rounded cap of spikes on a tube's last ring (a horn's tip, a coil's end).
fn point(o: &mut Out, r: &Ring, tip: V3, sides: usize, material: u8) {
    let dirs = circle(sides);
    for j in 0..sides {
        let (s0, c0) = dirs[j];
        let (s1, c1) = dirs[(j + 1) % sides];
        let a = r.p + r.n1 * (c0 * r.r) + r.n2 * (s0 * r.r);
        let b = r.p + r.n1 * (c1 * r.r) + r.n2 * (s1 * r.r);
        o.tri(a, b, tip, [[0.0, 0.0], [r.r, 0.0], [r.r * 0.5, r.r * 2.0]], material);
    }
}

// ─── the head ──────────────────────────────────────────────────────────────

/// The angles of the frill's spines from straight up, and how long each runs.
const FRILL: [(f32, f32); 7] = [
    (-1.3, 4.2),
    (-0.88, 5.4),
    (-0.44, 6.2),
    (0.0, 6.6),
    (0.44, 6.2),
    (0.88, 5.4),
    (1.3, 4.2),
];

/// The skin of the head's scales, plates, bone and hooks.
const SCALE: u8 = mat::STABLE_ROOF;
const PLATE: u8 = mat::STABLE_WALL;
const HORN: u8 = mat::STABLE_TIMBER;
const BELLY: u8 = mat::STABLE_FLOOR;
const WEB: u8 = mat::TACK_LEATHER;
/// The coil's slab, near the eye: the scale in a darker coat.
const SLAB: u8 = mat::HAY;

fn horn_path(fr: &Fr, side: f32) -> Vec<V3> {
    let s = side;
    spline(
        &[
            fr.p(1.4, 2.4 * s, 1.8),
            fr.p(1.0, 4.2 * s, 2.5),
            fr.p(0.4, 5.9 * s, 3.9),
            fr.p(-0.6, 6.5 * s, 5.5),
            fr.p(-1.8, 5.7 * s, 6.9),
        ],
        2,
    )
}

fn horn_rings(fr: &Fr, side: f32) -> Vec<Ring> {
    let path = horn_path(fr, side);
    rings(&path, fr.a, |t| 0.95 * (1.0 - t) + 0.08)
}

/// The head: skull, brow, eyes, snout, jaw, teeth, horns, crest and hooks.
fn skull(o: &mut Out, sc: &Scene, fr: &Fr, rs: &Rings) {
    let p = &sc.pose;
    let alpha = p.jaw * 0.62;
    let eye = p.eyes.mat();
    let eye = if sc.age.is_some() { Glow::Dark.mat() } else { eye };
    let mouth = p.mouth.mat();
    let stone = mat::STONE_DARK;
    let fp = |a: f32, b: f32, c: f32| fr.p(a, b, c);
    let flashed = |m: u8| if p.flash { mat::MOONLIGHT } else { m };

    // The core: cranium, snout, brows, eyes, nostrils, the roof of the mouth
    // and its teeth.
    o.xf = sc.xf(Part::Core, fr);
    o.prism(
        [fp(2.6, -2.3, 3.0), fp(2.6, 2.3, 3.0), fp(2.6, 3.3, -1.6), fp(2.6, -3.3, -1.6)],
        [fp(-1.0, -1.9, 2.3), fp(-1.0, 1.9, 2.3), fp(-1.0, 2.7, -1.3), fp(-1.0, -2.7, -1.3)],
        (true, false),
        flashed(SCALE),
    );
    o.prism(
        [fp(-0.9, -1.5, 0.3), fp(-0.9, 1.5, 0.3), fp(-0.9, 1.5, -1.1), fp(-0.9, -1.5, -1.1)],
        [fp(-4.4, -0.9, 0.3), fp(-4.4, 0.9, 0.3), fp(-4.4, 0.9, -0.9), fp(-4.4, -0.9, -0.9)],
        (false, true),
        flashed(PLATE),
    );
    // A ridge down the nose, and the brows barred over the eyes.
    o.prism(
        [fp(-0.5, -0.35, 2.5), fp(-0.5, 0.35, 2.5), fp(-0.5, 0.35, 0.3), fp(-0.5, -0.35, 0.3)],
        [fp(-4.0, -0.3, 0.6), fp(-4.0, 0.3, 0.6), fp(-4.0, 0.3, 0.3), fp(-4.0, -0.3, 0.3)],
        (false, true),
        flashed(HORN),
    );
    for s in [-1.0f32, 1.0] {
        o.prism(
            [fp(-0.5, 2.9 * s, 1.55), fp(-1.3, 2.9 * s, 1.55), fp(-1.3, 2.9 * s, 2.35), fp(-0.5, 2.9 * s, 2.35)],
            [fp(-1.1, 0.5 * s, 0.95), fp(-1.9, 0.5 * s, 0.95), fp(-1.9, 0.5 * s, 1.75), fp(-1.1, 0.5 * s, 1.75)],
            (true, true),
            flashed(PLATE),
        );
        // The eye: a slit slanted under the brow.
        o.quad(
            fp(-1.45, 0.8 * s, 0.55),
            fp(-1.45, 2.3 * s, 0.95),
            fp(-1.45, 2.3 * s, 1.35),
            fp(-1.45, 0.8 * s, 0.95),
            eye,
        );
        // A nostril.
        o.quad(
            fp(-4.45, 0.62 * s, -0.05),
            fp(-4.45, 0.28 * s, -0.05),
            fp(-4.45, 0.28 * s, 0.22),
            fp(-4.45, 0.62 * s, 0.22),
            if p.mouth == Glow::Coals { mat::EMBER } else { stone },
        );
        // Upper teeth along the lip, long fangs at the front.
        for (a, b, len) in [(-4.15f32, 0.72f32, 1.7f32), (-3.2, 1.0, 0.9), (-2.1, 1.35, 0.8), (-1.0, 1.55, 0.7)] {
            let base = fp(a, b * s, -1.0);
            o.spike(base, base - fr.c * len, 0.2 + len * 0.06, fr.b, HORN);
        }
        // Spikes along the cheek, swept out and back: the skull's outline is
        // no box.
        for (a, b0, c0, tb, ta, tc) in [
            (2.2f32, 3.1f32, 0.6f32, 5.4f32, 3.3f32, 1.4f32),
            (1.0, 3.0, -0.2, 5.0, 1.2, -0.3),
            (-0.2, 2.7, -0.9, 4.2, -0.8, -1.5),
        ] {
            o.spike(fp(a, b0 * s, c0), fp(ta, tb * s, tc), 0.42, fr.c, HORN);
        }
    }
    // The roof of the mouth: the maw seen when the jaw is down.
    o.quad(
        fp(-0.4, -1.5, -1.15),
        fp(-0.4, 1.5, -1.15),
        fp(-4.3, 0.85, -0.88),
        fp(-4.3, -0.85, -0.88),
        stone,
    );

    // The jaw: a wedge from its hinge to a blunt chin, its teeth rising, the
    // throat and tongue lit as the tell lights them.
    o.xf = sc.xf(Part::Jaw, fr);
    let jp = |a: f32, b: f32, c: f32| fr.jaw(alpha, a, b, c);
    o.prism(
        [jp(1.2, -2.3, -1.2), jp(1.2, 2.3, -1.2), jp(1.2, 2.3, -2.3), jp(1.2, -2.3, -2.3)],
        [jp(-4.0, -0.95, -0.95), jp(-4.0, 0.95, -0.95), jp(-4.0, 0.95, -1.7), jp(-4.0, -0.95, -1.7)],
        (false, true),
        flashed(PLATE),
    );
    // The floor of the mouth, lit by the throat.
    o.quad(jp(1.0, -1.7, -1.2), jp(1.0, 1.7, -1.2), jp(-3.9, 0.85, -0.95), jp(-3.9, -0.85, -0.95), if p.heat > 0.2 { mouth } else { stone });
    // The throat, lit.
    o.quad(
        fr.p(0.6, -1.7, -1.1),
        fr.p(0.6, 1.7, -1.1),
        fr.jaw(alpha, 0.6, 1.7, -1.2),
        fr.jaw(alpha, 0.6, -1.7, -1.2),
        if p.mouth == Glow::Dark { stone } else { mouth },
    );
    for s in [-1.0f32, 1.0] {
        for (a, len) in [(-3.8f32, 1.3f32), (-2.4, 0.8), (-1.0, 0.7)] {
            let b = if a < -3.0 { 0.78 } else { 1.4 } * s;
            let base = jp(a, b, -1.0);
            o.spike(base, jp(a, b, -1.0 + len), 0.2 + len * 0.05, fr.b, HORN);
        }
    }

    // A barb under the chin.
    o.spike(jp(-3.9, 0.0, -1.7), jp(-4.5, 0.0, -3.5), 0.42, fr.b, HORN);

    // The horns.
    for (k, (part, side)) in [(Part::HornL, -1.0f32), (Part::HornR, 1.0)].into_iter().enumerate() {
        o.xf = sc.xf(part, fr);
        let hr = &rs.horns[k];
        let keep = match sc.horn_cut[k] {
            Some(_) => 5,
            None => hr.len() - 1,
        };
        // A collar of dark stone at the root.
        tube(o, hr, 0, 1, 5, -fr.a, 1.0, &|_, _| stone);
        tube(o, hr, 1, keep, 5, -fr.a, 1.0, &|_, _| flashed(HORN));
        if sc.horn_cut[k].is_some() {
            // The stump, charred where it was struck off.
            point(o, &hr[keep], hr[keep].p + fr.c * 0.2, 5, mat::EMBER);
        } else {
            point(o, &hr[hr.len() - 1], hr[hr.len() - 1].p + (hr[hr.len() - 1].p - hr[hr.len() - 2].p).normalize() * 0.9, 5, HORN);
        }
        let _ = side;
    }

    // The crest horn, and the hooks that hold the lanterns out, the first
    // lamp's hook dark once it is out.
    o.xf = sc.xf(Part::Crest, fr);
    tube(o, &rs.crest, 0, rs.crest.len() - 1, 5, -fr.a, 1.0, &|_, _| flashed(HORN));
    cradle(o, fr, sc.lamps[2], 0.0);
    for (k, part) in [Part::HookL, Part::HookR].into_iter().enumerate() {
        o.xf = sc.xf(part, fr);
        let lit = sc.out as usize <= k;
        let m = if lit { mat::BRASS } else { stone };
        tube(o, &rs.hooks[k], 0, rs.hooks[k].len() - 1, 4, -fr.a, 1.0, &|_, _| m);
        cradle(o, fr, sc.lamps[k], 0.0);
    }

    // The frill.
    spread(o, sc, fr, p.flare);
}

/// A brass cradle round a lamp's place, open toward the eye.
fn cradle(o: &mut Out, fr: &Fr, at_: V3, lift: f32) {
    let c = at_ + fr.a * 1.2 + fr.c * lift;
    let (w, h) = (0.55, 0.5);
    o.prism(
        [c + fr.b * w + fr.c * h, c - fr.b * w + fr.c * h, c - fr.b * w - fr.c * h, c + fr.b * w - fr.c * h],
        [c - fr.a * 0.9 + fr.b * (w + 0.3) + fr.c * (h + 0.3), c - fr.a * 0.9 - fr.b * (w + 0.3) + fr.c * (h + 0.3), c - fr.a * 0.9 - fr.b * (w + 0.3) - fr.c * (h + 0.3), c - fr.a * 0.9 + fr.b * (w + 0.3) - fr.c * (h + 0.3)],
        (false, false),
        mat::BRASS,
    );
}

/// The fan of spines behind the head, and the web between them.
fn spread(o: &mut Out, sc: &Scene, fr: &Fr, flare: f32) {
    let f = flare.clamp(0.0, 1.4);
    let hub = fr.p(2.7, 0.0, 0.3);
    let torn = sc.out; // a spine goes with each lamp
    let mut tips: Vec<Option<(V3, V3)>> = Vec::new();
    for (i, &(phi, len)) in FRILL.iter().enumerate() {
        let (sp, cp) = (phi * (0.75 + 0.25 * f)).dsin_cos();
        let reach = len * (0.62 + 0.4 * f);
        let up = fr.b * sp + fr.c * cp;
        let base = hub + up * 2.6;
        let tip = hub + up * (2.6 + reach) + fr.a * (1.6 + 1.2 * (1.0 - f).max(0.0) + reach * 0.12);
        // The outermost spines are the first to go.
        let gone = (i == 0 && torn >= 1) || (i == 6 && torn >= 2) || (i == 1 && torn >= 3);
        tips.push(if gone { None } else { Some((base, tip)) });
    }
    for (i, t) in tips.iter().enumerate() {
        let part = if i < 4 { Part::FrillL } else { Part::FrillR };
        o.xf = sc.xf(part, fr);
        let Some((base, tip)) = t else { continue };
        o.spike(*base, *tip, 0.5, fr.a, HORN);
        if let Some(Some((b2, t2))) = tips.get(i + 1) {
            // The web between two spines, scalloped.
            let mid = (*base + *b2) * 0.5;
            let tm = (*tip + *t2) * 0.5;
            let notch = mid.lerp(tm, 0.52);
            o.tri(*base, *tip, notch, [[0.0, 0.0], [3.0, 0.0], [1.0, 3.0]], WEB);
            o.tri(*b2, *t2, notch, [[0.0, 0.0], [3.0, 0.0], [1.0, 3.0]], WEB);
            o.tri(*base, *b2, notch, [[0.0, 0.0], [3.0, 0.0], [1.0, 3.0]], WEB);
        }
    }
}

// ─── the neck and the coils ────────────────────────────────────────────────

/// Every ring path of the Wyrm built once a frame.
struct Rings {
    horns: [Vec<Ring>; 2],
    crest: Vec<Ring>,
    hooks: [Vec<Ring>; 2],
    neck: Vec<Ring>,
    loop_: Vec<Ring>,
}

fn rings_for(sc: &Scene, fr: &Fr, nominal: V3) -> Rings {
    let (f, r) = (sc.f, sc.r);
    let t = sc.tick as f32;
    let p = &sc.pose;
    let wind = p.wind;
    // The roost's coils: the neck bends in an S out of the mist, more
    // widely the tighter the body winds, and sags in death.
    let sway = |k: f32, amp: f32| (t * 0.045 + k).dsin() * amp * (1.0 + 1.1 * wind);
    let at_ = |a: f32, b: f32, c: f32| nominal + f * a + r * b + V3::UP * c;
    let droop = p.droop;
    let neck_pts = [
        fr.p(2.4, 0.0, -1.0),
        at_(3.4, sway(0.0, 0.6), -4.6 - droop * 1.0),
        at_(4.4 + wind * 1.0, 3.6 + sway(1.2, 1.4), -9.0 - droop * 2.0),
        at_(5.6, -3.6 + sway(2.4, 1.6), -13.6 - droop * 2.0),
        at_(6.6, 3.2 + sway(3.6, 1.4), -18.4),
        at_(8.0, -1.0, -25.0),
    ];
    let path = spline(&neck_pts, 3);
    let neck = rings(&path, r, |t| 3.0 + 1.0 * t);
    let coil_pts = [
        at_(5.0, -3.0, -18.0),
        at_(5.0, -12.0 + sway(0.6, 1.0), -14.0),
        at_(8.0, -13.0, -8.0),
        at_(10.5, -7.0 + sway(1.9, 1.0), -5.5),
        at_(12.0, -1.5, -9.0),
    ];
    let loop_ = rings(&spline(&coil_pts, 3), r, |t| 2.6 - 1.2 * t);
    let horns = [horn_rings(fr, -1.0), horn_rings(fr, 1.0)];
    let crest = rings(
        &spline(
            &[fr.p(0.8, 0.0, 2.6), fr.p(0.2, 0.0, 3.4), fr.p(-1.0, 0.0, 3.5), sc.lamps[2] + f * 1.4 + V3::UP * 0.3],
            2,
        ),
        fr.b,
        |t| 0.7 * (1.0 - 0.45 * t),
    );
    let hook = |side: f32, lamp: V3| {
        rings(
            &spline(
                &[fr.p(1.2, 2.9 * side, -0.1), fr.p(-0.4, 4.3 * side, -0.5), fr.p(-1.6, 5.0 * side, 0.4), lamp + f * 1.6 - V3::UP * 0.1],
                2,
            ),
            fr.c,
            |t| 0.6 * (1.0 - 0.4 * t),
        )
    };
    let hooks = [hook(-1.0, sc.lamps[0]), hook(1.0, sc.lamps[1])];
    Rings { horns, crest, hooks, neck, loop_ }
}

/// The neck and the coil behind it.
fn body(o: &mut Out, sc: &Scene, fr: &Fr, rs: &Rings) {
    let p = &sc.pose;
    let n = rs.neck.len();
    // Chunks the neck breaks in (ring ranges).
    let chunks = [
        (Part::Neck0, 0, n * 3 / 10),
        (Part::Neck1, n * 3 / 10, n * 5 / 10),
        (Part::Neck2, n * 5 / 10, n * 7 / 10),
        (Part::Neck3, n * 7 / 10, n - 1),
    ];
    let eye_dir = -sc.f;
    for (part, a, b) in chunks {
        o.xf = sc.xf(part, fr);
        let skin = |i: usize, belly: bool| -> u8 {
            if !belly {
                return SCALE;
            }
            let t = i as f32 / (n - 1) as f32;
            // A band of light climbing the neck to the head, and the whole
            // belly smouldering as the heat rises.
            let climbing = p.wave >= 0.0 && ((1.0 - t) - p.wave * 1.15).abs() < 0.09;
            let burning = p.heat > 0.5 && t < 0.2;
            if climbing || burning {
                match p.mouth {
                    Glow::Cold => mat::MOONLIGHT,
                    Glow::Coals => mat::EMBER,
                    _ => {
                        if p.wind > 0.0 {
                            mat::EMBER
                        } else {
                            BELLY
                        }
                    }
                }
            } else {
                BELLY
            }
        };
        tube(o, &rs.neck, a, b, 8, eye_dir, 1.0, &skin);
        // Spikes down both edges of the neck, outward.
        for i in (a..b).step_by(2) {
            let ring = &rs.neck[i];
            for s in [-1.0f32, 1.0] {
                let base = ring.p + ring.n1 * (ring.r * 0.85 * s);
                o.spike(base, base + ring.n1 * (1.5 * s) + ring.n2 * 0.3, 0.38, ring.n2, HORN);
            }
        }
    }
    // The coil behind, in the roost's mist.
    o.xf = sc.xf(Part::Coil, fr);
    let m = rs.loop_.len();
    let skin = |i: usize, belly: bool| if belly && i % 3 == 0 { BELLY } else { SCALE };
    tube(o, &rs.loop_, 0, m - 1, 8, eye_dir, 1.0, &skin);
    for i in (1..m - 1).step_by(2) {
        let ring = &rs.loop_[i];
        let base = ring.p + ring.n2 * (ring.r * 0.85);
        o.spike(base, base + ring.n2 * 1.5, 0.4, ring.n1, HORN);
    }
    let last = rs.loop_[m - 1];
    point(o, &last, last.p + (last.p - rs.loop_[m - 2].p).normalize() * 1.6, 8, SCALE);
}

// ─── the Wyrm in the scene ─────────────────────────────────────────────────

/// How many of the Wyrm's three lamps are out.
fn lamps_out(s: &Session) -> u32 {
    s.foes.iter().filter(|f| f.kind == Kind::Lamp && f.st == St::Broken).count() as u32
}

/// The tick a lamp was broken, by how many were out when it went.
fn lamp_born(s: &Session, n: u32) -> Option<u32> {
    s.show
        .fx
        .iter()
        .find(|f| f.what == What::LampOut && u32::from(f.n) == n)
        .map(|f| f.born)
}

/// The age of the Wyrm's fall (ticks since its last lamp broke), and where
/// its head was when it did (along, across, up): `None` when it has not
/// fallen or the wreck is no longer worth drawing.
pub(super) fn fall_of(s: &Session) -> Option<(u32, i32, i32, i32)> {
    let f = s.show.fx.iter().find(|f| f.what == What::WyrmDown)?;
    let age = f.age(s.elapsed);
    (age < WRECK_LIFE).then_some((age, f.s, f.x, f.z))
}

/// The Wyrm as this frame has it: its place and pose, the head's frame and
/// every curve its body is laid along. `None` when there is none to draw.
fn build(s: &Session) -> Option<(Scene, Fr, Rings)> {
    let anchor = units(s.anchor());
    let tick = s.elapsed;
    let fall = fall_of(s);
    let (head_s, x, z, pose, st, phase, atk, side, age) = match (&s.boss, fall) {
        (Some(b), fall) => {
            // The fall's own record while it lasts; on the way home and at the
            // tally the Wyrm has been dying as long as the flight has run.
            let dying = DYING_TICKS as u32;
            let age = if b.st != Wyrm::Dying {
                None
            } else {
                match s.mode {
                    Mode::Victory => Some(dying + s.mode_t),
                    Mode::Tally => Some(dying + VICTORY + s.mode_t),
                    _ => fall.map(|(a, ..)| a).or(Some(dying.saturating_sub(b.ticks))),
                }
            };
            // Dying, it holds its pace ahead of the wing; once done it stays
            // where it stood, and what is left of it falls.
            let settled = age.map_or(0.0, |a| a.saturating_sub(DYING_TICKS as u32) as f32 * pace());
            (anchor + units(boss::HEAD) - settled, units(b.x), units(b.z), pose_of(b, tick), b.st, b.phase, b.atk, f32::from(b.side), age)
        }
        (None, Some((age, fs, fx_, fz))) => {
            // The Wyrm is gone and its parts fall: held where the head stood
            // when it was done dying.
            let s_end = units(fs) + FRONT + pace() * DYING_TICKS;
            (s_end, units(fx_), units(fz) - CROWN, wreck_pose(), Wyrm::Dying, 2, 0, 1.0, Some(age))
        }
        (None, None) => return None,
    };
    // Nothing is left of it once its last part has fallen out of the picture.
    if age.is_some_and(|a| a >= LAST_START + GONE_AFTER) {
        return None;
    }
    let (f, r, _) = frame(head_s);
    let head = at(head_s, x, z);
    let fr = Fr::new(head, f, r, &pose);
    let out = lamps_out(s).min(3);
    let lamp_w = |k: usize| {
        let (lx, lz) = match k {
            0 => (-4.5, 1.0),
            1 => (4.5, 1.0),
            _ => (0.0, CROWN),
        };
        at(head_s - FRONT, x + lx, z + lz)
    };
    let sc = Scene {
        head,
        f,
        r,
        pose,
        age,
        out,
        horn_cut: [(out >= 1).then_some(0), (out >= 2).then_some(0)],
        tick,
        side,
        phase,
        atk,
        st,
        lamps: [lamp_w(0), lamp_w(1), lamp_w(2)],
        sweep: 0.0,
    };
    let rs = rings_for(&sc, &fr, head);
    Some((sc, fr, rs))
}

/// The Wyrm, or the parts of it that are left, into `m`.
pub(super) fn draw(m: &mut Mesh, s: &Session) {
    let Some((sc, fr, rs)) = build(s) else { return };
    let mut o = Out { m, xf: Xf::ID };
    skull(&mut o, &sc, &fr, &rs);
    body(&mut o, &sc, &fr, &rs);
    // A horn struck off by a lamp's going spins away from where it broke.
    for (k, n) in [(0usize, 1u32), (1, 2)] {
        if let Some(born) = lamp_born(s, n) {
            let age = sc.tick.saturating_sub(born);
            if age < 50 && sc.age.is_none() {
                severed(&mut o, &fr, &rs, k, age);
            }
        }
    }
    let _ = (sc.head, sc.phase, sc.atk, sc.st, sc.sweep);
}

/// Where the Wyrm's mouth is in the world (the throat of the roar's shaft and
/// where the spit starts).
pub(super) fn mouth(s: &Session) -> Option<V3> {
    let (_, fr, _) = build(s)?;
    Some(fr.p(MOUTH.0, MOUTH.1, MOUTH.2))
}

/// The mouth, in the head's own coordinates.
const MOUTH: (f32, f32, f32) = (-5.5, 0.0, -1.0);

/// A guide line: world points and the ink to draw them in.
pub(super) struct Guide {
    pub pts: Vec<V3>,
    pub ink: char,
}

/// Every path the Wyrm is drawn along or moves on, as guides for the debug
/// layer of the shots: the curves its neck, coil, horns, crest and hooks are
/// laid on, the arc each part takes when it falls, the spit's lines from the
/// mouth to its locked marks, the sweep's path in from the walls and the
/// roar's column.
pub(super) fn guides(s: &Session) -> Vec<Guide> {
    let Some((sc, fr, rs)) = build(s) else { return Vec::new() };
    let line = |v: &[Ring], ink: char| Guide { pts: v.iter().map(|r| r.p).collect(), ink };
    let mut out = vec![
        line(&rs.neck, ']'),
        line(&rs.loop_, ']'),
        line(&rs.horns[0], '%'),
        line(&rs.horns[1], '%'),
        line(&rs.crest, '%'),
        line(&rs.hooks[0], '%'),
        line(&rs.hooks[1], '%'),
    ];
    if let Some(age) = sc.age {
        for part in ALL_PARTS {
            let br = &BREAKS[part as usize];
            let from = sc.pivot_of(br, &fr);
            let pts: Vec<V3> = (0..=40)
                .step_by(4)
                .map(|t| sc.xf_at(part, &fr, br.start + t).apply(from))
                .collect();
            // Only the arcs still to come or in flight.
            if age < br.start + 40 {
                out.push(Guide { pts, ink: '9' });
            }
        }
    }
    for (k, n) in [(0usize, 1u32), (1, 2)] {
        if let Some(born) = lamp_born(s, n) {
            if sc.tick.saturating_sub(born) < 50 {
                let hr = &rs.horns[k];
                let pivot = hr[5.min(hr.len() - 1)].p;
                let pts = (0..=48)
                    .step_by(4)
                    .map(|t| severed_xf(&fr, &rs, k, t).apply(pivot))
                    .collect();
                out.push(Guide { pts, ink: '9' });
            }
        }
    }
    if let Some(b) = &s.boss {
        let anchor = units(s.anchor());
        let mouth = fr.p(MOUTH.0, MOUTH.1, MOUTH.2);
        let along = |a: V3, c: V3| (0..=8).map(|i| a.lerp(c, i as f32 / 8.0)).collect::<Vec<_>>();
        if matches!(b.st, Wyrm::Tell | Wyrm::Idle) && b.atk == 0 {
            let here = s.seats.values().next().map_or(anchor, |seat| units(s.craft_s(&seat.craft)) + 1.0);
            for &(mx, mz) in &b.marks {
                out.push(Guide { pts: along(mouth, at(here, units(mx), units(mz))), ink: 'C' });
            }
        }
        if matches!(b.st, Wyrm::Tell | Wyrm::Coil) && b.atk == 1 {
            let half = rail::rail().at(anchor).half * 0.9;
            let (gap, g) = (units(b.gap), units(boss::GAP_HALF));
            out.push(Guide { pts: along(at(anchor, -half, 0.0), at(anchor, gap - g, 0.0)), ink: 'C' });
            out.push(Guide { pts: along(at(anchor, half, 0.0), at(anchor, gap + g, 0.0)), ink: 'C' });
        }
        if matches!(b.st, Wyrm::Tell | Wyrm::Roar) && b.atk == 2 {
            out.push(Guide { pts: along(mouth, at(anchor, units(b.col), 0.0)), ink: 'C' });
        }
    }
    out
}

const ALL_PARTS: [Part; PARTS] = [
    Part::Core,
    Part::Jaw,
    Part::HornL,
    Part::HornR,
    Part::Crest,
    Part::HookL,
    Part::HookR,
    Part::FrillL,
    Part::FrillR,
    Part::Neck0,
    Part::Neck1,
    Part::Neck2,
    Part::Neck3,
    Part::Coil,
];

/// The transform of the tip of horn `k` `age` ticks after it was struck off:
/// a throw out and up, a turn end over end and the world's gravity.
fn severed_xf(fr: &Fr, rs: &Rings, k: usize, age: u32) -> Xf {
    let hr = &rs.horns[k];
    let side = if k == 0 { -1.0f32 } else { 1.0 };
    let t = age as f32;
    let axis = (fr.a * 1.0 + fr.c * 0.4).normalize();
    let (s, c) = (0.2 * t * side).dsin_cos();
    let pivot = hr[5.min(hr.len() - 1)].p;
    let disp = fr.b * (0.2 * t * side) + fr.c * (0.12 * t) - fr.a * (0.04 * t) + V3::UP * (-0.5 * G * t * t);
    Xf { on: true, pivot, axis, c, s, disp }
}

/// The tip of horn `k` after it was struck off `age` ticks ago, turning end
/// over end and falling.
fn severed(o: &mut Out, fr: &Fr, rs: &Rings, k: usize, age: u32) {
    let hr = &rs.horns[k];
    o.xf = severed_xf(fr, rs, k, age);
    tube(o, hr, 5, hr.len() - 1, 5, -fr.a, 1.0, &|_, _| HORN);
    let last = hr[hr.len() - 1];
    point(o, &last, last.p + (last.p - hr[hr.len() - 2].p).normalize() * 0.9, 5, HORN);
    o.xf = Xf::ID;
}

/// The coil's body sweeping in over the canyon's width: a banded slab of
/// serpent from the wall `x0` toward the gap's edge `x1`, `reach` (0 to 1) of
/// the way, flat along the rail and tall, its tip rounded; the face toward the
/// eye is the pale belly.
pub(super) fn coil_slab(m: &mut Mesh, s: f32, x0: f32, x1: f32, reach: f32, tick: u32) {
    let (f, _, _) = frame(s);
    let inward = if x1 > x0 { 1.0 } else { -1.0 };
    let len = (x1 - x0).abs() * reach.clamp(0.0, 1.0);
    if len < 0.8 {
        return;
    }
    const N: usize = 9;
    const SIDES: usize = 10;
    let mut o = Out { m, xf: Xf::ID };
    let dirs = circle(SIDES);
    // The section's half height and half depth along the rail.
    let section = |t: f32| {
        let close = ((1.0 - t) / 0.2).clamp(0.0, 1.0);
        let k = 0.1 + 0.9 * close * (2.0 - close);
        (9.4 * k, 2.4 * k)
    };
    let centre = |i: usize| {
        let t = i as f32 / (N - 1) as f32;
        let wob = (tick as f32 * 0.2 + t * 6.0).dsin() * 0.25 * t;
        at(s, x0 + inward * len * t, wob)
    };
    let mut v = 0.0;
    // A scale on the slab is half a unit across: it is near the eye.
    const K: f32 = 0.5;
    for i in 0..N - 1 {
        let (ta, tb) = (i as f32 / (N - 1) as f32, (i + 1) as f32 / (N - 1) as f32);
        let ((ha, da), (hb, db)) = (section(ta), section(tb));
        let (ca, cb) = (centre(i), centre(i + 1));
        let seg = (cb - ca).length();
        let pt = |c: V3, h: f32, d: f32, j: usize| {
            let (sn, cs) = dirs[j % SIDES];
            c + V3::UP * (cs * h) + f * (sn * d)
        };
        let mut u = 0.0;
        for j in 0..SIDES {
            let (s0, c0) = dirs[j];
            let (s1, c1) = dirs[(j + 1) % SIDES];
            let nrm = (V3::UP * ((c0 + c1) * 0.5 / 9.4) + f * ((s0 + s1) * 0.5 / 2.4)).normalize();
            let material = if nrm.dot(-f) > 0.75 && i % 2 == 0 { BELLY } else { SLAB };
            let edge = (pt(ca, ha, da, j + 1) - pt(ca, ha, da, j)).length();
            let (u0, u1) = (u * K, (u + edge) * K);
            u += edge;
            o.quad_uv(
                [pt(ca, ha, da, j), pt(ca, ha, da, j + 1), pt(cb, hb, db, j + 1), pt(cb, hb, db, j)],
                [[u0, v * K], [u1, v * K], [u1, (v + seg) * K], [u0, (v + seg) * K]],
                material,
            );
        }
        v += seg;
    }
}
