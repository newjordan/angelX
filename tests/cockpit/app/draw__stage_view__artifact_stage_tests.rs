use super::*;

#[cfg(feature = "scryglass-video")]
#[test]
fn native_video_stage_paints_real_mp4_pixels_without_owning_the_composer() {
    use ratatui::style::Color;
    let _guard = crate::tests::env_lock();
    let protocol =
        std::env::var("ANGEL_NATIVE_VIDEO_PROTOCOL").unwrap_or_else(|_| "halfblocks".into());
    assert!(matches!(protocol.as_str(), "halfblocks" | "iterm2"));
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", &protocol);
    // Manual rich-content review can supply an existing MP4; it is never
    // overwritten or removed by this test. CI uses the controlled color.
    let supplied = std::env::var_os("ANGEL_NATIVE_VIDEO_FIXTURE");
    let path = supplied
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "angel-native-video-stage-{}.mp4",
                std::process::id()
            ))
        });
    if supplied.is_none() {
        let generated = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x22cc88:s=48x32:r=4:d=1",
                "-pix_fmt",
                "yuv420p",
                "-y",
            ])
            .arg(&path)
            .status();
        if !generated.is_ok_and(|status| status.success()) {
            eprintln!("SKIP native Stage MP4 fixture: ffmpeg CLI unavailable");
            return;
        }
    }
    let mut app = App::preview(crate::viewer::Viewer::new());
    app.visual_motion = crate::viz::lifecycle_viz::MotionMode::Off;
    app.input = "preserve the operator draft λ".into();
    app.media.push(Media::Video {
        label: "Real pixel reel".into(),
        path: path.display().to_string(),
    });
    app.scryglass.reveal_media(0, true);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(72, 24)).unwrap();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(5);
    while !app.scryglass.media_ready() && Instant::now() < deadline {
        app.scryglass.begin_frame();
        terminal
            .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
            .unwrap();
        app.scryglass.finish_frame();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        app.scryglass.media_ready(),
        "actual Stage never presented the decoded MP4"
    );
    assert_eq!(app.scryglass.active_error(), None);
    assert_eq!(app.input, "preserve the operator draft λ");
    assert_eq!(app.scryglass.active_media(), Some(0));
    assert!(
        app.scryglass.video_status().2,
        "motion-off is one frame, then paused"
    );
    let buffer = terminal.backend().buffer();
    let native_png = if protocol == "iterm2" {
        use base64::Engine as _;
        let (width, height) = app
            .viewer
            .video_decode_viewport(ratatui::layout::Rect::new(0, 0, 72, 24));
        let raw = app
            .scryglass
            .video_frame(&app.media[0], width, height, 1, false)
            .unwrap()
            .0
            .unwrap();
        assert!(
            raw.rgba.width() > 200 && raw.rgba.height() > 100,
            "the decoder itself must retain native detail before protocol encoding"
        );
        eprintln!(
            "native Stage decoded source={}x{}",
            raw.rgba.width(),
            raw.rgba.height()
        );
        let sequence = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .find(|symbol| symbol.contains("]1337;File="))
            .expect("real iTerm2 payload in Stage cells");
        let data = sequence
            .split_once("]1337;File=")
            .unwrap()
            .1
            .split_once(':')
            .unwrap()
            .1;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.split('\u{7}').next().unwrap())
            .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert!(
            decoded.width() > 200 && decoded.height() > 100,
            "native payload must retain actual raster detail, not a halfblock grid"
        );
        assert!(u64::from(decoded.width()) * u64::from(decoded.height()) <= 120_000);
        assert!(
            sequence.len() < 700_000,
            "bounded raster must have a bounded terminal payload"
        );
        eprintln!(
            "native Stage iTerm2 decoded payload={}x{} png_bytes={} terminal_bytes={}",
            decoded.width(),
            decoded.height(),
            bytes.len(),
            sequence.len()
        );
        Some(decoded)
    } else {
        None
    };
    let pixel_cells = buffer.content.iter().filter(|cell| {
            if supplied.is_some() {
                return matches!(cell.symbol(), "▀" | "▄" | "█");
            }
            let video_color = |color| matches!(color, Color::Rgb(r, g, b) if r < 60 && g > 170 && b > 90 && b < 170);
            video_color(cell.fg) || video_color(cell.bg)
        }).count();
    assert!(
        native_png.is_some() || pixel_cells > 100,
        "real MP4 pixels must fill a meaningful surface, got {pixel_cells}"
    );
    eprintln!(
        "native Stage fixture={} first_present_ms={} pixel_cells={pixel_cells} (headless debug test; not terminal fps)",
        if supplied.is_some() {
            "supplied rich content"
        } else {
            "controlled green"
        },
        started.elapsed().as_millis()
    );
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert!(
        text.contains("Real pixel reel") && text.contains("Paused"),
        "{text}"
    );
    if let Ok(out) = std::env::var("ANGEL_NATIVE_VIDEO_DUMP") {
        if let Some(png) = native_png {
            // Exact decoded protocol payload, not a recolored or upscaled
            // screenshot. Physical terminal display remains unverified.
            png.save(out).unwrap();
        } else {
            // Actual TestBackend media pixels, not a source-frame substitution.
            let mut raster = image::RgbaImage::new(72, 48);
            let rgb = |color| match color {
                ratatui::style::Color::Rgb(r, g, b) => [r, g, b, 255],
                _ => [0, 0, 0, 255],
            };
            for y in 0..24 {
                for x in 0..72 {
                    let cell = &buffer[(x, y)];
                    let (top, bottom) = match cell.symbol() {
                        "▀" => (cell.fg, cell.bg),
                        "▄" => (cell.bg, cell.fg),
                        "█" => (cell.fg, cell.fg),
                        _ => (cell.bg, cell.bg),
                    };
                    raster.put_pixel(u32::from(x), u32::from(y) * 2, image::Rgba(rgb(top)));
                    raster.put_pixel(u32::from(x), u32::from(y) * 2 + 1, image::Rgba(rgb(bottom)));
                }
            }
            image::imageops::resize(&raster, 576, 384, image::imageops::FilterType::Nearest)
                .save(out)
                .unwrap();
        }
    }
    app.scryglass.return_to_world();
    if supplied.is_none() {
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn still_inspector_input_scope_pin_footer_and_cleanup() {
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::viewer::Viewer::new());
    app.scryglass
        .navigate(crate::scryglass::StageRoute::Explore(
            crate::world_viz::Building::Smithy,
        ));
    let route = app.scryglass.controller.route();
    app.media.push(Media::Image {
        label: "Inspector input fixture".into(),
        path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/agents/apollo-neutral.png")
            .display()
            .to_string(),
    });
    app.scryglass.reveal_media(0, false);
    let request = app.scryglass.media_request_id();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(72, 24)).unwrap();
    fn ready(app: &mut App, terminal: &mut ratatui::Terminal<ratatui::backend::TestBackend>) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            terminal
                .draw(|f| render_artifacts(f, app, f.area()))
                .unwrap();
            if app.viewer.inspector.viewport.is_some() && !app.viewer.inspector.loading {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{:?}",
                app.scryglass.active_error()
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
    let mouse = |kind, column, row| MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    ready(&mut app, &mut terminal);
    let rect = app.viewer.inspector.viewport.unwrap();
    assert!(rect.y >= 2, "source identity is not an image hit target");
    assert_eq!(
        app.world_buttons
            .iter()
            .filter(|(_, b)| matches!(b, WorldButton::Still(_)))
            .count(),
        3
    );
    assert!(
        app.world_buttons
            .iter()
            .any(|(_, b)| *b == WorldButton::Back)
    );
    let center = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    app.on_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        center.0,
        center.1,
    ));
    assert!(app.scryglass.active_pinned());
    assert_eq!(app.module_host.focused().unwrap().as_str(), "artifacts");
    app.on_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 0, 0));
    assert!(app.viewer.inspector.drag.is_none());
    app.on_mouse(mouse(MouseEventKind::ScrollUp, center.0, center.1));
    assert_eq!(app.viewer.inspector.percent(), 125);
    assert!(app.viewer.inspector.viewport.is_some());
    // One event batch, no intervening draw or worker completion: every
    // wheel tick changes the desired view, retaining the old painted map.
    let mut expected = app.viewer.inspector.view;
    let point = app.viewer.inspector.pointer(center.0, center.1);
    for _ in 0..9 {
        expected.zoom_at(
            true,
            point,
            app.viewer.inspector.source.unwrap(),
            app.viewer.inspector.pixels,
        );
        app.on_mouse(mouse(MouseEventKind::ScrollUp, center.0, center.1));
    }
    assert_eq!(app.viewer.inspector.view, expected);
    assert!(app.viewer.inspector.viewport.is_some());
    assert_ne!(app.viewer.inspector.painted, Some(expected));
    assert_eq!(app.viewer.inspector.semantic()["status"], "updating");
    // Six '+' presses are six steps, not eight (595% is eight steps).
    app.inspect_still(crate::still_inspector::Action::Fit);
    for _ in 0..6 {
        app.on_key(key(KeyCode::Char('+')));
    }
    assert_eq!(app.viewer.inspector.percent(), 381);
    assert!(
        app.viewer
            .inspector
            .status_label()
            .contains("inspect (of Fit)")
    );
    assert!(
        app.scryglass.active_pinned(),
        "repeated gesture must not toggle pin"
    );
    assert_eq!(app.scryglass.media_request_id(), request);
    assert_eq!(app.scryglass.controller.route(), route);
    // Draft navigation belongs to composer, including slash commands.
    for draft in ["my typed λ", "/model incomplete"] {
        app.input = draft.into();
        app.cursor = app.input.len();
        let view = app.viewer.inspector.view;
        app.on_key(key(KeyCode::Left));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Char('+')));
        assert_eq!(app.viewer.inspector.view, view);
        assert!(app.input.contains('+'));
        assert!(
            app.input
                .starts_with(draft.trim_end_matches(draft.chars().last().unwrap()))
        );
    }
    app.input.clear();
    app.cursor = 0;
    app.focus_module("core");
    let view = app.viewer.inspector.view;
    app.on_key(key(KeyCode::Char('-')));
    assert_eq!(app.viewer.inspector.view, view);
    assert_eq!(app.input, "-");
    app.input.clear();
    app.cursor = 0;
    app.focus_module("artifacts");
    let fit = app
        .world_buttons
        .iter()
        .find(|(_, b)| *b == WorldButton::Still(crate::still_inspector::Action::Fit))
        .unwrap()
        .0;
    app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), fit.x, fit.y));
    assert_eq!(app.viewer.inspector.view, Default::default());
    app.on_key(key(KeyCode::Char('+')));
    app.on_key(key(KeyCode::Char('0')));
    assert_eq!(app.viewer.inspector.view, Default::default());
    for _ in 0..8 {
        app.on_key(key(KeyCode::Char('+')));
    }
    ready(&mut app, &mut terminal);
    app.on_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        center.0,
        center.1,
    ));
    let before_pan = app.viewer.inspector.view;
    app.on_mouse(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        center.0 + 2,
        center.1,
    ));
    assert!(app.viewer.inspector.view.x < before_pan.x);
    assert!(app.viewer.inspector.viewport.is_some());
    app.on_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 0, 0));
    assert!(app.viewer.inspector.drag.is_none());
    ready(&mut app, &mut terminal);
    app.on_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        center.0,
        center.1,
    ));
    app.set_terminal_focused(false);
    assert!(app.viewer.inspector.drag.is_none());
    app.set_terminal_focused(true);
    app.on_key(key(KeyCode::Char('+')));
    ready(&mut app, &mut terminal);
    app.on_mouse(mouse(
        MouseEventKind::Down(MouseButton::Right),
        center.0,
        center.1,
    ));
    assert_eq!(app.viewer.inspector.percent(), 100);
    app.viewer.inspector.drag = Some(center);
    app.viewer.invalidate_still_layout();
    assert!(app.viewer.inspector.viewport.is_none());
    assert!(app.viewer.inspector.drag.is_none());
    ready(&mut app, &mut terminal);
    app.on_key(key(KeyCode::Esc));
    assert!(app.scryglass.controller.overlay().is_none());
    assert_eq!(app.scryglass.controller.route(), route);
    assert!(app.viewer.inspector.source.is_none());
    assert!(app.viewer.inspector.viewport.is_none());
    // Video/document/world controls retain their own routes; inspector keys
    // cannot change the still display state once the overlay is dismissed.
    for surface in [
        crate::scryglass::StageSurface::Video(0),
        crate::scryglass::StageSurface::Document(0),
        crate::scryglass::StageSurface::WorldMap,
    ] {
        app.scryglass.surface = surface;
        let view = app.viewer.inspector.view;
        app.input = "/draft".into();
        app.cursor = app.input.len();
        app.on_key(key(KeyCode::Char('+')));
        assert_eq!(app.input, "/draft+");
        assert_eq!(app.viewer.inspector.view, view);
    }
    app.input.clear();
    app.cursor = 0;
    // A nonempty but too-small scene must not retain the preceding hitbox.
    app.scryglass.reveal_media(0, true);
    ready(&mut app, &mut terminal);
    app.viewer.inspector.drag = Some(center);
    terminal
        .draw(|f| render_artifacts(f, &mut app, ratatui::layout::Rect::new(0, 0, 12, 6)))
        .unwrap();
    assert!(app.viewer.inspector.viewport.is_none());
    assert!(app.viewer.inspector.source.is_none());
    assert!(app.viewer.inspector.drag.is_none());
    // A hidden/root-zero frame clears all input/cache authority too.
    ready(&mut app, &mut terminal);
    app.viewer.inspector.drag = Some(center);
    let mut empty = ratatui::Terminal::new(ratatui::backend::TestBackend::new(0, 0)).unwrap();
    empty.draw(|f| crate::draw::ui(f, &mut app)).unwrap();
    assert!(app.viewer.inspector.source.is_none());
    assert!(app.viewer.inspector.drag.is_none());
}

#[test]
fn native_artifact_stage_keeps_identity_draft_and_motion_off_ownership() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::viewer::Viewer::new());
    app.visual_motion = crate::viz::lifecycle_viz::MotionMode::Off;
    app.input = "keep this exact draft λ".into();
    app.media.push(Media::Image {
        label: "Requested evidence".into(),
        path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/agents/apollo-neutral.png")
            .display()
            .to_string(),
    });
    app.scryglass.reveal_media(0, true);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(72, 24)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app.scryglass.media_ready() && Instant::now() < deadline {
        terminal
            .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        app.scryglass.media_ready(),
        "explicit images render even with motion off"
    );
    assert!(app.scryglass.active_error().is_none());
    assert_eq!(app.input, "keep this exact draft λ");
    assert_eq!(app.scryglass.active_media(), Some(0));
    assert!(app.scryglass.active_pinned());
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Requested evidence"), "{text}");
    assert!(text.contains("Source:"), "{text}");
    assert!(text.contains("apollo-neutral.png"), "{text}");
    assert!(!text.contains("Loading"), "{text}");
    let request = app.scryglass.media_request_id();
    app.scryglass.reveal_media(0, true);
    assert_ne!(app.scryglass.media_request_id(), request);
}
