//! A game's scene as layers (docs/RENDER_FOUNDATION.md, "The game
//! pipeline"): what the window (crates/glass) draws on its stage and the
//! browser draws on its canvas, from the same description.
//!
//! A layer is plain data (what, where, how high). In the window, below the
//! text is negative `z`, above is zero or positive; on a stage, layers draw
//! in their order. Layers are solid colours, pictures, glows and motes,
//! turned and lifted toward the camera. `expand` turns a stage's layers into
//! plain quads for a renderer that draws nothing else, and `wire` carries
//! them and their pictures to the browser. `scene` is the 3D description
//! beside it: nodes with transforms, meshes and materials under a camera
//! (docs/GLASS_CORE.md).

pub mod brand;
pub mod look;
pub mod marks;
pub mod scene;
pub mod set;
pub mod shapes;
pub mod primitives;
pub mod wire;
pub mod primitive;

#[derive(Clone, Debug, PartialEq)]
pub enum Placement {
    /// `[x, y, width, height]` in window pixels.
    Pixels([f32; 4]),
    /// `[column, row, columns, rows]`; fractions are allowed.
    Cells([f32; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayerKind {
    /// A flat colour, straight (not premultiplied) RGBA in 0..1.
    Solid([f32; 4]),
    /// A picture by id (`Host::upload_picture`), over the whole placement.
    /// Kitty pictures from the cockpit use the same ids, so a layer's own
    /// pictures take ids from `LAYER_PICTURES` up.
    Picture(u32),
    /// Part of a picture, `[x, y, width, height]` in its pixels, over the
    /// whole placement: one figure from a sheet of them.
    PicturePart(u32, [f32; 4]),
    /// A soft round glow of this colour (straight RGBA), brightest in the
    /// middle of the placement and gone at its edge: a halo, a lamp.
    Glow([f32; 4]),
    /// Motes the window animates itself, born across the placement.
    Particles(Particles),
    /// Lit signed-distance solid with a luminous silhouette. Shapes 0..6:
    /// cube, sphere, pyramid, cylinder, cone, torus, octahedron. Phase is
    /// radians; speed is radians/second (zero freezes all motion).
    Primitive { shape: u32, color: [f32; 4], phase: f32, speed: f32 },
}

/// Motes: embers off a fire, dust in the light, sparks from a blow. Each is
/// a square of `size` pixels. A stream keeps `count` in the air, each living
/// `life` seconds, rising `rise` pixels over it (negative falls) and swaying
/// `sway` either way, born somewhere new each time. A burst (`burst` is how
/// long ago it went off) throws `count` out from the placement's middle at
/// up to `rise` pixels a second, falling with `sway` pixels a second
/// squared, gone after `life`. Every mote fades in and out; `seed` keeps two
/// emitters apart. All of it is a function of time, so nothing is kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Particles {
    /// Straight RGBA at a mote's brightest.
    pub color: [f32; 4],
    pub count: u16,
    pub size: f32,
    pub life: f32,
    pub rise: f32,
    pub sway: f32,
    pub seed: u32,
    pub burst: Option<f32>,
}

/// Where each mote of `p` born in `rect` is at `t` seconds, and how bright
/// (0 to 1): `[x, y, alpha]`.
pub fn motes(p: &Particles, rect: [f32; 4], t: f32) -> Vec<[f32; 3]> {
    let [x, y, w, h] = rect;
    let life = p.life.max(0.05);
    let mut out = Vec::with_capacity(usize::from(p.count));
    for i in 0..u32::from(p.count) {
        let r = |salt: u32, cycle: u32| unit(p.seed, i, salt, cycle);
        if let Some(age) = p.burst {
            if !(0.0..life).contains(&age) {
                break;
            }
            let k = age / life;
            let angle = r(1, 0) * std::f32::consts::TAU;
            let speed = p.rise * (0.35 + 0.65 * r(2, 0));
            let (vx, vy) = (angle.cos() * speed, angle.sin() * speed * 0.7);
            out.push([
                x + w / 2.0 + vx * age,
                y + h / 2.0 + vy * age + 0.5 * p.sway * age * age,
                1.0 - k * k,
            ]);
            continue;
        }
        let clock = t + r(0, 0) * life;
        let cycle = (clock / life).floor();
        let k = clock / life - cycle;
        let cycle = cycle as u32;
        let wave = (clock * 1.7 + r(3, 0) * std::f32::consts::TAU).sin();
        let fade = if k < 0.2 { k / 0.2 } else { (1.0 - k) / 0.8 };
        out.push([
            x + r(1, cycle) * w + p.sway * wave,
            y + r(2, cycle) * h - p.rise * k,
            fade,
        ]);
    }
    out
}

/// A steady number in [0, 1) for mote `i` of emitter `seed`.
fn unit(seed: u32, i: u32, salt: u32, cycle: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9E37_79B1)
        ^ i.wrapping_mul(0x85EB_CA77)
        ^ salt.wrapping_mul(0xC2B2_AE3D)
        ^ cycle.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embers(burst: Option<f32>) -> Particles {
        Particles {
            color: [1.0, 0.6, 0.3, 1.0],
            count: 8,
            size: 1.0,
            life: 2.0,
            rise: 20.0,
            sway: 2.0,
            seed: 7,
            burst,
        }
    }

    #[test]
    fn a_stream_keeps_its_motes_in_the_air_near_where_they_are_born() {
        let p = embers(None);
        for t in [0.0, 0.7, 13.3] {
            let m = motes(&p, [10.0, 10.0, 6.0, 4.0], t);
            assert_eq!(m.len(), 8);
            for [x, y, a] in m {
                assert!((8.0..=18.0).contains(&x), "x {x}");
                assert!((-10.0..=14.0).contains(&y), "y {y}");
                assert!((0.0..=1.0).contains(&a));
            }
        }
        // The same moment is the same motes.
        assert_eq!(motes(&p, [0.0; 4], 3.0), motes(&p, [0.0; 4], 3.0));
    }

    #[test]
    fn a_burst_spreads_then_is_gone() {
        let early = motes(&embers(Some(0.05)), [0.0, 0.0, 10.0, 10.0], 0.0);
        let late = motes(&embers(Some(1.5)), [0.0, 0.0, 10.0, 10.0], 0.0);
        let spread = |m: &[[f32; 3]]| m.iter().map(|p| (p[0] - 5.0).hypot(p[1] - 5.0)).sum::<f32>();
        assert!(spread(&late) > spread(&early) * 5.0);
        assert!(motes(&embers(Some(2.5)), [0.0; 4], 0.0).is_empty());
    }
}

/// How a layer stands when its stage is seen at a tilt (`Frame::tilt`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Stand {
    /// Painted on the floor: it tips away with it (the room, its light).
    #[default]
    Floor,
    /// Standing on the floor at this point (placement pixels), facing the
    /// camera: smaller further away, never tipped (knights, monsters, shots).
    /// Everything drawn on one figure shares its foot.
    Upright([f32; 2]),
    /// On the glass in front of the scene, untouched by the tilt (the
    /// screen's own marks).
    Screen,
}

/// A stage's floor seen at a tilt (`Frame::tilt`), in the stage's own
/// pixels: rows further up stand further away. Every drawer projects with
/// it the same way (crates/glass `glass.wgsl`, the browser's `stage.js`):
/// a point `p` on the floor, `d = p - centre`, depth `w = 1 - near·d.y`,
/// lands at `centre + (zoom·d.x, zoom·squash·d.y) / w`; what stands upright
/// is placed on its foot so and scaled by `zoom / w`, never tipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tilt {
    pub centre: [f32; 2],
    pub near: f32,
    pub zoom: f32,
    pub squash: f32,
}

/// The most a stage's floor may tip.
pub const MAX_TILT: f32 = 0.6;

impl Tilt {
    /// `tilt` over a stage of `size` pixels (any scale of it gives the same
    /// picture); none when it is flat.
    pub fn for_stage(tilt: f32, (w, h): (u32, u32)) -> Option<Self> {
        if tilt.is_nan() || tilt <= 0.0 || w == 0 || h == 0 {
            return None;
        }
        let tilt = tilt.min(MAX_TILT);
        let (w, h) = (w as f32, h as f32);
        // The row that keeps its size sits low, near the near wall; the far
        // wall recedes into the black above.
        const PIVOT: f32 = 0.85;
        let squash = 1.0 - tilt * 0.25;
        Some(Self {
            centre: [w / 2.0, h * PIVOT],
            near: tilt / h,
            // Zoomed so the near edge lands on the stage's bottom edge: the
            // whole floor stays in view.
            zoom: (1.0 - tilt * (1.0 - PIVOT)) / squash,
            squash,
        })
    }

    /// How much larger something standing on floor point `p` is drawn.
    pub fn scale_at(&self, p: [f32; 2]) -> f32 {
        self.zoom / (1.0 - self.near * (p[1] - self.centre[1])).max(0.05)
    }

    /// Where floor point `p` lands.
    pub fn floor(&self, p: [f32; 2]) -> [f32; 2] {
        let d = [p[0] - self.centre[0], p[1] - self.centre[1]];
        let w = (1.0 - self.near * d[1]).max(0.05);
        [self.centre[0] + self.zoom * d[0] / w, self.centre[1] + self.zoom * self.squash * d[1] / w]
    }
}

/// Picture ids from here up belong to layers; the cockpit's Kitty pictures
/// stay below.
pub const LAYER_PICTURES: u32 = 0x8000_0000;

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: u64,
    pub z: i32,
    pub placement: Placement,
    pub opacity: f32,
    pub kind: LayerKind,
    /// A turn, in radians, about the placement's centre.
    pub angle: f32,
    /// How far the layer has lifted toward the camera, in pixels. It grows
    /// and rises as it lifts while its shadow stays on the ground: a lobbed
    /// bomb, a boss's boulder. 0 is on the ground, with no shadow.
    pub height: f32,
    /// A slow breath: the layer rises and settles by up to this many pixels
    /// (on top of `height`) every few seconds, animated by the window itself.
    pub breathe: f32,
    /// Light: the layer adds to what is under it instead of covering it (a
    /// glow lighting a room).
    pub additive: bool,
    /// A flame's flicker: the layer's strength wavers by up to this part of
    /// itself, animated by the window, each layer in its own time (by id).
    pub flicker: f32,
    /// How it stands when the stage is tilted.
    pub stand: Stand,
}

/// The furthest a layer may lift, in pixels: the largest texture side a
/// host ever deals in. A lift past this is a producer bug, and an
/// unbounded one would grow a screen-filling quad without limit.
pub const MAX_LIFT_PX: f32 = 8192.0;

/// The most motes one emitter may keep in the air. An uncapped count is
/// an uncapped quad allocation per frame in every host.
pub const MAX_MOTES: u16 = 4096;

/// The most quads one frame's layers may expand to. Layers past the cap
/// simply do not draw; a producer asking for more has a bug, and the
/// frame's cost stays bounded whatever it sends.
pub const MAX_QUADS: usize = 1_000_000;

impl Layer {
    /// This layer made safe to draw, or `None` when it must not be drawn at
    /// all. Layers are plain data and arrive from games and, soon, remote
    /// seats, so nothing a producer sends may reach a renderer unchecked:
    /// a non-finite placement, colour or lift becomes NaN vertices and
    /// driver-defined output, and a non-positive size has no quad. Finite
    /// but out-of-contract values are clamped to the documented ranges
    /// (straight RGBA in 0..1, lifts from the ground up, flicker no
    /// stronger than the flame itself). Every host calls this at its draw
    /// boundary: the window's compositor, and `expand` for the browser.
    pub fn sanitized(mut self) -> Option<Self> {
        let rect = match self.placement {
            Placement::Pixels(rect) | Placement::Cells(rect) => rect,
        };
        if !rect.iter().all(|v| v.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
            return None;
        }
        if ![self.opacity, self.angle, self.height, self.breathe, self.flicker]
            .iter()
            .all(|v| v.is_finite())
        {
            return None;
        }
        fn clamp_rgba(rgba: &mut [f32; 4]) -> bool {
            if !rgba.iter().all(|v| v.is_finite()) {
                return false;
            }
            rgba.iter_mut().for_each(|v| *v = v.clamp(0.0, 1.0));
            true
        }
        match &mut self.kind {
            LayerKind::Solid(rgba) | LayerKind::Glow(rgba) => {
                if !clamp_rgba(rgba) {
                    return None;
                }
            }
            LayerKind::Picture(_) => {}
            LayerKind::Primitive { shape, color, phase, speed } => {
                if *shape > 6 || !clamp_rgba(color) || !phase.is_finite() || !speed.is_finite() {
                    return None;
                }
                *phase = phase.rem_euclid(std::f32::consts::TAU);
                *speed = speed.clamp(-2.0, 2.0);
            }
            LayerKind::PicturePart(_, source) => {
                if !source.iter().all(|v| v.is_finite()) || source[2] <= 0.0 || source[3] <= 0.0 {
                    return None;
                }
            }
            LayerKind::Particles(p) => {
                if !clamp_rgba(&mut p.color)
                    || ![p.size, p.life, p.rise, p.sway].iter().all(|v| v.is_finite())
                    || p.burst.is_some_and(|b| !b.is_finite())
                    || p.size <= 0.0
                {
                    return None;
                }
                p.count = p.count.min(MAX_MOTES);
                p.size = p.size.min(1024.0);
                p.life = p.life.clamp(0.05, 3600.0);
                p.rise = p.rise.clamp(-MAX_LIFT_PX, MAX_LIFT_PX);
                p.sway = p.sway.clamp(-MAX_LIFT_PX, MAX_LIFT_PX);
            }
        }
        self.opacity = self.opacity.clamp(0.0, 1.0);
        self.height = self.height.clamp(0.0, MAX_LIFT_PX);
        self.breathe = self.breathe.clamp(0.0, MAX_LIFT_PX);
        self.flicker = self.flicker.clamp(0.0, 1.0);
        self.angle = self.angle.rem_euclid(std::f32::consts::TAU);
        // A foot nowhere stands on the floor instead.
        if let Stand::Upright(foot) = self.stand
            && !foot.iter().all(|v| v.is_finite())
        {
            self.stand = Stand::Floor;
        }
        Some(self)
    }

    /// [`sanitized`](Self::sanitized) without the copy when the layer is
    /// already sound — the hot path, called per layer per frame by every
    /// host. Sound means sanitizing would change nothing.
    pub fn sanitized_cow(&self) -> Option<std::borrow::Cow<'_, Layer>> {
        if self.is_sound() {
            Some(std::borrow::Cow::Borrowed(self))
        } else {
            self.clone().sanitized().map(std::borrow::Cow::Owned)
        }
    }

    /// Already exactly as [`sanitized`](Self::sanitized) would return it.
    fn is_sound(&self) -> bool {
        let rect = match self.placement {
            Placement::Pixels(rect) | Placement::Cells(rect) => rect,
        };
        let in_unit = |v: f32| (0.0..=1.0).contains(&v);
        let rgba_sound = |rgba: &[f32; 4]| rgba.iter().all(|&v| in_unit(v));
        rect.iter().all(|v| v.is_finite())
            && rect[2] > 0.0
            && rect[3] > 0.0
            && in_unit(self.opacity)
            && (0.0..std::f32::consts::TAU).contains(&self.angle)
            && (0.0..=MAX_LIFT_PX).contains(&self.height)
            && (0.0..=MAX_LIFT_PX).contains(&self.breathe)
            && in_unit(self.flicker)
            && !matches!(self.stand, Stand::Upright(foot) if !foot.iter().all(|v| v.is_finite()))
            && match &self.kind {
                LayerKind::Solid(rgba) | LayerKind::Glow(rgba) => rgba_sound(rgba),
                LayerKind::Picture(_) => true,
                LayerKind::Primitive { shape, color, phase, speed } => {
                    *shape <= 6 && rgba_sound(color)
                        && (0.0..std::f32::consts::TAU).contains(phase)
                        && (-2.0..=2.0).contains(speed)
                }
                LayerKind::PicturePart(_, s) => {
                    s.iter().all(|v| v.is_finite()) && s[2] > 0.0 && s[3] > 0.0
                }
                LayerKind::Particles(p) => {
                    rgba_sound(&p.color)
                        && p.count <= MAX_MOTES
                        && (0.0..=1024.0).contains(&p.size)
                        && p.size > 0.0
                        && (0.05..=3600.0).contains(&p.life)
                        && (-MAX_LIFT_PX..=MAX_LIFT_PX).contains(&p.rise)
                        && (-MAX_LIFT_PX..=MAX_LIFT_PX).contains(&p.sway)
                        && p.burst.is_none_or(|b| b.is_finite())
                }
            }
    }

    /// Whether this layer needs autonomous compositor frames.
    pub fn animating(&self) -> bool {
        self.breathe > 0.0 || self.flicker > 0.0
            || matches!(self.kind, LayerKind::Particles(_))
            || matches!(self.kind, LayerKind::Primitive { speed, .. } if speed != 0.0)
    }

    /// A layer on the ground, unturned, fully opaque.
    pub fn new(id: u64, z: i32, placement: Placement, kind: LayerKind) -> Self {
        Self {
            id,
            z,
            placement,
            opacity: 1.0,
            kind,
            angle: 0.0,
            height: 0.0,
            breathe: 0.0,
            additive: false,
            flicker: 0.0,
            stand: Stand::Floor,
        }
    }
}

/// A flame's strength at `t` seconds: 1, wavering by up to `amount` either
/// way on two slow waves whose phases come from `seed`, so no two flames
/// keep time.
pub fn flicker(amount: f32, seed: u64, t: f32) -> f32 {
    if amount <= 0.0 {
        return 1.0;
    }
    let phase = (seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f32 / 16_777_216.0
        * std::f32::consts::TAU;
    let wave = 0.6 * (t * FLICKER_HZ[0] * std::f32::consts::TAU + phase).sin()
        + 0.4 * (t * FLICKER_HZ[1] * std::f32::consts::TAU + phase * 2.7).sin();
    1.0 + amount * wave
}

/// A flicker's two waves, a second.
const FLICKER_HZ: [f32; 2] = [1.3, 3.1];

/// A picture a stage's layers name by `id`: straight (not premultiplied)
/// RGBA rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One frame of a stage: its canvas in its own pixels, its layers back to
/// front, the pictures that are new or changed since the last frame, and
/// which scene it is, with the way a new one comes in (`[1, 0]`: from the
/// right).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub size: (u32, u32),
    pub layers: Vec<Layer>,
    pub pictures: Vec<Picture>,
    pub scene: u64,
    pub enter: Option<[f32; 2]>,
    /// How far the floor tips away from the camera: 0 is straight down, as
    /// painted; at 0.3 the far edge stands 1.3 times as far as the near one.
    /// What stands upright stays facing the camera on it.
    pub tilt: f32,
    /// 3D things on the stage, between its layers (`scene::Props`).
    pub props: Option<scene::Props>,
    /// How hard the world was just hit, 0 (nothing) to 1 (a guardian's
    /// fall): a renderer's post pass splits the colours and lifts the light
    /// by it for as long as it lasts. The screen's own marks are untouched.
    pub impact: f32,
}

/// What a quad shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Flat colour, or a picture's texels times the colour's alpha.
    Fill,
    /// A light's round falloff in the colour, full at the middle.
    Round,
}

/// One rectangle to draw, in the stage's pixels: where, which part of
/// which picture (0: none), and its colour, premultiplied. A colour with no
/// alpha adds to what is under it (light). It turns by `angle` about its
/// middle, and stands on a tilted floor as `stand` says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub rect: [f32; 4],
    pub picture: u32,
    pub source: [f32; 4],
    pub color: [f32; 4],
    pub shape: Shape,
    pub angle: f32,
    pub stand: Stand,
}

/// A stage's layers as quads at `t` seconds, back to front, for a renderer
/// that draws nothing else (the browser). `size` gives each named picture's
/// size, for a layer that shows all of one. Each quad keeps its layer's turn
/// and stand; lifting is left to the window (a stage's layers never lift).
pub fn expand(layers: &[Layer], t: f32, size: impl Fn(u32) -> Option<(u32, u32)>) -> Vec<Quad> {
    let mut out = Vec::with_capacity(layers.len());
    for layer in layers {
        if out.len() >= MAX_QUADS {
            break;
        }
        // Nothing unchecked becomes a quad (`Layer::sanitized`).
        let Some(layer) = layer.sanitized_cow() else {
            continue;
        };
        let layer = &layer;
        let rect = match layer.placement {
            Placement::Pixels(rect) | Placement::Cells(rect) => rect,
        };
        let coverage = if layer.additive { 0.0 } else { 1.0 };
        let strength = layer.opacity * flicker(layer.flicker, layer.id, t);
        match &layer.kind {
            LayerKind::Primitive { shape, color, phase, speed } => {
                // Quad-only hosts get a bounded projection; Glass evaluates
                // the surface continuously in the fragment shader.
                const SIDE: usize = 24;
                for row in 0..SIDE {
                    for col in 0..SIDE {
                        if out.len() >= MAX_QUADS { break; }
                        let shade = primitive::sample(*shape, *phase + t * *speed,
                            (col as f32 + 0.5) / SIDE as f32 * 2.8 - 1.4,
                            1.4 - (row as f32 + 0.5) / SIDE as f32 * 2.8);
                        if shade <= 0.0 { continue; }
                        let a = color[3] * strength;
                        out.push(Quad {
                            rect: [rect[0] + col as f32 * rect[2] / SIDE as f32,
                                rect[1] + row as f32 * rect[3] / SIDE as f32,
                                rect[2] / SIDE as f32, rect[3] / SIDE as f32],
                            picture: 0, source: [0.0; 4],
                            color: [color[0] * shade * a, color[1] * shade * a, color[2] * shade * a, a * coverage],
                            shape: Shape::Fill, angle: layer.angle, stand: layer.stand,
                        });
                    }
                }
            }
            LayerKind::Solid(c) => {
                let a = c[3] * strength;
                out.push(Quad {
                    rect,
                    picture: 0,
                    source: [0.0; 4],
                    color: [c[0] * a, c[1] * a, c[2] * a, a * coverage],
                    shape: Shape::Fill,
                    angle: layer.angle,
                    stand: layer.stand,
                });
            }
            LayerKind::Glow(c) => {
                let a = c[3] * strength;
                out.push(Quad {
                    rect,
                    picture: 0,
                    source: [0.0; 4],
                    color: [c[0] * a, c[1] * a, c[2] * a, a * coverage],
                    shape: Shape::Round,
                    angle: layer.angle,
                    stand: layer.stand,
                });
            }
            LayerKind::Picture(id) => {
                let Some((w, h)) = size(*id) else {
                    continue;
                };
                out.push(Quad {
                    rect,
                    picture: *id,
                    source: [0.0, 0.0, w as f32, h as f32],
                    color: [1.0, 1.0, 1.0, strength],
                    shape: Shape::Fill,
                    angle: layer.angle,
                    stand: layer.stand,
                });
            }
            LayerKind::PicturePart(id, source) => out.push(Quad {
                rect,
                picture: *id,
                source: *source,
                color: [1.0, 1.0, 1.0, strength],
                shape: Shape::Fill,
                angle: layer.angle,
                stand: layer.stand,
            }),
            LayerKind::Particles(p) => {
                let a = p.color[3] * strength;
                for [x, y, fade] in motes(p, rect, t) {
                    if out.len() >= MAX_QUADS {
                        break;
                    }
                    if fade <= 0.0 {
                        continue;
                    }
                    let k = a * fade;
                    out.push(Quad {
                        rect: [x - p.size / 2.0, y - p.size / 2.0, p.size, p.size],
                        picture: 0,
                        source: [0.0; 4],
                        color: [p.color[0] * k, p.color[1] * k, p.color[2] * k, k * coverage],
                        shape: Shape::Fill,
                        angle: layer.angle,
                        stand: layer.stand,
                    });
                }
            }
        }
    }
    out
}

/// [`expand`] a frame's layers around its props: the quads, and how many of
/// them lie under the props, which `at` counts in layers (a layer may be many
/// quads, motes, or none, if it is unsound), and the wire counts in quads.
pub fn expand_at(
    layers: &[Layer],
    at: usize,
    t: f32,
    size: impl Fn(u32) -> Option<(u32, u32)>,
) -> (Vec<Quad>, usize) {
    let at = at.min(layers.len());
    let mut quads = expand(&layers[..at], t, &size);
    let under = quads.len();
    quads.extend(expand(&layers[at..], t, &size));
    quads.truncate(MAX_QUADS);
    (quads, under.min(MAX_QUADS))
}

/// [`expand`] a frame's layers with places marked among them (each mark a
/// count of layers, as props and a world name where they lie): the quads,
/// and each mark as the count of quads under it, which is how the wire
/// counts them.
pub fn expand_marks(
    layers: &[Layer],
    marks: &[usize],
    t: f32,
    size: impl Fn(u32) -> Option<(u32, u32)>,
) -> (Vec<Quad>, Vec<usize>) {
    let mut cuts: Vec<usize> = marks.iter().map(|&m| m.min(layers.len())).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut quads = Vec::new();
    let mut under = Vec::with_capacity(cuts.len());
    let mut from = 0;
    for &cut in &cuts {
        quads.extend(expand(&layers[from..cut], t, &size));
        under.push(quads.len().min(MAX_QUADS));
        from = cut;
    }
    quads.extend(expand(&layers[from..], t, &size));
    quads.truncate(MAX_QUADS);
    let counted = marks
        .iter()
        .map(|&m| under[cuts.binary_search(&m.min(layers.len())).unwrap_or(0)])
        .collect();
    (quads, counted)
}

/// Quads as layers, the other way from [`expand`]: for a renderer that draws
/// layers (the browser's `angel-gpu`, which compiles them the way the window
/// does) given the quads the wire carries. A quad is a layer that has had its
/// moment (opacity applied, flame flickered, motes born), so it comes back as
/// a layer with none of those left to do: `expand(&layers_of(&quads), t, ..)`
/// gives the quads again, at any `t`, up to the float rounding of dividing a
/// premultiplied colour by its alpha and multiplying it back.
///
/// - A picture quad is the part of the picture its `source` names, over the
///   quad's rect, at its alpha.
/// - A round quad is a glow, a flat one a solid.
/// - A colour with no alpha adds to what is under it (a light): it comes back
///   `additive`, its premultiplied colour as the straight colour at full alpha.
/// - Strength past 1 (a flame flickering up) is clamped to 1, as any layer's is.
pub fn layers_of(quads: &[Quad]) -> Vec<Layer> {
    quads
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let [r, g, b, a] = q.color;
            let place = Placement::Pixels(q.rect);
            let mut layer = if q.shape == Shape::Fill && q.picture != 0 {
                let mut layer = Layer::new(i as u64, 0, place, LayerKind::PicturePart(q.picture, q.source));
                layer.opacity = a;
                layer
            } else {
                // Straight colour and alpha from the premultiplied one; no
                // alpha is light, which adds (and has no alpha to give back).
                let additive = a.is_nan() || a <= 0.0;
                let straight = if additive { [r, g, b, 1.0] } else { [r / a, g / a, b / a, a] };
                let kind = if q.shape == Shape::Round { LayerKind::Glow(straight) } else { LayerKind::Solid(straight) };
                let mut layer = Layer::new(i as u64, 0, place, kind);
                layer.additive = additive;
                layer
            };
            layer.angle = q.angle;
            layer.stand = q.stand;
            layer
        })
        .collect()
}

#[cfg(test)]
mod expand_tests {
    use super::*;

    #[test]
    fn a_light_adds_and_a_picture_shows_whole() {
        let mut light = Layer::new(1, 0, Placement::Pixels([0.0, 0.0, 8.0, 8.0]), LayerKind::Glow([1.0, 0.5, 0.0, 0.5]));
        light.additive = true;
        let picture = Layer::new(2, 0, Placement::Pixels([4.0, 4.0, 2.0, 2.0]), LayerKind::Picture(9));
        let quads = expand(&[light, picture], 0.0, |id| (id == 9).then_some((3, 5)));
        assert_eq!(quads.len(), 2);
        assert_eq!(quads[0].shape, Shape::Round);
        assert_eq!(quads[0].color, [0.5, 0.25, 0.0, 0.0], "premultiplied, no coverage");
        assert_eq!(quads[1].source, [0.0, 0.0, 3.0, 5.0]);
        assert_eq!(quads[1].picture, 9);
    }

    #[test]
    fn the_props_place_counts_quads_not_layers() {
        let solid = |id| Layer::new(id, 0, Placement::Pixels([0.0, 0.0, 4.0, 4.0]), LayerKind::Solid([1.0; 4]));
        let motes = Layer::new(
            9,
            0,
            Placement::Pixels([0.0, 0.0, 8.0, 8.0]),
            LayerKind::Particles(Particles { color: [1.0; 4], count: 5, size: 1.0, life: 2.0, rise: 0.0, sway: 0.0, seed: 1, burst: None }),
        );
        let mut unsound = solid(8);
        unsound.placement = Placement::Pixels([f32::NAN, 0.0, 4.0, 4.0]);
        // Under the props: a solid, an unsound layer (no quad), five motes; over: a solid.
        let layers = [solid(1), unsound, motes, solid(2)];
        let (quads, under) = expand_at(&layers, 3, 0.5, |_| None);
        assert_eq!(under, 1 + 5, "motes that are bright enough to show");
        assert_eq!(quads.len(), 1 + 5 + 1);
        assert_eq!(expand_at(&layers, 0, 0.5, |_| None).1, 0);
        assert_eq!(expand_at(&layers, 99, 0.5, |_| None).1, quads.len(), "past the end is the end");
        assert_eq!(quads, expand(&layers, 0.5, |_| None), "the same quads, split or whole");
        // Many marks at once, in any order, each counted as `expand_at` counts one.
        let marks = [3, 0, 99, 1, 3, 2];
        let (many, counted) = expand_marks(&layers, &marks, 0.5, |_| None);
        assert_eq!(many, quads);
        let one: Vec<usize> = marks.iter().map(|&m| expand_at(&layers, m, 0.5, |_| None).1).collect();
        assert_eq!(counted, one);
        assert_eq!(expand_marks(&layers, &[], 0.5, |_| None), (quads, vec![]));
    }

    #[test]
    fn unsound_layers_never_become_quads() {
        let sound = || {
            Layer::new(
                1,
                0,
                Placement::Pixels([10.0, 20.0, 30.0, 40.0]),
                LayerKind::Solid([0.25, 0.5, 0.75, 1.0]),
            )
        };
        assert_eq!(sound().sanitized(), Some(sound()));
        let mut nan_place = sound();
        nan_place.placement = Placement::Pixels([f32::NAN, 0.0, 10.0, 10.0]);
        assert_eq!(nan_place.sanitized(), None);
        let mut inf_height = sound();
        inf_height.height = f32::INFINITY;
        assert_eq!(inf_height.sanitized(), None);
        let mut flat = sound();
        flat.placement = Placement::Cells([0.0, 0.0, 0.0, 4.0]);
        assert_eq!(flat.sanitized(), None);
        // expand drops them too: one sound layer in, its quad out.
        let quads = expand(&[sound(), nan_place2()], 0.0, |_| None);
        assert_eq!(quads.len(), 1);
        fn nan_place2() -> Layer {
            let mut layer = Layer::new(
                2,
                0,
                Placement::Pixels([0.0, f32::INFINITY, 10.0, 10.0]),
                LayerKind::Glow([1.0, 1.0, 1.0, 1.0]),
            );
            layer.flicker = f32::NAN;
            layer
        }
    }

    #[test]
    fn out_of_range_values_clamp_to_the_contract() {
        let mut wild = Layer::new(
            1,
            0,
            Placement::Pixels([0.0, 0.0, 10.0, 10.0]),
            LayerKind::Particles(Particles {
                color: [2.0, -1.0, 0.5, 9.0],
                count: u16::MAX,
                size: 5000.0,
                life: 0.0,
                rise: 1.0e9,
                sway: -1.0e9,
                seed: 1,
                burst: None,
            }),
        );
        wild.opacity = 7.5;
        wild.height = -30.0;
        wild.flicker = 4.0;
        wild.angle = std::f32::consts::TAU * 3.0 + 0.5;
        let clean = wild.sanitized().expect("finite clamps, not drops");
        assert_eq!(clean.opacity, 1.0);
        assert_eq!(clean.height, 0.0);
        assert_eq!(clean.flicker, 1.0);
        assert!((clean.angle - 0.5).abs() < 1e-6);
        let LayerKind::Particles(p) = clean.kind else {
            panic!("particles kept");
        };
        assert_eq!(p.color, [1.0, 0.0, 0.5, 1.0]);
        assert_eq!(p.count, MAX_MOTES);
        assert_eq!(p.size, 1024.0);
        assert_eq!(p.life, 0.05);
        assert_eq!(p.rise, MAX_LIFT_PX);
        assert_eq!(p.sway, -MAX_LIFT_PX);
        let bad_part = Layer::new(
            3,
            0,
            Placement::Pixels([0.0, 0.0, 4.0, 4.0]),
            LayerKind::PicturePart(9, [0.0, 0.0, -2.0, 4.0]),
        );
        assert_eq!(bad_part.sanitized(), None, "a part with no area");
    }

    #[test]
    fn expansion_stops_at_the_quad_cap() {
        let emitter = |id: u64| {
            Layer::new(
                id,
                0,
                Placement::Pixels([0.0, 0.0, 100.0, 100.0]),
                LayerKind::Particles(Particles {
                    color: [1.0; 4],
                    count: MAX_MOTES,
                    size: 2.0,
                    life: 10.0,
                    rise: 0.0,
                    sway: 0.0,
                    seed: id as u32,
                    burst: None,
                }),
            )
        };
        // 300 emitters at the mote cap ask for over a million quads.
        let layers: Vec<Layer> = (0..300).map(emitter).collect();
        let quads = expand(&layers, 0.5, |_| None);
        assert!(quads.len() <= MAX_QUADS, "{}", quads.len());
        assert!(quads.len() > MAX_QUADS / 2, "the cap is the limiter, not the scene");
    }

    #[test]
    fn the_cow_agrees_with_sanitized_and_borrows_the_sound() {
        let sound = Layer::new(
            1,
            0,
            Placement::Pixels([1.0, 2.0, 3.0, 4.0]),
            LayerKind::Solid([0.5; 4]),
        );
        assert!(matches!(sound.sanitized_cow(), Some(std::borrow::Cow::Borrowed(_))));
        let mut variants = vec![sound];
        let mut wild = variants[0].clone();
        wild.opacity = 9.0;
        variants.push(wild);
        let mut turned = variants[0].clone();
        turned.angle = -1.0;
        variants.push(turned);
        let mut bad = variants[0].clone();
        bad.flicker = f32::NAN;
        variants.push(bad);
        let mut motes = variants[0].clone();
        motes.kind = LayerKind::Particles(Particles {
            color: [2.0, 0.0, 0.0, 1.0],
            count: 99_999.min(u16::MAX.into()) as u16,
            size: 3.0,
            life: 2.0,
            rise: 0.0,
            sway: 0.0,
            seed: 0,
            burst: None,
        });
        variants.push(motes);
        for layer in variants {
            let cow = layer.sanitized_cow().map(|c| c.into_owned());
            assert_eq!(cow, layer.clone().sanitized(), "{layer:?}");
        }
    }

    #[test]
    fn random_layers_sanitize_to_the_contract_and_the_cow_agrees() {
        // Layers arrive from games and remote seats as plain data: any
        // bit pattern in any field must either drop or land inside the
        // documented ranges, and the fast-path soundness check must
        // agree with the full sanitize on every one.
        let mut state = 0x2545_f491_u64;
        let mut next = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            state >> 33
        };
        let wild_float = |next: &mut dyn FnMut() -> u64| -> f32 {
            match next() % 3 {
                0 => f32::from_bits(next() as u32),
                1 => (next() % 2000) as f32 / 10.0 - 100.0,
                _ => (next() % 200) as f32 / 100.0,
            }
        };
        for _ in 0..3000 {
            let f = |next: &mut dyn FnMut() -> u64| wild_float(next);
            let kind = match next() % 5 {
                0 => LayerKind::Solid([f(&mut next), f(&mut next), f(&mut next), f(&mut next)]),
                1 => LayerKind::Glow([f(&mut next), f(&mut next), f(&mut next), f(&mut next)]),
                2 => LayerKind::Picture(next() as u32),
                3 => LayerKind::PicturePart(
                    next() as u32,
                    [f(&mut next), f(&mut next), f(&mut next), f(&mut next)],
                ),
                _ => LayerKind::Particles(Particles {
                    color: [f(&mut next), f(&mut next), f(&mut next), f(&mut next)],
                    count: next() as u16,
                    size: f(&mut next),
                    life: f(&mut next),
                    rise: f(&mut next),
                    sway: f(&mut next),
                    seed: next() as u32,
                    burst: (next() % 2 == 0).then(|| f(&mut next)),
                }),
            };
            let rect = [f(&mut next), f(&mut next), f(&mut next), f(&mut next)];
            let mut layer = Layer::new(
                next(),
                0,
                if next() % 2 == 0 {
                    Placement::Pixels(rect)
                } else {
                    Placement::Cells(rect)
                },
                kind,
            );
            layer.opacity = f(&mut next);
            layer.angle = f(&mut next);
            layer.height = f(&mut next);
            layer.breathe = f(&mut next);
            layer.flicker = f(&mut next);
            let cow = layer.sanitized_cow().map(|c| c.into_owned());
            let clean = layer.clone().sanitized();
            assert_eq!(cow, clean, "cow disagrees on {layer:?}");
            let Some(clean) = clean else { continue };
            let in_unit = |v: f32| (0.0..=1.0).contains(&v);
            assert!(in_unit(clean.opacity) && in_unit(clean.flicker));
            assert!((0.0..=MAX_LIFT_PX).contains(&clean.height));
            assert!((0.0..=MAX_LIFT_PX).contains(&clean.breathe));
            assert!((0.0..std::f32::consts::TAU).contains(&clean.angle));
            let rect = match clean.placement {
                Placement::Pixels(r) | Placement::Cells(r) => r,
            };
            assert!(rect.iter().all(|v| v.is_finite()) && rect[2] > 0.0 && rect[3] > 0.0);
            match &clean.kind {
                LayerKind::Solid(rgba) | LayerKind::Glow(rgba) => {
                    assert!(rgba.iter().all(|&v| in_unit(v)));
                }
                LayerKind::Particles(p) => {
                    assert!(p.color.iter().all(|&v| in_unit(v)));
                    assert!(p.count <= MAX_MOTES);
                    assert!(p.size > 0.0 && p.size <= 1024.0);
                    assert!((0.05..=3600.0).contains(&p.life));
                    assert!((-MAX_LIFT_PX..=MAX_LIFT_PX).contains(&p.rise));
                    assert!((-MAX_LIFT_PX..=MAX_LIFT_PX).contains(&p.sway));
                }
                _ => {}
            }
            // Expansion of anything that survives stays finite and capped.
            let quads = expand(&[clean], 1.5, |_| Some((64, 64)));
            assert!(quads.len() <= MAX_QUADS);
            assert!(quads.iter().all(|q| {
                q.rect.iter().chain(&q.source).chain(&q.color).all(|v| v.is_finite())
            }));
        }
    }

    #[test]
    fn layers_of_is_the_other_way_from_expand() {
        let mut light = Layer::new(1, 0, Placement::Pixels([0.0, 0.0, 8.0, 8.0]), LayerKind::Glow([1.0, 0.5, 0.0, 0.5]));
        light.additive = true;
        light.angle = 0.7;
        let mut lamp = Layer::new(2, 0, Placement::Pixels([2.0, 3.0, 8.0, 8.0]), LayerKind::Glow([0.2, 0.4, 0.8, 0.6]));
        lamp.stand = Stand::Upright([6.0, 11.0]);
        let mut fire = Layer::new(3, 0, Placement::Pixels([1.0, 1.0, 4.0, 4.0]), LayerKind::Solid([0.9, 0.3, 0.1, 0.8]));
        fire.additive = true;
        let mut fog = Layer::new(4, 0, Placement::Pixels([0.0, 0.0, 30.0, 20.0]), LayerKind::Solid([0.1, 0.2, 0.3, 0.5]));
        fog.opacity = 0.5;
        fog.stand = Stand::Screen;
        let mut knight = Layer::new(5, 0, Placement::Pixels([9.0, 9.0, 3.0, 5.0]), LayerKind::PicturePart(7, [4.0, 0.0, 3.0, 5.0]));
        knight.opacity = 0.75;
        knight.angle = 5.5;
        let sparks = Layer::new(
            6,
            0,
            Placement::Pixels([0.0, 0.0, 10.0, 10.0]),
            LayerKind::Particles(Particles { color: [1.0, 0.8, 0.2, 1.0], count: 5, size: 1.0, life: 2.0, rise: 6.0, sway: 1.0, seed: 3, burst: None }),
        );
        let sheet = Layer::new(7, 0, Placement::Pixels([0.0, 0.0, 6.0, 6.0]), LayerKind::Picture(9));
        let layers = [light, lamp, fire, fog, knight, sparks, sheet];
        let size = |id: u32| (id == 9).then_some((6, 6));
        for t in [0.0, 1.3] {
            let quads = expand(&layers, t, size);
            assert!(quads.len() > layers.len(), "the motes became quads");
            let back = expand(&layers_of(&quads), t + 100.0, |_| None);
            assert_eq!(back.len(), quads.len());
            for (a, b) in quads.iter().zip(&back) {
                assert_eq!((a.picture, a.shape, a.stand), (b.picture, b.shape, b.stand), "{a:?} {b:?}");
                let close = |x: &[f32], y: &[f32]| x.iter().zip(y).all(|(x, y)| (x - y).abs() < 1e-5);
                assert!(close(&a.rect, &b.rect) && close(&a.source, &b.source) && close(&a.color, &b.color), "{a:?} {b:?}");
                let turn = (a.angle - b.angle).rem_euclid(std::f32::consts::TAU);
                assert!(!(1e-5..=std::f32::consts::TAU - 1e-5).contains(&turn), "{a:?} {b:?}");
            }
        }
        // Light comes back as light, a covering colour as covering.
        let quads = expand(&layers[..3], 0.0, size);
        let back = layers_of(&quads);
        assert!(back[0].additive && !back[1].additive && back[2].additive);
    }

    #[test]
    fn a_flame_wavers_about_its_strength() {
        let values: Vec<f32> = (0..50).map(|i| flicker(0.2, 7, i as f32 * 0.1)).collect();
        assert!(values.iter().all(|v| (0.8..=1.2).contains(v)));
        assert!(values.iter().any(|v| *v > 1.05) && values.iter().any(|v| *v < 0.95));
        assert_eq!(flicker(0.0, 7, 3.0), 1.0);
    }
}
