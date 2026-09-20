use super::super::mesh::mat;
use super::super::scene;
use super::*;

fn proving_view() -> View3 {
    View3 {
        pos: v3(0.0, 0.0, 0.62),
        heading_rad: 0.0,
        pitch: 0.0,
        fov_rad: 1.05,
    }
}

fn luma(pixel: [u8; 3]) -> f32 {
    0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32
}

fn pixel(frame: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let p = frame.get_pixel(x, y).0;
    [p[0], p[1], p[2]]
}

#[test]
fn renders_are_byte_identical_for_identical_inputs() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let view = proving_view();
    let first = render_scene(&scene, &view, 96, 72);
    let second = render_scene(&scene, &view, 96, 72);
    assert_eq!(
        first.as_raw(),
        second.as_raw(),
        "identical inputs must produce byte-identical frames"
    );
}

#[test]
fn sky_sits_above_the_ground_plane() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let frame = render_scene(&scene, &proving_view(), 96, 72);
    // The night sky is blue-dominant; lit ground (grass/path) is not.
    for row in [0u32, 3, 6] {
        let sky = pixel(&frame, 48, row);
        assert!(
            sky[2] > sky[1],
            "row {row} should be sky, got {sky:?} (needs blue over green)"
        );
    }
    for row in [66u32, 69, 71] {
        let ground = pixel(&frame, 48, row);
        assert!(
            ground[1] >= ground[2],
            "row {row} should be lit ground, got {ground:?}"
        );
    }
}

#[test]
fn the_z_buffer_hides_geometry_behind_a_wall() {
    // A near quad squarely in front of a far quad: the far one must never
    // reach the frame, whichever order the mesh lists them in.
    let far = |mesh: &mut Mesh| {
        mesh.push_quad(
            v3(9.0, -4.0, 0.0),
            v3(9.0, 4.0, 0.0),
            v3(9.0, 4.0, 6.0),
            v3(9.0, -4.0, 6.0),
            mat::WINDOW,
        );
    };
    let near = |mesh: &mut Mesh| {
        mesh.push_quad(
            v3(3.0, -4.0, 0.0),
            v3(3.0, 4.0, 0.0),
            v3(3.0, 4.0, 6.0),
            v3(3.0, -4.0, 6.0),
            mat::STONE_DARK,
        );
    };
    let mut far_first = Mesh::new();
    far(&mut far_first);
    near(&mut far_first);
    let mut near_first = Mesh::new();
    near(&mut near_first);
    far(&mut near_first);

    let view = proving_view();
    let a = render_scene(&far_first, &view, 64, 48);
    let b = render_scene(&near_first, &view, 64, 48);
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "depth order must not depend on list order"
    );
    // The emissive far quad would be unmistakable; the near stone is dim.
    let centre = pixel(&a, 32, 24);
    assert!(
        luma(centre) < 90.0,
        "the near wall must occlude the lit quad behind it, got {centre:?}"
    );
}

#[test]
fn near_plane_clipping_survives_a_camera_inside_the_geometry() {
    // Camera planted in the middle of a box: every face straddles or sits
    // behind the near plane. This must render, not panic or blank out.
    let mut mesh = Mesh::new();
    let (lo, hi) = (-2.0f32, 2.0f32);
    for &z in &[0.0f32, 3.0] {
        mesh.push_quad(
            v3(lo, lo, z),
            v3(hi, lo, z),
            v3(hi, hi, z),
            v3(lo, hi, z),
            mat::FLOOR,
        );
    }
    mesh.push_quad(
        v3(lo, lo, 0.0),
        v3(hi, lo, 0.0),
        v3(hi, lo, 3.0),
        v3(lo, lo, 3.0),
        mat::STONE,
    );
    mesh.push_quad(
        v3(lo, hi, 0.0),
        v3(hi, hi, 0.0),
        v3(hi, hi, 3.0),
        v3(lo, hi, 3.0),
        mat::STONE,
    );
    mesh.push_quad(
        v3(lo, lo, 0.0),
        v3(lo, hi, 0.0),
        v3(lo, hi, 3.0),
        v3(lo, lo, 3.0),
        mat::STONE,
    );
    mesh.push_quad(
        v3(hi, lo, 0.0),
        v3(hi, hi, 0.0),
        v3(hi, hi, 3.0),
        v3(hi, lo, 3.0),
        mat::STONE,
    );
    let view = View3 {
        pos: v3(0.0, 0.0, 1.5),
        heading_rad: 0.6,
        pitch: 0.0,
        fov_rad: 1.05,
    };
    let frame = render_scene(&mesh, &view, 64, 48);
    let interior_pixels = frame
        .pixels()
        .filter(|p| p.0[3] == 255 && luma([p.0[0], p.0[1], p.0[2]]) > 0.0)
        .count();
    assert_eq!(interior_pixels, 64 * 48, "every pixel must be painted");
    // Standing inside, the walls fill the frame — no sky punches through.
    let above = pixel(&frame, 32, 2);
    assert!(
        luma(above) > 45.0,
        "an enclosed camera must see lit wall, not night sky, got {above:?}"
    );
}

#[test]
fn the_moon_is_the_brightest_thing_in_the_sky() {
    let empty = Mesh::new();
    let moonward = View3 {
        pos: v3(0.0, 0.0, 0.62),
        heading_rad: MOON_BEARING * std::f32::consts::TAU,
        pitch: MOON_ELEVATION_RAD,
        fov_rad: 1.05,
    };
    let frame = render_scene(&empty, &moonward, 96, 72);
    let centre = luma(pixel(&frame, 48, 36));
    let corner = luma(pixel(&frame, 4, 4));
    assert!(centre > 170.0, "moon core should burn, got {centre:.1}");
    assert!(
        corner < 40.0,
        "the rest of the night stays dark, got {corner:.1}"
    );
}

#[test]
fn window_glow_is_brightest_at_the_pane_centre_and_keeps_its_lead() {
    // The mullion lattice stays dark — the lead owns the window's shape.
    assert_eq!(window_tint(0.0, 0.0), [96, 62, 34]);
    assert_eq!(window_tint(0.02, 0.02), [96, 62, 34]);
    // The glow concentrates at the pane centre and falls toward the lead —
    // the promised "one hot dot" reading, subtle enough to stay glass.
    let centre = window_tint(0.5 * 0.16, 0.5 * 0.20);
    let off_centre = window_tint(0.84 * 0.16, 0.5 * 0.20);
    let corner = window_tint(0.84 * 0.16, 0.90 * 0.20);
    assert!(centre[0] > off_centre[0], "centre outshines the pane edge");
    assert!(
        off_centre[0] >= corner[0],
        "the falloff is monotone toward the lead"
    );
    assert_eq!(
        centre,
        [255, 198, 116],
        "the authored glow survives at its heart"
    );
    assert!(
        corner[0] >= 230,
        "the falloff stays subtle — the pane never reads as dim: {corner:?}"
    );
    // Deterministic: same uv in, same glass out (cache-safe by construction).
    assert_eq!(centre, window_tint(0.5 * 0.16, 0.5 * 0.20));
}

#[test]
fn windows_outshine_the_stone_they_sit_in() {
    let mut mesh = Mesh::new();
    mesh.push_quad(
        v3(8.0, -6.0, 0.0),
        v3(8.0, 6.0, 0.0),
        v3(8.0, 6.0, 8.0),
        v3(8.0, -6.0, 8.0),
        mat::STONE,
    );
    mesh.push_quad(
        v3(7.98, -0.6, 1.4),
        v3(7.98, 0.6, 1.4),
        v3(7.98, 0.6, 2.6),
        v3(7.98, -0.6, 2.6),
        mat::WINDOW,
    );
    let frame = render_scene(&mesh, &proving_view(), 96, 72);
    let brightest = frame
        .pixels()
        .map(|p| luma([p.0[0], p.0[1], p.0[2]]))
        .fold(0.0f32, f32::max);
    assert!(
        brightest > 150.0,
        "an emissive window must survive the night grade, got {brightest:.1}"
    );
}

#[test]
fn distance_sinks_stone_into_the_night_fog() {
    let wall_luma = |x: f32| {
        let mut mesh = Mesh::new();
        mesh.push_quad(
            v3(x, -20.0, 0.0),
            v3(x, 20.0, 0.0),
            v3(x, 20.0, 14.0),
            v3(x, -20.0, 14.0),
            mat::STONE,
        );
        let frame = render_scene(&mesh, &proving_view(), 64, 48);
        luma(pixel(&frame, 32, 24))
    };
    let near = wall_luma(4.0);
    let far = wall_luma(30.0);
    assert!(
        near > far + 10.0,
        "night fog must sink distant stone: near={near:.1} far={far:.1}"
    );
}

#[test]
fn a_zero_sized_pane_renders_nothing_instead_of_panicking() {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let frame = render_scene(&scene, &proving_view(), 0, 0);
    assert_eq!(frame.width(), 0);
    assert_eq!(frame.height(), 0);
}
