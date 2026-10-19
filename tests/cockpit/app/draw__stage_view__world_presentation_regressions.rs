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
    use crate::stage::world_viz::Building;
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
        let mut app = App::preview(crate::ui::viewer::Viewer::new());
        app.world = crate::stage::world_viz::World::new(71);
        app.world.settle_at_for_test(building);
        app.scryglass = crate::ui::scryglass::Scryglass::for_world(building);
        app.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Full;
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

/// Exercise the default route, including the real stage dispatcher and dot
/// bridge: an active mine journey must move without opting into Explore.
#[test]
fn default_realm_animates_the_expedition_and_map_visits_still_win() {
    let _guard = crate::tests::env_lock();
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    use crate::stage::world_viz::{AdventureEvent, Building, LoopKind, World};
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.world = World::new(42);
    app.world.settle_at_for_test(Building::Keep);
    app.world.note_adventure(AdventureEvent::LoopStarted {
        kind: LoopKind::Coding,
        task: "presentation fixture".into(),
    });
    app.world.note_adventure(AdventureEvent::Iteration { n: 1 });
    app.scryglass = crate::ui::scryglass::Scryglass::default();
    app.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Full;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(48, 18)).unwrap();
    terminal
        .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
        .unwrap();
    let first = terminal.backend().buffer().clone();
    assert_eq!(
        app.scryglass.surface,
        crate::ui::scryglass::StageSurface::WorldMap
    );
    assert!(
        first
            .content
            .iter()
            .filter(|cell| cell
                .symbol()
                .chars()
                .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch)))
            .count()
            > 40
    );
    for _ in 0..80 {
        app.world.tick();
    }
    terminal
        .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
        .unwrap();
    assert_ne!(
        &first,
        terminal.backend().buffer(),
        "real Realm camera must travel"
    );
    assert!(app.world.toggle_adventure_map());
    terminal
        .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
        .unwrap();
    assert!(!app.world.live_adventure_view());
    assert!(terminal.backend().buffer().content.iter().any(|cell| {
        cell.symbol()
            .chars()
            .any(|ch| matches!(ch, '▀' | '▄' | '█'))
    }));
    assert!(app.world.toggle_adventure_map());
    assert!(app.world.live_adventure_view());
}

#[test]
fn object_dock_routes_real_stage_actions_and_collapsed_draw_removes_hits() {
    let _guard = crate::tests::env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.scryglass = crate::ui::scryglass::Scryglass::for_world(
        crate::stage::world_viz::Building::Keep);
    app.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Off;
    let mut terminal = ratatui::Terminal::new(
        ratatui::backend::TestBackend::new(100, 26)).unwrap();
    let paint = |app: &mut App, terminal: &mut ratatui::Terminal<ratatui::backend::TestBackend>| {
        terminal.draw(|frame| {
            render_artifacts(frame, app, frame.area());
            crate::ui::object_dock::render_overlay(frame, app);
        }).unwrap();
    };
    paint(&mut app, &mut terminal);
    assert!(app.object_dock.visible);
    assert!(!app.object_dock.open);
    assert!(app.world_buttons.iter().any(|(_, b)| *b == WorldButton::DockToggle));
    assert!(!app.world_buttons.iter().any(|(_, b)| *b == WorldButton::Assets));
    app.apply_world_button(WorldButton::DockToggle);
    paint(&mut app, &mut terminal);
    for action in [WorldButton::ScryglassEnter, WorldButton::ScryglassMap, WorldButton::Tower,
        WorldButton::Assets, WorldButton::ScryglassLibrary, WorldButton::ScryglassVault, WorldButton::Back] {
        assert!(app.world_buttons.iter().any(|(_, b)| *b == action), "missing {action:?}");
    }
    // A pane under the dock's width keeps its classic row, Back in reach.
    let mut tight = ratatui::Terminal::new(ratatui::backend::TestBackend::new(28, 18)).unwrap();
    paint(&mut app, &mut tight);
    assert!(!app.object_dock.visible, "a narrow pane keeps its row");
    assert!(app.world_buttons.iter().any(|(_, b)| *b == WorldButton::Back));
    // Explicit paging exposes every object on a compact pane; resizing back
    // clamps the page instead of leaving a blank drawer.
    let mut narrow = ratatui::Terminal::new(ratatui::backend::TestBackend::new(44, 18)).unwrap();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..7 {
        paint(&mut app, &mut narrow);
        for (_, action) in &app.object_dock.tiles { seen.insert(format!("{action:?}")); }
        if !app.world_buttons.iter().any(|(_, b)| *b == WorldButton::DockNext) { break; }
        app.apply_world_button(WorldButton::DockNext);
    }
    for action in ["ScryglassEnter", "ScryglassMap", "Tower", "Assets", "ScryglassLibrary", "ScryglassVault", "Back"] {
        assert!(seen.contains(action), "compact pane lost {action}");
    }
    paint(&mut app, &mut terminal);
    assert_eq!(app.object_dock.page, 0);
    let open_height = app.object_dock.area.height;
    assert_eq!(open_height, 5);
    app.apply_world_button(WorldButton::DockToggle);
    assert!(!app.world_buttons.iter().any(|(_, b)| *b == WorldButton::Assets));
    paint(&mut app, &mut terminal);
    assert_eq!(app.object_dock.area.height, open_height, "viewport must not reflow");
    assert_eq!(app.object_dock.tiles.len(), 1, "collapsed handle only");
    app.apply_world_button(WorldButton::DockToggle);
    paint(&mut app, &mut terminal);
    let assets = app.world_buttons.iter().find(|(_, b)| *b == WorldButton::Assets).unwrap().0;
    // Real mouse dispatch must choose the object before its background surface.
    app.on_mouse(ratatui::crossterm::event::MouseEvent {
        kind: ratatui::crossterm::event::MouseEventKind::Down(ratatui::crossterm::event::MouseButton::Left),
        column: assets.x + assets.width / 2,
        row: assets.y + assets.height / 2,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    });
    paint(&mut app, &mut terminal);
    assert!(!app.world_buttons.iter().any(|(_, b)| *b == WorldButton::DockToggle),
        "asset overlays must not inherit hidden dock hits");
}

#[test]
fn object_dock_every_action_gets_real_mouse_clicks_even_when_called_to_delve() {
    use crate::tests::{env_lock, TestEnvGuard};
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let _guard = env_lock();
    let _image = TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _map = TestEnvGuard::unset("ANGEL_WORLD_MAP");
    for button in [WorldButton::ScryglassEnter, WorldButton::ScryglassMap,
        WorldButton::Tower, WorldButton::Assets, WorldButton::ScryglassLibrary,
        WorldButton::ScryglassVault, WorldButton::Back] {
        let mut app = seed_preview_app();
        app.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Off;
        app.scryglass.return_to_world();
        app.world.delve_called = true;
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(96, 36)).unwrap();
        terminal.draw(|frame| {
            render_artifacts(frame, &mut app, frame.area());
            crate::ui::object_dock::render_overlay(frame, &mut app);
        }).unwrap();
        app.apply_world_button(WorldButton::DockToggle);
        terminal.draw(|frame| {
            render_artifacts(frame, &mut app, frame.area());
            crate::ui::object_dock::render_overlay(frame, &mut app);
        }).unwrap();
        let mut expected = seed_preview_app();
        expected.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Off;
        expected.scryglass.return_to_world();
        expected.world.delve_called = true;
        expected.apply_world_button(button);
        let mut expected_terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(96, 36)).unwrap();
        expected_terminal.draw(|frame| {
            render_artifacts(frame, &mut expected, frame.area());
            crate::ui::object_dock::render_overlay(frame, &mut expected);
        }).unwrap();
        let rect = app.world_buttons.iter().find(|(_, b)| *b == button).unwrap().0;
        for kind in [MouseEventKind::Down(MouseButton::Left), MouseEventKind::Up(MouseButton::Left)] {
            app.on_mouse(MouseEvent { kind, column: rect.x + rect.width / 2,
                row: rect.y + rect.height / 2, modifiers: KeyModifiers::NONE });
        }
        terminal.draw(|frame| {
            render_artifacts(frame, &mut app, frame.area());
            crate::ui::object_dock::render_overlay(frame, &mut app);
        }).unwrap();
        assert_eq!(app.scryglass.surface, expected.scryglass.surface, "{button:?} routed to the wrong stage");
        assert_eq!(app.world.inside_interior(), expected.world.inside_interior(), "{button:?} lost its room action");
        assert_eq!(app.world.interior_building(), expected.world.interior_building(), "{button:?} entered the wrong room");
        assert!(app.dungeon.shooter.is_none(), "{button:?} must not launch a run");
        assert!(app.dungeon.intro.is_none(), "{button:?} must not trigger the gate");
    }
}
