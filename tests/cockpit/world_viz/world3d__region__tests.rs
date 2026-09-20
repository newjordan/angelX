use super::super::super::raycast::{RayMap, RaySprite, RayView};
use super::super::mesh::mat;
use super::*;

/// The dot canvases the floors are judged at: the ride's own small pane
/// (48 × 18 cells) and the wide one the cockpit usually runs (72 × 26).
const PANES: [(usize, usize); 2] = [(144, 104), (96, 72)];

/// A settled 31 × 31 window with one lit landmark — the shape
/// `travel_scene()` hands the seam, so the operator swing reads as zero.
fn window() -> (RayMap, RayView) {
    let span = 31usize;
    let centre = (span as f32 - 1.0) * 0.5;
    let map = RayMap {
        width: span as u32,
        height: span as u32,
        cells: vec![0; span * span],
        terrain: vec![0.0; span * span],
        terrain_kind: vec![0; span * span],
        sprites: vec![RaySprite {
            x: centre + 5.5,
            y: centre,
            width: 3.0,
            height: 3.0,
            id: 0,
            lit: true,
        }],
    };
    let view = RayView {
        x: centre,
        y: centre,
        heading_rad: super::super::super::cinematics::VISTA_MARKS[0].thirds_offset_rad,
        look_yaw: 0.0,
        fov_rad: 1.05,
        bob: 0.0,
        eye_h: 0.0,
    };
    (map, view)
}

#[test]
fn every_region_but_castle_town_has_a_stage_and_a_walk() {
    assert_eq!(
        stage_for(Region::CastleTown),
        None,
        "the court is unchanged"
    );
    for region in [
        Region::TheMines,
        Region::DarkForest,
        Region::Swamp,
        Region::DragonKeep,
        Region::Homecoming,
    ] {
        let stage = stage_for(region).expect("every adventure region stages");
        assert!(
            !marks(stage).is_empty(),
            "{}: a region with no marks has no camera",
            stage.label()
        );
        assert_eq!(waypoint_count(region), marks(stage).len());
    }
    // The Mines' walk is Z2's room grid: one waypoint per chamber.
    assert_eq!(
        waypoint_count(Region::TheMines),
        arch::regions::MINE_STATIONS,
        "a gallery station per authored chamber"
    );
}

#[test]
fn each_stage_is_built_once_shared_and_inside_the_triangle_ceiling() {
    for stage in Stage::ALL {
        let first = super::super::scene::region_scene(stage, 0, 0, 0);
        let second = super::super::scene::region_scene(stage, 0, 0, 0);
        assert!(
            std::sync::Arc::ptr_eq(&first, &second),
            "{}: the stage must be cached, not rebuilt per frame",
            stage.label()
        );
        assert!(
            first.tris.len() > 400,
            "{}: {} triangles is an empty box",
            stage.label(),
            first.tris.len()
        );
        // Z3 spec §3: 8k triangles per stage.
        assert!(
            first.tris.len() <= 8_000,
            "{}: {} triangles blew the stage budget",
            stage.label(),
            first.tris.len()
        );
    }
}

#[test]
fn every_stage_is_finite_bounded_and_deterministic_across_builds() {
    for stage in Stage::ALL {
        let extent = arch::stage_extent(stage);
        for danger in 0u8..4 {
            for chests in 0u8..=arch::MAX_CHESTS as u8 {
                let a = arch::region_stage(stage, SCENE_SEED, danger, chests, 0);
                let b = arch::region_stage(stage, SCENE_SEED, danger, chests, 0);
                assert_eq!(
                    a.tris.len(),
                    b.tris.len(),
                    "{} d{danger} c{chests}: triangle count is not stable",
                    stage.label()
                );
                for (left, right) in a.tris.iter().zip(b.tris.iter()) {
                    assert_eq!(left.mat, right.mat);
                    for index in 0..3 {
                        assert_eq!(
                            left.v[index],
                            right.v[index],
                            "{}: byte-identical determinism broken",
                            stage.label()
                        );
                        assert_eq!(left.uv[index], right.uv[index]);
                    }
                }
                for tri in &a.tris {
                    for point in tri.v {
                        assert!(
                            point.x.is_finite() && point.y.is_finite() && point.z.is_finite(),
                            "{}: non-finite vertex {point:?}",
                            stage.label()
                        );
                        assert!(
                            point.x.abs() <= extent + 2.0 && point.y.abs() <= extent + 2.0,
                            "{}: geometry outside the ground patch at {point:?}",
                            stage.label()
                        );
                    }
                    assert!(
                        tri.normal.length() > 0.9,
                        "{}: degenerate triangle normal {:?}",
                        stage.label(),
                        tri.normal
                    );
                }
            }
        }
    }
}

#[test]
fn more_loot_and_more_danger_change_the_stage_they_are_asked_for() {
    for stage in Stage::ALL {
        let calm = arch::region_stage(stage, SCENE_SEED, 0, 0, 0);
        let dark = arch::region_stage(stage, SCENE_SEED, 3, 0, 0);
        assert!(
            dark.tris.len() < calm.tris.len(),
            "{}: danger must gutter the lights ({} → {})",
            stage.label(),
            calm.tris.len(),
            dark.tris.len()
        );
        let loot = arch::region_stage(stage, SCENE_SEED, 0, 3, 0);
        assert!(
            loot.tris.len() > calm.tris.len(),
            "{}: treasure must put chests on the floor",
            stage.label()
        );
    }
}

#[test]
fn every_mark_stages_thirds_standoff_and_a_reach() {
    for stage in Stage::ALL {
        let extent = arch::stage_extent(stage);
        for (index, vantage) in marks(stage).iter().enumerate() {
            assert!(
                (vantage.frame_x - 0.5).abs() >= 0.06,
                "{} [{index}]: {} centres its hero",
                stage.label(),
                vantage.hero
            );
            match vantage.framing {
                Framing::Vista { horizon } => assert!(
                    horizon >= 0.60,
                    "{} [{index}]: horizon {horizon} is not in the lower third",
                    stage.label()
                ),
                Framing::Room { frame_y, .. } => {
                    // Indoors the identity piece sits in the upper-middle
                    // band — never centred, never on the floor line. (The
                    // Mines aim at the gallery's vanishing point at chest
                    // height, so "above the eye" is not a rule here.)
                    assert!(
                        (0.30..=0.60).contains(&frame_y),
                        "{} [{index}]: identity piece at {frame_y} is not in the upper-middle band",
                        stage.label()
                    );
                }
            }
            assert!(
                vantage.eye.0.abs() <= extent - 2.0 && vantage.eye.1.abs() <= extent - 2.0,
                "{} [{index}]: the eye stands off the edge of the world",
                stage.label()
            );
            let reach = ((vantage.anchor.0 - vantage.eye.0).powi(2)
                + (vantage.anchor.1 - vantage.eye.1).powi(2))
            .sqrt();
            assert!(
                reach >= 8.0,
                "{} [{index}]: {} is a close-up at {reach:.1} tiles",
                stage.label(),
                vantage.hero
            );
        }
    }
}

#[test]
fn every_staged_region_frame_honours_the_vista_floors() {
    // The floors are the court's own (`ride.rs`): open sky above, mass
    // never more than half the frame, real ground to stand on, and the eye
    // never inside four tiles of a facade. New stages pass them or the
    // staging is wrong — they are never loosened.
    //
    // Every mark is measured before anything fails, because restaging is a
    // whole-table job: fixing the one mark a `?` short-circuit reported
    // just moves the failure to the next one.
    //
    // Z3b: a **roomed** mark (`Framing::Room` — the Mines, the Dragon Keep)
    // has no horizon and no sky, so the outdoor sky/mass floors would fail
    // every honest interior. It is judged the way `interior.rs` judges its
    // eight rooms: a floor band to stand on, a light source somewhere in
    // the scene, and the centre-of-frame reach below. The floors are not
    // loosened — a different family of place gets its own instrument.
    let mut broken: Vec<String> = Vec::new();
    for stage in Stage::ALL {
        let scene = super::super::scene::region_scene(stage, 0, 3, 0);
        let lit = scene
            .tris
            .iter()
            .any(|tri| matches!(tri.mat, mat::EMBER | mat::WINDOW | mat::MOONLIGHT));
        if is_interior(stage) && !lit {
            broken.push(format!(
                "{}: nothing in this interior gives light",
                stage.label()
            ));
        }
        for (waypoint, vantage) in marks(stage).iter().enumerate() {
            for &(dot_w, dot_h) in &PANES {
                let (sky, wall, ground) = composition_mix(stage, waypoint, 0, 3, dot_w, dot_h);
                let label = format!(
                    "{} [{waypoint}] {} @{dot_w}x{dot_h}",
                    stage.label(),
                    vantage.hero
                );
                eprintln!("{label}: sky={sky:.3} wall={wall:.3} ground={ground:.3}");
                if !vantage.framing.is_room() {
                    if sky < 0.20 {
                        broken.push(format!("{label}: only {sky:.3} sky"));
                    }
                    if wall > 0.50 {
                        broken.push(format!("{label}: {wall:.3} of the frame is wall"));
                    }
                }
                if ground < 0.08 {
                    broken.push(format!("{label}: only {ground:.3} ground"));
                }
            }
            let (clear, near_x, near_y) = standoff(stage, waypoint, 0, 3, 200, 304);
            eprintln!(
                "{} [{waypoint}]: standoff={clear:.1}@({near_x:.1},{near_y:.1})",
                stage.label()
            );
            if clear < 4.0 {
                broken.push(format!(
                        "{} [{waypoint}]: a facade at {clear:.1} tiles ({near_x:.1},{near_y:.1}) — a doorway, not a vista",
                        stage.label()
                    ));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "vista floors broken:\n  {}",
        broken.join("\n  ")
    );
}

#[test]
fn the_staged_ride_lands_on_the_mark_the_probe_reports() {
    let (map, view) = window();
    for stage in Stage::ALL {
        for waypoint in 0..marks(stage).len() {
            let live = staged_view(stage, waypoint, &map, &view, 0.0, 200, 304);
            let probe = settled_camera(stage, waypoint, 200, 304);
            assert!(
                (live.pos.x - probe.pos.x).abs() < 0.9
                    && (live.pos.y - probe.pos.y).abs() < 0.9
                    && (live.pitch - probe.pitch).abs() < 1e-4
                    && (live.fov_rad - probe.fov_rad).abs() < 1e-4,
                "{} [{waypoint}]: probe {probe:?} drifted from the ride {live:?}",
                stage.label()
            );
        }
    }
}

#[test]
fn the_anchor_lands_on_its_third_and_the_horizon_in_the_lower_third() {
    let (map, view) = window();
    for &(dot_w, dot_h) in &[(96usize, 72usize), (144, 104), (200, 304)] {
        for stage in Stage::ALL {
            for (waypoint, vantage) in marks(stage).iter().enumerate() {
                let camera = staged_view(stage, waypoint, &map, &view, 0.0, dot_w, dot_h);
                let tan_h = (camera.fov_rad * 0.5).tan();
                let tan_v = tan_h * dot_h as f32 / dot_w as f32;
                let bearing =
                    (vantage.anchor.1 - camera.pos.y).atan2(vantage.anchor.0 - camera.pos.x);
                let at_x = 0.5 * (1.0 + wrap_pi(bearing - camera.heading_rad).tan() / tan_h);
                assert!(
                    (at_x - vantage.frame_x).abs() < 0.06,
                    "{} [{waypoint}] @{dot_w}x{dot_h}: anchor at {at_x:.3}, want {:.3}",
                    stage.label(),
                    vantage.frame_x
                );
                let (at_y, want_y, what) = match vantage.framing {
                    Framing::Vista { horizon } => {
                        (0.5 * (1.0 + camera.pitch.tan() / tan_v), horizon, "horizon")
                    }
                    Framing::Room { anchor_z, frame_y } => {
                        // Where the identity piece's own point lands down
                        // the frame, from the camera actually solved.
                        let reach = ((vantage.anchor.0 - camera.pos.x).powi(2)
                            + (vantage.anchor.1 - camera.pos.y).powi(2))
                        .sqrt();
                        let elevation = ((anchor_z - camera.pos.z) / reach).atan();
                        (
                            0.5 * (1.0 - (elevation - camera.pitch).tan() / tan_v),
                            frame_y,
                            "identity piece",
                        )
                    }
                };
                assert!(
                    (at_y - want_y).abs() < 0.03,
                    "{} [{waypoint}] @{dot_w}x{dot_h}: {what} at {at_y:.3}, want {want_y:.3}",
                    stage.label()
                );
            }
        }
    }
}

#[test]
fn a_tall_pane_keeps_its_vertical_field_painterly() {
    let (map, view) = window();
    for &(dot_w, dot_h) in &[(96usize, 72usize), (200, 304), (136, 164)] {
        for stage in Stage::ALL {
            let camera = staged_view(stage, 0, &map, &view, 0.0, dot_w, dot_h);
            let tan_v = (camera.fov_rad * 0.5).tan() * dot_h as f32 / dot_w as f32;
            let vertical = 2.0 * tan_v.atan();
            assert!(
                vertical <= VERT_FOV_MAX + 1e-3,
                "{} @{dot_w}x{dot_h}: vertical fov {vertical:.3} blew the cap",
                stage.label()
            );
            assert!(
                camera.fov_rad > 0.4,
                "{} @{dot_w}x{dot_h}: fov collapsed",
                stage.label()
            );
        }
    }
}

#[test]
fn operator_yaw_and_pitch_turn_the_staged_region_camera() {
    let (map, mut view) = window();
    let level = staged_view(Stage::Mines, 4, &map, &view, 0.0, 96, 72);
    let turned = staged_view(Stage::Mines, 4, &map, &view, 0.5, 96, 72);
    let taken = wrap_pi(turned.heading_rad - level.heading_rad);
    assert!(
        taken > 0.41 && taken < 0.5,
        "a staged region camera must follow operator yaw, took {taken:.3} of 0.5"
    );
    // Monotone with no saturation: a bigger turn is always a bigger turn,
    // or the arrow keys stop answering past some angle.
    let further = staged_view(Stage::Mines, 4, &map, &view, 0.9, 96, 72);
    assert!(
        wrap_pi(further.heading_rad - turned.heading_rad) > 0.25,
        "the swing must keep answering past half a radian"
    );
    // The town's own heading is not the region's: a ride pointed anywhere
    // must not drag the staged mark off its composition.
    view.heading_rad += 1.3;
    let unmoved = staged_view(Stage::Mines, 4, &map, &view, 0.0, 96, 72);
    assert!(
        (unmoved.heading_rad - level.heading_rad).abs() < 1e-6,
        "a region mark must ignore the town ride's heading"
    );
    view.heading_rad -= 1.3;
    view.bob = 10.0;
    let raised = staged_view(Stage::Mines, 4, &map, &view, 0.0, 96, 72);
    assert!(
        (raised.pitch - level.pitch - 0.30).abs() < 1e-3,
        "the ride's bob must still ride in as pitch"
    );
}

#[test]
fn a_rendered_region_frame_is_byte_identical_twice_over() {
    let (map, view) = window();
    for stage in Stage::ALL {
        let camera = staged_view(stage, 1, &map, &view, 0.0, 96, 72);
        let scene = super::super::scene::region_scene(stage, 2, 2, 0);
        let first = raster::render_scene_fogged(
            &scene,
            &camera,
            96,
            72,
            Firelight::STEADY,
            fog_distance(stage, 2),
        );
        let second = raster::render_scene_fogged(
            &scene,
            &camera,
            96,
            72,
            Firelight::STEADY,
            fog_distance(stage, 2),
        );
        assert_eq!(
            first.as_raw(),
            second.as_raw(),
            "{}: a region frame must be byte-identical",
            stage.label()
        );
    }
}

#[test]
fn danger_pulls_the_haze_in_and_never_past_the_lantern() {
    for stage in Stage::ALL {
        let mut previous = f32::INFINITY;
        for danger in 0u8..4 {
            let fog = fog_distance(stage, danger);
            assert!(
                fog < previous,
                "{}: danger {danger} must shorten the sight line",
                stage.label()
            );
            // Fog closer than the rider's lantern reach would black the
            // near ground out and the frame would have no foreground.
            assert!(
                fog > 12.0,
                "{}: danger {danger} fogged the frame to {fog:.1} tiles",
                stage.label()
            );
            previous = fog;
        }
        assert!(
            fog_distance(stage, 0) <= raster::FOG_DISTANCE,
            "{}: no region is clearer than a clear night over the court",
            stage.label()
        );
    }
}
