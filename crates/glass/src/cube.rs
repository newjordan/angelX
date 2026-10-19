//! The cube: a turning, floating cube with a different picture on each of
//! its six faces, the first scene `angel-gpu` draws in 3D
//! (`ANGEL_GLASS_DEMO=cube` shows it over the cockpit). Everything is a pure
//! function of the clock so it can be tested without a window.
//!
//! The faces are test cards, not art: a colour and a big number, upright
//! when seen from outside, so a mirrored or turned-over face is plain to
//! see. Face `i` carries the number `i + 1`, in the cube's face order (+X,
//! -X, +Y, -Y, +Z, -Z).

use angel_stage::scene::{
    Camera, Material, MeshRef, Node, Projection, Sampling, Scene, Transform,
};

use crate::layer::LAYER_PICTURES;

/// The picture the scene is drawn into, which a layer then places.
pub const SCENE_ID: u32 = LAYER_PICTURES + 0x70;
/// The six face pictures: this id and the five after it.
pub const FACE_ID: u32 = LAYER_PICTURES + 0x71;
/// A face picture's side, in pixels.
pub const FACE_SIDE: u32 = 128;
/// The scene's texture, in pixels: twice the size it is shown at, so the
/// edges that the layer shrinks it by are smoothed.
pub const TARGET: (u32, u32) = (768, 768);
/// The size the layer shows it at, before it is lifted.
pub const SHOWN: f32 = 384.0;

/// A face's colour: tonal, each one easy to tell from the rest.
const COLOURS: [[u8; 3]; 6] = [
    [178, 62, 58],  // 1  red
    [208, 132, 52], // 2  orange
    [74, 150, 84],  // 3  green
    [56, 148, 156], // 4  teal
    [66, 100, 188], // 5  blue
    [136, 82, 170], // 6  violet
];

/// The digits 1 to 6, five columns by seven rows, one string a row.
const DIGITS: [[&str; 7]; 6] = [
    ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
    [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
    [".###.", "#...#", "....#", "..##.", "....#", "#...#", ".###."],
    ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
    ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
    [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."],
];

/// Face `i`'s picture: its colour with a frame, a corner mark so a turned
/// picture reads, and its number (`i + 1`) large in the middle. Straight RGBA,
/// [`FACE_SIDE`] square.
pub fn face_picture(i: usize) -> Vec<u8> {
    let side = FACE_SIDE as usize;
    let [r, g, b] = COLOURS[i % 6];
    let digit = &DIGITS[i % 6];
    let mut rgba = Vec::with_capacity(side * side * 4);
    // The digit: each of its cells a 12 pixel block, centred.
    let block = 12usize;
    let (left, top) = ((side - 5 * block) / 2, (side - 7 * block) / 2);
    for y in 0..side {
        for x in 0..side {
            let edge = x.min(y).min(side - 1 - x).min(side - 1 - y);
            let mut px = if edge < 4 {
                // The frame: the colour, darker.
                [r / 2, g / 2, b / 2]
            } else {
                [r, g, b]
            };
            // A mark in the top left corner, so the picture's top reads.
            if (8..22).contains(&x) && (8..22).contains(&y) {
                px = [236, 232, 220];
            }
            let (dx, dy) = (x.wrapping_sub(left), y.wrapping_sub(top));
            if dx < 5 * block && dy < 7 * block && digit[dy / block].as_bytes()[dx / block] == b'#' {
                px = [236, 232, 220];
            }
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    rgba
}

/// The scene at `t` seconds: the cube turning and tilting, bobbing a little,
/// under a camera that leaves the rest of the picture clear (transparent), so
/// the layer that shows it floats the cube over whatever is behind.
pub fn scene(t: f32) -> Scene {
    let spin = Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], t * 0.9);
    let tilt = Transform::from_rotation_axis_angle([1.0, 0.0, 0.0], 0.45 + 0.5 * (t * 0.37).sin());
    let bob = Transform::from_translation([0.0, 0.08 * (t * 1.6).sin(), 0.0]);
    let mut cube = Node::new(
        1,
        MeshRef::Cube,
        Material { sampling: Sampling::Linear, ..Material::default() },
    );
    cube.transform = bob.then(&spin).then(&tilt);
    cube.face_materials = (0..6)
        .map(|i| Material {
            sampling: Sampling::Linear,
            ..Material::picture(FACE_ID + i)
        })
        .collect();
    Scene {
        camera: Camera {
            transform: Transform::from_translation([0.0, 0.0, 3.6]),
            projection: Projection::Perspective {
                fov_y: 38.0_f32.to_radians(),
                near: 0.1,
                far: 50.0,
            },
            clear: Some([0.0; 4]),
        },
        nodes: vec![cube],
        look: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_stage::scene::TextureSource;

    #[test]
    fn each_face_has_its_own_picture_in_the_cubes_face_order() {
        let s = scene(0.0);
        let cube = &s.nodes[0];
        let ids: Vec<u32> = cube
            .face_materials
            .iter()
            .map(|m| match m.source {
                TextureSource::Picture(id) => id,
                TextureSource::None => 0,
            })
            .collect();
        assert_eq!(ids, [0, 1, 2, 3, 4, 5].map(|i| FACE_ID + i));
        assert_eq!(s.draw_list().len(), 1);
    }

    #[test]
    fn the_face_pictures_are_square_opaque_and_all_different() {
        let faces: Vec<Vec<u8>> = (0..6).map(face_picture).collect();
        for face in &faces {
            assert_eq!(face.len(), (FACE_SIDE * FACE_SIDE * 4) as usize);
            assert!(face.chunks(4).all(|p| p[3] == 255));
        }
        for a in 0..6 {
            for b in a + 1..6 {
                assert_ne!(faces[a], faces[b], "faces {a} and {b}");
            }
        }
    }

    #[test]
    fn a_face_has_its_colour_a_corner_mark_and_a_number() {
        let face = face_picture(4);
        let at = |x: usize, y: usize| {
            let i = (y * FACE_SIDE as usize + x) * 4;
            [face[i], face[i + 1], face[i + 2]]
        };
        // Away from the mark and the number: the face's blue; the frame darker.
        assert_eq!(at(60, 14), COLOURS[4]);
        assert_eq!(at(1, 60), COLOURS[4].map(|c| c / 2));
        // The corner mark.
        assert_eq!(at(14, 14), [236, 232, 220]);
        // The 5's top bar: the digit's first row is all on.
        assert_eq!(at(64, 22 + 6), [236, 232, 220]);
        // Not in the bottom right corner.
        assert_eq!(at(110, 110), COLOURS[4]);
    }

    #[test]
    fn the_cube_turns_and_bobs_with_the_clock() {
        let (a, b) = (scene(0.0), scene(1.0));
        assert_ne!(a.nodes[0].transform, b.nodes[0].transform);
        // Nothing but the clock moves it.
        assert_eq!(scene(2.5), scene(2.5));
        // It stays inside the camera's sight whatever the time.
        for step in 0..200 {
            let s = scene(step as f32 * 0.37);
            let aspect = TARGET.0 as f32 / TARGET.1 as f32;
            assert!(s.camera.view_projection(aspect).is_some());
            let [x, y, z] = s.nodes[0].transform.translation;
            assert!(x.abs() < 0.2 && y.abs() < 0.2 && z.abs() < 0.2);
        }
    }
}
