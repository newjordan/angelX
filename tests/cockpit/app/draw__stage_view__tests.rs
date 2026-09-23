use super::*;

#[test]
fn running_loop_dancer_stays_in_scene_and_yields_when_too_small() {
    let _guard = crate::tests::env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.loop_ctl.status = crate::drive::loop_ctl::LoopStatus::Running;
    const SENTINEL: char = '\u{00a4}';
    const SENTINEL_FG: ratatui::style::Color = ratatui::style::Color::Cyan;
    const SENTINEL_BG: ratatui::style::Color = ratatui::style::Color::Magenta;
    let sentinel = SENTINEL.to_string();

    for (scene, should_paint) in [
        (Rect::new(5, 4, 8, 6), false),
        (Rect::new(5, 4, 14, 6), false),
        (Rect::new(5, 4, 18, 7), false),
        (Rect::new(5, 4, 8, 8), true),
        (Rect::new(5, 4, 16, 8), true),
    ] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 18)).unwrap();
        terminal
            .draw(|frame| {
                for cell in &mut frame.buffer_mut().content {
                    cell.set_char(SENTINEL)
                        .set_fg(SENTINEL_FG)
                        .set_bg(SENTINEL_BG);
                }
                render_hammertime_mascot(frame, &mut app, scene);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut changed_inside = 0;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if x >= scene.x && x < scene.right() && y >= scene.y && y < scene.bottom() {
                    changed_inside += usize::from(buffer[(x, y)].symbol() != sentinel);
                    continue;
                }
                let cell = &buffer[(x, y)];
                assert_eq!(
                    (cell.symbol(), cell.fg, cell.bg),
                    (sentinel.as_str(), SENTINEL_FG, SENTINEL_BG),
                    "{scene:?} altered outside cell ({x}, {y})"
                );
            }
        }
        assert_eq!(
            changed_inside > 0,
            should_paint,
            "{scene:?} should{} paint a dancer overlay",
            if should_paint { "" } else { " not" }
        );
    }
}

#[test]
fn world_overlay_assets_use_dots_even_when_native_images_are_available() {
    let _guard = crate::tests::env_lock();
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "kitty");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in [
        root.join("assets/loop/hammertime-a.png"),
        root.join("assets/loop/hammertime-b.png"),
        crate::ui::viz::spend_viz::asset_path().to_path_buf(),
    ] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(16, 8)).unwrap();
        terminal
            .draw(|frame| {
                assert!(paint_world_overlay_dots(frame, frame.area(), &path));
            })
            .unwrap();
        let cells = &terminal.backend().buffer().content;
        assert!(
            cells.iter().any(|cell| cell
                .symbol()
                .chars()
                .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))),
            "{}",
            path.display()
        );
        assert!(cells.iter().all(|cell| {
            cell.symbol()
                .chars()
                .all(|c| c == ' ' || ('\u{2800}'..='\u{28ff}').contains(&c))
        }));
    }
}

#[test]
fn dotmax_room_plate_requires_explicit_entry_and_closes_on_leave() {
    let _guard = crate::tests::env_lock();
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.world = crate::stage::world_viz::World::new(71);
    app.world
        .select_landmark(crate::stage::world_viz::Building::Smithy);
    for _ in 0..500 {
        app.world.tick();
    }
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| assert!(!render_dotmax_interior(frame, &mut app, frame.area())))
        .unwrap();
    assert!(app.world.enter_interior());
    terminal
        .draw(|frame| assert!(render_dotmax_interior(frame, &mut app, frame.area())))
        .unwrap();
    let cells = &terminal.backend().buffer().content;
    assert!(cells.iter().any(|cell| {
        cell.symbol()
            .chars()
            .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
    }));
    assert!(
        cells
            .iter()
            .all(|cell| !cell.symbol().chars().any(|c| matches!(c, '▀' | '▄' | '█')))
    );
    assert!(app.world.leave_interior());
    terminal
        .draw(|frame| assert!(!render_dotmax_interior(frame, &mut app, frame.area())))
        .unwrap();
    assert!(!app.world.ambient_interior_visible());
}

#[test]
#[ignore = "manual living-painting Stage review: set REALM_AMBIENT_DUMP"]
fn dump_living_painting_stage_cells_for_review() {
    let _guard = crate::tests::env_lock();
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _backed = crate::tests::TestEnvGuard::set("ANGEL_BACKED_MAP", "1");
    let out = std::path::PathBuf::from(
        std::env::var("REALM_AMBIENT_DUMP").expect("set REALM_AMBIENT_DUMP"),
    );
    std::fs::create_dir_all(&out).unwrap();
    use crate::stage::world_viz::Building;
    for (id, building) in [
        ("keep", Building::Keep),
        ("gatehouse", Building::Gatehouse),
        ("rookery", Building::Rookery),
        ("scriptorium", Building::Scriptorium),
        ("smithy", Building::Smithy),
        ("chapel", Building::Chapel),
        ("round-table", Building::RoundTable),
        ("observatory", Building::Observatory),
    ] {
        for (cols, rows) in [(60u16, 20u16), (40, 12)] {
            for phase in 0..16 {
                let mut app = App::preview(crate::ui::viewer::Viewer::new());
                app.world = crate::stage::world_viz::World::new(71);
                app.world.settle_at_for_test(building);
                assert!(app.world.enter_interior());
                app.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Full;
                for _ in 0..phase * 10 {
                    app.world.tick();
                }
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(cols, rows)).unwrap();
                terminal
                    .draw(|frame| assert!(render_dotmax_interior(frame, &mut app, frame.area())))
                    .unwrap();
                // Rasterize the actual Dotmax cells with equal 4px pitch
                // on both axes; this exports the displayed presentation.
                let mut pixels = image::RgbaImage::from_pixel(
                    u32::from(cols) * 8,
                    u32::from(rows) * 16,
                    image::Rgba([10, 10, 16, 255]),
                );
                const BITS: [[u8; 2]; 4] = [[1, 8], [2, 16], [4, 32], [64, 128]];
                for y in 0..rows {
                    for x in 0..cols {
                        let cell = &terminal.backend().buffer()[(x, y)];
                        let symbol = cell.symbol().chars().next().unwrap_or(' ');
                        let bits = match symbol {
                            ' ' => 0,
                            '\u{2800}'..='\u{28ff}' => (symbol as u32 - 0x2800) as u8,
                            other => panic!("unexpected room cell {other:?}"),
                        };
                        let ink = match cell.fg {
                            ratatui::style::Color::Rgb(r, g, b) => [r, g, b, 255],
                            _ => [0, 0, 0, 255],
                        };
                        for (dy, row) in BITS.iter().enumerate() {
                            for (dx, mask) in row.iter().enumerate() {
                                if bits & mask == 0 {
                                    continue;
                                }
                                for py in 0..3 {
                                    for px in 0..3 {
                                        pixels.put_pixel(
                                            u32::from(x) * 8 + dx as u32 * 4 + 1 + px,
                                            u32::from(y) * 16 + dy as u32 * 4 + 1 + py,
                                            image::Rgba(ink),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                pixels
                    .save(out.join(format!("{id}-stage-{cols}x{rows}-{phase:02}.png")))
                    .unwrap();
            }
        }
    }
}

#[test]
fn room_graphics_require_entry_and_leaving_restores_outdoors() {
    let _env = crate::tests::env_lock();
    for alias in ["dotmax", "raycast", "ambient", "art"] {
        let _view = crate::tests::TestEnvGuard::set("ANGEL_WORLD_VIEW", alias);
        let mut world = crate::stage::world_viz::World::new(42);
        world.settle_at_for_test(crate::stage::world_viz::Building::Keep);
        assert!(!world.ambient_interior_visible());
        assert!(world.enter_interior());
        assert!(world.ambient_interior_visible());
        assert!(world.leave_interior());
        assert!(!world.ambient_interior_visible());
    }
}

#[test]
fn overworld_half_blocks_letterbox_the_frame_on_black_paper() {
    use crate::stage::world_viz::overworld;
    let img = overworld::frame(&overworld::Scene::resting("Dologard"));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| paint_halfblock_frame(frame, Rect::new(0, 0, 40, 12), &img))
        .unwrap();
    let buf = terminal.backend().buffer();
    let black = ratatui::style::Color::Rgb(0, 0, 0);
    assert!(buf.content.iter().all(|cell| cell.symbol() == "▀"));
    // 256x240 into a 40x24 half-block grid: 25.6 columns wide, centred.
    for y in 0..12 {
        assert_eq!(buf[(0, y)].fg, black, "left letterbox row {y}");
        assert_eq!(buf[(39, y)].bg, black, "right letterbox row {y}");
    }
    let lit = buf
        .content
        .iter()
        .filter(|cell| cell.fg != black || cell.bg != black)
        .count();
    assert!(lit > 60, "the frame itself paints ({lit} lit cells)");
}
