use super::super::mesh::mat;
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
    assert_eq!(window_tint(0.0, 0.0), [76, 55, 35]);
    assert_eq!(window_tint(0.02, 0.02), [76, 55, 35]);
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
        [218, 174, 112],
        "the bounded amber glow survives at its heart"
    );
    assert!(
        corner[0] >= 200,
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
fn stone_and_timber_have_soft_shoulders_and_fire_has_no_glazing_grid() {
    for material in [mat::STONE, mat::FLOOR, mat::WOOD, mat::ROOF, mat::PATH] {
        let mut last = material_sample(material, [0.0, 0.1]).0;
        for i in 1..96 {
            let next = material_sample(material, [i as f32 / 64.0, 0.1]).0;
            assert!(
                (0..3).all(|k| next[k].abs_diff(last[k]) <= 18),
                "material {material}: hard band {last:?} -> {next:?}"
            );
            last = next;
        }
    }
    let flame = material_sample(mat::FLAME, [0.0; 2]).0;
    let lead = material_sample(mat::WINDOW, [0.0; 2]).0;
    assert!(flame[0] > lead[0] + 70, "a flame has no dark glass mullion");
    for i in 0..30 {
        assert!(
            material_sample(mat::FLAME, [i as f32 * 0.1, 0.0])
                .0
                .iter()
                .all(|&c| c <= 228)
        );
    }
}

#[test]
fn fire_glass_keeps_cames_and_exposed_flame_keeps_a_bright_inner_tongue() {
    for uv in [[0.0, 0.0], [0.08, 0.10], [0.18, 0.21]] {
        assert_eq!(
            material_sample(mat::FIRE, uv).0,
            material_sample(mat::WINDOW, uv).0
        );
    }
    let lead = material_sample(mat::FIRE, [0.0, 0.0]).0;
    let pane = material_sample(mat::FIRE, [0.08, 0.10]).0;
    assert!(
        pane[1] >= lead[1] + 100,
        "lead cames must separate the warm panes"
    );
    let core = material_sample(mat::FLAME, [0.5, 0.2]).0;
    let edge = material_sample(mat::FLAME, [0.1, 0.2]).0;
    let tip = material_sample(mat::FLAME, [0.5, 0.9]).0;
    assert!(
        core[1] >= edge[1] + 45 && core[2] >= edge[2] + 50,
        "flat flame: {core:?} / {edge:?}"
    );
    assert!(
        core[1] >= tip[1] + 45,
        "the core must cool into the outer tongue"
    );
    let normal = v3(-1.0, 0.0, 0.0);
    let fire = Surface::lit(mat::FLAME, normal, normal, moon_direction());
    assert_ne!(
        fire.shade([0.5, 0.2], 3.0, Firelight::STEADY, FOG_DISTANCE),
        fire.shade(
            [0.5, 0.2],
            3.0,
            Firelight {
                gain: 0.8,
                warm: 0.05
            },
            FOG_DISTANCE
        )
    );
}

// ─── a scene's own painted surfaces are opt-in ──────────────────────────────

/// A wall of three painted-capable materials side by side, and a floor.
fn painted_wall() -> Mesh {
    let mut mesh = Mesh::new();
    for (k, material) in [mat::ROCK, mat::STONE, mat::BONE].into_iter().enumerate() {
        let y = -3.0 + k as f32 * 2.0;
        mesh.push_quad(
            v3(9.0, y, 0.0),
            v3(9.0, y + 2.0, 0.0),
            v3(9.0, y + 2.0, 4.0),
            v3(9.0, y, 4.0),
            material,
        );
    }
    mesh.push_quad(
        v3(2.0, -3.0, 0.0),
        v3(14.0, -3.0, 0.0),
        v3(14.0, 3.0, 0.0),
        v3(2.0, 3.0, 0.0),
        mat::PATH,
    );
    mesh
}

fn solid(rgb: [u8; 3]) -> Vec<[u8; 3]> {
    vec![rgb; Painted::SIDE * Painted::SIDE]
}

#[test]
fn a_scene_that_paints_nothing_draws_the_shared_table_exactly() {
    let mesh = painted_wall();
    let view = proving_view();
    let shared = render_scene(&mesh, &view, 96, 72);
    let (none, depth_none) = render_meshes_painted(
        &[&mesh],
        &view,
        96,
        72,
        Firelight::STEADY,
        FOG_DISTANCE,
        Backdrop::Night,
        None,
    );
    let (empty, depth_empty) = render_meshes_painted(
        &[&mesh],
        &view,
        96,
        72,
        Firelight::STEADY,
        FOG_DISTANCE,
        Backdrop::Night,
        Some(&Painted::new()),
    );
    let (plain, depth_plain) = render_meshes(
        &[&mesh],
        &view,
        96,
        72,
        Firelight::STEADY,
        FOG_DISTANCE,
        Backdrop::Night,
    );
    assert_eq!(shared.as_raw(), none.as_raw());
    assert_eq!(shared.as_raw(), empty.as_raw());
    assert_eq!(shared.as_raw(), plain.as_raw());
    assert_eq!(depth_none, depth_plain);
    assert_eq!(depth_empty, depth_plain);
}

#[test]
fn a_painted_material_changes_its_own_dots_and_no_others() {
    let mesh = painted_wall();
    let view = proving_view();
    let shared = render_scene(&mesh, &view, 96, 72);
    let mut painted = Painted::new();
    painted.set(mat::ROCK, solid([200, 20, 20]));
    painted.set(mat::PATH, solid([20, 200, 20]));
    let (frame, _) = render_meshes_painted(
        &[&mesh],
        &view,
        96,
        72,
        Firelight::STEADY,
        FOG_DISTANCE,
        Backdrop::Night,
        Some(&painted),
    );
    let seen = visible_materials(&mesh, &view, 96, 72);
    let (mut changed, mut kept) = (0, 0);
    for (i, material) in seen.iter().enumerate() {
        let (x, y) = ((i % 96) as u32, (i / 96) as u32);
        let same = pixel(&shared, x, y) == pixel(&frame, x, y);
        match *material {
            m if m == mat::ROCK || m == mat::PATH => {
                assert!(!same, "a painted dot ({x}, {y}) of material {m} is unchanged");
                changed += 1;
            }
            _ => {
                assert!(same, "dot ({x}, {y}) of material {material} changed");
                kept += 1;
            }
        }
    }
    assert!(changed > 200 && kept > 200, "{changed} painted, {kept} kept");
    // The painted rock is red, and still lit by the shared lighting: a wall in
    // shadow is darker than one in the key.
    let rock = seen.iter().position(|&m| m == mat::ROCK).expect("rock in view");
    let [r, g, b] = pixel(&frame, (rock % 96) as u32, (rock / 96) as u32);
    assert!(r > g && r > b, "painted rock is red: {r} {g} {b}");
}

#[test]
fn a_painted_surface_reads_a_coarser_copy_far_away() {
    let mut checker = Vec::new();
    for y in 0..Painted::SIDE {
        for x in 0..Painted::SIDE {
            checker.push(if (x + y) % 2 == 0 { [200, 200, 200] } else { [0, 0, 0] });
        }
    }
    let mut painted = Painted::new();
    painted.set(mat::STONE, checker);
    // Near, a texel is the table's own; far, it is the mean of its neighbours.
    assert_eq!(painted.sample(mat::STONE, 0, 0, 1.0), Some([200, 200, 200]));
    assert_eq!(painted.sample(mat::STONE, 1, 0, 1.0), Some([0, 0, 0]));
    assert_eq!(painted.sample(mat::STONE, 0, 0, 100.0), Some([100, 100, 100]));
    assert_eq!(painted.sample(mat::STONE, 255, 255, 100.0), Some([100, 100, 100]));
    // What it does not paint it does not answer for.
    assert_eq!(painted.sample(mat::ROCK, 0, 0, 1.0), None);
}

#[test]
#[should_panic(expected = "not a baked table")]
fn the_moonlight_is_not_a_table_a_scene_can_paint() {
    Painted::new().set(mat::MOONLIGHT, solid([1, 2, 3]));
}
