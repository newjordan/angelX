use super::*;

/// Exercise the real Stage router while the native image worker would have
/// enough time to replace the first Dotmax fallback. All rooms, two layouts,
/// animated ticks and a leave/re-enter cycle must retain dot glyphs.
#[test]
fn every_room_stays_dot_rendered_after_worker_warmup_and_resize() {
    let _guard = crate::tests::env_lock();
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _backed = crate::tests::TestEnvGuard::set("ANGEL_BACKED_MAP", "1");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    use crate::world_viz::Building;
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
        let mut app = App::preview(crate::viewer::Viewer::new());
        app.world = crate::world_viz::World::new(71);
        app.world.settle_at_for_test(building);
        app.scryglass = crate::scryglass::Scryglass::for_world(building);
        app.visual_motion = crate::viz::lifecycle_viz::MotionMode::Full;
        assert!(app.world.enter_interior());
        for (width, height) in [(48, 18), (96, 40), (48, 18)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            for tick in 0..80 {
                app.world.tick();
                terminal
                    .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
                    .unwrap();
                let cells = &terminal.backend().buffer().content;
                let solid = cells
                    .iter()
                    .filter(|cell| cell.symbol().chars().any(|c| matches!(c, '▀' | '▄' | '█')))
                    .count();
                assert_eq!(
                    solid, 0,
                    "{building:?} {width}x{height} tick {tick}: native/solid world plate replaced Dotmax"
                );
                let dots = cells
                    .iter()
                    .filter(|cell| {
                        cell.symbol()
                            .chars()
                            .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
                    })
                    .count();
                assert!(
                    dots > 20,
                    "{building:?} {width}x{height} tick {tick}: world must remain visibly dot-rendered, dots={dots}"
                );
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
        }
        assert!(app.world.leave_interior());
        assert!(!app.world.ambient_interior_visible());
        assert!(app.world.enter_interior());
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(48, 18)).unwrap();
        terminal
            .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
            .unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|cell| !cell.symbol().chars().any(|c| matches!(c, '▀' | '▄' | '█')))
        );
    }
}
