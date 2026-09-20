//! Simple Lambert diffuse lighting (VIZ-011)

use super::math::Vector3;

#[derive(Debug, Clone, Copy)]
pub struct Light {
    pub position: Vector3,
    pub intensity: f32, // 0..1
}

impl Light {
    pub fn new(position: Vector3, intensity: f32) -> Self {
        Self {
            position,
            intensity,
        }
    }
}

pub fn calculate_diffuse_shading(point: Vector3, normal: Vector3, light: &Light) -> f32 {
    let l = (light.position - point).normalize();
    let n = normal.normalize();
    let ndotl = n.dot(&l).max(0.0);
    (ndotl * light.intensity).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diffuse_front_lit() {
        let light = Light::new(Vector3::new(0.0, 0.0, 1.0), 1.0);
        let val = calculate_diffuse_shading(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            &light,
        );
        assert!(val > 0.0);
    }

    #[test]
    fn test_diffuse_back_lit() {
        let light = Light::new(Vector3::new(0.0, 0.0, -1.0), 1.0);
        let val = calculate_diffuse_shading(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            &light,
        );
        assert_eq!(val, 0.0);
    }

    #[test]
    fn test_diffuse_scaled_intensity() {
        let light = Light::new(Vector3::new(0.0, 0.0, 1.0), 0.5);
        let val = calculate_diffuse_shading(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            &light,
        );
        assert!(val > 0.0 && val <= 0.5);
    }
}
