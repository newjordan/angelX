use super::*;

#[test]
fn asset_shelf_keeps_selected_source_and_copy_actions_visible_at_compact_widths() {
    use crate::ui::scryglass::StageCopyTarget;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    for index in 0..15 {
        app.media.push(Media::Image {
            label: format!("Plot {index}"),
            path: format!("/tmp/plot-{index}.png"),
        });
    }
    app.scryglass.open_assets();
    app.scryglass.selected = 14;
    for width in [24, 36, 72] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 14)).unwrap();
        terminal
            .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
            .unwrap();
        let text = test_backend_text(terminal.backend());
        assert!(
            text.contains("Plot 14") && text.contains("plot-14.png"),
            "{width}: {text}"
        );
        for button in [
            WorldButton::AssetSelect(14),
            WorldButton::AssetCopy(StageCopyTarget::Image),
            WorldButton::AssetCopy(StageCopyTarget::Location),
            WorldButton::Tower,
            WorldButton::Back,
        ] {
            let (rect, _) = app
                .world_buttons
                .iter()
                .find(|(_, candidate)| *candidate == button)
                .unwrap_or_else(|| panic!("{button:?} missing at {width}: {text}"));
            assert!(rect.width > 0 && rect.height > 0);
            assert!(rect.right() <= width && rect.bottom() <= 14);
        }
        assert!(
            app.scryglass.active_media().is_none(),
            "shelf must not start a preview"
        );
        assert!(app.scryglass.visible);
    }
}

#[test]
fn empty_asset_shelf_has_a_local_file_onramp_and_a_route_back_to_the_world() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.scryglass.open_assets();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(72, 18)).unwrap();
    terminal
        .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
        .unwrap();
    let text = test_backend_text(terminal.backend());
    assert!(
        text.contains("palantir is quiet") && text.contains("/show <path>"),
        "{text}"
    );
    for button in [WorldButton::Tower, WorldButton::Back] {
        assert!(
            app.world_buttons
                .iter()
                .any(|(_, candidate)| *candidate == button),
            "{text}"
        );
    }
    assert!(
        !app.world_buttons
            .iter()
            .any(|(_, button)| matches!(button, WorldButton::AssetCopy(_)))
    );
    assert_eq!(
        app.scryglass.surface,
        crate::ui::scryglass::StageSurface::Assets
    );
}

#[test]
fn active_asset_rail_exposes_copy_path_and_shelf_without_document_zoom_controls() {
    use crate::ui::scryglass::StageCopyTarget;
    let _guard = crate::tests::env_lock();
    let _protocol = crate::tests::TestEnvGuard::set("ANGEL_IMAGE_PROTOCOL", "halfblocks");
    let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    crate::drive::comp_mode::invalidate_cache();
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.media.push(Media::Image {
        label: "Evidence image".into(),
        path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/agents/apollo-neutral.png")
            .display()
            .to_string(),
    });
    app.media.push(Media::Link {
        label: "Graph source".into(),
        url: "https://example.invalid/graph".into(),
    });
    app.media.push(Media::Graph {
        label: "Raster chart".into(),
        url: app.media[0].target(),
    });
    for (index, copy_target) in [
        (0, StageCopyTarget::Image),
        (1, StageCopyTarget::Location),
        (2, StageCopyTarget::Image),
    ] {
        app.scryglass.reveal_media(index, true);
        for width in [36, 72] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 20)).unwrap();
            terminal
                .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
                .unwrap();
            let text = test_backend_text(terminal.backend());
            for label in ["[Path]", "[Assets]", "[Copy"] {
                assert!(text.contains(label), "{label} missing at {width}: {text}");
            }
            for button in [
                WorldButton::AssetCopy(copy_target),
                WorldButton::AssetCopy(StageCopyTarget::Location),
                WorldButton::Assets,
            ] {
                assert!(
                    app.world_buttons
                        .iter()
                        .any(|(_, candidate)| *candidate == button),
                    "{button:?} missing at {width}: {text}"
                );
            }
            if index != 1 {
                for action in [
                    crate::ui::still_inspector::Action::ZoomOut,
                    crate::ui::still_inspector::Action::ZoomIn,
                    crate::ui::still_inspector::Action::Fit,
                ] {
                    assert!(
                        app.world_buttons
                            .iter()
                            .any(|(_, button)| *button == WorldButton::Still(action)),
                        "image zoom control missing at {width}: {text}"
                    );
                }
            } else {
                assert_eq!(
                    app.scryglass.surface,
                    crate::ui::scryglass::StageSurface::Document(1)
                );
                assert!(
                    !app.world_buttons
                        .iter()
                        .any(|(_, button)| matches!(button, WorldButton::Still(_))),
                    "documents cannot offer image zoom: {text}"
                );
            }
        }
    }
}

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
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Off;
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
    let payload = app
        .scryglass
        .copy_payload(&app.media, crate::ui::scryglass::StageCopyTarget::Image)
        .expect("paused decoded frame is copyable");
    assert!(
        payload.text.is_empty(),
        "frame copy must not substitute a path"
    );
    let Some(crate::ui::clipboard::ClipboardImageSource::Frame(frame)) = payload.image else {
        panic!("video copy must own a decoded frame");
    };
    assert_eq!(frame.identity.source, app.media[0].source().unwrap());
    assert_eq!(frame.identity.request_id, app.scryglass.media_request_id());
    assert!(frame.identity.generation > 0);
    assert!(frame.rgba.width() > 0 && frame.rgba.height() > 0);
    if supplied.is_none() {
        let pixel = frame.rgba.get_pixel(0, 0);
        assert!(
            pixel[1] > 180 && pixel[0] < 70,
            "copy holds the fixture's decoded green pixels, not a placeholder: {pixel:?}"
        );
    }
    let again = app
        .scryglass
        .copy_payload(&app.media, crate::ui::scryglass::StageCopyTarget::Image)
        .unwrap();
    let Some(crate::ui::clipboard::ClipboardImageSource::Frame(again)) = again.image else {
        panic!("paused frame should remain available");
    };
    assert!(std::sync::Arc::ptr_eq(&frame, &again));
    assert_eq!(frame.identity.generation, again.identity.generation);
    // A card switch cannot borrow the previous decoder's frame before a draw.
    let original = std::mem::replace(
        &mut app.media[0],
        Media::Video {
            label: "different video".into(),
            path: path.with_extension("different.mp4").display().to_string(),
        },
    );
    assert!(
        app.scryglass
            .copy_payload(&app.media, crate::ui::scryglass::StageCopyTarget::Image)
            .is_err()
    );
    app.media[0] = original;
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
fn stage_copy_still_inspector_input_scope_pin_footer_and_cleanup() {
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.scryglass
        .navigate(crate::ui::scryglass::StageRoute::Explore(
            crate::stage::world_viz::Building::Smithy,
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
    app.inspect_still(crate::ui::still_inspector::Action::Fit);
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
        .find(|(_, b)| *b == WorldButton::Still(crate::ui::still_inspector::Action::Fit))
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
    let mut fit_click = mouse(MouseEventKind::Down(MouseButton::Right), center.0, center.1);
    // Plain right-click now copies the artifact location. Ctrl retains Fit.
    fit_click.modifiers = KeyModifiers::CONTROL;
    app.on_mouse(fit_click);
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
        crate::ui::scryglass::StageSurface::Video(0),
        crate::ui::scryglass::StageSurface::Document(0),
        crate::ui::scryglass::StageSurface::WorldMap,
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
    empty.draw(|f| crate::ui::draw::ui(f, &mut app)).unwrap();
    assert!(app.viewer.inspector.source.is_none());
    assert!(app.viewer.inspector.drag.is_none());
}

#[test]
fn native_artifact_stage_keeps_identity_draft_and_motion_off_ownership() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::new());
    app.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Off;
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
    assert!(text.contains("apollo-neutral.png"), "{text}");
    for (label, button) in [
        (
            "[Copy image]",
            WorldButton::AssetCopy(crate::ui::scryglass::StageCopyTarget::Image),
        ),
        (
            "[Path]",
            WorldButton::AssetCopy(crate::ui::scryglass::StageCopyTarget::Location),
        ),
        ("[Assets]", WorldButton::Assets),
    ] {
        assert!(text.contains(label), "{label} must be visible: {text}");
        assert!(
            app.world_buttons
                .iter()
                .any(|(_, candidate)| *candidate == button),
            "{label} must be clickable"
        );
    }
    assert!(!text.contains("Loading"), "{text}");
    let request = app.scryglass.media_request_id();
    app.scryglass.reveal_media(0, true);
    assert_ne!(app.scryglass.media_request_id(), request);
}

#[test]
fn dungeon_miniviz_stays_the_realm_while_a_delve_is_on() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.dungeon_command(Some("start"));
    app.collapse_dungeon();
    app.input = "Tune my sword".into();
    app.cursor = 5;
    let raid = app.dungeon.shooter.as_ref().unwrap().raid_id;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(64, 24)).unwrap();
    terminal
        .draw(|frame| render_artifacts(frame, &mut app, frame.area()))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        !text.contains("DELVE · F4"),
        "the mini-viz is the realm, not the room: {text}"
    );
    assert_eq!(app.input, "Tune my sword");
    assert!(!app.dungeon.expanded);
    assert!(app.dungeon.guest.is_none());
    assert_eq!(app.dungeon.shooter.as_ref().unwrap().raid_id, raid);
}
