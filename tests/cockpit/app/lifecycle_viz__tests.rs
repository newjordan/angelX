use super::*;

#[test]
fn custom_dim_ink_survives_braille_cell_voting() {
    let mut canvas = DotCanvas::new(1, 1);
    let dim_red = [48, 15, 13];
    canvas.set(0, 0, dim_red);
    let lines = canvas.into_lines();
    assert_eq!(lines[0].spans[0].style.fg, Some(rgb(dim_red)));
}

#[test]
fn black_sprite_ink_occludes_backdrop_but_transparency_does_not() {
    let mut canvas = DotCanvas::new(1, 1);
    canvas.dots.fill(Some(CYAN));
    let mut sprite = image::RgbaImage::new(2, 4);
    sprite.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
    canvas.erase_silhouette(&sprite, 0, 0, 1, 1, false);
    assert_eq!(canvas.dots[0], None);
    assert!(canvas.dots[1..].iter().all(|color| *color == Some(CYAN)));
}

#[test]
#[ignore = "manual art review: paired tournament cast across phases"]
fn dump_tourney_cast_for_review() {
    let out = std::env::var("TOURNEY_DUMP_DIR").expect("TOURNEY_DUMP_DIR");
    std::fs::create_dir_all(&out).unwrap();
    for kind in [
        CeremonyKind::LoopEscalate,
        CeremonyKind::LoopDone,
        CeremonyKind::LoopFailed,
    ] {
        for (index, elapsed) in [0.4, 0.9, 1.4, 1.9, 2.4, 2.9, 3.3].into_iter().enumerate() {
            for (width, height) in [(48, 12), (72, 16)] {
                let text = render_with_asset_root(
                    kind,
                    "cast review",
                    elapsed,
                    width,
                    height,
                    MotionMode::Full,
                    &default_asset_root(),
                );
                crate::ui::retro_kit::gallery::rasterize(
                    &text,
                    &std::path::PathBuf::from(format!(
                        "{out}/{kind:?}-{index}-{width}x{height}.png"
                    )),
                );
            }
        }
    }
}

const ALL_KINDS: [CeremonyKind; 9] = [
    CeremonyKind::GoalSet,
    CeremonyKind::GoalDone,
    CeremonyKind::GoalCleared,
    CeremonyKind::LoopStart,
    CeremonyKind::LoopPaused,
    CeremonyKind::LoopDone,
    CeremonyKind::LoopStopped,
    CeremonyKind::LoopFailed,
    CeremonyKind::LoopEscalate,
];

fn signature(text: &Text<'static>) -> String {
    let mut out = String::new();
    for line in &text.lines {
        for span in &line.spans {
            out.push_str(span.content.as_ref());
            out.push_str(&format!("{:?}", span.style.fg));
        }
        out.push('\n');
    }
    out
}

#[test]
fn deterministic_keyframes_cover_every_lifecycle_kind() {
    let keyframes = [0.0, 0.9, 1.75, 2.55, 3.35];
    for kind in ALL_KINDS {
        let frames = keyframes
            .into_iter()
            .map(|elapsed| render(kind, "ship the thing", elapsed, 72, 16, MotionMode::Full))
            .collect::<Vec<_>>();
        for (elapsed, frame) in keyframes.into_iter().zip(&frames) {
            assert_eq!(frame.lines.len(), 16, "{kind:?} at {elapsed}");
            assert_eq!(
                signature(frame),
                signature(&render(
                    kind,
                    "ship the thing",
                    elapsed,
                    72,
                    16,
                    MotionMode::Full
                )),
                "identical input must match for {kind:?} at {elapsed}"
            );
        }
        let distinct = frames
            .iter()
            .map(|frame| {
                let mut art = frame.clone();
                art.lines.truncate(16 - STATUS_ROWS);
                signature(&art)
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            distinct.len() >= 2,
            "actual art must animate for {kind:?}; changing a frame counter is insufficient"
        );
    }
}

#[test]
fn ceremony_handles_zero_tiny_compact_standard_and_large_bounds() {
    assert!(
        render(CeremonyKind::GoalSet, "x", 0.0, 0, 0, MotionMode::Full)
            .lines
            .is_empty()
    );
    for (width, height) in [(1, 1), (18, 6), (72, 16), (96, 30), (144, 48)] {
        let text = render(
            CeremonyKind::LoopDone,
            "x",
            2.6,
            width,
            height,
            MotionMode::Full,
        );
        assert_eq!(text.lines.len(), height as usize);
        for line in &text.lines {
            let columns = line
                .spans
                .iter()
                .map(|span| span.content.chars().count())
                .sum::<usize>();
            assert_eq!(columns, width as usize, "{width}x{height}");
        }
    }
}

#[test]
fn status_rows_never_overlap_art() {
    let text = render(
        CeremonyKind::LoopFailed,
        "broken target",
        2.4,
        72,
        16,
        MotionMode::Full,
    );
    let art = &text.lines[..14];
    let status = &text.lines[14..];
    assert!(art.iter().any(|line| line.spans.iter().any(|span| {
        span.content
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
    })));
    assert!(status.iter().all(|line| line.spans.iter().all(|span| {
        !span
            .content
            .chars()
            .any(|ch| ('\u{2800}'..='\u{28ff}').contains(&ch))
    })));
}

#[test]
fn reduced_and_off_motion_are_restrained_and_static() {
    let reduced_a = render(
        CeremonyKind::LoopDone,
        "x",
        0.1,
        72,
        16,
        MotionMode::Reduced,
    );
    let reduced_b = render(
        CeremonyKind::LoopDone,
        "x",
        1.2,
        72,
        16,
        MotionMode::Reduced,
    );
    assert_ne!(signature(&reduced_a), signature(&reduced_b));

    let off_a = render(CeremonyKind::LoopDone, "x", 0.0, 72, 16, MotionMode::Off);
    let off_b = render(CeremonyKind::LoopDone, "x", 3.5, 72, 16, MotionMode::Off);
    assert_eq!(signature(&off_a), signature(&off_b));
    assert!(!MotionMode::Off.animates());
}

#[test]
fn missing_and_corrupt_assets_use_visible_fallbacks() {
    let root = std::env::temp_dir().join(format!("angel-tourney-assets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("poses")).unwrap();
    std::fs::write(root.join("arena-plate.png"), b"not an image").unwrap();
    std::fs::write(root.join("poses/canter.png"), b"also corrupt").unwrap();
    let text = render_with_asset_root(
        CeremonyKind::LoopEscalate,
        "fallback",
        1.2,
        72,
        16,
        MotionMode::Full,
        &root,
    );
    let _ = std::fs::remove_dir_all(root);
    assert!(
        text.lines[..14]
            .iter()
            .any(|line| line.spans.iter().any(|span| {
                span.content
                    .chars()
                    .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
            }))
    );
}

#[test]
fn supporting_sequences_never_replace_the_cast_or_award_a_failed_turn() {
    for frame in 0..FRAME_COUNT {
        let t = Timeline::at(frame as f32 / FPS, MotionMode::Full);
        if let Some(cue) = sequence_cue(CeremonyKind::LoopStart, t) {
            assert_eq!(cue.sequence, Sequence::FlagRaise);
            assert!(cue.frame < cue.sequence.frame_count());
        }
        assert!(sequence_cue(CeremonyKind::LoopFailed, t).is_none());
    }
    let victory = Timeline::at(2.5, MotionMode::Full);
    assert_eq!(
        sequence_cue(CeremonyKind::LoopDone, victory)
            .unwrap()
            .sequence,
        Sequence::MaidenWave
    );
}

#[test]
fn journey_captions_distinguish_completion_stop_and_preview() {
    let completed = render(
        CeremonyKind::GoalDone,
        "verified work",
        1.0,
        72,
        16,
        MotionMode::Full,
    )
    .to_string();
    let stopped = render(
        CeremonyKind::LoopStopped,
        "service",
        1.0,
        72,
        16,
        MotionMode::Full,
    )
    .to_string();
    let preview = render(
        CeremonyKind::GoalDone,
        "calibration · service",
        1.0,
        72,
        16,
        MotionMode::Full,
    )
    .to_string();
    assert!(completed.contains("Goal completed") && completed.contains("verified work"));
    assert!(stopped.contains("Loop stopped") && !stopped.contains("completed"));
    assert!(preview.contains("Preview only") && !preview.contains("completed"));
    for text in [completed, stopped, preview] {
        assert!(!text.contains("lance") && !text.contains("frame "));
    }
}

#[test]
fn library_sequences_wire_into_mapped_ceremonies() {
    let root = default_asset_root();
    if !root.join("library/sequences").is_dir() {
        return; // library intake not present in this checkout
    }
    // Twin roots isolate one feature each. `no_library` lacks the whole
    // library, so backdrop rows assert against it; `plates_only` carries
    // the location plates but no sequences or character stills, so on its
    // rows an identical backdrop can never mask a sequence regression.
    let scratch = std::env::temp_dir().join(format!("angel-tourney-twins-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let no_library = scratch.join("no-library");
    let plates_only = scratch.join("plates-only");
    for twin in [&no_library, &plates_only] {
        std::fs::create_dir_all(twin).unwrap();
        let _ = std::os::unix::fs::symlink(root.join("poses"), twin.join("poses"));
        let _ =
            std::os::unix::fs::symlink(root.join("arena-plate.png"), twin.join("arena-plate.png"));
    }
    std::fs::create_dir_all(plates_only.join("library")).unwrap();
    let _ = std::os::unix::fs::symlink(
        root.join("library/plates"),
        plates_only.join("library/plates"),
    );
    for (kind, elapsed, twin) in [
        (CeremonyKind::LoopStart, 0.2, &plates_only), // herald fanfare
        (CeremonyKind::LoopStart, 0.9, &plates_only), // flag_raise cut-in
        (CeremonyKind::LoopEscalate, 0.9, &plates_only), // favor, shared cast stays visible
        (CeremonyKind::LoopEscalate, 1.75, &plates_only), // authored impact FX
        (CeremonyKind::LoopFailed, 2.55, &plates_only), // authored aftermath FX
        (CeremonyKind::LoopDone, 2.55, &plates_only), // maiden_wave cut-in
        (CeremonyKind::LoopStopped, 1.75, &no_library), // lists_dusk backdrop
    ] {
        let wired = render_with_asset_root(kind, "seq", elapsed, 72, 16, MotionMode::Full, &root);
        let plain = render_with_asset_root(kind, "seq", elapsed, 72, 16, MotionMode::Full, twin);
        assert_ne!(
            signature(&wired),
            signature(&plain),
            "{kind:?} at {elapsed}s must stage its library asset"
        );
    }
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn knight_journey_owns_real_lifecycle_and_jousting_preview_stays_fallback() {
    let real = render(
        CeremonyKind::LoopDone,
        "verified loop",
        2.6,
        72,
        16,
        MotionMode::Full,
    );
    let preview = render(
        CeremonyKind::LoopDone,
        "calibration · win",
        2.6,
        72,
        16,
        MotionMode::Full,
    );
    let stopped = render(
        CeremonyKind::LoopStopped,
        "verified loop",
        2.6,
        72,
        16,
        MotionMode::Full,
    );
    assert_ne!(
        signature(&real),
        signature(&preview),
        "jousting preview must remain distinct from verified completion art"
    );
    assert_ne!(
        signature(&real),
        signature(&stopped),
        "stopped work must not reuse the success sheet"
    );
    assert!(
        real.lines.iter().any(|line| line
            .spans
            .iter()
            .any(|span| span.content.contains("verified loop"))),
        "status rows still carry the real target"
    );
    assert!(
        preview.lines.iter().any(|line| line
            .spans
            .iter()
            .any(|span| span.content.contains("calibration"))),
        "preview status must stay labeled as calibration"
    );
}

#[test]
fn motion_env_parser_defaults_to_full() {
    assert_eq!(MotionMode::parse("full"), MotionMode::Full);
    assert_eq!(MotionMode::parse("reduced"), MotionMode::Reduced);
    assert_eq!(MotionMode::parse("off"), MotionMode::Off);
    assert_eq!(MotionMode::parse("surprise"), MotionMode::Full);
}

#[test]
fn warmed_large_frame_stays_under_cockpit_draw_budget() {
    warm_assets();
    // A single wall-clock sample inside the parallel test suite measures
    // scheduler preemption as if it were render work. Several warmed samples
    // preserve the strict uncontended draw budget while making that external
    // noise visible instead of flaky.
    let mut fastest = std::time::Duration::MAX;
    let mut frame = None;
    for _ in 0..5 {
        let started = std::time::Instant::now();
        frame = Some(render(
            CeremonyKind::LoopDone,
            "performance",
            2.6,
            144,
            48,
            MotionMode::Full,
        ));
        fastest = fastest.min(started.elapsed());
    }
    let frame = frame.unwrap();
    assert_eq!(frame.lines.len(), 48);
    assert!(
        fastest < std::time::Duration::from_millis(50),
        "fastest of five warmed tourney frames took {fastest:?}"
    );
}
