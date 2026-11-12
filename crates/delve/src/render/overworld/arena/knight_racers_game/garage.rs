//! The garage: one pod on a turntable under the race's own lens, for the
//! course and pod select screens.
use super::*;
use super::camera::{H, W};

/// A pod of rig `look` in seat `seat`'s colours, turned `turn` radians, as the
/// 384 x 224 picture the select screens sit over.
pub fn garage(look: usize, seat: u32, turn: f32, ticks: u32) -> Img { garage_at(look, seat, turn, ticks, 0.) }

/// The same with the pod `shift` metres across the picture (positive to the left), to leave room
/// for a plate on either side.
pub fn garage_at(look: usize, seat: u32, turn: f32, ticks: u32, shift: f32) -> Img {
    let mut floor = Mesh::new();
    let ring = |r: f32, i: usize| { let a = i as f32 / 28. * TAU; v3(r * a.cos(), r * a.sin(), 0.) };
    for i in 0..28 {
        floor.push_tri([v3(0., 0., 0.), ring(9., i), ring(9., i + 1)], [[0.; 2]; 3], mat::PATH);
        floor.push_quad(ring(8.4, i) + v3(0., 0., 0.03), ring(9.2, i) + v3(0., 0., 0.03), ring(9.2, i + 1) + v3(0., 0., 0.03), ring(8.4, i + 1) + v3(0., 0., 0.03), if i % 2 == 0 { STEEL } else { IRON });
    }
    let t = ticks as f32 / 30.;
    let mut pod = Mesh::new();
    pods::build(&mut pod, look, seat, t, 0.55 + 0.35 * (t * 2.).sin());
    // Turn about the pod's middle, not its engines'.
    let (s, c) = turn.sin_cos();
    let mid = v3(-3.3, 0., 0.);
    for tr in &mut pod.tris {
        for p in &mut tr.v {
            let q = *p - mid;
            *p = v3(q.x * c - q.y * s, q.x * s + q.y * c, q.z);
        }
        let n = tr.normal;
        tr.normal = v3(n.x * c - n.y * s, n.x * s + n.y * c, n.z);
    }
    // Along the lens's own right-hand side, so the shift is across the picture; a little closer
    // in than the example's first camera, so a pod fills its room.
    let right = v3(-(0.63f32).sin(), (0.63f32).cos(), 0.);
    let aim = v3(0., 0., 2.0) + right * shift;
    let eye = aim + (v3(-15., -11., 7.5) - v3(0., 0., 2.0)) * 0.8;
    let d = aim - eye;
    let view = View3 { pos: eye, heading_rad: d.y.atan2(d.x), pitch: d.z.atan2(d.x.hypot(d.y)), fov_rad: 0.95 };
    let (rgba, _) = raster::render_meshes_painted(&[&floor, &pod], &view, W as usize, H as usize, Firelight { gain: 1.65, warm: 0.06 }, 130., Backdrop::Dark([10, 12, 20]), Some(pods::palette()));
    let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
    let boxes = [(0, 0, 384, 224)];
    pods::recolour(&mut im, rgba.as_raw(), &boxes);
    im
}

/// Meshes shot through the pods' own painted palette, as the screens' own scenes (the podium,
/// the ghost) are: the picture and its depth.
pub fn shoot(meshes: &[&Mesh], view: &View3, fire: Firelight, fog: f32, backdrop: Backdrop) -> (image::RgbaImage, Vec<f32>) {
    raster::render_meshes_painted(meshes, view, W as usize, H as usize, fire, fog, backdrop, Some(pods::palette()))
}
