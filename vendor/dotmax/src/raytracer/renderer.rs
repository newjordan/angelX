//! Rendering pipeline producing a 2D intensity buffer (VIZ-010)

use super::camera::Camera;
use super::lighting::calculate_diffuse_shading;
use super::scene::Scene;
use super::wireframe::{is_on_wireframe_normal_rotated, rotate_vec_yaw_pitch_roll};
use super::RenderMode;

#[derive(Debug, Clone, Copy)]
pub struct WireframeRotation {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32, // reserved for future use
}
impl Default for WireframeRotation {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        }
    }
}

pub fn render_with_orientation(
    scene: &Scene,
    camera: &Camera,
    width: usize,
    height: usize,
    mode: RenderMode,
    orientation: WireframeRotation,
) -> Vec<Vec<f32>> {
    let mut buffer = vec![vec![0.0_f32; width]; height];

    for y in 0..height {
        for x in 0..width {
            let u = if width > 1 {
                x as f32 / (width as f32 - 1.0)
            } else {
                0.0
            };
            let v = if height > 1 {
                y as f32 / (height as f32 - 1.0)
            } else {
                0.0
            };
            let ray = camera.get_ray(u, v);

            let intensity = if let Some(hit) = scene.hit(&ray, 0.001, f32::MAX) {
                match mode {
                    RenderMode::Wireframe { step_rad, tol_rad } => {
                        // Apply optional orientation to the normal before grid test
                        if is_on_wireframe_normal_rotated(
                            hit.normal,
                            step_rad,
                            tol_rad,
                            orientation.yaw,
                            orientation.pitch,
                        ) {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    RenderMode::Solid => {
                        let mut sum = 0.0;
                        for light in scene.lights() {
                            sum += calculate_diffuse_shading(hit.point, hit.normal, light);
                        }
                        sum.clamp(0.0, 1.0)
                    }
                }
            } else {
                0.0
            };

            buffer[y][x] = intensity;
        }
    }

    buffer
}

/// Simple edge/vertex renderer for mesh scenes with hidden-line removal.
pub fn render_edges_with_orientation(
    scene: &Scene,
    camera: &Camera,
    width: usize,
    height: usize,
    yaw: f32,
    pitch: f32,
    roll: f32,
    model_scale: f32,
    vertex_px: i32,
    line_px: i32,
) -> Vec<Vec<f32>> {
    use super::math::Vector3;
    // rotate vector with yaw/pitch/roll
    // use direct import rotate_vec_yaw_pitch_roll

    let mut buffer = vec![vec![0.0_f32; width]; height];
    let mut depth = vec![vec![f32::INFINITY; width]; height];

    let Some(verts) = scene.mesh_vertices() else {
        return buffer;
    };
    let Some(edges) = scene.mesh_edges() else {
        return buffer;
    };

    let half_w = camera.viewport_width * 0.5;
    let half_h = camera.viewport_height * 0.5;
    let w_max = (width as i32) - 1;
    let h_max = (height as i32) - 1;

    let project = |p: Vector3| -> Option<(i32, i32, f32)> {
        // Apply scale and rotation around origin
        let ps = Vector3::new(p.x * model_scale, p.y * model_scale, p.z * model_scale);
        let pr = rotate_vec_yaw_pitch_roll(ps, yaw, pitch, roll);
        // Transform to camera space (camera looks down -Z)
        let q = Vector3::new(
            pr.x - camera.origin.x,
            pr.y - camera.origin.y,
            pr.z - camera.origin.z,
        );
        if q.z >= -1e-4 {
            return None;
        }
        // Intersect with plane z = -f
        let t = -camera.focal_length / q.z;
        let x_plane = q.x * t;
        let y_plane = q.y * t;
        // Map plane coords to [0,1] using viewport size
        let u = (x_plane + half_w) / camera.viewport_width;
        let v = (y_plane + half_h) / camera.viewport_height;
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return None;
        }
        let px = (u * (width as f32 - 1.0)).round() as i32;
        let py = (v * (height as f32 - 1.0)).round() as i32;
        // Positive depth = distance from camera
        let d = -q.z;
        Some((px.clamp(0, w_max), py.clamp(0, h_max), d))
    };

    let mut set_px_depth = |x: i32, y: i32, val: f32, d: f32| {
        if x >= 0 && x <= w_max && y >= 0 && y <= h_max {
            let ux = x as usize;
            let uy = y as usize;
            if d < depth[uy][ux] {
                depth[uy][ux] = d;
                buffer[uy][ux] = val;
            } else if (d - depth[uy][ux]).abs() < 1e-4 {
                // same depth: keep brighter
                if val > buffer[uy][ux] {
                    buffer[uy][ux] = val;
                }
            }
        }
    };

    let draw_disc = |cx: i32, cy: i32, r: i32, d: f32, set: &mut dyn FnMut(i32, i32, f32, f32)| {
        let rr = r.max(0);
        for dy in -rr..=rr {
            for dx in -rr..=rr {
                if dx * dx + dy * dy <= rr * rr {
                    set(cx + dx, cy + dy, 1.0, d);
                }
            }
        }
    };

    let mut draw_line = |x0: i32, y0: i32, d0: f32, x1: i32, y1: i32, d1: f32, thick: i32| {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let steps = dx.abs().max(dy.abs());
        if steps == 0 {
            draw_disc(x0, y0, thick, d0, &mut set_px_depth);
            return;
        }
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let x = x0 as f32 + dx as f32 * t;
            let y = y0 as f32 + dy as f32 * t;
            let d = d0 + (d1 - d0) * t;
            draw_disc(
                x.round() as i32,
                y.round() as i32,
                thick,
                d,
                &mut set_px_depth,
            );
        }
    };

    // Draw edges with hidden-line removal via depth buffer
    let line_r = (line_px.max(1) / 2).max(0);
    for (a, b) in edges.iter().copied() {
        if let (Some((x0, y0, d0)), Some((x1, y1, d1))) = (project(a), project(b)) {
            draw_line(x0, y0, d0, x1, y1, d1, line_r);
        }
    }

    // Draw vertices on top (will only appear if nearest at that pixel)
    let vert_r = (vertex_px.max(1) / 2).max(0);
    for &p in verts.iter() {
        if let Some((x, y, d)) = project(p) {
            draw_disc(x, y, vert_r, d, &mut set_px_depth);
        }
    }

    buffer
}

pub fn render(
    scene: &Scene,
    camera: &Camera,
    width: usize,
    height: usize,
    mode: RenderMode,
) -> Vec<Vec<f32>> {
    render_with_orientation(
        scene,
        camera,
        width,
        height,
        mode,
        WireframeRotation::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raytracer::math::Vector3;
    use crate::raytracer::{DEFAULT_WIREFRAME_STEP_RAD, DEFAULT_WIREFRAME_TOL_RAD};

    #[test]
    fn test_render_dimensions() {
        let scene = Scene::new_with_sphere();
        let cam = Camera::new(Vector3::new(0.0, 0.0, 0.0), 4.0, 3.0);
        let buffer = render(
            &scene,
            &cam,
            40,
            30,
            RenderMode::Wireframe {
                step_rad: DEFAULT_WIREFRAME_STEP_RAD,
                tol_rad: DEFAULT_WIREFRAME_TOL_RAD,
            },
        );
        assert_eq!(buffer.len(), 30);
        assert_eq!(buffer[0].len(), 40);
    }

    #[test]
    fn test_render_single_sphere_center_hit() {
        let scene = Scene::new_with_sphere_and_light();
        let cam = Camera::new(Vector3::new(0.0, 0.0, 0.0), 4.0, 3.0);
        let w = 40_usize;
        let h = 30_usize;
        let buffer = render(&scene, &cam, w, h, RenderMode::Solid);
        let center = buffer[h / 2][w / 2];
        assert!(center > 0.0, "Center should hit the lit sphere");
        // corners should be mostly background
        assert!(
            buffer[0][0] < 0.5
                && buffer[0][w - 1] < 0.5
                && buffer[h - 1][0] < 0.5
                && buffer[h - 1][w - 1] < 0.5
        );
    }
}
