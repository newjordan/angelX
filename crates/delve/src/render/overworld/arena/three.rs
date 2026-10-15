//! three.rs — the arena's own little 3D.
//!
//! A fixed-camera pinhole painter for the festival's dimensional
//! attractions: the duck booth, the half-pipe, the mine trolley. World
//! coordinates are x right, y into the stage, z up, one unit one tile; the
//! camera is authored per attraction and never chases. Surfaces paint as
//! lambert-lit ramps in the realm's own inks, softened to many steps and
//! falling off into black with distance (`tones`), and billboards stamp as
//! depth-scaled sprites, so a 3D stage sits in the same night as the rest
//! of the arena. Pure and deterministic: identical inputs, identical pixels.

use super::super::ink::{bayer, ink, nearest_plain, vnoise, Img, Rgb, BLACK};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Steps a facet's shade is resolved to, and how many depths it fades by.
const STEPS: usize = 24;
const FADES: usize = 8;

/// A ramp's soft tones, baked once: for each of `FADES` depths and `STEPS`
/// shades, its two neighbouring inks mixed and dimmed toward black by
/// distance, then put back on the realm's palette (or on the ramp's own
/// inks, for a ramp of signal inks that mark game state). A surface steps
/// gently down its own material into the night instead of flickering
/// between three inks in a checker.
fn tones(ramp: &'static [char]) -> &'static [Rgb] {
    static TABLES: OnceLock<Mutex<HashMap<(usize, usize), &'static [Rgb]>>> = OnceLock::new();
    let key = (ramp.as_ptr() as usize, ramp.len());
    let tables = TABLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(t) = tables.lock().unwrap().get(&key) {
        return t;
    }
    let inks: Vec<Rgb> = ramp.iter().map(|&c| ink(c).unwrap_or(BLACK)).collect();
    let own: Vec<Rgb> = inks.clone();
    let mut out = Vec::with_capacity(STEPS * FADES);
    for fade in 0..FADES {
        let keep = 1.0 - 0.72 * fade as f32 / (FADES - 1) as f32;
        for step in 0..STEPS {
            let t = step as f32 / (STEPS - 1) as f32 * (inks.len() - 1).max(1) as f32;
            let (i, f) = ((t.floor() as usize).min(inks.len() - 1), t.fract());
            let (a, b) = (inks[i], inks[(i + 1).min(inks.len() - 1)]);
            let c: [f32; 3] = std::array::from_fn(|k| {
                (f32::from(a[k]) + (f32::from(b[k]) - f32::from(a[k])) * f) * keep
            });
            let d = |p: Rgb| (0..3).map(|k| (f32::from(p[k]) - c[k]).powi(2)).sum::<f32>();
            let plain = nearest_plain(c);
            let best = own.iter().copied().fold(plain, |best, p| if d(p) < d(best) { p } else { best });
            out.push(best);
        }
    }
    let table: &'static [Rgb] = Box::leak(out.into_boxed_slice());
    tables.lock().unwrap().insert(key, table);
    table
}

/// The realm's stage light, matching the sprite kit's top-left sun.
const LIGHT_DIR: [f32; 3] = [-0.5, -0.62, 0.6];

/// A pinhole camera: authored once per attraction.
pub(super) struct Cam {
    pub(super) eye: [f32; 3],
    pub(super) fwd: [f32; 3],
    pub(super) right: [f32; 3],
    pub(super) up: [f32; 3],
    /// Horizontal focal length in pixels.
    pub(super) focal: f32,
    /// Screen centre the focal length is stated against.
    pub(super) cx: f32,
    pub(super) cy: f32,
}

impl Cam {
    /// A camera at `eye`, turned `yaw_deg` from +y toward +x and pitched
    /// `pitch_deg` up, with a horizontal field of `fov_deg` across `w`.
    pub(super) fn look(
        eye: [f32; 3],
        yaw_deg: f32,
        pitch_deg: f32,
        fov_deg: f32,
        w: i32,
        h: i32,
    ) -> Cam {
        let (yaw, pitch) = (yaw_deg.to_radians(), pitch_deg.to_radians());
        let fwd = [
            pitch.cos() * yaw.sin(),
            pitch.cos() * yaw.cos(),
            pitch.sin(),
        ];
        let right = [yaw.cos(), -yaw.sin(), 0.0];
        let up = [
            -pitch.sin() * yaw.sin(),
            -pitch.sin() * yaw.cos(),
            pitch.cos(),
        ];
        Cam {
            eye,
            fwd,
            right,
            up,
            focal: (w as f32) * 0.5 / (fov_deg * 0.5).to_radians().tan(),
            cx: w as f32 * 0.5,
            cy: h as f32 * 0.5,
        }
    }

    /// A world point on screen: `(x, y, depth)`; `None` behind the eye.
    pub(super) fn project(&self, p: [f32; 3]) -> Option<(i32, i32, f32)> {
        let d = [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]];
        let z = d[0] * self.fwd[0] + d[1] * self.fwd[1] + d[2] * self.fwd[2];
        if z <= 0.05 {
            return None;
        }
        let xr = d[0] * self.right[0] + d[1] * self.right[1] + d[2] * self.right[2];
        let yu = d[0] * self.up[0] + d[1] * self.up[1] + d[2] * self.up[2];
        Some((
            (self.cx + xr * self.focal / z).round() as i32,
            (self.cy - yu * self.focal / z).round() as i32,
            z,
        ))
    }
}

/// How a facet's surface is dressed, sampled in world space so seams and
/// grain line up across abutting quads.
#[derive(Clone, Copy)]
pub(super) enum Tex {
    /// One even tone: the lambert shade and nothing else.
    Flat,
    /// Sawn planks: seams every `every` units along `axis`, grain between.
    Planks {
        axis: usize,
        every: f32,
        grain: f32,
        seed: u32,
    },
    /// A painted sea for a backdrop: rolling crests in the paint.
    Waves { seed: u32 },
}

impl Tex {
    /// The texture's multiplier on the shade, `0..=1` darker to lighter.
    fn shade(self, wx: f32, wy: f32) -> f32 {
        match self {
            Tex::Flat => 1.0,
            Tex::Planks {
                axis,
                every,
                grain,
                seed,
            } => {
                let along = if axis == 0 { wx } else { wy };
                let seam = (along / every).fract().abs();
                let mut v = 0.86 + (vnoise(wx / grain, wy / grain, seed) - 0.5) * 0.22;
                if seam < 0.06 || seam > 0.94 {
                    v -= 0.35;
                }
                v
            }
            Tex::Waves { seed } => {
                let roll = (wy * 0.55 + (wx * 0.2).sin() * 0.8).sin();
                0.8
                    + roll * 0.12
                    + (vnoise(wx / 2.2, wy / 1.6, seed) - 0.5) * 0.3
                    + ((wy * 2.4 + (wx * 0.7).sin() * 0.5).sin() * 0.5 + 0.5) * 0.14
            }
        }
    }
}

/// A lit surface quad: four points (counter-clockwise seen from the front)
/// shaded onto `ramp` (dark → light).
pub(super) struct Facet {
    pub(super) pts: [[f32; 3]; 4],
    pub(super) ramp: &'static [char],
    pub(super) tex: Tex,
}

/// Paint `facet` with the camera. Far facets first; the caller sorts.
/// Shading is per pixel: perspective-correct world coordinates inside the
/// quad feed the texture, the texture feeds the ramp, and distance dims the
/// lot toward the stage's night.
pub(super) fn facet(cv: &mut Img, cam: &Cam, f: &Facet) {
    let mut corners = [(0i32, 0i32, 0f32); 4];
    for (slot, &p) in f.pts.iter().enumerate() {
        match cam.project(p) {
            Some(s) => corners[slot] = s,
            None => return, // authored scenes keep facets whole on screen
        }
    }
    let (u, v) = (
        [
            f.pts[1][0] - f.pts[0][0],
            f.pts[1][1] - f.pts[0][1],
            f.pts[1][2] - f.pts[0][2],
        ],
        [
            f.pts[3][0] - f.pts[0][0],
            f.pts[3][1] - f.pts[0][1],
            f.pts[3][2] - f.pts[0][2],
        ],
    );
    let mut n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    n = [n[0] / len, n[1] / len, n[2] / len];
    let facing = [
        cam.eye[0] - f.pts[0][0],
        cam.eye[1] - f.pts[0][1],
        cam.eye[2] - f.pts[0][2],
    ];
    let flip = if n[0] * facing[0] + n[1] * facing[1] + n[2] * facing[2] < 0.0 {
        -1.0
    } else {
        1.0
    };
    let lambert = (n[0] * LIGHT_DIR[0] + n[1] * LIGHT_DIR[1] + n[2] * LIGHT_DIR[2]) * flip * 0.5
        + 0.5;
    // Perspective-correct world position: interpolate the corner positions
    // weighted by inverse depth, which is affine in screen space.
    let inv: [f32; 4] = [
        1.0 / corners[0].2,
        1.0 / corners[1].2,
        1.0 / corners[2].2,
        1.0 / corners[3].2,
    ];
    let world_at = |sx: f32, sy: f32| -> [f32; 3] {
        // Bilinear over the quad in screen space on inverse-depth-weighted
        // corners; the weights need no normalization because everything is
        // divided by their sum at the end.
        let tx = ((sx - corners[0].0 as f32)
            / (corners[1].0 as f32 - corners[0].0 as f32 + 1e-6))
        .clamp(0.0, 1.0);
        let ty = ((sy - corners[0].1 as f32)
            / (corners[3].1 as f32 - corners[0].1 as f32 + 1e-6))
        .clamp(0.0, 1.0);
        let mut acc = [0.0f32; 4]; // x*w, y*w, z*w, w
        for (i, &(x, y, _)) in corners.iter().enumerate() {
            let _ = (x, y);
            let (b0, b1) = match i {
                0 => (1.0 - tx, 1.0 - ty),
                1 => (tx, 1.0 - ty),
                2 => (tx, ty),
                _ => (1.0 - tx, ty),
            };
            let wgt = b0 * b1 * inv[i];
            acc[0] += f.pts[i][0] * wgt;
            acc[1] += f.pts[i][1] * wgt;
            acc[3] += wgt;
        }
        // World x and y for the texture, and the depth along the view: the
        // weights' sum is the interpolated inverse depth.
        let inv_w = 1.0 / acc[3];
        [acc[0] * inv_w, acc[1] * inv_w, inv_w]
    };
    let (mut y0, mut y1) = (i32::MAX, i32::MIN);
    for &(_, y, _) in &corners {
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let edge = |y: f32, a: (i32, i32), b: (i32, i32)| -> Option<f32> {
        let (dy, dx) = (b.1 - a.1, b.0 - a.0);
        if dy == 0 {
            return None;
        }
        let t = (y - a.1 as f32) / dy as f32;
        if (0.0..1.0).contains(&t) {
            Some(a.0 as f32 + dx as f32 * t)
        } else {
            None
        }
    };
    let tones = tones(f.ramp);
    for y in y0.max(0)..=y1.min(cv.h - 1) {
        let mut spans: [f32; 4] = [0.0; 4];
        let mut count = 0;
        for i in 0..4 {
            let a = (corners[i].0, corners[i].1);
            let b = (corners[(i + 1) % 4].0, corners[(i + 1) % 4].1);
            if let Some(x) = edge(y as f32 + 0.5, a, b)
                && count < 4
            {
                spans[count] = x;
                count += 1;
            }
        }
        if count < 2 {
            continue;
        }
        spans[..count].sort_by(|a, b| a.total_cmp(b));
        let mut i = 0;
        while i + 1 < count {
            let (xa, xb) = (
                spans[i].ceil().max(0.0) as i32,
                spans[i + 1].floor().min(cv.w as f32 - 1.0) as i32,
            );
            for x in xa..=xb {
                let [wx, wy, depth] = world_at(x as f32 + 0.5, y as f32 + 0.5);
                let tex = f.tex.shade(wx, wy);
                // Distance falls off into the stage's night, per pixel, so
                // a long surface (a canopy, a floor) darkens as it recedes.
                let fade = (((depth - 10.0) * (1.0 / 30.0)).clamp(0.0, 1.0) * (FADES - 1) as f32) as usize;
                let shade = (lambert * tex).clamp(0.0, 1.0);
                let step = (shade * (STEPS - 1) as f32 + 0.5) as usize;
                cv.set(x, y, tones[fade * STEPS + step]);
            }
            i += 2;
        }
    }
}

/// A contact shadow pooled under a world point on the ground plane: a
/// squashed dark ellipse, the stage's one concession to gravity.
pub(super) fn shadow(cv: &mut Img, cam: &Cam, p: [f32; 3], radius: f32) {
    let ground = [p[0], p[1], 0.02];
    let Some((x, y, z)) = cam.project(ground) else {
        return;
    };
    let rx = (cam.focal * radius / z) as i32;
    let ry = (rx as f32 * 0.38) as i32;
    // A solid core thinning out at its rim: soft, never a striped patch.
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let (fx, fy) = (dx as f32 / rx.max(1) as f32, dy as f32 / ry.max(1) as f32);
            let r = fx * fx + fy * fy;
            if r <= 0.55 || (r <= 1.0 && bayer(x + dx, y + dy) > (r - 0.55) / 0.45) {
                cv.put(x + dx, y + dy, 'k');
            }
        }
    }
}

/// A billboard sprite at a world point, `world_h` tall (a fraction of a
/// tile), scaled by depth and stamped upright through the camera. The scale
/// is quantized to eighths so a glide never shimmers between sizes.
pub(super) fn billboard(cv: &mut Img, cam: &Cam, sprite: &Img, p: [f32; 3], world_h: f32) {
    let Some((x, y, z)) = cam.project(p) else {
        return;
    };
    let k = (cam.focal * world_h / z / sprite.h as f32 * 8.0).floor().max(8.0) / 8.0;
    let im = if k > 1.01 {
        super::enlarge(sprite, k)
    } else {
        sprite.clone()
    };
    cv.stamp(&im, x - im.w / 2, y - im.h);
}

/// Order a facet list far-to-near and paint it.
pub(super) fn paint(cv: &mut Img, cam: &Cam, facets: &mut Vec<(f32, Facet)>) {
    facets.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_, f) in facets.iter() {
        facet(cv, cam, f);
    }
    facets.clear();
}

/// The depth of a point, for sorting billboards among facets.
pub(super) fn depth(cam: &Cam, p: [f32; 3]) -> f32 {
    let d = [p[0] - cam.eye[0], p[1] - cam.eye[1], p[2] - cam.eye[2]];
    d[0] * cam.fwd[0] + d[1] * cam.fwd[1] + d[2] * cam.fwd[2]
}
