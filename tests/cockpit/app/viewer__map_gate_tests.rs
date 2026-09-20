use super::*;

#[test]
fn native_video_iterm_payload_retains_details_above_halfblock_resolution() {
    use base64::Engine as _;
    use ratatui::{Terminal, backend::TestBackend};
    let mut viewer = Viewer::with_picker(Viewer::picker_for_protocol(ProtocolType::Iterm2));
    assert_eq!(
        viewer.video_decode_viewport(Rect::new(0, 0, 72, 24)),
        (192, 64)
    );
    let pixels = Arc::new(crate::scryglass::VideoPixels {
        identity: crate::scryglass::VideoIdentity {
            source: crate::media::MediaSource::operator(PathBuf::from("controlled-detail.mp4")),
            request_id: 1,
            generation: 1,
        },
        sequence: 1,
        rgba: image::RgbaImage::from_fn(320, 200, |x, _| {
            let value = if x / 4 % 2 == 0 { 0 } else { 255 };
            image::Rgba([value, value, value, 255])
        }),
    });
    let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut ready = false;
    while !ready && std::time::Instant::now() < deadline {
        terminal
            .draw(|frame| {
                ready = viewer
                    .render_video(frame, frame.area(), Arc::clone(&pixels))
                    .unwrap()
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(ready);
    let sequence = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .find(|symbol| symbol.contains("]1337;File="))
        .unwrap();
    let payload = sequence
        .split_once("]1337;File=")
        .unwrap()
        .1
        .split_once(':')
        .unwrap()
        .1
        .split('\u{7}')
        .next()
        .unwrap();
    let decoded = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(payload)
            .unwrap(),
    )
    .unwrap()
    .to_rgba8();
    assert!(decoded.width() > 300 && decoded.height() > 150);
    assert!(decoded.width() * decoded.height() <= MAX_INLINE_PREVIEW_PIXELS);
    assert!(sequence.len() < 700_000);
    let row: Vec<_> = (0..decoded.width())
        .map(|x| decoded.get_pixel(x, decoded.height() / 2)[0] > 127)
        .collect();
    let transitions = row.windows(2).filter(|pair| pair[0] != pair[1]).count();
    assert!(
        transitions >= 70,
        "{transitions} transitions: a 40-column halfblock grid would lose this detail"
    );
    eprintln!(
        "native iTerm2 worker payload={}x{} transitions={transitions} bytes={}",
        decoded.width(),
        decoded.height(),
        sequence.len()
    );
}

#[test]
fn native_video_pixels_are_bounded_exact_and_coalesce_without_stale_generations() {
    use ratatui::{Terminal, backend::TestBackend};
    let pixels = |generation, sequence, color| {
        Arc::new(crate::scryglass::VideoPixels {
            identity: crate::scryglass::VideoIdentity {
                source: crate::media::MediaSource::operator(PathBuf::from("controlled.mp4")),
                request_id: 7,
                generation,
            },
            sequence,
            rgba: image::RgbaImage::from_pixel(32, 24, image::Rgba(color)),
        })
    };
    let mut viewer = Viewer::with_picker(halfblock_picker());
    let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
    let red = pixels(1, 1, [230, 10, 20, 255]);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut ready = false;
    while !ready && std::time::Instant::now() < deadline {
        terminal
            .draw(|frame| {
                ready = viewer
                    .render_video(frame, frame.area(), Arc::clone(&red))
                    .unwrap()
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(ready);
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| { cell.fg == ratatui::style::Color::Rgb(230, 10, 20) }),
        "decoded color must reach actual terminal cells"
    );
    let (tx, rx) = mpsc::channel();
    let pending_key = viewer.video.as_ref().unwrap().0.clone();
    viewer.video_pending = Some(PendingVideo {
        key: pending_key.clone(),
        sequence: 2,
        rx,
    });
    terminal
        .draw(|frame| {
            assert!(
                viewer
                    .render_video(frame, frame.area(), pixels(1, 99, [10, 230, 20, 255]))
                    .unwrap(),
                "same-generation frame remains during a slow encode"
            );
        })
        .unwrap();
    assert_eq!(
        viewer.video_pending.as_ref().unwrap().sequence,
        2,
        "latest frames do not queue behind pending work"
    );
    for (generation, width, request_id, source) in [
        (2, 40, 7, "controlled.mp4"),
        (1, 20, 7, "controlled.mp4"),
        (1, 40, 8, "controlled.mp4"),
        (1, 40, 7, "other.mp4"),
    ] {
        let mut changed = pixels(generation, 3, [10, 230, 20, 255]);
        let identity = &mut Arc::get_mut(&mut changed).unwrap().identity;
        identity.request_id = request_id;
        identity.source.path = PathBuf::from(source);
        terminal
            .draw(|frame| {
                assert!(
                    !viewer
                        .render_video(frame, Rect::new(0, 0, width, 16), changed.clone())
                        .unwrap()
                );
            })
            .unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|cell| cell.symbol() == " "),
            "seek/reveal or resize must never paint an old protocol"
        );
    }
    drop(tx);
    terminal
        .draw(|frame| {
            assert!(
                !viewer
                    .render_video(frame, frame.area(), pixels(2, 4, [10, 230, 20, 255]))
                    .unwrap()
            );
        })
        .unwrap();
    assert_eq!(
        viewer
            .video_pending
            .as_ref()
            .unwrap()
            .key
            .identity
            .generation,
        2
    );

    for protocol in [
        ProtocolType::Halfblocks,
        ProtocolType::Kitty,
        ProtocolType::Iterm2,
        ProtocolType::Sixel,
    ] {
        let picker = Viewer::picker_for_protocol(protocol);
        let area = video_encode_area(&picker, 16, 16, Rect::new(0, 0, 192, 64));
        let font = picker.font_size();
        let pw = u32::from(area.width) * u32::from(font.width);
        let ph = u32::from(area.height) * u32::from(font.height);
        assert!(pw * ph <= MAX_INLINE_PREVIEW_PIXELS);
        assert!(
            pw.abs_diff(ph) <= u32::from(font.width.max(font.height)),
            "square media stays square in physical pixels"
        );
        picker
            .new_protocol(
                image::DynamicImage::ImageRgba8(red.rgba.clone()),
                area.into(),
                Resize::Scale(Some(image::imageops::FilterType::Triangle)),
            )
            .unwrap();
    }
}

#[test]
fn the_tile_map_accepts_every_real_graphics_protocol() {
    // This gate was originally copied from the WebGPU portal, which is
    // Kitty-only on purpose. That left the map dark on sixel terminals —
    // the protocol this project's own sessions actually negotiate — for no
    // reason, because a composed frame is just pixels.
    for protocol in [
        ProtocolType::Kitty,
        ProtocolType::Sixel,
        ProtocolType::Iterm2,
    ] {
        assert!(map_supported(protocol), "{protocol:?} should carry the map");
    }
}

#[test]
fn sixel_prefers_living_braille_unless_backed_map_forced() {
    // Capability stays open (operator can force plates), but the default
    // picture on sixel is the living braille world — static sixel plates
    // were reading as a broken still.
    let _lock = crate::tests::env_lock();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_BACKED_MAP") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_TILE_MAP") };
    assert!(map_supported(ProtocolType::Sixel));
    assert!(!map_desired(ProtocolType::Sixel));
    assert!(map_desired(ProtocolType::Kitty));
    assert!(map_desired(ProtocolType::Iterm2));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_BACKED_MAP", "1") };
    assert!(map_desired(ProtocolType::Sixel));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_BACKED_MAP", "0") };
    assert!(!map_desired(ProtocolType::Kitty));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_BACKED_MAP") };
}

#[test]
fn halfblocks_falls_back_to_the_braille_map() {
    // Excluded on resolution, not capability: one pixel per cell half
    // cannot hold a 512px island, and braille reads better than mush.
    assert!(!map_supported(ProtocolType::Halfblocks));
}

#[test]
fn artifact_still_pixels_preserve_portrait_rgb_flat_opacity_and_fail_closed() {
    use ratatui::{Terminal, backend::TestBackend, style::Color};
    let root = std::env::temp_dir().join(format!("angel-artifact-rgb-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let color = [34, 153, 221];
    for (name, alpha) in [
        ("flat.png", 255),
        ("transparent.png", 0),
        ("partial.png", 128),
    ] {
        image::RgbaImage::from_pixel(16, 16, image::Rgba([color[0], color[1], color[2], alpha]))
            .save(root.join(name))
            .unwrap();
    }
    std::fs::write(root.join("corrupt.png"), b"not a png").unwrap();
    let portrait =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
    for (index, path) in [
        portrait,
        root.join("flat.png"),
        root.join("transparent.png"),
        root.join("partial.png"),
        root.join("corrupt.png"),
    ]
    .into_iter()
    .enumerate()
    {
        let source = crate::media::MediaSource::operator(path);
        let mut viewer = Viewer::with_picker(halfblock_picker());
        let (width, height) = if index == 0 { (52, 20) } else { (8, 4) };
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        for area in [Rect::new(0, 0, 0, height), Rect::new(0, 0, width, 0)] {
            terminal
                .draw(|frame| {
                    assert!(
                        !viewer
                            .render_artifact(frame, area, source.clone(), 1)
                            .unwrap()
                    )
                })
                .unwrap();
            assert!(
                viewer.artifact_pending.is_none(),
                "empty geometry must not start decoding"
            );
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut result = Ok(false);
        while matches!(result, Ok(false)) && std::time::Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    result = viewer.render_artifact(frame, frame.area(), source.clone(), 1)
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        let cells = &terminal.backend().buffer().content;
        if index == 4 {
            assert!(result.is_err(), "corrupt requested image must fail");
            assert!(cells.iter().all(|cell| cell.symbol() == " "));
            continue;
        }
        assert_eq!(
            result,
            Ok(true),
            "actual artifact pixels must finish encoding"
        );
        let source_color = Color::Rgb(color[0], color[1], color[2]);
        match index {
            0 => {
                assert!(
                    cells.iter().filter(|cell| cell.symbol() != " ").count() > cells.len() / 4,
                    "portrait must remain dense and legible"
                );
                assert!(cells.iter().any(|cell| [cell.fg, cell.bg].iter().any(|color| matches!(color, Color::Rgb(r, g, b) if !crate::terminal_art::DMD_PALETTE.contains(&[*r, *g, *b])))), "artifact RGB must not collapse to the DMD palette");
            }
            1 => assert!(
                // Uniform halfblocks use a colored space: its background
                // still covers the whole terminal cell opaquely.
                cells
                    .iter()
                    .all(|cell| cell.fg == source_color && cell.bg == source_color),
                "opaque flat image must fill its exact-fit raster with source RGB"
            ),
            2 => assert!(
                cells
                    .iter()
                    .all(|cell| cell.fg == Color::Rgb(0, 0, 0) && cell.bg == Color::Rgb(0, 0, 0)),
                "transparent RGB must contribute no ink to the black canvas"
            ),
            3 => {
                assert!(
                    cells.iter().all(|cell| cell.fg == Color::Rgb(17, 76, 110)
                        && cell.bg == Color::Rgb(17, 76, 110)),
                    "partial alpha must blend exact source channels against black"
                )
            }
            _ => unreachable!(),
        }
    }
    use base64::Engine as _;
    let source = crate::media::MediaSource::operator(root.join("partial.png"));
    let mut viewer = Viewer::with_picker(Viewer::picker_for_protocol(ProtocolType::Iterm2));
    let mut terminal = Terminal::new(TestBackend::new(8, 4)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut painted = false;
    while !painted && std::time::Instant::now() < deadline {
        terminal
            .draw(|frame| {
                painted = viewer
                    .render_artifact_with_picker(
                        frame,
                        frame.area(),
                        source.clone(),
                        1,
                        still_pixel_picker(ProtocolType::Iterm2, Some((10, 20))),
                    )
                    .unwrap()
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(painted);
    let sequence = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .find(|symbol| symbol.contains("]1337;File="))
        .unwrap();
    let payload = sequence
        .split_once("]1337;File=")
        .unwrap()
        .1
        .split_once(':')
        .unwrap()
        .1
        .split('\u{7}')
        .next()
        .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .unwrap();
    let native = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert!(
        native.pixels().all(|pixel| pixel.0 == [34, 153, 221, 128]),
        "native artifact payload must preserve source RGB and alpha"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn still_inspector_native_geometry_pending_continuity_and_failure() {
    use crate::still_inspector::Action;
    use ratatui::{Terminal, backend::TestBackend};
    let source = crate::media::MediaSource::operator(PathBuf::from("not-opened-geometry-test.png"));
    let decoded = Arc::new(image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_pixel(112, 112, image::Rgba([240, 60, 30, 255])),
    ));
    let mut viewer = Viewer::with_picker(Viewer::picker_for_protocol(ProtocolType::Kitty));
    let mut terminal = Terminal::new(TestBackend::new(16, 8)).unwrap();
    let scene = Rect::new(2, 1, 8, 4);
    // Prime identity/source, not protocol; the real worker encodes with the
    // same ioctl-derived picker that defines the cache and pointer mapping.
    viewer.artifact_identity = Some((source.clone(), 1));
    viewer.artifact_decoded = Some(decoded);
    let draw = |viewer: &mut Viewer, terminal: &mut Terminal<TestBackend>, font| {
        let mut ready = false;
        terminal
            .draw(|f| {
                ready = viewer
                    .render_artifact_with_picker(
                        f,
                        scene,
                        source.clone(),
                        1,
                        still_pixel_picker(ProtocolType::Kitty, font),
                    )
                    .unwrap()
            })
            .unwrap();
        ready
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !draw(&mut viewer, &mut terminal, Some((14, 28))) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(viewer.inspector.pixels, (112, 112));
    assert_eq!(viewer.inspector.pointer(2, 1), (7.0, 14.0));
    let symbols: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        symbols.contains("s=112,v=112"),
        "native payload must match actual cells, not 10x20"
    );
    let original = viewer.artifact.as_ref().unwrap().0.clone();
    let original_view = viewer.inspector.painted;
    viewer.inspector.action(Action::ZoomIn);
    let mut blocked = original.clone();
    blocked.view = viewer.inspector.view;
    let (tx, rx) = mpsc::channel();
    viewer.artifact_pending = Some(PendingArtifact { key: blocked, rx });
    viewer.inspector.drag = Some((4, 2));
    for _ in 0..6 {
        viewer.inspector.zoom(true, (56.0, 56.0));
        assert!(!draw(&mut viewer, &mut terminal, Some((14, 28))));
        assert_eq!(viewer.inspector.viewport, Some(scene));
        assert_eq!(viewer.inspector.painted, original_view);
        assert!(viewer.inspector.loading);
        assert_eq!(viewer.inspector.drag, Some((4, 2)));
        assert_eq!(viewer.artifact.as_ref().unwrap().0, original);
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|c| c.symbol().contains('\u{10eeee}'))
        );
    }
    // Any same-scene encoder failure cancels old authority, even if desired
    // view advanced. No stale-success ready flag and no faulted old image.
    tx.send(Err("controlled encoder failure".into())).unwrap();
    terminal
        .draw(|f| {
            assert!(
                viewer
                    .render_artifact_with_picker(
                        f,
                        scene,
                        source.clone(),
                        1,
                        still_pixel_picker(ProtocolType::Kitty, Some((14, 28)))
                    )
                    .is_err()
            )
        })
        .unwrap();
    assert!(viewer.inspector.viewport.is_none());
    assert!(viewer.inspector.painted.is_none());
    assert!(viewer.inspector.drag.is_none());
    assert!(viewer.artifact.is_none());

    // Explicit resize epoch prevents A→B→A resurrecting a pending protocol.
    viewer.artifact_identity = Some((source.clone(), 1));
    let (tx, rx) = mpsc::channel();
    viewer.artifact_pending = Some(PendingArtifact {
        key: original.clone(),
        rx,
    });
    viewer.invalidate_still_layout();
    assert!(!draw(&mut viewer, &mut terminal, Some((16, 32))));
    assert!(viewer.inspector.viewport.is_none());
    assert_eq!(viewer.inspector.pixels, (128, 128));
    assert!(!draw(&mut viewer, &mut terminal, Some((14, 28))));
    assert_ne!(viewer.still_layout_epoch, original.layout_epoch);
    assert!(!draw(&mut viewer, &mut terminal, None));
    assert_eq!(viewer.inspector.pixels, (16, 16));
    assert_eq!(viewer.still_protocol, Some(ProtocolType::Halfblocks));
    assert!(viewer.inspector.viewport.is_none());
    drop(tx);
    viewer.clear_still();
}

#[test]
fn still_inspector_source_decode_once_coalesces_and_cleans_error_identity_resize() {
    use crate::still_inspector::Action;
    use ratatui::{Terminal, backend::TestBackend};
    let root = std::env::temp_dir().join(format!(
        "angel-still-detail-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("detail.png");
    let mut raster = image::RgbaImage::from_pixel(2048, 2048, image::Rgba([27, 41, 53, 255]));
    raster.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
    raster.put_pixel(2047, 2047, image::Rgba([0, 255, 0, 255]));
    raster.save(&path).unwrap();
    let source = crate::media::MediaSource {
        path,
        root: Some(root.clone()),
    };
    let decoded = artifact_preview(&source).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (2048, 2048),
        "decoder must not return the 120k thumbnail"
    );
    assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0, [255, 0, 0, 255]);
    drop(decoded);
    let mut viewer = Viewer::with_picker(halfblock_picker());
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    fn ready(
        viewer: &mut Viewer,
        terminal: &mut Terminal<TestBackend>,
        area: Rect,
        source: &crate::media::MediaSource,
        request: u64,
    ) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let mut painted = false;
            terminal
                .draw(|f| {
                    painted = viewer
                        .render_artifact(f, area, source.clone(), request)
                        .unwrap()
                })
                .unwrap();
            if painted {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let area = Rect::new(2, 3, 48, 16);
    ready(&mut viewer, &mut terminal, area, &source, 1);
    let allocation = Arc::as_ptr(viewer.artifact_decoded.as_ref().unwrap());
    // Once opened, gestures and resize must succeed even with file removed.
    std::fs::remove_file(&source.path).unwrap();
    viewer.inspector.action(Action::ZoomIn);
    terminal
        .draw(|f| assert!(!viewer.render_artifact(f, area, source.clone(), 1).unwrap()))
        .unwrap();
    let pending = viewer.artifact_pending.as_ref().unwrap().key.clone();
    assert!(viewer.inspector.viewport.is_some());
    assert!(viewer.inspector.loading);
    assert_ne!(viewer.inspector.painted, Some(viewer.inspector.view));
    for _ in 0..12 {
        viewer.inspector.action(Action::ZoomIn);
    }
    viewer.inspector.action(Action::Right);
    viewer.inspector.drag = Some((10, 10));
    let resized = Rect::new(4, 2, 40, 20);
    ready(&mut viewer, &mut terminal, resized, &source, 1);
    assert!(viewer.inspector.drag.is_none());
    assert_eq!(viewer.inspector.viewport, Some(resized));
    let admitted = &viewer.artifact.as_ref().unwrap().0;
    assert_ne!(admitted, &pending);
    assert_eq!(admitted.view, viewer.inspector.view);
    assert_eq!(admitted.scene, resized);
    assert_eq!(
        Arc::as_ptr(viewer.artifact_decoded.as_ref().unwrap()),
        allocation,
        "all gestures reuse the single source allocation"
    );
    assert_eq!(
        Arc::strong_count(viewer.artifact_decoded.as_ref().unwrap()),
        1
    );
    // Same path + new request must NOT use cached decoded data. Missing file
    // produces an error and clears every input/pixel/cache authority.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut fault = None;
    while fault.is_none() && std::time::Instant::now() < deadline {
        terminal
            .draw(|f| fault = viewer.render_artifact(f, resized, source.clone(), 2).err())
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(fault.is_some());
    assert!(viewer.artifact_decoded.is_none());
    assert!(viewer.artifact.is_none());
    assert!(viewer.inspector.source.is_none());
    assert!(viewer.inspector.viewport.is_none());
    assert!(viewer.inspector.drag.is_none());
    assert!(viewer.artifact_identity.is_none());
    terminal
        .draw(|f| {
            assert!(
                !viewer
                    .render_artifact(f, Rect::default(), source.clone(), 3)
                    .unwrap()
            )
        })
        .unwrap();
    assert!(viewer.artifact_pending.is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_artifact_pixels_preserve_source_resize_and_failure_identity() {
    use ratatui::{Terminal, backend::TestBackend};
    const ASYNC_IMAGE_TEST_TIMEOUT: Duration = Duration::from_secs(10);
    let root = std::env::temp_dir().join(format!(
        "angel-artifact-pixels-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("requested.png");
    image::RgbaImage::from_pixel(24, 16, image::Rgba([230, 40, 20, 255]))
        .save(&path)
        .unwrap();
    let source = crate::media::MediaSource {
        path,
        root: Some(root.clone()),
    };
    let decoded = artifact_preview(&source).unwrap();
    assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0, [230, 40, 20, 255]);
    let mut viewer = Viewer::with_picker(halfblock_picker());
    let mut terminal = Terminal::new(TestBackend::new(32, 16)).unwrap();
    // Browsing cannot abandon a running decoder and launch another one.
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let blocked_key = ArtifactKey {
        request_id: 0,
        source: source.clone(),
        width: 32,
        height: 8,
        scene: Rect::new(0, 0, 32, 8),
        font: HALFBLOCK_FONT_SIZE,
        protocol: ProtocolType::Halfblocks,
        layout_epoch: 0,
        view: Default::default(),
    };
    viewer.artifact_pending = Some(PendingArtifact {
        key: blocked_key.clone(),
        rx: blocked_rx,
    });
    for width in [8, 16, 32] {
        terminal
            .draw(|frame| {
                assert!(
                    !viewer
                        .render_artifact(frame, Rect::new(0, 0, width, 8), source.clone(), 1,)
                        .unwrap()
                );
            })
            .unwrap();
        assert_eq!(viewer.artifact_pending.as_ref().unwrap().key, blocked_key);
    }
    blocked_tx
        .send(Ok(PreparedArtifact {
            decoded: Arc::new(decoded.clone()),
            protocol: halfblock_picker()
                .new_protocol(decoded, Rect::new(0, 0, 32, 8).into(), Resize::Fit(None))
                .unwrap(),
        }))
        .unwrap();
    terminal
        .draw(|frame| {
            assert!(
                !viewer
                    .render_artifact(frame, Rect::new(0, 0, 32, 8), source.clone(), 1,)
                    .unwrap(),
                "a completed older reveal cannot satisfy a newer same-path request"
            );
        })
        .unwrap();
    assert_eq!(viewer.artifact_pending.as_ref().unwrap().key.request_id, 1);
    for area in [Rect::new(0, 0, 32, 16), Rect::new(0, 0, 16, 8)] {
        let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        let mut painted = false;
        terminal
            .draw(|frame| {
                assert!(
                    !viewer
                        .render_artifact(frame, area, source.clone(), 1)
                        .unwrap()
                );
            })
            .unwrap();
        while !painted && std::time::Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    painted = viewer
                        .render_artifact(frame, area, source.clone(), 1)
                        .unwrap();
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(painted, "actual halfblock pixels must finish encoding");
        assert!(terminal.backend().buffer().content.iter().any(|cell| {
            cell.fg == ratatui::style::Color::Rgb(230, 40, 20)
                || cell.bg == ratatui::style::Color::Rgb(230, 40, 20)
        }));
    }
    image::RgbaImage::from_pixel(24, 16, image::Rgba([20, 210, 50, 255]))
        .save(&source.path)
        .unwrap();
    let mut refreshed = false;
    terminal
        .draw(|frame| {
            assert!(
                !viewer
                    .render_artifact(frame, Rect::new(0, 0, 16, 8), source.clone(), 2,)
                    .unwrap(),
                "an explicit reopen must reload changed bytes at the same path"
            );
        })
        .unwrap();
    let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
    while !refreshed && std::time::Instant::now() < deadline {
        terminal
            .draw(|frame| {
                refreshed = viewer
                    .render_artifact(frame, Rect::new(0, 0, 16, 8), source.clone(), 2)
                    .unwrap();
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(refreshed);
    assert!(terminal.backend().buffer().content.iter().any(|cell| {
        cell.fg == ratatui::style::Color::Rgb(20, 210, 50)
            || cell.bg == ratatui::style::Color::Rgb(20, 210, 50)
    }));
    let missing = crate::media::MediaSource {
        path: root.join("missing.png"),
        root: Some(root.clone()),
    };
    terminal
        .draw(|frame| {
            assert!(
                !viewer
                    .render_artifact(frame, frame.area(), missing.clone(), 3)
                    .unwrap()
            );
        })
        .unwrap();
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .all(|cell| cell.symbol() == " "),
        "a missing requested artifact must not paint the previous image"
    );
    let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
    let mut failed = false;
    while !failed && std::time::Instant::now() < deadline {
        terminal
            .draw(|frame| {
                failed = viewer
                    .render_artifact(frame, frame.area(), missing.clone(), 3)
                    .is_err();
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(failed);
    #[cfg(unix)]
    {
        let link = root.join("escape.png");
        std::os::unix::fs::symlink(&source.path, &link).unwrap();
        assert!(
            artifact_preview(&crate::media::MediaSource {
                path: link,
                root: Some(root.clone())
            })
            .is_err(),
            "confined sources retain no-follow protection"
        );
    }
    let oversized = root.join("oversized.png");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        artifact_preview(&crate::media::MediaSource {
            path: oversized,
            root: Some(root.clone())
        })
        .unwrap_err()
        .contains("64 MiB")
    );
    std::fs::remove_dir_all(&root).unwrap();
}
