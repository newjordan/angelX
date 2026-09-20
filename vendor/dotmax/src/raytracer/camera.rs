//! Simple pinhole camera (VIZ-010)

use super::math::{Ray, Vector3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub origin: Vector3,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub focal_length: f32,
}

impl Camera {
    pub fn new(origin: Vector3, viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            origin,
            viewport_width,
            viewport_height,
            focal_length: 1.0,
        }
    }

    /// Generate ray for pixel coordinates (u, v) in [0.0, 1.0]
    pub fn get_ray(&self, u: f32, v: f32) -> Ray {
        // Lower-left corner of the viewport (centered around origin, in front at -f)
        let half_w = self.viewport_width * 0.5;
        let half_h = self.viewport_height * 0.5;
        let lower_left = self.origin + Vector3::new(-half_w, -half_h, -self.focal_length);
        let horizontal = Vector3::new(self.viewport_width, 0.0, 0.0);
        let vertical = Vector3::new(0.0, self.viewport_height, 0.0);
        let target = lower_left + horizontal * u + vertical * v;
        Ray::new(self.origin, target - self.origin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_ray_normalized() {
        let cam = Camera::new(Vector3::new(0.0, 0.0, 0.0), 4.0, 3.0);
        let r = cam.get_ray(0.5, 0.5);
        assert!((r.direction.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_camera_corners() {
        let cam = Camera::new(Vector3::new(0.0, 0.0, 0.0), 4.0, 3.0);
        let r_ll = cam.get_ray(0.0, 0.0);
        let r_ur = cam.get_ray(1.0, 1.0);
        assert!(r_ll.direction.x < 0.0 && r_ll.direction.y < 0.0);
        assert!(r_ur.direction.x > 0.0 && r_ur.direction.y > 0.0);
    }
}
