use super::*;
use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
use crate::stage::world_viz::{AdventureEvent, Building, LoopKind};

fn success() -> ToolOutcome {
    ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    }
}

fn expedition() -> World {
    let mut world = World::new(42);
    world.settle_at_for_test(Building::Keep);
    world.note_adventure(AdventureEvent::LoopStarted {
        kind: LoopKind::Coding,
        task: "an actual coding loop".into(),
    });
    world.note_adventure(AdventureEvent::Iteration { n: 1 });
    world
}

fn picture(world: &World, motion: MotionMode) -> image::RgbaImage {
    let (map, mut view) = world.travel_scene();
    view.bob = 0.0;
    render(
        Stage::Mines,
        world.quest(),
        &map,
        &view,
        0.0,
        world.expedition_playback(motion),
        (256, 160),
    )
}

#[test]
fn a_real_iteration_walks_between_marks_without_inventing_a_second_iteration() {
    let mut world = expedition();
    let first = world.expedition_playback(MotionMode::Full);
    let (map, view) = world.travel_scene();
    let start = camera(
        Stage::Mines,
        world.quest(),
        &map,
        &view,
        0.0,
        first,
        (256, 160),
    );
    for _ in 0..80 {
        world.tick();
    }
    let middle = world.expedition_playback(MotionMode::Full);
    let during = camera(
        Stage::Mines,
        world.quest(),
        &map,
        &view,
        0.0,
        middle,
        (256, 160),
    );
    assert!(middle.moving && middle.travel > first.travel);
    assert!(during.pos.x > start.pos.x + 0.5);
    for _ in 0..300 {
        world.tick();
    }
    let arrived = world.expedition_playback(MotionMode::Full);
    assert!(!arrived.moving);
    assert_eq!(arrived.travel, 1024);
    assert_eq!(world.quest().iteration(), 1);
    assert_eq!(world.quest().treasures(), 0);
}

#[test]
fn visits_override_the_adventure_and_follow_returns_to_it() {
    let mut world = expedition();
    assert!(world.live_adventure_view());
    world.visit_overworld("school-vault").unwrap();
    assert!(!world.live_adventure_view());
    world.follow_overworld();
    assert!(world.live_adventure_view());
    world.visit_overworld("mines").unwrap();
    assert!(
        !world.live_adventure_view(),
        "an explicit map visit remains a map visit"
    );
    world.follow_overworld();
    world.note_adventure(AdventureEvent::LoopFinished { ok: false });
    assert!(!world.live_adventure_view());
}

#[test]
fn map_toggle_returns_to_travel_and_preserves_other_operator_visits() {
    let mut world = expedition();
    assert!(world.toggle_adventure_map());
    assert_eq!(world.overworld_view_label(), Some("THE MINES"));
    assert!(!world.live_adventure_view());
    assert!(!world.has_authored_interior());
    assert!(
        !world.enter_interior(),
        "a mine map must not enter the civic Keep"
    );
    assert!(world.toggle_adventure_map());
    assert!(world.live_adventure_view());
    world.visit_overworld("school").unwrap();
    assert!(!world.toggle_adventure_map());
    assert!(world.visiting_school());
    assert!(world.has_authored_interior());
    assert!(world.enter_interior());
    assert!(world.inside_interior());
    world.visit_overworld("school-vault").unwrap();
    assert!(!world.toggle_adventure_map());
    assert!(world.inside_interior());
}

#[test]
fn real_inflight_tools_choose_the_pose_and_returning_clears_it() {
    let mut world = expedition();
    assert_eq!(world.expedition_playback(MotionMode::Full).work, Work::Rest);
    let id = ToolEventId("expedition-tool".into());
    world.note_tool_call_event(id.clone(), "read_file", "src/main.rs");
    assert_eq!(
        world.expedition_playback(MotionMode::Full).work,
        Work::Study
    );
    world.note_tool_result_event(&id, "read_file", "read", success());
    assert_eq!(world.expedition_playback(MotionMode::Full).work, Work::Rest);
    world.note_tool_call_event(
        ToolEventId("expedition-build".into()),
        "write_file",
        "src/main.rs",
    );
    assert_eq!(
        world.expedition_playback(MotionMode::Full).work,
        Work::Forge
    );
}

#[test]
fn settled_motion_off_and_reduced_do_not_drift_or_fabricate_animation() {
    for motion in [MotionMode::Off, MotionMode::Reduced] {
        let mut world = expedition();
        let before = picture(&world, motion);
        for _ in 0..200 {
            world.tick();
        }
        assert_eq!(before, picture(&world, motion));
        assert_eq!(world.expedition_playback(motion).phase, 0);
    }
}

#[test]
fn actor_geometry_is_bounded_finite_and_deterministic() {
    for work in [
        Work::Rest,
        Work::Study,
        Work::Forge,
        Work::Council,
        Work::Dispatch,
    ] {
        for phase in [0, 12, 47, 100, 239] {
            let playback = Playback {
                travel: 1024,
                phase,
                moving: false,
                work,
            };
            let mesh = knight(playback, false);
            assert!(mesh.tris.len() <= 240, "{} triangles", mesh.tris.len());
            for tri in mesh.tris {
                assert!(tri.normal.length() > 0.9);
                for p in tri.v {
                    assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
                    assert!(p.x.abs() < 1.0 && p.y.abs() < 1.0 && (0.0..=1.6).contains(&p.z));
                }
            }
        }
    }
    let world = expedition();
    assert_eq!(
        picture(&world, MotionMode::Full),
        picture(&world, MotionMode::Full)
    );
}

#[test]
fn every_mine_station_keeps_camera_and_party_clear_of_cart_walls_and_roof() {
    let mut world = expedition();
    let (map, view) = world.travel_scene();
    for iteration in 0..=18 {
        world.note_adventure(AdventureEvent::Iteration { n: iteration });
        for travel in [0, 128, 512, 896, 1024] {
            let playback = Playback {
                travel,
                phase: 12,
                moving: travel < 1024,
                work: Work::Forge,
            };
            let actor_view = camera(
                Stage::Mines,
                world.quest(),
                &map,
                &view,
                0.0,
                playback,
                (96, 72),
            );
            for yaw in [-0.52, 0.0, 0.52] {
                let eye = camera(
                    Stage::Mines,
                    world.quest(),
                    &map,
                    &view,
                    yaw,
                    playback,
                    (96, 72),
                )
                .pos;
                assert!((-19.6..19.6).contains(&eye.x));
                assert!((0.76..2.7).contains(&eye.y), "eye at {eye:?}");
                assert!((1.0..2.25).contains(&eye.z));
            }
            for member in 0..3 {
                let position = party_position(Stage::Mines, actor_view, 0.0, member);
                let facing = if playback.moving { 0.0 } else { -2.05 };
                let mesh = knight(playback, member != 0)
                    .rotated_z(facing)
                    .translated(position);
                for point in mesh.tris.iter().flat_map(|tri| tri.v) {
                    assert!(
                        point.x > -20.0 && point.x < 20.0,
                        "party beyond end at {point:?}"
                    );
                    assert!(
                        point.y > 0.72 && point.y < 2.85,
                        "party off ledge at {point:?}"
                    );
                    assert!(
                        point.z >= 0.0 && point.z < 2.4,
                        "party in floor or roof at {point:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_mine_has_a_real_lower_working_and_wall_height_torches() {
    let mesh = scene::region_scene(Stage::Mines, 0, 0, 0);
    assert!(mesh.tris.len() < 2_000, "mine cache must stay modest");
    assert!(
        mesh.tris
            .iter()
            .flat_map(|tri| tri.v)
            .any(|point| point.z <= -2.5)
    );
    let flames: Vec<_> = mesh
        .tris
        .iter()
        .filter(|tri| tri.mat == mat::EMBER)
        .collect();
    assert!(!flames.is_empty());
    assert!(
        flames
            .iter()
            .flat_map(|tri| tri.v)
            .all(|point| point.z > 1.0),
        "wall torch geometry must not be planted at floor level"
    );
}

#[test]
fn expedition_cache_ignores_town_assets_and_holds_each_animation_sample() {
    let mut world = expedition();
    let frame = |world: &World, motion| {
        world
            .scryglass_frame_with_motion(48, 18, false, 0.0, 0.0, 1.05, motion)
            .unwrap()
    };
    let first = frame(&world, MotionMode::Full);
    for _ in 0..3 {
        world.tick();
        assert!(std::sync::Arc::ptr_eq(
            &first,
            &frame(&world, MotionMode::Full)
        ));
    }
    world.tick();
    assert!(!std::sync::Arc::ptr_eq(
        &first,
        &frame(&world, MotionMode::Full)
    ));
    let quiet = frame(&world, MotionMode::Off);
    world.settle_at_for_test(Building::Smithy);
    for _ in 0..200 {
        world.tick();
    }
    assert!(std::sync::Arc::ptr_eq(
        &quiet,
        &frame(&world, MotionMode::Off)
    ));
    world.ride_cache.replace(None);
    assert_eq!(
        quiet,
        frame(&world, MotionMode::Off),
        "fresh region rendering must also ignore town location and camera height"
    );
    world.note_tool_call_event(
        ToolEventId("cache-study".into()),
        "read_file",
        "src/main.rs",
    );
    let working = frame(&world, MotionMode::Off);
    assert!(
        !std::sync::Arc::ptr_eq(&quiet, &working),
        "a changed work key must recompose the cached scene, even if its pixels match"
    );
}

#[test]
fn a_torch_pool_counts_as_the_floor_it_lights() {
    let view = View3 {
        pos: v3(0.0, 0.0, 1.5),
        heading_rad: 0.0,
        pitch: -0.3,
        fov_rad: 1.05,
    };
    let mut floor = Mesh::new();
    prim::quad(
        &mut floor,
        v3(-4.0, -12.0, 0.0),
        v3(20.0, -12.0, 0.0),
        v3(20.0, 12.0, 0.0),
        v3(-4.0, 12.0, 0.0),
        mat::FLOOR,
    );
    let expected = raster::composition_mix(&floor, &view, 96, 72);
    assert!(expected.2 > 0.5);
    for triangle in &mut floor.tris {
        triangle.mat = mat::TORCH_POOL;
    }
    assert_eq!(expected, raster::composition_mix(&floor, &view, 96, 72));
}

/// Render actual braille cells using the same dot/cell proportions as the
/// retained ride review fixture. This reveals what survives the mini pane.
fn terminal_preview(cells: &crate::ui::term::art::ColoredBrailleImage) -> image::RgbaImage {
    let mut output = image::RgbaImage::from_pixel(
        cells.width as u32 * 8,
        cells.height as u32 * 16,
        image::Rgba([10, 10, 16, 255]),
    );
    for y in 0..cells.height {
        for x in 0..cells.width {
            let cell = cells.cells[y * cells.width + x];
            let bits = (cell.glyph as u32).saturating_sub(0x2800) as u8;
            for dy in 0..4 {
                for dx in 0..2 {
                    if bits & crate::ui::term::art::braille_dot_bit(dx, dy) == 0 {
                        continue;
                    }
                    for py in 0..3 {
                        for px in 0..3 {
                            output.put_pixel(
                                (x * 8 + dx * 4 + 1 + px) as u32,
                                (y * 16 + dy * 4 + 1 + py) as u32,
                                image::Rgba([cell.fg[0], cell.fg[1], cell.fg[2], 255]),
                            );
                        }
                    }
                }
            }
        }
    }
    output
}

#[test]
#[ignore = "exports the production expedition path as a reviewable frame sequence"]
fn write_expedition_review_sequence() {
    let Some(directory) = std::env::var_os("ANGEL_EXPEDITION_REVIEW") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let mut world = expedition();
    // Labelled event fixture: travel, a real call-in-flight shape, then a
    // different tool. There are no invented success/fitness receipts.
    for frame in 0..48 {
        if frame == 22 {
            world.note_tool_call_event(
                ToolEventId("review-study".into()),
                "read_file",
                "src/main.rs",
            );
        }
        if frame == 32 {
            world.note_tool_result_event(
                &ToolEventId("review-study".into()),
                "read_file",
                "read",
                success(),
            );
            world.note_tool_call_event(
                ToolEventId("review-forge".into()),
                "write_file",
                "src/main.rs",
            );
        }
        picture(&world, MotionMode::Full)
            .save(directory.join(format!("mines-{frame:03}.png")))
            .unwrap();
        if [0, 21, 28, 47].contains(&frame) {
            let (map, mut view) = world.travel_scene();
            view.bob = 0.0;
            render(
                Stage::Mines,
                world.quest(),
                &map,
                &view,
                0.0,
                world.expedition_playback(MotionMode::Full),
                (96, 72),
            )
            .save(directory.join(format!("mini-{frame:03}-raw.png")))
            .unwrap();
            let cells = world
                .scryglass_frame_with_motion(48, 18, false, 0.0, 0.0, 1.05, MotionMode::Full)
                .unwrap();
            terminal_preview(&cells)
                .save(directory.join(format!("mini-{frame:03}-braille.png")))
                .unwrap();
        }
        for _ in 0..8 {
            world.tick();
        }
    }
    let _ = world
        .scryglass_frame_with_motion(72, 26, false, 0.0, 0.0, 1.05, MotionMode::Full)
        .unwrap();
    for iteration in 0..=9 {
        world.note_adventure(AdventureEvent::Iteration { n: iteration });
        for _ in 0..450 {
            world.tick();
        }
        let (map, mut view) = world.travel_scene();
        view.bob = 0.0;
        for (label, yaw) in [("follow", 0.0), ("pan", -0.52)] {
            render(
                Stage::Mines,
                world.quest(),
                &map,
                &view,
                yaw,
                world.expedition_playback(MotionMode::Full),
                (144, 104),
            )
            .save(directory.join(format!("station-{iteration:02}-{label}.png")))
            .unwrap();
        }
    }
    for (stage, event) in [
        (
            Stage::DarkForest,
            AdventureEvent::LoopStarted {
                kind: LoopKind::Research,
                task: "review fixture".into(),
            },
        ),
        (Stage::Swamp, AdventureEvent::Stall { level: 2 }),
        (Stage::DragonKeep, AdventureEvent::Submitted),
    ] {
        world.note_adventure(event);
        assert_eq!(region::stage_for(world.quest().region()), Some(stage));
        let (map, mut view) = world.travel_scene();
        view.bob = 0.0;
        render(
            stage,
            world.quest(),
            &map,
            &view,
            0.0,
            world.expedition_playback(MotionMode::Full),
            (256, 160),
        )
        .save(directory.join(format!("{stage:?}.png")))
        .unwrap();
    }
}
