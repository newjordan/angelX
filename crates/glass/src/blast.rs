//! The blast: a full multimedia stress scene for the compositor, every
//! piece a pure function of the clock so it can be tested without a
//! window (`ANGEL_GLASS_DEMO=blast` puts it over the cockpit).
//!
//! - A **video** in a frame: an animated test card generated per frame and
//!   raycast onto a plane spinning and warping in 3D, then streamed to the
//!   GPU as a same-size picture upload every frame — the video path.
//! - **Raycast primitives**: spheres over a checker ground and a rotating
//!   cube, bobbing up and down, each its own picture layer lifted toward
//!   the camera with its window-drawn shadow.
//! - **Line testing**: sine-wave ribbons as a picture, and whole formations
//!   of thin solid layers (waves, starbursts, weaves, spirals) alternating
//!   below and above the text on the z axis, re-randomized every few
//!   seconds from a seeded generator.
//!
//! Everything drifts on Lissajous paths like a screensaver. Nothing here
//! is art direction; it is the M2-shaped load — many layers, several
//! per-frame picture streams, rotation, lift and alpha at once — made
//! visible and measurable.

use crate::layer::{Layer, LayerKind, Placement};

pub const VIDEO_W: u32 = 320;
pub const VIDEO_H: u32 = 200;
pub const ORBS_W: u32 = 192;
pub const ORBS_H: u32 = 160;
pub const CUBE_W: u32 = 160;
pub const CUBE_H: u32 = 160;
pub const RIBBONS_W: u32 = 480;
pub const RIBBONS_H: u32 = 140;

/// The most solid segments one formation may emit: a hard cap so a
/// formation can never flood the instance buffer.
pub const MAX_SEGMENTS: usize = 260;

/// How long one randomized formation holds before the next seed.
pub const FORMATION_SECONDS: f32 = 4.0;

type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> V3 {
    let len = dot(a, a).sqrt().max(1e-9);
    mul(a, 1.0 / len)
}
fn rot_y(v: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [c * v[0] + s * v[2], v[1], -s * v[0] + c * v[2]]
}
fn rot_x(v: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [v[0], c * v[1] - s * v[2], s * v[1] + c * v[2]]
}

/// A ray through pixel `(x, y)` of a `w` × `h` picture, camera at the
/// origin looking down +z, `fov` the half-height at z = 1.
fn ray(x: u32, y: u32, w: u32, h: u32, fov: f32) -> V3 {
    let aspect = w as f32 / h as f32;
    let nx = ((x as f32 + 0.5) / w as f32 * 2.0 - 1.0) * aspect * fov;
    let ny = (1.0 - (y as f32 + 0.5) / h as f32 * 2.0) * fov;
    norm([nx, ny, 1.0])
}

fn put(rgba: &mut [u8], w: u32, x: u32, y: u32, px: [u8; 4]) {
    let i = ((y * w + x) * 4) as usize;
    rgba[i..i + 4].copy_from_slice(&px);
}

fn shade(base: [f32; 3], light: f32) -> [u8; 4] {
    let c = |v: f32| (v * light * 255.0).clamp(0.0, 255.0) as u8;
    [c(base[0]), c(base[1]), c(base[2]), 255]
}

/// The animated test card the warped plane shows: sky, a travelling sun,
/// ridge silhouettes, a scrolling checker road, a bouncing ball, a sweep
/// line and a frame counter in blocks along the bottom. 256 × 144.
pub fn video_source(t: f32) -> Vec<u8> {
    const W: u32 = 256;
    const H: u32 = 144;
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    let horizon = 92.0;
    let sun_x = 128.0 + 95.0 * (t * 0.13).sin();
    let sun_y = 44.0 - 20.0 * (t * 0.13).cos();
    let ball_x = 20.0 + (t * 47.0) % 216.0;
    let ball_y = 108.0 - (t * 2.2).sin().abs() * 46.0;
    let sweep = (t * 36.0) % H as f32;
    let frame = (t * 60.0) as u32;
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            let mut px;
            if fy < horizon {
                let k = fy / horizon;
                let shift = 0.5 + 0.5 * (t * 0.21).sin();
                px = [
                    (18.0 + 40.0 * k + 30.0 * shift * (1.0 - k)) as u8,
                    (42.0 + 90.0 * k) as u8,
                    (110.0 + 80.0 * (1.0 - k)) as u8,
                    255,
                ];
                let ridge = horizon - 14.0 - 8.0 * (fx * 0.045 + 1.7).sin() - 5.0 * (fx * 0.11).sin();
                if fy > ridge {
                    px = [24, 52, 62, 255];
                }
                let d = ((fx - sun_x).powi(2) + (fy - sun_y).powi(2)).sqrt();
                if d < 13.0 {
                    px = [255, 214, 110, 255];
                } else if d < 19.0 {
                    px = [255, 170, 80, 255];
                }
            } else {
                let depth = 26.0 / (fy - horizon + 2.0);
                let cell = (((fx - 128.0) * depth * 0.14) as i32).rem_euclid(2)
                    ^ ((depth * 3.0 + t * 2.4) as i32).rem_euclid(2);
                px = if cell == 0 {
                    [36, 40, 58, 255]
                } else {
                    [96, 104, 128, 255]
                };
            }
            let d = ((fx - ball_x).powi(2) + (fy - ball_y).powi(2)).sqrt();
            if d < 9.0 {
                px = [255, 96, 120, 255];
            }
            if (fy - sweep).abs() < 1.5 {
                px = [px[0].saturating_add(70), px[1].saturating_add(70), px[2].saturating_add(70), 255];
            }
            if y >= H - 10 {
                let bit = x / 12;
                px = if bit < 21 && (frame >> bit) & 1 == 1 {
                    [120, 255, 170, 255]
                } else {
                    [14, 20, 26, 255]
                };
            }
            put(&mut rgba, W, x, y, px);
        }
    }
    rgba
}

/// The test card on a framed plane spinning and warping in 3D, raycast
/// per pixel (exact perspective, the back of the frame shown when it
/// turns away). Transparent outside the plane. [`VIDEO_W`] × [`VIDEO_H`].
pub fn warped_video(t: f32) -> Vec<u8> {
    let source = video_source(t);
    let (sw, sh) = (256u32, 144u32);
    let mut rgba = vec![0u8; (VIDEO_W * VIDEO_H * 4) as usize];
    let yaw = t * 0.7;
    let pitch = 0.35 * (t * 0.9).sin();
    let center: V3 = [0.0, 0.05 * (t * 1.3).sin(), 3.0];
    let orient = |v: V3| rot_y(rot_x(v, pitch), yaw);
    let u_axis = orient([1.02, 0.0, 0.0]);
    let v_axis = orient([0.0, 0.58, 0.0]);
    let normal = cross(u_axis, v_axis);
    for y in 0..VIDEO_H {
        for x in 0..VIDEO_W {
            let d = ray(x, y, VIDEO_W, VIDEO_H, 0.62);
            let denom = dot(d, normal);
            if denom.abs() < 1e-6 {
                continue;
            }
            let s = dot(center, normal) / denom;
            if s <= 0.0 {
                continue;
            }
            let hit = sub(mul(d, s), center);
            let u = dot(hit, u_axis) / dot(u_axis, u_axis);
            let v = dot(hit, v_axis) / dot(v_axis, v_axis);
            if !(-1.0..=1.0).contains(&u) || !(-1.0..=1.0).contains(&v) {
                continue;
            }
            let edge = u.abs().max(v.abs());
            let px = if edge > 0.9 {
                // The frame: brass, with darker corners.
                if u.abs() > 0.96 && v.abs() > 0.94 {
                    [92, 64, 28, 255]
                } else {
                    [196, 150, 72, 255]
                }
            } else if denom < 0.0 {
                // The back of the frame: slate with a diagonal brace.
                if (u - v).abs() < 0.06 || (u + v).abs() < 0.06 {
                    [70, 78, 96, 255]
                } else {
                    [42, 48, 62, 255]
                }
            } else {
                let sx = ((u * 0.5 + 0.5) * sw as f32) as u32;
                let sy = ((0.5 - v * 0.5) * sh as f32) as u32;
                let i = ((sy.min(sh - 1) * sw + sx.min(sw - 1)) * 4) as usize;
                [source[i], source[i + 1], source[i + 2], 255]
            };
            put(&mut rgba, VIDEO_W, x, y, px);
        }
    }
    rgba
}

const LIGHT: V3 = [-0.42, 0.72, -0.55];

fn lambert(normal: V3, view: V3) -> f32 {
    let diffuse = dot(normal, norm(LIGHT)).max(0.0);
    let reflected = sub(mul(normal, 2.0 * dot(normal, norm(LIGHT))), norm(LIGHT));
    let spec = dot(reflected, mul(view, -1.0)).max(0.0).powi(24);
    0.22 + 0.75 * diffuse + 0.7 * spec
}

/// Three raycast spheres bobbing over a checker ground, blob shadows
/// shrinking as each rises. Transparent above the horizon.
pub fn raycast_orbs(t: f32) -> Vec<u8> {
    let mut rgba = vec![0u8; (ORBS_W * ORBS_H * 4) as usize];
    let ground_y = -0.85f32;
    let spheres: [(V3, f32, [f32; 3]); 3] = [
        ([-0.78, 0.28 * (t * 1.7).sin(), 3.0], 0.44, [1.0, 0.45, 0.4]),
        ([0.0, 0.3 * (t * 1.3 + 2.0).sin(), 3.2], 0.58, [0.3, 0.9, 0.85]),
        ([0.8, 0.26 * (t * 2.1 + 4.0).sin(), 2.9], 0.4, [1.0, 0.78, 0.3]),
    ];
    for y in 0..ORBS_H {
        for x in 0..ORBS_W {
            let d = ray(x, y, ORBS_W, ORBS_H, 0.6);
            let mut best: Option<(f32, [u8; 4])> = None;
            for (c, r, base) in spheres {
                let oc = mul(c, -1.0);
                let b = dot(oc, d);
                let disc = b * b - (dot(oc, oc) - r * r);
                if disc < 0.0 {
                    continue;
                }
                let s = -b - disc.sqrt();
                if s <= 0.0 || best.is_some_and(|(bs, _)| s >= bs) {
                    continue;
                }
                let p = mul(d, s);
                let n = norm(sub(p, c));
                best = Some((s, shade(base, lambert(n, d))));
            }
            if d[1] < -1e-6 {
                let s = ground_y / d[1];
                if s > 0.0 && best.is_none_or(|(bs, _)| s < bs) {
                    let p = mul(d, s);
                    let cell = (p[0] * 1.6).floor() as i32 ^ (p[2] * 1.6).floor() as i32;
                    let mut light = if cell.rem_euclid(2) == 0 { 0.5 } else { 0.28 };
                    for (c, r, _) in spheres {
                        let flat = ((p[0] - c[0]).powi(2) + (p[2] - c[2]).powi(2)).sqrt();
                        let reach = r * (1.5 - 0.5 * ((c[1] - ground_y) / 1.4).clamp(0.0, 1.0));
                        if flat < reach {
                            light *= 1.0 - 0.55 * (1.0 - flat / reach);
                        }
                    }
                    let fog = (1.0 - (s - 3.0) / 14.0).clamp(0.15, 1.0);
                    best = Some((s, shade([0.75, 0.8, 0.95], light * fog)));
                }
            }
            if let Some((_, px)) = best {
                put(&mut rgba, ORBS_W, x, y, px);
            }
        }
    }
    rgba
}

/// A raycast cube turning on two axes and bobbing, faces coloured by
/// axis, edges picked out bright. Transparent around it.
pub fn raycast_cube(t: f32) -> Vec<u8> {
    let mut rgba = vec![0u8; (CUBE_W * CUBE_H * 4) as usize];
    let center: V3 = [0.0, 0.22 * (t * 1.6).sin(), 3.0];
    let half = 0.62f32;
    let (yaw, pitch) = (t * 0.9, 0.5 + 0.35 * (t * 0.7).sin());
    let to_local = |v: V3| rot_x(rot_y(v, -yaw), -pitch);
    let to_world = |v: V3| rot_y(rot_x(v, pitch), yaw);
    let faces: [[f32; 3]; 6] = [
        [1.0, 0.5, 0.35],
        [0.45, 0.85, 1.0],
        [0.55, 1.0, 0.6],
        [1.0, 0.85, 0.4],
        [0.8, 0.55, 1.0],
        [1.0, 0.65, 0.85],
    ];
    for y in 0..CUBE_H {
        for x in 0..CUBE_W {
            let d = ray(x, y, CUBE_W, CUBE_H, 0.58);
            let o = to_local(mul(center, -1.0));
            let ld = to_local(d);
            let mut near = f32::NEG_INFINITY;
            let mut far = f32::INFINITY;
            let mut axis = 0usize;
            let mut sign = 1.0f32;
            let mut missed = false;
            for a in 0..3 {
                if ld[a].abs() < 1e-9 {
                    if o[a].abs() > half {
                        missed = true;
                    }
                    continue;
                }
                let (mut t0, mut t1) = ((-half - o[a]) / ld[a], (half - o[a]) / ld[a]);
                let mut s = -1.0;
                if t0 > t1 {
                    std::mem::swap(&mut t0, &mut t1);
                    s = 1.0;
                }
                if t0 > near {
                    near = t0;
                    axis = a;
                    sign = s;
                }
                far = far.min(t1);
            }
            if missed || near > far || far < 0.0 {
                continue;
            }
            let p_local = add(o, mul(ld, near));
            let mut n_local = [0.0; 3];
            n_local[axis] = sign;
            let n = norm(to_world(n_local));
            let mut light = lambert(n, d);
            let edge = (0..3).filter(|&a| a != axis && p_local[a].abs() > half * 0.86).count();
            if edge > 0 {
                light += 0.35;
            }
            let face = faces[axis * 2 + usize::from(sign > 0.0)];
            put(&mut rgba, CUBE_W, x, y, shade(face, light));
        }
    }
    rgba
}

/// Sine ribbons as one transparent picture: five curves running in
/// alternating directions, stamped with a soft brush.
pub fn line_ribbons(t: f32) -> Vec<u8> {
    let mut rgba = vec![0u8; (RIBBONS_W * RIBBONS_H * 4) as usize];
    let colours: [[u8; 3]; 5] = [
        [255, 120, 140],
        [120, 230, 255],
        [170, 255, 150],
        [255, 205, 110],
        [205, 160, 255],
    ];
    for (i, colour) in colours.iter().enumerate() {
        let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
        let base = RIBBONS_H as f32 * (0.18 + 0.16 * i as f32);
        let amp = 16.0 + 10.0 * (t * 0.6 + i as f32).sin();
        let freq = 0.016 + 0.004 * i as f32;
        for x in 0..RIBBONS_W {
            let phase = x as f32 * freq + dir * t * (1.4 + 0.3 * i as f32);
            let y = base + amp * phase.sin() + 5.0 * (phase * 2.7).sin();
            for dy in -1i32..=1 {
                for dx in 0..2u32 {
                    let py = y as i32 + dy;
                    if py < 0 || py >= RIBBONS_H as i32 {
                        continue;
                    }
                    let a = if dy == 0 { 235 } else { 110 };
                    put(&mut rgba, RIBBONS_W, (x + dx).min(RIBBONS_W - 1), py as u32, [colour[0], colour[1], colour[2], a]);
                }
            }
        }
    }
    rgba
}

/// A tiny seeded generator (xorshift64): formations are randomized but
/// reproducible, so a bug seen on screen can be replayed from its seed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// In `0.0..1.0`.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    pub fn pick(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Which formation the clock is in; changes every [`FORMATION_SECONDS`].
pub fn formation_seed(t: f32) -> u64 {
    ((t.max(0.0) / FORMATION_SECONDS) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xB1A5_7E57
}

const PALETTE: [[f32; 4]; 6] = [
    [1.0, 0.42, 0.5, 1.0],
    [0.4, 0.9, 1.0, 1.0],
    [0.6, 1.0, 0.55, 1.0],
    [1.0, 0.8, 0.35, 1.0],
    [0.78, 0.58, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
];

fn segment(
    layers: &mut Vec<Layer>,
    from: (f32, f32),
    to: (f32, f32),
    thickness: f32,
    colour: [f32; 4],
    z: i32,
    opacity: f32,
) {
    if layers.len() >= MAX_SEGMENTS {
        return;
    }
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.5 {
        return;
    }
    let mut layer = Layer::new(
        layers.len() as u64,
        z,
        Placement::Pixels([
            (from.0 + to.0) / 2.0 - len / 2.0,
            (from.1 + to.1) / 2.0 - thickness / 2.0,
            len,
            thickness,
        ]),
        LayerKind::Solid(colour),
    );
    layer.angle = dy.atan2(dx);
    layer.opacity = opacity;
    layers.push(layer);
}

/// One randomized line formation as solid segment layers over a
/// `window`-sized area in pixels, alternating below the text (z < 0) and
/// above it (z > 0). Kinds: parallel sine waves, a rotating starburst, a
/// sine weave, and a spiral. Every formation also carries two slow waves
/// behind the text so the z split is always on screen.
pub fn line_formation(t: f32, window: (f32, f32)) -> Vec<Layer> {
    let (w, h) = window;
    let mut layers = Vec::new();
    if w < 50.0 || h < 50.0 {
        return layers;
    }
    let mut rng = Rng::new(formation_seed(t));
    let kind = rng.pick(4);
    let thick = rng.range(2.0, 4.5);
    // Two slow waves behind the text, always.
    for (wave, &color) in PALETTE.iter().enumerate().take(2) {
        let base = h * (0.3 + 0.4 * wave as f32);
        let amp = h * 0.06;
        let steps = 36;
        let mut prev = (0.0, base);
        for i in 1..=steps {
            let x = w * i as f32 / steps as f32;
            let dir = if wave == 0 { 1.0 } else { -1.0 };
            let y = base + amp * (x / w * 9.0 + dir * t * 1.2).sin();
            segment(&mut layers, prev, (x, y), thick, color, -3, 0.55);
            prev = (x, y);
        }
    }
    match kind {
        0 => {
            // Parallel sine waves, alternating z and direction.
            let waves = 3 + rng.pick(3);
            for wave in 0..waves {
                let base = h * rng.range(0.12, 0.9);
                let amp = h * rng.range(0.03, 0.13);
                let freq = rng.range(5.0, 14.0);
                let speed = rng.range(0.8, 2.6) * if wave % 2 == 0 { 1.0 } else { -1.0 };
                let z = if wave % 2 == 0 { 4 } else { -2 };
                let colour = PALETTE[rng.pick(PALETTE.len())];
                let steps = 40;
                let mut prev = (0.0, base + amp * (speed * t).sin());
                for i in 1..=steps {
                    let x = w * i as f32 / steps as f32;
                    let y = base + amp * (x / w * freq + speed * t).sin();
                    segment(&mut layers, prev, (x, y), thick, colour, z, 0.9);
                    prev = (x, y);
                }
            }
        }
        1 => {
            // A rotating starburst about a drifting centre.
            let centre = drift(t, 2.4, window, (0.0, 0.0));
            let spokes = 10 + rng.pick(10);
            let reach = w.min(h) * rng.range(0.3, 0.55);
            for spoke in 0..spokes {
                let a = t * 0.45 + spoke as f32 / spokes as f32 * std::f32::consts::TAU;
                let wobble = 1.0 + 0.25 * (t * 2.0 + spoke as f32 * 1.7).sin();
                let end = (centre.0 + reach * wobble * a.cos(), centre.1 + reach * wobble * a.sin());
                let mid = (
                    centre.0 + reach * 0.55 * (a + 0.22 * (t * 1.3).sin()).cos(),
                    centre.1 + reach * 0.55 * (a + 0.22 * (t * 1.3).sin()).sin(),
                );
                let z = if spoke % 2 == 0 { 5 } else { -2 };
                let colour = PALETTE[spoke % PALETTE.len()];
                segment(&mut layers, centre, mid, thick, colour, z, 0.9);
                segment(&mut layers, mid, end, thick * 0.7, colour, z, 0.75);
            }
        }
        2 => {
            // A weave: horizontal and vertical sine threads crossing.
            let threads = 4 + rng.pick(3);
            for i in 0..threads {
                let frac = i as f32 / threads.max(1) as f32;
                let z = if i % 2 == 0 { 3 } else { -2 };
                let colour = PALETTE[rng.pick(PALETTE.len())];
                let steps = 28;
                let mut prev_h = (0.0, h * frac);
                let mut prev_v = (w * frac, 0.0);
                for s in 1..=steps {
                    let f = s as f32 / steps as f32;
                    let ph = (f * w, h * frac + h * 0.045 * (f * 12.0 + t * 1.6 + i as f32).sin());
                    segment(&mut layers, prev_h, ph, thick * 0.8, colour, z, 0.8);
                    prev_h = ph;
                    let pv = (w * frac + w * 0.03 * (f * 12.0 - t * 1.4 + i as f32).sin(), f * h);
                    segment(&mut layers, prev_v, pv, thick * 0.8, colour, -z, 0.8);
                    prev_v = pv;
                }
            }
        }
        _ => {
            // A spiral turning about a drifting centre.
            let centre = drift(t, 4.1, window, (0.0, 0.0));
            let arms = 2 + rng.pick(3);
            let max_r = w.min(h) * rng.range(0.28, 0.5);
            for arm in 0..arms {
                let z = if arm % 2 == 0 { 5 } else { -2 };
                let colour = PALETTE[(arm * 2) % PALETTE.len()];
                let steps = 44;
                let mut prev = centre;
                for s in 1..=steps {
                    let f = s as f32 / steps as f32;
                    let a = t * 0.8 + arm as f32 * std::f32::consts::TAU / arms as f32 + f * 7.0;
                    let p = (centre.0 + max_r * f * a.cos(), centre.1 + max_r * f * a.sin());
                    segment(&mut layers, prev, p, thick * (1.2 - 0.6 * f), colour, z, 0.9);
                    prev = p;
                }
            }
        }
    }
    layers
}

/// Picture ids the blast streams into (from [`crate::layer::LAYER_PICTURES`]).
pub const VIDEO_ID: u32 = crate::layer::LAYER_PICTURES + 16;
pub const ORBS_ID: u32 = crate::layer::LAYER_PICTURES + 17;
pub const CUBE_ID: u32 = crate::layer::LAYER_PICTURES + 18;
pub const RIBBONS_ID: u32 = crate::layer::LAYER_PICTURES + 19;

/// The whole blast as layers over a `window`-sized area: the framed video
/// with a glow behind it, the raycast orbs and cube lifted and bobbing,
/// the ribbon picture, two translucent turning panels, and the line
/// formation. The caller streams the four pictures each frame and hands
/// these over as one layer group. The line formation repeats `copies`
/// times (different
/// seeds and phases), for scale testing: how many segment layers can the
/// compositor carry before the frame budget goes? Ids stay unique across
/// the whole scene.
pub fn scene_layers_x(t: f32, window: (f32, f32), copies: usize) -> Vec<Layer> {
    let (w, h) = window;
    let mut layers = Vec::new();
    for copy in 0..copies.max(1) {
        let offset = layers.len() as u64;
        layers.extend(
            line_formation(t + copy as f32 * 7.31, window)
                .into_iter()
                .map(|mut layer| {
                    layer.id += offset;
                    layer
                }),
        );
    }
    let mut next = layers.len() as u64;
    let mut push = |layers: &mut Vec<Layer>, mut layer: Layer| {
        layer.id = next;
        next += 1;
        layers.push(layer);
    };
    // The framed, warping video, drifting slowly, a glow behind it.
    let vw = (w * 0.44).min(600.0);
    let vh = vw * VIDEO_H as f32 / VIDEO_W as f32;
    let (vx, vy) = drift(t, 0.0, window, (vw, vh));
    push(&mut layers, Layer::new(0, 1, Placement::Pixels([vx - vw * 0.18, vy - vh * 0.28, vw * 1.36, vh * 1.56]), LayerKind::Glow([0.45, 0.6, 1.0, 0.4])));
    let mut video = Layer::new(0, 6, Placement::Pixels([vx, vy, vw, vh]), LayerKind::Picture(VIDEO_ID));
    video.angle = 0.07 * (t * 0.5).sin();
    video.height = 26.0 + 18.0 * (t * 0.9).sin().max(0.0);
    push(&mut layers, video);
    // Raycast orbs, lifted well off the ground, bobbing.
    let ow = (w * 0.3).min(360.0);
    let oh = ow * ORBS_H as f32 / ORBS_W as f32;
    let (ox, oy) = drift(t, 2.9, window, (ow, oh));
    let mut orbs = Layer::new(0, 7, Placement::Pixels([ox, oy, ow, oh]), LayerKind::Picture(ORBS_ID));
    orbs.height = 70.0 + 55.0 * (0.5 + 0.5 * (t * 1.3).sin());
    push(&mut layers, orbs);
    // The raycast cube, smaller, higher, counter-drifting.
    let cs = (w * 0.18).min(230.0);
    let (cx, cy) = drift(t, 5.2, window, (cs, cs));
    let mut cube = Layer::new(0, 8, Placement::Pixels([cx, cy, cs, cs]), LayerKind::Picture(CUBE_ID));
    cube.height = 110.0 + 70.0 * (0.5 + 0.5 * (t * 1.7 + 1.0).sin());
    cube.breathe = 5.0;
    push(&mut layers, cube);
    // Sine ribbons as a picture, sliding across near the bottom.
    let rw = (w * 0.52).min(680.0);
    let rh = rw * RIBBONS_H as f32 / RIBBONS_W as f32;
    let rx = ((t * 22.0) % (w + rw)) - rw;
    let ry = h * 0.72 + h * 0.05 * (t * 0.4).sin();
    push(&mut layers, Layer::new(0, 2, Placement::Pixels([rx, ry, rw, rh]), LayerKind::Picture(RIBBONS_ID)));
    // Two translucent panels turning slowly, one either side of the text.
    for i in 0..2 {
        let pw = w * 0.16;
        let ph = h * 0.2;
        let (px, py) = drift(t, 1.3 + 3.1 * i as f32, window, (pw, ph));
        let mut panel = Layer::new(
            0,
            if i == 0 { -1 } else { 3 },
            Placement::Pixels([px, py, pw, ph]),
            LayerKind::Solid(if i == 0 { [0.2, 0.7, 1.0, 0.16] } else { [1.0, 0.55, 0.3, 0.14] }),
        );
        panel.angle = t * 0.24 * if i == 0 { 1.0 } else { -1.0 };
        push(&mut layers, panel);
    }
    layers
}

/// A Lissajous drift inside `window` for something of `size`, centred:
/// the screensaver's path.
pub fn drift(t: f32, phase: f32, window: (f32, f32), size: (f32, f32)) -> (f32, f32) {
    let (w, h) = window;
    let span_x = ((w - size.0) / 2.0).max(0.0);
    let span_y = ((h - size.1) / 2.0).max(0.0);
    (
        w / 2.0 + span_x * 0.82 * (t * 0.21 + phase).sin() - size.0 / 2.0,
        h / 2.0 + span_y * 0.82 * (t * 0.157 + phase * 1.7).sin() - size.1 / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(rgba: &[u8], w: u32, x: u32, y: u32) -> u8 {
        rgba[((y * w + x) * 4 + 3) as usize]
    }
    fn opaque(rgba: &[u8]) -> usize {
        rgba.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    fn the_video_is_deterministic_and_moves() {
        assert_eq!(video_source(1.25), video_source(1.25));
        assert_ne!(video_source(0.0), video_source(0.5));
        assert_eq!(video_source(0.0).len(), 256 * 144 * 4);
    }

    #[test]
    fn the_warped_plane_fills_the_middle_not_the_corners() {
        let frame = warped_video(0.0);
        assert_eq!(alpha(&frame, VIDEO_W, VIDEO_W / 2, VIDEO_H / 2), 255);
        assert_eq!(alpha(&frame, VIDEO_W, 0, 0), 0);
        assert_eq!(alpha(&frame, VIDEO_W, VIDEO_W - 1, VIDEO_H - 1), 0);
        // Turned past edge-on later, the back still shows as a sliver.
        assert!(opaque(&warped_video(2.6)) > 400);
    }

    #[test]
    fn raycast_primitives_hit_in_the_middle_and_miss_the_sky() {
        let orbs = raycast_orbs(0.0);
        assert_eq!(alpha(&orbs, ORBS_W, ORBS_W / 2, ORBS_H / 2), 255, "centre sphere");
        assert_eq!(alpha(&orbs, ORBS_W, 2, 2), 0, "sky stays transparent");
        assert!(opaque(&orbs) > ORBS_W as usize * 20, "the ground is drawn");
        let cube = raycast_cube(0.0);
        assert_eq!(alpha(&cube, CUBE_W, CUBE_W / 2, CUBE_H / 2), 255);
        assert_eq!(alpha(&cube, CUBE_W, 0, 0), 0);
        assert_ne!(raycast_cube(0.0), raycast_cube(0.4), "the cube turns");
    }

    #[test]
    fn ribbons_draw_lines_on_transparency() {
        let ribbons = line_ribbons(1.0);
        let drawn = opaque(&ribbons);
        assert!(drawn > 2_000, "{drawn} drawn");
        assert!(drawn < (RIBBONS_W * RIBBONS_H) as usize / 3, "mostly clear");
    }

    #[test]
    fn formations_alternate_z_and_stay_capped_and_finite() {
        for t in [0.0, 1.7, 4.2, 8.9, 13.3, 21.0] {
            let layers = line_formation(t, (1280.0, 800.0));
            assert!(layers.len() > 40, "t={t}: {} segments", layers.len());
            assert!(layers.len() <= MAX_SEGMENTS, "t={t} over cap");
            assert!(layers.iter().any(|l| l.z < 0), "t={t}: nothing below text");
            assert!(layers.iter().any(|l| l.z > 0), "t={t}: nothing above text");
            assert!(
                layers.iter().all(|l| l.clone().sanitized().is_some()),
                "t={t}: a segment the renderer would drop"
            );
        }
        assert!(line_formation(0.0, (10.0, 10.0)).is_empty(), "no room, no layers");
    }

    #[test]
    fn formations_change_with_the_seed_and_repeat_with_it() {
        let a = line_formation(0.5, (1280.0, 800.0));
        let b = line_formation(0.5, (1280.0, 800.0));
        assert_eq!(a, b, "same clock, same formation");
        let later = line_formation(0.5 + FORMATION_SECONDS, (1280.0, 800.0));
        assert_ne!(a, later, "the next seed differs");
    }

    #[test]
    fn the_scene_has_every_medium_at_once() {
        let layers = scene_layers_x(3.0, (1280.0, 800.0), 1);
        let pictures: Vec<u32> = layers
            .iter()
            .filter_map(|l| match l.kind {
                LayerKind::Picture(id) => Some(id),
                _ => None,
            })
            .collect();
        for id in [VIDEO_ID, ORBS_ID, CUBE_ID, RIBBONS_ID] {
            assert!(pictures.contains(&id), "picture {id} placed");
        }
        assert!(layers.iter().any(|l| matches!(l.kind, LayerKind::Glow(_))));
        assert!(layers.iter().any(|l| matches!(l.kind, LayerKind::Solid(_)) && l.z < 0));
        assert!(layers.iter().any(|l| l.height > 50.0), "something flies");
        assert!(layers.iter().all(|l| l.clone().sanitized().is_some()));
    }

    #[test]
    fn scaled_scenes_repeat_the_formation_with_unique_ids() {
        let one = scene_layers_x(3.0, (1280.0, 800.0), 1);
        let four = scene_layers_x(3.0, (1280.0, 800.0), 4);
        assert!(four.len() > one.len() * 2, "{} vs {}", four.len(), one.len());
        let mut ids: Vec<u64> = four.iter().map(|l| l.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), four.len(), "ids unique across copies");
    }

    /// `BLAST_DUMP=/tmp/blast cargo test -p angel-glass dump_blast_frames`
    /// writes PPM stills of every medium for a human to open.
    #[test]
    fn dump_blast_frames() {
        let Ok(dir) = std::env::var("BLAST_DUMP") else {
            return;
        };
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, rgba: &[u8], w: u32, h: u32| {
            let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
            for px in rgba.chunks_exact(4) {
                // Composite over a dark slate so alpha reads.
                let a = px[3] as u32;
                for &c in &px[..3] {
                    out.push(((c as u32 * a + 24 * (255 - a)) / 255) as u8);
                }
            }
            std::fs::write(format!("{dir}/{name}"), out).unwrap();
        };
        for (i, t) in [0.0f32, 1.3, 2.6].iter().enumerate() {
            write(&format!("video{i}.ppm"), &warped_video(*t), VIDEO_W, VIDEO_H);
            write(&format!("orbs{i}.ppm"), &raycast_orbs(*t), ORBS_W, ORBS_H);
            write(&format!("cube{i}.ppm"), &raycast_cube(*t), CUBE_W, CUBE_H);
        }
        write("ribbons.ppm", &line_ribbons(1.0), RIBBONS_W, RIBBONS_H);
    }
}
