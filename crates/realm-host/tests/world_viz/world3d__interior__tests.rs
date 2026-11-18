use super::super::super::raycast::RayView;
use super::*;

fn interior_view(yaw_offset: f32, pitch: f32) -> RayView {
    RayView {
        x: 4.5,
        y: 7.25,
        heading_rad: BASE_HEADING + yaw_offset,
        look_yaw: 0.0,
        fov_rad: 1.05,
        bob: pitch / 0.03,
        eye_h: 0.0,
    }
}

#[test]
fn every_room_is_built_once_shared_and_inside_its_triangle_budget() {
    for index in 0u8..8 {
        let first = super::super::scene::interior_scene(index);
        let second = super::super::scene::interior_scene(index);
        assert!(
            std::sync::Arc::ptr_eq(&first, &second),
            "{}: the room must be cached, not rebuilt per frame",
            hero(index)
        );
        assert!(
            first.tris.len() >= 120,
            "{}: {} triangles is an empty box",
            hero(index),
            first.tris.len()
        );
        assert!(
            first.tris.len() <= 700,
            "{}: {} triangles blew the interior budget",
            hero(index),
            first.tris.len()
        );
    }
}

#[test]
fn every_shaft_falls_from_a_real_bay() {
    for (index, chamber) in CHAMBERS.iter().enumerate() {
        for &shaft in chamber.shafts {
            assert!(
                chamber.bays.iter().any(|&bay| (bay - shaft).abs() < 1e-3),
                "{index}: {} drops a shaft at x = {shaft} with no bay above it (bays {:?})",
                chamber.hero,
                chamber.bays
            );
        }
    }
}

#[test]
fn rooms_are_finite_bounded_and_deterministic() {
    for index in 0u8..8 {
        let first = interior_mesh(index);
        let second = interior_mesh(index);
        assert_eq!(first.tris.len(), second.tris.len(), "{}", hero(index));
        for (left, right) in first.tris.iter().zip(second.tris.iter()) {
            assert_eq!(left.mat, right.mat, "{}", hero(index));
            for corner in 0..3 {
                assert_eq!(left.v[corner], right.v[corner], "{}", hero(index));
                assert_eq!(left.uv[corner], right.uv[corner], "{}", hero(index));
            }
        }
        for tri in &first.tris {
            for point in tri.v {
                assert!(
                    point.x.is_finite() && point.y.is_finite() && point.z.is_finite(),
                    "{}: non-finite vertex {point:?}",
                    hero(index)
                );
                assert!(
                    point.x.abs() <= 32.0 && point.y.abs() <= 32.0 && point.z <= 12.0,
                    "{}: geometry escaped the room at {point:?}",
                    hero(index)
                );
            }
            assert!(
                tri.normal.length() > 0.9,
                "{}: degenerate normal {:?}",
                hero(index),
                tri.normal
            );
        }
    }
}

#[test]
fn every_room_carries_a_floor_walls_and_a_light_source() {
    for index in 0u8..8 {
        let scene = super::super::scene::interior_scene(index);
        let mut seen = [false; mat::FLAME as usize + 1];
        for tri in &scene.tris {
            if (tri.mat as usize) < seen.len() {
                seen[tri.mat as usize] = true;
            }
        }
        assert!(seen[mat::FLOOR as usize], "{}: no floor", hero(index));
        assert!(seen[mat::WINDOW as usize], "{}: no light", hero(index));
        assert!(
            seen[mat::FIRE as usize] || seen[mat::FLAME as usize],
            "{}: nothing in this room burns",
            hero(index)
        );
        assert!(seen[mat::WOOD as usize], "{}: no timber", hero(index));
        assert!(
            seen[mat::STONE as usize] || seen[mat::STONE_DARK as usize],
            "{}: no masonry",
            hero(index)
        );
    }
}

#[test]
fn the_staged_eye_stands_off_the_walls_and_frames_its_hero() {
    for &(dot_w, dot_h) in &[(96usize, 72usize), (200, 304)] {
        for index in 0u8..8 {
            let chamber = &CHAMBERS[index as usize];
            let view = interior_view(0.0, 0.0);
            let camera = staged_view(index, &view, dot_w, dot_h);

            let clearance = standoff(index, &view, dot_w, dot_h);
            assert!(
                clearance >= 2.5,
                "{} @{dot_w}x{dot_h}: the frame centre lands {clearance:.2} tiles out",
                hero(index)
            );

            let anchor = rotate((chamber.anchor.0, chamber.anchor.1), chamber.yaw);
            let tan_h = (camera.fov_rad * 0.5).tan();
            let tan_v = tan_h * dot_h as f32 / dot_w as f32;
            let bearing = (anchor.1 - camera.pos.y).atan2(anchor.0 - camera.pos.x);
            let at_x = 0.5 * (1.0 + wrap_pi(bearing - camera.heading_rad).tan() / tan_h);
            assert!(
                (at_x - chamber.frame_x).abs() < 0.02,
                "{} @{dot_w}x{dot_h}: hero at {at_x:.3}, want {:.3}",
                hero(index),
                chamber.frame_x
            );
            let reach =
                ((anchor.0 - camera.pos.x).powi(2) + (anchor.1 - camera.pos.y).powi(2)).sqrt();
            let elevation = ((chamber.anchor.2 - camera.pos.z) / reach).atan();
            let at_y = 0.5 * (1.0 - (elevation - camera.pitch).tan() / tan_v);
            assert!(
                (at_y - chamber.frame_y).abs() < 0.02,
                "{} @{dot_w}x{dot_h}: hero at {at_y:.3} down, want {:.3}",
                hero(index),
                chamber.frame_y
            );
            assert!(
                at_x > 0.08 && at_x < 0.92 && at_y > 0.08 && at_y < 0.92,
                "{}: the hero fell off the frame",
                hero(index)
            );
        }
    }
}

#[test]
fn the_table_never_centres_its_hero_and_never_stands_in_a_wall() {
    for (index, chamber) in CHAMBERS.iter().enumerate() {
        assert!(
            (chamber.frame_x - 0.5).abs() >= 0.04,
            "{index}: {} centres its hero",
            chamber.hero
        );
        assert!(
            chamber.eye.0 >= -chamber.hx + 1.6 && chamber.eye.0 <= chamber.hx - 1.6,
            "{index}: {} stands in an end wall",
            chamber.hero
        );
        assert!(
            chamber.eye.1.abs() <= chamber.hy - 1.5,
            "{index}: {} stands in a flank wall",
            chamber.hero
        );
        let reach = ((chamber.anchor.0 - chamber.eye.0).powi(2)
            + (chamber.anchor.1 - chamber.eye.1).powi(2))
        .sqrt();
        assert!(
            reach >= 3.0,
            "{index}: {} is a close-up at {reach:.1} tiles",
            chamber.hero
        );
    }
}

#[test]
fn operator_yaw_and_pitch_pan_the_interior_camera() {
    for index in 0u8..8 {
        let level = staged_view(index, &interior_view(0.0, 0.0), 96, 72);
        let turned = staged_view(index, &interior_view(0.5, 0.0), 96, 72);
        let taken = wrap_pi(turned.heading_rad - level.heading_rad);
        assert!(
            (0.22..0.32).contains(&taken),
            "{}: a pan of 0.5 took {taken:.3} (SWING_GAIN = {SWING_GAIN})",
            hero(index)
        );
        let further = staged_view(index, &interior_view(0.9, 0.0), 96, 72);
        assert!(
            wrap_pi(further.heading_rad - turned.heading_rad) > 0.12,
            "{}: the pan saturated",
            hero(index)
        );
        assert!(
            (turned.pos.x - level.pos.x).abs() + (turned.pos.y - level.pos.y).abs() > 0.05,
            "{}: no parallax on the pan",
            hero(index)
        );
        let raised = staged_view(index, &interior_view(0.0, 0.24), 96, 72);
        assert!(
            (raised.pitch - level.pitch - 0.24).abs() < 1e-3,
            "{}: operator pitch did not ride in",
            hero(index)
        );
    }
}

#[test]
fn the_settled_sway_never_swings_the_hero_off_the_frame() {
    for &(dot_w, dot_h) in &[(96usize, 72usize), (114, 136), (200, 304)] {
        for index in 0u8..8 {
            let chamber = &CHAMBERS[index as usize];
            let anchor = rotate((chamber.anchor.0, chamber.anchor.1), chamber.yaw);
            for step in -3..=3 {
                let sway = step as f32 * 0.10;
                let camera = staged_view(index, &interior_view(sway, 0.0), dot_w, dot_h);
                let tan_h = (camera.fov_rad * 0.5).tan();
                let bearing = (anchor.1 - camera.pos.y).atan2(anchor.0 - camera.pos.x);
                let at_x = 0.5 * (1.0 + wrap_pi(bearing - camera.heading_rad).tan() / tan_h);
                assert!(
                    (0.08..0.92).contains(&at_x),
                    "{} @{dot_w}x{dot_h}: sway {sway:+.2} put the hero at {at_x:.3}",
                    hero(index)
                );
            }
        }
    }
}

#[test]
fn a_tall_pane_keeps_the_room_painterly() {
    for index in 0u8..8 {
        for &(dot_w, dot_h) in &[(96usize, 72usize), (200, 304), (136, 164)] {
            let camera = staged_view(index, &interior_view(0.0, 0.0), dot_w, dot_h);
            let tan_v = (camera.fov_rad * 0.5).tan() * dot_h as f32 / dot_w as f32;
            let vertical = 2.0 * tan_v.atan();
            assert!(
                vertical <= VERT_FOV_MAX + 1e-3,
                "{} @{dot_w}x{dot_h}: vertical fov {vertical:.3} blew the cap",
                hero(index)
            );
            assert!(camera.fov_rad > 0.4, "{}: fov collapsed", hero(index));
        }
    }
}

#[test]
fn rooms_render_byte_identically_and_differ_from_each_other() {
    let view = interior_view(0.0, 0.0);
    let mut frames = Vec::new();
    for index in 0u8..8 {
        let camera = staged_view(index, &view, 96, 72);
        let scene = super::super::scene::interior_scene(index);
        let first = raster::render_scene(&scene, &camera, 96, 72);
        let second = raster::render_scene(&scene, &camera, 96, 72);
        assert_eq!(
            first.as_raw(),
            second.as_raw(),
            "{}: identical inputs must render byte-identical frames",
            hero(index)
        );
        frames.push(first.into_raw());
    }
    for a in 0..frames.len() {
        for b in a + 1..frames.len() {
            assert_ne!(
                frames[a],
                frames[b],
                "{} and {} render the same room",
                hero(a as u8),
                hero(b as u8)
            );
        }
    }
}

#[test]
fn scrying_glass_is_visible_in_the_production_room_at_mini_and_native_sizes() {
    let room = super::super::scene::interior_scene(7);
    let without_glass = Mesh {
        tris: room
            .tris
            .iter()
            .filter(|tri| tri.mat != mat::CRYSTAL)
            .copied()
            .collect(),
    };
    for (width, height) in [(80, 64), (256, 224), (768, 672)] {
        let camera = staged_view(7, &interior_view(0.0, 0.0), width, height);
        let with = raster::render_scene(&room, &camera, width, height);
        let without = raster::render_scene(&without_glass, &camera, width, height);
        let changed = with
            .pixels()
            .zip(without.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > width * height / 100,
            "the glass must occupy visible pixels, not hide behind scenery: {changed} at {width}x{height}"
        );
        let centre = with.get_pixel((width as f32 * 0.41) as u32, (height as f32 * 0.53) as u32);
        assert!(
            centre[2] > centre[0] + 20 && centre[1] > centre[0] + 20,
            "glass centre is occluded: {centre:?}"
        );
    }
}

const EVERY_BUILDING: [super::super::super::Building; 8] = {
    use super::super::super::Building as B;
    [
        B::Keep,
        B::Gatehouse,
        B::Rookery,
        B::Scriptorium,
        B::Smithy,
        B::Chapel,
        B::RoundTable,
        B::Observatory,
    ]
};

#[test]
fn the_room_table_is_in_building_index_order() {
    for (index, &building) in EVERY_BUILDING.iter().enumerate() {
        assert_eq!(
            super::super::super::cinematics::building_index(building) as usize,
            index,
            "{building:?} is not room {index}"
        );
    }
}

fn interior_at(index: u8, bucket: u64) -> Vec<u8> {
    render_interior_frame(
        EVERY_BUILDING[index as usize],
        &interior_view(0.0, 0.0),
        96,
        72,
        bucket,
    )
    .into_raw()
}

fn mean_delta(a: &[u8], b: &[u8]) -> f32 {
    assert_eq!(a.len(), b.len());
    let total: u64 = a
        .iter()
        .zip(b.iter())
        .map(|(&l, &r)| l.abs_diff(r) as u64)
        .sum();
    total as f32 / a.len() as f32
}

#[test]
fn one_bucket_is_one_fire() {
    for index in 0u8..8 {
        for bucket in [0u64, 5, 37, 1_000_003] {
            assert_eq!(
                interior_at(index, bucket),
                interior_at(index, bucket),
                "{}: bucket {bucket} rendered two different fires",
                hero(index)
            );
        }
        let held = interior_at(index, 1);
        for bucket in 2..BUCKETS_PER_STEP {
            assert_eq!(
                held,
                interior_at(index, bucket),
                "{}: bucket {bucket} broke step with bucket 1",
                hero(index)
            );
        }
    }
}

#[test]
fn adjacent_breaths_differ_and_stay_subtle() {
    const SUBTLE: f32 = 1.0;
    for index in 0u8..8 {
        for step in 0..BREATH_STEPS {
            let here = interior_at(index, step * BUCKETS_PER_STEP);
            let next = interior_at(index, (step + 1) * BUCKETS_PER_STEP);
            assert_ne!(
                here,
                next,
                "{}: the fire stopped breathing at step {step}",
                hero(index)
            );
            let delta = mean_delta(&here, &next);
            assert!(
                delta <= SUBTLE,
                "{}: step {step} moved {delta:.3} code values — that is a strobe, \
                     not firelight",
                hero(index)
            );
            let worst = here
                .iter()
                .zip(next.iter())
                .map(|(&l, &r)| l.abs_diff(r))
                .max()
                .unwrap_or(0);
            assert!(
                worst <= 20,
                "{}: step {step} moved one channel by {worst}",
                hero(index)
            );
        }
    }
}

#[test]
fn the_breath_comes_round_again() {
    let period = BREATH_STEPS * BUCKETS_PER_STEP;
    for index in 0u8..8 {
        for bucket in [0u64, 3, 11, 26] {
            assert_eq!(
                firelight(index, bucket),
                firelight(index, bucket + period),
                "{}: the cycle did not wrap at bucket {bucket}",
                hero(index)
            );
            assert_eq!(
                interior_at(index, bucket),
                interior_at(index, bucket + period),
                "{}: bucket {bucket} and {} render different rooms",
                hero(index),
                bucket + period
            );
        }
        let states: Vec<_> = (0..BREATH_STEPS)
            .map(|step| firelight(index, step * BUCKETS_PER_STEP))
            .collect();
        for a in 0..states.len() {
            for b in a + 1..states.len() {
                assert_ne!(
                    states[a],
                    states[b],
                    "{}: breath steps {a} and {b} are the same fire",
                    hero(index)
                );
            }
        }
    }
}

#[test]
fn the_authored_breath_stays_inside_a_few_percent() {
    for (step, &(gain, warm)) in BREATH.iter().enumerate() {
        assert!(
            (0.95..=1.05).contains(&gain),
            "step {step}: gain {gain} is more than a few percent",
        );
        assert!(
            warm.abs() <= 0.10,
            "step {step}: warm swing {warm} is a colour change, not a settle",
        );
    }
    assert_eq!(BREATH[0], (1.000, 0.00));
}

#[test]
fn only_the_flame_breathes() {
    for index in 0u8..8 {
        let scene = super::super::scene::interior_scene(index);
        let steady = raster::render_scene(&scene, &lit_room(index), 96, 72);
        let flared = raster::render_scene_lit(
            &scene,
            &lit_room(index),
            96,
            72,
            raster::Firelight {
                gain: 1.60,
                warm: -0.40,
            },
        );
        assert_ne!(
            steady.as_raw(),
            flared.as_raw(),
            "{}: a 60% flare changed nothing — the fire is not wired up",
            hero(index)
        );
        let doused = Mesh {
            tris: scene
                .tris
                .iter()
                .filter(|tri| !matches!(tri.mat, mat::FIRE | mat::FLAME | mat::EMBER))
                .copied()
                .collect(),
        };
        let cold = raster::render_scene(&doused, &lit_room(index), 96, 72);
        let cold_flared = raster::render_scene_lit(
            &doused,
            &lit_room(index),
            96,
            72,
            raster::Firelight {
                gain: 1.60,
                warm: -0.40,
            },
        );
        assert_eq!(
            cold.as_raw(),
            cold_flared.as_raw(),
            "{}: the flicker reached a window, a shaft or a wall",
            hero(index)
        );
    }
}

fn lit_room(index: u8) -> View3 {
    staged_view(index, &interior_view(0.0, 0.0), 96, 72)
}

#[test]
#[ignore]
fn write_room_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ROOM_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for index in 0..8u8 {
        let scene = super::super::scene::interior_scene(index);
        for (label, w, h) in [("large", 384, 288), ("mini", 96, 72)] {
            let view = staged_view(index, &interior_view(0.0, 0.0), w, h);
            raster::render_scene(&scene, &view, w, h)
                .save(dir.join(format!("room{index}-{label}.png")))
                .unwrap();
        }
    }
}
