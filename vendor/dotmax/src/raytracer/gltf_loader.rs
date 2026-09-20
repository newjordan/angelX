//! glTF/GLB model loading (VIZ-016)
//!
//! This module handles loading 3D models from glTF 2.0 files and extracting
//! raw mesh data (vertices, normals, indices) for use by the ray tracer.
//!
//! Single Responsibility: File I/O and data extraction only.
//! Does NOT create geometry objects - that's the job of mesh.rs.

use super::math::Vector3;
use anyhow::{Context, Result};

/// Raw mesh data extracted from a glTF file
///
/// This is a simple data container that separates loading from geometry construction.
/// The mesh module will consume this data to build Triangle objects.
#[derive(Debug, Clone)]
pub struct MeshData {
    /// Vertex positions in 3D space
    pub positions: Vec<Vector3>,
    /// Vertex normals (optional - if None, use flat shading)
    pub normals: Option<Vec<Vector3>>,
    /// Triangle indices (groups of 3, referencing positions/normals)
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Get the number of triangles in this mesh
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Get the number of vertices in this mesh
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Check if this mesh has vertex normals
    pub fn has_normals(&self) -> bool {
        self.normals.is_some()
    }

    /// Calculate the bounding box of the mesh
    /// Returns (min, max) corners of the axis-aligned bounding box
    pub fn bounds(&self) -> (Vector3, Vector3) {
        if self.positions.is_empty() {
            return (Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0));
        }

        let mut min = self.positions[0];
        let mut max = self.positions[0];

        for pos in &self.positions {
            min.x = min.x.min(pos.x);
            min.y = min.y.min(pos.y);
            min.z = min.z.min(pos.z);
            max.x = max.x.max(pos.x);
            max.y = max.y.max(pos.y);
            max.z = max.z.max(pos.z);
        }

        (min, max)
    }

    /// Get the center point of the mesh's bounding box
    pub fn center(&self) -> Vector3 {
        let (min, max) = self.bounds();
        Vector3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (min.z + max.z) * 0.5,
        )
    }

    /// Get the size (dimensions) of the mesh's bounding box
    pub fn size(&self) -> Vector3 {
        let (min, max) = self.bounds();
        Vector3::new(max.x - min.x, max.y - min.y, max.z - min.z)
    }

    /// Get the maximum dimension of the bounding box
    pub fn max_dimension(&self) -> f32 {
        let size = self.size();
        size.x.max(size.y).max(size.z)
    }

    /// Normalize the mesh to fit within a unit cube centered at origin
    /// Returns a new MeshData with transformed positions
    pub fn normalize(&self) -> Self {
        let center = self.center();
        let max_dim = self.max_dimension();

        if max_dim == 0.0 {
            return self.clone();
        }

        let scale = 2.0 / max_dim; // Scale to fit in [-1, 1] range

        let positions: Vec<Vector3> = self
            .positions
            .iter()
            .map(|p| {
                Vector3::new(
                    (p.x - center.x) * scale,
                    (p.y - center.y) * scale,
                    (p.z - center.z) * scale,
                )
            })
            .collect();

        MeshData {
            positions,
            normals: self.normals.clone(),
            indices: self.indices.clone(),
        }
    }
}

/// Load mesh data from a glTF or GLB file
///
/// This function:
/// 1. Opens and parses the glTF file
/// 2. Extracts the first mesh's first primitive
/// 3. Reads vertex positions (required)
/// 4. Reads vertex normals (optional)
/// 5. Reads triangle indices (required)
///
/// # Arguments
/// * `path` - Path to .gltf or .glb file
///
/// # Returns
/// * `Ok(MeshData)` - Successfully loaded mesh data
/// * `Err(...)` - File not found, invalid format, or missing required data
///
/// # Example
/// ```ignore
/// use dotmax::raytracer::gltf_loader::load_gltf;
///
/// let mesh_data = load_gltf("models/cube.glb").expect("Failed to load model");
/// println!("Loaded {} triangles", mesh_data.triangle_count());
/// ```
pub fn load_gltf(path: &str) -> Result<MeshData> {
    // Import the glTF file (handles both .gltf and .glb)
    let (document, buffers, _images) =
        gltf::import(path).with_context(|| format!("Failed to load glTF file: {}", path))?;

    // Get the first mesh (simplification for MVP - can extend to support multiple meshes)
    let mesh = document
        .meshes()
        .next()
        .context("No meshes found in glTF file")?;

    // Get the first primitive (a mesh can have multiple primitives with different materials)
    let primitive = mesh
        .primitives()
        .next()
        .context("No primitives found in mesh")?;

    // Create a reader to access the binary data
    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

    // Extract vertex positions (REQUIRED)
    let positions: Vec<Vector3> = reader
        .read_positions()
        .context("No position data in primitive - positions are required")?
        .map(|p| Vector3::new(p[0], p[1], p[2]))
        .collect();

    // Validate we got some positions
    if positions.is_empty() {
        anyhow::bail!("Mesh has no vertices");
    }

    // Extract vertex normals (OPTIONAL - for smooth shading)
    let normals: Option<Vec<Vector3>> = reader
        .read_normals()
        .map(|iter| iter.map(|n| Vector3::new(n[0], n[1], n[2])).collect());

    // Validate normals match positions if present
    if let Some(ref normals_vec) = normals {
        if normals_vec.len() != positions.len() {
            anyhow::bail!(
                "Normal count ({}) doesn't match position count ({})",
                normals_vec.len(),
                positions.len()
            );
        }
    }

    // Extract triangle indices (REQUIRED for indexed geometry)
    let indices: Vec<u32> = reader
        .read_indices()
        .context("No index data in primitive - indexed geometry required")?
        .into_u32()
        .collect();

    // Validate indices
    if indices.is_empty() {
        anyhow::bail!("Mesh has no indices");
    }

    if indices.len() % 3 != 0 {
        anyhow::bail!(
            "Index count ({}) is not a multiple of 3 - invalid triangle data",
            indices.len()
        );
    }

    // Validate all indices are in bounds
    let max_index = *indices.iter().max().unwrap();
    if max_index >= positions.len() as u32 {
        anyhow::bail!(
            "Index out of bounds: {} >= {} vertices",
            max_index,
            positions.len()
        );
    }

    Ok(MeshData {
        positions,
        normals,
        indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_data_triangle_count() {
        let data = MeshData {
            positions: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
            ],
            normals: None,
            indices: vec![0, 1, 2],
        };

        assert_eq!(data.triangle_count(), 1);
        assert_eq!(data.vertex_count(), 3);
        assert!(!data.has_normals());
    }

    #[test]
    fn test_mesh_data_with_normals() {
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

        assert!(data.has_normals());
    }

    #[test]
    fn test_mesh_data_multiple_triangles() {
        let data = MeshData {
            positions: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(1.0, 1.0, 0.0),
            ],
            normals: None,
            indices: vec![0, 1, 2, 1, 3, 2], // Two triangles
        };

        assert_eq!(data.triangle_count(), 2);
        assert_eq!(data.vertex_count(), 4);
    }

    // Note: Testing actual glTF file loading requires test assets
    // Integration tests with real files will be in tests/ directory
}
