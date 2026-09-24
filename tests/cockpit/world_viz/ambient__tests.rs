use super::*;

#[test]
fn dotmax_arrival_requires_entry_and_leaving_restores_the_world() {
    let _env = crate::tests::env_lock();
    let _ink = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
    let _view = super::super::world3d::pin(super::super::world3d::WorldView::Mesh3d);
    for building in BUILDINGS {
        let mut world = World::new(71);
        world.select_landmark(building);
        assert!(!world.ambient_interior_visible(), "travel stays in Dotmax");
        for _ in 0..500 {
            world.tick();
        }
        assert!(!world.ambient_interior_visible(), "arrival is not entry");
        let outdoors = world
            .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
            .unwrap();
        assert!(world.enter_interior());
        assert!(world.ambient_interior_visible());
        assert!(
            world.ambient_interior_visible(),
            "no map actor in the painting"
        );
        let frame = world.ambient_frame(MotionMode::Off);
        assert_eq!(
            frame.pixels.as_ref(),
            plate(building).as_raw(),
            "interior is the approved painting without a placed knight"
        );
        let entered = world
            .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
            .unwrap();
        assert_ne!(
            entered, outdoors,
            "{building:?}: relaxed entry must replace outdoors"
        );
        let reused = world
            .scryglass_frame_paced(40, 16, true, 0.3, 0.2, 1.30)
            .unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&entered, &reused),
            "room art ignores camera offsets"
        );
        let resized = world
            .scryglass_frame_paced(32, 14, true, 0.0, 0.0, 1.05)
            .unwrap();
        assert_eq!((resized.width, resized.height), (32, 14));
        assert!(!std::sync::Arc::ptr_eq(&entered, &resized));
        // Put a same-size room frame back into the cache immediately before
        // leaving, so a stale relaxed-cache hit cannot hide behind a resize.
        let entered = world
            .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
            .unwrap();
        assert!(world.leave_interior());
        assert!(!world.ambient_interior_visible());
        let returned = world
            .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
            .unwrap();
        assert_eq!(
            returned, outdoors,
            "{building:?}: same-tick leave restores the Dotmax frame"
        );
        assert!(!std::sync::Arc::ptr_eq(&entered, &returned));
        assert_eq!(
            super::super::world3d::current(),
            super::super::world3d::WorldView::Mesh3d
        );
    }
}

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
fn living_paintings_keep_architecture_fixed_and_use_material_colors() {
    for building in BUILDINGS {
        let base = plate(building);
        let location =
            &matrix().locations[super::super::cinematics::building_index(building) as usize];
        let mut changes = 0;
        for tick in (0..160).step_by(10) {
            let animated = frame(building, Pose::at(tick, MotionMode::Full));
            for (x, y, pixel) in animated.enumerate_pixels() {
                if pixel != base.get_pixel(x, y) {
                    changes += 1;
                    assert!(
                        location.effects.iter().any(|effect| {
                            let [rx, ry, w, h] = effect.rect;
                            x >= rx && x < rx + w && y >= ry && y < ry + h
                        }),
                        "architecture moved in {building:?} at {x},{y}"
                    );
                    assert_eq!(pixel[3], 255);
                    let rgb = [pixel[0], pixel[1], pixel[2]];
                    assert!(WARM.contains(&rgb) || WATER.contains(&rgb));
                }
            }
        }
        assert!(changes > 0, "{building:?} has no visible animated material");
        assert_eq!(frame(building, Pose::at(900, MotionMode::Off)), *base);
        assert_eq!(
            frame(building, Pose::at(0, MotionMode::Full)),
            frame(building, Pose::at(160, MotionMode::Full)),
            "loop seam"
        );
    }
}

#[test]
fn native_paintings_preserve_source_detail_and_localized_motion_masks() {
    for building in BUILDINGS {
        let source = detail_plate(building);
        assert_eq!(source.dimensions(), (DETAIL_WIDTH, DETAIL_HEIGHT));
        assert!(
            std::ptr::eq(source, detail_plate(building)),
            "bounded cache reuses its plate"
        );
        assert_eq!(
            detail_frame(building, Pose::at(900, MotionMode::Off)),
            *source
        );
        let colours: std::collections::HashSet<_> = source.pixels().map(|pixel| pixel.0).collect();
        assert!(
            colours.len() > 75,
            "native art must retain original colours"
        );
        let base = plate(building);
        for mode in [MotionMode::Reduced, MotionMode::Full] {
            let mut changes = 0;
            for tick in [0, 40, 80, 120] {
                let pose = Pose::at(tick, mode);
                let low = frame(building, pose);
                let high = detail_frame(building, pose);
                for (x, y, pixel) in high.enumerate_pixels() {
                    assert_eq!(pixel[3], 255);
                    if pixel != source.get_pixel(x, y) {
                        changes += 1;
                        assert_ne!(
                            low.get_pixel(x / 3, y / 3),
                            base.get_pixel(x / 3, y / 3),
                            "native animation escaped its admitted mask in {building:?}"
                        );
                    }
                }
            }
            assert!(
                changes > 0,
                "native {building:?} silently lost its material animation"
            );
        }
        assert_eq!(
            detail_frame(building, Pose::at(0, MotionMode::Full)),
            detail_frame(building, Pose::at(160, MotionMode::Full)),
            "native loop seam"
        );
    }
}

#[test]
fn native_ambient_frame_keeps_room_identity_and_declared_pixel_dimensions() {
    let _pin = super::super::world3d::pin(super::super::world3d::WorldView::Mesh3d);
    let mut world = World::new(71);
    for building in BUILDINGS {
        world.interior = Some(building);
        let frame = world.ambient_detail_frame(MotionMode::Off);
        assert_eq!((frame.width, frame.height), (DETAIL_WIDTH, DETAIL_HEIGHT));
        let pixels = frame.pixels.as_ref();
        assert_eq!(pixels.len(), (frame.width * frame.height * 4) as usize);
        assert_eq!(pixels, detail_plate(building).as_raw());
        assert!(std::ptr::eq(pixels, frame.pixels.as_ref()));
    }
}

#[test]
fn room_motion_cache_is_paced_and_off_freezes_paintings() {
    let mut world = World::new(71);
    world.settle_at_for_test(Building::Keep);
    assert!(world.enter_interior());
    let key = world.ambient_scene_sequence(MotionMode::Full);
    let still = world
        .ambient_braille_frame(30, 12, MotionMode::Off)
        .unwrap();
    let pixels = world
        .ambient_frame(MotionMode::Off)
        .pixels
        .as_ref()
        .to_vec();
    world.tick = 9;
    assert_eq!(key, world.ambient_scene_sequence(MotionMode::Full));
    world.tick = 10;
    assert_ne!(key, world.ambient_scene_sequence(MotionMode::Full));
    assert!(std::sync::Arc::ptr_eq(
        &still,
        &world
            .ambient_braille_frame(30, 12, MotionMode::Off)
            .unwrap()
    ));
    assert_eq!(pixels, world.ambient_frame(MotionMode::Off).pixels.as_ref());
    let moving = world
        .ambient_braille_frame(30, 12, MotionMode::Full)
        .unwrap();
    assert!(!std::sync::Arc::ptr_eq(&still, &moving));
    world.tick = 19;
    assert!(std::sync::Arc::ptr_eq(
        &moving,
        &world
            .ambient_braille_frame(30, 12, MotionMode::Full)
            .unwrap()
    ));
    let reduced = frame(Building::Keep, Pose::at(240, MotionMode::Reduced));
    let base = plate(Building::Keep);
    let [x, y, w, h] = matrix().locations[0]
        .effects
        .iter()
        .find(|e| matches!(e.kind, Kind::Water))
        .unwrap()
        .rect;
    for py in y..y + h {
        for px in x..x + w {
            assert_eq!(reduced.get_pixel(px, py), base.get_pixel(px, py));
        }
    }
}

#[test]
#[ignore = "manual living-painting art review: set REALM_AMBIENT_DUMP"]
fn dump_living_paintings_for_review() {
    let out = std::path::PathBuf::from(
        std::env::var("REALM_AMBIENT_DUMP").expect("set REALM_AMBIENT_DUMP"),
    );
    std::fs::create_dir_all(&out).unwrap();
    for building in BUILDINGS {
        let name =
            &matrix().locations[super::super::cinematics::building_index(building) as usize].id;
        for phase in 0..16 {
            frame(building, Pose::at(phase * 10, MotionMode::Full))
                .save(out.join(format!("{name}-{phase:02}.png")))
                .unwrap();
        }
    }
}

#[test]
fn all_locations_have_distinct_bounded_opaque_art_without_state_colors() {
    let palette: serde_json::Value =
        serde_json::from_str(include_str!("../../../cockpit/assets/realm/palette.json")).unwrap();
    let materials: std::collections::HashSet<[u8; 3]> = palette["banks"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(name, _)| name.as_str() != "signal")
        .flat_map(|(_, bank)| bank["colors"].as_array().unwrap())
        .map(|value| {
            let text = value.as_str().unwrap();
            [1, 3, 5].map(|offset| u8::from_str_radix(&text[offset..offset + 2], 16).unwrap())
        })
        .collect();
    for (index, building) in BUILDINGS.into_iter().enumerate() {
        assert!(
            PLATES[index].len() < 128 * 1024,
            "{building:?} compressed budget"
        );
        let image = plate(building);
        assert_eq!(image.dimensions(), (256, 224));
        assert_eq!(image.as_raw().len(), 229_376);
        assert!(image.pixels().all(
                |pixel| pixel[3] == 255 && materials.contains(&[pixel[0], pixel[1], pixel[2]])
            ));
        assert!(std::ptr::eq(image, plate(building)), "decode is retained");
        for prior in &BUILDINGS[..index] {
            assert_ne!(
                image.as_raw(),
                plate(*prior).as_raw(),
                "locations share a plate"
            );
        }
    }
}

#[test]
fn ambient_fallback_is_visible_narrow_and_reuses_static_frames() {
    let mut world = World::new(0xA11CE);
    for building in BUILDINGS {
        world.settle_at_for_test(building);
        assert!(world.enter_interior());
        let first = world
            .scryglass_frame_paced(20, 7, false, 0.0, 0.0, 1.05)
            .unwrap();
        assert_eq!((first.width, first.height), (20, 7));
        assert!(first.cells.iter().any(|cell| cell.glyph != '\u{2800}'));
        world.tick();
        let second = world
            .scryglass_frame_paced(20, 7, true, 0.4, 0.2, 1.30)
            .unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&first, &second),
            "still art does not animate"
        );
    }
    assert!(
        world
            .scryglass_frame_paced(0, 7, false, 0.0, 0.0, 1.05)
            .is_none()
    );
}

#[test]
fn real_tool_event_leaves_the_entered_plate_before_rendering_new_outdoors() {
    let mut world = World::new(0xA11CE);
    world.settle_at_for_test(Building::Chapel);
    assert!(world.enter_interior());
    let first = world
        .scryglass_frame_paced(32, 14, false, 0.0, 0.0, 1.05)
        .unwrap();
    assert_eq!(world.ambient_building(), Building::Chapel);
    world.note_tool_call_event(
        crate::agent::harness::ToolEventId("ambient-read-event".to_string()),
        "read_file",
        "path=research.md",
    );
    assert_eq!(
        world.ambient_building(),
        Building::Chapel,
        "entered room stays authoritative until leave"
    );
    world.tick();
    assert!(
        !world.inside_interior(),
        "new work retains the established room-exit lifecycle"
    );
    assert_eq!(world.ambient_building(), Building::Scriptorium);
    let outside = world
        .scryglass_frame_paced(32, 14, true, 0.0, 0.0, 1.05)
        .unwrap();
    assert!(
        !std::sync::Arc::ptr_eq(&first, &outside),
        "leaving must not reuse the plate in relaxed mode"
    );
    assert_eq!(
        super::super::world3d::current(),
        super::super::world3d::WorldView::Mesh3d
    );
}
