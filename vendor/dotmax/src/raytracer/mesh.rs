//! Triangle mesh container for ray tracing (VIZ-016)
//!
//! This module provides a container for collections of triangles that form a mesh.
//! It implements the Hittable trait by testing rays against all triangles and
//! returning the closest intersection.
//!
//! Single Responsibility: Mesh container and management only.
//! Delegates intersection logic to Triangle (DRY principle).

use super::gltf_loader::MeshData;
use super::hittable::{HitRecord, Hittable};
use super::math::{Ray, Vector3};
use super::triangle::Triangle;

/// A collection of triangles forming a 3D mesh
///
/// This is a simple container that holds triangles and implements ray intersection
/// by testing against all triangles. Future optimizations could add spatial
/// acceleration structures (BVH, octree, etc.) for large meshes.
pub struct TriangleMesh {
    triangles: Vec<Triangle>,
    aabb_min: Vector3,
    aabb_max: Vector3,
}

impl TriangleMesh {
    /// Create an empty mesh
    pub fn new() -> Self {
        Self {
            triangles: Vec::new(),
            aabb_min: Vector3::new(0.0, 0.0, 0.0),
            aabb_max: Vector3::new(0.0, 0.0, 0.0),
        }
    }

    /// Create a mesh from raw triangle data
    pub fn from_triangles(triangles: Vec<Triangle>) -> Self {
        let (aabb_min, aabb_max) = Self::compute_aabb(&triangles);
        Self {
            triangles,
            aabb_min,
            aabb_max,
        }
    }

    /// Create a mesh from glTF-loaded data
    ///
    /// This constructor builds triangles from indexed vertex data.
    /// It handles both smooth shading (with vertex normals) and flat shading
    /// (without vertex normals).
    ///
    /// # Arguments
    /// * `data` - MeshData loaded from a glTF file
    ///
    /// # Returns
    /// A TriangleMesh containing all triangles from the mesh data
    pub fn from_gltf_data(data: MeshData) -> Self {
        let mut triangles = Vec::new();

        // Build triangles from indexed vertex data
        // Indices come in groups of 3 (one triangle per group)
        for chunk in data.indices.chunks(3) {
            if chunk.len() != 3 {
                // Skip incomplete triangles (shouldn't happen with valid data)
                continue;
            }

            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;

            // Bounds guard against malformed indices
            if i0 >= data.positions.len()
                || i1 >= data.positions.len()
                || i2 >= data.positions.len()
            {
                continue;
            }

            // Get vertex positions
            let v0 = data.positions[i0];
            let v1 = data.positions[i1];
            let v2 = data.positions[i2];

            // Create triangle with or without normals (guard length)
            let triangle = if let Some(ref normals) = data.normals {
                if normals.len() == data.positions.len() {
                    Triangle::with_normals(v0, v1, v2, normals[i0], normals[i1], normals[i2])
                } else {
                    Triangle::new(v0, v1, v2)
                }
            } else {
                // Flat shading: no vertex normals
                Triangle::new(v0, v1, v2)
            };

            triangles.push(triangle);
        }

        let (aabb_min, aabb_max) = Self::compute_aabb(&triangles);
        Self {
            triangles,
            aabb_min,
            aabb_max,
        }
    }

    /// Compute axis-aligned bounding box for a list of triangles
    fn compute_aabb(tris: &[Triangle]) -> (Vector3, Vector3) {
        if tris.is_empty() {
            return (Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0));
        }
        let mut min = Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for t in tris {
            let pts = [t.v0, t.v1, t.v2];
            for p in &pts {
                if p.x < min.x {
                    min.x = p.x;
                }
                if p.y < min.y {
                    min.y = p.y;
                }
                if p.z < min.z {
                    min.z = p.z;
                }
                if p.x > max.x {
                    max.x = p.x;
                }
                if p.y > max.y {
                    max.y = p.y;
                }
                if p.z > max.z {
                    max.z = p.z;
                }
            }
        }
        (min, max)
    }

    #[inline]
    fn ray_intersects_aabb(
        ray: &Ray,
        mut t_min: f32,
        mut t_max: f32,
        aabb_min: &Vector3,
        aabb_max: &Vector3,
    ) -> bool {
        // Slab method
        for i in 0..3 {
            let (origin, dir, min_b, max_b) = match i {
                0 => (ray.origin.x, ray.direction.x, aabb_min.x, aabb_max.x),
                1 => (ray.origin.y, ray.direction.y, aabb_min.y, aabb_max.y),
                _ => (ray.origin.z, ray.direction.z, aabb_min.z, aabb_max.z),
            };
            if dir.abs() < 1e-8 {
                // Ray parallel to slab; if origin outside, no hit
                if origin < min_b || origin > max_b {
                    return false;
                }
                continue;
            }
            let inv_d = 1.0 / dir;
            let mut t0 = (min_b - origin) * inv_d;
            let mut t1 = (max_b - origin) * inv_d;
            if inv_d < 0.0 {
                std::mem::swap(&mut t0, &mut t1);
            }
            t_min = t_min.max(t0);
            t_max = t_max.min(t1);
            if t_max < t_min {
                return false;
            }
        }
        true
    }

    /// Get the number of triangles in this mesh
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Add a triangle to the mesh
    pub fn add_triangle(&mut self, triangle: Triangle) {
        self.triangles.push(triangle);
    }

    /// Get an iterator over the triangles
    pub fn triangles(&self) -> impl Iterator<Item = &Triangle> {
        self.triangles.iter()
    }
}

impl Default for TriangleMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl Hittable for TriangleMesh {
    /// Test ray intersection against all triangles in the mesh
    ///
    /// This uses a simple linear search through all triangles, keeping track
    /// of the closest hit. For large meshes, this could be optimized with
    /// spatial acceleration structures (BVH, octree, etc.).
    ///
    /// The algorithm:
    /// 1. Start with t_max as the furthest we'll search
    /// 2. Test each triangle
    /// 3. If we hit, update closest_so_far to that hit distance
    /// 4. Continue testing remaining triangles with the new closer bound
    /// 5. Return the closest hit found (if any)
    fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        // Fast reject using mesh AABB
        if !Self::ray_intersects_aabb(ray, t_min, t_max, &self.aabb_min, &self.aabb_max) {
            return None;
        }

        let mut closest_so_far = t_max;
        let mut best_hit: Option<HitRecord> = None;

        // Test ray against all triangles, keeping the closest hit
        for triangle in &self.triangles {
            if let Some(hit) = triangle.hit(ray, t_min, closest_so_far) {
                // Found a closer hit - update our search bound
                closest_so_far = hit.t;
                best_hit = Some(hit);
            }
        }

        best_hit
    }
}

#[cfg(test)]
mod tests {
    use super::super::math::Vector3;
    use super::*;

    #[test]
    fn test_empty_mesh() {
        let mesh = TriangleMesh::new();
        assert_eq!(mesh.triangle_count(), 0);

        let ray = Ray::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, -1.0));
        let hit = mesh.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_none(), "Empty mesh should not hit anything");
    }

    #[test]
    fn test_single_triangle_mesh() {
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        let mesh = TriangleMesh::from_triangles(vec![tri]);
        assert_eq!(mesh.triangle_count(), 1);

        // Ray that hits the triangle
        let ray = Ray::new(Vector3::new(0.0, 0.3, 0.0), Vector3::new(0.0, 0.0, -1.0));
        let hit = mesh.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_some(), "Should hit single triangle");
    }

    #[test]
    fn test_multiple_triangles_closest_hit() {
        // Two triangles at different depths
        let tri1 = Triangle::new(
            Vector3::new(-1.0, 0.0, -5.0), // Far triangle
            Vector3::new(1.0, 0.0, -5.0),
            Vector3::new(0.0, 1.0, -5.0),
        );

        let tri2 = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0), // Near triangle
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        let mesh = TriangleMesh::from_triangles(vec![tri1, tri2]);
        assert_eq!(mesh.triangle_count(), 2);

        // Ray that hits both triangles
        let ray = Ray::new(Vector3::new(0.0, 0.3, 0.0), Vector3::new(0.0, 0.0, -1.0));
        let hit = mesh.hit(&ray, 0.001, f32::MAX).expect("Should hit");

        // Should return the closer triangle (at z = -3)
        assert!(
            (hit.point.z - (-3.0)).abs() < 1e-4,
            "Should hit closer triangle at z=-3, got z={}",
            hit.point.z
        );
    }

    #[test]
    fn test_from_gltf_data_flat_shading() {
        // Create simple mesh data (one triangle, no normals)
        let data = MeshData {
            positions: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
            ],
            normals: None,
            indices: vec![0, 1, 2],
        };

        let mesh = TriangleMesh::from_gltf_data(data);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn test_from_gltf_data_smooth_shading() {
        // Create mesh data with normals
        let data = MeshData {
            positions: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
            ],
            normals: Some(vec![
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, 1.0),
            ]),
            indices: vec![0, 1, 2],
        };

        let mesh = TriangleMesh::from_gltf_data(data);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn test_from_gltf_data_multiple_triangles() {
        // Create mesh data with two triangles (a quad split into two tris)
        let data = MeshData {
            positions: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(1.0, 1.0, 0.0),
            ],
            normals: None,
            indices: vec![
                0, 1, 2, // First triangle
                1, 3, 2, // Second triangle
            ],
        };

        let mesh = TriangleMesh::from_gltf_data(data);
        assert_eq!(mesh.triangle_count(), 2);
    }

    #[test]
    fn test_add_triangle() {
        let mut mesh = TriangleMesh::new();
        assert_eq!(mesh.triangle_count(), 0);

        let tri = Triangle::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        );

        mesh.add_triangle(tri);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn test_mesh_miss() {
        let tri = Triangle::new(
            Vector3::new(-1.0, 0.0, -3.0),
            Vector3::new(1.0, 0.0, -3.0),
            Vector3::new(0.0, 1.0, -3.0),
        );

        let mesh = TriangleMesh::from_triangles(vec![tri]);

        // Ray that misses the triangle
        let ray = Ray::new(Vector3::new(5.0, 5.0, 0.0), Vector3::new(0.0, 0.0, -1.0));
        let hit = mesh.hit(&ray, 0.001, f32::MAX);
        assert!(hit.is_none(), "Should miss triangle");
    }
}
