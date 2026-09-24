#[test]
fn idle_world_keeps_the_same_cast_without_implying_work() {
    let mut world = super::World::new(42);
    world.settle_at_for_test(super::Building::Keep);
    let at_rest = super::rider_frame_key(&world);
    for tick in [0, 360, 720, 36_000] {
        world.tick = tick;
        assert_eq!(super::rider_frame_key(&world), at_rest);
    }
}

#[test]
fn cast_work_poses_follow_correlated_active_work_and_return_to_rest() {
    use crate::agent::harness::ToolEventId;
    use crate::stage::knight_cast::{Pose, Side};
    let mut world = super::World::new(42);
    let id = ToolEventId("cast-read".into());
    world.note_tool_call_event(id.clone(), "read_file", "src/main.rs");
    world.settle_at_for_test(super::Building::Scriptorium);
    assert_eq!(
        super::rider_frame_key(&world).pose(Side::BlueRight),
        Pose::Study
    );
    assert_eq!(
        super::rider_frame_key(&world).pose(Side::RedLeft),
        Pose::Mounted
    );
    // A visible label alone must never animate a craftsman at another site.
    world.settle_at_for_test(super::Building::Smithy);
    world.activity = "work on victory".into();
    assert_eq!(
        super::rider_frame_key(&world).pose(Side::RedLeft),
        Pose::Mounted
    );
    world.active_work.clear();
    world.settle_at_for_test(super::Building::Scriptorium);
    assert_eq!(
        super::rider_frame_key(&world).pose(Side::BlueRight),
        Pose::Mounted
    );
}

fn sprite_signature(sprite: &super::RaySprite) -> (u8, u32, u32, u32, u32, bool) {
    (
        sprite.id,
        sprite.x.to_bits(),
        sprite.y.to_bits(),
        sprite.width.to_bits(),
        sprite.height.to_bits(),
        sprite.lit,
    )
}

fn synthetic_scatter(
    seed: u64,
    origin_x: i32,
    origin_y: i32,
    span: u32,
    biome: super::Biome,
    terrain_kind: u8,
) -> Vec<super::RaySprite> {
    let len = (span * span) as usize;
    let mut sprites = Vec::new();
    super::scatter_scenery(
        seed,
        origin_x,
        origin_y,
        span,
        len / 2,
        &vec![0; len],
        &vec![biome; len],
        &vec![terrain_kind; len],
        &mut sprites,
    );
    sprites
}

#[test]
fn scenery_scatter_is_deterministic_world_anchored_and_capped() {
    let first = synthetic_scatter(
        42,
        100,
        200,
        31,
        super::Biome::Forest,
        super::raycast::TERRAIN_MEADOW,
    );
    let repeated = synthetic_scatter(
        42,
        100,
        200,
        31,
        super::Biome::Forest,
        super::raycast::TERRAIN_MEADOW,
    );
    assert_eq!(
        first.iter().map(sprite_signature).collect::<Vec<_>>(),
        repeated.iter().map(sprite_signature).collect::<Vec<_>>()
    );
    assert!(!first.is_empty());
    assert!(first.len() <= super::SCENERY_SPRITE_CAP);

    // This small window stays below the cap, so shared world cells must
    // have byte-identical sprites after a one-cell window shift.
    let left = synthetic_scatter(
        91,
        100,
        200,
        9,
        super::Biome::Forest,
        super::raycast::TERRAIN_MEADOW,
    );
    let right = synthetic_scatter(
        91,
        101,
        200,
        9,
        super::Biome::Forest,
        super::raycast::TERRAIN_MEADOW,
    );
    let world_signature = |sprite: &super::RaySprite, origin_x: i32| {
        (
            sprite.id,
            (sprite.x + origin_x as f32).to_bits(),
            (sprite.y + 200.0).to_bits(),
            sprite.width.to_bits(),
            sprite.height.to_bits(),
        )
    };
    let mut overlap_left: Vec<_> = left
        .iter()
        .filter(|sprite| sprite.x >= 1.0)
        .map(|sprite| world_signature(sprite, 100))
        .collect();
    let mut overlap_right: Vec<_> = right
        .iter()
        .filter(|sprite| sprite.x < 8.0)
        .map(|sprite| world_signature(sprite, 101))
        .collect();
    overlap_left.sort_unstable();
    overlap_right.sort_unstable();
    assert_eq!(overlap_left, overlap_right);
}

#[test]
fn scenery_scatter_obeys_surface_occupancy_and_camera_exclusions() {
    let roads = synthetic_scatter(
        42,
        0,
        0,
        31,
        super::Biome::Forest,
        super::raycast::TERRAIN_ROAD,
    );
    assert!(roads.is_empty(), "an all-road window must stay bare");

    let span = 31u32;
    let len = (span * span) as usize;
    let mut cells = vec![0; len];
    let mut terrain = vec![super::raycast::TERRAIN_MEADOW; len];
    let camera = len / 2;
    cells[1] = 4;
    terrain[2] = super::raycast::TERRAIN_ROAD;
    terrain[3] = super::raycast::TERRAIN_SAND;
    let mut sprites = vec![super::RaySprite {
        x: 4.5,
        y: 0.5,
        width: 1.0,
        height: 1.0,
        id: 0,
        lit: false,
    }];
    super::scatter_scenery(
        42,
        0,
        0,
        span,
        camera,
        &cells,
        &vec![super::Biome::Forest; len],
        &terrain,
        &mut sprites,
    );
    let scenery_cells: Vec<_> = sprites
        .iter()
        .filter(|sprite| sprite.id >= super::SCENERY_TREE_ID)
        .map(|sprite| (sprite.x.floor() as usize, sprite.y.floor() as usize))
        .collect();
    for forbidden in [(1, 0), (2, 0), (3, 0), (4, 0), (15, 15)] {
        assert!(!scenery_cells.contains(&forbidden));
    }
}

#[test]
fn district_inside_travel_window_emits_a_deterministic_waymarker() {
    let mut world = super::World::new(7);
    let anchor = (world.avatar_vis.0 as i32 + 2, world.avatar_vis.1 as i32 + 1);
    world.districts = vec![super::District {
        name: "Lantern".to_string(),
        anchor,
        banner_color: (52, 211, 153),
    }];

    let first = world.travel_scene();
    let marker = first
        .0
        .sprites
        .iter()
        .find(|sprite| sprite.id == super::DISTRICT_WAYMARKER_ID_BASE + 3)
        .expect("district in the travel window has a waymarker");
    let expected = sprite_signature(marker);
    let repeated = world.travel_scene();
    let repeated = repeated
        .0
        .sprites
        .iter()
        .find(|sprite| sprite.id == super::DISTRICT_WAYMARKER_ID_BASE + 3)
        .expect("repeated scene has the same waymarker");
    assert_eq!(expected, sprite_signature(repeated));
}

#[test]
fn village_sprites_use_window_offsets_and_live_state() {
    let mut world = super::World::new(7);
    world.avatar_vis = (30.0, 13.0);
    let heads = vec!["apollo".to_string(), "atlas".to_string()];
    world.enable_village(crate::stage::village::VillageState::default(), None, &heads);
    let village = world.village.as_mut().expect("village enabled");
    village.forge_pos = (33, 13);
    village.granary_pos = (29, 15);
    village.cottages[0].1 = (31, 12);
    village.cottages[1].1 = (28, 13);
    village.forge_up = true;
    village.training = true;
    village.heads_up[0].1 = true;

    let (map, _) = world.travel_scene();
    let origin_x = world.avatar_vis.0.round() as i32 - super::FP_WINDOW;
    let origin_y = world.avatar_vis.1.round() as i32 - super::FP_WINDOW;
    let sprites: Vec<_> = map
        .sprites
        .iter()
        .filter(|sprite| {
            matches!(
                sprite.id,
                super::VILLAGE_FORGE_ID | super::VILLAGE_GRANARY_ID | super::VILLAGE_COTTAGE_ID
            )
        })
        .collect();
    assert_eq!(sprites.len(), 4);
    for (id, pos, lit) in [
        (super::VILLAGE_FORGE_ID, (33, 13), true),
        (super::VILLAGE_GRANARY_ID, (29, 15), false),
        (super::VILLAGE_COTTAGE_ID, (31, 12), true),
        (super::VILLAGE_COTTAGE_ID, (28, 13), false),
    ] {
        assert!(sprites.iter().any(|sprite| {
            sprite.id == id
                && sprite.x == (pos.0 - origin_x) as f32 + 0.5
                && sprite.y == (pos.1 - origin_y) as f32 + 0.5
                && sprite.lit == lit
        }));
    }
    world.village.as_mut().unwrap().training = false;
    assert!(
        !world
            .travel_scene()
            .0
            .sprites
            .iter()
            .find(|sprite| sprite.id == super::VILLAGE_FORGE_ID)
            .expect("forge remains in window")
            .lit
    );
}

#[test]
fn disabled_village_leaves_stage_five_frame_stable() {
    let world = super::World::new(7);
    let (first_map, first_view) = world.travel_scene();
    let (second_map, second_view) = world.travel_scene();
    let first = super::super::world3d::render_region_frame(
        world.quest(),
        &first_map,
        &first_view,
        0.0,
        0,
        96,
        48,
    );
    let second = super::super::world3d::render_region_frame(
        world.quest(),
        &second_map,
        &second_view,
        0.0,
        0,
        96,
        48,
    );
    assert_eq!(first.as_raw(), second.as_raw());
    assert!(first_map.sprites.iter().all(|sprite| !matches!(
        sprite.id,
        super::VILLAGE_FORGE_ID | super::VILLAGE_GRANARY_ID | super::VILLAGE_COTTAGE_ID
    )));
}

#[test]
fn travel_scene_emits_landmarks_behind_their_facades() {
    let mut world = super::World::new(7);
    world.target = super::Building::Observatory;
    let keep = world.building_pos(super::Building::Keep);
    world.avatar_vis = (keep.0 as f32, keep.1 as f32);
    let scene = world.travel_scene();
    let origin_x = world.avatar_vis.0.floor() as i32 - (scene.0.width as i32 / 2);
    let origin_y = world.avatar_vis.1.floor() as i32 - (scene.0.height as i32 / 2);
    let visible: Vec<_> = world
        .buildings
        .iter()
        .filter(|(_, pos)| {
            let x = pos.0 as i32 - origin_x;
            let y = pos.1 as i32 - origin_y;
            x >= 0 && y >= 0 && x < scene.0.width as i32 && y < scene.0.height as i32
        })
        .collect();
    assert_eq!(
        scene
            .0
            .sprites
            .iter()
            .filter(|sprite| sprite.id < 8)
            .count(),
        visible.len()
    );
    for &&(building, pos) in &visible {
        let sprite = scene
            .0
            .sprites
            .iter()
            .find(|sprite| sprite.id == super::building_index(building))
            .expect("visible building has one landmark sprite");
        assert_eq!(sprite.lit, building == world.target);
        let facade = (
            pos.0 as f32 - origin_x as f32 + 0.5,
            pos.1 as f32 - origin_y as f32 + 0.5,
        );
        let axis = world.travel_facade_axis(pos);
        let normal = (-axis.1 as f32, axis.0 as f32);
        let displacement = (sprite.x - facade.0, sprite.y - facade.1);
        assert_eq!(displacement.0.abs() + displacement.1.abs(), 1.0);
        assert!((displacement.0 * normal.1 - displacement.1 * normal.0).abs() < f32::EPSILON);
    }
    let repeated = world.travel_scene();
    assert_eq!(world.cinematic_key(), world.cinematic_key());
    assert_eq!(scene.1.x.to_bits(), repeated.1.x.to_bits());
    assert_eq!(scene.1.y.to_bits(), repeated.1.y.to_bits());
}

use super::*;

const BUILDINGS: [Building; 8] = [
    Building::Keep,
    Building::Gatehouse,
    Building::Rookery,
    Building::Scriptorium,
    Building::Smithy,
    Building::Chapel,
    Building::RoundTable,
    Building::Observatory,
];

#[test]
fn settled_camera_uses_each_authored_vista_mark() {
    let mut world = World::new(42);
    for building in BUILDINGS {
        world.settle_at_for_test(building);
        // The deterministic table pose precedes the retained breathing sway.
        world.settle_ticks = 0;
        let (_, view) = world.travel_scene();
        let (camera, heading, eye_lift) = world.vista_camera_pose(building);
        let origin_x = world.avatar_vis.0.round() as i32 - super::FP_WINDOW;
        let origin_y = world.avatar_vis.1.round() as i32 - super::FP_WINDOW;
        assert!(
            (view.x - (camera.0 - origin_x as f32)).abs() < 0.001,
            "{building:?} x"
        );
        assert!(
            (view.y - (camera.1 - origin_y as f32)).abs() < 0.001,
            "{building:?} y"
        );
        assert!(
            (view.heading_rad.sin() - heading.sin()).abs() < 0.001
                && (view.heading_rad.cos() - heading.cos()).abs() < 0.001,
            "{building:?} heading"
        );
        assert!(view.eye_h >= eye_lift, "{building:?} lift");
    }
}

#[test]
fn vista_marks_are_authored_safe_and_alternate_thirds() {
    for (index, mark) in super::VISTA_MARKS.iter().enumerate() {
        assert!((3.0..=6.5).contains(&mark.standoff), "mark {index}");
        assert!(mark.thirds_offset_rad.abs() >= 0.17, "mark {index}");
        if index > 0 {
            assert_ne!(
                mark.thirds_offset_rad.signum(),
                super::VISTA_MARKS[index - 1].thirds_offset_rad.signum(),
                "mark {index} alternates its thirds anchor"
            );
        }
    }
}

#[test]
fn approach_camera_keeps_door_pull_up_as_a_hard_minimum() {
    let mut world = World::new(42);
    world.target = Building::Chapel;
    let door = world.building_pos(world.target);
    world.avatar = door;
    world.avatar_vis = (door.0 as f32 - 0.2, door.1 as f32);
    let (_, view) = world.travel_scene();
    let origin_x = world.avatar_vis.0.round() as i32 - super::FP_WINDOW;
    let origin_y = world.avatar_vis.1.round() as i32 - super::FP_WINDOW;
    let camera_world = (view.x + origin_x as f32, view.y + origin_y as f32);
    let dx = camera_world.0 - (door.0 as f32 + 0.5);
    let dy = camera_world.1 - (door.1 as f32 + 0.5);
    assert!((dx * dx + dy * dy).sqrt() + 0.001 >= super::DOOR_PULL_UP);
}

#[test]
fn settled_destinations_emit_their_deterministic_props_only() {
    let expected = [
        (Building::Keep, 5, 2),
        (Building::Gatehouse, 6, 1),
        (Building::Rookery, 4, 1),
        (Building::Scriptorium, 7, 1),
        (Building::Smithy, 0, 2),
        (Building::Chapel, 1, 2),
        (Building::RoundTable, 2, 5),
        (Building::Observatory, 3, 1),
    ];
    for (building, prop_kind, count) in expected {
        let mut world = World::new(42);
        world.target = building;
        let pos = world.building_pos(building);
        world.avatar = pos;
        world.avatar_vis = (pos.0 as f32, pos.1 as f32);
        world.settle_ticks = 0;
        assert!(
            world
                .travel_scene()
                .0
                .sprites
                .iter()
                .all(|s| !(PROP_ID_BASE..PROP_ID_BASE + 8).contains(&s.id))
        );

        world.settle_ticks = 2;
        let props = |world: &World| {
            world
                .travel_scene()
                .0
                .sprites
                .into_iter()
                .filter(|s| (PROP_ID_BASE..PROP_ID_BASE + 8).contains(&s.id))
                .map(|s| {
                    (
                        s.id,
                        s.x.to_bits(),
                        s.y.to_bits(),
                        s.width.to_bits(),
                        s.height.to_bits(),
                        s.lit,
                    )
                })
                .collect::<Vec<_>>()
        };
        let first = props(&world);
        assert_eq!(first.len(), count, "{building:?} prop count");
        assert!(first.iter().all(|p| p.0 == PROP_ID_BASE + prop_kind));
        assert_eq!(first, props(&world), "{building:?} props must be stable");
    }
}

#[test]
fn settled_key_is_stable_for_unchanged_inputs_and_tracks_door_pulse_bucket() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = World::new(42);
    world.target = Building::Smithy;
    world.avatar = world.building_pos(Building::Smithy);
    world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
    world.settle_ticks = 2;
    let unchanged = world.cinematic_key();
    assert_eq!(unchanged, world.cinematic_key());
    world.settle_ticks = 3;
    assert_ne!(unchanged, world.cinematic_key());
}

#[test]
fn first_person_key_tracks_heading_and_departure_fade() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = World::new(42);
    world.target = Building::Observatory;
    world.avatar = world.building_pos(Building::Keep);
    world.avatar_vis = (world.avatar.0 as f32 - 3.0, world.avatar.1 as f32);
    let base = world.cinematic_key();
    world.travel_heading += std::f32::consts::TAU / 16.0;
    let turned = world.cinematic_key();
    assert_ne!(base, turned, "a heading swing must re-render the frame");
    world.travel_ticks = 1;
    let fading = world.cinematic_key();
    assert_ne!(turned, fading, "departure dissolve steps must re-render");
    world.travel_ticks = DEPART_FADE_TICKS + 5;
    let settled = world.cinematic_key();
    world.travel_ticks = DEPART_FADE_TICKS + 9;
    assert_eq!(
        settled,
        world.cinematic_key(),
        "a finished dissolve must stop keying on travel_ticks"
    );
}

#[test]
fn first_person_landmarks_have_doored_fronts_and_deep_footprints() {
    let mut world = World::new(42);
    for building in BUILDINGS {
        world.target = building;
        let pos = world.building_pos(building);
        // Keep the saddle several cells off the frontage so its mandatory
        // open camera cell cannot erase any part of the kit under test.
        world.avatar_vis = (pos.0 as f32 + 6.0, pos.1 as f32 + 6.0);
        let origin_x = world.avatar_vis.0.round() as i32 - FP_WINDOW;
        let origin_y = world.avatar_vis.1.round() as i32 - FP_WINDOW;
        let (map, _) = world.travel_scene();
        let center = (pos.0 as i32 - origin_x, pos.1 as i32 - origin_y);
        assert_eq!(
            map.at(center.0, center.1),
            raycast::MATERIAL_DOOR,
            "{building:?} keeps an accent door at its center"
        );
        let axis = world.travel_facade_axis(pos);
        let behind = world.travel_facade_behind(pos);
        let footprint = travel_landmark_footprint(building);
        let wing = travel_facade_material(building);
        assert!(
            footprint.depth >= 2,
            "{building:?} needs enough depth to expose a return wall"
        );
        for depth in 0..=footprint.depth {
            for side in [-footprint.half_width, footprint.half_width] {
                assert_eq!(
                    map.at(
                        center.0 + axis.0 * side + behind.0 * depth,
                        center.1 + axis.1 * side + behind.1 * depth,
                    ),
                    wing,
                    "{building:?} needs a side wall at depth {depth} on side {side}"
                );
            }
        }
        for offset in -footprint.half_width..=footprint.half_width {
            assert_eq!(
                map.at(
                    center.0 + axis.0 * offset + behind.0 * footprint.depth,
                    center.1 + axis.1 * offset + behind.1 * footprint.depth,
                ),
                wing,
                "{building:?} needs a closed rear wall at offset {offset}"
            );
        }
        let skyline = map
            .sprites
            .iter()
            .find(|sprite| sprite.id == building_index(building))
            .expect("landmark keeps its skyline billboard");
        assert_eq!(
            (skyline.x, skyline.y),
            (
                center.0 as f32 + behind.0 as f32 + 0.5,
                center.1 as f32 + behind.1 as f32 + 0.5,
            )
        );
        assert_eq!(skyline.width, (footprint.half_width * 2 + 1) as f32);
    }
}

#[test]
fn quarter_view_geometry_retains_front_and_return_walls() {
    use super::raycast::{RayMap, TERRAIN_MEADOW};
    const FRONT_PROBE: u8 = 250;
    const RETURN_PROBE: u8 = 251;
    let building = Building::RoundTable;
    let mut world = World::new(42);
    world.target = building;
    let pos = world.building_pos(building);
    let axis = world.travel_facade_axis(pos);
    let front = world.travel_facade_front(pos);
    let behind = world.travel_facade_behind(pos);
    let footprint = travel_landmark_footprint(building);
    // Put the window around the same front-and-side diagonal used below.
    world.avatar_vis = (
        pos.0 as f32 + front.0 * 7.0 + axis.0 as f32 * 4.0,
        pos.1 as f32 + front.1 * 7.0 + axis.1 as f32 * 4.0,
    );
    let origin_x = world.avatar_vis.0.round() as i32 - FP_WINDOW;
    let origin_y = world.avatar_vis.1.round() as i32 - FP_WINDOW;
    let (scene, _) = world.travel_scene();
    let center = (pos.0 as i32 - origin_x, pos.1 as i32 - origin_y);
    let len = (scene.width * scene.height) as usize;
    let mut probe = RayMap {
        width: scene.width,
        height: scene.height,
        cells: vec![0; len],
        terrain: vec![0.0; len],
        terrain_kind: vec![TERRAIN_MEADOW; len],
        sprites: Vec::new(),
    };
    for depth in 0..=footprint.depth {
        for offset in -footprint.half_width..=footprint.half_width {
            if depth != 0 && depth != footprint.depth && offset.abs() != footprint.half_width {
                continue;
            }
            let x = center.0 + axis.0 * offset + behind.0 * depth;
            let y = center.1 + axis.1 * offset + behind.1 * depth;
            assert_ne!(
                scene.at(x, y),
                0,
                "production footprint is missing ({offset}, {depth})"
            );
            probe.cells[y as usize * probe.width as usize + x as usize] = if depth == 0 {
                FRONT_PROBE
            } else {
                RETURN_PROBE
            };
        }
    }

    assert!(
        probe.cells.contains(&FRONT_PROBE),
        "quarter view must retain frontage"
    );
    assert!(
        probe.cells.contains(&RETURN_PROBE),
        "quarter view must retain depth"
    );
}

#[test]
fn arrival_camera_hold_steps_the_key() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = World::new(43);
    world.target = Building::Smithy;
    world.avatar = world.building_pos(Building::Smithy);
    world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
    world.settle_ticks = 0;
    let landing = world.cinematic_key();
    world.settle_ticks = 2;
    assert_ne!(
        landing,
        world.cinematic_key(),
        "the in-scene camera hold must animate the door pulse and prop reveal"
    );
}

#[test]
fn travel_heightfield_stays_low_finite_and_open() {
    for seed in [7, 42] {
        let world = World::new(seed);
        let (map, _) = world.travel_scene();
        assert!(map.terrain.iter().all(|height| height.is_finite()));
        for (cell, height) in map.cells.iter().zip(&map.terrain) {
            if *cell != 0 {
                assert_eq!(*height, 0.0, "non-open cells must stay flat");
            }
        }
        let mean = map.terrain.iter().sum::<f32>() / map.terrain.len() as f32;
        let max = map.terrain.iter().copied().fold(0.0f32, f32::max);
        assert!(mean < 0.35, "seed {seed} mean terrain height {mean}");
        assert!(max < 0.75, "seed {seed} max terrain height {max}");
    }
}

#[test]
fn tile_elevation_cache_matches_tile_centres() {
    let world = World::new(42);
    for (x, y) in [
        (0, 0),
        (1, 1),
        (WORLD_W / 2, WORLD_H / 2),
        (17, 9),
        (WORLD_W - 1, WORLD_H - 1),
    ] {
        let cached = world.tile_elevation[y * WORLD_W + x] as f64;
        let sampled = world.elevation_at(x as f64 + 0.5, y as f64 + 0.5);
        assert!((cached - sampled).abs() <= 1e-6, "tile ({x}, {y})");
    }
}

#[test]
fn travel_scene_marks_road_cells() {
    for seed in [7, 42] {
        let world = World::new(seed);
        let origin_x = world.avatar_vis.0.round() as i32 - FP_WINDOW;
        let origin_y = world.avatar_vis.1.round() as i32 - FP_WINDOW;
        let (map, _) = world.travel_scene();
        let mut roads = 0;
        for map_y in 0..map.height as i32 {
            for map_x in 0..map.width as i32 {
                let world_x = origin_x + map_x;
                let world_y = origin_y + map_y;
                if world_x >= 0
                    && world_y >= 0
                    && world_x < WORLD_W as i32
                    && world_y < WORLD_H as i32
                    && world.tile_at_world(world_x as f32 + 0.5, world_y as f32 + 0.5)
                        == Biome::Path
                    && map.at(map_x, map_y) == 0
                {
                    roads += 1;
                    assert_eq!(map.terrain_kind_at(map_x, map_y), raycast::TERRAIN_ROAD);
                }
            }
        }
        assert!(roads > 0, "seed {seed} travel window must contain a road");
    }
}

fn ambient_ids(scene: &super::raycast::RayMap) -> Vec<u8> {
    scene
        .sprites
        .iter()
        .filter_map(|sprite| {
            matches!(
                sprite.id,
                super::RAVEN_SPRITE_ID | super::SMOKE_SPRITE_ID | super::VILLAGER_SPRITE_ID
            )
            .then_some(sprite.id)
        })
        .collect()
}

#[test]
fn ravens_follow_only_live_research_work() {
    let mut world = super::super::World::new(71);
    world.note_tool_call("science_search", "web sources");
    let landmark = world.active_research_landmark().expect("research route");
    world.settle_at_for_test(landmark);
    let live = ambient_ids(&world.travel_scene().0);
    assert!(
        (2..=super::RAVEN_SPRITE_CAP).contains(
            &live
                .iter()
                .filter(|&&id| id == super::RAVEN_SPRITE_ID)
                .count()
        )
    );
    world.active_work.clear();
    assert!(!ambient_ids(&world.travel_scene().0).contains(&super::RAVEN_SPRITE_ID));
}

#[test]
fn smoke_follows_only_live_build_work() {
    let mut world = super::super::World::new(72);
    world.note_tool_call("shell", "cargo test world_viz");
    let landmark = world.active_build_landmark().expect("build route");
    world.settle_at_for_test(landmark);
    assert_eq!(
        ambient_ids(&world.travel_scene().0)
            .iter()
            .filter(|&&id| id == super::SMOKE_SPRITE_ID)
            .count(),
        super::SMOKE_SPRITE_CAP
    );
    world.active_work.clear();
    assert!(!ambient_ids(&world.travel_scene().0).contains(&super::SMOKE_SPRITE_ID));
}

#[test]
fn healthy_villager_walk_is_deterministic_and_down_head_stays_home() {
    let ids = vec!["dice".to_string()];
    let mut first = super::super::World::new(73);
    let mut second = super::super::World::new(73);
    first.enable_village(crate::stage::village::VillageState::default(), None, &ids);
    second.enable_village(crate::stage::village::VillageState::default(), None, &ids);
    for world in [&mut first, &mut second] {
        let village = world.village.as_mut().unwrap();
        village.heads_up[0].1 = true;
        let granary = village.granary_pos;
        world.avatar = granary;
        world.avatar_vis = (granary.0 as f32, granary.1 as f32);
        world.tick = 80;
    }
    let position = |world: &super::super::World| {
        world
            .travel_scene()
            .0
            .sprites
            .iter()
            .find(|sprite| sprite.id == super::VILLAGER_SPRITE_ID)
            .map(super::tests::sprite_signature)
    };
    assert_eq!(position(&first), position(&second));
    first.village.as_mut().unwrap().heads_up[0].1 = false;
    assert_eq!(position(&first), None);
}

#[test]
fn first_person_scene_never_contains_knight_self_sprite_and_ambient_is_capped() {
    let ids: Vec<String> = (0..32).map(|index| format!("head-{index}")).collect();
    let mut world = super::super::World::new(74);
    world.enable_village(crate::stage::village::VillageState::default(), None, &ids);
    for (_, up) in &mut world.village.as_mut().unwrap().heads_up {
        *up = true;
    }
    world.note_tool_call("science_search", "web sources");
    world.note_tool_call("shell", "cargo test");
    let landmark = world.active_build_landmark().unwrap();
    world.settle_at_for_test(landmark);
    let ids = ambient_ids(&world.travel_scene().0);
    assert!(ids.len() <= super::AMBIENT_SPRITE_CAP);
    assert!(
        !world
            .travel_scene()
            .0
            .sprites
            .iter()
            .any(|sprite| sprite.id == super::KNIGHT_SELF_SPRITE_ID)
    );
}

#[test]
fn cinematic_key_tracks_ambient_bits_and_only_slow_bucket_crossings() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = super::super::World::new(75);
    world.note_tool_call("science_search", "web sources");
    let landmark = world.active_research_landmark().unwrap();
    world.settle_at_for_test(landmark);
    // Stay clear of both the ambient and rider frame boundaries: this
    // assertion isolates active-work keying rather than animation motion.
    world.tick = super::AMBIENT_TICK_TICKS * 100 + 1;
    let live = world.cinematic_key();
    world.tick += 1;
    assert_eq!(live, world.cinematic_key());
    world.active_work.clear();
    assert_ne!(live, world.cinematic_key());
}

#[test]
fn completion_ceremony_keys_and_emits_only_its_capped_keep_pennants() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = World::new(42);
    world.avatar = world.building_pos(Building::Keep);
    world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
    let idle_key = world.cinematic_key();
    assert!(
        !world
            .travel_scene()
            .0
            .sprites
            .iter()
            .any(|sprite| sprite.id == CEREMONY_PENNANT_ID)
    );

    world.set_completion_ceremony_active(true);
    assert_ne!(idle_key, world.cinematic_key());
    assert_eq!(
        world
            .travel_scene()
            .0
            .sprites
            .iter()
            .filter(|sprite| sprite.id == CEREMONY_PENNANT_ID)
            .count(),
        2
    );
    world.set_completion_ceremony_active(false);
    assert_eq!(idle_key, world.cinematic_key());
}

#[test]
fn cinematic_key_tracks_only_hearth_render_bucket_crossings() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = World::new(42);
    world.target = Building::Smithy;
    world.avatar = world.building_pos(Building::Smithy);
    world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
    world.settle_ticks = 2;

    let mut within = None;
    let mut crossing = None;
    let mut previous = (0, world.hearth.render_bucket(world.seed));
    for beats in 1..20_000 {
        world.hearth.beats = beats;
        let bucket = world.hearth.render_bucket(world.seed);
        if bucket == previous.1 && within.is_none() {
            within = Some((previous.0, beats));
        } else if bucket != previous.1 && crossing.is_none() {
            crossing = Some((previous.0, beats));
        }
        if within.is_some() && crossing.is_some() {
            break;
        }
        previous = (beats, bucket);
    }

    let (same_a, same_b) = within.expect("clock must have drift inside a render bucket");
    world.hearth.beats = same_a;
    let same_key = world.cinematic_key();
    world.hearth.beats = same_b;
    assert_eq!(
        same_key,
        world.cinematic_key(),
        "within-bucket beat drift must preserve the cached frame"
    );

    let (old_beats, new_beats) = crossing.expect("clock must cross a render bucket");
    world.hearth.beats = old_beats;
    let old_key = world.cinematic_key();
    world.hearth.beats = new_beats;
    assert_ne!(
        old_key,
        world.cinematic_key(),
        "a daylight or weather bucket crossing must re-march"
    );
}

#[test]
fn hill_cells_are_open_tall_rock_terrain() {
    let mut world = super::World::new(7);
    for y in 0..super::WORLD_H as i32 {
        for x in 0..super::WORLD_W as i32 {
            if !matches!(
                world.natural_biome_at(f64::from(x) + 1.0 / 32.0, f64::from(y) + 1.0 / 32.0),
                super::Biome::Hill | super::Biome::Peak
            ) {
                continue;
            }
            world.avatar_vis = (x as f32, y as f32);
            let (map, _) = world.travel_scene();
            let centre = (super::FP_WINDOW as u32 * map.width + super::FP_WINDOW as u32) as usize;
            if map.cells[centre] == 0
                && map.terrain[centre] >= 0.55
                && map.terrain_kind[centre] == super::raycast::TERRAIN_HILL
            {
                return;
            }
        }
    }
    panic!("seed 7 must expose an open, tall Hill/Peak cell in the FP window");
}

#[test]
fn travel_eye_height_tracks_smoothed_ground_deterministically() {
    let mut first = super::World::new(7);
    let mut high = (0, 0, 0.0_f64);
    for y in 0..super::WORLD_H as i32 {
        for x in 0..super::WORLD_W as i32 {
            let elevation = first.smoothed_travel_elevation(x, y);
            if elevation > high.2 {
                high = (x, y, elevation);
            }
        }
    }
    assert!(high.2 > 0.0, "seed 7 must contain measurably high ground");
    first.avatar_vis = (high.0 as f32, high.1 as f32);
    let mut repeated = super::World::new(7);
    repeated.avatar_vis = first.avatar_vis;
    let first_eye = first.travel_scene().1.eye_h;
    let repeated_eye = repeated.travel_scene().1.eye_h;
    assert!(first_eye > 0.0);
    assert_eq!(first_eye.to_bits(), repeated_eye.to_bits());

    let mut flat = super::World::new(7);
    flat.tile_elevation.fill(0.0);
    flat.avatar_vis = (32.0, 13.0);
    let flat_eye = flat.travel_scene().1.eye_h;
    assert!(flat_eye.is_finite() && (0.0..=1.0).contains(&flat_eye));
}

#[test]
fn eye_relief_lowers_the_same_crest_from_high_ground() {
    let mut world = super::World::new(7);
    let (crest_x, crest_y) = (10..super::WORLD_W as i32 - 10)
        .flat_map(|x| (3..super::WORLD_H as i32 - 3).map(move |y| (x, y)))
        .find(|&(x, y)| {
            matches!(
                world.natural_biome_at(f64::from(x) + 1.0 / 32.0, f64::from(y) + 1.0 / 32.0),
                super::Biome::Grass
            ) && matches!(world.at(x as usize, y as usize), super::Biome::Grass)
        })
        .expect("seed 7 has meadow ground away from map edges");
    world.tile_elevation.fill(0.0);
    let crest_idx = crest_y as usize * super::WORLD_W + crest_x as usize;
    world.tile_elevation[crest_idx] = 1.0;

    let terrain_at = |world: &super::World, camera_x: i32| {
        let (map, _) = world.travel_scene();
        let origin_x = camera_x - super::FP_WINDOW;
        let origin_y = crest_y - super::FP_WINDOW;
        let local_x = (crest_x - origin_x) as u32;
        let local_y = (crest_y - origin_y) as u32;
        map.terrain[(local_y * map.width + local_x) as usize]
    };

    world.avatar_vis = ((crest_x - 8) as f32, crest_y as f32);
    let from_low = terrain_at(&world, crest_x - 8);
    world.avatar_vis = (crest_x as f32, crest_y as f32);
    let from_high = terrain_at(&world, crest_x);
    assert!(
        from_high < from_low,
        "eye-relative relief must lower the same crest: high={from_high}, low={from_low}"
    );
}
