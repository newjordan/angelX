//! Triangle soup helpers for the world renderer, with explicit surface
//! coordinates (the painted materials read strata and grain off `uv`).
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Tri;

#[derive(Default)]
pub struct Soup { pub t: Vec<Tri> }

impl Soup {
    pub fn new() -> Soup { Soup::default() }

    pub fn tri_uv(&mut self, a: V3, b: V3, c: V3, uv: [[f32; 2]; 3], mat: u8) {
        let n = (b - a).cross(c - a);
        let len = n.length();
        if len < 1e-9 { return; }
        let (uv, mat) = match super::palette::flat_kind(mat) { Some(k) => ([super::palette::special_uv(k); 3], super::palette::SKY), None => (uv, mat) };
        self.t.push(Tri { v: [a, b, c], uv, mat, normal: n / len });
    }

    pub fn quad_uv(&mut self, a: V3, b: V3, c: V3, d: V3, uv: [[f32; 2]; 4], mat: u8) {
        self.tri_uv(a, b, c, [uv[0], uv[1], uv[2]], mat);
        self.tri_uv(a, c, d, [uv[0], uv[2], uv[3]], mat);
    }

    pub fn tri(&mut self, a: V3, b: V3, c: V3, mat: u8) {
        self.tri_uv(a, b, c, [[a.x * 0.25, a.y * 0.25], [b.x * 0.25, b.y * 0.25], [c.x * 0.25, c.y * 0.25]], mat);
    }

    pub fn quad(&mut self, a: V3, b: V3, c: V3, d: V3, mat: u8) {
        self.tri(a, b, c, mat);
        self.tri(a, c, d, mat);
    }

    pub fn wall(&mut self, a: V3, b: V3, c: V3, d: V3, mat: u8) {
        let u = |p: V3| (p.x + p.y) * 0.18;
        let uv = |p: V3| [u(p), p.z * 0.35];
        self.tri_uv(a, b, c, [uv(a), uv(b), uv(c)], mat);
        self.tri_uv(a, c, d, [uv(a), uv(c), uv(d)], mat);
    }

    pub fn cuboid(&mut self, lo: V3, hi: V3, mat: u8) {
        let p = |x: bool, y: bool, z: bool| v3(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
        let (f, t) = (false, true);
        self.quad(p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t), mat);
        self.quad(p(f, f, f), p(t, f, f), p(t, t, f), p(f, t, f), mat);
        self.quad(p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t), mat);
        self.quad(p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t), mat);
        self.quad(p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t), mat);
        self.quad(p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t), mat);
    }

    pub fn spike(&mut self, base: V3, tip: V3, r: f32, mat: u8) {
        let ax = (tip - base).normalize();
        let u = ax.cross(if ax.z.abs() > 0.9 { v3(1., 0., 0.) } else { V3::UP }).normalize();
        let w = ax.cross(u);
        let at = |i: usize| {
            let (s, c) = (i as f32 / 4. * std::f32::consts::TAU).sin_cos();
            base + (u * c + w * s) * r
        };
        for i in 0..4 { self.tri(at(i), at(i + 1), tip, mat); }
    }

    pub fn limb(&mut self, a: V3, b: V3, r0: f32, r1: f32, mat: u8) { self.limb_n(a, b, r0, r1, 4, mat); }

    pub fn limb_n(&mut self, a: V3, b: V3, r0: f32, r1: f32, n: usize, mat: u8) {
        let len = (b - a).length();
        if len < 1e-4 { return; }
        let ax = (b - a) * (1. / len);
        let u = ax.cross(if ax.z.abs() > 0.9 { v3(1., 0., 0.) } else { V3::UP }).normalize();
        let w = ax.cross(u);
        let ring = |c: V3, r: f32, i: usize| {
            let (s, co) = (i as f32 / n as f32 * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4).sin_cos();
            c + (u * co + w * s) * r
        };
        for i in 0..n {
            let j = (i + 1) % n;
            self.quad(ring(a, r0, i), ring(a, r0, j), ring(b, r1, j), ring(b, r1, i), mat);
        }
    }

    pub fn column(&mut self, c: V3, h: f32, r0: f32, r1: f32, n: usize, mat: u8) {
        let ring = |z: f32, r: f32, i: usize| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            v3(c.x + r * a.cos(), c.y + r * a.sin(), c.z + z)
        };
        for i in 0..n {
            let j = (i + 1) % n;
            self.quad(ring(0., r0, i), ring(0., r0, j), ring(h, r1, j), ring(h, r1, i), mat);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            self.tri(v3(c.x, c.y, c.z + h), ring(h, r1, i), ring(h, r1, j), mat);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Fr { pub o: V3, pub f: V3, pub l: V3 }

impl Fr {
    pub fn on(row: &super::frame::Row, lat: f32) -> Fr {
        Fr { o: v3(row.x - lat * row.sn, row.y + lat * row.cs, row.h), f: v3(row.cs, row.sn, 0.), l: v3(-row.sn, row.cs, 0.) }
    }
    pub fn pt(&self, a: f32, l: f32, u: f32) -> V3 { self.o + self.f * a + self.l * l + V3::UP * u }
}

impl Soup {
    pub fn bx(&mut self, fr: &Fr, a: (f32, f32), l: (f32, f32), u: (f32, f32), mat: u8) {
        let p = |x: bool, y: bool, z: bool| fr.pt(if x { a.1 } else { a.0 }, if y { l.1 } else { l.0 }, if z { u.1 } else { u.0 });
        let (f, t) = (false, true);
        self.quad(p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t), mat);
        self.quad(p(f, f, f), p(t, f, f), p(t, t, f), p(f, t, f), mat);
        self.quad(p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t), mat);
        self.quad(p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t), mat);
        self.quad(p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t), mat);
        self.quad(p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t), mat);
    }

    pub fn bx_open(&mut self, fr: &Fr, a: (f32, f32), l: (f32, f32), u: (f32, f32), mat: u8) {
        let p = |x: bool, y: bool, z: bool| fr.pt(if x { a.1 } else { a.0 }, if y { l.1 } else { l.0 }, if z { u.1 } else { u.0 });
        let (f, t) = (false, true);
        self.quad(p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t), mat);
        self.quad(p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t), mat);
        self.quad(p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t), mat);
        self.quad(p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t), mat);
        self.quad(p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t), mat);
    }

    pub fn slab(&mut self, fr: &Fr, a: (f32, f32), l: (f32, f32), u: (f32, f32), thick: f32, mat: u8) {
        let p = |x: bool, y: bool, z: f32| fr.pt(if x { a.1 } else { a.0 }, if y { l.1 } else { l.0 }, z);
        let (f, t) = (false, true);
        self.quad(p(f, f, u.0), p(t, f, u.1), p(t, t, u.1), p(f, t, u.0), mat);
        self.quad(p(f, f, u.0 - thick), p(t, f, u.1 - thick), p(t, t, u.1 - thick), p(f, t, u.0 - thick), mat);
        self.quad(p(f, f, u.0), p(t, f, u.1), p(t, f, u.1 - thick), p(f, f, u.0 - thick), mat);
        self.quad(p(f, t, u.0), p(t, t, u.1), p(t, t, u.1 - thick), p(f, t, u.0 - thick), mat);
        self.quad(p(f, f, u.0), p(f, t, u.0), p(f, t, u.0 - thick), p(f, f, u.0 - thick), mat);
        self.quad(p(t, f, u.1), p(t, t, u.1), p(t, t, u.1 - thick), p(t, f, u.1 - thick), mat);
    }

    pub fn sweep(&mut self, fr: &Fr, a: (f32, f32), steps: usize, ring: impl Fn(f32) -> (f32, f32), mat: u8) {
        for i in 0..steps {
            let (r0, r1) = (ring(i as f32 / steps as f32), ring((i + 1) as f32 / steps as f32));
            self.quad(fr.pt(a.0, r0.0, r0.1), fr.pt(a.1, r0.0, r0.1), fr.pt(a.1, r1.0, r1.1), fr.pt(a.0, r1.0, r1.1), mat);
        }
    }
}
