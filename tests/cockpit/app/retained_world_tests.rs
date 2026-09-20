#[cfg(test)]
mod retained_world {
    use crate::world_viz::{Building, World, world3d};

    #[test]
    fn dotmax_outdoors_and_all_eight_room_plates_round_trip() {
        let _env = crate::tests::env_lock();
        assert_eq!(world3d::current(), world3d::WorldView::Mesh3d);
        assert_eq!(
            world3d::WorldView::parse("3d"),
            Some(world3d::WorldView::Mesh3d)
        );
        let _view = world3d::pin_world3d();
        for building in [
            Building::Keep,
            Building::Gatehouse,
            Building::Rookery,
            Building::Scriptorium,
            Building::Smithy,
            Building::Chapel,
            Building::RoundTable,
            Building::Observatory,
        ] {
            let mut world = World::new(71);
            world.settle_at_for_test(building);
            assert!(!world.ambient_interior_visible());
            let stage = crate::scryglass::Scryglass::for_world(building);
            assert_eq!(
                stage.controller.route(),
                crate::scryglass::StageRoute::Explore(building)
            );
            let before = world
                .scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05)
                .expect("Dotmax outdoors");
            assert!(
                before
                    .cells
                    .iter()
                    .any(|cell| cell.glyph != ' ' && cell.glyph != '\u{2800}')
            );
            assert!(world.enter_interior(), "{building:?}: entry");
            assert!(
                world.ambient_interior_visible(),
                "{building:?}: room plate visible"
            );
            let frame = world.ambient_frame(crate::lifecycle_viz::MotionMode::Off);
            assert_eq!(frame.pixels.as_ref().len(), 256 * 224 * 4);
            assert!(
                frame
                    .pixels
                    .as_ref()
                    .chunks_exact(4)
                    .map(|rgba| [rgba[0], rgba[1], rgba[2]])
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    > 8
            );
            assert!(world.leave_interior());
            assert!(!world.ambient_interior_visible());
            assert_eq!(world3d::current(), world3d::WorldView::Mesh3d);
            let after = world
                .scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05)
                .expect("Dotmax restored");
            assert_eq!(
                before
                    .cells
                    .iter()
                    .map(|c| (c.glyph, c.fg))
                    .collect::<Vec<_>>(),
                after
                    .cells
                    .iter()
                    .map(|c| (c.glyph, c.fg))
                    .collect::<Vec<_>>()
            );
        }
    }
}
