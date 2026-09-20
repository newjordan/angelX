//! Wireframe grid detection on a sphere surface (VIZ-011)

use super::math::Vector3;

/// Default wireframe grid spacing (radians) and tolerance (thickness in radians)
pub const DEFAULT_WIREFRAME_STEP_RAD: f32 = 10.0_f32.to_radians();
pub const DEFAULT_WIREFRAME_TOL_RAD: f32 = 0.03; // ~1.7 degrees

/// Returns true if the surface normal lies on a wireframe grid line.
/// Uses only the normal, so it works regardless of sphere center/radius.
pub fn is_on_wireframe_normal(normal: Vector3, step_rad: f32, tol_rad: f32) -> bool {
    // Normal should already be unit length, but normalize defensively
    let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z)
        .sqrt()
        .max(1e-6);
    let x = normal.x / len;
    let y = normal.y / len;
    let z = normal.z / len;

    // Spherical coordinates from normal
    let theta = z.atan2(x); // longitude [-pi, pi]
    let phi = y.clamp(-1.0, 1.0).acos(); // polar angle [0, pi] from +Y

    // Distance to nearest multiple of step (in radians)
    let d_theta = {
        let m = theta.rem_euclid(step_rad);
        m.min(step_rad - m)
    };
    let d_phi = {
        let m = phi.rem_euclid(step_rad);
        m.min(step_rad - m)
    };

    d_theta < tol_rad || d_phi < tol_rad
}

/// Rotate a vector by yaw (Y), pitch (X), then roll (Z)
pub fn rotate_vec_yaw_pitch_roll(v: Vector3, yaw: f32, pitch: f32, roll: f32) -> Vector3 {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let (sr, cr) = roll.sin_cos();
    // Yaw around Y
    let x1 = cy * v.x + sy * v.z;
    let y1 = v.y;
    let z1 = -sy * v.x + cy * v.z;
    // Pitch around X
    let x2 = x1;
    let y2 = cp * y1 - sp * z1;
    let z2 = sp * y1 + cp * z1;
    // Roll around Z
    let x3 = cr * x2 - sr * y2;
    let y3 = sr * x2 + cr * y2;
    let z3 = z2;
    Vector3::new(x3, y3, z3)
}

/// Backward-compat: rotate a normal by yaw (Y axis) then pitch (X axis)
pub fn rotate_normal_yaw_pitch(normal: Vector3, yaw: f32, pitch: f32) -> Vector3 {
    rotate_vec_yaw_pitch_roll(normal, yaw, pitch, 0.0)
}

/// Wireframe test with an additional orientation (rotation) applied before grid check
pub fn is_on_wireframe_normal_rotated(
    normal: Vector3,
    step_rad: f32,
    tol_rad: f32,
    yaw: f32,
    pitch: f32,
) -> bool {
    let r = rotate_normal_yaw_pitch(normal, yaw, pitch);
    is_on_wireframe_normal(r, step_rad, tol_rad)
}

/// Backward-compatible helper: check wireframe using point/center/radius with default params.
pub fn is_on_wireframe(point: Vector3, center: Vector3, radius: f32) -> bool {
    let p = point - center;
    let r = radius.max(1e-6);
    let normal = Vector3::new(p.x / r, p.y / r, p.z / r);
    is_on_wireframe_normal(
        normal,
        DEFAULT_WIREFRAME_STEP_RAD,
        DEFAULT_WIREFRAME_TOL_RAD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wireframe_equator_and_meridian() {
        let c = Vector3::new(0.0, 0.0, 0.0);
        let r = 1.0;
        // Equator (phi ~ pi/2) should be on a latitude line (depending on step).
        let on_equator = is_on_wireframe(Vector3::new(1.0, 0.0, 0.0), c, r);
        // Prime meridian (theta ~ 0)
        let on_meridian = is_on_wireframe(Vector3::new(0.0, 1.0, 0.0), c, r);
        assert!(on_equator || on_meridian);
    }
}
