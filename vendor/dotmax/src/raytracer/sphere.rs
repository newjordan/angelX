//! Sphere implementation of Hittable

use super::hittable::{HitRecord, Hittable};
use super::math::{Ray, Vector3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
}

impl Sphere {
    #[inline]
    pub fn new(center: Vector3, radius: f32) -> Self {
        Self { center, radius }
    }
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        // Quadratic in terms of t: a t^2 + 2b t + c = 0 (we use half_b = b for stability)
        let oc = ray.origin - self.center;
        let a = ray.direction.length_squared();
        let half_b = oc.dot(&ray.direction);
        let c = oc.length_squared() - self.radius * self.radius;
        let discriminant = half_b * half_b - a * c;
        if discriminant < 0.0 {
            return None;
        }
        let sqrt_d = discriminant.sqrt();

        // Find the nearest root that lies in the acceptable range.
        let mut root = (-half_b - sqrt_d) / a;
        if root < t_min || root > t_max {
            root = (-half_b + sqrt_d) / a;
            if root < t_min || root > t_max {
                return None;
            }
        }

        let point = ray.at(root);
        let outward_normal = (point - self.center) / self.radius;
        Some(HitRecord::new(point, outward_normal, root, ray))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_equal(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_sphere_hit_from_outside() {
        let sphere = Sphere::new(Vector3::new(0.0, 0.0, -5.0), 1.0);
        let ray = Ray::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, -1.0));
        let hit = sphere.hit(&ray, 0.001, f32::MAX).expect("should hit");
        // Expect intersection near z = -4 at t ~= 4
        assert!(hit.t > 3.9 && hit.t < 4.1, "t was {}", hit.t);
        assert!(approx_equal(hit.point.z, -4.0));
        assert!(hit.front_face);
        assert!(approx_equal(hit.normal.z, 1.0));
    }

    #[test]
    fn test_sphere_hit_from_inside() {
        let sphere = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        let ray = Ray::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        let hit = sphere
            .hit(&ray, 0.001, f32::MAX)
            .expect("should hit from inside");
        // From center to surface along +X is exactly radius
        assert!(approx_equal(hit.t, 1.0));
        assert!(!hit.front_face); // Ray exits the surface; outward normal points +X, ray dir +X
        assert!(approx_equal(hit.normal.x, -1.0)); // Adjusted to always oppose ray
    }

    #[test]
    fn test_sphere_miss() {
        let sphere = Sphere::new(Vector3::new(0.0, 0.0, -5.0), 1.0);
        let ray = Ray::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        let hit = sphere.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_none());
    }

    #[test]
    fn test_sphere_tangent() {
        // Sphere at origin with radius 1. Ray skims at y=1 from left to right
        let sphere = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        let ray = Ray::new(Vector3::new(-2.0, 1.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        let hit = sphere
            .hit(&ray, 0.0, f32::MAX)
            .expect("tangent should count as hit");
        // Discriminant == 0 => single root. Tangent point is (0,1,0) at t=2.0
        assert!(approx_equal(hit.t, 2.0));
        assert!(approx_equal(hit.point.x, 0.0));
        assert!(approx_equal(hit.point.y, 1.0));
    }

    #[test]
    fn test_sphere_closest_hit_root_selected() {
        // Two roots: expect nearer root chosen
        let sphere = Sphere::new(Vector3::new(0.0, 0.0, -5.0), 1.0);
        let ray = Ray::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, -1.0));
        let hit = sphere.hit(&ray, 0.001, f32::MAX).unwrap();
        assert!(hit.t < 5.0); // nearer than center distance
    }
}
