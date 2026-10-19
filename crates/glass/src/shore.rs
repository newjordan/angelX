//! The shore: a night shoreline in the world's look, the first scene to ask for
//! one (`ANGEL_GLASS_DEMO=shore` fills the window with it under the cockpit's
//! text). It is a test of the look (docs/GLASS_CORE.md, "The world's look"),
//! not new art: every shape is a built-in mesh (plane, cube, sphere, quad) and
//! the one figure is a knight from the Delve's own committed sprites, a
//! billboard walking along the water's edge.
//!
//! The reference is Jason Strachan's dithered night shoreline
//! (x.com/jasonstrachan/status/2111047350609453291): water with a foam edge,
//! sand, whale-rib arches, a beached shape, a seawall, one warm accent (the
//! moon on the water), a slow camera, a tiny figure in a big quiet space. The
//! blue is this scene's own: the look is per scene.
//!
//! Everything is a pure function of the clock, so it can be tested without a
//! window. Brightness is what the look reads (it places each pixel on the ramp
//! by luminance), so surfaces here are greys of chosen luminance, and a box is
//! "lit" by a fixed brightness for each of its faces.

use angel_stage::look::{Dither, Fog, Look};
use angel_stage::scene::{
    AlphaMode, Camera, Material, MeshRef, Node, Projection, Sampling, Scene, TextureSource, Transform,
    quat_axis_angle, quat_mul, quat_normalize,
};

use crate::layer::LAYER_PICTURES;

/// The picture the scene is drawn into, which a layer then places.
pub const SCENE_ID: u32 = LAYER_PICTURES + 0x80;
/// The knight's sprite.
pub const KNIGHT_ID: u32 = LAYER_PICTURES + 0x81;

/// The reference's internal height, in pixels.
pub const ROWS: u32 = 540;

/// The measured ramp, dark to light: the reference's indigos, with its pale
/// highlight as the top step.
pub const RAMP: [[u8; 3]; 7] = [
    [0x1e, 0x24, 0x31],
    [0x2d, 0x35, 0x55],
    [0x44, 0x4e, 0x6f],
    [0x50, 0x59, 0x96],
    [0x5c, 0x66, 0x9c],
    [0x6c, 0x77, 0xae],
    [0x8a, 0x93, 0xc7],
];

/// The one warm accent: the moon's glints on the water.
pub const MOON: [u8; 3] = [0xf2, 0xe4, 0x8e];

/// The knight's sprite, committed with the Delve's art.
const KNIGHT_PNG: &[u8] = include_bytes!("../../../cockpit/assets/dungeon/art-v2/knight-run.png");

/// The scene's look at `rows` internal rows: the ramp, the moon, a 4x4 Bayer
/// dither, fog toward the dark end.
pub fn look(rows: u32) -> Look {
    Look {
        ramp: RAMP.to_vec(),
        accent: vec![MOON],
        dither: Dither::Bayer4,
        fog: Some(Fog { color: 1, start: 8.0, end: 100.0 }),
        pixel_rows: rows,
        ..Look::new(Vec::new())
    }
}

/// The knight's sprite as straight RGBA: `(width, height, pixels)`.
pub fn knight_sprite() -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(KNIGHT_PNG));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    buffer.truncate(info.buffer_size());
    Some((info.width, info.height, buffer))
}

// ---------------------------------------------------------------- helpers ----

/// A surface of luminance `l` (a grey: the look reads brightness only).
fn tone(l: f32) -> Material {
    Material::color([l, l, l, 1.0])
}

/// The brightness each face of a box has, in the cube's face order (+X, -X,
/// +Y, -Y, +Z, -Z): light from above, to the right and in front.
const FACE_LIGHT: [f32; 6] = [0.62, 0.50, 1.0, 0.30, 0.82, 0.40];

/// A box of luminance `l` on its lit face, each face lit by its own share.
fn lit_box(id: u64, parent: u64, at: [f32; 3], scale: [f32; 3], l: f32) -> Node {
    let mut node = Node::new(id, MeshRef::Cube, tone(l));
    node.parent = Some(parent);
    node.transform = Transform { translation: at, scale, ..Transform::IDENTITY };
    node.face_materials = FACE_LIGHT.iter().map(|k| tone(l * k)).collect();
    node
}

fn flat(id: u64, parent: u64, mesh: MeshRef, at: [f32; 3], scale: [f32; 3], l: f32) -> Node {
    let mut node = Node::new(id, mesh, tone(l));
    node.parent = Some(parent);
    node.transform = Transform { translation: at, scale, ..Transform::IDENTITY };
    node
}

/// A number in 0..1 that looks arbitrary and is the same every time.
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65536.0
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// A camera at `eye` looking at `target`, +Y up: its transform turns -Z
/// toward the target.
pub fn look_at(eye: [f32; 3], target: [f32; 3]) -> Transform {
    let f = normalize(sub(target, eye));
    let r = normalize(cross(f, [0.0, 1.0, 0.0]));
    let u = cross(r, f);
    // The turn whose columns are right, up and back (-forward).
    let b = [-f[0], -f[1], -f[2]];
    let (m00, m01, m02) = (r[0], u[0], b[0]);
    let (m10, m11, m12) = (r[1], u[1], b[1]);
    let (m20, m21, m22) = (r[2], u[2], b[2]);
    let trace = m00 + m11 + m22;
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s]
    };
    Transform { translation: eye, rotation: quat_normalize(q), scale: [1.0; 3] }
}

/// How far the shore's line is turned from straight across the view, in
/// radians: the water's edge climbs to the right.
const YAW: f32 = 0.58;

/// A shore-space point (x along the shore, y up, z toward the sea) in the
/// world.
fn world(p: [f32; 3]) -> [f32; 3] {
    let (s, c) = YAW.sin_cos();
    [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]
}

/// The slow camera: a drift of a few units a minute over the shore, never
/// still, never quick.
pub fn camera(t: f32) -> Camera {
    let eye = [
        -4.0 + 6.0 * (t * 0.045).sin(),
        15.5 + 1.6 * (t * 0.031 + 1.0).sin(),
        25.0 + 2.5 * (t * 0.027).cos(),
    ];
    let target = [-2.0 + 3.0 * (t * 0.038 + 2.0).sin(), 0.0, -6.0 + 1.5 * (t * 0.05).cos()];
    let [r, g, b] = [RAMP[1][0], RAMP[1][1], RAMP[1][2]].map(|c| f32::from(c) / 255.0);
    Camera {
        transform: look_at(eye, target),
        projection: Projection::Perspective { fov_y: 42.0_f32.to_radians(), near: 0.5, far: 220.0 },
        // The sky is the fog's own colour, so the far water has no seam.
        clear: Some([r, g, b, 1.0]),
    }
}

/// Where the knight is at `t`: along the water's edge in shore space, and
/// whether it is going toward +x (the sprite runs to the right).
fn knight_walk(t: f32) -> (f32, bool) {
    // A slow pace: 0.55 units a second over 24 units there and 24 back.
    let span = 24.0;
    let lap = (t * 0.55) % (2.0 * span);
    if lap < span { (-17.0 + lap, true) } else { (7.0 - (lap - span), false) }
}

/// The shore at `t` seconds, looked at through [`look`] at `rows` rows.
pub fn scene(t: f32, rows: u32) -> Scene {
    let mut nodes: Vec<Node> = Vec::with_capacity(320);
    let mut next = 1u64;
    let mut id = || {
        let n = next;
        next += 1;
        n
    };
    // The whole shore hangs from one turned place, so its line is diagonal.
    let root = id();
    let mut shore = Node::empty(root);
    shore.transform = Transform::from_rotation_axis_angle([0.0, 1.0, 0.0], YAW);
    nodes.push(shore);

    // The sea: bands of falling brightness out from the edge, the way a
    // shoal deepens, each a plane a hair above the one beyond it.
    for (z0, z1, l, y) in [(0.0, 3.2, 0.52, 0.045), (3.2, 10.0, 0.46, 0.03), (10.0, 24.0, 0.41, 0.015), (24.0, 340.0, 0.37, 0.0)] {
        nodes.push(flat(id(), root, MeshRef::Plane, [0.0, y, (z0 + z1) / 2.0], [900.0, 1.0, z1 - z0], l));
    }
    // The sand, darker than the water, with a wet band at the edge.
    nodes.push(flat(id(), root, MeshRef::Plane, [0.0, 0.2, -170.0], [900.0, 1.0, 340.0], 0.165));
    nodes.push(flat(id(), root, MeshRef::Plane, [0.0, 0.215, -1.6], [900.0, 1.0, 3.2], 0.235));
    // Foam: long thin strips on the edge, drifting slowly.
    for i in 0..7u32 {
        let h = [hash01(i * 3), hash01(i * 3 + 1), hash01(i * 3 + 2)];
        let len = 7.0 + 15.0 * h[0];
        let x = -34.0 + 11.0 * i as f32 + 3.0 * (t * 0.17 + i as f32 * 1.9).sin();
        let z = 0.5 + 2.3 * h[1] + 0.35 * (t * 0.31 + i as f32 * 2.3).sin();
        nodes.push(flat(id(), root, MeshRef::Plane, [x, 0.06, z], [len, 1.0, 0.28 + 0.4 * h[2]], 0.62));
    }

    // The seawall: a run of boxes, with dark pillars standing over it.
    for i in -12i32..=12 {
        let h = 2.6 + 0.9 * hash01((i + 100) as u32);
        nodes.push(lit_box(id(), root, [i as f32 * 6.0, 0.2 + h / 2.0, -38.0], [6.1, h, 3.2], 0.37));
    }
    for i in -3..=3 {
        let x = i as f32 * 19.0 + 6.0;
        nodes.push(lit_box(id(), root, [x, 0.2 + 4.8, -40.5], [1.7, 9.6, 1.7], 0.15));
    }

    // The ribs: arches of boxes, leaning, some broken, standing on the sand.
    for i in 0..9u32 {
        let h = [hash01(i * 7 + 200), hash01(i * 7 + 201), hash01(i * 7 + 202), hash01(i * 7 + 203)];
        let x = -27.0 + 4.6 * i as f32 + 1.8 * (h[0] - 0.5);
        let z = -10.0 + 5.0 * (h[1] - 0.5);
        let height = 7.0 + 4.5 * h[2];
        let span = 3.6 + 2.2 * h[3];
        let group = id();
        let mut place = Node::empty(group);
        place.parent = Some(root);
        place.transform = Transform {
            translation: [x, 0.2, z],
            rotation: quat_normalize(quat_mul(
                quat_axis_angle([0.0, 1.0, 0.0], 0.5 * (h[0] - 0.5)),
                quat_axis_angle([0.0, 0.0, 1.0], 0.22 * (h[1] - 0.5)),
            )),
            scale: [1.0; 3],
        };
        nodes.push(place);
        let segments = 9;
        // A broken arch stops short.
        let used = if h[3] > 0.7 { 6 } else { segments };
        let point = |k: usize| {
            let a = std::f32::consts::PI * k as f32 / segments as f32;
            [-a.cos() * span / 2.0, a.sin() * height]
        };
        for k in 0..used {
            let (p, q) = (point(k), point(k + 1));
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            let len = (dx * dx + dy * dy).sqrt();
            let a = dy.atan2(dx);
            let thick = 0.55 + 0.55 * (std::f32::consts::PI * (k as f32 + 0.5) / segments as f32).sin();
            let mut seg = lit_box(
                id(),
                group,
                [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, 0.0],
                [len + 0.06, thick, thick],
                0.53 + 0.07 * (std::f32::consts::PI * k as f32 / segments as f32).sin(),
            );
            seg.transform.rotation = quat_axis_angle([0.0, 0.0, 1.0], a);
            nodes.push(seg);
        }
    }
    // The spine: a row of vertebrae shrinking away along the sand.
    for k in 0..14u32 {
        let s = 1.5 - 0.07 * k as f32;
        nodes.push(lit_box(
            id(),
            root,
            [8.0 + 2.4 * k as f32, 0.2 + s * 0.4, -13.0 - 0.4 * (k as f32 * 0.7).sin()],
            [s, s * 0.8, s * 0.9],
            0.50,
        ));
    }
    // The beached shape: a long low mound, a lighter shoulder, a dark shadow
    // round its foot.
    nodes.push(flat(id(), root, MeshRef::Sphere { segments: 32 }, [-14.0, 0.24, -4.6], [14.5, 0.07, 6.6], 0.10));
    nodes.push(flat(id(), root, MeshRef::Sphere { segments: 32 }, [-14.0, 0.7, -5.0], [12.5, 3.6, 5.8], 0.43));
    nodes.push(flat(id(), root, MeshRef::Sphere { segments: 32 }, [-16.2, 1.9, -5.8], [7.2, 2.0, 3.2], 0.56));

    // The moon on the water: accent strips, each coming and going.
    let moon = Material { accent: true, ..tone(0.9) };
    for i in 0..72u32 {
        let h = [hash01(i * 5 + 300), hash01(i * 5 + 301), hash01(i * 5 + 302), hash01(i * 5 + 303)];
        let along = (i as f32 + h[0]) / 72.0;
        // A path out from the shore that widens.
        let z = 1.6 + 17.0 * along * along.sqrt();
        let x = -10.0 + 7.0 * along + (h[1] - 0.5) * (4.0 + 12.0 * along);
        let wave = (t * (0.5 + 0.9 * h[2]) + h[3] * std::f32::consts::TAU).sin();
        if wave <= -0.1 {
            continue;
        }
        let size = 0.3 + (0.3 + 1.2 * h[1]) * (0.35 + 0.65 * (0.5 + 0.5 * wave)) * (0.55 + along);
        let mut node = Node::new(id(), MeshRef::Plane, moon);
        node.parent = Some(root);
        node.transform = Transform {
            translation: [x + 0.7 * (t * 0.3 + h[3] * 9.0).sin(), 0.09, z],
            scale: [size * 1.2, 1.0, 0.16 + 0.16 * along],
            ..Transform::IDENTITY
        };
        nodes.push(node);
    }

    // The knight, a billboard: upright, turned to face the camera, standing at
    // the water's edge and walking along it. Placed in the world, not the
    // shore's turned place, so it can face the camera however the shore lies.
    let (x, rightward) = knight_walk(t);
    let step = (t * 5.2).sin().abs();
    let foot = world([x, 0.25 + 0.08 * step, 0.9]);
    let eye = camera(t).transform.translation;
    let face = (eye[0] - foot[0]).atan2(eye[2] - foot[2]);
    let height = 2.7;
    let (sheet_w, sheet_h) = knight_sprite().map_or((39, 44), |(w, h, _)| (w, h));
    let width = height * sheet_w as f32 / sheet_h as f32;
    let sprite = Material {
        source: TextureSource::Picture(KNIGHT_ID),
        tint: [0.8, 0.8, 0.84, 1.0],
        sampling: Sampling::Nearest,
        alpha: AlphaMode::Blend,
        double_sided: true,
        lit: false,
        emissive: 0.0,
        accent: false,
        projector: None,
        patch: None,
        role: Default::default(),
        ceiling: 0,
    };
    let mut knight = Node::new(id(), MeshRef::Quad, sprite);
    knight.transform = Transform {
        translation: [foot[0], foot[1] + height / 2.0, foot[2]],
        // Turned to the camera; going the other way, shown from behind, which
        // is the sprite mirrored.
        rotation: quat_axis_angle([0.0, 1.0, 0.0], face + if rightward { 0.0 } else { std::f32::consts::PI }),
        scale: [width, height, 1.0],
    };
    nodes.push(knight);
    // Its shadow on the sand.
    let shadow = world([x, 0.0, 0.9]);
    let mut dark = flat(id(), root, MeshRef::Sphere { segments: 16 }, [0.0; 3], [1.5, 0.03, 0.55], 0.09);
    dark.parent = None;
    dark.transform.translation = [shadow[0], 0.27, shadow[2]];
    dark.transform.rotation = quat_axis_angle([0.0, 1.0, 0.0], YAW);
    nodes.push(dark);

    Scene { camera: camera(t), nodes, look: Some(look(rows)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_stage::scene::quat_rotate;

    #[test]
    fn the_look_is_the_measured_one_and_is_valid() {
        let l = look(ROWS);
        assert_eq!(l.validate(), Ok(()));
        assert_eq!(l.ramp.len(), 7);
        assert_eq!(l.accent, vec![MOON]);
        assert_eq!((l.dither, l.pixel_rows), (Dither::Bayer4, 540));
        // Fog toward the dark end, over a long reach.
        let fog = l.fog.expect("fog");
        assert!(fog.color <= 1 && fog.end > fog.start);
        // The ramp rises in brightness (the quantizer holds it so anyway).
        let levels = l.ramp_levels();
        assert!(levels.windows(2).all(|w| w[0] < w[1]), "{levels:?}");
    }

    #[test]
    fn look_at_turns_minus_z_toward_the_target() {
        for (eye, target) in [
            ([0.0, 0.0, 5.0], [0.0, 0.0, 0.0]),
            ([4.0, 10.0, 27.0], [-1.0, 0.5, -7.0]),
            ([-3.0, 2.0, -8.0], [5.0, 1.0, 6.0]),
            ([0.0, 20.0, 0.1], [0.0, 0.0, 0.0]),
        ] {
            let t = look_at(eye, target);
            let forward = quat_rotate(t.rotation, [0.0, 0.0, -1.0]);
            let want = normalize(sub(target, eye));
            for k in 0..3 {
                assert!((forward[k] - want[k]).abs() < 1e-4, "{eye:?} -> {target:?}: {forward:?} vs {want:?}");
            }
            // The camera stays level: its right is horizontal.
            let right = quat_rotate(t.rotation, [1.0, 0.0, 0.0]);
            assert!(right[1].abs() < 1e-4);
            assert_eq!(t.translation, eye);
        }
    }

    #[test]
    fn the_knights_sprite_is_the_committed_one() {
        let (w, h, rgba) = knight_sprite().expect("decodes");
        assert_eq!((w, h), (39, 44));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        // Transparent round the figure, opaque in it.
        assert_eq!(rgba[3], 0);
        assert!(rgba.chunks(4).filter(|p| p[3] == 255).count() > 400);
        assert!(rgba.chunks(4).all(|p| p[3] == 0 || p[3] == 255), "a cutout, no soft edges");
    }

    #[test]
    fn the_scene_is_valid_whole_and_all_in_sight() {
        for step in 0..60 {
            let t = step as f32 * 7.3;
            let s = scene(t, ROWS);
            let aspect = 16.0 / 9.0;
            assert!(s.camera.view_projection(aspect).is_some(), "t {t}");
            assert!(s.look.as_ref().is_some_and(|l| l.validate().is_ok()));
            let placed = s.world_matrices();
            assert!(placed.iter().all(Option::is_some), "every node can be placed at t {t}");
            // Ids are taken once each.
            let mut ids: Vec<u64> = s.nodes.iter().map(|n| n.id).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), s.nodes.len());
            assert!(s.nodes.len() < 400, "{} nodes", s.nodes.len());
        }
    }

    #[test]
    fn it_is_only_built_in_meshes_and_the_one_sprite() {
        let s = scene(3.0, ROWS);
        for node in &s.nodes {
            if let Some(mesh) = node.mesh {
                assert!(!matches!(mesh, MeshRef::Uploaded(_)), "node {} uses an uploaded mesh", node.id);
            }
            for m in std::iter::once(&node.material).chain(&node.face_materials) {
                assert!(matches!(m.source, TextureSource::None | TextureSource::Picture(KNIGHT_ID)));
            }
        }
        let sprites = s.nodes.iter().filter(|n| n.material.source == TextureSource::Picture(KNIGHT_ID)).count();
        assert_eq!(sprites, 1);
    }

    #[test]
    fn the_moon_is_the_only_accent_and_it_shimmers() {
        let count = |t: f32| {
            let s = scene(t, ROWS);
            for node in &s.nodes {
                if node.material.accent {
                    assert_eq!(node.mesh, Some(MeshRef::Plane));
                    assert_eq!(node.material.alpha, AlphaMode::Opaque);
                }
            }
            s.nodes.iter().filter(|n| n.material.accent).count()
        };
        let counts: Vec<usize> = (0..12).map(|i| count(i as f32 * 1.3)).collect();
        println!("glints shown over time: {counts:?}");
        assert!(counts.iter().all(|&c| (10..=72).contains(&c)), "{counts:?}");
        assert!(counts.iter().any(|&c| c != counts[0]), "they come and go: {counts:?}");
    }

    #[test]
    fn the_knight_walks_slowly_along_the_edge_and_turns_round() {
        let mut direction_changes = 0;
        let mut last = knight_walk(0.0);
        let mut most = f32::MIN;
        let mut least = f32::MAX;
        for step in 1..4000 {
            let t = step as f32 * 0.05;
            let now = knight_walk(t);
            // Never quicker than a walk: under a unit a second.
            if now.1 == last.1 {
                assert!((now.0 - last.0).abs() / 0.05 < 1.0);
            } else {
                direction_changes += 1;
            }
            most = most.max(now.0);
            least = least.min(now.0);
            last = now;
        }
        println!("the knight walks from {least:.1} to {most:.1} and turns {direction_changes} times in 200 s");
        assert!(least >= -17.1 && most <= 7.1);
        assert!(direction_changes >= 3);
    }

    #[test]
    fn nothing_moves_but_the_clock() {
        assert_eq!(scene(12.5, ROWS), scene(12.5, ROWS));
        assert_ne!(scene(12.5, ROWS).camera, scene(40.0, ROWS).camera);
    }

    /// The shore drawn by the core on a real GPU (skipped with no adapter):
    /// every pixel is a ramp colour or the moon, the moon shows, and the
    /// knight's sprite is in it. `ANGEL_SHORE_PNG=dir` also saves frames.
    #[test]
    fn it_draws_on_a_real_gpu_in_ramp_colours_and_the_moon() {
        use std::collections::HashSet;
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        })) else {
            println!("no GPU adapter: skipped");
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("shore-test"),
            required_limits: adapter.limits(),
            ..Default::default()
        })) else {
            println!("no GPU device: skipped");
            return;
        };
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let size = (960u32, 540u32);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shore-test-target"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut core = angel_gpu::Core::new(device.clone(), queue, format, 1.0);
        let (w, h, rgba) = knight_sprite().expect("sprite");
        core.upload(angel_gpu::Picture { id: KNIGHT_ID, width: w, height: h, rgba, cols: 0, rows: 0 });
        let ramp: HashSet<[u8; 3]> = RAMP.iter().copied().chain([MOON]).collect();
        for (n, t) in [0.0f32, 40.0, 95.0].into_iter().enumerate() {
            core.begin_frame();
            core.draw_scene(&view, size, &scene(t, ROWS), "shore-test");
            let pixels = core.read_back(&texture, size);
            let colours: HashSet<[u8; 3]> = pixels.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
            let off_palette = colours.iter().filter(|c| !ramp.contains(*c)).count();
            let moon = pixels.chunks(4).filter(|p| [p[0], p[1], p[2]] == MOON).count();
            let opaque = pixels.chunks(4).all(|p| p[3] == 255);
            println!("t {t}: {} distinct colours, {off_palette} off the palette, {moon} moon pixels, opaque {opaque}", colours.len());
            assert_eq!(off_palette, 0);
            assert!(opaque);
            assert!(moon > 30, "the moon shows: {moon}");
            assert!(colours.len() >= 6, "the ramp is used: {}", colours.len());
            if let Some(dir) = std::env::var_os("ANGEL_SHORE_PNG") {
                let dir = std::path::PathBuf::from(dir);
                let _ = std::fs::create_dir_all(&dir);
                let file = std::fs::File::create(dir.join(format!("shore-{n}.png"))).expect("file");
                let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                enc.write_header().expect("header").write_image_data(&pixels).expect("data");
            }
        }
    }
}
