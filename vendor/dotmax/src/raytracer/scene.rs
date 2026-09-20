//! Scene container and intersection (VIZ-010)

use super::gltf_loader::{load_gltf, MeshData};
use super::hittable::{HitRecord, Hittable};
use super::lighting::Light;
use super::math::{Ray, Vector3};
use super::mesh::TriangleMesh;
use super::obj_loader::load_obj;
use super::sphere::Sphere;
use anyhow::Result;

pub struct Scene {
    pub(crate) objects: Vec<Box<dyn Hittable + Send + Sync>>, // allow future parallelism
    pub(crate) lights: Vec<Light>,
    // Optional cached geometry for simple edge/vertex rendering (OBJ path)
    pub(crate) mesh_vertices: Option<Vec<Vector3>>,
    pub(crate) mesh_edges: Option<Vec<(Vector3, Vector3)>>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            lights: Vec::new(),
            mesh_vertices: None,
            mesh_edges: None,
        }
    }

    pub fn new_with_sphere() -> Self {
        let sphere = Box::new(Sphere::new(Vector3::new(0.0, 0.0, -3.0), 1.0));
        Self {
            objects: vec![sphere],
            lights: Vec::new(),
            mesh_vertices: None,
            mesh_edges: None,
        }
    }

    pub fn new_with_sphere_and_light() -> Self {
        let sphere = Box::new(Sphere::new(Vector3::new(0.0, 0.0, -3.0), 1.0));
        let light = Light::new(Vector3::new(-2.0, 2.0, 0.0), 1.0);
        Self {
            objects: vec![sphere],
            lights: vec![light],
            mesh_vertices: None,
            mesh_edges: None,
        }
    }

    /// Create a scene with a glTF model loaded from file
    ///
    /// This constructor:
    /// 1. Loads mesh data from the glTF/GLB file
    /// 2. Builds a TriangleMesh from the data
    /// 3. Adds a default light to illuminate the model
    ///
    /// # Arguments
    /// * `path` - Path to .gltf or .glb file
    ///
    /// # Returns
    /// * `Ok(Scene)` - Scene with loaded model and light
    /// * `Err(...)` - Failed to load model
    ///
    /// # Example
    /// ```no_run
    /// use dotmax::raytracer::Scene;
    ///
    /// let scene = Scene::new_with_model("models/cube.glb").expect("Failed to load model");
    /// ```
    pub fn new_with_model(path: &str) -> Result<Self> {
        // Load mesh data from glTF file
        let mesh_data = load_gltf(path)?;
        Self::from_mesh_data(mesh_data)
    }

    /// Create a scene with an OBJ model loaded from file (simple importer)
    pub fn new_with_obj_model(path: &str) -> Result<Self> {
        let data = load_obj(path)?;
        Self::from_mesh_data(data)
    }

    /// Common builder from MeshData: normalize, build mesh, add default light
    fn from_mesh_data(mesh_data: MeshData) -> Result<Self> {
        // Normalize the mesh to fit in a standard view (centered at origin, scaled to ~2 units)
        let normalized_data = mesh_data.normalize();

        // Build triangle mesh from normalized data
        let mesh = Box::new(TriangleMesh::from_gltf_data(normalized_data.clone()));

        // Cache vertices and edges for simple edge/vertex rendering
        use std::collections::HashSet;
        let positions = normalized_data.positions.clone();
        let mut edge_set: HashSet<(u32, u32)> = HashSet::new();
        for tri in normalized_data.indices.chunks(3) {
            if tri.len() != 3 {
                continue;
            }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
                continue;
            }
            let e01 = if tri[0] < tri[1] {
                (tri[0], tri[1])
            } else {
                (tri[1], tri[0])
            };
            let e12 = if tri[1] < tri[2] {
                (tri[1], tri[2])
            } else {
                (tri[2], tri[1])
            };
            let e20 = if tri[2] < tri[0] {
                (tri[2], tri[0])
            } else {
                (tri[0], tri[2])
            };
            edge_set.insert(e01);
            edge_set.insert(e12);
            edge_set.insert(e20);
        }
        let mut edges: Vec<(Vector3, Vector3)> = Vec::with_capacity(edge_set.len());
        for (a, b) in edge_set {
            let ia = a as usize;
            let ib = b as usize;
            if ia < positions.len() && ib < positions.len() {
                edges.push((positions[ia], positions[ib]));
            }
        }

        // Add a default light to illuminate the model
        // Position it above and to the side for good visibility
        let light = Light::new(Vector3::new(2.0, 2.0, 2.0), 1.0);

        Ok(Self {
            objects: vec![mesh],
            lights: vec![light],
            mesh_vertices: Some(positions),
            mesh_edges: Some(edges),
        })
    }

    /// If the scene was created from a mesh, returns cached vertex positions
    pub fn mesh_vertices(&self) -> Option<&[Vector3]> {
        self.mesh_vertices.as_deref()
    }

    /// If the scene was created from a mesh, returns cached unique edges
    pub fn mesh_edges(&self) -> Option<&[(Vector3, Vector3)]> {
        self.mesh_edges.as_deref()
    }

    pub fn add_object(&mut self, obj: Box<dyn Hittable + Send + Sync>) {
        self.objects.push(obj);
    }
    pub fn add_light(&mut self, light: Light) {
        self.lights.push(light);
    }

    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let mut closest_so_far = t_max;
        let mut best: Option<HitRecord> = None;
        for obj in &self.objects {
            if let Some(hr) = obj.hit(ray, t_min, closest_so_far) {
                closest_so_far = hr.t;
                best = Some(hr);
            }
        }
        best
    }

    pub fn lights(&self) -> &[Light] {
        &self.lights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_single_sphere() {
        let scene = Scene::new_with_sphere();
        assert_eq!(scene.objects.len(), 1);
    }

    #[test]
    fn test_scene_hit_and_miss() {
        let scene = Scene::new_with_sphere();
        let cam_origin = Vector3::new(0.0, 0.0, 0.0);
        let hit_ray = Ray::new(cam_origin, Vector3::new(0.0, 0.0, -1.0));
        assert!(scene.hit(&hit_ray, 0.001, f32::MAX).is_some());
        let miss_ray = Ray::new(cam_origin, Vector3::new(1.0, 0.0, 0.0));
        assert!(scene.hit(&miss_ray, 0.001, f32::MAX).is_none());
    }
}
