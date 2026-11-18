//! A shot in flight as a few solid shapes (docs/GLASS_CORE.md, "3D shots"),
//! for a window that draws it itself and a browser that does: the same nodes
//! in both, in the game's colours, from the built-in meshes.
//!
//! The world is the stage's (`angel_stage::Tilt::camera`): `x` across the
//! room, `z` down it (the room's `y`), `y` the height off the floor, all in
//! the room's pixels. A shot is built in its own frame, `l` along its flight,
//! `u` up and `s` across, and turned into the world by the way it flies.
//!
//! Every colour is an ink of the game's palette (`ink`), the one the shot's
//! sprite is painted in, so a shape is the sprite's colours, solid. Fire, magic
//! and light shine by themselves (`Paint::glow`); arrows, blades, bones, ore
//! and pages are lit by the scene's one light and nothing more. The halo, the
//! comet's tail and the shadow on the floor are not here: they stay layers of
//! light (`stage_frame::shot_layers`), under and around the shape.
//!
//! The stage's projection is oblique (it has to be: what stands up in it is
//! placed so, docs/GLASS_CORE.md), so a ball would show taller than it is
//! wide. Round things are therefore flattened in depth by [`DEPTH`], which
//! makes them round again on the screen; things that go along the flight keep
//! their length whichever way it points.

use angel_stage::scene::{MeshRef, Material, Node, QUAT_IDENTITY, Transform, quat_axis_angle, quat_mul, quat_rotate};

use super::room_scene::{ClassShot, ForgedColour, ForgedShape, SceneShot, ShotForm};
use crate::render::overworld::ink::ink;
use crate::rules::Shot;

pub const DEPTH: f32 = 0.45;

pub const LIFT: f32 = 4.0;

const FIRST_NODE: u64 = 0x5_0000_0000;
const NODES_PER_SHOT: u64 = 32;

const TURN_PX: f32 = 22.0;

#[derive(Clone, Copy)]
struct Paint {
    ink: char,
    emissive: f32,
    alpha: f32,
}

impl Paint {
    const fn lit(ink: char) -> Self {
        Self { ink, emissive: 0.0, alpha: 1.0 }
    }

    const fn glow(ink: char, emissive: f32) -> Self {
        Self { ink, emissive, alpha: 1.0 }
    }

    const fn veil(ink: char, emissive: f32, alpha: f32) -> Self {
        Self { ink, emissive, alpha }
    }

    fn material(self) -> Material {
        let [r, g, b] = ink(self.ink).unwrap_or([255, 0, 255]);
        Material::lit([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, self.alpha], self.emissive)
    }
}

struct Maker<'a> {
    out: &'a mut Vec<Node>,
    next: u64,
    at: [f32; 3],
    yaw: [f32; 4],
    body: [f32; 4],
    travel: f32,
}

fn axis(l: f32, u: f32, s: f32) -> [f32; 3] {
    [l, u, s]
}

impl Maker<'_> {
    fn world(&self, offset: [f32; 3]) -> [f32; 3] {
        let p = quat_rotate(self.yaw, quat_rotate(self.body, offset));
        [self.at[0] + p[0], self.at[1] + p[1], self.at[2] + p[2]]
    }

    fn push(&mut self, mesh: MeshRef, transform: Transform, paint: Paint) {
        let mut node = Node::new(self.next, mesh, paint.material());
        node.transform = transform;
        self.next += 1;
        self.out.push(node);
    }

    fn solid(&mut self, mesh: MeshRef, offset: [f32; 3], turn: [f32; 4], size: [f32; 3], paint: Paint) {
        let rotation = quat_mul(self.yaw, quat_mul(self.body, turn));
        self.push(mesh, Transform { translation: self.world(offset), rotation, scale: size }, paint);
    }

    fn ball(&mut self, offset: [f32; 3], d: f32, paint: Paint) {
        let transform = Transform {
            translation: self.world(offset),
            rotation: QUAT_IDENTITY,
            scale: [d, d, d * DEPTH],
        };
        self.push(MeshRef::Sphere { segments: 12 }, transform, paint);
    }

    fn ball_in_world(&mut self, offset: [f32; 3], d: f32, paint: Paint) {
        let at = [self.at[0] + offset[0], self.at[1] + offset[1], self.at[2] + offset[2]];
        let transform = Transform { translation: at, rotation: QUAT_IDENTITY, scale: [d, d, d * DEPTH] };
        self.push(MeshRef::Sphere { segments: 12 }, transform, paint);
    }

    fn bar(&mut self, offset: [f32; 3], size: [f32; 3], paint: Paint) {
        self.solid(MeshRef::Cube, offset, QUAT_IDENTITY, size, paint);
    }

    fn spike(&mut self, offset: [f32; 3], width: f32, len: f32, forward: bool, paint: Paint) {
        let quarter = std::f32::consts::FRAC_PI_2;
        let turn = quat_axis_angle([0.0, 0.0, 1.0], if forward { -quarter } else { quarter });
        self.solid(MeshRef::Cone { segments: 6 }, offset, turn, [width, len, width], paint);
    }

    fn gem(&mut self, offset: [f32; 3], size: [f32; 3], paint: Paint) {
        self.solid(MeshRef::Gem, offset, QUAT_IDENTITY, size, paint);
    }

    fn jack(&mut self, offset: [f32; 3], reach: f32, thick: f32, paint: Paint) {
        self.gem(offset, [reach, thick, thick], paint);
        self.gem(offset, [thick, reach, thick], paint);
        self.gem(offset, [thick, thick, reach], paint);
    }

    fn orb(&mut self, offset: [f32; 3], d: f32, veil: Paint, core: Paint) {
        self.ball(offset, d * 0.55, core);
        self.ball(offset, d, veil);
    }

    fn fins(&mut self, l: f32, len: f32, span: f32, paint: Paint) {
        self.bar(axis(l, 0.0, 0.0), [len, span, 0.3], paint);
        self.bar(axis(l, 0.0, 0.0), [len, 0.3, span], paint);
    }

    fn turn(&mut self, about: [f32; 3], turns: f32) {
        self.body = quat_axis_angle(about, self.travel / TURN_PX * turns * std::f32::consts::TAU);
    }

    fn phase(&self, times: f32) -> f32 {
        self.travel / TURN_PX * times * std::f32::consts::TAU
    }

    fn beads(&mut self, beads: &[(f32, f32, Paint)]) {
        for &(back, d, paint) in beads {
            self.ball(axis(-back, 0.0, 0.0), d, paint);
        }
    }
}

pub fn build(out: &mut Vec<Node>, n: u64, shot: &SceneShot, (cx, cy): (f32, f32)) -> bool {
    if shot.form == ShotForm::Card {
        return false;
    }
    let speed = shot.vx.hypot(shot.vy);
    let (dx, dy) = if speed > 0.0 { (shot.vx / speed, shot.vy / speed) } else { (1.0, 0.0) };
    let (x, y) = (shot.x - cx, shot.y - cy);
    let yaw = quat_axis_angle([0.0, 1.0, 0.0], -dy.atan2(dx));
    let start = out.len();
    let at = [x, shot.dimension.lift(), y];
    let travel = shot.x * dx + shot.y * dy;
    let mut m = Maker { out, next: FIRST_NODE + n * NODES_PER_SHOT, at, yaw, body: QUAT_IDENTITY, travel };
    match shot.form {
        ShotForm::Kind(kind) => kind_shape(&mut m, kind),
        ShotForm::Forged(colour, shape) => forged_shape(&mut m, colour, shape),
        ShotForm::Class(class) => class_shape(&mut m, class),
        ShotForm::Card => return false,
    }
    raise(&mut out[start..], at, [dx, 0.0, dy], travel, shot.dimension);
    if shot.scale != 1.0 {
        for node in &mut out[start..] {
            let t = &mut node.transform;
            for axis in 0..3 {
                t.translation[axis] = at[axis] + (t.translation[axis] - at[axis]) * shot.scale;
            }
            t.scale = t.scale.map(|v| v * shot.scale);
        }
    }
    true
}

fn raise(nodes: &mut [Node], at: [f32; 3], along: [f32; 3], travel: f32, dimension: super::dimensions::Dimension) {
    let (k, roll) = (dimension.scale(), quat_axis_angle(along, travel * dimension.roll()));
    if k == 1.0 && dimension.roll() == 0.0 {
        return;
    }
    for node in nodes {
        let t = &mut node.transform;
        let p = quat_rotate(roll, [(t.translation[0] - at[0]) * k, (t.translation[1] - at[1]) * k, (t.translation[2] - at[2]) * k]);
        t.translation = [at[0] + p[0], at[1] + p[1], at[2] + p[2]];
        if !matches!(node.mesh, Some(MeshRef::Sphere { .. })) {
            t.rotation = quat_mul(roll, t.rotation);
        }
        t.scale = t.scale.map(|v| v * k);
    }
}

fn kind_shape(m: &mut Maker, kind: Shot) {
    let profile = super::volleys::ranged::look(kind);
    let up = [0.0, 1.0, 0.0];
    let across = [0.0, 0.0, 1.0];
    let along = [1.0, 0.0, 0.0];
    match kind {
        Shot::Blade => {
            m.turn(up, 1.0);
            m.bar(axis(0.0, 0.0, 0.0), [9.0, 0.8, 1.8], Paint::lit('v'));
            m.bar(axis(0.0, 0.1, 0.0), [1.8, 0.8, 9.0], Paint::lit('v'));
            m.bar(axis(4.0, 0.0, 0.0), [1.6, 0.9, 1.6], Paint::lit('W'));
            m.bar(axis(-4.0, 0.0, 0.0), [1.6, 0.9, 1.6], Paint::lit('W'));
            m.ball(axis(0.0, 0.5, 0.0), 2.4, Paint::glow('3', 0.6));
        }
        Shot::Arrow => {
            m.bar(axis(-1.0, 0.0, 0.0), [profile.length * 0.8, profile.width, profile.width], Paint::lit('o'));
            m.spike(axis(4.7, 0.0, 0.0), 2.6, 3.4, true, Paint::lit('W'));
            m.fins(-4.4, 2.8, 2.8, Paint::lit('o'));
        }
        Shot::Bolt => {
            m.bar(axis(-0.5, 0.0, 0.0), [profile.length * 0.8, profile.width * 0.7, profile.width * 0.7], Paint::lit('R'));
            m.spike(axis(3.6, 0.0, 0.0), 2.6, 2.8, true, Paint::lit('V'));
            m.fins(-2.7, 1.8, 2.4, Paint::lit(profile.edge));
        }
        Shot::Ball => {
            m.ball(axis(0.0, 0.0, 0.0), profile.width * 1.8, Paint::lit(profile.edge));
            m.ball(axis(1.2, 0.8, 0.0), 2.2, Paint::lit(profile.core));
        }
        Shot::Bone => {
            m.turn(across, 1.0);
            m.bar(axis(0.0, 0.0, 0.0), [1.0, 5.0, 1.0], Paint::lit('h'));
            m.ball(axis(0.0, 2.6, 0.5), 2.2, Paint::lit('H'));
            m.ball(axis(0.0, 2.6, -0.5), 2.2, Paint::lit('H'));
            m.ball(axis(0.0, -2.6, 0.5), 2.2, Paint::lit('H'));
            m.ball(axis(0.0, -2.6, -0.5), 2.2, Paint::lit('H'));
        }
        Shot::Orb => m.orb(axis(0.0, 0.0, 0.0), 6.4, Paint::veil('2', 0.8, 0.6), Paint::glow('9', 1.0)),
        Shot::Ember => m.orb(axis(0.0, 0.0, 0.0), 6.6, Paint::veil('@', 0.9, 0.7), Paint::glow('c', 1.0)),
        Shot::Sacred => {
            m.bar(axis(-3.0, 0.0, 0.0), [14.0, 1.1, 1.1], Paint::glow('6', 0.9));
            m.spike(axis(6.4, 0.0, 0.0), 3.4, 4.8, true, Paint::glow('9', 1.0));
            m.fins(-9.0, 3.4, 4.0, Paint::glow('5', 0.8));
            m.fins(-6.0, 2.6, 3.2, Paint::glow('6', 0.8));
        }
        Shot::Hex => {
            m.orb(axis(0.0, 0.0, 0.0), 6.4, Paint::veil('Y', 0.8, 0.65), Paint::glow('9', 1.0));
            let a = m.phase(2.0);
            m.ball(axis(4.2 * a.cos(), 0.0, 4.2 * a.sin()), 1.7, Paint::glow('C', 0.9));
            m.ball(axis(-4.2 * a.cos(), 0.0, -4.2 * a.sin()), 1.7, Paint::glow('C', 0.9));
        }
        Shot::Frost => {
            m.turn(up, 0.4);
            m.jack(axis(0.0, 0.0, 0.0), 7.0, 1.7, Paint::glow('z', 0.45));
            m.ball(axis(0.0, 0.0, 0.0), 3.2, Paint::glow('9', 1.0));
        }
        Shot::Missile => {
            m.bar(axis(-0.5, 0.0, 0.0), [7.0, 2.2, 2.2], Paint::lit('J'));
            m.bar(axis(0.8, 0.0, 0.0), [1.5, 2.6, 2.6], Paint::lit('7'));
            m.spike(axis(4.9, 0.0, 0.0), 2.2, 2.8, true, Paint::lit('H'));
            m.fins(-3.4, 2.4, 4.4, Paint::lit('G'));
        }
        Shot::Wisp => {
            m.orb(axis(0.0, 0.0, 0.0), 5.8, Paint::veil('3', 0.8, 0.6), Paint::glow('9', 1.0));
            m.beads(&[
                (3.6, 4.2, Paint::veil('3', 0.8, 0.7)),
                (6.4, 3.2, Paint::veil('2', 0.8, 0.7)),
                (8.8, 2.2, Paint::veil('q', 0.8, 0.7)),
            ]);
        }
        Shot::Ore => {
            m.turn([0.3, 1.0, 0.5], 0.8);
            m.solid(MeshRef::Sphere { segments: 5 }, axis(0.0, 0.0, 0.0), QUAT_IDENTITY, [5.2, 4.4, 4.8], Paint::lit('R'));
            m.solid(MeshRef::Gem, axis(1.4, 1.0, -0.6), QUAT_IDENTITY, [3.0, 2.6, 2.6], Paint::lit('o'));
            m.ball(axis(-1.2, -0.8, 1.2), 1.6, Paint::lit('T'));
        }
        Shot::Flame => {
            m.spike(axis(-5.2, 0.0, 0.0), 5.4, 9.0, false, Paint::veil('@', 0.9, 0.75));
            m.orb(axis(0.0, 0.0, 0.0), 6.2, Paint::veil('@', 0.9, 0.7), Paint::glow('c', 1.0));
        }
        Shot::Page => {
            m.turn(along, 1.2);
            m.bar(axis(0.0, 0.0, 0.0), [4.4, 0.4, 5.4], Paint::lit('$'));
            m.bar(axis(0.0, 0.3, 0.2), [3.0, 0.3, 0.6], Paint::lit('Q'));
            m.bar(axis(0.0, 0.3, -1.2), [2.2, 0.3, 0.5], Paint::lit('Q'));
        }
        Shot::Spore => {
            m.ball(axis(0.0, 0.0, 0.0), 5.2, Paint::glow('Y', 0.6));
            m.ball(axis(0.0, 0.0, 0.0), 7.2, Paint::veil('C', 0.7, 0.4));
            let a = m.phase(1.5);
            m.ball(axis(3.6 * a.cos(), 1.0, 3.6 * a.sin()), 1.6, Paint::glow('y', 0.8));
            m.ball(axis(-3.6 * a.cos(), -1.0, -3.6 * a.sin()), 1.6, Paint::glow('y', 0.8));
        }
        Shot::Eye => {
            m.ball(axis(0.0, 0.0, 0.0), 5.8, Paint::glow('9', 0.35));
            m.ball_in_world([0.0, 1.5, 2.4], 3.4, Paint::lit('1'));
            m.ball_in_world([0.0, 1.9, 3.1], 1.8, Paint::lit('0'));
        }
    }
}

fn forged_shape(m: &mut Maker, colour: ForgedColour, shape: ForgedShape) {
    let (core, rim) = match colour {
        ForgedColour::Ember => ('6', '7'),
        ForgedColour::Ice => ('w', '3'),
        ForgedColour::Gold => ('6', '4'),
        ForgedColour::Venom => ('3', '2'),
        ForgedColour::Void => ('1', '0'),
        ForgedColour::Rose => ('9', '7'),
        ForgedColour::Plain => ('w', 'c'),
    };
    match shape {
        ForgedShape::Arrow => {
            m.bar(axis(-1.0, 0.0, 0.0), [7.0, 0.9, 0.9], Paint::glow(rim, 0.5));
            m.spike(axis(4.4, 0.0, 0.0), 2.6, 3.4, true, Paint::glow(core, 0.9));
            m.fins(-4.2, 2.6, 2.8, Paint::glow(rim, 0.5));
        }
        ForgedShape::Shard => {
            m.gem(axis(0.0, 0.0, 0.0), [9.0, 2.4, 2.4], Paint::glow(rim, 0.55));
            m.gem(axis(0.6, 0.0, 0.0), [5.0, 1.3, 1.3], Paint::glow(core, 1.0));
        }
        ForgedShape::Star => {
            m.jack(axis(0.0, 0.0, 0.0), 6.0, 1.6, Paint::glow(rim, 0.55));
            m.ball(axis(0.0, 0.0, 0.0), 3.0, Paint::glow(core, 1.0));
        }
        ForgedShape::Wave => {
            for (i, s) in [-4.0f32, -2.0, 0.0, 2.0, 4.0].into_iter().enumerate() {
                let paint = Paint::glow(if i == 2 { core } else { rim }, if i == 2 { 1.0 } else { 0.6 });
                m.ball(axis(-0.12 * s * s + 1.0, 0.0, s), if i == 2 { 2.6 } else { 2.0 }, paint);
            }
        }
        ForgedShape::Coal => {
            m.turn([0.4, 1.0, 0.3], 0.8);
            m.solid(MeshRef::Sphere { segments: 5 }, axis(0.0, 0.0, 0.0), QUAT_IDENTITY, [5.6, 4.6, 5.0], Paint::lit('g'));
            m.ball(axis(1.6, 1.0, 0.4), 1.7, Paint::glow('@', 1.0));
            m.ball(axis(-1.4, -0.6, 1.4), 1.4, Paint::glow('7', 1.0));
        }
        ForgedShape::Bolt => m.orb(axis(0.0, 0.0, 0.0), 5.6, Paint::veil(rim, 0.8, 0.65), Paint::glow(core, 1.0)),
    }
}

fn class_shape(m: &mut Maker, class: ClassShot) {
    match class {
        ClassShot::Arcane => {
            m.orb(axis(0.0, 0.0, 0.0), 6.0, Paint::veil('z', 0.8, 0.6), Paint::glow('9', 1.0));
            let wave = m.phase(3.0);
            for (k, (back, ink)) in [(3.2, 'z'), (4.6, 'Q'), (6.0, 'Q'), (7.4, 'q'), (8.8, 'S')].into_iter().enumerate() {
                let sway = (wave + k as f32 * 1.3).sin() * 1.1;
                let d = 3.0 - 0.38 * k as f32;
                m.ball(axis(-back, 0.0, sway), d, Paint::veil(ink, 0.8, 0.8));
            }
            let a = m.phase(2.0);
            m.ball(axis(3.8 * a.cos(), 0.0, 3.8 * a.sin()), 1.6, Paint::glow('V', 0.9));
            m.ball(axis(-3.8 * a.cos(), 0.0, -3.8 * a.sin()), 1.6, Paint::glow('z', 0.9));
        }
        ClassShot::Holy => {
            m.turn([1.0, 0.0, 0.0], 0.7);
            m.jack(axis(0.0, 0.0, 0.0), 7.0, 1.6, Paint::glow('6', 0.85));
            m.ball(axis(0.0, 0.0, 0.0), 3.4, Paint::glow('9', 1.0));
            m.body = QUAT_IDENTITY;
            m.beads(&[
                (4.2, 2.2, Paint::veil('c', 0.9, 0.8)),
                (6.2, 1.8, Paint::veil('o', 0.9, 0.8)),
                (8.0, 1.4, Paint::veil('P', 0.9, 0.8)),
            ]);
        }
        ClassShot::Song => {
            let bob = m.phase(4.0).sin() * 1.2;
            m.ball(axis(0.0, bob, 0.0), 4.2, Paint::glow('6', 1.0));
            for (k, up) in [1.6f32, 3.0, 4.4].into_iter().enumerate() {
                m.ball(axis(1.2, bob + up, 0.0), 1.3 - 0.1 * k as f32, Paint::glow('4', 1.0));
            }
            m.ball(axis(2.4, bob + 4.0, 0.0), 1.5, Paint::glow('5', 1.0));
            let wave = m.phase(3.0);
            for (k, (back, ink)) in [(3.6, '3'), (5.2, '2'), (6.8, '0')].into_iter().enumerate() {
                let sway = (wave + k as f32 * 1.7).sin() * 1.4;
                m.ball(axis(-back, 0.0, sway), 1.8 - 0.35 * k as f32, Paint::glow(ink, 0.9));
            }
        }
    }
}

#[cfg(test)]
pub fn count(form: ShotForm) -> usize {
    let shot = SceneShot {
        x: 40.0,
        y: 40.0,
        vx: 30.0,
        vy: 0.0,
        hue: super::room_scene::Hue::Cold,
        bright: false,
        hostile: false,
        empowered: false,
        form,
        dimension: Default::default(),
        mark: super::Img::new(1, 1),
        paint: None,
        scale: 1.0,
    };
    let mut out = Vec::new();
    build(&mut out, 0, &shot, (0.0, 0.0));
    out.len()
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__shot_meshes_tests.rs"]
mod tests;
