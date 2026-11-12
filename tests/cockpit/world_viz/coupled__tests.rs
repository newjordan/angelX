//! The miniworld's tests that need the cockpit: a `LoopState` walking the
//! adventure mirror, the scryglass route, the realm's wish catalog, a guest's
//! picture and the resident art through the cockpit's image encoder. They ran
//! inside the world's own modules; the world is the realm host's now, so they
//! reach it by its public paths and stay here beside the types they need.

use crate::drive::loop_ctl::{EscalationTier as StateTier, LoopState, LoopStatus as StateStatus};
use crate::delve::world_viz::Building;
use crate::delve::world_viz::adventure::*;
use crate::delve::world_viz::overworld::map_enabled;
use crate::telemetry::ToolEventId;

/// The mirror emits the full ladder from a fake `LoopState` walk.
#[test]
fn loop_mirror_drains_the_event_ladder() {
    let mix = ToolMix::default();
    let mut mirror = LoopMirror::default();
    let party = 1u8;

    // Boot: nothing.
    let mut st = LoopState::default();
    assert!(mirror.drain(&st.summary(), &mix, party).is_empty());

    // A competition loop starts.
    st.status = StateStatus::Running;
    st.task = "optimize the kernel benchmark".to_string();
    st.podrace = true;
    let events = mirror.drain(&st.summary(), &mix, party);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        AdventureEvent::LoopStarted {
            kind: LoopKind::Competition,
            ..
        }
    ));

    // One observed inactive iteration → level 3 (pivot default 0 → max(1)).
    st.stale_count = 1;
    st.iteration = 1;
    st.log.push(
        serde_json::from_value(serde_json::json!({
            "iteration":1,"direction":"waiting","new_findings":0,"stale_count":1,"ts_ms":0
        }))
        .unwrap(),
    );
    let events = mirror.drain(&st.summary(), &mix, party);
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(events[0], AdventureEvent::Iteration { n: 1 }));
    assert!(matches!(events[1], AdventureEvent::Stall { level: 3 }));

    // A measurement receipt is activity; prose does not prove improvement.
    st.stale_count = 0;
    st.iteration = 2;
    st.measured_candidates = 1;
    st.log.push(crate::drive::loop_ctl::LoopIterLog {
        iteration: 2,
        direction: "tuned the tile loop — improved throughput".to_string(),
        new_findings: 0,
        reported_findings: 0,
        unverified_findings: 0,
        tool_calls: 1,
        tool_errors: 0,
        duplicate_costly_actions: 0,
        outcome_progress: 0,
        novel_outcome_actions: 0,
        verified_outcome_actions: 1,
        workspace_changed: false,
        evidence_review: false,
        stale_count: 0,
        ts_ms: 0,
    });
    let events = mirror.drain(&st.summary(), &mix, party);
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(matches!(events[0], AdventureEvent::Iteration { n: 2 }));
    assert!(matches!(events[1], AdventureEvent::Stall { level: 0 }));
    assert!(matches!(events[2], AdventureEvent::MeasurementObserved));

    // Escalation is a pivot.
    st.tier = StateTier::Swarm;
    let events = mirror.drain(&st.summary(), &mix, party);
    assert!(matches!(events[0], AdventureEvent::Pivot), "{events:?}");

    // Submitting, then the acceptance gate takes the work.
    st.submissions = 1;
    let events = mirror.drain(&st.summary(), &mix, party);
    assert!(matches!(events[0], AdventureEvent::Submitted), "{events:?}");
    st.status = StateStatus::Verifying;
    let events = mirror.drain(&st.summary(), &mix, party);
    assert!(matches!(events[0], AdventureEvent::Submitted), "{events:?}");

    // Accepted and done in one transition: settled before finished.
    st.status = StateStatus::Done;
    let events = mirror.drain(&st.summary(), &mix, party);
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(
        events[0],
        AdventureEvent::SubmissionSettled { accepted: true }
    ));
    assert!(matches!(
        events[1],
        AdventureEvent::LoopFinished { ok: true }
    ));

    // Party changes ride along.
    let events = mirror.drain(&st.summary(), &mix, 3);
    assert!(matches!(events[0], AdventureEvent::Party { size: 3 }));
}

#[test]
fn loop_mirror_verifier_block_forces_swamp_danger() {
    let mix = ToolMix::default();
    let mut mirror = LoopMirror::default();
    let mut st = LoopState {
        status: StateStatus::Running,
        task: "vague".to_string(),
        pivot: 3,
        stale_count: 1,
        ..Default::default()
    };
    let events = mirror.drain(&st.summary(), &mix, 1);
    assert!(matches!(
        events.last(),
        Some(AdventureEvent::Stall { level: 1 })
    ));
    st.verifier_blocked = Some("benchmark missing --golden".to_string());
    let events = mirror.drain(&st.summary(), &mix, 1);
    assert!(
        matches!(events.last(), Some(AdventureEvent::Stall { level: 2 })),
        "verifier_blocked forces ≥2: {events:?}"
    );
}

#[test]
fn loop_mirror_rejected_gate_lands_without_penalty() {
    let mix = ToolMix::default();
    let mut mirror = LoopMirror::default();
    let mut st = LoopState {
        status: StateStatus::Verifying,
        task: "fix the flake".to_string(),
        ..Default::default()
    };
    mirror.drain(&st.summary(), &mix, 1);
    // The gate fails and the loop arms another iteration: the settled
    // verdict rides after the iteration event per the emission order.
    st.status = StateStatus::Running;
    st.iteration = 1;
    let events = mirror.drain(&st.summary(), &mix, 1);
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(events[0], AdventureEvent::Iteration { n: 1 }));
    assert!(matches!(
        events[1],
        AdventureEvent::SubmissionSettled { accepted: false }
    ));
}

#[test]
fn active_podrace_without_comparable_objective_is_not_a_visual_stall() {
    let mut st = LoopState {
        status: StateStatus::Running,
        podrace: true,
        stale_count: 999,
        pivot: 3,
        ..Default::default()
    };
    assert_eq!(
        LoopMirror::stall_level(&st.summary()),
        0,
        "missing observations do not prove inactivity"
    );
    st.log.push(serde_json::from_value(serde_json::json!({"iteration":1,"direction":"deep cut explored","new_findings":0,"tool_calls":3,"tool_errors":0,"workspace_changed":true,"stale_count":999,"ts_ms":0})).unwrap());
    assert_eq!(LoopMirror::stall_level(&st.summary()), 0);
    st.log.last_mut().unwrap().tool_errors = 3;
    assert_eq!(
        LoopMirror::stall_level(&st.summary()),
        2,
        "observed tool failures remain visible"
    );
    st.log.last_mut().unwrap().tool_errors = 0;
    st.verifier_blocked = Some("verifier unavailable".into());
    assert_eq!(
        LoopMirror::stall_level(&st.summary()),
        2,
        "concrete verifier blocker remains visible"
    );
}

#[test]
fn prose_improvement_never_awards_treasure_for_an_opaque_measurement() {
    let mut st = LoopState {
        status: StateStatus::Running,
        podrace: true,
        ..Default::default()
    };
    let mut mirror = LoopMirror::default();
    let mix = ToolMix::default();
    let mut quest = Quest::idle();
    for event in mirror.drain(&st.summary(), &mix, 1) {
        quest.apply(event, 0);
    }
    st.measured_candidates = 1;
    st.log.push(serde_json::from_value(serde_json::json!({"iteration":1,"direction":"improved faster better","new_findings":0,"tool_calls":1,"verified_outcome_actions":1,"stale_count":999,"ts_ms":0})).unwrap());
    let events = mirror.drain(&st.summary(), &mix, 1);
    assert!(events.contains(&AdventureEvent::MeasurementObserved));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AdventureEvent::Measured { .. }))
    );
    for event in events {
        quest.apply(event, 1);
    }
    assert_eq!(quest.treasures(), 0);
    assert_eq!(
        quest.empty_chests(),
        0,
        "unknown is neither reward nor failure"
    );
    assert_eq!(quest.region(), Region::TheMines);
}

/// A submission's official result as the board returns it: accepted, and
/// whether it improved and was promoted.
fn official_result(
    id: &str,
    promoted: bool,
) -> crate::agent::harness::cartridges::records::TerminalEvidence {
    serde_json::from_value(serde_json::json!({
        "official": {
            "id": id,
            "benchmark_id": "kernel",
            "status": "accepted",
            "promotion_status": if promoted { "promoted" } else { "held" },
            "promoted_source_ref": "refs/heads/candidate",
            "official_score": "948.94M",
            "improved": promoted,
            "fetched_at_ms": 1
        },
        "benchmark_id": "kernel",
        "submission_id": id,
        "status": "accepted",
        "frontier_win_verified": promoted,
        "source_id": "test"
    }))
    .unwrap()
}

/// A promoted record brings the guardian's fight; one already promoted
/// when the mirror first looks is history; a result that wasn't promoted
/// brings nothing.
#[test]
fn a_promoted_record_brings_the_guardian_and_only_news_does() {
    let mix = ToolMix::default();
    let mut mirror = LoopMirror::default();
    let mut st = LoopState {
        status: StateStatus::Running,
        ..LoopState::default()
    };
    st.submission_results.push(official_result("old", true));
    let first = mirror.drain(&st.summary(), &mix, 1);
    assert!(
        !first
            .iter()
            .any(|e| matches!(e, AdventureEvent::RecordPromoted { .. })),
        "an old record isn't news"
    );
    st.submission_results.push(official_result("held", false));
    assert!(
        !mirror
            .drain(&st.summary(), &mix, 1)
            .iter()
            .any(|e| matches!(e, AdventureEvent::RecordPromoted { .. }))
    );
    st.submission_results.push(official_result("new", true));
    let events = mirror.drain(&st.summary(), &mix, 1);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, AdventureEvent::RecordPromoted { .. }))
            .count(),
        1
    );
    assert!(events.contains(&AdventureEvent::RecordPromoted {
        score: Some("948.94M".into())
    }));
}

/// The quest keeps the guardian's fight for three minutes from the record.
#[test]
fn the_guardians_fight_lasts_three_minutes_from_the_record() {
    let mut q = Quest::idle();
    assert_eq!(q.record_fight(10), None);
    q.apply(
        AdventureEvent::RecordPromoted {
            score: Some("1.2s".into()),
        },
        100,
    );
    assert_eq!(q.record_fight(100), Some((0, true)));
    assert_eq!(
        q.record_fight(100 + RECORD_FIGHT - 1),
        Some((RECORD_FIGHT as u32 - 1, true))
    );
    assert_eq!(q.record_fight(100 + RECORD_FIGHT), None);
    assert_eq!((q.records(), q.record_score()), (1, Some("1.2s")));
    // A promotion that didn't come: a shorter fight, and a retreat.
    q.apply(AdventureEvent::PromotionFailed { score: None }, 10_000);
    assert_eq!(
        q.record_fight(10_000 + RETREAT_FIGHT - 1).map(|f| f.1),
        Some(false)
    );
    assert_eq!(q.record_fight(10_000 + RETREAT_FIGHT), None);
    assert_eq!(q.bouts(), 2);
}

/// A submission's official result that settled without a promotion, or
/// hasn't settled yet.
fn settled_result(
    id: &str,
    status: &str,
    promotion: Option<&str>,
    improved: Option<bool>,
) -> crate::agent::harness::cartridges::records::TerminalEvidence {
    serde_json::from_value(serde_json::json!({
        "official": {
            "id": id,
            "benchmark_id": "kernel",
            "status": status,
            "promotion_status": promotion,
            "official_score": "951.02M",
            "improved": improved,
            "fetched_at_ms": 1
        },
        "benchmark_id": "kernel",
        "submission_id": id,
        "status": status,
        "frontier_win_verified": false,
        "source_id": "test"
    }))
    .unwrap()
}

/// A submission settled without a promotion is a retreat; one still
/// waiting on its promotion is nothing yet.
#[test]
fn a_promotion_that_didnt_come_is_a_retreat_and_a_pending_one_is_nothing() {
    let mix = ToolMix::default();
    let mut mirror = LoopMirror::default();
    let mut st = LoopState {
        status: StateStatus::Running,
        ..LoopState::default()
    };
    mirror.drain(&st.summary(), &mix, 1);
    let failed = |events: &[AdventureEvent]| {
        events
            .iter()
            .filter(|e| matches!(e, AdventureEvent::PromotionFailed { .. }))
            .count()
    };
    st.submission_results
        .push(settled_result("waiting", "pending", None, None));
    st.submission_results.push(settled_result(
        "deciding",
        "accepted",
        Some("unavailable"),
        None,
    ));
    assert_eq!(
        failed(&mirror.drain(&st.summary(), &mix, 1)),
        0,
        "not settled yet"
    );
    st.submission_results
        .push(settled_result("no", "rejected", None, None));
    assert_eq!(failed(&mirror.drain(&st.summary(), &mix, 1)), 1);
    st.submission_results.push(settled_result(
        "held",
        "accepted",
        Some("held"),
        Some(false),
    ));
    let events = mirror.drain(&st.summary(), &mix, 1);
    assert!(events.contains(&AdventureEvent::PromotionFailed {
        score: Some("951.02M".into())
    }));
}

#[test]
fn the_realm_route_keeps_travel_on_the_map() {
    use crate::ui::scryglass::{Scryglass, StageRoute, StageSurface};
    let _env = crate::tests::env_lock();
    let mut stage = Scryglass::default();
    stage.sync_arrival(Some(Building::Keep));
    stage.begin_journey(ToolEventId("walk".into()), Building::Smithy, false);
    assert_eq!(stage.controller.route(), StageRoute::Realm);
    {
        let _map = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_MAP");
        assert!(map_enabled());
        assert_eq!(
            stage.controller.resolved_scene(false, false, true),
            StageSurface::WorldMap,
            "the knight walks on the map, even when a quest owns the pane"
        );
    }
    let _ride = crate::tests::TestEnvGuard::set("ANGEL_WORLD_MAP", "3d");
    assert!(!map_enabled());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldFirstPerson
    );
}

#[test]
fn catalog_wishes_pass_their_checker_without_price_notes() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/realm/wishes");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert_eq!(
        files.len(),
        7,
        "six from the start, the Grail Chapel after a victory"
    );
    for path in &files {
        let id = path.file_stem().unwrap().to_str().unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        let (draft, notes) = crate::delve::together_realm::check(id, &raw).unwrap();
        assert!(notes.is_empty(), "{id}: {notes:?}");
        assert!(!draft.near.is_empty() && !draft.words.is_empty(), "{id}");
    }
    // The works a surveyor plans are raised by the knights where they stand
    // (`raisings`), not granted from the catalog; the Grail Chapel waits on a victory.
    let granted = files
        .iter()
        .map(|p| p.file_stem().unwrap().to_str().unwrap())
        .filter(|id| *id != "grail-chapel" && angel_delve::rules::raisings::plan(id).is_none())
        .count();
    assert!(granted > 0);
    assert_eq!(catalog_built().len(), granted);
}

/// Every catalog wish, granted from a full treasury, in catalog order.
fn catalog_built() -> std::sync::Arc<Vec<crate::delve::together_realm::Wish>> {
    use crate::delve::together_realm::{Realm, Spoil, Spoils};
    let mut realm = Realm::default();
    realm.offer_catalog();
    let mut plenty = Spoils::default();
    for spoil in Spoil::ALL {
        plenty.add(spoil, 10_000);
    }
    realm.bank("Jordan", &plenty);
    let ids: Vec<String> = realm.wishes.iter().map(|w| w.id.clone()).collect();
    for id in ids {
        realm.grant(&id).unwrap();
    }
    std::sync::Arc::new(realm.built())
}

/// The realm host's overworld tests read the granted catalog from a snapshot
/// (`crates/realm-host/tests/fixtures/catalog_built.json`) because the realm
/// that grants it is the cockpit's. This keeps the snapshot honest: it fails
/// when the catalog moves on, and `ANGEL_WRITE_CATALOG_SNAPSHOT=1 cargo test
/// catalog_snapshot` rewrites it.
#[test]
fn catalog_snapshot_matches_the_realm() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/realm-host/tests/fixtures/catalog_built.json");
    let built = catalog_built();
    if std::env::var_os("ANGEL_WRITE_CATALOG_SNAPSHOT").is_some() {
        let mut json = serde_json::to_string_pretty(&*built).unwrap();
        json.push('\n');
        std::fs::write(&path, json).unwrap();
    }
    let snapshot: Vec<crate::delve::together_realm::Wish> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        snapshot, *built,
        "the catalog changed: ANGEL_WRITE_CATALOG_SNAPSHOT=1 cargo test catalog_snapshot"
    );
}

/// Resident cockpit art through the image braille encoder, as the crawl's
/// review dump shows it: `ANGEL_BRAILLE_SHOTS=<dir> cargo test
/// resident_art_through_the_image_braille_encoder -- --ignored`.
#[test]
#[ignore]
fn resident_art_through_the_image_braille_encoder() {
    let Some(dir) = std::env::var_os("ANGEL_BRAILLE_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, path) in [
        ("dmd-council", "assets/moa-cards/council.png"),
        ("dmd-duel", "assets/moa-cards/duel.png"),
        ("dmd-recon", "assets/moa-cards/recon.png"),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        if let Some(cells) = crate::ui::term::art::colored_image_braille(&path, 60, 24) {
            terminal_preview(&cells)
                .save(dir.join(format!("{name}-braille.png")))
                .unwrap();
        }
    }
}

/// The braille a terminal shows for a frame, drawn as dots.
fn terminal_preview(cells: &crate::ui::term::art::ColoredBrailleImage) -> image::RgbaImage {
    let mut output = image::RgbaImage::from_pixel(
        cells.width as u32 * 8,
        cells.height as u32 * 16,
        image::Rgba([10, 10, 16, 255]),
    );
    for y in 0..cells.height {
        for x in 0..cells.width {
            let cell = cells.cells[y * cells.width + x];
            let bits = (cell.glyph as u32).saturating_sub(0x2800) as u8;
            for dy in 0..4 {
                for dx in 0..2 {
                    if bits & crate::ui::term::art::braille_dot_bit(dx, dy) == 0 {
                        continue;
                    }
                    for py in 0..3 {
                        for px in 0..3 {
                            output.put_pixel(
                                (x * 8 + dx * 4 + 1 + px) as u32,
                                (y * 16 + dy * 4 + 1 + py) as u32,
                                image::Rgba([cell.fg[0], cell.fg[1], cell.fg[2], 255]),
                            );
                        }
                    }
                }
            }
        }
    }
    output
}
