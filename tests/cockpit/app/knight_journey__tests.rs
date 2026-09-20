use super::*;

#[test]
fn manifest_playback_is_the_source_of_truth() {
    let manifest = load_manifest(&default_root()).expect("knight-journey manifest");
    assert_eq!(manifest.scenes.len(), 6);
    let study = manifest.scene(Scene::Study).expect("study");
    assert_eq!(study.sequence, vec![0, 1, 2, 3]);
    assert_eq!(study.frame_ms, 420);
    assert_eq!(study.frames.len(), 4);
    let service = manifest.scene(Scene::Service).expect("service");
    assert_eq!(service.sequence, vec![0, 1, 2, 2, 1]);
    assert_eq!(
        service.frames[0],
        ImageRegion {
            x: 160,
            y: 120,
            width: 420,
            height: 200
        }
    );
}

#[test]
fn verified_success_is_the_only_path_to_service_or_guardian() {
    assert_eq!(
        scene_for(CeremonyKind::GoalDone, "ship it"),
        Some(Scene::Service)
    );
    assert_eq!(
        scene_for(CeremonyKind::LoopDone, "ship it"),
        Some(Scene::Guardian)
    );
    for kind in [
        CeremonyKind::GoalSet,
        CeremonyKind::GoalCleared,
        CeremonyKind::LoopStart,
        CeremonyKind::LoopPaused,
        CeremonyKind::LoopStopped,
        CeremonyKind::LoopFailed,
        CeremonyKind::LoopEscalate,
    ] {
        let scene = scene_for(kind, "ship it").expect("mapped");
        assert!(
            !scene.is_success(),
            "{kind:?} must not celebrate as success (got {scene:?})"
        );
    }
    assert_eq!(
        scene_for(CeremonyKind::LoopStopped, "stopped task"),
        Some(Scene::Perseverance)
    );
    assert_eq!(
        scene_for(
            CeremonyKind::GoalDone,
            "calibration · service · not an achieved outcome"
        ),
        Some(Scene::Service)
    );
    assert_eq!(
        scene_for(
            CeremonyKind::LoopDone,
            "calibration · guardian · not an achieved outcome"
        ),
        Some(Scene::Guardian)
    );
}

#[test]
fn adversarial_labels_cannot_unlock_success_art() {
    let hostile = [
        "calibration · guardian",
        "calibration · service · not an achieved outcome",
        "calibration · guardian · not an achieved outcome",
        "calibration · service",
        "guardian",
        "service",
        "not an achieved outcome · guardian",
        "won via service",
    ];
    let kinds = [
        CeremonyKind::GoalSet,
        CeremonyKind::GoalCleared,
        CeremonyKind::LoopStart,
        CeremonyKind::LoopPaused,
        CeremonyKind::LoopStopped,
        CeremonyKind::LoopFailed,
        CeremonyKind::LoopEscalate,
    ];
    for kind in kinds {
        for label in hostile {
            let scene = scene_for(kind, label).expect("mapped");
            assert!(
                !scene.is_success(),
                "{kind:?} with label {label:?} must not show success art (got {scene:?})"
            );
        }
    }
}

#[test]
fn jousting_preview_names_decline_this_path() {
    assert_eq!(scene_for(CeremonyKind::LoopDone, "calibration · win"), None);
    assert_eq!(scene_for(CeremonyKind::LoopStart, "start"), None);
    assert_eq!(scene_for(CeremonyKind::LoopEscalate, "joust"), None);
    assert!(is_journey_calibration_name("study"));
    assert!(!is_journey_calibration_name("win"));
    assert_eq!(
        scene_for(CeremonyKind::LoopStart, "calibration · study"),
        Some(Scene::Study)
    );
}

#[test]
fn motion_off_is_stable_and_reduced_is_restrained() {
    let spec = manifest()
        .and_then(|manifest| manifest.scene(Scene::Study))
        .expect("study spec");
    assert_eq!(
        playback_frame(0.0, MotionMode::Off, spec.frame_ms, spec.sequence.len()),
        playback_frame(3.5, MotionMode::Off, spec.frame_ms, spec.sequence.len())
    );
    let reduced_entry =
        playback_frame(0.1, MotionMode::Reduced, spec.frame_ms, spec.sequence.len());
    let reduced_hold = playback_frame(1.2, MotionMode::Reduced, spec.frame_ms, spec.sequence.len());
    assert_ne!(reduced_entry, reduced_hold);
    let full_a = playback_frame(0.0, MotionMode::Full, spec.frame_ms, spec.sequence.len());
    let full_b = playback_frame(0.9, MotionMode::Full, spec.frame_ms, spec.sequence.len());
    assert_ne!(full_a, full_b);
}

#[test]
fn selected_frames_fit_with_margins_and_equal_pitch() {
    let (cell_w, cell_h, x, y) = fit_cell_box(420, 200, 72, 14);
    assert!(x >= MARGIN_CELLS && y >= MARGIN_CELLS);
    assert!(x + cell_w <= 72 - MARGIN_CELLS);
    assert!(y + cell_h <= 14 - MARGIN_CELLS);
    let pitch_x = cell_w as f32 * 2.0 / 420.0;
    let pitch_y = cell_h as f32 * 4.0 / 200.0;
    assert!(
        (pitch_x - pitch_y).abs() / pitch_x.max(pitch_y) < 0.08,
        "unequal pitch {pitch_x} vs {pitch_y} for {cell_w}x{cell_h}"
    );

    let (large_w, large_h, large_x, large_y) = fit_cell_box(420, 200, 220, 160);
    assert!(large_x >= MARGIN_CELLS && large_y >= MARGIN_CELLS);
    assert!(large_x + large_w <= 220 - MARGIN_CELLS);
    assert!(large_y + large_h <= 160 - MARGIN_CELLS);
    let large_px = large_w as f32 * 2.0 / 420.0;
    let large_py = large_h as f32 * 4.0 / 200.0;
    assert!(
        (large_px - large_py).abs() / large_px.max(large_py) < 0.08,
        "220x160 unequal pitch {large_px} vs {large_py}"
    );

    let (narrow_w, narrow_h, _, _) = fit_cell_box(420, 200, 12, 40);
    assert!(narrow_w <= 12 && narrow_h <= 40);
    let narrow_px = narrow_w as f32 * 2.0 / 420.0;
    let narrow_py = narrow_h as f32 * 4.0 / 200.0;
    assert!(
        (narrow_px - narrow_py).abs() / narrow_px.max(narrow_py) < 0.12,
        "narrow pane unequal pitch {narrow_px} vs {narrow_py}"
    );
}

#[test]
fn decode_cache_is_bounded_and_sheets_convert_to_braille() {
    warm();
    let cache = sheet_cache().lock().expect("sheet cache");
    assert!(!cache.is_empty());
    assert!(cache.len() <= SHEET_CACHE_LIMIT);
    drop(cache);

    let art = try_art(
        CeremonyKind::LoopStart,
        "read the problem",
        0.4,
        72,
        14,
        MotionMode::Full,
    )
    .expect("study art");
    assert_eq!(art.len(), 14);
    assert!(art.iter().all(|line| line.spans.len() == 72));
    let lit = art.iter().any(|line| {
        line.spans.iter().any(|span| {
            span.content
                .chars()
                .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
        })
    });
    assert!(lit, "fitted frame must light braille dots");
    let gutter = art.iter().any(|line| {
        line.spans
            .iter()
            .any(|span| span.content.chars().any(|ch| ch == ' '))
    });
    assert!(!gutter, "no cell gutters");

    let large = try_art(
        CeremonyKind::LoopDone,
        "verified loop",
        2.6,
        220,
        160,
        MotionMode::Full,
    )
    .expect("large guardian");
    assert_eq!(large.len(), 160);
    assert!(large.iter().all(|line| line.spans.len() == 220));

    let narrow = try_art(
        CeremonyKind::LoopStart,
        "read the problem",
        0.4,
        12,
        40,
        MotionMode::Full,
    )
    .expect("narrow study");
    assert_eq!(narrow.len(), 40);
    assert!(narrow.iter().all(|line| line.spans.len() == 12));
}

#[test]
fn success_art_differs_from_failure_and_calibration_win_stays_off_this_path() {
    let done = try_art(
        CeremonyKind::LoopDone,
        "verified loop",
        2.6,
        72,
        14,
        MotionMode::Full,
    )
    .expect("guardian");
    let failed = try_art(
        CeremonyKind::LoopFailed,
        "verified loop",
        2.6,
        72,
        14,
        MotionMode::Full,
    )
    .expect("dragon");
    assert_ne!(format!("{done:?}"), format!("{failed:?}"));
    assert!(
        try_art(
            CeremonyKind::LoopDone,
            "calibration · win",
            2.6,
            72,
            14,
            MotionMode::Full
        )
        .is_none()
    );
}
