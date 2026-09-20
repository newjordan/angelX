use super::super::super::raycast::RayView;
use super::*;

/// The `RayView` `interiors::compose` hands the seam, plus whatever the
/// operator has done to it (`ride.rs:138-142`).
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

/// A shaft is the light *from* a window. One falling out of blank masonry
/// reads as a rendering fault — the Chapel shipped that way for a pass,
/// with a pool of moonlight on its nave floor and no bay anywhere near the
/// x it fell at, and the frame looked broken without anybody being able to
/// say why.
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
                // The gatehouse aprons its road out under the night sky;
                // everything else stays inside a generous room box.
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
        let mut seen = [false; 24];
        for tri in &scene.tris {
            if (tri.mat as usize) < seen.len() {
                seen[tri.mat as usize] = true;
            }
        }
        assert!(seen[mat::FLOOR as usize], "{}: no floor", hero(index));
        assert!(seen[mat::WINDOW as usize], "{}: no light", hero(index));
        // ...and a *live* one. Every room is dressed with fire somewhere —
        // a hearth mouth, a brazier, a taper, a hung lamp — and that fire
        // is the only thing in the room that moves.
        assert!(
            seen[mat::FIRE as usize],
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

            // Never a wall close-up: whatever the middle of the frame is
            // pointed at stands well back from the lens.
            // An infinite reading is the *best* case, not a failure: the
            // observatory's centre ray goes straight out of the star slit
            // into the night, which is the opposite of a wall close-up.
            let clearance = standoff(index, &view, dot_w, dot_h);
            assert!(
                clearance >= 2.5,
                "{} @{dot_w}x{dot_h}: the frame centre lands {clearance:.2} tiles out",
                hero(index)
            );

            // The identity piece lands on its authored third.
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
        // The hero is across the room, not under the nose.
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
        // Monotone past the soft limit — the arrow keys never stop
        // answering, they only slow down.
        let further = staged_view(index, &interior_view(0.9, 0.0), 96, 72);
        assert!(
            wrap_pi(further.heading_rad - turned.heading_rad) > 0.12,
            "{}: the pan saturated",
            hero(index)
        );
        // ...and the eye breathes with it instead of pivoting on the spot.
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
    // The regression the damping exists for: `orbit_sway` runs to ±0.30 rad
    // (life.rs:757) and arrives at the seam folded into the heading, so the
    // staged frame has to survive its whole range at every pane shape.
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

// ── the fire breathes ─────────────────────────────────────────────────

/// The eight civic landmarks in `building_index` order, so these rails can
/// go through the seam's own entry point rather than round the back of it.
/// `the_room_table_is_in_building_index_order` keeps the list honest.
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

/// A room at a given tick-bucket, straight down the seam's own path.
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

/// Mean absolute per-channel difference between two frames, in code values
/// on the 0–255 scale. The flicker's whole budget is measured in this.
fn mean_delta(a: &[u8], b: &[u8]) -> f32 {
    assert_eq!(a.len(), b.len());
    let total: u64 = a
        .iter()
        .zip(b.iter())
        .map(|(&l, &r)| l.abs_diff(r) as u64)
        .sum();
    total as f32 / a.len() as f32
}

/// The memo law. The ride keys interiors on `tick / 2` and hands the same
/// bucket back here; if the same bucket rendered two different fires the
/// cache would flip between them for free and the room would strobe.
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
        // ...and a bucket inside the same breath step is the same fire, so
        // the flicker runs at the rate `BUCKETS_PER_STEP` says it does and
        // not at the 20 Hz the raw bucket ticks at.
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

/// Firelight, not a strobe: consecutive steps must *move* — a fire that
/// holds perfectly still is a lamp — and must move barely.
#[test]
fn adjacent_breaths_differ_and_stay_subtle() {
    // Measured off the eight staged rooms at 96 × 72 with the authored
    // −4.2% gain: the Smithy moves most, at 0.32 code values averaged over
    // the whole frame and 11 on its hottest single channel. The ceilings
    // sit a few times above that — room for a future room to be dressed
    // with more fire, none for a hearth that turns into a beacon.
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
            // Nothing may be *driven* to a new value wholesale either: a
            // few percent of a warm dot is a few code values, never a jump
            // from ember to daylight.
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

/// The breath is a loop. Buckets run forever; the fire must not.
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
        // Every phase of the cycle is actually reached, and no two rooms
        // are stuck on the same phase forever.
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

/// Subtle is a number, not an opinion: the authored table itself is held
/// inside a few percent, so no future edit can smuggle a beacon in.
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
    // A hearth at rest is the authored burn, so an interior rendered at
    // bucket 0 in a room with phase 0 matches every pinned plate.
    assert_eq!(BREATH[0], (1.000, 0.00));
}

/// The flicker reaches fire and nothing else. Windows are glazing, a shaft
/// of moonlight is a clock — a room whose panes pulse with its hearth is a
/// rendering fault, and this is the rail that says so.
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
        // Same room, same camera, a violent swing: every pixel that moved
        // has to belong to a surface the flame is allowed to touch. Proven
        // by rebuilding the scene with its fire deleted — if the swing
        // reached anything else, the two frames would still differ.
        let doused = Mesh {
            tris: scene
                .tris
                .iter()
                .filter(|tri| !matches!(tri.mat, mat::FIRE | mat::EMBER))
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

/// The staged camera for a room, shared by the lighting rails above.
fn lit_room(index: u8) -> View3 {
    staged_view(index, &interior_view(0.0, 0.0), 96, 72)
}
