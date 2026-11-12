//! The Keep's site as a working yard, in 3D (the Works, `world::works`): the
//! treadwheel crane that lifts a stone from the yard to the course being
//! laid, the stone stacked at its foot, the carts that arrive from the gate
//! and unload there, mortar tubs, a ladder up the scaffold with hodmen going
//! up it and masons at the top. Everything is a pure function of the castle,
//! the stones laid, the yard's stock and the time, so the flat view's crew
//! (`world::works_life`) and this one work the same hour.
//!
//! The crane is the one `world::works_life::crane` moves (the flat view's
//! too): its mast stands where the baked falsework puts it and its jib turns,
//! hoists and lowers as that says; what is drawn here is the thing itself,
//! stood up tall and lit: a tapering mast of four posts and braces, a
//! treadwheel beside it turning, a jib with its counterweight and stays, a
//! pennon and a lantern at the head, a hook on a rope and slings round
//! the stone. (The first 3D crane was one thin mast laid flat on the ground,
//! which is why it was lost among the scaffolding.)
//!
//! A tone is a place on the monolith look's ramp, so the yard lies in a zone
//! of the timber ramp (`works_look`), set against the Keep's limestone, and
//! what is made of wood is warm and what is stone is pale: [`Yard::zone`].
//!
//! Performance: about two hundred nodes for a whole site, made each frame
//! from numbers (no meshes), clones of the kit's cached carts aside.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use angel_stage::scene::{Material, MeshRef, Node, Transform, quat_axis_angle};
use angel_stage::set::{Kind, SetLamp};

use super::hillside::vehicle_nodes;
use super::works_kit::{Dir, LineKind};
use crate::render::overworld::works_kit::{FRAMES, Load};
use crate::rules::{HZ, Run};
use crate::world::hillside::{self as hill, Delivery, KeepYard};
use crate::world::works::{Castle, PX_PER_M, Work};
use crate::world::works_life as wl;

/// Metres to kingdom pixels.
const K: f32 = PX_PER_M;

/// How fast an ox walks the yard, metres a second.
const OX_PACE: f32 = 1.5;

/// Seconds a cart stands unloading.
const UNLOAD: f32 = 5.0;

/// The most carts at the yard at once.
const MOST_CARTS: usize = 2;

/// What the site stands up as this frame.
#[derive(Debug, Default)]
pub struct Yard {
    pub nodes: Vec<Node>,
    pub lamps: Vec<SetLamp>,
    /// The yard's ground in the kingdom's pixels, `[x0, z0, x1, z1]`, where
    /// the look dyes the timber ramp, and the lane the carts come by from the gate.
    pub zone: Option<[f32; 4]>,
    pub lane: Option<[f32; 4]>,
}

// -------------------------------------------------------------- the pen ----

/// Nodes in castle metres (`x` east, `y` north, `z` up); a size is given in
/// that order too. The kingdom's own order (`x`, `y` up, `z` south) is
/// handled here and nowhere else.
struct Pen<'a> {
    castle: &'a Castle,
    out: Vec<Node>,
}

fn lit(t: f32) -> Material {
    Material::lit([t, t, t, 1.0], 0.0)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len3(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The turn that takes the x axis to the direction `d` (kingdom order).
fn x_to(d: [f32; 3]) -> [f32; 4] {
    let l = len3(d).max(1e-6);
    let v = [d[0] / l, d[1] / l, d[2] / l];
    // x cross v.
    let axis = [0.0, -v[2], v[1]];
    if len3(axis) < 1e-5 {
        return if v[0] >= 0.0 { [0.0, 0.0, 0.0, 1.0] } else { quat_axis_angle([0.0, 1.0, 0.0], PI) };
    }
    quat_axis_angle(axis, v[0].clamp(-1.0, 1.0).acos())
}

impl<'a> Pen<'a> {
    fn new(castle: &'a Castle) -> Self {
        Self { castle, out: Vec::with_capacity(256) }
    }

    fn px(&self, p: [f32; 3]) -> [f32; 3] {
        self.castle.world(p)
    }

    fn node(&mut self, mesh: MeshRef, at: [f32; 3], scale_k: [f32; 3], rotation: [f32; 4], material: Material) {
        let mut n = Node::new(0, mesh, material);
        n.transform = Transform { translation: at, rotation, scale: scale_k };
        self.out.push(n);
    }

    /// A box centred at `c`, its size `[east, north, up]` metres before a turn of `yaw` about the vertical.
    fn bx(&mut self, c: [f32; 3], s: [f32; 3], yaw: f32, t: f32) {
        let at = self.px(c);
        self.node(MeshRef::Cube, at, [s[0] * K, s[2] * K, s[1] * K], quat_axis_angle([0.0, 1.0, 0.0], yaw), lit(t));
    }

    /// A box between two points `a` and `b`, `thick` metres square.
    fn strut(&mut self, a: [f32; 3], b: [f32; 3], thick: f32, t: f32) {
        let (pa, pb) = (self.px(a), self.px(b));
        let d = sub(pb, pa);
        let l = len3(d);
        if l < 0.5 {
            return;
        }
        let mid = lerp3(pa, pb, 0.5);
        self.node(MeshRef::Cube, mid, [l, thick * K, thick * K], x_to(d), lit(t));
    }

    /// A column `r` across, `h` high, its foot at `c`.
    fn cyl(&mut self, c: [f32; 3], r: f32, h: f32, t: f32) {
        let at = self.px([c[0], c[1], c[2] + h / 2.0]);
        self.node(MeshRef::Cylinder { segments: 10 }, at, [2.0 * r * K, h * K, 2.0 * r * K], quat_axis_angle([0.0, 1.0, 0.0], 0.0), lit(t));
    }

    /// A cylinder narrowing upward (a tub), its foot at `c`.
    fn tub(&mut self, c: [f32; 3], r: f32, h: f32, t: f32) {
        let at = self.px([c[0], c[1], c[2] + h / 2.0]);
        self.node(MeshRef::Frustum { segments: 10 }, at, [2.0 * r * K, h * K, 2.0 * r * K], quat_axis_angle([0.0, 1.0, 0.0], 0.0), lit(t));
    }

    fn ball(&mut self, c: [f32; 3], r: [f32; 3], t: f32) {
        let at = self.px(c);
        self.node(MeshRef::Sphere { segments: 8 }, at, [2.0 * r[0] * K, 2.0 * r[2] * K, 2.0 * r[1] * K], quat_axis_angle([0.0, 1.0, 0.0], 0.0), lit(t));
    }

    /// A flat of colour that shines by itself (a pennon, a lantern's glass).
    fn glow(&mut self, c: [f32; 3], s: [f32; 3], yaw: f32, kind: Kind) {
        let [r, g, b] = kind.capture();
        let at = self.px(c);
        let m = Material { accent: true, ..Material::color([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]) };
        self.node(MeshRef::Cube, at, [s[0] * K, s[2] * K, s[1] * K], quat_axis_angle([0.0, 1.0, 0.0], yaw), m);
    }

    /// A person standing with feet at `at`, facing `yaw` (radians, east is 0, counter-clockwise),
    /// mid-stride at `walk` (any number: a stride a unit), bent over `bend` (0 to 1), a hod of
    /// `hod` on the shoulder (a stone block, or `None`).
    fn folk(&mut self, at: [f32; 3], yaw: f32, walk: f32, bend: f32, hod: Option<bool>, cloth: f32) {
        let (s, c) = yaw.sin_cos();
        let fwd = |d: f32, side: f32| [at[0] + c * d - s * side, at[1] + s * d + c * side];
        // Legs as one block that sways with the stride, a body, a head with its cap, the hod.
        let stride = (walk * TAU).sin() * 0.14;
        let l = fwd(stride, 0.0);
        self.bx([l[0], l[1], at[2] + 0.36], [0.2, 0.34, 0.72], yaw, 0.20);
        let lean = 0.22 * bend;
        let b = fwd(lean, 0.0);
        self.bx([b[0], b[1], at[2] + 1.0], [0.28 + 0.1 * bend, 0.44, 0.62], yaw, cloth);
        let h = fwd(lean * 2.4, 0.0);
        self.bx([h[0], h[1], at[2] + 1.48 - 0.16 * bend], [0.28, 0.28, 0.36], yaw, 0.58);
        if let Some(stone) = hod {
            let p = fwd(-0.2, 0.0);
            self.bx([p[0], p[1], at[2] + 1.38], [0.32, 0.44, 0.62], yaw, if stone { 0.72 } else { 0.40 });
        }
    }
}

// ------------------------------------------------------------ the crane ----

/// The treadwheel crane as `c` has it: mast, wheel, jib, rope and stone.
fn crane(p: &mut Pen, c: &wl::Crane, working: bool, lamps: &mut Vec<SetLamp>) {
    let [mx, my] = c.at;
    let h = c.top;
    // Which side of the mast the wheel and the yard are on: away from the keep.
    let out = if mx < 0.0 { -1.0 } else { 1.0 };
    // The footing.
    p.bx([mx, my, 0.18], [2.4, 2.4, 0.36], 0.0, 0.34);
    // The mast: four posts leaning in to the top, a ring of beams every few metres, an X brace on the
    // south face between rings.
    let rings = (h / 5.0).ceil().max(2.0) as usize;
    let half = |z: f32| 0.78 - 0.34 * (z / h).clamp(0.0, 1.0);
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        p.strut([mx + sx * half(0.0), my + sy * half(0.0), 0.0], [mx + sx * half(h), my + sy * half(h), h], 0.2, 0.52);
    }
    for i in 0..rings {
        let (z0, z1) = (i as f32 * h / rings as f32, (i as f32 + 1.0) * h / rings as f32);
        let (w0, w1) = (half(z0), half(z1));
        for (a, b) in [((-1.0f32, -1.0f32), (1.0f32, -1.0f32)), ((1.0, -1.0), (1.0, 1.0)), ((1.0, 1.0), (-1.0, 1.0)), ((-1.0, 1.0), (-1.0, -1.0))] {
            p.strut([mx + a.0 * w1, my + a.1 * w1, z1], [mx + b.0 * w1, my + b.1 * w1, z1], 0.14, 0.40);
        }
        let (l, r) = if i % 2 == 0 { (-1.0f32, 1.0f32) } else { (1.0, -1.0) };
        p.strut([mx + l * w0, my - w0, z0], [mx + r * w1, my - w1, z1], 0.1, 0.34);
    }
    // The head: a deck, the post the jib turns on, a pennon pole.
    let hd = h + 0.15;
    p.bx([mx, my, h + 0.08], [1.7, 1.7, 0.2], 0.0, 0.58);
    p.cyl([mx, my, h + 0.18], 0.3, 0.95, 0.30);
    let pole_top = hd + 4.0;
    p.strut([mx, my, hd + 0.3], [mx, my, pole_top], 0.12, 0.50);
    let wind = ((c.wheel * 0.05).sin() * 0.25) + 0.15;
    p.glow([mx + 0.62 * wind.cos() + 0.55, my + 0.62 * wind.sin(), pole_top - 0.4], [1.2, 0.04, 0.62], wind, Kind::Fire);
    // The lantern that says the crane works: lit while it does, dark when the yard has nothing for it.
    if working {
        p.glow([mx + 0.5, my - 0.6, hd + 0.8], [0.34, 0.34, 0.5], 0.0, Kind::Window);
        lamps.push(SetLamp { at: p.px([mx + 0.5, my - 0.6, hd + 0.9]), reach: 84.0, strength: 0.9 });
    } else {
        p.bx([mx + 0.5, my - 0.6, hd + 0.8], [0.34, 0.34, 0.5], 0.0, 0.14);
    }

    // The jib: a boom with a counterweight behind it, stayed to the pole.
    let (cy, sy) = (c.yaw.cos(), c.yaw.sin());
    let at = |d: f32, z: f32| [mx + cy * d, my + sy * d, z];
    let tip_d = c.reach + 0.6;
    p.strut(at(-2.4, hd + 0.55), at(tip_d, hd + 0.55), 0.3, 0.50);
    p.strut(at(-1.8, hd + 0.0), at(tip_d - 0.4, hd + 0.0), 0.16, 0.38);
    for d in [0.2f32, tip_d - 1.4] {
        p.strut(at(d, hd), at(d + 0.5, hd + 0.55), 0.1, 0.34);
    }
    p.bx([mx + cy * -2.2, my + sy * -2.2, hd + 0.2], [1.1, 1.1, 0.9], c.yaw, 0.20);
    p.strut([mx, my, pole_top], at(tip_d, hd + 0.55), 0.07, 0.30);
    p.strut([mx, my, pole_top], at(-2.4, hd + 0.6), 0.07, 0.30);
    // The block at the jib's tip and the rope down to the hook.
    let tip = at(c.reach, hd);
    p.bx([tip[0], tip[1], hd - 0.2], [0.36, 0.3, 0.4], c.yaw, 0.26);
    let carrying = c.stone[2] > 0.0 && (c.stone[0] - tip[0]).hypot(c.stone[1] - tip[1]) < 0.6;
    let hook_z = if carrying { c.stone[2] + 0.34 + 1.1 } else { hd - 1.6 };
    p.strut([tip[0], tip[1], hd - 0.3], [tip[0], tip[1], hook_z], 0.05, 0.22);
    p.bx([tip[0], tip[1], hook_z - 0.1], [0.2, 0.2, 0.22], 0.0, 0.26);
    if carrying {
        // The stone in four slings: a dressed block with its joints cut.
        let (sx, sy_, sz) = (c.stone[0], c.stone[1], c.stone[2]);
        p.bx([sx, sy_, sz], [1.3, 0.65, 0.62], c.yaw, 0.78);
        for (dx, dy) in [(-0.5f32, 0.0f32), (0.5, 0.0)] {
            let (px, py) = (sx + cy * dx - sy * dy, sy_ + sy * dx + cy * dy);
            p.strut([tip[0], tip[1], hook_z - 0.2], [px, py, sz + 0.32], 0.035, 0.22);
        }
    }

    // The treadwheel beside the mast, turning: two rims, treads between, spokes, a hub, its frame.
    // Its axle runs north and south on the west crane (the disc faces the camera); on the east one,
    // where the curtain is close, it stands south of the mast with its axle east and west.
    let (centre, u, v) = if mx < 0.0 { ([mx + out * 2.55, my], [1.0f32, 0.0f32], [0.0f32, 1.0f32]) } else { ([mx, my - 3.4], [0.0, 1.0], [1.0, 0.0]) };
    let (wz, r) = (1.85, 1.65);
    let n = 8;
    let a0 = c.wheel;
    let rim = |k: usize, side: f32, rr: f32| {
        let a = a0 + k as f32 / n as f32 * TAU;
        let (cu, cz) = (a.cos() * rr, a.sin() * rr);
        [centre[0] + u[0] * cu + v[0] * side, centre[1] + u[1] * cu + v[1] * side, wz + cz]
    };
    for k in 0..n {
        for side in [-0.6f32, 0.6] {
            p.strut(rim(k, side, r), rim(k + 1, side, r), 0.22, 0.46);
        }
        // A tread across between the rims, a little inside the rim, every other one.
        if k % 2 == 0 {
            p.strut(rim(k, -0.6, r - 0.12), rim(k, 0.6, r - 0.12), 0.34, 0.56);
        }
    }
    for k in 0..2 {
        for side in [-0.6f32, 0.6] {
            p.strut(rim(k, side, r), rim(k + n / 2, side, r), 0.14, 0.40);
        }
    }
    p.strut(rim(0, -0.8, 0.0), rim(0, 0.8, 0.0), 0.34, 0.26);
    // The frame it stands in: two A-frames over the axle and a sill under it.
    let at_c = |du: f32, dv: f32, z: f32| [centre[0] + u[0] * du + v[0] * dv, centre[1] + u[1] * du + v[1] * dv, z];
    for dv in [-1.0f32, 1.0] {
        p.strut(at_c(-1.5, dv, 0.0), at_c(0.0, dv, wz + 0.2), 0.18, 0.48);
        p.strut(at_c(1.5, dv, 0.0), at_c(0.0, dv, wz + 0.2), 0.18, 0.48);
    }
    p.strut(at_c(0.0, 0.0, wz), [mx, my, 2.6], 0.1, 0.30);
    let sill_yaw = if mx < 0.0 { 0.0 } else { FRAC_PI_2 };
    p.bx(at_c(0.0, 0.0, 0.12), [3.4, 2.2, 0.24], sill_yaw, 0.38);
}

// -------------------------------------------------------------- the yard ----

/// The crane's jib, metres (`works_life`'s, which the flat view moves by).
const JIB: f32 = 4.4;

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The crane of the lift being laid with its stone's cycle on `pick`, the
/// place its yard is: `works_life::crane` with the yard's place its own,
/// the same cycle (9 s: made fast, hoisted, turned, lowered, back empty).
/// For the first crane the two are the same crane.
fn pose(castle: &Castle, laid: u32, seconds: f32, pick: [f32; 2]) -> Option<wl::Crane> {
    let base = wl::crane(castle, laid, seconds)?;
    let at = base.at;
    let top = base.top;
    let aim = if laid > 0 {
        let c = castle.stones[(laid - 1) as usize].corners();
        [0, 1, 2].map(|k| c.iter().map(|p| p[k]).sum::<f32>() / 8.0)
    } else {
        [at[0], at[1] + 3.0, 0.3]
    };
    let to_yard = [pick[0] - at[0], pick[1] - at[1]];
    let to_aim = [aim[0] - at[0], aim[1] - at[1]];
    let yaw_yard = to_yard[1].atan2(to_yard[0]);
    let mut yaw_aim = to_aim[1].atan2(to_aim[0]);
    while yaw_aim - yaw_yard > PI {
        yaw_aim -= TAU;
    }
    while yaw_aim - yaw_yard < -PI {
        yaw_aim += TAU;
    }
    let p = (seconds / 9.0).fract();
    let (yaw, rise, lower) = if p < 0.18 {
        (yaw_yard, 0.0, 0.0)
    } else if p < 0.42 {
        (yaw_yard, smooth((p - 0.18) / 0.24), 0.0)
    } else if p < 0.62 {
        (yaw_yard + (yaw_aim - yaw_yard) * smooth((p - 0.42) / 0.20), 1.0, 0.0)
    } else if p < 0.78 {
        (yaw_aim, 1.0, smooth((p - 0.62) / 0.16))
    } else {
        (yaw_aim + (yaw_yard - yaw_aim) * smooth((p - 0.78) / 0.22), 1.0, 1.0)
    };
    let reach_yard = JIB.min(to_yard[0].hypot(to_yard[1])).max(1.6);
    let reach_aim = to_aim[0].hypot(to_aim[1]).min(JIB).max(2.0);
    // The jib reaches in or out as it turns, between the yard's reach and the wall's.
    let reach = if p < 0.42 {
        reach_yard
    } else if p < 0.62 {
        reach_yard + (reach_aim - reach_yard) * smooth((p - 0.42) / 0.20)
    } else if p < 0.78 {
        reach_aim
    } else {
        reach_aim + (reach_yard - reach_aim) * smooth((p - 0.78) / 0.22)
    };
    let hang = top - 0.7;
    let stone_z = if lower > 0.0 && p < 0.78 { hang + (aim[2] + 0.3 - hang) * lower } else if p >= 0.78 { hang } else { 0.3 + (hang - 0.3) * rise };
    let tip = [at[0] + reach * yaw.cos(), at[1] + reach * yaw.sin()];
    // On the empty return no stone hangs on the hook: the next waits in the cradle.
    let stone = if p >= 0.78 { [pick[0], pick[1], 0.3] } else { [tip[0], tip[1], stone_z] };
    Some(wl::Crane { at, top, yaw, reach, rope: top - stone_z, stone, wheel: base.wheel })
}

/// Where the yard is, in castle metres. The bailey west of the keep is
/// where the game's own stations stand (the yard of cut stone, the quarry's
/// heap, the tally board, the mortar tub: `rules::hands`), so the first
/// crane's yard is laid round them; the second crane, on the east flank, has
/// only the strip south of its mast between the keep and the curtain.
struct Plan {
    /// Where the jib lifts from.
    pick: [f32; 2],
    /// The places of the stack's blocks, a course of them, then the next.
    slots: Vec<[f32; 2]>,
    heap: [f32; 2],
    /// A pile of poles: its middle and whether it runs north and south.
    timber: ([f32; 2], bool),
    lantern: [f32; 2],
    /// Where a cart stands unloading, its way in and its way out.
    stop: [f32; 2],
    way_in: Vec<[f32; 2]>,
    way_out: Vec<[f32; 2]>,
    /// The ground the timber ramp dyes: `[x0, y0, x1, y1]`, and the strip of lane between it and the gate.
    ground: [f32; 4],
    lane: [f32; 4],
}

impl Plan {
    fn of(mast: [f32; 2]) -> Self {
        let grid = |xs: [f32; 3], ys: [f32; 3]| -> Vec<[f32; 2]> { ys.iter().flat_map(|&y| xs.iter().map(move |&x| [x, y])).collect() };
        if mast[0] < 0.0 {
            Self {
                pick: [mast[0] - 1.6, mast[1] - 4.1],
                slots: grid([-13.5, -12.4, -11.3], [-2.0, -2.7, -3.4]),
                heap: [-14.7, -5.4],
                timber: ([-14.9, -1.2], true),
                lantern: [-14.9, -3.4],
                stop: [-12.8, -9.0],
                way_in: vec![[-0.8, -19.0], [-0.8, -9.0], [-12.8, -9.0]],
                way_out: vec![[-12.8, -10.2], [0.8, -10.2], [0.8, -19.0]],
                ground: [-15.6, -11.0, -9.7, 5.4],
                lane: [-9.7, -11.0, 1.8, -8.2],
            }
        } else {
            Self {
                pick: [mast[0], mast[1] - 4.6],
                slots: grid([12.0, 13.1, 14.2], [-6.4, -7.1, -7.8]),
                heap: [15.0, -3.6],
                timber: ([11.7, -3.4], true),
                lantern: [11.8, -5.2],
                stop: [12.5, -9.0],
                way_in: vec![[0.8, -19.0], [0.8, -9.0], [12.5, -9.0]],
                way_out: vec![[12.5, -10.2], [-0.8, -10.2], [-0.8, -19.0]],
                ground: [11.0, -11.0, 15.6, 1.8],
                lane: [-1.8, -11.0, 11.0, -8.2],
            }
        }
    }
}

fn path_len(path: &[[f32; 2]]) -> f32 {
    path.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum()
}

fn dir_of(dx: f32, dy: f32) -> Dir {
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { Dir::East } else { Dir::West }
    } else if dy >= 0.0 {
        Dir::North
    } else {
        Dir::South
    }
}

/// The point `d` metres along a path, and the way it is heading there.
fn along(path: &[[f32; 2]], d: f32) -> ([f32; 2], Dir) {
    let mut left = d.max(0.0);
    for w in path.windows(2) {
        let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let l = dx.hypot(dy);
        if left <= l || l < 1e-4 {
            let t = if l < 1e-4 { 0.0 } else { (left / l).min(1.0) };
            return ([w[0][0] + dx * t, w[0][1] + dy * t], dir_of(dx, dy));
        }
        left -= l;
    }
    let (last, prev) = (path[path.len() - 1], path[path.len() - 2]);
    (last, dir_of(last[0] - prev[0], last[1] - prev[1]))
}

/// The yard's ground, the track and the stack, the heaps and the lantern.
/// What the yard holds to be seen: the sim's stock in the Keep's yard, when the sim keeps it, else the
/// Works' own (`Run.hands.yard`). Counts of what is drawn: blocks in the stack, lumps in the heap, logs in the pile.
struct Held {
    blocks: u32,
    lumps: u32,
    logs: u32,
}

/// Units of dressed stone a block of the stack stands for, of rubble a lump of the heap, of poles and joists a
/// log of the pile: fixed numbers, so a yard of nine hundred stones is a full stack and one of ten is a block.
const PER_BLOCK: u32 = 20;
const PER_LUMP: u32 = 25;
const PER_LOG: u32 = 12;

fn held(run: &Run, real: Option<&KeepYard>, slots: usize) -> Held {
    let most = slots as u32 * 2;
    match real {
        Some(y) => Held {
            blocks: if y.cut == 0 { 0 } else { (1 + (y.cut - 1) / PER_BLOCK).min(most) },
            lumps: if y.rough == 0 { 0 } else { (1 + (y.rough - 1) / PER_LUMP).min(3) },
            logs: if y.timber == 0 { 0 } else { (1 + (y.timber - 1) / PER_LOG).min(3) },
        },
        None => Held { blocks: run.hands.yard.cut.min(most), lumps: 3, logs: 3 },
    }
}

fn yard(p: &mut Pen, plan: &Plan, run: &Run, real: Option<&KeepYard>, lamps: &mut Vec<SetLamp>, working: bool) {
    let have = held(run, real, plan.slots.len());
    // The worn ground of the yard: a pad of trodden earth.
    let [gx0, gy0, gx1, gy1] = plan.ground;
    p.bx([(gx0 + gx1) / 2.0, (gy0 + gy1) / 2.0, 0.03], [gx1 - gx0 - 0.4, gy1 - gy0 - 0.4, 0.06], 0.0, 0.50);
    // The track from the gate: a lane of trodden earth, a rut each side.
    for path in [&plan.way_in, &plan.way_out] {
        for w in path.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy);
            if l < 0.3 {
                continue;
            }
            let yw = dy.atan2(dx);
            let (cx, cy) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
            p.bx([cx, cy, 0.05], [l + 1.0, 1.6, 0.09], yw, 0.52);
            p.bx([cx, cy, 0.1], [l + 0.8, 0.3, 0.05], yw, 0.32);
        }
    }
    // The cradle the crane lifts from: two bearers, so the slings come free.
    for off in [-0.42f32, 0.42] {
        p.bx([plan.pick[0], plan.pick[1] + off, 0.12], [1.4, 0.3, 0.24], 0.0, 0.34);
    }
    // The stone ready for the hook: the block the jib comes down on, while the yard has one for it.
    if working {
        p.bx([plan.pick[0], plan.pick[1], 0.55], [1.3, 0.65, 0.62], 0.0, 0.78);
    }
    // The stack of dressed blocks: courses as the yard's cut stone fills them.
    let blocks = if real.is_some() { have.blocks } else if working { have.blocks.max(9) } else { have.blocks };
    for i in 0..blocks {
        let (layer, k) = (i as usize / plan.slots.len(), i as usize % plan.slots.len());
        let q = plan.slots[k];
        // A little askew, as a stack is, by the block's number; every other course turned.
        let jit = ((i * 37 % 7) as f32 - 3.0) * 0.015;
        let t = 0.70 + ((i * 13) % 5) as f32 * 0.035;
        let (sx, sy, yaw) = if layer % 2 == 0 { (1.05, 0.64, jit) } else { (0.64, 1.05, jit) };
        p.bx([q[0], q[1], 0.25 + layer as f32 * 0.52], [sx, sy, 0.5], yaw, t);
    }
    // A heap of rough stone, from the stockyard.
    for (k, (dx, dy, s)) in [(0.0f32, 0.0f32, 0.9f32), (0.7, 0.3, 0.7), (-0.4, 0.6, 0.6)].into_iter().enumerate().take(have.lumps as usize) {
        let hh = 0.2 + s * 0.5;
        p.ball([plan.heap[0] + dx, plan.heap[1] + dy, hh * 0.9], [s * 0.6, s * 0.55, hh * 0.9], 0.44 + (k % 3) as f32 * 0.06);
    }
    // Poles and joists, a pile of timber on skids.
    let (t0, ns) = plan.timber;
    let mut laid_logs = 0;
    for i in 0..2 {
        for j in 0..(2 - i) {
            if laid_logs >= have.logs {
                continue;
            }
            laid_logs += 1;
            let off = (j as f32 - (1 - i) as f32 / 2.0) * 0.5;
            let (c, size) = if ns { ([t0[0] + off, t0[1], 0.28 + i as f32 * 0.3], [0.26, 3.0, 0.26]) } else { ([t0[0], t0[1] + off, 0.28 + i as f32 * 0.3], [3.0, 0.26, 0.26]) };
            p.bx(c, size, 0.0, 0.30 + i as f32 * 0.04);
        }
    }
    // A post with a lantern over the stack, so the yard is lit where the work is.
    let post = plan.lantern;
    p.bx([post[0], post[1], 1.2], [0.12, 0.12, 2.4], 0.0, 0.34);
    p.glow([post[0], post[1], 2.55], [0.28, 0.28, 0.34], 0.0, Kind::Window);
    lamps.push(SetLamp { at: p.px([post[0], post[1], 2.7]), reach: 60.0, strength: 0.7 });
}

/// The carts that come to the yard: in from the gate, standing at the bay
/// while two hodmen carry the load to the stack, out empty. Each its own
/// turn round the clock, staggered so the bay is never bare and never crowded.
fn carts(p: &mut Pen, plan: &Plan, seconds: f32, count: usize, step: u32) {
    let (lin, lout) = (path_len(&plan.way_in), path_len(&plan.way_out));
    let t_in = lin / OX_PACE;
    let cycle = t_in + UNLOAD + lout / OX_PACE;
    let dump = plan.slots[plan.slots.len() / 2];
    for k in 0..count {
        let t = (seconds + k as f32 * cycle / count as f32).rem_euclid(cycle);
        let (pos, dir, load, moving) = if t < t_in {
            let (q, d) = along(&plan.way_in, t * OX_PACE);
            (q, d, Some(Load::Ashlar), true)
        } else if t < t_in + UNLOAD {
            let (_, d) = along(&plan.way_in, lin);
            // Unloaded half way through the stop.
            let load = if t - t_in < UNLOAD * 0.5 { Some(Load::Ashlar) } else { None };
            (plan.stop, d, load, false)
        } else {
            let (q, d) = along(&plan.way_out, (t - t_in - UNLOAD) * OX_PACE);
            (q, d, None, true)
        };
        let nodes = vehicle_nodes(LineKind::Cart, dir, load, if moving { step } else { 0 }, true);
        let at = p.px([pos[0], pos[1], 0.0]);
        for n in nodes.iter() {
            let mut n = n.clone();
            n.transform.translation = [n.transform.translation[0] + at[0], n.transform.translation[1], n.transform.translation[2] + at[2]];
            p.out.push(n);
        }
        // Two hodmen carry the load from the cart to the stack while it stands.
        if t >= t_in && t < t_in + UNLOAD {
            let u = (t - t_in) / UNLOAD;
            for h in 0..2 {
                let phase = (u * 1.6 + h as f32 * 0.5).fract();
                let (to_stack, f) = if phase < 0.5 { (true, phase * 2.0) } else { (false, (phase - 0.5) * 2.0) };
                let s = if to_stack { f } else { 1.0 - f };
                let from = [plan.stop[0] + 0.6 * (h as f32 - 0.5), plan.stop[1] + 1.2];
                let to = [dump[0] + 0.9 * (h as f32 - 0.5), dump[1] - 1.0];
                let q = [from[0] + (to[0] - from[0]) * s, from[1] + (to[1] - from[1]) * s];
                let yaw = if to_stack { (to[1] - from[1]).atan2(to[0] - from[0]) } else { (from[1] - to[1]).atan2(from[0] - to[0]) };
                p.folk([q[0], q[1], 0.0], yaw, seconds * 1.5 + h as f32, 0.0, Some(to_stack), 0.50);
            }
        }
    }
}

/// How far apart carts stand in a queue on the lane, metres.
const BERTH: f32 = 3.6;

/// The carts of the sim bound for the Keep's yard, on the bailey's lane, where the real ones are: each
/// at the place its distance from the yard puts it, held in the queue behind the one at the berth, standing
/// while the sim has it unloading, and two hodmen carrying its load to the stack for as long as the sim's
/// unloading takes. A cart that has not come within the lane's reach is on the road (`world3d::hillside`).
fn real_carts(p: &mut Pen, plan: &Plan, y: &KeepYard, seconds: f32, step: u32) {
    let lin = path_len(&plan.way_in);
    let dump = plan.slots[plan.slots.len() / 2];
    let rank = |c: &hill::YardCart| match c.phase {
        Delivery::Unloading => 0,
        Delivery::Waiting => 1,
        Delivery::Hauling => 2,
    };
    let mut near: Vec<&hill::YardCart> = y.carts.iter().filter(|c| c.dist <= hill::RELAY).collect();
    near.sort_by(|a, b| (rank(a), a.dist).partial_cmp(&(rank(b), b.dist)).unwrap_or(std::cmp::Ordering::Equal));
    for (ahead, c) in near.into_iter().enumerate() {
        let queue = lin - BERTH * ahead as f32;
        let (s, moving) = match c.phase {
            Delivery::Hauling => {
                // A tile is one and a half metres; the cart that has come as far as its place stands in the queue.
                let by_road = lin - c.dist * 1.5;
                (by_road.min(queue), by_road < queue - 0.05)
            }
            _ => (queue, false),
        };
        let (pos, dir) = along(&plan.way_in, s.max(0.0));
        // The load is aboard until half way through the unloading, then it is on the hodmen's shoulders.
        let u = if c.phase == Delivery::Unloading { 1.0 - c.left as f32 / c.dwell.max(1) as f32 } else { 0.0 };
        let load = if c.phase == Delivery::Unloading && u >= 0.5 { None } else { c.load.or(Some(Load::Ashlar)) };
        let nodes = vehicle_nodes(LineKind::Cart, dir, load, if moving { step } else { 0 }, true);
        let at = p.px([pos[0], pos[1], 0.0]);
        for n in nodes.iter() {
            let mut n = n.clone();
            n.transform.translation = [n.transform.translation[0] + at[0], n.transform.translation[1], n.transform.translation[2] + at[2]];
            p.out.push(n);
        }
        if c.phase == Delivery::Unloading {
            for h in 0..2 {
                let phase = (u * 1.6 + h as f32 * 0.5).fract();
                let (to_stack, f) = if phase < 0.5 { (true, phase * 2.0) } else { (false, (phase - 0.5) * 2.0) };
                let t = if to_stack { f } else { 1.0 - f };
                let from = [pos[0] + 0.6 * (h as f32 - 0.5), pos[1] + 1.2];
                let to = [dump[0] + 0.9 * (h as f32 - 0.5), dump[1] - 1.0];
                let q = [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t];
                let yaw = if to_stack { (to[1] - from[1]).atan2(to[0] - from[0]) } else { (from[1] - to[1]).atan2(from[0] - to[0]) };
                p.folk([q[0], q[1], 0.0], yaw, seconds * 1.5 + h as f32, 0.0, Some(to_stack), 0.50);
            }
        }
    }
}

// ----------------------------------------------------------- the ladder ----

/// The scaffold bay nearest the crane at the lift being laid and a ladder up
/// its outer side: hodmen climbing with mortar and stone, masons on the deck.
fn ladder(p: &mut Pen, castle: &Castle, c: &wl::Crane, laid: u32, built: u32, seconds: f32, working: bool, lamps: &mut Vec<SetLamp>) {
    if laid == 0 || laid >= castle.len() {
        return;
    }
    // The standing bay nearest the crane whose deck is at the lift being laid: the hodmen go up beside
    // the crane, where the stone comes up too.
    let lift = c.top - 0.9;
    let mut best: Option<(f32, [f32; 6])> = None;
    for f in castle.falsework.iter().filter(|f| f.kind == Work::Scaffold && f.appears <= built.max(laid) && laid < f.gone) {
        let b = f.bx.map(|v| f32::from(v) / 100.0);
        let (cx, cy) = ((b[0] + b[3]) / 2.0, (b[1] + b[4]) / 2.0);
        let score = (cx - c.at[0]).hypot(cy - c.at[1]) + 0.8 * (b[5] - (lift + 0.2)).abs();
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, b));
        }
    }
    let Some((_, bay)) = best else { return };
    let (cx, cy) = ((bay[0] + bay[3]) / 2.0, (bay[1] + bay[4]) / 2.0);
    // The ladder stands on the bay's outer face: away from the castle's middle along the longer way.
    let (nx, ny) = if cx.abs() >= cy.abs() { (cx.signum(), 0.0) } else { (0.0, cy.signum()) };
    let (half_x, half_y) = ((bay[3] - bay[0]) / 2.0, (bay[4] - bay[1]) / 2.0);
    let (tx, ty) = (-ny, nx);
    let height = bay[5];
    // A ladder leaning on the bay: its foot out a metre more than its head.
    let head = [cx + nx * (half_x + 0.3), cy + ny * (half_y + 0.3), height + 0.3];
    let base = [cx + nx * (half_x + 1.3), cy + ny * (half_y + 1.3), 0.0];
    for off in [-0.3f32, 0.3] {
        p.strut([base[0] + tx * off, base[1] + ty * off, 0.0], [head[0] + tx * off, head[1] + ty * off, head[2]], 0.09, 0.40);
    }
    let rungs = ((height / 1.6).floor() as usize).clamp(2, 16);
    for r in 1..=rungs {
        let t = r as f32 / (rungs as f32 + 0.5);
        let q = lerp3(base, head, t);
        p.strut([q[0] - tx * 0.3, q[1] - ty * 0.3, q[2]], [q[0] + tx * 0.3, q[1] + ty * 0.3, q[2]], 0.06, 0.34);
    }
    // Hodmen up and down it, each in their own time: loaded going up (mortar or stone), empty coming down.
    // With nothing for the crane to lift the course is not worked: the ladder stands bare.
    let yaw_up = (-ny).atan2(-nx);
    let climb = 3.0 + height / 0.7;
    let cycle = climb * 2.0 + 4.0;
    for k in 0..if working { 3 } else { 0 } {
        let t = (seconds + k as f32 * cycle / 3.0).rem_euclid(cycle);
        let (s, up) = if t < climb { (t / climb, true) } else if t < climb + 2.0 { (1.0, true) } else if t < 2.0 * climb + 2.0 { (1.0 - (t - climb - 2.0) / climb, false) } else { (0.0, false) };
        let q = lerp3(base, head, s);
        // On the rungs: a little out from the ladder.
        p.folk([q[0] + nx * 0.34, q[1] + ny * 0.34, q[2] - 0.3], yaw_up, seconds * 1.2 + k as f32, 0.1, up.then_some(k % 2 == 0), 0.50);
    }
    // At the top: a mason setting the course, a hodman tipping mortar, the tub on the deck.
    let deck = [cx, cy, height + 0.1];
    let beat = (seconds * 3.0).fract();
    let swing = if working { 0.5 + 0.4 * (beat * TAU).sin().abs() } else { 0.1 };
    p.folk([deck[0] - nx * 0.2, deck[1] - ny * 0.2, deck[2]], yaw_up + PI, 0.0, swing, None, 0.60);
    p.folk([deck[0] + tx * 1.1, deck[1] + ty * 1.1, deck[2]], yaw_up + PI, 0.0, 0.2, None, 0.46);
    p.tub([deck[0] - tx * 0.9, deck[1] - ty * 0.9, deck[2]], 0.32, 0.4, 0.24);
    // The foot of the ladder: a tub of mortar.
    let q = [base[0] + tx * 1.4 + nx * 0.4, base[1] + ty * 1.4 + ny * 0.4];
    p.tub([q[0], q[1], 0.0], 0.4, 0.5, 0.22);
    p.cyl([q[0], q[1], 0.42], 0.34, 0.06, 0.74);
    lamps.push(SetLamp { at: p.px([deck[0], deck[1], deck[2] + 1.2]), reach: 52.0, strength: 0.55 });
}

// ------------------------------------------------------------- the site ----

/// The Keep's site as a working yard for `laid` stones standing and
/// the carpenters' bays raised as far as stone `built`, at `seconds` of the crane's
/// own clock; empty once the castle stands.
pub fn of(castle: &'static Castle, run: &Run, laid: u32, built: u32, seconds: f32) -> Yard {
    let mut pen = Pen::new(castle);
    let mut lamps = Vec::new();
    let mut zone = None;
    let mut lane = None;
    let live = laid < castle.len();
    if let Some(base) = wl::crane(castle, laid, seconds) {
        let plan = Plan::of(base.at);
        // With the production game running, what the yard holds and the carts that bring it are the sim's, and
        // the crane works while the yard can pay for a stone: with nothing to lift it stands (the jib parked over
        // the cradle, the wheel still, the hook up and the lantern out). Without the sim it is the Works' own
        // stock and a crane that turns on its clock.
        let hv = crate::world::hillside::of(run);
        let real = hv.as_deref().and_then(|v| v.keep_yard.as_ref());
        let working = live && real.is_none_or(|y| y.can_pay);
        if let Some(mut c) = pose(castle, laid, if working { seconds } else { 0.0 }, plan.pick) {
            if !working {
                // A hook with no stone on it: the rope up, the block at the jib's tip.
                c.stone[2] = -1.0;
            }
            crane(&mut pen, &c, working, &mut lamps);
            ladder(&mut pen, castle, &c, laid, built, seconds, working, &mut lamps);
        }
        yard(&mut pen, &plan, run, real, &mut lamps, working);
        let sim = run.tick as f32 / HZ as f32;
        let step = (run.tick / u64::from((HZ / 8).max(1))) as u32 % FRAMES;
        match real {
            Some(y) => {
                if live {
                    real_carts(&mut pen, &plan, y, sim, step);
                }
            }
            None => {
                // The old crane's carts: as many as the road brings, one at least while stone is wanted.
                let loaded = hv.as_deref().map_or(0, |v| v.carts.iter().filter(|c| c.load.is_some()).count());
                let count = if live { (1 + loaded.min(8) / 4).min(MOST_CARTS) } else { 0 };
                if count > 0 {
                    carts(&mut pen, &plan, sim, count, step);
                }
            }
        }
        // The ground the timber ramp dyes: the yard, the crane's foot and the cart bay.
        let [x0, y0, x1, y1] = plan.ground;
        let (a, b) = (pen.px([x0, y1, 0.0]), pen.px([x1, y0, 0.0]));
        zone = Some([a[0].min(b[0]), a[2].min(b[2]), a[0].max(b[0]), a[2].max(b[2])]);
        let [x0, y0, x1, y1] = plan.lane;
        let (a, b) = (pen.px([x0, y1, 0.0]), pen.px([x1, y0, 0.0]));
        lane = Some([a[0].min(b[0]), a[2].min(b[2]), a[0].max(b[0]), a[2].max(b[2])]);
    }
    Yard { nodes: pen.out, lamps, zone, lane }
}

/// Whether the bailey draws the sim's carts for the Keep's yard (so that the road does not draw them too): the
/// Keep is raising, its crane stands, and the sim has staked it.
pub fn shows_real_carts(run: &Run) -> bool {
    if !crate::world::works::on_map() || run.keep_live().is_none() {
        return false;
    }
    let Some(site) = run.sites.keep() else { return false };
    let Some(castle) = site.castle() else { return false };
    let laid = run.site_shown(site);
    laid < castle.len() && wl::crane(castle, laid, 0.0).is_some()
}

#[cfg(test)]
#[path = "../../tests/world3d/works_yard__tests.rs"]
mod tests;
