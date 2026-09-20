//! Vector3 and Ray primitives

use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    #[inline]
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    #[inline]
    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn dot(&self, other: &Vector3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub fn cross(&self, other: &Vector3) -> Vector3 {
        Vector3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    #[inline]
    pub fn normalize(&self) -> Vector3 {
        let len = self.length();
        if len == 0.0 {
            *self
        } else {
            *self / len
        }
    }
}

impl Add for Vector3 {
    type Output = Vector3;
    #[inline]
    fn add(self, rhs: Vector3) -> Self::Output {
        Vector3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl AddAssign for Vector3 {
    #[inline]
    fn add_assign(&mut self, rhs: Vector3) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for Vector3 {
    type Output = Vector3;
    #[inline]
    fn sub(self, rhs: Vector3) -> Self::Output {
        Vector3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl SubAssign for Vector3 {
    #[inline]
    fn sub_assign(&mut self, rhs: Vector3) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul<f32> for Vector3 {
    type Output = Vector3;
    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Vector3::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl Div<f32> for Vector3 {
    type Output = Vector3;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Vector3::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

impl Neg for Vector3 {
    type Output = Vector3;
    #[inline]
    fn neg(self) -> Self::Output {
        Vector3::new(-self.x, -self.y, -self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vector3,
    pub direction: Vector3, // expected to be normalized
}

impl Ray {
    #[inline]
    pub fn new(origin: Vector3, direction: Vector3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    #[inline]
    pub fn at(&self, t: f32) -> Vector3 {
        self.origin + self.direction * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector3_length() {
        let v = Vector3::new(3.0, 4.0, 12.0);
        assert!((v.length() - 13.0).abs() < 1e-6);
    }

    #[test]
    fn test_vector3_normalize() {
        let v = Vector3::new(3.0, 4.0, 0.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_vector3_dot_product() {
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(4.0, -5.0, 6.0);
        assert!((a.dot(&b) - (1.0 * 4.0 + 2.0 * (-5.0) + 3.0 * 6.0)).abs() < 1e-6);
    }

    #[test]
    fn test_vector3_cross_product() {
        let i = Vector3::new(1.0, 0.0, 0.0);
        let j = Vector3::new(0.0, 1.0, 0.0);
        let k = Vector3::new(0.0, 0.0, 1.0);
        assert_eq!(i.cross(&j), k);
        assert_eq!(j.cross(&k), i);
        assert_eq!(k.cross(&i), j);
    }

    #[test]
    fn test_ray_at() {
        let origin = Vector3::new(0.0, 0.0, 0.0);
        let dir = Vector3::new(0.0, 0.0, -1.0);
        let r = Ray::new(origin, dir);
        let p = r.at(2.5);
        assert_eq!(p, Vector3::new(0.0, 0.0, -2.5));
    }
}
