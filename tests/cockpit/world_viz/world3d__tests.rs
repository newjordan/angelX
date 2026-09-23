use super::*;

#[test]
fn retired_view_commands_and_launch_flags_cannot_select_other_outdoors() {
    let _env = crate::tests::env_lock();
    for word in [
        "3d", "MESH", "mesh3d", "dotmax", "journey", "living", "top", "topdown", "top-down", "2d",
        "raycast", "ray", "ambient", "art",
    ] {
        let view = WorldView::parse(word).expect("supported compatibility alias");
        let _named = crate::tests::TestEnvGuard::set("ANGEL_WORLD_VIEW", word);
        for legacy in [
            "", "0", "off", "false", "no", "n", "1", "on", "true", "yes", "y", "typo",
        ] {
            let _legacy = crate::tests::TestEnvGuard::set("ANGEL_WORLD_3D", legacy);
            set(view);
            assert_eq!(current(), WorldView::Mesh3d);
            assert!(toggle());
            assert_eq!(cycle(), WorldView::Mesh3d);
            assert!(status_line().contains("Dotmax 3D"));
        }
    }
    assert_eq!(WorldView::parse("unsupported"), None);
}

use super::super::raycast::{RayMap, RaySprite, RayView};

/// A 31 × 31 window with one lit landmark billboard `standoff` tiles away
/// on the +x bearing — the shape `travel_scene()` hands the seam.
fn window(building: u8, standoff: f32) -> (RayMap, RayView) {
    let span = 31usize;
    let centre = (span as f32 - 1.0) * 0.5;
    let map = RayMap {
        width: span as u32,
        height: span as u32,
        cells: vec![0; span * span],
        terrain: vec![0.0; span * span],
        terrain_kind: vec![0; span * span],
        sprites: vec![RaySprite {
            x: centre + standoff,
            y: centre,
            width: 3.0,
            height: 3.0,
            id: building,
            lit: true,
        }],
    };
    let view = RayView {
        x: centre,
        y: centre,
        // Looking straight at the landmark, plus the authored thirds
        // offset — exactly what a settled `RayView` carries.
        heading_rad: super::super::cinematics::VISTA_MARKS[building as usize].thirds_offset_rad,
        look_yaw: 0.0,
        fov_rad: 1.05,
        bob: 0.0,
        eye_h: 0.0,
    };
    (map, view)
}

#[test]
fn every_building_lands_on_its_own_staged_vantage() {
    for building in 0u8..8 {
        let (map, view) = window(building, 5.5);
        let camera = view3_from_ray(&map, &view, 200, 304);
        let vantage = &VANTAGES[building as usize];
        assert!(
            (camera.pos.x - vantage.eye.0).abs() < 0.05
                && (camera.pos.y - vantage.eye.1).abs() < 0.05,
            "{}: settled at {:?}, want {:?}",
            vantage.hero,
            (camera.pos.x, camera.pos.y),
            vantage.eye
        );
        // The eye stands on the court's own ground, not on z = 0.
        let ground = arch::court_ground_z(scene::SCENE_SEED, camera.pos.x, camera.pos.y);
        assert!(
            (camera.pos.z - (ground + vantage.eye_h)).abs() < 1e-3,
            "{}: eye {} is not {} above ground {ground}",
            vantage.hero,
            camera.pos.z,
            vantage.eye_h
        );
    }
}

#[test]
fn the_anchor_lands_on_its_third_and_the_horizon_in_the_lower_third() {
    // Framing is expressed in screen fractions and solved per aspect, so
    // both the wide ride pane and the tall vista plate must honour it.
    for &(dot_w, dot_h) in &[(96usize, 72usize), (200, 304), (216, 200)] {
        for building in 0u8..8 {
            let (map, view) = window(building, 5.5);
            let camera = view3_from_ray(&map, &view, dot_w, dot_h);
            let vantage = &VANTAGES[building as usize];

            let tan_h = (camera.fov_rad * 0.5).tan();
            let tan_v = tan_h * dot_h as f32 / dot_w as f32;
            let bearing = (vantage.anchor.1 - camera.pos.y).atan2(vantage.anchor.0 - camera.pos.x);
            let offset = wrap_pi(bearing - camera.heading_rad);
            let at_x = 0.5 * (1.0 + offset.tan() / tan_h);
            assert!(
                (at_x - vantage.frame_x).abs() < 0.02,
                "{} @{dot_w}x{dot_h}: anchor at {at_x:.3}, want {:.3}",
                vantage.hero,
                vantage.frame_x
            );
            let at_y = 0.5 * (1.0 + camera.pitch.tan() / tan_v);
            assert!(
                (at_y - vantage.horizon).abs() < 0.02,
                "{} @{dot_w}x{dot_h}: horizon at {at_y:.3}, want {:.3}",
                vantage.hero,
                vantage.horizon
            );
            assert!(
                at_y >= 0.60 && at_x > 0.06 && at_x < 0.94,
                "{}: composition off the frame",
                vantage.hero
            );
        }
    }
}

#[test]
fn the_table_stages_thirds_standoff_and_open_sky() {
    for (index, vantage) in VANTAGES.iter().enumerate() {
        assert!(
            (vantage.frame_x - 0.5).abs() >= 0.06,
            "{index}: {} centres its hero",
            vantage.hero
        );
        assert!(
            vantage.horizon >= 0.60,
            "{index}: horizon {} is not in the lower third",
            vantage.horizon
        );
        assert!(
            vantage.eye.0.abs() <= arch::COURT_EXTENT - 2.0
                && vantage.eye.1.abs() <= arch::COURT_EXTENT - 2.0,
            "{index}: the eye stands off the edge of the world"
        );
        let reach = ((vantage.anchor.0 - vantage.eye.0).powi(2)
            + (vantage.anchor.1 - vantage.eye.1).powi(2))
        .sqrt();
        assert!(
            reach >= 8.0,
            "{index}: {} is a close-up at {reach:.1} tiles",
            vantage.hero
        );
    }
}

#[test]
fn the_ride_sweeps_the_approach_road_before_it_settles() {
    // Far out: the camera runs the road up from the river crossing.
    let (map, view) = window(2, 19.0);
    let far = view3_from_ray(&map, &view, 96, 72);
    assert!(
        far.pos.y < -24.5 && (far.pos.x - VANTAGES[1].eye.0).abs() < 1.0,
        "a distant ride should retain the framed approach, got {:?}",
        (far.pos.x, far.pos.y)
    );
    // The approach moves gently before cutting to the destination mark.
    let reach = |camera: &View3| {
        ((camera.pos.x - VANTAGES[2].eye.0).powi(2) + (camera.pos.y - VANTAGES[2].eye.1).powi(2))
            .sqrt()
    };
    let (map, view) = window(2, 12.0);
    let mid = view3_from_ray(&map, &view, 96, 72);
    let (map, view) = window(2, 5.5);
    let settled = view3_from_ray(&map, &view, 96, 72);
    assert!(
        reach(&far) > reach(&mid) && reach(&mid) > reach(&settled),
        "the sweep must close on the mark: {:.1} → {:.1} → {:.1}",
        reach(&far),
        reach(&mid),
        reach(&settled)
    );
    assert!(reach(&settled) < 0.6, "arrival must land on the mark");
}

#[test]
fn operator_yaw_and_pitch_turn_the_staged_camera() {
    let (map, mut view) = window(0, 5.5);
    let level = view3_from_ray(&map, &view, 96, 72);
    view.heading_rad += 0.5;
    let turned = view3_from_ray(&map, &view, 96, 72);
    let taken = wrap_pi(turned.heading_rad - level.heading_rad);
    assert!(
        taken > 0.44 && taken < 0.5,
        "a settled camera must follow operator yaw, took {taken:.3} of 0.5"
    );
    // Monotone with no saturation: a bigger turn is always a bigger turn,
    // or the arrow keys stop answering past some angle.
    view.heading_rad += 0.4;
    let further = view3_from_ray(&map, &view, 96, 72);
    assert!(
        wrap_pi(further.heading_rad - turned.heading_rad) > 0.30,
        "the swing must keep answering past half a radian"
    );
    view.heading_rad -= 0.4;
    view.heading_rad -= 0.5;
    view.bob = 10.0;
    let raised = view3_from_ray(&map, &view, 96, 72);
    assert!(
        (raised.pitch - level.pitch - 0.30).abs() < 1e-3,
        "the ride's bob must still ride in as pitch"
    );
    view.bob = 400.0;
    assert_eq!(
        view3_from_ray(&map, &view, 96, 72).pitch,
        0.90,
        "pitch must stay clamped"
    );
}

#[test]
fn a_tall_pane_keeps_its_vertical_field_painterly() {
    let (map, view) = window(0, 5.5);
    for &(dot_w, dot_h) in &[(96usize, 72usize), (200, 304), (136, 164)] {
        let camera = view3_from_ray(&map, &view, dot_w, dot_h);
        let tan_v = (camera.fov_rad * 0.5).tan() * dot_h as f32 / dot_w as f32;
        let vertical = 2.0 * tan_v.atan();
        assert!(
            vertical <= VERT_FOV_MAX + 1e-3,
            "{dot_w}x{dot_h}: vertical fov {vertical:.3} blew the cap"
        );
        assert!(camera.fov_rad > 0.4, "{dot_w}x{dot_h}: fov collapsed");
    }
}

/// The probe camera has to *be* the settled ride camera, or a staging
/// session tunes a picture nobody will ever see.
#[test]
fn a_probe_camera_matches_the_settled_ride() {
    for building in 0u8..8 {
        let (map, view) = window(building, 5.5);
        let live = view3_from_ray(&map, &view, 200, 304);
        let probe = settled_camera(&VANTAGES[building as usize], 200, 304);
        assert!(
            (live.pos.x - probe.pos.x).abs() < 0.9
                && (live.pos.y - probe.pos.y).abs() < 0.9
                && (live.heading_rad - probe.heading_rad).abs() < 0.05
                && (live.pitch - probe.pitch).abs() < 1e-4
                && (live.fov_rad - probe.fov_rad).abs() < 1e-4,
            "vantage {building}: probe {probe:?} drifted from the ride {live:?}"
        );
    }
}

/// Manual staging bench. Prints, for every mark in `VANTAGES` and for any
/// candidate poses parked in `PROBE`, exactly the numbers the vista law is
/// written in — sky / wall / ground, standoff, and where the hero's own
/// silhouette lands across the frame. Restaging by editing the table and
/// re-reading PNGs costs a render per guess; this costs one compile for a
/// whole grid.
///
/// `cargo test --bin angel probe_the_staging -- --ignored --nocapture`
#[test]
#[ignore = "manual staging bench: run with --nocapture to read the table"]
fn probe_the_staging() {
    /// Candidate marks under consideration. Empty in a landed tree.
    const PROBE: &[Vantage] = &[];

    let report = |label: &str, vantage: &Vantage| {
        let camera = settled_camera(vantage, 200, 304);
        let scene = scene::scene_for(scene::SceneKey::COURT);
        let (sky, wall, ground) = raster::composition_mix(&scene, &camera, 200, 304);
        let (standoff, near_x, near_y) = camera_standoff(&camera);
        let tan_h = (camera.fov_rad * 0.5).tan();
        let bearing = (vantage.anchor.1 - camera.pos.y).atan2(vantage.anchor.0 - camera.pos.x);
        let at_x = 0.5 * (1.0 + wrap_pi(bearing - camera.heading_rad).tan() / tan_h);
        let reach = ((vantage.anchor.0 - vantage.eye.0).powi(2)
            + (vantage.anchor.1 - vantage.eye.1).powi(2))
        .sqrt();
        eprintln!(
            "{label:<12} eye=({:>6.1},{:>6.1}) h={:.1} lens={:.2} | sky={sky:.3} wall={wall:.3} ground={ground:.3} | standoff={standoff:.1}@({near_x:.1},{near_y:.1}) reach={reach:.1} hero_at={at_x:.2} hdg={:.1}° | {}",
            vantage.eye.0,
            vantage.eye.1,
            vantage.eye_h,
            vantage.lens,
            camera.heading_rad.to_degrees().rem_euclid(360.0),
            vantage.hero,
        );
    };
    for (index, vantage) in VANTAGES.iter().enumerate() {
        report(&format!("mark {index}"), vantage);
    }
    for (index, vantage) in PROBE.iter().enumerate() {
        report(&format!("probe {index}"), vantage);
    }
}

#[test]
fn a_windowless_scene_still_produces_a_finite_camera() {
    // Interiors and the first frames of a long journey carry no landmark.
    let map = RayMap {
        width: 9,
        height: 9,
        cells: vec![0; 81],
        terrain: vec![0.0; 81],
        terrain_kind: vec![0; 81],
        sprites: Vec::new(),
    };
    let view = RayView {
        x: 4.0,
        y: 4.0,
        heading_rad: 2.0,
        look_yaw: 0.0,
        fov_rad: 1.05,
        bob: 0.0,
        eye_h: 0.0,
    };
    let camera = view3_from_ray(&map, &view, 96, 72);
    assert!(camera.pos.x.is_finite() && camera.pos.y.is_finite() && camera.pos.z.is_finite());
    assert!(camera.heading_rad.is_finite() && camera.pitch.is_finite());
}
#[test]
fn travel_heading_cannot_replace_intentional_camera_look() {
    let (map, mut view) = window(2, 13.0);
    let level = view3_from_ray(&map, &view, 96, 72);
    view.heading_rad += 2.0;
    let automatic_turn = view3_from_ray(&map, &view, 96, 72);
    assert_eq!(level.heading_rad, automatic_turn.heading_rad);
    view.look_yaw = 0.4;
    let intentional_turn = view3_from_ray(&map, &view, 96, 72);
    assert!((wrap_pi(intentional_turn.heading_rad - level.heading_rad) - 0.4).abs() < 1e-4);
    view.bob = 10.0;
    let raised = view3_from_ray(&map, &view, 96, 72);
    assert!((raised.pitch - intentional_turn.pitch - 0.30).abs() < 1e-4);
}
