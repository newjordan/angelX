//! Triangle primitive for ray tracing (VIZ-016)
//!
//! This module implements ray-triangle intersection using the Möller-Trumbore algorithm,
//! which is the industry-standard method for efficient ray-triangle intersection testing.
//!
//! Single Responsibility: Ray-triangle intersection math only.

use super::hittable::{HitRecord, Hittable};
use super::math::{Ray, Vector3};

/// A single triangle defined by three vertices
///
/// Triangles are the fundamental building block of 3D meshes.
/// This implementation supports both flat shading (using geometric normal)
/// and smooth shading (using interpolated vertex normals).
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    /// First vertex position
    pub v0: Vector3,
    /// Second vertex position
    pub v1: Vector3,
    /// Third vertex position
    pub v2: Vector3,
    /// Optional normal at v0 (for smooth shading)
    pub n0: Option<Vector3>,
    /// Optional normal at v1 (for smooth shading)
    pub n1: Option<Vector3>,
    /// Optional normal at v2 (for smooth shading)
    pub n2: Option<Vector3>,
}

impl Triangle {
    /// Create a new triangle with flat shading (no vertex normals)
    ///
    /// The geometric normal will be computed from the triangle's vertices.
    #[inline]
    pub fn new(v0: Vector3, v1: Vector3, v2: Vector3) -> Self {
        Self {
            v0,
            v1,
            v2,
            n0: None,
            n1: None,
            n2: None,
        }
    }

    /// Create a new triangle with smooth shading (vertex normals provided)
    ///
    /// Normals will be interpolated across the triangle surface using
    /// barycentric coordinates for smooth shading.
    #[inline]
    pub fn with_normals(
        v0: Vector3,
        v1: Vector3,
        v2: Vector3,
        n0: Vector3,
        n1: Vector3,
        n2: Vector3,
    ) -> Self {
        Self {
            v0,
            v1,
            v2,
            n0: Some(n0),
            n1: Some(n1),
            n2: Some(n2),
        }
    }

    /// Calculate the geometric (face) normal of the triangle
    ///
    /// This is computed from the cross product of two edges.
    /// Used for flat shading when vertex normals are not provided.
    #[inline]
    fn geometric_normal(&self) -> Vector3 {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        edge1.cross(&edge2).normalize()
    }
}

impl Hittable for Triangle {
    /// Test ray-triangle intersection using Möller-Trumbore algorithm
    ///
    /// This is the industry-standard algorithm for ray-triangle intersection.
    /// It computes barycentric coordinates (u, v, w) which are used for:
    /// - Determining if the ray hits inside the triangle
    /// - Interpolating vertex normals for smooth shading
    ///
    /// Algorithm reference: Möller & Trumbore (1997)
    /// "Fast, Minimum Storage Ray-Triangle Intersection"
    fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        // Epsilon for floating-point comparisons
        const EPSILON: f32 = 1e-6;

        // Compute edges from v0
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;

        // Begin calculating determinant - also used to calculate u parameter
        let h = ray.direction.cross(&edge2);
        let a = edge1.dot(&h);

        // If determinant is near zero, ray is parallel to triangle
        if a.abs() < EPSILON {
            return None;
        }

        let f = 1.0 / a;
        let s = ray.origin - self.v0;
        let u = f * s.dot(&h);

        // Check if intersection is outside triangle (barycentric u coordinate)
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(&edge1);
        let v = f * ray.direction.dot(&q);

        // Check if intersection is outside triangle (barycentric v coordinate)
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        // At this point we can compute t to find out where the intersection point is on the line
        let t = f * edge2.dot(&q);

        // Check if intersection is within valid ray range
        if t < t_min || t > t_max {
            return None;
        }

        // We have a valid intersection!
        let point = ray.at(t);

        // Compute normal: interpolate if vertex normals provided, else use geometric normal
        let normal = if let (Some(n0), Some(n1), Some(n2)) = (self.n0, self.n1, self.n2) {
            // Smooth shading: interpolate normals using barycentric coordinates
            // w = 1 - u - v (third barycentric coordinate)
            let w = 1.0 - u - v;
            (n0 * w + n1 * u + n2 * v).normalize()
        } else {
            // Flat shading: use geometric normal
            self.geometric_normal()
        };

        Some(HitRecord::new(point, normal, t, ray))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_equal(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_triangle_hit_center() {
        // Triangle in XY plane at z = -3
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        // Ray from origin pointing at center of triangle
        let ray = Ray::new(Vector3::new(0.0, 0.3, 0.0), Vector3::new(0.0, 0.0, -1.0));

        let hit = tri.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_some(), "Ray should hit triangle center");

        let hit = hit.unwrap();
        assert!(approx_equal(hit.t, 3.0), "Hit distance should be ~3.0");
        assert!(
            approx_equal(hit.point.z, -3.0),
            "Hit point should be at z=-3"
        );
    }

    #[test]
    fn test_triangle_hit_vertex() {
        // Triangle in XY plane at z = -3
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        // Ray pointing directly at v0
        let ray = Ray::new(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, -1.0));

        let hit = tri.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_some(), "Ray should hit triangle vertex");
    }

    #[test]
    fn test_triangle_miss_outside() {
        // Triangle in XY plane at z = -3
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        // Ray pointing way outside the triangle
        let ray = Ray::new(Vector3::new(5.0, 5.0, 0.0), Vector3::new(0.0, 0.0, -1.0));

        let hit = tri.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_none(), "Ray should miss triangle");
    }

    #[test]
    fn test_triangle_miss_parallel() {
        // Triangle in XY plane at z = -3
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        // Ray parallel to triangle (pointing along X axis)
        let ray = Ray::new(Vector3::new(0.0, 0.5, -3.0), Vector3::new(1.0, 0.0, 0.0));

        let hit = tri.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_none(), "Parallel ray should miss triangle");
    }

    #[test]
    fn test_triangle_miss_behind() {
        // Triangle in XY plane at z = -3
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        // Ray pointing away from triangle
        let ray = Ray::new(Vector3::new(0.0, 0.3, 0.0), Vector3::new(0.0, 0.0, 1.0));

        let hit = tri.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_none(), "Ray pointing away should miss triangle");
    }

    #[test]
    fn test_triangle_geometric_normal() {
        // Triangle in XY plane, normal should point in +Z direction
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        );

        let normal = tri.geometric_normal();

        // Normal should point in +Z direction (perpendicular to XY plane)
        assert!(approx_equal(normal.x, 0.0), "Normal X should be 0");
        assert!(approx_equal(normal.y, 0.0), "Normal Y should be 0");
        assert!(approx_equal(normal.z.abs(), 1.0), "Normal Z should be ±1");
    }

    #[test]
    fn test_triangle_smooth_shading() {
        // Triangle with custom vertex normals (all pointing in +Z)
        let tri = Triangle::with_normals(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
            Vector3::new(0.0, 0.0, 1.0), // n0
            Vector3::new(0.0, 0.0, 1.0), // n1
            Vector3::new(0.0, 0.0, 1.0), // n2
        );

        let ray = Ray::new(Vector3::new(0.0, 0.3, 0.0), Vector3::new(0.0, 0.0, -1.0));

        let hit = tri.hit(&ray, 0.001, f32::MAX).expect("Should hit");

        // Interpolated normal should be close to +Z
        assert!(
            approx_equal(hit.normal.z.abs(), 1.0),
            "Interpolated normal should point in Z"
        );
    }

    #[test]
    fn test_triangle_edge_case_tiny() {
        // Very small triangle (but not degenerate)
        let tri = Triangle::new(
            Vector3::new(0.0, 0.0, -3.0),
            Vector3::new(0.01, 0.0, -3.0),
            Vector3::new(0.0, 0.01, -3.0),
        );

        let ray = Ray::new(
            Vector3::new(0.003, 0.003, 0.0),
            Vector3::new(0.0, 0.0, -1.0),
        );

        let hit = tri.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_some(), "Should hit tiny triangle");
    }

    #[test]
    fn test_triangle_t_range() {
        // Triangle at z = -3
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        let ray = Ray::new(Vector3::new(0.0, 0.3, 0.0), Vector3::new(0.0, 0.0, -1.0));

        // Hit should be rejected if t_max is too small
        let hit = tri.hit(&ray, 0.001, 2.0);
        assert!(
            hit.is_none(),
            "Should miss if t_max < intersection distance"
        );

        // Hit should be accepted if t_max is large enough
        let hit = tri.hit(&ray, 0.001, 10.0);
        assert!(hit.is_some(), "Should hit if t_max > intersection distance");
    }
}
