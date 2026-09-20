//! Minimal 3D vector math for the world3d renderer. Seeded from the vendored
//! dotmax raytracer's Vector3, trimmed to f32 and the ops the raster path uses.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub(crate) const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub(crate) const ZERO: V3 = v3(0.0, 0.0, 0.0);
    pub(crate) const UP: V3 = v3(0.0, 0.0, 1.0);

    pub(crate) fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub(crate) fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub(crate) fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Zero-safe: degenerate vectors normalize to ZERO instead of NaN.
    pub(crate) fn normalize(self) -> V3 {
        let len = self.length();
        if len <= 1e-12 { V3::ZERO } else { self / len }
    }

    pub(crate) fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for V3 {
    fn add_assign(&mut self, o: V3) {
        *self = *self + o;
    }
}

impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, s: f32) -> V3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<V3> for f32 {
    type Output = V3;
    fn mul(self, v: V3) -> V3 {
        v * self
    }
}

impl Div<f32> for V3 {
    type Output = V3;
    fn div(self, s: f32) -> V3 {
        v3(self.x / s, self.y / s, self.z / s)
    }
}
