//! The layer compiler: layers in z order become instances, each lifted layer
//! with its shadow, each lit layer with its light, and on a tilted stage
//! each stamped with how it stands on the floor.

use angel_stage::{Layer, LayerKind, Placement, Stand, Tilt, motes};
use bytemuck::Zeroable;

use crate::Core;
use crate::frame::{Draw, Instance, MASK, PRIMITIVE, extend_part, push_part};

/// A lifted layer: how far away the camera is (a layer this high is twice
/// its size), how much of its lift shows as rising up the screen, and the
/// height its shadow fades out at.
const CAMERA_PX: f32 = 600.0;
const RISE: f32 = 0.5;
const SHADOW_FADE_PX: f32 = 900.0;
const SHADOW_LAYER_ALPHA: f32 = 0.55;

/// How long one breath takes (`Layer::breathe`).
const BREATH_SECONDS: f32 = 3.2;

impl Core {
    /// Layers in z order as draw parts: for each, its shadow on the ground
    /// when it is lifted, then the layer itself. `now` is the host's clock
    /// in seconds since a start of its choosing: what breathes, flickers or
    /// drifts (motes) is a function of it, and the first breath begins when
    /// the first breathing layer is seen.
    pub fn layer_parts<'a>(
        &mut self,
        layers: impl Iterator<Item = &'a Layer>,
        origin: (f32, f32),
        cw: f32,
        ch: f32,
        tilt: Option<Tilt>,
        now: f64,
    ) -> Vec<(Draw, Vec<Instance>)> {
        // The window's side of `angel_stage::MAX_QUADS`: however many
        // layers arrive, one call emits at most this many instances.
        const MAX_INSTANCES: usize = 1_000_000;
        let mut emitted = 0usize;
        let mut parts = Vec::new();
        for layer in layers {
            if emitted >= MAX_INSTANCES {
                break;
            }
            // Nothing unchecked reaches the GPU (`Layer::sanitized`).
            let Some(layer) = layer.sanitized_cow() else {
                continue;
            };
            let layer = &layer;
            let [x, y, w, h] = match layer.placement {
                Placement::Pixels(rect) => rect,
                Placement::Cells([x, y, w, h]) => {
                    [origin.0 + x * cw, origin.1 + y * ch, w * cw, h * ch]
                }
            };
            // Where it stands on a tilted stage (the window's own layers name
            // their foot in placement pixels, as a stage's are).
            let foot = match layer.stand {
                Stand::Upright(foot) => foot,
                _ => [x + w / 2.0, y + h],
            };
            let since = tilt.map(|_| part_mark(&parts));
            let lift = (layer.height + self.breath(layer.breathe, now)).max(0.0);
            if lift > 0.0
                && let Some(shadow) = self.shadow()
            {
                // The shadow keeps the ground footprint, shrinking and
                // fading as the layer rises.
                let fade = (1.0 - lift / SHADOW_FADE_PX).clamp(0.0, 1.0);
                let size = 1.0 - 0.4 * (1.0 - fade);
                let (sw, sh) = (w * size, (h * 0.35).max(4.0) * size);
                let a = SHADOW_LAYER_ALPHA * fade * layer.opacity;
                push_part(
                    &mut parts,
                    Draw::Atlas,
                    Instance {
                        rect: [x + (w - sw) / 2.0, y + h - sh * 0.6, sw, sh],
                        uv: shadow.uv,
                        color: [0.0, 0.0, 0.0, a],
                        kind: MASK,
                        pivot: [0.0; 2],
                        angle: 0.0,
                        ..Zeroable::zeroed()
                    },
                );
                emitted += 1;
            }
            // A shadow lies on the floor whatever casts it.
            if let (Some(tilt), Some(since)) = (tilt, since) {
                let floor = if matches!(layer.stand, Stand::Screen) { Stand::Screen } else { Stand::Floor };
                stamp_since(&mut parts, since, |i| stamp_tilt(&tilt, i, floor, foot));
            }
            let since = tilt.map(|_| part_mark(&parts));
            // Lifting toward the camera: bigger, and up the screen a little.
            let scale = 1.0 + lift / CAMERA_PX;
            let (lw, lh) = (w * scale, h * scale);
            let (cx, cy) = (x + w / 2.0, y + h / 2.0 - lift * RISE);
            let rect = [cx - lw / 2.0, cy - lh / 2.0, lw, lh];
            // A light adds: premultiplied colour with no coverage.
            let coverage = if layer.additive { 0.0 } else { 1.0 };
            let strength = layer.opacity * flicker(layer.flicker, layer.id, now);
            match layer.kind {
                LayerKind::Primitive { shape, color, phase, speed } => {
                    // The solid is ray-marched in the fragment shader; its
                    // `uv` carries the turn and the placement's aspect, not
                    // atlas coordinates.
                    let a = color[3] * strength;
                    push_part(
                        &mut parts,
                        Draw::Atlas,
                        Instance {
                            rect,
                            uv: [phase + now as f32 * speed, 0.0, lw / lh.max(0.01), 0.0],
                            color: [color[0], color[1], color[2], a],
                            kind: PRIMITIVE + shape,
                            pivot: [cx, cy],
                            angle: layer.angle,
                            ..Zeroable::zeroed()
                        },
                    );
                    emitted += 1;
                }
                LayerKind::Glow(rgba) => {
                    let glow = if layer.additive { self.light() } else { self.shadow() };
                    let Some(glow) = glow else {
                        continue;
                    };
                    let a = rgba[3] * strength;
                    push_part(
                        &mut parts,
                        Draw::Atlas,
                        Instance {
                            rect,
                            uv: glow.uv,
                            color: [rgba[0] * a, rgba[1] * a, rgba[2] * a, a * coverage],
                            kind: MASK,
                            pivot: [cx, cy],
                            angle: layer.angle,
                            ..Zeroable::zeroed()
                        },
                    );
                    emitted += 1;
                }
                LayerKind::Particles(ref p) => {
                    // The motes, square, where the host's clock has them.
                    let size = p.size * scale;
                    let a = p.color[3] * strength;
                    let mut instances: Vec<Instance> = motes(p, rect, now as f32)
                        .into_iter()
                        .filter(|m| m[2] > 0.0)
                        .map(|[mx, my, fade]| {
                            let k = a * fade;
                            Instance::solid(
                                mx - size / 2.0,
                                my - size / 2.0,
                                size,
                                size,
                                [p.color[0] * k, p.color[1] * k, p.color[2] * k, k * coverage],
                            )
                        })
                        .collect();
                    instances.truncate(MAX_INSTANCES - emitted);
                    emitted += instances.len();
                    extend_part(&mut parts, Draw::Atlas, instances);
                }
                LayerKind::Solid(rgba) => {
                    let a = rgba[3] * strength;
                    let mut solid = Instance::solid(
                        rect[0],
                        rect[1],
                        rect[2],
                        rect[3],
                        [rgba[0] * a, rgba[1] * a, rgba[2] * a, a * coverage],
                    );
                    solid.pivot = [cx, cy];
                    solid.angle = layer.angle;
                    push_part(&mut parts, Draw::Atlas, solid);
                    emitted += 1;
                }
                LayerKind::Picture(_) | LayerKind::PicturePart(..) => {
                    let (id, source) = match layer.kind {
                        LayerKind::PicturePart(id, source) => (id, Some(source)),
                        LayerKind::Picture(id) => (id, None),
                        _ => continue,
                    };
                    let frame = self.frame_number;
                    let Some(picture) = self.pictures.get_mut(&id) else {
                        continue;
                    };
                    picture.last_used = frame;
                    let uv = source.unwrap_or([
                        0.0,
                        0.0,
                        picture.width as f32,
                        picture.height as f32,
                    ]);
                    let linear = lw + 0.01 < uv[2] || lh + 0.01 < uv[3];
                    let kind = picture.kind();
                    push_part(
                        &mut parts,
                        Draw::Picture { id, linear },
                        Instance {
                            rect,
                            uv,
                            color: [1.0, 1.0, 1.0, layer.opacity],
                            kind,
                            pivot: [cx, cy],
                            angle: layer.angle,
                            ..Zeroable::zeroed()
                        },
                    );
                    emitted += 1;
                }
            }
            if let (Some(tilt), Some(since)) = (tilt, since) {
                stamp_since(&mut parts, since, |i| stamp_tilt(&tilt, i, layer.stand, foot));
            }
        }
        parts
    }

    /// How far a breathing layer has risen now: from nothing to `amount` and
    /// back, once a breath, the same for every layer so they breathe together.
    fn breath(&mut self, amount: f32, now: f64) -> f32 {
        if amount <= 0.0 {
            return 0.0;
        }
        let origin = *self.breath_origin.get_or_insert(now);
        let t = (now - origin) as f32;
        amount * 0.5 * (1.0 - (t / BREATH_SECONDS * std::f32::consts::TAU).cos())
    }
}

/// A flame's strength now (`angel_stage::flicker` on the host's clock).
fn flicker(amount: f32, seed: u64, now: f64) -> f32 {
    angel_stage::flicker(amount, seed, now as f32)
}

/// How an instance stands on a tilted stage's floor (`glass.wgsl`).
fn stamp_tilt(tilt: &Tilt, instance: &mut Instance, stand: Stand, foot: [f32; 2]) {
    let mode = match stand {
        Stand::Screen => return,
        Stand::Floor => 1.0,
        Stand::Upright(_) => 2.0,
    };
    instance.tilt = [tilt.centre[0], tilt.centre[1], tilt.near, tilt.zoom];
    instance.stand = [mode, foot[0], foot[1], tilt.squash];
}

/// Where the next instances pushed into `parts` will begin.
fn part_mark(parts: &[(Draw, Vec<Instance>)]) -> (usize, usize) {
    parts.last().map_or((0, 0), |(_, list)| (parts.len() - 1, list.len()))
}

/// Apply `f` to every instance pushed since `mark`.
fn stamp_since(parts: &mut [(Draw, Vec<Instance>)], (part, start): (usize, usize), f: impl Fn(&mut Instance)) {
    for (n, (_, list)) in parts.iter_mut().enumerate().skip(part) {
        let from = if n == part { start } else { 0 };
        for instance in list.iter_mut().skip(from) {
            f(instance);
        }
    }
}