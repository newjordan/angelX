use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn startup_intro_rises_once_holds_and_fades_without_restarting() {
    let now = Instant::now();
    let mut intro = StartupIntro {
        started: Some(now),
        ..Default::default()
    };
    assert_eq!(intro.sample(now, MotionMode::Full), Some((0, None, 0, 255)));
    assert_eq!(
        intro.sample(now + Duration::from_secs(2), MotionMode::Full),
        Some((24, None, 0, 255))
    );
    // After the rise the sword holds; the water keeps flowing: the late-clip
    // frames cycle while the blade frame stays pinned.
    let (held, flow_a, _, _) = intro
        .sample(now + Duration::from_secs(50), MotionMode::Full)
        .unwrap();
    assert_eq!(held, HOLD);
    let flow_a = flow_a.expect("the settled blade keeps the water flowing");
    assert!(RIPPLE.contains(&flow_a.frame));
    let (_, flow_b, _, _) = intro
        .sample(now + Duration::from_secs(51), MotionMode::Full)
        .unwrap();
    assert!(RIPPLE.contains(&flow_b.unwrap().frame));
    // The cycle closes on a dissolve rather than the 6.3-level jump a hard wrap
    // left: the last late-clip frame melts into the first over SEAM ticks, then
    // the cycle restarts on that first frame outright.
    let cycle = RIPPLE.len() as u128;
    assert_eq!(
        Flow::at(0),
        Flow {
            frame: RIPPLE[0],
            next: RIPPLE[0],
            melt: 0
        }
    );
    let melts: Vec<u8> = (cycle..cycle + u128::from(SEAM))
        .map(|tick| {
            let flow = Flow::at(tick);
            assert_eq!(
                (flow.frame, flow.next),
                (RIPPLE[RIPPLE.len() - 1], RIPPLE[0])
            );
            flow.melt
        })
        .collect();
    assert!(
        melts.windows(2).all(|pair| pair[0] < pair[1])
            && melts.iter().all(|&melt| melt > 0 && melt < 255),
        "the whole seam span dissolves, never snapping: {melts:?}"
    );
    assert_eq!(
        Flow::at(cycle + u128::from(SEAM)),
        Flow {
            frame: RIPPLE[0],
            next: RIPPLE[0],
            melt: 0
        }
    );
    intro.dismiss(now + Duration::from_secs(2), MotionMode::Full);
    assert_eq!(
        intro.sample(now + Duration::from_millis(2300), MotionMode::Full),
        Some((24, None, 0, 128))
    );
    intro.dismiss(now + Duration::from_millis(2500), MotionMode::Full);
    assert!(
        intro
            .sample(now + Duration::from_millis(2600), MotionMode::Full)
            .is_none()
    );
    intro.cancel();
    intro.begin_frame(false, false, MotionMode::Full);
    assert!(intro.sample(now, MotionMode::Full).is_none());
}

/// The clip's waterline and hand live in the frame's lower rows, so a prepared
/// canvas has to carry them on the pane floor. In a wide pane the fit is
/// height-limited and the anchor cannot be told apart from a centered one; in a
/// narrow pane (mini-viz, a tall bay) it can — a centered Y floated the blade
/// into the middle of the pane with a dead strip underneath, which is what read
/// as a hard cut through the water instead of a surface resting on the floor.
#[test]
fn startup_intro_anchors_the_waterline_to_the_pane_floor() {
    let atlas = decode_atlas(ATLAS).unwrap();
    let extent = |columns: usize, rows: usize| -> (u32, u32, u32) {
        let canvas = prepare(&atlas, HOLD, columns, rows);
        let height = rows as u32 * 4;
        let inked = |y: u32| (0..canvas.width()).any(|x| canvas.get_pixel(x, y).0[0] > 0);
        let first = (0..height).find(|&y| inked(y)).expect("frame carries ink");
        let last = (0..height)
            .rev()
            .find(|&y| inked(y))
            .expect("frame carries ink");
        (first, last, height)
    };
    // A height-limited pane carries no fit slack at all, so whatever blank rows
    // sit under the water there are the clip's own tail — its floor. Every other
    // geometry has to reproduce that same gap; a centered Y adds half the slack
    // on top of it, which is the dead strip the operator saw.
    let (_, baseline_last, baseline_h) = extent(72, 32);
    let floor_gap = baseline_h - 1 - baseline_last;
    // The crop ends where the clip's ink ends (`CROP_H`), so the clip's own
    // tail is at most the levels curve eating the faintest rows: on the pane
    // floor means on the floor, not a dead strip the height of a text row.
    assert!(
        floor_gap <= 4,
        "the settled frame leaves {floor_gap} blank dot rows under the water; the crop should end at the ink"
    );
    for (columns, rows) in [(36usize, 24usize), (40, 40), (36, 32)] {
        let (first, last, height) = extent(columns, rows);
        let gap = height - 1 - last;
        assert!(
            gap <= floor_gap + 2,
            "waterline must sit on the pane floor: {gap} blank rows under the ink against the \
             clip's own {floor_gap} ({columns}x{rows})"
        );
        // Mirror of prepare()'s own fit, so this can tell "bottom-anchored" apart
        // from "centered" instead of only "some ink somewhere": the spare height
        // has to be above the blade, which is why the case has to be width-limited.
        let crop_w = (FRAME_W - 140 * 2) as f32;
        let scale = (columns as f32 * 2.0 / crop_w).min(height as f32 / CROP_H as f32);
        let fitted_h = (CROP_H as f32 * scale).round().max(1.0) as u32;
        let anchored_top = height.saturating_sub(fitted_h);
        assert!(
            anchored_top > 0,
            "({columns}x{rows}) is height-limited, so it cannot distinguish the anchors"
        );
        assert!(
            first >= anchored_top,
            "spare height belongs above the blade: first inked row {first}, the \
             bottom-anchored top is {anchored_top} ({columns}x{rows})"
        );
    }
}

#[test]
fn startup_intro_respects_motion_and_hidden_tick_policy() {
    let now = Instant::now();
    let mut intro = StartupIntro {
        started: Some(now),
        visible: true,
        ..Default::default()
    };
    for motion in [MotionMode::Reduced, MotionMode::Off] {
        assert_eq!(intro.sample(now, motion), Some((59, None, 0, 255)));
        assert!(!intro.animating(now, motion));
    }
    assert!(intro.animating(now, MotionMode::Full));
    intro.begin_frame(false, false, MotionMode::Full);
    assert!(!intro.animating(now, MotionMode::Full));
    intro.dismiss(now, MotionMode::Reduced);
    assert!(
        intro
            .sample(now + Duration::from_millis(200), MotionMode::Reduced)
            .is_none()
    );
    let mut off = StartupIntro {
        started: Some(now),
        ..Default::default()
    };
    off.dismiss(now, MotionMode::Off);
    assert!(off.finished);
}

#[test]
fn startup_intro_keys_mountain_reveal_and_summit_walk_to_the_sword() {
    let now = Instant::now();
    let intro = StartupIntro {
        started: Some(now),
        ..Default::default()
    };
    assert_eq!(intro.landscape_tick(now, MotionMode::Full), Some(0));
    assert_eq!(
        intro.landscape_tick(now + Duration::from_millis(4_100), MotionMode::Full),
        Some(HOLD)
    );
    assert_eq!(
        intro.landscape_tick(now + Duration::from_millis(6_100), MotionMode::Full),
        Some(WIZARD_SETTLE)
    );
    assert_eq!(
        intro.landscape_tick(now + Duration::from_secs(60), MotionMode::Full),
        Some(WIZARD_SETTLE + (((720 - u128::from(WIZARD_SETTLE)) / 3) % 8) as u8),
        "the completed summit keeps a bounded sprite idle loop"
    );
    assert_eq!(
        intro.landscape_tick(now, MotionMode::Reduced),
        Some(WIZARD_SETTLE),
        "reduced motion selects the final static tableau"
    );
}

#[test]
fn startup_intro_keystroke_and_paste_dismiss_before_submission() {
    let _guard = crate::tests::env_lock();
    for paste in [false, true] {
        let mut app = crate::seed_preview_app();
        app.messages.clear();
        app.input.clear();
        app.cursor = 0;
        app.visual_motion = MotionMode::Full;
        app.startup_intro = StartupIntro::default();
        app.startup_intro.started = Some(Instant::now());
        app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        app.on_paste("");
        assert!(app.startup_intro.fading.is_none());
        if paste {
            app.on_paste("pick up the sword");
            assert_eq!(app.input, "pick up the sword");
        } else {
            app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
            assert_eq!(app.input, "a");
            app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            assert!(app.input.is_empty());
        }
        assert!(app.startup_intro.fading.is_some());
        assert!(app.messages.is_empty());
        assert!(app.pending_turn.is_none());
    }
}

#[test]
fn startup_intro_real_content_and_prefilled_drafts_never_flash_art() {
    let _guard = crate::tests::env_lock();
    for occupied in [false, true] {
        let mut intro = StartupIntro::default();
        intro.begin_frame(occupied, !occupied, MotionMode::Full);
        intro.begin_frame(false, false, MotionMode::Full);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| intro.render(frame, frame.area(), None, MotionMode::Full))
            .unwrap();
        assert!(intro.finished);
        assert!(intro.worker.is_none());
        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .all(|c| c.symbol() == " ")
        );
    }
}

#[test]
fn startup_intro_actual_clip_is_braille_fitted_and_fades_without_dot_shimmer() {
    let atlas = decode_atlas(ATLAS).unwrap();
    // A settled frame with no flow: the still case the fade assertions cover.
    let still = |opacity| Key {
        scene: Scene::Sword,
        frame: 59,
        flow: None,
        water: 0,
        opacity,
        size: Size::new(0, 0),
        geometry: None,
    };
    for (w, h) in [(1, 1), (24, 40), (100, 35), (240, 70)] {
        let raised = compose(&atlas, &mut Cache::default(), &still(255), w, h).unwrap();
        let faded = compose(&atlas, &mut Cache::default(), &still(64), w, h).unwrap();
        assert_eq!(raised.cells.len(), w * h);
        assert!(
            raised
                .cells
                .iter()
                .all(|c| ('\u{2800}'..='\u{28ff}').contains(&c.glyph))
        );
        // The fade now drops dimmer dots instead of dimming ink, so the faded
        // frame keeps a subset of the raised frame's glyphs and pure white ink.
        let raised_lit = raised
            .cells
            .iter()
            .filter(|c| c.glyph != '\u{2800}')
            .count();
        let faded_lit = faded.cells.iter().filter(|c| c.glyph != '\u{2800}').count();
        assert!(
            faded_lit < raised_lit || raised_lit == 0,
            "fade did not drop dots at w={w} h={h}"
        );
        assert!(
            raised
                .cells
                .iter()
                .all(|c| c.fg == [0; 3] || c.fg == [255; 3])
        );
        assert!(
            faded
                .cells
                .iter()
                .all(|c| c.fg == [0; 3] || c.fg == [255; 3])
        );
        if w > 1 {
            assert!(
                raised
                    .cells
                    .iter()
                    .filter(|c| c.glyph != '\u{2800}')
                    .count()
                    > 10
            );
            // Letterboxing preserves the sword tip and the waterline.
            assert!(raised.cells[..w].iter().all(|c| c.glyph == '\u{2800}'));
        }
    }
    let black = GrayImage::new(FRAME_W * 10, FRAME_H * 6);
    assert!(
        compose(&black, &mut Cache::default(), &still(255), 100, 40)
            .unwrap()
            .cells
            .iter()
            .all(|c| c.glyph == '\u{2800}')
    );
}

#[test]
fn startup_mountain_is_a_low_rising_contour_with_a_compact_summit_wizard() {
    assert!(ASCENT[0].1 > 0.85, "the base sits near the bottom");
    assert!(
        ASCENT
            .windows(2)
            .all(|p| p[1].0 > p[0].0 && p[1].1 <= p[0].1)
    );
    assert_eq!(wizard_x_at(HOLD - 1), None);
    assert!(wizard_x_at(HOLD).unwrap() > 1.0);
    assert!((wizard_x_at(WIZARD_SETTLE).unwrap() - 0.84).abs() < 0.001);
    for (columns, rows) in [(36, 16), (72, 32), (120, 24)] {
        let ridge = compose_mountain(HOLD, columns, rows, 255).unwrap();
        let settled = compose_mountain(WIZARD_SETTLE, columns, rows, 255).unwrap();
        let lit = |cell: &&ColoredBrailleCell| cell.glyph != NO_DOTS;
        assert!(settled.cells.iter().filter(lit).count() > ridge.cells.iter().filter(lit).count());
        // Below the low contour there is no floor, wireframe or back face.
        assert!(
            settled.cells[(rows - 1) * columns..]
                .iter()
                .all(|c| c.glyph == NO_DOTS)
        );
        assert!(settled.cells.iter().filter(lit).all(|c| c.fg == INTRO_INK));
        assert!(
            compose_mountain(WIZARD_SETTLE, columns, rows, 0)
                .unwrap()
                .cells
                .iter()
                .all(|c| c.glyph == NO_DOTS)
        );
    }
}

#[test]
fn startup_wizard_idle_moves_cloth_without_moving_the_summit() {
    let still = compose_mountain(WIZARD_SETTLE, 72, 32, 255).unwrap();
    let breeze = compose_mountain(WIZARD_SETTLE + 2, 72, 32, 255).unwrap();
    assert!(
        still
            .cells
            .iter()
            .zip(&breeze.cells)
            .any(|(a, b)| a.glyph != b.glyph)
    );
    assert!(
        still.cells[14 * 72..]
            .iter()
            .zip(&breeze.cells[14 * 72..])
            .all(|(a, b)| a.glyph == b.glyph)
    );
    let now = Instant::now();
    let intro = StartupIntro {
        started: Some(now),
        ..Default::default()
    };
    for motion in [MotionMode::Reduced, MotionMode::Off] {
        assert_eq!(
            intro.landscape_tick(now + Duration::from_secs(90), motion),
            Some(WIZARD_SETTLE)
        );
    }
}

#[test]
fn startup_mountain_clips_plot_lines_to_the_dot_canvas() {
    let mut image = ColoredBrailleImage {
        width: 4,
        height: 2,
        cells: vec![ColoredBrailleCell::default(); 8],
    };
    draw_dot_line(&mut image, (-10_000, 4), (10_000, 4), INTRO_INK, 255);
    assert_eq!(
        image
            .cells
            .iter()
            .filter(|cell| cell.glyph != NO_DOTS)
            .count(),
        4,
        "a crossing line is clipped to one dot row across the four cells"
    );
    let before = image.clone();
    draw_dot_line(
        &mut image,
        (-10_000, -10_000),
        (-5_000, -5_000),
        INTRO_INK,
        255,
    );
    assert_eq!(image, before, "a wholly offscreen line is rejected");
}

#[test]
fn startup_intro_worker_is_bounded_and_resize_rejects_old_geometry() {
    let mut intro = StartupIntro::default();
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    let first = Rect::new(2, 2, 80, 30);
    let second = Rect::new(2, 2, 40, 20);
    terminal
        .draw(|frame| intro.render(frame, first, None, MotionMode::Reduced))
        .unwrap();
    assert!(intro.surfaces[0].pending);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        terminal
            .draw(|frame| intro.render(frame, second, None, MotionMode::Reduced))
            .unwrap();
        if let Some((key, _)) = &intro.surfaces[0].current {
            assert_eq!(key.size, second.as_size());
            break;
        }
        assert!(Instant::now() < deadline, "intro worker did not deliver");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!intro.surfaces[0].pending);
    assert!(!intro.animating(Instant::now(), MotionMode::Reduced));
    intro.dismiss(Instant::now(), MotionMode::Off);
    assert!(intro.surfaces[0].current.is_none() && intro.worker.is_none());
}

#[test]
fn startup_intro_fine_transport_owns_separate_ids_and_transparent_dots() {
    let mut intro = StartupIntro::default();
    let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
    let area = Rect::new(2, 2, 60, 24);
    let geometry = DotGeometry::new(area.width, area.height, (8, 16), 2).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        terminal
            .draw(|frame| {
                intro.render(frame, area, Some(geometry), MotionMode::Reduced);
                intro.flush_upload(frame);
            })
            .unwrap();
        if let Some((_, ready)) = &intro.surfaces[0].current {
            let protocol = ready.protocol.as_ref().expect("fine-dot transport");
            assert_eq!(protocol.image_id() & 2, 0, "world owns the other ID pair");
            assert_eq!(ready.dots.width, geometry.grid_width);
            let buffer = terminal.backend().buffer();
            assert!(buffer.cell((0, 0)).unwrap().symbol().contains("\x1b_G"));
            assert!(
                buffer
                    .cell((2, 2))
                    .unwrap()
                    .symbol()
                    .starts_with('\u{10eeee}')
            );
            assert_eq!(buffer.cell((1, 2)).unwrap().symbol(), " ");
            // The transported pixels are the animation: the generated dots on
            // a transparent background, so the terminal composites the sword
            // over the pane instead of over a plate uploaded behind it.
            let upload = buffer
                .cell((0, 0))
                .unwrap()
                .symbol()
                .strip_suffix(' ')
                .expect("upload keeps the cell's own blank");
            let transported = crate::ui::dots::protocol::decode_upload(upload);
            assert_eq!(
                transported,
                geometry.rasterize_on(&ready.dots, TRANSPARENT).unwrap()
            );
            assert_eq!(
                transported.get_pixel(0, 0).0[3],
                0,
                "the fitted frame's empty margin must stay transparent"
            );
            assert!(transported.pixels().any(|pixel| pixel.0[3] == 255));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    intro.begin_frame(true, false, MotionMode::Full);
    assert!(intro.surfaces[0].current.is_none() && intro.worker.is_none());
}

/// Terminals without fine-dot transport get the Braille fallback. It paints
/// white ink and nothing else: cells the sword never reaches keep whatever the
/// pane painted there, and no cell carries a background under the dots.
#[test]
fn startup_intro_braille_fallback_paints_only_white_dots() {
    let mut intro = StartupIntro {
        started: Some(Instant::now()),
        ..Default::default()
    };
    let mut terminal = Terminal::new(TestBackend::new(70, 30)).unwrap();
    let area = Rect::new(2, 2, 60, 22);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        terminal
            .draw(|frame| {
                for y in area.y..area.bottom() {
                    for x in area.x..area.right() {
                        frame.buffer_mut().cell_mut((x, y)).unwrap().set_symbol("#");
                    }
                }
                intro.render(frame, area, None, MotionMode::Reduced);
            })
            .unwrap();
        if intro.surfaces[0].current.is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "intro worker did not deliver");
        std::thread::sleep(Duration::from_millis(5));
    }
    let buffer = terminal.backend().buffer();
    let backdrop = (area.y..area.bottom())
        .filter(|&y| buffer.cell((area.x, y)).unwrap().symbol() == "#")
        .count();
    assert_eq!(
        backdrop,
        usize::from(area.height),
        "the empty margin left of the fitted sword must keep the backdrop"
    );
    let mut painted = 0;
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = buffer.cell((x, y)).unwrap();
            if cell.symbol() == "#" {
                continue;
            }
            painted += 1;
            assert_eq!(cell.bg, Color::Reset, "the fallback paints no background");
            assert_eq!(cell.fg, Color::Rgb(255, 255, 255), "dots are white ink");
            assert_ne!(cell.symbol(), " ");
            assert_ne!(cell.symbol(), "\u{2800}", "empty cells stay backdrop");
        }
    }
    assert!(painted > 0, "the fallback painted no dots");
}

#[test]
fn startup_intro_keeps_sword_and_mountain_in_distinct_panes_with_shared_dismissal() {
    let mut intro = StartupIntro::default();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let areas = [Rect::new(2, 2, 70, 35), Rect::new(80, 20, 36, 16)];
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        terminal
            .draw(|frame| {
                intro.begin_frame(false, false, MotionMode::Reduced);
                intro.render(
                    frame,
                    areas[0],
                    DotGeometry::new(70, 35, (8, 16), 2),
                    MotionMode::Reduced,
                );
                assert!(intro.render_miniviz(
                    frame,
                    areas[1],
                    DotGeometry::new(36, 16, (8, 16), 2),
                    MotionMode::Reduced
                ));
                intro.flush_upload(frame);
            })
            .unwrap();
        if intro
            .surfaces
            .iter()
            .all(|surface| surface.current.is_some())
        {
            break;
        }
        assert!(Instant::now() < deadline, "both intro panes must finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    let sword = &intro.surfaces[0].current.as_ref().unwrap().1.dots;
    assert!(
        sword
            .cells
            .iter()
            .all(|cell| cell.fg == [0; 3] || cell.fg == [255; 3]),
        "the left pane remains the white Excalibur clip"
    );
    let mountain = &intro.surfaces[1].current.as_ref().unwrap().1.dots;
    assert!(mountain.cells.iter().any(|cell| cell.glyph != NO_DOTS));
    assert!(
        mountain
            .cells
            .iter()
            .filter(|cell| cell.glyph != NO_DOTS)
            .all(|cell| cell.fg == [255; 3]),
        "the right pane must match the sword's white ink"
    );
    let ids = intro.surfaces.each_ref().map(|surface| {
        surface
            .current
            .as_ref()
            .unwrap()
            .1
            .protocol
            .as_ref()
            .unwrap()
            .image_id()
    });
    assert_ne!(ids[0], ids[1]);
    assert!(ids.iter().all(|id| id & 2 == 0));
    for area in areas {
        assert!(
            terminal
                .backend()
                .buffer()
                .cell((area.x, area.y))
                .unwrap()
                .symbol()
                .starts_with('\u{10eeee}')
        );
    }
    intro.dismiss(Instant::now() - FADE, MotionMode::Full);
    terminal
        .draw(|frame| {
            assert!(!intro.render_miniviz(frame, areas[1], None, MotionMode::Full));
        })
        .unwrap();
    assert!(
        intro
            .surfaces
            .iter()
            .all(|surface| surface.current.is_none() && !surface.pending)
    );
    assert!(intro.worker.is_none());
}

#[test]
fn startup_intro_full_cockpit_wires_empty_shell_and_never_replays_after_draft() {
    let _guard = crate::tests::env_lock();
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    app.startup_intro = StartupIntro::default();
    app.visual_motion = MotionMode::Reduced;
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .startup_intro
        .surfaces
        .iter()
        .any(|surface| surface.current.is_none())
    {
        terminal
            .draw(|frame| crate::ui::draw::ui(frame, &mut app))
            .unwrap();
        assert!(
            Instant::now() < deadline,
            "full UI never painted both startup panes"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.startup_intro.visible);
    // Entering an actual room must retain the world even before typing;
    // only the still-empty agent shell keeps the launch ceremony.
    app.world
        .settle_at_for_test(crate::stage::world_viz::Building::Keep);
    assert!(app.world.enter_interior());
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    assert!(app.world_pane_visible);
    assert!(app.startup_intro.visible);
    app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.input, "a");
    app.startup_intro.fading = Some(Instant::now() - FADE);
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    assert!(app.startup_intro.finished);
    app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    assert!(app.input.is_empty());
    assert!(app.startup_intro.worker.is_none());
    assert!(!app.startup_intro.visible);
}

/// Visual proof sheet of the production renderer, saved the way the terminal
/// receives it: dots on transparency. Private output only.
#[test]
#[ignore = "writes a visual proof sheet from the production renderer"]
fn startup_intro_export_dotmax_proof() {
    let dir = std::env::var("ANGEL_T_INTRO_PROOF_DIR").expect("explicit proof output directory");
    let atlas = decode_atlas(ATLAS).unwrap();
    let geometry = DotGeometry::new(100, 35, (8, 16), 2).unwrap();
    let mut sheet = image::RgbaImage::new(geometry.width * 3, geometry.height * 2);
    for (i, (index, opacity)) in [
        (0, 255),
        (18, 255),
        (36, 255),
        (59, 255),
        (59, 128),
        (59, 20),
    ]
    .into_iter()
    .enumerate()
    {
        let key = Key {
            scene: Scene::Sword,
            frame: index,
            flow: None,
            water: 0,
            opacity,
            size: Size::new(0, 0),
            geometry: None,
        };
        let dots = compose(
            &atlas,
            &mut Cache::default(),
            &key,
            geometry.grid_width,
            geometry.grid_height,
        )
        .unwrap();
        // The proof carries the transported pixels: dots on transparency.
        let raster = geometry.rasterize_on(&dots, TRANSPARENT).unwrap();
        imageops::overlay(
            &mut sheet,
            &raster,
            (i % 3) as i64 * i64::from(geometry.width),
            (i / 3) as i64 * i64::from(geometry.height),
        );
    }
    sheet
        .save(std::path::Path::new(&dir).join("dotmax-proof.png"))
        .unwrap();
    let mini = DotGeometry::new(36, 16, (8, 16), 2).unwrap();
    let key = Key {
        scene: Scene::Mountain {
            tick: WIZARD_SETTLE,
        },
        frame: 0,
        flow: None,
        water: 0,
        opacity: 255,
        size: Size::new(0, 0),
        geometry: None,
    };
    let dots = compose(
        &atlas,
        &mut Cache::default(),
        &key,
        mini.grid_width,
        mini.grid_height,
    )
    .unwrap();
    mini.rasterize_on(&dots, TRANSPARENT)
        .unwrap()
        .save(std::path::Path::new(&dir).join("miniviz-proof.png"))
        .unwrap();

    let mut idle_sheet = image::RgbaImage::new(mini.width * 4, mini.height * 2);
    for pose in 0..WIZARD_IDLE_FRAMES {
        let dots =
            compose_mountain(WIZARD_SETTLE + pose, mini.grid_width, mini.grid_height, 255).unwrap();
        let raster = mini.rasterize_on(&dots, TRANSPARENT).unwrap();
        raster
            .save(std::path::Path::new(&dir).join(format!("wizard-idle-{pose}.png")))
            .unwrap();
        imageops::overlay(
            &mut idle_sheet,
            &raster,
            i64::from(pose % 4) * i64::from(mini.width),
            i64::from(pose / 4) * i64::from(mini.height),
        );
    }
    idle_sheet
        .save(std::path::Path::new(&dir).join("wizard-idle-proof.png"))
        .unwrap();

    let mut mountain_sheet = image::RgbaImage::new(mini.width * 3, mini.height * 2);
    for (i, tick) in [0, 16, 32, HOLD, HOLD + WIZARD_WALK_TICKS / 2, WIZARD_SETTLE]
        .into_iter()
        .enumerate()
    {
        let dots = compose_mountain(tick, mini.grid_width, mini.grid_height, 255).unwrap();
        let raster = mini.rasterize_on(&dots, TRANSPARENT).unwrap();
        imageops::overlay(
            &mut mountain_sheet,
            &raster,
            (i % 3) as i64 * i64::from(mini.width),
            (i / 3) as i64 * i64::from(mini.height),
        );
    }
    mountain_sheet
        .save(std::path::Path::new(&dir).join("mountain-sequence-proof.png"))
        .unwrap();

    // Timing proof: the actual sword and miniviz renderers sampled from the
    // same 12 fps clock. The smaller right frame is bottom-aligned, matching
    // its role as the cockpit's compact world window rather than a second
    // full-size presentation panel.
    let sword = DotGeometry::new(56, 20, (8, 16), 2).unwrap();
    let ticks = [0, 16, 32, HOLD, HOLD + WIZARD_WALK_TICKS / 2, WIZARD_SETTLE];
    let row_height = sword.height.max(mini.height);
    let mut timed = image::RgbaImage::new(sword.width + mini.width, row_height * 6);
    for (row, tick) in ticks.into_iter().enumerate() {
        let sword_key = Key {
            scene: Scene::Sword,
            frame: tick.min(HOLD),
            flow: None,
            water: 0,
            opacity: 255,
            size: Size::new(0, 0),
            geometry: None,
        };
        let sword_dots = compose(
            &atlas,
            &mut Cache::default(),
            &sword_key,
            sword.grid_width,
            sword.grid_height,
        )
        .unwrap();
        let sword_raster = sword.rasterize_on(&sword_dots, TRANSPARENT).unwrap();
        imageops::overlay(
            &mut timed,
            &sword_raster,
            0,
            i64::from(row as u32 * row_height),
        );

        let mountain_dots = compose_mountain(tick, mini.grid_width, mini.grid_height, 255).unwrap();
        let mountain_raster = mini.rasterize_on(&mountain_dots, TRANSPARENT).unwrap();
        imageops::overlay(
            &mut timed,
            &mountain_raster,
            i64::from(sword.width),
            i64::from(row as u32 * row_height + row_height - mini.height),
        );
    }
    timed
        .save(std::path::Path::new(&dir).join("startup-sequence-proof.png"))
        .unwrap();
}

/// Visual proof of the ambient loop: every tick of two cycles, as PNGs, so the
/// seam and the hand/hilt isolation can be seen rather than argued. Ignored by
/// default — it writes a frame sequence, it asserts nothing. Each frame is the
/// transported pixels: dots on transparency, never a plate behind them.
#[test]
#[ignore = "writes a visual proof sequence from the production renderer"]
fn startup_intro_export_water_flow_proof() {
    let dir = std::env::var("ANGEL_T_INTRO_PROOF_DIR").expect("explicit proof output directory");
    let atlas = decode_atlas(ATLAS).unwrap();
    let geometry = DotGeometry::new(100, 35, (8, 16), 2).unwrap();
    let mut cache = Cache::default();
    let ticks = (RIPPLE.len() as u128 + u128::from(SEAM)) * 2;
    for tick in 0..ticks {
        let key = Key {
            scene: Scene::Sword,
            frame: HOLD,
            flow: Some(Flow::at(tick)),
            // Past the handoff: the flow is fully in charge of the water.
            water: 255,
            opacity: 255,
            size: Size::new(geometry.width as u16, geometry.height as u16),
            geometry: None,
        };
        let dots = compose(
            &atlas,
            &mut cache,
            &key,
            geometry.grid_width,
            geometry.grid_height,
        )
        .unwrap();
        geometry
            .rasterize_on(&dots, TRANSPARENT)
            .unwrap()
            .save(std::path::Path::new(&dir).join(format!("flow-{tick:03}.png")))
            .unwrap();
    }
}

/// Sideways slack the band leaves in its pane: left, then right.
fn band_slack(area: Rect, band: Rect) -> (u16, u16) {
    (band.x - area.x, area.x + area.width - (band.x + band.width))
}

/// The intro band: one rule — never compose a surface wider than the dots that
/// can carry the art — plus a centring and a floor lock. The crop is nearly
/// square, so a wider pane never bought a wider blade; it only grew the canvas,
/// the fine-dot raster and the Braille grid the same blade was carried in, which
/// is what read as the width pixelating the clip.
#[test]
fn startup_intro_band_clamps_width_centres_and_stands_on_the_floor() {
    let fill = |rows: usize| (rows * 4 * CROP_W as usize / CROP_H as usize / 2) as u16;
    let cases = [
        // (pane, expected width, why)
        (
            Rect::new(0, 0, 200, 20),
            fill(20),
            "wide and short: the fit width",
        ),
        (
            Rect::new(0, 0, 300, 64),
            INTRO_MAX_COLUMNS,
            "tall: the hard cap",
        ),
        (Rect::new(7, 3, fill(10), 10), fill(10), "already the fit width"),
        (
            Rect::new(7, 3, 30, 10),
            fill(10),
            "narrower pane, still the fit",
        ),
        // Degenerate rows still yield a small band rather than a zero one.
        (Rect::new(4, 4, 30, 1), fill(1), "one row"),
    ];
    for (pane, width, why) in cases {
        let band = intro_band(pane);
        assert_eq!(band.width, width, "{why}: {pane:?}");
        assert!(band.width <= pane.width, "{why}: never wider than its pane");
        assert!(
            band.width <= INTRO_MAX_COLUMNS,
            "{why}: width genuinely clamped"
        );
        assert!(
            band.width <= fill(usize::from(pane.height)).max(1),
            "{why}: never wider than the art can fill"
        );
        // Centred, to within the odd column the split cannot share.
        let (left, right) = band_slack(pane, band);
        assert!(
            left.abs_diff(right) <= 1,
            "{why}: slack {left}/{right} is not a centring"
        );
        // Floor lock: the band is the pane's rows, so its foot is the pane's foot.
        assert_eq!(band.y, pane.y, "{why}: top held");
        assert_eq!(band.height, pane.height, "{why}: the rows are untouched");
        assert_eq!(
            band.y + band.height,
            pane.y + pane.height,
            "{why}: foot of the band is the foot of the pane"
        );
        // Banding a band is a no-op, so a banded caller and this rule cannot
        // disagree about the rectangle.
        assert_eq!(intro_band(band), band, "{why}: not idempotent");
    }
    // A zero-area pane is handed back untouched rather than turned into a
    // one-column band at the origin.
    let empty = Rect::new(9, 9, 0, 40);
    assert_eq!(intro_band(empty), empty);
    assert_eq!(intro_band(Rect::new(9, 9, 40, 0)), Rect::new(9, 9, 40, 0));
}

/// The render boundary, not just the helper: the Sword clip is banded before the
/// geometry is derived, so the composed canvas, the fine-dot raster and the
/// declared cell rect are one width, and nothing paints outside the band. A
/// narrower rect declared over a wider raster is what squeezed the clip.
#[test]
fn startup_intro_sword_draws_only_inside_its_band() {
    let pane = Rect::new(0, 0, 120, 24);
    let band = intro_band(pane);
    assert!(band.width < pane.width, "this pane must actually clamp");
    // Banded before the geometry, exactly as the transcript pane does it.
    let geometry = DotGeometry::new(band.width, band.height, (8, 16), 2).unwrap();
    // The raster is the band's pixels, cell for cell: no wider canvas exists for
    // the transport to squeeze into the band.
    assert_eq!(geometry.width, u32::from(band.width) * 8);
    let mut intro = StartupIntro {
        // Settled: the blade has landed and the water is in, so the band's floor
        // is the waterline rather than an empty early rise frame.
        started: Some(Instant::now() - Duration::from_secs(90)),
        ..Default::default()
    };
    let mut terminal = Terminal::new(TestBackend::new(pane.width, pane.height)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        terminal
            .draw(|frame| intro.render(frame, band, Some(geometry), MotionMode::Reduced))
            .unwrap();
        if intro.surfaces[0].current.is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "intro worker did not deliver");
        std::thread::sleep(Duration::from_millis(5));
    }
    let columns = usize::from(pane.width);
    let mut inked = 0usize;
    let mut floor_inked = 0usize;
    for (index, cell) in terminal.backend().buffer().content().iter().enumerate() {
        if cell.symbol() == " " {
            continue;
        }
        inked += 1;
        let x = (index % columns) as u16;
        let y = (index / columns) as u16;
        assert!(
            (band.x..band.x + band.width).contains(&x)
                && (band.y..band.y + band.height).contains(&y),
            "intro ink at ({x}, {y}) is outside the band {band:?}"
        );
        if y == band.y + band.height - 1 {
            floor_inked += 1;
        }
    }
    assert!(inked > 0, "the settled clip painted nothing to check");
    // The band is the aspect-fill width for its rows, so the fitted clip fills it
    // rather than letterboxing inside it.
    let band_area = usize::from(band.width) * usize::from(band.height);
    assert!(
        inked * 10 >= band_area * 9,
        "the clip fills only {inked}/{band_area} of its band"
    );
    // Floor lock: the waterline is the last row of the band, edge to edge.
    assert_eq!(
        floor_inked,
        usize::from(band.width),
        "the clip is not standing on the floor of its band: {floor_inked} of {} columns inked on the last row",
        band.width
    );
    assert!(!intro.finished);
    intro.dismiss(Instant::now(), MotionMode::Off);
    assert!(intro.finished && intro.surfaces[0].current.is_none());
}
