use super::*;

fn base_input(area: Rect) -> CockpitLayoutInput {
    CockpitLayoutInput {
        area,
        agent_active: true,
        artifacts_active: true,
        reasoning_visible: false,
        side_width: 40,
        idle_agent_height_cap: 12,
        backdrop: BackdropMode::InProcess,
        join: SurfaceJoin::SharedBorder,
    }
}

#[test]
fn backdrop_off_gives_transcript_full_body() {
    let area = Rect::new(0, 0, 160, 40);
    let mut input = base_input(area);
    input.backdrop = BackdropMode::Off;
    let plan = plan_cockpit_surfaces(input);
    assert_eq!(plan.surfaces.len(), 1);
    assert_eq!(plan.surfaces[0].kind, PanelKind::Transcript);
    assert_eq!(plan.surfaces[0].frame, area);
    assert!(plan.surfaces[0].accepts_clipboard_selection());
}

/// A2: Off is lean — no side column even when agent/artifacts claim activity.
#[test]
fn backdrop_off_ignores_side_column_activity_flags() {
    let area = Rect::new(0, 0, 200, 50);
    let mut input = base_input(area);
    input.backdrop = BackdropMode::Off;
    input.agent_active = true;
    input.artifacts_active = true;
    input.reasoning_visible = true;
    input.side_width = 80;
    let plan = plan_cockpit_surfaces(input);
    assert!(plan.agent_frame().is_none());
    assert!(plan.artifacts_frame().is_none());
    assert_eq!(plan.transcript_frame(), Some(area));
    assert!(!plan.backdrop_mode.paints_in_process());
    assert!(!plan.backdrop_mode.shows_side_column());
}

/// A2: Lazy still hosts the side column in-process until external paint lands.
#[test]
fn backdrop_lazy_still_splits_like_in_process() {
    let area = Rect::new(0, 0, 160, 40);
    let mut input = base_input(area);
    input.backdrop = BackdropMode::Lazy;
    let plan = plan_cockpit_surfaces(input);
    assert!(plan.backdrop_mode.paints_in_process());
    assert!(plan.agent_frame().is_some());
    assert!(plan.artifacts_frame().is_some());
}

#[test]
fn backdrop_mode_from_env_parses_off_synonyms() {
    let _lock = crate::tests::env_lock();
    for v in ["off", "0", "false", "no", "text", "OFF", " Off "] {
        let _g = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", v);
        assert_eq!(
            BackdropMode::from_env(),
            BackdropMode::Off,
            "ANGEL_BACKDROP={v:?}"
        );
        assert!(!BackdropMode::from_env().paints_in_process());
    }
    let _lazy = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "lazy");
    assert_eq!(BackdropMode::from_env(), BackdropMode::Lazy);
    let _on = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in_process");
    assert_eq!(BackdropMode::from_env(), BackdropMode::InProcess);
    drop(_on);
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_BACKDROP");
    assert_eq!(BackdropMode::from_env(), BackdropMode::InProcess);
}

#[test]
fn in_process_splits_transcript_and_backdrops() {
    let area = Rect::new(0, 0, 160, 40);
    let plan = plan_cockpit_surfaces(base_input(area));
    assert!(plan.transcript_frame().is_some());
    assert!(plan.agent_frame().is_some());
    assert!(plan.artifacts_frame().is_some());
    assert!(
        plan.surfaces
            .iter()
            .filter(|s| s.role == SurfaceRole::Backdrop)
            .all(|s| s.copy_rect.is_none())
    );
    let copy = plan.transcript_copy_rect().expect("prose rect");
    // Prose is strictly inside the transcript frame (no side-column cells).
    let tf = plan.transcript_frame().unwrap();
    assert!(copy.x >= tf.x);
    assert!(copy.right() <= tf.right());
}

#[test]
fn live_reasoning_uses_eighty_percent_height_idle_stays_capped() {
    let area = Rect::new(0, 0, 160, 40);
    let idle = plan_cockpit_surfaces(base_input(area));
    let mut live_in = base_input(area);
    live_in.reasoning_visible = true;
    let live = plan_cockpit_surfaces(live_in);
    let idle_agent = idle.agent_frame().expect("idle bay");
    let live_agent = live.agent_frame().expect("live bay");
    let idle_scry = idle.artifacts_frame().expect("idle scryglass");
    let live_scry = live.artifacts_frame().expect("live scryglass");
    assert_eq!(
        idle.transcript_frame().map(|r| r.width),
        live.transcript_frame().map(|r| r.width),
        "transcript width is independent of reasoning height"
    );
    assert!(
        idle_agent.height <= 12,
        "idle bay stays at the cap: {idle_agent:?}"
    );
    assert!(
        idle_scry.height > idle_agent.height,
        "idle Scryglass keeps the column: {idle_agent:?} {idle_scry:?}"
    );
    let expected_live = reasoning_agent_height(area.height)
        .max(3)
        .min(area.height.saturating_sub(3).max(3));
    assert_eq!(
        live_agent.height, expected_live,
        "live reasoning uses thinking height: {live_agent:?}"
    );
    assert!(
        live_agent.height > idle_agent.height,
        "streaming height must exceed idle cap: live={live_agent:?} idle={idle_agent:?}"
    );
    assert!(
        live_agent.height > live_scry.height,
        "live thought owns the column: {live_agent:?} {live_scry:?}"
    );
}

#[test]
fn gap_join_does_not_overlap_columns() {
    let area = Rect::new(0, 0, 160, 40);
    let mut input = base_input(area);
    input.join = SurfaceJoin::Gap;
    let plan = plan_cockpit_surfaces(input);
    let left = plan.transcript_frame().unwrap();
    let agent = plan.agent_frame().unwrap();
    // Gap mode: transcript right edge strictly left of agent column.
    assert!(
        left.right() <= agent.x,
        "gap layout must not share cells: left.right={} agent.x={}",
        left.right(),
        agent.x
    );
}

#[test]
fn narrow_terminal_is_transcript_only() {
    let area = Rect::new(0, 0, 80, 30);
    let plan = plan_cockpit_surfaces(base_input(area));
    assert_eq!(plan.surfaces.len(), 1);
    assert_eq!(plan.surfaces[0].kind, PanelKind::Transcript);
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.width > 0
        && b.width > 0
        && a.height > 0
        && b.height > 0
        && a.x < b.right()
        && b.x < a.right()
        && a.y < b.bottom()
        && b.y < a.bottom()
}

/// A1 adversarial: clipboard prose must never claim a cell that also sits
/// inside a backdrop frame (shared-border junctions included).
#[test]
fn copy_rect_never_intersects_backdrop_frames() {
    let area = Rect::new(0, 0, 160, 40);
    for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
        for reasoning in [false, true] {
            let mut input = base_input(area);
            input.join = join;
            input.reasoning_visible = reasoning;
            let plan = plan_cockpit_surfaces(input);
            let copy = plan.transcript_copy_rect().expect("prose");
            for s in &plan.surfaces {
                if s.role == SurfaceRole::Backdrop {
                    assert!(
                        !rects_intersect(copy, s.frame),
                        "join={join:?} reasoning={reasoning}: copy {copy:?} intersects backdrop {:?} frame {:?}",
                        s.kind,
                        s.frame
                    );
                }
            }
        }
    }
}

/// A1: Gap mode must isolate agent/artifacts vertically the same way it
/// isolates transcript/side columns horizontally.
#[test]
fn gap_join_does_not_overlap_agent_and_artifacts_rows() {
    let area = Rect::new(0, 0, 160, 40);
    let mut input = base_input(area);
    input.join = SurfaceJoin::Gap;
    input.reasoning_visible = true;
    let plan = plan_cockpit_surfaces(input);
    let agent = plan.agent_frame().expect("agent");
    let art = plan.artifacts_frame().expect("artifacts");
    assert!(
        agent.bottom() <= art.y,
        "gap layout must not share rows: agent.bottom={} art.y={}",
        agent.bottom(),
        art.y
    );
}

/// A1: after a full-width → split rebind, selection spans stay inside copy_rect.
#[test]
fn layout_rebind_keeps_selection_inside_copy_rect() {
    use crate::ui::mouse::{PaneId, Selection, selection_spans};
    let full = Rect::new(0, 0, 160, 40);
    let mut full_in = base_input(full);
    full_in.backdrop = BackdropMode::Off;
    let full_plan = plan_cockpit_surfaces(full_in);
    let wide = full_plan.transcript_copy_rect().unwrap();
    let mut sel = Selection::new(PaneId::Transcript, wide, wide.x + 2, wide.y + 2);
    sel.extend(
        wide.right().saturating_sub(1),
        wide.bottom().saturating_sub(1),
    );
    // Layout rebind: side column appears, prose narrows.
    let split = plan_cockpit_surfaces(base_input(full));
    let live = split.transcript_copy_rect().unwrap();
    assert!(
        sel.rebind(live),
        "full-body → split rebind must keep a pick that still covers prose"
    );
    for (y, x0, x1) in selection_spans(&sel) {
        assert!((live.y..live.y + live.height).contains(&y), "row {y}");
        assert!(
            x0 >= live.x && x1 < live.x + live.width,
            "cols {x0}..={x1} left copy_rect {live:?}"
        );
    }
    // And still must not enter agent frame.
    let agent = split.agent_frame().unwrap();
    for (y, x0, x1) in selection_spans(&sel) {
        for x in x0..=x1 {
            assert!(
                !(x >= agent.x && x < agent.right() && y >= agent.y && y < agent.bottom()),
                "selection cell ({x},{y}) entered agent frame {agent:?}"
            );
        }
    }
}

/// Stress: tight width at the side-column threshold.
#[test]
fn tight_side_column_copy_stays_left_of_agent() {
    for w in [100u16, 101, 110, 120] {
        for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
            let mut input = base_input(Rect::new(0, 0, w, 36));
            input.join = join;
            input.side_width = 40;
            let plan = plan_cockpit_surfaces(input);
            if plan.agent_frame().is_none() {
                continue;
            }
            let copy = plan.transcript_copy_rect().unwrap();
            let agent = plan.agent_frame().unwrap();
            assert!(
                copy.right() <= agent.x,
                "w={w} join={join:?}: copy.right={} must be <= agent.x={}",
                copy.right(),
                agent.x
            );
        }
    }
}

/// Short side columns must not invent overlapping or inverted agent/artifacts
/// frames (Gap and SharedBorder).
#[test]
fn short_side_column_frames_stay_ordered() {
    for h in [6u16, 8, 10, 12, 16] {
        for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
            let mut input = base_input(Rect::new(0, 0, 140, h));
            input.join = join;
            input.reasoning_visible = true;
            let plan = plan_cockpit_surfaces(input);
            let Some(agent) = plan.agent_frame() else {
                continue;
            };
            let Some(art) = plan.artifacts_frame() else {
                continue;
            };
            assert!(agent.height >= 1, "h={h} join={join:?}");
            assert!(art.height >= 1, "h={h} join={join:?}");
            if join == SurfaceJoin::Gap {
                assert!(
                    agent.bottom() <= art.y,
                    "h={h}: gap agent.bottom={} art.y={}",
                    agent.bottom(),
                    art.y
                );
            } else {
                // Shared border may touch by one row, never invert.
                assert!(
                    agent.y <= art.y,
                    "h={h}: inverted agent.y={} art.y={}",
                    agent.y,
                    art.y
                );
                assert!(
                    agent.bottom() <= art.bottom(),
                    "h={h}: agent extends past artifacts"
                );
            }
        }
    }
}

/// A6: side_width 0 (or sub-border) must not open a zero-width side column.
#[test]
fn zero_or_tiny_side_width_stays_transcript_only() {
    for side in [0u16, 1, 2] {
        let mut input = base_input(Rect::new(0, 0, 160, 40));
        input.side_width = side;
        let plan = plan_cockpit_surfaces(input);
        assert!(
            plan.agent_frame().is_none() && plan.artifacts_frame().is_none(),
            "side={side}: backdrops must not open with sub-min width"
        );
        assert_eq!(
            plan.transcript_frame(),
            Some(input.area),
            "side={side}: transcript keeps full body"
        );
        assert!(plan.transcript_copy_rect().is_some());
    }
}

/// A6: short body heights collapse to one plate (or full transcript) instead
/// of inventing empty/inverted agent+artifacts bands.
#[test]
fn tiny_body_height_never_emits_empty_backdrop_frames() {
    for h in [1u16, 2, 3, 4, 5, 6] {
        for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
            let mut input = base_input(Rect::new(0, 0, 140, h));
            input.join = join;
            input.reasoning_visible = true;
            let plan = plan_cockpit_surfaces(input);
            for s in &plan.surfaces {
                assert!(
                    s.frame.width > 0 && s.frame.height > 0,
                    "h={h} join={join:?}: empty frame {:?}",
                    s.frame
                );
            }
            if let (Some(a), Some(b)) = (plan.agent_frame(), plan.artifacts_frame()) {
                if join == SurfaceJoin::Gap {
                    assert!(a.bottom() <= b.y, "h={h}: gap overlap");
                } else {
                    assert!(a.y <= b.y, "h={h}: inverted stack");
                }
            }
        }
    }
}

/// A6: exhaustive layout invariants across sizes/flags (copy isolation + geometry).
#[test]
fn a6_layout_invariants_grid() {
    let widths = [0u16, 1, 24, 50, 99, 100, 101, 120, 140, 160, 200, 240];
    let heights = [0u16, 1, 2, 3, 4, 5, 6, 8, 12, 20, 36, 48];
    let sides = [0u16, 1, 10, 40, 80, 200];
    for w in widths {
        for h in heights {
            for side in sides {
                for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
                    for reasoning in [false, true] {
                        for (aa, fa) in [(true, true), (true, false), (false, true), (false, false)]
                        {
                            let area = Rect::new(3, 2, w, h);
                            let mut input = base_input(area);
                            input.join = join;
                            input.reasoning_visible = reasoning;
                            input.side_width = side;
                            input.agent_active = aa;
                            input.artifacts_active = fa;
                            let plan = plan_cockpit_surfaces(input);
                            for s in &plan.surfaces {
                                assert!(
                                    s.frame.width > 0 && s.frame.height > 0,
                                    "zero frame {:?} kind={:?} w={w} h={h} side={side} join={join:?} aa={aa} fa={fa}",
                                    s.frame,
                                    s.kind
                                );
                                assert!(
                                    s.frame.x >= area.x
                                        && s.frame.y >= area.y
                                        && s.frame.right() <= area.right()
                                        && s.frame.bottom() <= area.bottom(),
                                    "frame spills area: {:?} area={area:?} kind={:?}",
                                    s.frame,
                                    s.kind
                                );
                                if let Some(c) = s.copy_rect {
                                    assert!(c.width > 0 && c.height > 0);
                                    assert!(
                                        c.x >= s.frame.x
                                            && c.y >= s.frame.y
                                            && c.right() <= s.frame.right()
                                            && c.bottom() <= s.frame.bottom(),
                                        "copy outside frame: copy={c:?} frame={:?}",
                                        s.frame
                                    );
                                    for o in &plan.surfaces {
                                        if o.kind == s.kind {
                                            continue;
                                        }
                                        assert!(
                                            !rects_intersect(c, o.frame),
                                            "copy {:?} intersects other {:?} frame {:?} (w={w} h={h} side={side} join={join:?})",
                                            c,
                                            o.kind,
                                            o.frame
                                        );
                                    }
                                } else if s.kind == PanelKind::Transcript {
                                    // Only legal when the frame cannot host a chrome-free cell.
                                    let inner = mouse::inner_border(s.frame);
                                    assert!(
                                        inner.width == 0 || inner.height == 0,
                                        "transcript frame {:?} dropped copy_rect with usable inner {inner:?}",
                                        s.frame
                                    );
                                }
                            }
                            if join == SurfaceJoin::Gap {
                                for i in 0..plan.surfaces.len() {
                                    for j in (i + 1)..plan.surfaces.len() {
                                        let a = &plan.surfaces[i];
                                        let b = &plan.surfaces[j];
                                        assert!(
                                            !rects_intersect(a.frame, b.frame),
                                            "gap frames intersect {:?} {:?} vs {:?} {:?} (w={w} h={h} side={side} aa={aa} fa={fa} reason={reasoning})",
                                            a.kind,
                                            a.frame,
                                            b.kind,
                                            b.frame
                                        );
                                    }
                                }
                            }
                            // SharedBorder: frames may share a 1-cell junction, never a 2D slab.
                            if join == SurfaceJoin::SharedBorder {
                                for i in 0..plan.surfaces.len() {
                                    for j in (i + 1)..plan.surfaces.len() {
                                        let a = plan.surfaces[i].frame;
                                        let b = plan.surfaces[j].frame;
                                        if !rects_intersect(a, b) {
                                            continue;
                                        }
                                        let ix = a.x.max(b.x);
                                        let iy = a.y.max(b.y);
                                        let iw = a.right().min(b.right()).saturating_sub(ix);
                                        let ih = a.bottom().min(b.bottom()).saturating_sub(iy);
                                        assert!(
                                            iw == 1 || ih == 1,
                                            "shared-border overlap thicker than junction: {:?}∩{:?} = {}x{} (w={w} h={h} side={side})",
                                            a,
                                            b,
                                            iw,
                                            ih
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A6 adversarial: after horizontal split, a non-zero but sub-min side
/// column (or empty-inner backdrop) must not be published — reclaim full
/// body transcript instead of registering a useless hit target.
#[test]
fn side_column_below_min_outer_reclaims_full_transcript() {
    // Force the planner through show_side, then simulate what happens when
    // layout yields a skinny right plate by using the narrowest legal inputs
    // and asserting every published backdrop still has usable chrome-free
    // inner cells. Also pin the reclaim path: width just under the
    // side-column threshold stays full-body.
    let mut input = base_input(Rect::new(0, 0, 99, 40));
    input.side_width = 40;
    let plan = plan_cockpit_surfaces(input);
    assert!(
        plan.agent_frame().is_none() && plan.artifacts_frame().is_none(),
        "w=99 must stay transcript-only"
    );
    assert_eq!(plan.transcript_frame(), Some(Rect::new(0, 0, 99, 40)));

    // Legal split: every backdrop frame must admit Borders::ALL content.
    for w in [100u16, 101, 120, 160, 200] {
        for side in [3u16, 4, 10, 40, 80] {
            for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
                for h in [3u16, 5, 7, 12, 40] {
                    let mut input = base_input(Rect::new(2, 1, w, h));
                    input.side_width = side;
                    input.join = join;
                    input.agent_active = true;
                    input.artifacts_active = true;
                    let plan = plan_cockpit_surfaces(input);
                    for s in &plan.surfaces {
                        if s.role != SurfaceRole::Backdrop {
                            continue;
                        }
                        let inner = mouse::inner_border(s.frame);
                        assert!(
                            inner.width > 0 && inner.height > 0,
                            "backdrop {:?} frame {:?} has empty inner (w={w} h={h} side={side} join={join:?})",
                            s.kind,
                            s.frame
                        );
                        assert!(
                            s.frame.width >= MIN_SIDE_OUTER && s.frame.height >= MIN_SIDE_OUTER,
                            "backdrop {:?} frame {:?} below MIN_SIDE_OUTER",
                            s.kind,
                            s.frame
                        );
                    }
                }
            }
        }
    }
}

/// Static and live clipboard geometry must agree with renderer body/rail
/// geometry at tiny, split, and fullscreen widths, including strip collapse.
#[test]
fn transcript_copy_agrees_with_draw_measure() {
    for w in [0u16, 1, 2, 3, 4, 10, 40, 103, 104, 105, 120, 200, 320] {
        for h in [0u16, 1, 2, 3, 4, 10, 40] {
            let frame = Rect::new(3, 5, w, h);
            let inner = mouse::inner_border(frame);
            for strip_h in [0, 1, 2, 3, 5, h, u16::MAX] {
                let body = Rect {
                    height: inner.height.saturating_sub(strip_h),
                    ..inner
                };
                let (text, rail) = crate::ui::draw::transcript_text_and_rail(body);
                let expected = (text.width > 0 && text.height > 0).then_some(text);
                assert_eq!(
                    transcript_live_copy_rect(frame, strip_h),
                    expected,
                    "copy must equal renderer text: frame={frame:?}, strip={strip_h}"
                );
                if let Some(rail) = rail {
                    assert_eq!(text.right(), rail.x);
                    assert_eq!(rail.right(), inner.right());
                }
            }
        }
    }
}

/// A6 adversarial: a live tool strip must shrink the clipboard prose rect so
/// selection/copy cannot cover status/loading chrome at the pane bottom.
#[test]
fn live_copy_rect_excludes_tool_strip_rows() {
    let frame = Rect::new(0, 0, 48, 24);
    let full = transcript_live_copy_rect(frame, 0).expect("full prose");
    for strip_h in [1u16, 2, 3, 5] {
        let live = transcript_live_copy_rect(frame, strip_h).expect("strip prose");
        assert_eq!(live.x, full.x, "strip_h={strip_h}");
        assert_eq!(live.y, full.y, "strip_h={strip_h}");
        assert_eq!(live.width, full.width, "strip_h={strip_h}");
        assert_eq!(
            live.height,
            full.height.saturating_sub(strip_h),
            "strip_h={strip_h}: height must drop by strip"
        );
        assert_eq!(
            live.bottom(),
            full.bottom().saturating_sub(strip_h),
            "strip_h={strip_h}: bottom must sit above strip"
        );
        // Strip band must not intersect live copy.
        let strip_band = Rect {
            x: full.x,
            y: live.bottom(),
            width: full.width,
            height: strip_h.min(full.height),
        };
        assert!(
            !rects_intersect(live, strip_band) || strip_band.height == 0,
            "live copy intersects strip band: live={live:?} strip={strip_band:?}"
        );
    }
    // Strip taller than the pane yields no copy surface.
    let inner = mouse::inner_border(frame);
    assert!(transcript_live_copy_rect(frame, inner.height).is_none());
    assert!(transcript_live_copy_rect(frame, inner.height + 5).is_none());
}

/// A6: Gap join must reserve the spacer row when sizing the agent plate so
/// Layout does not shrink plates below MIN_SIDE_OUTER.
#[test]
fn gap_stack_plates_stay_at_least_min_outer() {
    for h in 7u16..=24 {
        for reason in [false, true] {
            let mut input = base_input(Rect::new(0, 0, 140, h));
            input.join = SurfaceJoin::Gap;
            input.reasoning_visible = reason;
            input.idle_agent_height_cap = 100;
            input.agent_active = true;
            input.artifacts_active = true;
            let plan = plan_cockpit_surfaces(input);
            let (Some(a), Some(b)) = (plan.agent_frame(), plan.artifacts_frame()) else {
                // Short columns may collapse to one plate — that plate must
                // still be usable.
                let plate = plan.agent_frame().or(plan.artifacts_frame());
                if let Some(p) = plate {
                    assert!(
                        p.width >= MIN_SIDE_OUTER && p.height >= MIN_SIDE_OUTER,
                        "h={h}: single plate sub-min {p:?}"
                    );
                    let inner = mouse::inner_border(p);
                    assert!(inner.width > 0 && inner.height > 0, "h={h}: empty inner");
                }
                continue;
            };
            assert!(
                a.height >= MIN_SIDE_OUTER && b.height >= MIN_SIDE_OUTER,
                "h={h} reason={reason}: sub-min stack a={a:?} b={b:?}"
            );
            assert!(a.bottom() <= b.y, "h={h}: gap overlap a={a:?} b={b:?}");
            assert_eq!(
                b.y.saturating_sub(a.bottom()),
                1,
                "h={h}: expected 1-row gap a={a:?} b={b:?}"
            );
        }
    }
}

/// A6: split geometry conserves body width; stacked plates fill the side column.
#[test]
fn split_frames_conserve_body_width() {
    for w in [100u16, 101, 110, 120, 140, 160, 200, 240] {
        for h in [5u16, 7, 8, 12, 20, 40] {
            for side in [3u16, 10, 40, 70, 100] {
                for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
                    for (aa, fa) in [(true, true), (true, false), (false, true)] {
                        let area = Rect::new(2, 1, w, h);
                        let mut input = base_input(area);
                        input.side_width = side;
                        input.join = join;
                        input.agent_active = aa;
                        input.artifacts_active = fa;
                        input.reasoning_visible = false;
                        let plan = plan_cockpit_surfaces(input);
                        let Some(tr) = plan.transcript_frame() else {
                            continue;
                        };
                        let agent = plan.agent_frame();
                        let art = plan.artifacts_frame();
                        if agent.is_none() && art.is_none() {
                            assert_eq!(tr, area, "transcript-only must own full body");
                            continue;
                        }
                        let side_col = agent.or(art).unwrap();
                        // Horizontal conservation against the side *column* x/width
                        // (both plates share the same column when stacked).
                        match join {
                            SurfaceJoin::Gap => {
                                assert!(
                                    tr.right() <= side_col.x,
                                    "gap: tr must end before side: tr={tr:?} side={side_col:?}"
                                );
                                let gap = side_col.x.saturating_sub(tr.right());
                                assert_eq!(
                                    gap, 1,
                                    "gap join must leave exactly 1 column: tr={tr:?} side={side_col:?}"
                                );
                                assert_eq!(
                                    tr.width + side_col.width + gap,
                                    area.width,
                                    "gap width conservation tr={tr:?} side={side_col:?}"
                                );
                            }
                            SurfaceJoin::SharedBorder => {
                                let ow = tr
                                    .right()
                                    .min(side_col.right())
                                    .saturating_sub(tr.x.max(side_col.x));
                                assert_eq!(
                                    ow, 1,
                                    "shared border must overlap 1 col: tr={tr:?} side={side_col:?}"
                                );
                                assert_eq!(
                                    tr.width + side_col.width - 1,
                                    area.width,
                                    "shared width conservation tr={tr:?} side={side_col:?}"
                                );
                            }
                        }
                        assert_eq!(tr.y, area.y);
                        assert_eq!(tr.height, area.height);
                        // Stacked plates: fill the side column top-to-bottom.
                        match (agent, art) {
                            (Some(a), Some(b)) => {
                                assert_eq!(a.x, b.x);
                                assert_eq!(a.width, b.width);
                                assert_eq!(a.y, area.y, "agent tops the column");
                                assert_eq!(
                                    b.bottom(),
                                    area.bottom(),
                                    "artifacts must reach body bottom: a={a:?} b={b:?} area={area:?}"
                                );
                                if join == SurfaceJoin::Gap {
                                    assert!(
                                        a.bottom() <= b.y,
                                        "gap stack must not overlap: a={a:?} b={b:?}"
                                    );
                                    assert_eq!(
                                        b.y.saturating_sub(a.bottom()),
                                        1,
                                        "gap stack leaves 1 row: a={a:?} b={b:?}"
                                    );
                                } else {
                                    // Shared border: one-row junction.
                                    let oh =
                                        a.bottom().min(b.bottom()).saturating_sub(a.y.max(b.y));
                                    // May be 0 if they only touch at edge without area overlap
                                    // (bottom of a == y of b). SharedBorder spacing -1 means
                                    // 1-row overlap.
                                    assert_eq!(
                                        oh, 1,
                                        "shared stack overlap 1 row: a={a:?} b={b:?} oh={oh}"
                                    );
                                }
                            }
                            (Some(a), None) | (None, Some(a)) => {
                                assert_eq!(
                                    a,
                                    Rect::new(side_col.x, area.y, side_col.width, area.height),
                                    "single plate fills side column"
                                );
                            }
                            (None, None) => unreachable!(),
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn pane_clipboard_policy() {
    assert!(pane_accepts_clipboard(PaneId::Transcript));
    assert!(pane_accepts_clipboard(PaneId::AgentBay));
    assert!(!pane_accepts_clipboard(PaneId::Artifacts));
    assert!(!pane_accepts_clipboard(PaneId::Input));
    assert!(!pane_accepts_clipboard(PaneId::Shell));
}
