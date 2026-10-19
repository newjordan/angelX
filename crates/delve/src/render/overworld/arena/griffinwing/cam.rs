//! The camera, done by hand: the rasteriser's own view turns the world by
//! the platform's `sin` and `cos`, and those need not agree to the last place
//! between the browser's wasm and the host (a frame's hash would then differ
//! at the odd tick). So the stage moves the world instead: every vertex is
//! taken to the eye's frame here with `rules::griffinwing::trig`, and the
//! rasteriser is handed a camera at the origin looking straight down +x with a
//! level pitch (its trig of nothing is exact) and a fixed field of view.
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Mesh;
use crate::render::world3d::raster::View3;
use crate::rules::griffinwing::trig::Det;

/// The field of view every frame uses.
pub const FOV: f32 = 1.12;

/// An eye at `eye` looking along `heading` (yaw from +x, counter-clockwise)
/// and `pitch` (up is positive), as the world sees it.
#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub eye: V3,
    pub heading: f32,
    pub pitch: f32,
    sh: f32,
    ch: f32,
    sp: f32,
    cp: f32,
}

impl Cam {
    pub fn new(eye: V3, heading: f32, pitch: f32) -> Cam {
        let (sh, ch) = heading.dsin_cos();
        let (sp, cp) = pitch.dsin_cos();
        Cam {
            eye,
            heading,
            pitch,
            sh,
            ch,
            sp,
            cp,
        }
    }

    /// A direction (a surface's normal) in the eye's frame.
    pub fn dir(&self, d: V3) -> V3 {
        let x1 = d.x * self.ch + d.y * self.sh;
        let y1 = -d.x * self.sh + d.y * self.ch;
        v3(
            x1 * self.cp + d.z * self.sp,
            y1,
            -x1 * self.sp + d.z * self.cp,
        )
    }

    /// A world point in the eye's frame.
    pub fn point(&self, p: V3) -> V3 {
        self.dir(p - self.eye)
    }

    /// The view the rasteriser is given: the eye's frame, looking down +x.
    pub fn view(&self) -> View3 {
        View3 {
            pos: V3::ZERO,
            heading_rad: 0.0,
            pitch: 0.0,
            fov_rad: FOV,
        }
    }

    /// The view the sky is painted from: where the eye really looks.
    pub fn sky_view(&self) -> View3 {
        View3 {
            pos: self.eye,
            heading_rad: self.heading,
            pitch: self.pitch,
            fov_rad: FOV,
        }
    }

    /// `src` in the eye's frame, appended to `dst`.
    pub fn stage(&self, dst: &mut Mesh, src: &Mesh) {
        dst.tris.reserve(src.tris.len());
        for t in &src.tris {
            let mut o = *t;
            for k in 0..3 {
                o.v[k] = self.point(t.v[k]);
            }
            o.normal = self.dir(t.normal);
            dst.tris.push(o);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_straight_ahead_of_the_eye_lands_on_the_x_axis() {
        for (h, p) in [(0.0f32, 0.0f32), (0.7, -0.2), (2.9, 0.4), (-1.3, 0.1)] {
            let eye = v3(3.0, -2.0, 1.5);
            let cam = Cam::new(eye, h, p);
            let (sh, ch) = h.dsin_cos();
            let (sp, cp) = p.dsin_cos();
            let ahead = eye + v3(ch * cp, sh * cp, sp) * 10.0;
            let q = cam.point(ahead);
            assert!(
                (q.x - 10.0).abs() < 1e-4 && q.y.abs() < 1e-4 && q.z.abs() < 1e-4,
                "{q:?}"
            );
            // Up stays up and to the left stays left of the eye's line.
            let left = eye + v3(-sh, ch, 0.0) * 2.0;
            assert!((cam.point(left).y - 2.0).abs() < 1e-4);
        }
    }
}
