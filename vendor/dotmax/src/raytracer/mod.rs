//! CPU raytracer with braille/ASCII terminal output.
//!
//! Ported from the `crabmusic` project. Renders scenes (spheres, triangles,
//! meshes loaded from OBJ/glTF) to a 2D intensity buffer which can be
//! converted to coloured braille for the terminal.

#![allow(missing_docs)]

pub mod braille;
pub mod camera;
pub mod gltf_loader;
pub mod hittable;
pub mod lighting;
pub mod math;
pub mod mesh;
pub mod obj_loader;
pub mod renderer;
pub mod scene;
pub mod sphere;
pub mod triangle;
pub mod wireframe;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    /// Wireframe grid with configurable spacing and thickness (radians)
    Wireframe {
        step_rad: f32,
        tol_rad: f32,
    },
    Solid,
}

impl Default for RenderMode {
    fn default() -> Self {
        RenderMode::Wireframe {
            step_rad: wireframe::DEFAULT_WIREFRAME_STEP_RAD,
            tol_rad: wireframe::DEFAULT_WIREFRAME_TOL_RAD,
        }
    }
}

pub use braille::intensity_buffer_to_green_braille;
pub use camera::Camera;
pub use hittable::{HitRecord, Hittable};
pub use lighting::Light;
pub use math::{Ray, Vector3};
pub use mesh::TriangleMesh;
pub use renderer::{
    render, render_edges_with_orientation, render_with_orientation, WireframeRotation,
};
pub use scene::Scene;
pub use sphere::Sphere;
pub use triangle::Triangle;
pub use wireframe::{DEFAULT_WIREFRAME_STEP_RAD, DEFAULT_WIREFRAME_TOL_RAD};
