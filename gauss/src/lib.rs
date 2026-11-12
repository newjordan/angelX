//! Procedural 3D Gaussian UI, independent of the Glass window.
//! Each surfel has a centre and three scaled, oriented covariance axes.
use std::f32::consts::{PI, TAU};

pub type V3 = [f32; 3];
pub fn add(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] + b[i])
}
pub fn mul(a: V3, k: f32) -> V3 {
    a.map(|v| v * k)
}
pub fn dot(a: V3, b: V3) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
pub fn unit(a: V3) -> V3 {
    mul(a, 1.0 / dot(a, a).sqrt().max(0.00001))
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct Splat {
    pub centre: V3,
    pub axes: [V3; 3],
    pub normal: V3,
    pub material: usize,
    pub alpha: f32,
    pub moving: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Projected {
    pub centre: [f32; 2],
    pub sigma: [f32; 2],
    pub angle: f32,
    pub depth: f32,
    pub color: usize,
    pub alpha: f32,
}
#[derive(Clone, Debug)]
pub struct State {
    pub powered: bool,
    pub orbit: bool,
    pub warm: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub resonance: f32,
    pub energy: f32,
    pub time: f32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            powered: true,
            orbit: false,
            warm: true,
            yaw: -0.30,
            pitch: 0.55,
            zoom: 1.0,
            resonance: 0.48,
            energy: 1.0,
            time: 0.0,
        }
    }
}
impl State {
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        let target = if self.powered { 1.0 } else { 0.0 };
        self.energy += (target - self.energy) * (1.0 - (-dt * 6.0).exp());
        if self.orbit {
            self.yaw = (self.yaw + dt * 0.16).rem_euclid(TAU);
        }
    }
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.orbit = false;
        self.yaw = (self.yaw + dx * 0.008).rem_euclid(TAU);
        self.pitch = (self.pitch + dy * 0.006).clamp(-0.18, 1.25);
    }
    pub fn zoom_by(&mut self, steps: f32) {
        self.zoom = (self.zoom * (steps * 0.08).exp()).clamp(0.65, 1.55);
    }
    pub fn tune(&mut self, value: f32) {
        self.resonance = value.clamp(0.0, 1.0);
    }
    pub fn frequency(&self) -> f32 {
        110.0 * 4.0_f32.powf(self.resonance)
    }
    pub fn rotate(&self, p: V3) -> V3 {
        let (s, c) = self.yaw.sin_cos();
        let p = [c * p[0] + s * p[2], p[1], -s * p[0] + c * p[2]];
        let (s, c) = self.pitch.sin_cos();
        [p[0], c * p[1] - s * p[2], s * p[1] + c * p[2]]
    }
    pub fn screen(&self, p: V3, rect: [f32; 4]) -> ([f32; 2], f32) {
        let p = self.rotate(p);
        let d = 6.0 - p[2];
        let f = rect[2].min(rect[3] * 1.32) * 2.0 * self.zoom;
        (
            [
                rect[0] + rect[2] * 0.5 + f * p[0] / d,
                rect[1] + rect[3] * 0.52 - f * p[1] / d,
            ],
            d,
        )
    }
    pub fn power_position(&self, rect: [f32; 4]) -> [f32; 2] {
        self.screen([0.0, 0.30, 0.0], rect).0
    }
}

fn surfel(p: V3, n: V3, scale: f32, material: usize, moving: bool) -> Splat {
    let n = unit(n);
    let tangent = unit(cross(
        n,
        if n[1].abs() < 0.9 {
            [0., 1., 0.]
        } else {
            [1., 0., 0.]
        },
    ));
    Splat {
        centre: p,
        axes: [
            mul(tangent, scale),
            mul(cross(n, tangent), scale),
            mul(n, scale * 0.27),
        ],
        normal: n,
        material,
        alpha: 0.92,
        moving,
    }
}
fn torus(out: &mut Vec<Splat>, major: f32, minor: f32, y: f32, material: usize, moving: bool) {
    let u_count = (major * 210.0) as usize;
    let v_count = 14;
    for u in 0..u_count {
        for v in 0..v_count {
            let a = TAU * (u as f32 + 0.5 * (v % 2) as f32) / u_count as f32;
            let b = TAU * v as f32 / v_count as f32;
            let n = [a.cos() * b.cos(), b.sin(), a.sin() * b.cos()];
            let p = [
                (major + minor * b.cos()) * a.cos(),
                y + minor * b.sin(),
                (major + minor * b.cos()) * a.sin(),
            ];
            out.push(surfel(p, n, 0.024, material, moving));
        }
    }
}
/// A tactile, oblate ceramic instrument with metallic and emissive rings.
pub fn instrument() -> Vec<Splat> {
    let mut out = Vec::new();
    let count = 5600;
    for i in 0..count {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let a = i as f32 * PI * (3.0 - 5.0_f32.sqrt());
        let r = (1.0 - y * y).sqrt();
        let p = [1.13 * r * a.cos(), -0.20 + 0.34 * y, 1.13 * r * a.sin()];
        let n = [
            p[0] / 1.13_f32.powi(2),
            (p[1] + 0.20) / 0.34_f32.powi(2),
            p[2] / 1.13_f32.powi(2),
        ];
        out.push(surfel(p, n, 0.034, 0, false));
    }
    torus(&mut out, 1.03, 0.039, -0.08, 1, false);
    torus(&mut out, 0.97, 0.017, -0.13, 3, false);
    torus(&mut out, 0.72, 0.033, 0.08, 1, false);
    torus(&mut out, 0.56, 0.018, 0.13, 2, true);
    // A glassy central button, not a billboard. Its hit target projects with it.
    for i in 0..1100 {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / 1100.0;
        let a = i as f32 * 2.399963;
        let r = (1.0 - y * y).sqrt();
        let p = [0.36 * r * a.cos(), 0.17 + 0.15 * y, 0.36 * r * a.sin()];
        out.push(surfel(p, [p[0], (p[1] - 0.17) * 4.0, p[2]], 0.024, 2, true));
    }
    // Engraved radial tick marks on the bezel.
    for i in 0..72 {
        let a = i as f32 * TAU / 72.0;
        for j in 0..4 {
            let r = 0.79 + j as f32 * 0.012;
            out.push(surfel(
                [r * a.cos(), 0.077, r * a.sin()],
                [0., 1., 0.],
                0.008,
                if i % 6 == 0 { 1 } else { 3 },
                false,
            ));
        }
    }
    out
}

/// Palette: four materials, 64 linear-light-ish shades each.
pub fn palette(index: usize, warm: bool) -> [u8; 3] {
    let k = (index % 64) as f32 / 63.0;
    let (low, high) = match index / 64 {
        0 => ([0.023, 0.029, 0.047], [0.48, 0.52, 0.62]),
        1 if warm => ([0.05, 0.022, 0.013], [1.0, 0.79, 0.53]),
        1 => ([0.022, 0.038, 0.06], [0.71, 0.86, 1.0]),
        2 if warm => ([0.012, 0.055, 0.052], [0.52, 1.0, 0.87]),
        2 => ([0.025, 0.02, 0.06], [0.72, 0.62, 1.0]),
        _ => ([0.018, 0.025, 0.029], [0.19, 0.26, 0.29]),
    };
    std::array::from_fn(|i| ((low[i] + (high[i] - low[i]) * k) * 255.0) as u8)
}

pub fn project(splats: &[Splat], state: &State, rect: [f32; 4]) -> Vec<Projected> {
    let f = rect[2].min(rect[3] * 1.32) * 2.0 * state.zoom;
    let light = unit(if state.warm {
        [-0.5, 0.85, 1.0]
    } else {
        [0.8, 0.3, 0.8]
    });
    let half = unit(add(light, [0., 0., 1.]));
    let mut out = Vec::with_capacity(splats.len());
    for s in splats {
        let mut p = s.centre;
        if s.moving {
            let pulse = (state.time * 1.8).sin() * 0.012 * state.energy;
            let angle = p[2].atan2(p[0]);
            let shape = 1.0
                + (state.resonance - 0.48) * 0.16
                + (angle * 6.0 + state.time * 0.7).sin() * state.resonance * 0.035 * state.energy;
            p[0] *= shape;
            p[2] *= shape;
            p[1] += pulse + state.resonance * 0.035;
        }
        let p = state.rotate(p);
        let d = 6.0 - p[2];
        if d <= 0.1 {
            continue;
        }
        // Jacobian of perspective projection times each covariance axis.
        let mut c = [0.30_f32, 0.0, 0.30];
        for a in s.axes {
            let a = state.rotate(a);
            let x = f / d * (a[0] + p[0] * a[2] / d);
            let y = -f / d * (a[1] + p[1] * a[2] / d);
            c[0] += x * x;
            c[1] += x * y;
            c[2] += y * y;
        }
        let disc = ((c[0] - c[2]).powi(2) + 4.0 * c[1] * c[1]).sqrt();
        let sigma = [
            ((c[0] + c[2] + disc) * 0.5).sqrt(),
            ((c[0] + c[2] - disc) * 0.5).max(0.25).sqrt(),
        ];
        let normal = state.rotate(s.normal);
        let diffuse = dot(normal, light).max(0.0);
        let specular = dot(normal, half)
            .max(0.0)
            .powf(if s.material == 1 { 55.0 } else { 22.0 });
        let rim = (1.0 - normal[2].abs()).powi(3) * 0.18;
        let mut shade = 0.10 + diffuse * 0.52 + specular * 0.65 + rim;
        if s.material == 2 {
            shade = ((0.07 + shade * 0.43) * (0.20 + 0.80 * state.energy) + 0.38 * state.energy)
                .min(1.0);
        }
        let color = s.material * 64 + (shade.clamp(0.0, 1.0) * 63.0) as usize;
        let centre = [
            rect[0] + rect[2] * 0.5 + f * p[0] / d,
            rect[1] + rect[3] * 0.52 - f * p[1] / d,
        ];
        // Cull to the stage, including the full 3-sigma footprint.
        if centre[0] + 3.0 * sigma[0] < rect[0]
            || centre[0] - 3.0 * sigma[0] > rect[0] + rect[2]
            || centre[1] + 3.0 * sigma[0] < rect[1]
            || centre[1] - 3.0 * sigma[0] > rect[1] + rect[3]
        {
            continue;
        }
        out.push(Projected {
            centre,
            sigma,
            angle: 0.5 * (2.0 * c[1]).atan2(c[0] - c[2]),
            depth: d,
            color,
            alpha: s.alpha,
        });
    }
    // Alpha over requires far-to-near order, not additive point sprites.
    out.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_is_finite_anisotropic_and_depth_sorted() {
        let splats = instrument();
        assert!(splats.len() > 10_000);
        let p = project(&splats, &State::default(), [0., 0., 800., 600.]);
        assert!(!p.is_empty());
        assert!(p.windows(2).all(|p| p[0].depth >= p[1].depth));
        assert!(p.iter().all(|p| p.sigma[0].is_finite()
            && p.sigma[1] >= 0.5
            && p.angle.is_finite()
            && p.color < 256));
        assert!(p.iter().any(|p| p.sigma[0] / p.sigma[1] > 2.0));
    }
    #[test]
    fn controls_clamp_and_power_eases_in_both_directions() {
        let mut s = State::default();
        s.drag(20000., 20000.);
        assert!(!s.orbit);
        assert_eq!(s.pitch, 1.25);
        s.zoom_by(1000.);
        assert_eq!(s.zoom, 1.55);
        s.zoom_by(-1000.);
        assert_eq!(s.zoom, 0.65);
        s.tune(-1.);
        assert_eq!(s.frequency(), 110.);
        s.tune(2.);
        assert_eq!(s.frequency(), 440.);
        s.powered = false;
        for _ in 0..40 {
            s.tick(0.05);
        }
        assert!(s.energy < 0.001);
        s.powered = true;
        for _ in 0..40 {
            s.tick(0.05);
        }
        assert!(s.energy > 0.999);
    }
    #[test]
    fn hit_anchor_follows_camera_and_model_is_deterministic() {
        let rect = [20., 50., 640., 480.];
        let mut s = State::default();
        let p = s.power_position(rect);
        s.drag(70., 30.);
        assert_ne!(p, s.power_position(rect));
        assert_eq!(instrument().len(), instrument().len());
        assert!(project(&[], &s, rect).is_empty());
    }
}

pub mod import;

pub mod gpu;
