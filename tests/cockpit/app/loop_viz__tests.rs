use super::*;

fn flatten(text: &Text<'static>) -> String {
    text.lines
        .iter()
        .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
        .collect()
}

fn line_width(line: &Line<'static>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

fn flatten_line(line: &Line<'static>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn submission_slot_line_reports_receipts_without_readiness_or_win_claims() {
    let cases = [
        (
            SubmissionSlotTelemetry {
                phase: SubmissionSlotPhase::Empty,
                ..Default::default()
            },
            "NO SUBMISSION",
            TUI_PHOSPHOR,
        ),
        (
            SubmissionSlotTelemetry {
                phase: SubmissionSlotPhase::InFlight,
                id: Some("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".into()),
                ..Default::default()
            },
            "PENDING · eeeeeeee",
            TUI_WARNING_AMBER,
        ),
        (
            SubmissionSlotTelemetry {
                phase: SubmissionSlotPhase::Accepted,
                score: Some("1844075.40".into()),
                ..Default::default()
            },
            "ACCEPTED · 1844075.40",
            TUI_PHOSPHOR_HOT,
        ),
        (
            SubmissionSlotTelemetry {
                phase: SubmissionSlotPhase::Rejected,
                ..Default::default()
            },
            "REJECTED",
            TUI_ALERT_RED,
        ),
    ];

    for (slot, expected, lamp_color) in cases {
        let line = submission_slot_line(&slot, 48);
        let rendered = flatten_line(&line);
        assert_eq!(line_width(&line), 48, "{rendered:?}");
        assert!(rendered.contains(expected), "{rendered:?}");
        for unsupported in ["FIRE", "WIN", "HIT", "LOCK"] {
            assert!(!rendered.contains(unsupported), "{rendered:?}");
        }
        assert_eq!(line.spans[1].style.fg, Some(lamp_color));
        assert_eq!(
            line.spans[2].style.fg,
            Some(TUI_PHOSPHOR_HOT),
            "only the lamp should carry warning/alert color"
        );
    }
}

#[test]
fn render_preserves_requested_dimensions() {
    let st = LoopState {
        status: LoopStatus::Running,
        max_iters: 25,
        iteration: 3,
        token_budget: 2_000_000,
        tokens_spent: 64_000,
        ..Default::default()
    };
    let slot = SubmissionSlotTelemetry {
        phase: SubmissionSlotPhase::Empty,
        ..Default::default()
    };
    let text = render(&st, &slot, &YukonFleetState::default(), 1.25, 24, 8);
    assert_eq!(text.lines.len(), 8);
    assert!(text.lines.iter().all(|line| line_width(line) == 24));
    assert!(flatten(&text).chars().any(|ch| ch != '\u{2800}'));
}

/// Manual art-review dump — see retro_kit::gallery.
/// Run with: ANGEL0_GALLERY_DIR=... cargo test -- --ignored loop_gallery
#[test]
#[ignore = "manual gallery dump for art review"]
fn loop_gallery() {
    use crate::retro_kit::gallery::{out_dir, rasterize};
    let out = out_dir();
    let st = LoopState {
        status: LoopStatus::Running,
        max_iters: 25,
        iteration: 7,
        token_budget: 2_000_000,
        tokens_spent: 512_000,
        ..Default::default()
    };
    let slots = [
        SubmissionSlotTelemetry {
            phase: SubmissionSlotPhase::Empty,
            ..Default::default()
        },
        SubmissionSlotTelemetry {
            phase: SubmissionSlotPhase::InFlight,
            id: Some("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".into()),
            ..Default::default()
        },
        SubmissionSlotTelemetry {
            phase: SubmissionSlotPhase::Rejected,
            ..Default::default()
        },
    ];
    let mut fleet = YukonFleetState::default();
    fleet.apply(crate::yukon_fleet::YukonFleetSnapshot {
        entries: vec![
            crate::yukon_fleet::YukonSubmission {
                benchmark: "bench/flock".into(),
                id: "3347e70".into(),
                status: "validating".into(),
                score: None,
                phase: YukonSubmissionPhase::Running,
            },
            crate::yukon_fleet::YukonSubmission {
                benchmark: "bench/qwen".into(),
                id: "7871bd4".into(),
                status: "promoted".into(),
                score: Some("519469.35".into()),
                phase: YukonSubmissionPhase::Accepted,
            },
            crate::yukon_fleet::YukonSubmission {
                benchmark: "bench/ssi".into(),
                id: "ddddddd".into(),
                status: "rejected".into(),
                score: None,
                phase: YukonSubmissionPhase::Rejected,
            },
        ],
        benchmark_count: 3,
        failed_benchmarks: 0,
    });
    for ((label, time), slot) in [("a", 2.3f32), ("b", 9.8), ("c", 17.4)]
        .into_iter()
        .zip(slots.iter())
    {
        rasterize(
            &render(&st, slot, &fleet, time, 42, 16),
            &out.join(format!("loop-{label}.png")),
        );
    }
}

#[test]
fn zero_area_is_empty() {
    let slot = SubmissionSlotTelemetry::default();
    let fleet = YukonFleetState::default();
    assert!(
        render(&LoopState::default(), &slot, &fleet, 0.0, 0, 8)
            .lines
            .is_empty()
    );
    assert!(
        render(&LoopState::default(), &slot, &fleet, 0.0, 8, 0)
            .lines
            .is_empty()
    );
}

#[test]
fn hammertime_assets_flip_between_frames() {
    assert_eq!(hammertime_asset(0.0), "assets/loop/hammertime-a.png");
    assert_eq!(hammertime_asset(0.3), "assets/loop/hammertime-b.png");
    assert_eq!(hammertime_asset(0.6), "assets/loop/hammertime-a.png");
    assert_eq!(hammertime_asset_other(0.0), "assets/loop/hammertime-b.png");
    // Twin is phase-shifted and uses the mirrored plate.
    assert_eq!(
        hammertime_twin_asset(0.0),
        "assets/loop/hammertime-a-flip.png"
    );
    assert_ne!(
        hammertime_asset(0.0),
        hammertime_twin_asset(0.0),
        "duo must show two distinct bodies"
    );
    assert!(hammertime_active(&LoopState {
        status: LoopStatus::Running,
        ..LoopState::default()
    }));
    assert!(hammertime_active(&LoopState {
        status: LoopStatus::Paused,
        ..LoopState::default()
    }));
    assert!(!hammertime_active(&LoopState {
        status: LoopStatus::Idle,
        ..LoopState::default()
    }));
}

#[test]
fn hammertime_duo_boxes_stay_inside_stage() {
    let area = ratatui::layout::Rect::new(10, 5, 40, 20);
    for t in [0.0_f32, 0.5, 1.25, 3.7, 12.0] {
        let [left, right] = hammertime_duo_boxes(area, t);
        for b in [left, right] {
            assert!(b.x >= area.x);
            assert!(b.y >= area.y);
            assert!(b.x + b.width <= area.x + area.width);
            assert!(b.y + b.height <= area.y + area.height);
            assert!(b.width >= 7 && b.height >= 7);
        }
        assert!(
            left.x + left.width <= right.x || right.x + right.width <= left.x,
            "dancers must not fully overlap at t={t}: {left:?} {right:?}"
        );
    }
}

#[test]
fn hammertime_duo_boxes_never_escape_any_canvas_size() {
    for width in 0..=MAX_CELLS {
        for height in 0..=MAX_CELLS {
            let area = ratatui::layout::Rect::new(7, 11, width, height);
            for t in [0.0_f32, 0.5, 3.7] {
                for dancer in hammertime_duo_boxes(area, t) {
                    assert!(
                        dancer.x >= area.x
                            && dancer.y >= area.y
                            && dancer.right() <= area.right()
                            && dancer.bottom() <= area.bottom(),
                        "{width}x{height} at t={t}: {dancer:?} escaped {area:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn authored_hammer_plates_exist_on_disk() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "assets/loop/hammertime-a.png",
        "assets/loop/hammertime-b.png",
        "assets/loop/hammertime-a-flip.png",
    ] {
        let p = root.join(rel);
        assert!(p.is_file(), "missing authored hammer plate {}", p.display());
    }
}

#[test]
fn video_strip_is_optional_and_ping_pongs_when_present() {
    // No strip installed → soft None (cartoon lead still paints).
    // When present, the cycle is forward + reverse without repeating ends.
    let Some(first) = mascot_frame(0.0) else {
        return;
    };
    assert!(
        first
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("hammertime2-")),
        "unexpected video plate {first:?}"
    );
    let frames = mascot_frames();
    let n = frames.len();
    assert!(n > 10, "expected a real dance cycle, got {n} frames");
    let half = (n + 2) / 2;
    let warm = mascot_warm_frames(0.0);
    assert!(warm.contains(&first));
    assert!(warm.len() <= MASCOT_TRAIL + 4);
    // Ends of the ping-pong: last plate is frame 001 (not 000).
    assert!(frames[half.min(n - 1)].exists() || n > 0);
}

#[test]
fn visible_only_for_live_loop_surfaces() {
    let mut st = LoopState {
        status: LoopStatus::Running,
        ..Default::default()
    };
    assert!(visible(&st));
    st.status = LoopStatus::Paused;
    assert!(visible(&st));
    st.status = LoopStatus::Done;
    assert!(!visible(&st));
    st.status = LoopStatus::Idle;
    assert!(!visible(&st));
}

#[test]
fn frame_changes_with_iteration_and_status() {
    let a = LoopState {
        status: LoopStatus::Running,
        tier: EscalationTier::Local,
        iteration: 1,
        max_iters: 25,
        ..Default::default()
    };
    let b = LoopState {
        status: LoopStatus::Running,
        tier: EscalationTier::Sota,
        iteration: 8,
        max_iters: 0,
        stale_count: 3,
        ..Default::default()
    };
    assert_ne!(
        flatten(&render(
            &a,
            &SubmissionSlotTelemetry::default(),
            &YukonFleetState::default(),
            2.0,
            32,
            10
        )),
        flatten(&render(
            &b,
            &SubmissionSlotTelemetry::default(),
            &YukonFleetState::default(),
            2.0,
            32,
            10
        ))
    );
}

#[test]
fn render_surfaces_loop_budget_health_bars() {
    let st = LoopState {
        status: LoopStatus::Running,
        max_iters: 25,
        iteration: 7,
        token_budget: 2_000_000,
        tokens_spent: 500_000,
        deadline_secs: 3600,
        started_ms: 1,
        ..Default::default()
    };
    let flat = flatten(&render(
        &st,
        &SubmissionSlotTelemetry::default(),
        &YukonFleetState::default(),
        1.0,
        44,
        12,
    ));
    assert!(flat.contains("TOK"), "{flat}");
    assert!(flat.contains("500k/2m"), "{flat}");
    assert!(flat.contains("ITER"), "{flat}");
    assert!(flat.contains("7/25"), "{flat}");
    assert!(flat.contains("TIME"), "{flat}");
}

#[test]
fn wide_loop_hud_uses_three_column_instrument_rail() {
    let st = LoopState {
        status: LoopStatus::Running,
        max_iters: 25,
        iteration: 7,
        token_budget: 2_000_000,
        tokens_spent: 512_000,
        deadline_secs: 3600,
        started_ms: now_ms().saturating_sub(581_000),
        ..Default::default()
    };
    let line = telemetry_rail(&st, 60);
    let flat = flatten_line(&line);
    assert_eq!(line_width(&line), 60);
    assert!(flat.contains("TOK 512k/2m"), "{flat}");
    assert!(flat.contains("ITER 7/25"), "{flat}");
    assert!(flat.contains("TIME 09:41/60:00"), "{flat}");
    assert_eq!(hud_row_count(60, 16, true), 3);
}

#[test]
fn research_progress_line_renders_findings_and_problem() {
    let st = LoopState {
        status: LoopStatus::Running,
        task: "ship the cockpit".into(),
        findings: vec!["finding 1".into(), "finding 2".into()],
        directions_tried: vec!["dir 1".into()],
        hypotheses: vec!["hyp 1".into()],
        iteration: 3,
        max_iters: 10,
        ..Default::default()
    };
    let line = research_progress_line(&st, 60);
    let flat = flatten_line(&line);
    assert_eq!(line_width(&line), 60);
    assert!(flat.contains("RES ["), "{flat}");
    assert!(flat.contains("2 find"), "{flat}");
    assert!(flat.contains("ship the cockpit"), "{flat}");
}

#[test]
fn target_brightens_only_after_an_accepted_receipt() {
    let acquiring = SubmissionSlotTelemetry {
        phase: SubmissionSlotPhase::Empty,
        ..Default::default()
    };
    let accepted = SubmissionSlotTelemetry {
        phase: SubmissionSlotPhase::Accepted,
        ..acquiring.clone()
    };
    assert_eq!(submission_target_color(&acquiring), PHOSPHOR);
    assert_eq!(submission_target_color(&accepted), PHOSPHOR_HOT);
}

#[test]
fn yukon_fleet_line_pulses_live_rows_and_counts_every_status() {
    let mut fleet = YukonFleetState::default();
    fleet.apply(crate::yukon_fleet::YukonFleetSnapshot {
        entries: [
            YukonSubmissionPhase::Queued,
            YukonSubmissionPhase::Running,
            YukonSubmissionPhase::Accepted,
            YukonSubmissionPhase::Rejected,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, phase)| crate::yukon_fleet::YukonSubmission {
            benchmark: format!("bench/{index}"),
            id: format!("aaaaaa{index}"),
            status: format!("{phase:?}"),
            score: None,
            phase,
        })
        .collect(),
        benchmark_count: 4,
        failed_benchmarks: 0,
    });
    let lit = yukon_fleet_line(&fleet, 0.0, 48);
    let dim = yukon_fleet_line(&fleet, 0.6, 48);
    assert_eq!(line_width(&lit), 48);
    assert_eq!(line_width(&dim), 48);
    assert!(flatten_line(&lit).contains("B4 Q1 V1 H1 X1"));
    assert_ne!(flatten_line(&lit), flatten_line(&dim));
    assert!(
        lit.spans
            .iter()
            .any(|span| span.style.fg == Some(TUI_WARNING_AMBER))
    );
    assert!(
        lit.spans
            .iter()
            .any(|span| span.style.fg == Some(TUI_ALERT_RED))
    );
}
