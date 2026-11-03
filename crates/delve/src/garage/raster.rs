//! The garage's own little renderer: world3d meshes seen by a pinhole
//! camera, shaded per pixel by the shop's lamps and painted with the garage's
//! materials. The game's banded rasterizer lights a night with a moon; a
//! shop is lit from within, in pools under hanging lamps, so it has its own.
//!
//! It draws in two passes. The first only finds which triangle is nearest at
//! each pixel (a visibility buffer, so a pixel is lit once however many
//! surfaces were drawn over it); the second lights those pixels. Buffers are
//! kept between frames. Pure and deterministic: the same meshes, camera and
//! lamps give the same pixels.

use super::look::{Surf, fog_colour, surface};
use crate::render::world3d::math::V3;
use crate::render::world3d::mesh::Mesh;

/// A pinhole camera, looking along `fwd`.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: V3,
    pub fwd: V3,
    pub right: V3,
    pub up: V3,
    /// Focal length, in pixels.
    pub focal: f32,
    pub w: usize,
    pub h: usize,
}

impl Camera {
    /// A camera at `eye` looking at `target`, `fov` radians across `w` pixels.
    pub fn look(eye: V3, target: V3, fov: f32, w: usize, h: usize) -> Camera {
        let fwd = (target - eye).normalize();
        let right = fwd.cross(V3::UP).normalize();
        let up = right.cross(fwd);
        Camera { eye, fwd, right, up, focal: w as f32 * 0.5 / (fov * 0.5).tan(), w, h }
    }

    /// A world point as (x, y, depth) on the screen; none behind the lens.
    pub fn project(&self, p: V3) -> Option<(f32, f32, f32)> {
        let d = p - self.eye;
        let z = d.dot(self.fwd);
        (z > 0.05).then(|| {
            (
                self.w as f32 * 0.5 + d.dot(self.right) / z * self.focal,
                self.h as f32 * 0.5 - d.dot(self.up) / z * self.focal,
                z,
            )
        })
    }

    /// The world point at depth `z` seen through the middle of pixel (x, y).
    fn unproject(&self, x: usize, y: usize, z: f32) -> V3 {
        let sx = (x as f32 + 0.5 - self.w as f32 * 0.5) / self.focal;
        let sy = (self.h as f32 * 0.5 - y as f32 - 0.5) / self.focal;
        self.eye + (self.fwd + self.right * sx + self.up * sy) * z
    }
}

/// A lamp: light that falls to nothing at `reach`.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub pos: V3,
    pub colour: [f32; 3],
    pub reach: f32,
    /// A glow drawn round the lamp itself, in pixels at 10 units' distance.
    pub halo: f32,
}

/// The whole shop's lighting.
#[derive(Clone, Debug)]
pub struct Lighting {
    pub ambient: [f32; 3],
    pub lamps: Vec<Light>,
}

/// The buffers one frame is drawn in.
pub struct Raster {
    pub w: usize,
    pub h: usize,
    /// 1 / depth of the nearest surface at each pixel; 0 for none.
    inv_z: Vec<f32>,
    /// Which triangle: mesh in the top byte, triangle in the rest, plus one.
    hit: Vec<u32>,
    pub rgb: Vec<[u8; 3]>,
}

impl Raster {
    pub fn new(w: usize, h: usize) -> Raster {
        Raster { w, h, inv_z: vec![0.0; w * h], hit: vec![0; w * h], rgb: vec![[0; 3]; w * h] }
    }

    /// The depth at a pixel, if anything was drawn there.
    pub fn depth(&self, x: usize, y: usize) -> Option<f32> {
        let i = self.inv_z[y * self.w + x];
        (i > 0.0).then(|| 1.0 / i)
    }

    /// Which pixels the meshes cover and how near.
    pub fn visibility(&mut self, cam: &Camera, meshes: &[&Mesh]) {
        self.inv_z.fill(0.0);
        self.hit.fill(0);
        for (mi, mesh) in meshes.iter().enumerate() {
            for (ti, t) in mesh.tris.iter().enumerate() {
                // into the camera's space: x right, y up, z forward
                let c = t.v.map(|p| {
                    let d = p - cam.eye;
                    [d.dot(cam.right), d.dot(cam.up), d.dot(cam.fwd)]
                });
                let id = ((mi as u32) << 24) | (ti as u32 + 1);
                let near = 0.1;
                let (poly, n) = clip_near(&c, near);
                for k in 1..n.saturating_sub(1) {
                    self.fill(cam, [poly[0], poly[k], poly[k + 1]], id);
                }
            }
        }
    }

    fn fill(&mut self, cam: &Camera, v: [[f32; 3]; 3], id: u32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let s = v.map(|[x, y, z]| [w * 0.5 + x / z * cam.focal, h * 0.5 - y / z * cam.focal, 1.0 / z]);
        let area = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[1][1] - s[0][1]) * (s[2][0] - s[0][0]);
        if area.abs() < 1e-6 {
            return;
        }
        let x0 = s.iter().map(|p| p[0]).fold(f32::MAX, f32::min).max(0.0);
        let x1 = s.iter().map(|p| p[0]).fold(f32::MIN, f32::max).min(w - 1.0);
        let y0 = s.iter().map(|p| p[1]).fold(f32::MAX, f32::min).max(0.0);
        let y1 = s.iter().map(|p| p[1]).fold(f32::MIN, f32::max).min(h - 1.0);
        if x0 > x1 || y0 > y1 {
            return;
        }
        let inv = 1.0 / area;
        let edge = |a: [f32; 3], b: [f32; 3], x: f32, y: f32| (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
        for py in y0 as usize..=y1 as usize {
            let fy = py as f32 + 0.5;
            for px in x0 as usize..=x1 as usize {
                let fx = px as f32 + 0.5;
                let w0 = edge(s[1], s[2], fx, fy) * inv;
                let w1 = edge(s[2], s[0], fx, fy) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let iz = w0 * s[0][2] + w1 * s[1][2] + w2 * s[2][2];
                let i = py * self.w + px;
                if iz > self.inv_z[i] {
                    self.inv_z[i] = iz;
                    self.hit[i] = id;
                }
            }
        }
    }

    /// Light the covered pixels; the rest is the shop's dark.
    pub fn shade(&mut self, cam: &Camera, meshes: &[&Mesh], lights: &Lighting, sky: [u8; 3]) {
        let mut surf = Surf::default();
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                let id = self.hit[i];
                if id == 0 {
                    self.rgb[i] = sky;
                    continue;
                }
                let tri = &meshes[(id >> 24) as usize].tris[(id & 0x00ff_ffff) as usize - 1];
                let z = 1.0 / self.inv_z[i];
                let p = cam.unproject(x, y, z);
                let to_eye = (cam.eye - p).normalize();
                let n = if tri.normal.dot(to_eye) < 0.0 { -tri.normal } else { tri.normal };
                surface(tri.mat, p, n, &mut surf);
                self.rgb[i] = light(&surf, p, n, to_eye, z, lights);
            }
        }
    }

    /// Add a soft glow round each lamp that has one and is not hidden.
    pub fn halos(&mut self, cam: &Camera, lights: &Lighting) {
        for l in &lights.lamps {
            if l.halo <= 0.0 {
                continue;
            }
            let Some((sx, sy, z)) = cam.project(l.pos) else { continue };
            let r = l.halo * 10.0 / z.max(1.0);
            let (x0, x1) = ((sx - r).floor().max(0.0) as usize, ((sx + r).ceil() as usize).min(self.w.saturating_sub(1)));
            let (y0, y1) = ((sy - r).floor().max(0.0) as usize, ((sy + r).ceil() as usize).min(self.h.saturating_sub(1)));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let d = (((x as f32 + 0.5 - sx).powi(2) + (y as f32 + 0.5 - sy).powi(2)).sqrt() / r).min(1.0);
                    let k = (1.0 - d).powi(3) * 0.8;
                    if k <= 0.0 {
                        continue;
                    }
                    // hidden behind something nearer than the lamp: no glow
                    let i = y * self.w + x;
                    if self.inv_z[i] > 0.0 && 1.0 / self.inv_z[i] < z - 0.5 {
                        continue;
                    }
                    for (p, c) in self.rgb[i].iter_mut().zip(l.colour) {
                        *p = (f32::from(*p) + c * 255.0 * k * 0.5).min(255.0) as u8;
                    }
                }
            }
        }
    }
}

/// The polygon of a triangle in camera space that is in front of `near`.
fn clip_near(c: &[[f32; 3]; 3], near: f32) -> ([[f32; 3]; 4], usize) {
    let mut out = [[0.0; 3]; 4];
    let mut n = 0;
    for i in 0..3 {
        let (a, b) = (c[i], c[(i + 1) % 3]);
        let (ai, bi) = (a[2] >= near, b[2] >= near);
        if ai {
            out[n] = a;
            n += 1;
        }
        if ai != bi {
            let t = (near - a[2]) / (b[2] - a[2]);
            out[n] = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, near];
            n += 1;
        }
    }
    (out, n)
}

/// One lit surface point.
fn light(s: &Surf, p: V3, n: V3, to_eye: V3, depth: f32, lights: &Lighting) -> [u8; 3] {
    let mut c = [0.0f32; 3];
    // a little sky bounce, more on what faces up
    let sky = 0.55 + 0.45 * n.z.max(0.0);
    for (k, v) in c.iter_mut().enumerate() {
        *v = s.albedo[k] * lights.ambient[k] * sky;
    }
    for l in &lights.lamps {
        let d = l.pos - p;
        let d2 = d.dot(d);
        if d2 >= l.reach * l.reach {
            continue;
        }
        let dist = d2.sqrt();
        let dir = d / dist.max(1e-4);
        let fall = (1.0 - d2 / (l.reach * l.reach)).powi(2);
        let ndl = (n.dot(dir) * 0.85 + 0.15).max(0.0);
        let mut spec = 0.0;
        if s.spec > 0.0 && ndl > 0.0 {
            let h = (dir + to_eye).normalize();
            spec = n.dot(h).max(0.0).powf(s.shin) * s.spec * 0.55;
        }
        for (k, v) in c.iter_mut().enumerate() {
            *v += l.colour[k] * fall * (s.albedo[k] * ndl + spec);
        }
    }
    let fog = fog_colour();
    let haze = 1.0 - (-depth * 0.014).exp();
    let mut out = [0u8; 3];
    for k in 0..3 {
        let lit = c[k] + s.glow[k];
        // roll off toward white softly, never clip hard
        let mapped = 1.0 - (-lit * 1.45).exp();
        let v = mapped * (1.0 - haze) + fog[k] * haze;
        out[k] = (v.clamp(0.0, 1.0).powf(0.92) * 255.0) as u8;
    }
    out
}
