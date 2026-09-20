//! Hittable trait and HitRecord

use super::math::{Ray, Vector3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRecord {
    pub point: Vector3,
    pub normal: Vector3,
    pub t: f32,
    pub front_face: bool,
}

impl HitRecord {
    #[inline]
    pub fn new(point: Vector3, outward_normal: Vector3, t: f32, ray: &Ray) -> Self {
        let front_face = ray.direction.dot(&outward_normal) < 0.0;
        let normal = if front_face {
            outward_normal
        } else {
            -outward_normal
        };
        Self {
            point,
            normal,
            t,
            front_face,
        }
    }
}

pub trait Hittable {
    fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<HitRecord>;
}
