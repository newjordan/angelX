#[test]
fn r04c_cont2_multiplexer_markers_choose_query_free_fallback() {
    assert!(super::multiplexer_markers(
        Some("tmux-256color"),
        None,
        false
    ));
    assert!(super::multiplexer_markers(
        Some("screen-256color"),
        None,
        false
    ));
    assert!(super::multiplexer_markers(
        Some("xterm-256color"),
        None,
        true
    ));
    assert!(super::multiplexer_markers(None, Some("tmux"), false));
    assert!(!super::multiplexer_markers(
        Some("xterm-256color"),
        None,
        false
    ));
}

#[test]
fn status_report_reply_is_recognised_inside_noise() {
    assert!(super::status_report_answered(b"garbage\x1b[0nmore"));
    assert!(super::status_report_answered(b"\x1b[3n"));
    assert!(!super::status_report_answered(b"\x1b[5n\x1b[?1;2c"));
    assert!(!super::status_report_answered(b""));
}

use super::*;
use std::time::Duration;

const ASYNC_IMAGE_TEST_TIMEOUT: Duration = Duration::from_secs(10);

fn assert_responsive_draw_samples(samples: &mut [Duration], label: &str) {
    assert!(
        samples.len() >= 2,
        "{label} needs a queueing and a promotion draw"
    );
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    assert!(
        median < Duration::from_millis(50),
        "{label} blocked typical cockpit draws: median {median:?}"
    );
}

#[test]
fn ambient_halfblocks_use_one_resident_encode_and_keep_map_policy() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    let _guard = crate::tests::env_lock();
    let mut viewer = Viewer::with_picker(halfblock_picker());
    let mut terminal = Terminal::new(TestBackend::new(40, 14)).expect("test terminal");
    let composed = AtomicUsize::new(0);
    let compose = || {
        composed.fetch_add(1, Ordering::SeqCst);
        ([120, 72, 32, 255].repeat(256 * 224), 256, 224)
    };
    terminal
        .draw(|frame| {
            assert!(!viewer.render_native_location(frame, frame.area(), 77, compose));
        })
        .unwrap();
    assert_eq!(
        composed.load(Ordering::SeqCst),
        0,
        "live map still uses braille"
    );
    let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
    let mut ready = false;
    while !ready && Instant::now() < deadline {
        terminal
            .draw(|frame| {
                ready = viewer.render_ambient_location(frame, frame.area(), 88, compose);
            })
            .unwrap();
        if !ready {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    assert!(ready, "halfblock image worker did not finish");
    assert_eq!(composed.load(Ordering::SeqCst), 1);
    assert!(viewer.map_worker.is_some());
    assert!(viewer.map_pending.is_none());
    assert!(
        terminal.backend().buffer().content.iter().any(|cell| {
            matches!(cell.symbol(), "▀" | "▄" | "█" | " ")
                && (matches!(cell.fg, ratatui::style::Color::Rgb(..))
                    || matches!(cell.bg, ratatui::style::Color::Rgb(..)))
        }),
        "ordinary terminal did not receive filled color cells"
    );
    for _ in 0..32 {
        terminal
            .draw(|frame| {
                assert!(viewer.render_ambient_location(frame, frame.area(), 88, compose));
            })
            .unwrap();
    }
    assert_eq!(
        composed.load(Ordering::SeqCst),
        1,
        "cached frames must not compose again"
    );
    assert!(viewer.map_pending.is_none());

    // Hold a different admitted encode. A new destination or size must
    // return false to its current-location braille fallback, never paint
    // the previous room under the new caption while the worker catches up.
    let (_hold, rx) = mpsc::channel();
    viewer.map_pending = Some(PendingPortal {
        key: PortalCacheKey {
            sequence: 99,
            width: 40,
            height: 14,
        },
        rx,
    });
    terminal
        .draw(|frame| {
            assert!(!viewer.render_ambient_location(frame, frame.area(), 100, compose));
        })
        .unwrap();
    assert_eq!(
        composed.load(Ordering::SeqCst),
        1,
        "no obsolete jobs were queued"
    );
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .all(|cell| { cell.symbol() == " " && cell.bg == ratatui::style::Color::Reset }),
        "previous location was painted under a newer key"
    );
}

#[test]
fn room_pixels_distinguish_pending_failed_and_empty_surfaces() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut viewer = Viewer::with_picker(halfblock_picker());
    let hold = viewer.hold_room_worker_for_test();
    let mut terminal = Terminal::new(TestBackend::new(40, 14)).unwrap();
    terminal
        .draw(|frame| {
            assert_eq!(
                viewer.render_world_pixels(frame, frame.area(), 1, true, || (vec![0u8; 1], 16, 16)),
                WorldPixelsState::Pending
            );
            assert_eq!(
                viewer.render_world_pixels::<Vec<u8>, _>(frame, frame.area(), 2, true, || panic!(
                    "coalesce obsolete scene"
                )),
                WorldPixelsState::Pending
            );
            assert_eq!(
                viewer.render_world_pixels::<Vec<u8>, _>(
                    frame,
                    Rect::default(),
                    1,
                    true,
                    || panic!("empty area")
                ),
                WorldPixelsState::Unavailable
            );
        })
        .unwrap();
    drop(hold);
    let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
    while viewer.map_failure.is_none() && std::time::Instant::now() < deadline {
        viewer.promote_pending_map();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(viewer.map_failure.is_some());
    terminal
        .draw(|frame| {
            assert_eq!(
                viewer.render_world_pixels::<Vec<u8>, _>(frame, frame.area(), 1, true, || panic!(
                    "failed frame must not retry"
                )),
                WorldPixelsState::Unavailable
            );
        })
        .unwrap();
}

#[test]
fn protocol_env_values_select_real_inline_backends() {
    assert_eq!(protocol_from_value("kitty"), Some(ProtocolType::Kitty));
    assert_eq!(protocol_from_value("sixel"), Some(ProtocolType::Sixel));
    assert_eq!(
        protocol_from_value("halfblocks"),
        Some(ProtocolType::Halfblocks)
    );
    assert_eq!(protocol_from_value("auto"), None);
}

#[test]
fn trusted_terminal_hints_select_resident_pixel_protocols() {
    assert_eq!(
        protocol_from_terminal_hints(Some("xterm-kitty"), None, false, false, false),
        Some(ProtocolType::Kitty)
    );
    assert_eq!(
        protocol_from_terminal_hints(Some("xterm-256color"), Some("ghostty"), false, false, false,),
        Some(ProtocolType::Kitty)
    );
    assert_eq!(
        protocol_from_terminal_hints(Some("xterm-256color"), None, true, false, false,),
        Some(ProtocolType::Sixel)
    );
    assert_eq!(
        protocol_from_terminal_hints(Some("xterm-256color"), None, false, false, false,),
        None
    );
}

#[test]
fn static_preview_viewer_starts_empty() {
    let viewer = Viewer::static_preview();
    assert!(!viewer.has_image());
    assert_eq!(viewer.label(), None);
    assert!(!viewer.supports_agentviz_portal());
}

#[test]
fn kitty_portal_frame_is_encoded_off_thread_and_cached() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let mut viewer = Viewer::with_picker(picker);
    assert!(viewer.supports_agentviz_portal());
    let portal_frame = crate::ui::viz::agentviz_portal::PortalFrame {
        sequence: 77,
        pixels: Arc::from(vec![32; crate::ui::viz::agentviz_portal::FRAME_BYTES]),
    };
    let mut terminal = Terminal::new(TestBackend::new(40, 10)).expect("test terminal");
    let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
    let mut ready = false;
    while !ready && Instant::now() < deadline {
        terminal
            .draw(|frame| {
                ready = viewer.render_agentviz_portal(frame, frame.area(), &portal_frame);
            })
            .expect("render portal");
        if !ready {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert!(ready, "Kitty portal adapter did not finish");
    assert!(viewer.portal_current.is_some());
    assert!(!viewer.agentviz_portal_pending());
}

#[test]
fn changing_portal_and_map_sequences_coalesce_behind_one_pending_encode() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let mut viewer = Viewer::with_picker(picker);
    let old_key = PortalCacheKey {
        sequence: 1,
        width: 40,
        height: 10,
    };

    let (_portal_hold, portal_rx) = mpsc::channel();
    viewer.portal_pending = Some(PendingPortal {
        key: old_key.clone(),
        rx: portal_rx,
    });
    let portal_frame = crate::ui::viz::agentviz_portal::PortalFrame {
        sequence: 2,
        pixels: Arc::from(vec![32; crate::ui::viz::agentviz_portal::FRAME_BYTES]),
    };
    let mut terminal = Terminal::new(TestBackend::new(40, 10)).expect("test terminal");
    terminal
        .draw(|frame| {
            assert!(!viewer.render_agentviz_portal(frame, frame.area(), &portal_frame));
        })
        .expect("render coalesced portal");
    assert_eq!(
        viewer.portal_pending.as_ref().map(|pending| &pending.key),
        Some(&old_key),
        "a newer portal sequence replaced the admitted encode"
    );

    let (_map_hold, map_rx) = mpsc::channel();
    viewer.map_pending = Some(PendingPortal {
        key: old_key.clone(),
        rx: map_rx,
    });
    let composed = AtomicUsize::new(0);
    terminal
        .draw(|frame| {
            assert!(!viewer.render_native_location(frame, frame.area(), 2, || {
                composed.fetch_add(1, Ordering::Relaxed);
                (vec![0u8; 16 * 8 * 4], 16, 8)
            },));
        })
        .expect("render coalesced map");
    assert_eq!(composed.load(Ordering::Relaxed), 0);
    assert_eq!(
        viewer.map_pending.as_ref().map(|pending| &pending.key),
        Some(&old_key),
        "a newer map sequence queued behind the admitted encode"
    );
}

#[test]
fn kitty_map_composes_off_the_draw_thread_on_one_resident_worker() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    // Byte access is where the lazy compose runs; record which thread
    // takes it so the test pins compose placement, not just encoding.
    struct LazyPixels {
        bytes: Vec<u8>,
        seen: Arc<Mutex<Vec<std::thread::ThreadId>>>,
    }
    impl AsRef<[u8]> for LazyPixels {
        fn as_ref(&self) -> &[u8] {
            self.seen.lock().unwrap().push(std::thread::current().id());
            &self.bytes
        }
    }

    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let mut viewer = Viewer::with_picker(picker);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).expect("test terminal");
    for sequence in [1u64, 2] {
        let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        let mut ready = false;
        while !ready && Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    let seen = Arc::clone(&seen);
                    ready =
                        viewer.render_native_location(frame, frame.area(), sequence, move || {
                            let bytes = vec![9; 16 * 8 * 4];
                            (LazyPixels { bytes, seen }, 16, 8)
                        });
                })
                .expect("render world map");
            if !ready {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(ready, "map frame for sequence {sequence} never landed");
    }
    assert!(viewer.map_current.is_some());
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "each sequence miss composes exactly once");
    let draw_thread = std::thread::current().id();
    assert!(
        seen.iter().all(|id| *id != draw_thread),
        "map compose ran on the draw thread"
    );
    assert_eq!(seen[0], seen[1], "misses did not reuse one worker thread");
}

#[test]
fn portrait_character_protocol_is_anchored_to_the_bottom_right() {
    let picker = Picker::halfblocks();
    let protocol = picker
        .new_protocol(
            image::DynamicImage::ImageRgba8(image::RgbaImage::new(32, 32)),
            Rect::new(0, 0, 20, 12).into(),
            Resize::Fit(None),
        )
        .unwrap();
    let available = Rect::new(7, 9, 40, 25);
    let placed = bottom_right_protocol_area(&protocol, available);
    assert_eq!(placed.right(), available.right());
    assert_eq!(placed.bottom(), available.bottom());
    assert!(
        placed.y > available.y,
        "spare height belongs above the character"
    );
}

#[test]
fn portrait_canvas_moves_alpha_edges_without_rescaling() {
    // The figure: clearly visible alpha. Faint resampling haze around it is
    // not part of the portrait and is dropped by the anchor.
    fn visible(image: &image::RgbaImage) -> image::RgbaImage {
        let (mut left, mut top) = image.dimensions();
        let (mut right, mut bottom) = (0, 0);
        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel[3] >= PORTRAIT_FIGURE_ALPHA {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
        image::imageops::crop_imm(image, left, top, right - left, bottom - top).to_image()
    }
    for font in [(2, 4), (8, 16)] {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(font.into());
        for protocol_type in [ProtocolType::Halfblocks, ProtocolType::Kitty] {
            picker.set_protocol_type(protocol_type);
            for size in [
                Rect::new(0, 0, 20, 12),
                Rect::new(0, 0, 31, 7),
                Rect::new(0, 0, 7, 20),
            ] {
                let mut source = image::RgbaImage::new(80, 40);
                for y in 4..28 {
                    for x in 9..61 {
                        source.put_pixel(x, y, image::Rgba([20, 30, 40, 255]));
                    }
                }
                let source = image::DynamicImage::ImageRgba8(source);
                let before = picker
                    .new_protocol(
                        source.clone(),
                        size.into(),
                        Resize::Fit(Some(image::imageops::FilterType::Lanczos3)),
                    )
                    .unwrap();
                let fitted = if Resize::natural_size(&source, picker.font_size()).width
                    <= size.width
                    && Resize::natural_size(&source, picker.font_size()).height <= size.height
                {
                    source.clone()
                } else {
                    Resize::Fit(Some(image::imageops::FilterType::Lanczos3)).resize(
                        &source,
                        picker.font_size(),
                        before.size(),
                        None,
                    )
                };
                let anchored = anchor_portrait_canvas(source, &picker, size, false).to_rgba8();
                let (mut left, mut top) = anchored.dimensions();
                let (mut right, mut bottom) = (0, 0);
                for (x, y, pixel) in anchored.enumerate_pixels() {
                    if pixel[3] != 0 {
                        left = left.min(x);
                        top = top.min(y);
                        right = right.max(x + 1);
                        bottom = bottom.max(y + 1);
                    }
                }
                assert!(right > left && bottom > top);
                assert_eq!((right, bottom), (anchored.width(), anchored.height()));
                assert_eq!(
                    visible(&fitted.to_rgba8()),
                    visible(&anchored),
                    "portrait pixels changed at font {font:?}, size {size:?}"
                );
                let after = picker
                    .new_protocol(
                        image::DynamicImage::ImageRgba8(anchored),
                        size.into(),
                        Resize::Crop(None),
                    )
                    .expect("portrait protocol encodes without a terminal");
                assert_eq!(before.size(), after.size(), "portrait footprint changed");
            }
        }
    }
}

#[test]
fn atlas_portrait_anchor_preserves_armor_pixels_and_can_export_review() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(crate::ui::helm::sheet(
        crate::ui::agent_panel::profile::AgentKey::Atlas,
    ));
    let original = crate::ui::helm::frame_image(&path, 0).unwrap();
    #[allow(deprecated)]
    let picker = Picker::from_fontsize((10, 21).into());
    let size = Rect::new(0, 0, 18, 9);
    let fit = Resize::Fit(Some(image::imageops::FilterType::Lanczos3));
    let target = fit.size_for(&original, picker.font_size(), size.into());
    let before = fit
        .resize(&original, picker.font_size(), target, None)
        .to_rgba8();
    let after = anchor_portrait_canvas(original, &picker, size, false).to_rgba8();
    // The canvas is the fitted image widened to whole cells.
    let (fw, fh) = (
        u32::from(picker.font_size().width),
        u32::from(picker.font_size().height),
    );
    assert_eq!(
        after.dimensions(),
        (
            before.width().div_ceil(fw) * fw,
            before.height().div_ceil(fh) * fh
        )
    );
    // The figure (clearly visible alpha) is unchanged and sits hard in the
    // lower-right corner: shoulder against the wall, torso on the base.
    let figure = |image: &image::RgbaImage| {
        image
            .pixels()
            .filter(|pixel| pixel[3] >= PORTRAIT_FIGURE_ALPHA)
            .copied()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        figure(&before),
        figure(&after),
        "armor colors, alpha and scale must stay unchanged"
    );
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in after.enumerate_pixels() {
        if pixel[3] >= PORTRAIT_FIGURE_ALPHA {
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    assert_eq!(
        (right, bottom),
        after.dimensions(),
        "anchored to the corner"
    );
    if let Ok(output) = std::env::var("ANGEL_T_PORTRAIT_PREVIEW") {
        let (w, h) = before.dimensions();
        let mut comparison =
            image::RgbaImage::from_pixel(w * 2 + 12, h + 4, image::Rgba([0, 0, 0, 255]));
        for x in [0, w + 1, w + 8, w * 2 + 9] {
            for y in 0..h + 2 {
                comparison.put_pixel(x, y, image::Rgba([30, 100, 125, 255]));
            }
        }
        for y in [0, h + 1] {
            for x in 0..w * 2 + 10 {
                comparison.put_pixel(x, y, image::Rgba([30, 100, 125, 255]));
            }
        }
        image::imageops::overlay(&mut comparison, &before, 1, 1);
        image::imageops::overlay(&mut comparison, &after, i64::from(w + 9), 1);
        comparison.save(output).unwrap();
    }
}

#[test]
fn portrait_preview_renders_bundled_asset_as_halfblocks_and_reuses_cache() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/sparky-neutral.png");
    let mut viewer = Viewer::portrait_preview();
    let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
    for _ in 0..2 {
        terminal
            .draw(|frame| {
                assert!(viewer.render_portrait(frame, frame.area(), &path));
            })
            .expect("render portrait");
    }
    assert_eq!(viewer.portrait_cache.len(), 1);
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| matches!(cell.symbol(), "▀" | "▄" | "█")),
        "half-block portrait should paint terminal-native image cells"
    );
}

#[test]
fn production_portrait_preparation_never_blocks_the_draw_thread() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::{Duration, Instant};

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/codex-active-v2.png");
    let mut viewer = Viewer::with_picker(Picker::halfblocks());
    viewer.portrait_enabled = true;
    let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
    let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
    let mut visible = false;
    let mut draw_samples = Vec::new();
    while Instant::now() < deadline && !visible {
        let started = Instant::now();
        terminal
            .draw(|frame| {
                visible = viewer.render_portrait(frame, frame.area(), &path);
            })
            .expect("render async portrait");
        draw_samples.push(started.elapsed());
        if !visible {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    assert!(visible, "async agent portrait never became visible");
    assert_responsive_draw_samples(&mut draw_samples, "portrait preparation");
}

#[test]
fn portrait_prefetch_makes_the_first_opposite_state_frame_cache_hot() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::{Duration, Instant};

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents");
    let neutral = root.join("sparky-neutral.png");
    let active = root.join("sparky-active.png");
    let mut viewer = Viewer::with_picker(Picker::halfblocks());
    viewer.portrait_enabled = true;
    let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
    let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
    let mut neutral_visible = false;
    while Instant::now() < deadline
        && !viewer
            .portrait_cache
            .iter()
            .any(|entry| entry.key.path == active)
    {
        terminal
            .draw(|frame| {
                neutral_visible = viewer.render_portrait(frame, frame.area(), &neutral);
            })
            .expect("render neutral portrait");
        if neutral_visible {
            viewer.prefetch_portrait(Rect::new(0, 0, 28, 14), &active);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(neutral_visible, "neutral portrait never became visible");
    assert!(
        viewer
            .portrait_cache
            .iter()
            .any(|entry| entry.key.path == active)
    );

    let started = Instant::now();
    terminal
        .draw(|frame| {
            assert!(viewer.render_portrait(frame, frame.area(), &active));
        })
        .expect("render prefetched active portrait");
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "a prefetched active portrait should paint on its first frame"
    );
}

#[test]
fn synchronous_portrait_preview_skips_opposite_state_prefetch() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents");
    let neutral = root.join("sparky-neutral.png");
    let active = root.join("sparky-active.png");
    let mut viewer = Viewer::portrait_preview();
    let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
    terminal
        .draw(|frame| {
            assert!(viewer.render_portrait(frame, frame.area(), &neutral));
            viewer.prefetch_portrait(frame.area(), &active);
        })
        .expect("render static portrait preview");
    assert_eq!(viewer.portrait_cache.len(), 1);
    assert!(viewer.portrait_pending.is_none());
}

#[test]
fn visible_portrait_request_supersedes_a_different_prefetch() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents");
    let speculative = root.join("sparky-active.png");
    let visible = root.join("atlas-neutral.png");
    let area = Rect::new(0, 0, 28, 14);
    let mut viewer = Viewer::with_picker(Picker::halfblocks());
    viewer.portrait_enabled = true;
    viewer.prefetch_portrait(area, &speculative);
    assert!(
        viewer
            .portrait_pending
            .as_ref()
            .is_some_and(|pending| pending.prefetch)
    );

    let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
    terminal
        .draw(|frame| {
            assert!(!viewer.render_portrait(frame, frame.area(), &visible));
        })
        .expect("queue visible portrait");
    let pending = viewer
        .portrait_pending
        .as_ref()
        .expect("visible portrait worker");
    assert_eq!(pending.key.path, visible);
    assert!(!pending.prefetch);
}

#[test]
fn helm_pending_pose_retains_its_agent_and_never_borrows_another_identity() {
    use ratatui::{Terminal, backend::TestBackend};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(crate::ui::helm::sheet(
        crate::ui::agent_panel::profile::AgentKey::Turbo,
    ));
    let other = root.join(crate::ui::helm::sheet(
        crate::ui::agent_panel::profile::AgentKey::Atlas,
    ));
    let mut viewer = Viewer::portrait_preview();
    let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
    terminal
        .draw(|frame| assert!(viewer.render_helm(frame, frame.area(), &path, 0)))
        .unwrap();
    let first = terminal.backend().buffer().clone();
    viewer.portrait_synchronous = false;
    let mut key = Viewer::portrait_key(Rect::new(0, 0, 20, 10), &path);
    key.pose = Some(3);
    let (held, rx) = mpsc::channel();
    viewer.portrait_pending = Some(PendingPortrait {
        key: key.clone(),
        rx,
        prefetch: false,
    });
    for pose in 1..8 {
        terminal
            .draw(|frame| assert!(viewer.render_helm(frame, frame.area(), &path, pose)))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer(),
            &first,
            "no ASCII/empty flash between poses"
        );
        assert_eq!(
            viewer.portrait_pending.as_ref().unwrap().key,
            key,
            "one admitted decode"
        );
    }
    terminal
        .draw(|frame| assert!(!viewer.render_helm(frame, frame.area(), &other, 0)))
        .unwrap();
    assert!(
        viewer
            .portrait_cache
            .iter()
            .all(|entry| entry.key.path != other)
    );
    drop(held);
}

#[test]
fn generated_world_dots_hold_frames_and_reject_old_room_or_geometry_completions() {
    use crate::ui::term::art::{ColoredBrailleCell, ColoredBrailleImage};
    use ratatui::{Terminal, backend::TestBackend};
    let _guard = crate::tests::env_lock();
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize((8, 16).into());
    picker.set_protocol_type(ProtocolType::Kitty);
    let mut viewer = Viewer::with_picker(picker);
    let area = Rect::new(2, 2, 12, 6);
    let geometry = crate::ui::dots::canvas::DotGeometry::new(12, 6, (8, 16), 2).unwrap();
    let image = Arc::new(ColoredBrailleImage {
        width: geometry.grid_width,
        height: geometry.grid_height,
        cells: vec![
            ColoredBrailleCell {
                glyph: '\u{28ff}',
                fg: [80, 140, 160]
            };
            geometry.grid_width * geometry.grid_height
        ],
    });
    let mut terminal = Terminal::new(TestBackend::new(20, 12)).unwrap();
    let hold = viewer.hold_room_worker_for_test();
    terminal
        .draw(|frame| {
            assert!(!viewer.render_world_dots(frame, area, 1, geometry, Arc::clone(&image)))
        })
        .unwrap();
    assert!(viewer.dot_pending.is_some());
    drop(hold);
    let start = std::time::Instant::now();
    while viewer.dot_current.is_none() && start.elapsed() < Duration::from_secs(3) {
        terminal
            .draw(|frame| {
                viewer.render_world_dots(frame, area, 1, geometry, Arc::clone(&image));
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    let first = viewer
        .dot_current
        .as_ref()
        .expect("completed generated canvas")
        .key
        .clone();
    assert_eq!(
        viewer.dot_current.as_ref().unwrap().protocol.size(),
        Rect::new(0, 0, area.width, area.height).into()
    );
    let hold = viewer.hold_room_worker_for_test();
    let mut next = (*image).clone();
    next.cells[0].fg = [170, 90, 40];
    let next = Arc::new(next);
    for _ in 0..20 {
        terminal
            .draw(|frame| {
                assert!(viewer.render_world_dots(frame, area, 1, geometry, Arc::clone(&next)))
            })
            .unwrap();
        assert_eq!(
            viewer.dot_current.as_ref().unwrap().key,
            first,
            "retain exact last completed frame"
        );
    }
    terminal
        .draw(|frame| {
            assert!(!viewer.render_world_dots(frame, area, 2, geometry, Arc::clone(&image)))
        })
        .unwrap();
    assert!(
        viewer.dot_current.is_none(),
        "old room must disappear immediately"
    );
    assert_eq!(
        viewer.dot_pending.as_ref().unwrap().key.scene,
        1,
        "do not spawn obsolete work while pending"
    );
    drop(hold);
    let start = std::time::Instant::now();
    while viewer.dot_current.is_none() && start.elapsed() < Duration::from_secs(3) {
        terminal
            .draw(|frame| {
                viewer.render_world_dots(frame, area, 2, geometry, Arc::clone(&image));
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(viewer.dot_current.as_ref().unwrap().key.scene, 2);
    let smaller = Rect::new(2, 2, 10, 5);
    let resized = crate::ui::dots::canvas::DotGeometry::new(10, 5, (8, 16), 2).unwrap();
    terminal
        .draw(|frame| {
            assert!(!viewer.render_world_dots(frame, smaller, 2, resized, Arc::clone(&image)))
        })
        .unwrap();
    assert!(
        viewer.dot_current.is_none(),
        "old geometry cannot cover adjacent text"
    );
}

#[test]
fn fine_dot_pitch_has_a_portable_text_fallback() {
    assert_eq!(dot_pitch(None), Some(2));
    assert_eq!(dot_pitch(Some("3")), Some(3));
    assert_eq!(dot_pitch(Some("0")), None);
    assert_eq!(dot_pitch(Some("text")), None);
    assert_eq!(dot_pitch(Some("huge")), Some(2));
    assert!(
        Viewer::static_preview()
            .dot_geometry(Rect::new(0, 0, 40, 20))
            .is_none()
    );
}

#[test]
fn missing_portrait_is_negatively_cached_instead_of_retried_each_frame() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/definitely-missing.png");
    let mut viewer = Viewer::portrait_preview();
    let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
    for _ in 0..2 {
        terminal
            .draw(|frame| {
                assert!(!viewer.render_portrait(frame, frame.area(), &path));
            })
            .expect("render missing portrait fallback");
    }
    assert_eq!(viewer.portrait_failures.len(), 1);
    assert!(viewer.portrait_cache.is_empty());
}

#[test]
fn pixel_portraits_fill_the_bay_sharp_and_keep_their_frame() {
    #[allow(deprecated)]
    let picker = Picker::from_fontsize((8, 16).into());
    // A shared-canvas pixel frame: this pose leaves the top-left empty, which
    // another pose of the sheet fills. The empty space is kept, not trimmed.
    let mut frame = image::RgbaImage::new(120, 160);
    for y in 40..160 {
        for x in 30..120 {
            let v = if ((x / 2) + (y / 2)) % 2 == 0 {
                30
            } else {
                210
            };
            frame.put_pixel(x, y, image::Rgba([v, v, v, 255]));
        }
    }
    let visible_bounds = |image: &image::RgbaImage| {
        let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
        for (x, y, p) in image.enumerate_pixels() {
            if p[3] > 0 {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
        (left, top, right, bottom)
    };
    // Opaque pixels within `tolerance` of one of the sprite's two values.
    let hard = |image: &image::RgbaImage, tolerance: u8| {
        let opaque: Vec<_> = image.pixels().filter(|p| p[3] == 255).collect();
        let pure = opaque
            .iter()
            .filter(|p| p[0].abs_diff(30) <= tolerance || p[0].abs_diff(210) <= tolerance)
            .count();
        (pure, opaque.len())
    };

    // A 480x480px bay takes exactly 3x: every pixel stays hard.
    let out = anchor_portrait_canvas(
        image::DynamicImage::ImageRgba8(frame.clone()),
        &picker,
        Rect::new(0, 0, 60, 30),
        true,
    )
    .to_rgba8();
    let (pure, opaque) = hard(&out, 0);
    assert_eq!(pure, opaque, "no blended pixels at a whole multiple");
    assert_eq!(opaque, 270 * 360);
    let (left, top, right, bottom) = visible_bounds(&out);
    assert_eq!((right - left, bottom - top), (270, 360), "exactly 3x");
    assert_eq!(
        (right, bottom),
        (out.width(), out.height()),
        "lower-right anchor"
    );
    assert_eq!((out.width(), out.height()), (360, 480), "whole frame kept");

    // A 400x400px bay is filled at 2.5x, not left at the 2x half step, and
    // stays sharp: a seam is at most a faint blend, never a mid-grey.
    let out = anchor_portrait_canvas(
        image::DynamicImage::ImageRgba8(frame.clone()),
        &picker,
        Rect::new(0, 0, 50, 25),
        true,
    )
    .to_rgba8();
    assert_eq!(out.height(), 400, "fills the bay's height");
    let (_, _, right, bottom) = visible_bounds(&out);
    assert_eq!(
        (right, bottom),
        (out.width(), out.height()),
        "lower-right anchor"
    );
    let (pure, opaque) = hard(&out, 24);
    assert_eq!(pure, opaque, "{pure} of {opaque} hard");

    // A tiny bay shrinks the figure and still fits it.
    let tiny = anchor_portrait_canvas(
        image::DynamicImage::ImageRgba8(frame),
        &picker,
        Rect::new(0, 0, 5, 3),
        true,
    )
    .to_rgba8();
    assert!(
        tiny.width() <= 40 && tiny.height() <= 48,
        "{:?}",
        tiny.dimensions()
    );
    assert!(tiny.pixels().any(|p| p[3] > 0));
}

#[test]
fn a_tmux_client_on_a_kitty_graphics_terminal_gets_pixel_graphics() {
    for name in ["xterm-kitty", "xterm-ghostty", "wezterm"] {
        assert_eq!(
            super::protocol_for_client_termname(name),
            Some(ProtocolType::Kitty),
            "{name}"
        );
    }
    for name in ["screen-256color", "xterm-256color", "alacritty", ""] {
        assert_eq!(super::protocol_for_client_termname(name), None, "{name}");
    }
}
