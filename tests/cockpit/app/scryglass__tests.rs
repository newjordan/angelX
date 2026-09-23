use super::*;

static TEST_STILL_RENDER_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[test]
fn render_lease_admission_survives_abandoned_requests() {
    let lease = acquire_still_render(&TEST_STILL_RENDER_IN_FLIGHT).unwrap();
    assert!(acquire_still_render(&TEST_STILL_RENDER_IN_FLIGHT).is_none());

    drop(lease);
    assert!(acquire_still_render(&TEST_STILL_RENDER_IN_FLIGHT).is_some());
}

#[test]
fn automatic_reveals_queue_and_manual_reveal_supersedes() {
    let mut stage = Scryglass::default();
    stage.reveal_media(1, false);
    stage.reveal_media(2, false);
    assert_eq!(stage.active_media(), Some(1));
    assert_eq!(stage.pending_count(), 1);
    stage.reveal_media(3, true);
    assert_eq!(stage.active_media(), Some(3));
    assert_eq!(stage.pending_count(), 0);
    assert!(stage.active_pinned());
}

#[test]
fn camera_is_bounded_and_follow_resets_it() {
    let mut stage = Scryglass::default();
    stage.adjust_look(9.0, 1.0);
    stage.adjust_fov(8.0);
    assert!(!stage.follow_agent);
    assert_eq!(stage.look_pitch, 0.30);
    assert_eq!(stage.fov, 1.40);
    stage.follow();
    assert!(stage.follow_agent);
    assert_eq!(stage.look_yaw, 0.0);
    assert_eq!(stage.fov, 1.05);
}

#[test]
fn still_inspection_pin_is_sticky_against_timer_and_queued_tools() {
    let mut stage = Scryglass::default();
    stage.navigate(StageRoute::Explore(Building::Smithy));
    stage.reveal_media(0, false);
    stage.note_media_painted();
    let request = stage.media_request_id();
    stage.reveal_media(1, false);
    assert_eq!(stage.pending_count(), 1);
    stage.pin_inspection();
    stage.pin_inspection();
    assert_eq!(stage.pending_count(), 0);
    stage.reveal_media(2, false);
    let start = Instant::now();
    stage.set_stage_visibility(true, true);
    stage.tick_visible(start, false, false);
    stage.tick_visible(start + Duration::from_secs(3600), false, false);
    assert_eq!(stage.active_media(), Some(0));
    assert_eq!(stage.media_request_id(), request);
    assert!(stage.active_pinned());
    assert!(stage.back_overlay());
    assert_eq!(
        stage.controller.route(),
        StageRoute::Explore(Building::Smithy)
    );
    assert_eq!(stage.pending_count(), 0);
    assert!(stage.active_media().is_none());
}

#[test]
fn still_reveal_expires_after_eight_visible_seconds() {
    let mut stage = Scryglass::default();
    stage.reveal_media(4, false);
    let reveal = stage.reveal.as_mut().unwrap();
    reveal.state = RevealState::Showing;
    reveal.visible_for = Duration::from_millis(7_999);
    assert!(!reveal.expired(false));
    reveal.visible_for = Duration::from_secs(8);
    assert!(reveal.expired(false));
}

#[test]
fn show_work_compositor_bookkeeping_preserves_visible_reveal_time() {
    let mut stage = Scryglass::default();
    stage.reveal_media(0, false);
    stage.note_media_painted();
    let start = Instant::now();
    for seconds in 0..=8 {
        stage.begin_frame();
        stage.set_stage_visibility(true, true);
        stage.tick_visible(start + Duration::from_secs(seconds), false, false);
        stage.finish_frame();
    }
    assert!(
        stage.active_media().is_none(),
        "continuous compositor frames must consume visible reveal time"
    );
    stage.reveal_media(1, false);
    stage.note_media_painted();
    stage.set_stage_visibility(true, true);
    stage.tick_visible(start, false, false);
    stage.begin_frame();
    stage.finish_frame();
    stage.set_stage_visibility(true, true);
    stage.tick_visible(start + Duration::from_secs(60), false, false);
    assert_eq!(
        stage.active_media(),
        Some(1),
        "a genuinely hidden interval must remain excluded"
    );
}

#[test]
fn realm_is_the_startup_surface_without_an_arrival() {
    let stage = Scryglass::default();
    assert_eq!(stage.world_mode(), WorldMode::Map);
    assert_eq!(stage.arrival(), None);
    assert_eq!(stage.controller.route(), StageRoute::Realm);
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldMap
    );
}

#[test]
fn persistent_world_mode_follows_unwound_route_history() {
    let mut stage = Scryglass::default();
    stage.navigate(StageRoute::Explore(Building::Smithy));
    stage.navigate(StageRoute::Observatory);
    stage.navigate(StageRoute::Realm);
    assert_eq!(stage.world_mode(), WorldMode::Map);

    assert!(stage.controller.back());
    assert_eq!(stage.controller.route(), StageRoute::Observatory);
    assert!(stage.controller.back());
    assert_eq!(
        stage.controller.route(),
        StageRoute::Explore(Building::Smithy)
    );
    assert_eq!(stage.world_mode(), WorldMode::FirstPerson);
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldFirstPerson
    );
}

#[test]
fn controller_back_dismisses_overlay_then_unwinds_a_bounded_route_stack() {
    let mut controller = StageController::default();
    for route in [
        StageRoute::Observatory,
        StageRoute::Quest,
        StageRoute::Workshop,
        StageRoute::Formation,
        StageRoute::Vault,
        StageRoute::Raytrace,
        StageRoute::Loop,
        StageRoute::Explore(Building::Smithy),
        StageRoute::Observatory,
    ] {
        controller.navigate(route);
    }
    assert_eq!(controller.back_stack.len(), 8);
    controller.show_overlay(StageOverlay::Media { index: 4 });
    assert_eq!(
        controller.resolved_scene(false, false, false),
        StageSurface::Still(4)
    );
    assert!(controller.back());
    assert_eq!(controller.route(), StageRoute::Observatory);
    assert!(controller.overlay().is_none());
    assert!(controller.back());
    assert_eq!(controller.route(), StageRoute::Explore(Building::Smithy));
}

#[test]
fn subsystem_route_close_rehomes_media_to_the_recorded_owner() {
    let mut controller = StageController::default();
    controller.navigate(StageRoute::Observatory);
    controller.navigate(StageRoute::Raytrace);
    controller.show_overlay(StageOverlay::Media { index: 3 });

    assert!(controller.leave_route(StageRoute::Raytrace));
    assert_eq!(controller.route(), StageRoute::Observatory);
    assert_eq!(controller.overlay_owner(), Some(StageRoute::Observatory));
    assert_eq!(
        controller.resolved_scene(false, false, false),
        StageSurface::Still(3)
    );
    assert!(controller.back());
    assert_eq!(controller.route(), StageRoute::Observatory);
}

#[test]
fn explicit_reset_clears_route_history_and_overlay_ownership() {
    let mut controller = StageController::default();
    controller.navigate(StageRoute::Observatory);
    controller.navigate(StageRoute::Quest);
    controller.show_overlay(StageOverlay::Media { index: 5 });

    controller.reset(StageRoute::Realm);
    assert_eq!(controller.route(), StageRoute::Realm);
    assert!(controller.overlay().is_none());
    assert_eq!(controller.overlay_owner(), None);
    assert!(
        !controller.back(),
        "reset history must not reopen an abandoned route"
    );
}

#[test]
fn automatic_travel_never_displaces_an_operator_selected_destination() {
    let mut controller = StageController::default();
    controller.navigate(StageRoute::Observatory);
    assert!(!controller.show_overlay(StageOverlay::Journey {
        call_id: ToolEventId("call-9".to_string()),
        destination: Building::Smithy,
    }));
    assert!(!controller.show_overlay(StageOverlay::Arrival {
        destination: Building::Smithy,
    }));
    assert_eq!(
        controller.resolved_scene(false, false, false),
        StageSurface::Observatory
    );
    assert!(controller.overlay().is_none());
    assert!(controller.back());
    assert_eq!(controller.route(), StageRoute::Realm);
}

#[test]
fn world_lesson_is_sourced_overlay_and_back_clears_its_state() {
    let mut stage = Scryglass::default();
    assert!(stage.set_ready_lesson(
        "Poisson",
        "Poisson distribution",
        "A discrete probability distribution."
    ));
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::Lesson
    );
    assert!(stage.lesson().is_some());
    stage.scroll_lesson_down(7);
    assert_eq!(stage.lesson_scroll(), 7);
    assert!(!stage.controller.show_overlay(StageOverlay::Journey {
        call_id: ToolEventId("call-during-lesson".to_string()),
        destination: Building::Smithy,
    }));
    assert!(stage.back_overlay());
    assert_eq!(stage.lesson_scroll(), 0);
    assert!(stage.lesson().is_none());
    assert!(stage.controller.overlay().is_none());
}

#[test]
fn living_catalog_is_a_scrollable_world_overlay_with_clean_back_state() {
    let mut stage = Scryglass::default();
    stage.navigate(StageRoute::Explore(Building::Scriptorium));
    assert!(stage.open_catalog());
    assert!(stage.catalog_open());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::Catalog
    );

    stage.scroll_teaching_down(11);
    assert_eq!(stage.catalog_scroll(), 11);
    assert!(!stage.controller.show_overlay(StageOverlay::Journey {
        call_id: ToolEventId("call-during-catalog".to_string()),
        destination: Building::Smithy,
    }));
    assert!(stage.set_ready_lesson("matrix", "Matrix", "A rectangular array in linear algebra."));
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::Lesson
    );

    assert!(stage.back_overlay());
    assert_eq!(stage.lesson_scroll(), 0);
    assert!(!stage.catalog_open());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldFirstPerson
    );
}

#[test]
fn explicit_media_can_replace_the_catalog_and_retires_its_scroll() {
    let mut stage = Scryglass::default();
    assert!(stage.open_catalog());
    stage.scroll_teaching_bottom();
    stage.reveal_media(4, true);
    assert_eq!(stage.catalog_scroll(), 0);
    assert!(!stage.catalog_open());
    assert!(matches!(
        stage.controller.overlay(),
        Some(StageOverlay::Media { index: 4 })
    ));
}

#[test]
fn operator_media_blocks_lessons_but_can_explicitly_replace_one() {
    let mut stage = Scryglass::default();
    stage.reveal_media(3, true);
    assert!(!stage.set_ready_lesson("matrix", "Matrix", "A rectangular array in linear algebra."));
    assert!(stage.lesson().is_none());
    assert!(matches!(
        stage.controller.overlay(),
        Some(StageOverlay::Media { index: 3 })
    ));

    stage.back_overlay();
    assert!(stage.set_ready_lesson("matrix", "Matrix", "A rectangular array in linear algebra."));
    stage.scroll_lesson_bottom();
    stage.reveal_media(4, true);
    assert_eq!(stage.lesson_scroll(), 0);
    assert!(
        stage.lesson().is_none(),
        "explicit media must retire the lesson object as it replaces the overlay"
    );
    assert!(matches!(
        stage.controller.overlay(),
        Some(StageOverlay::Media { index: 4 })
    ));
}

#[test]
fn a_completed_lesson_can_be_replaced_and_world_reset_clears_it() {
    let mut stage = Scryglass::default();
    assert!(stage.set_ready_lesson("matrix", "Matrix", "A rectangular array in linear algebra."));
    stage.scroll_lesson_down(9);
    assert!(stage.set_ready_lesson(
        "shader",
        "Shader",
        "A program used in computer graphics rendering."
    ));
    assert_eq!(
        stage
            .lesson()
            .map(crate::ui::term::lookup::QuickLookup::term),
        Some("shader")
    );
    assert_eq!(stage.lesson_scroll(), 0);

    stage.scroll_lesson_down(5);
    stage.return_to_world();
    assert_eq!(stage.lesson_scroll(), 0);
    assert!(stage.lesson().is_none());
    assert!(stage.controller.overlay().is_none());
    assert_eq!(stage.controller.route(), StageRoute::Realm);
}

#[test]
fn a_new_local_lesson_replaces_loading_enrichment_without_waiting() {
    let mut stage = Scryglass::default();
    stage.queue_lesson_outcome(crate::ui::term::lookup::TestLookupOutcome::Success {
        title: "Poisson distribution",
        summary: "A discrete probability distribution.",
        source_url: "https://en.wikipedia.org/?curid=24268",
    });
    assert!(stage.begin_lesson("Poisson".to_string()));
    assert!(stage.lesson_loading());

    stage.queue_lesson_outcome(crate::ui::term::lookup::TestLookupOutcome::Success {
        title: "Matrix",
        summary: "A rectangular array in linear algebra.",
        source_url: "https://en.wikipedia.org/?curid=189106",
    });
    assert!(stage.begin_lesson("matrix".to_string()));
    assert_eq!(
        stage
            .lesson()
            .map(crate::ui::term::lookup::QuickLookup::term),
        Some("matrix")
    );
    assert_eq!(stage.lesson_scroll(), 0);
}

#[test]
fn catalog_selection_wraps_and_study_back_returns_to_the_same_shelf() {
    let mut stage = Scryglass::default();
    assert!(stage.open_catalog());
    stage.move_catalog_selection(3);
    let selected = stage.catalog_selection();
    assert_eq!(selected, 3);
    assert!(stage.begin_selected_lesson());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::Lesson
    );
    assert!(stage.back_overlay());
    assert!(stage.catalog_open());
    assert_eq!(stage.catalog_selection(), selected);
    stage.select_catalog_first();
    stage.move_catalog_selection(-1);
    assert_eq!(
        stage.catalog_selection(),
        crate::knowledge::library::CURRICULUM.len() - 1
    );
}

#[test]
fn camera_readout_is_truthful_about_follow_and_free_look() {
    let mut stage = Scryglass::default();
    assert!(stage.camera_readout().starts_with("FOLLOW"));
    stage.adjust_look(0.17, -0.05);
    let readout = stage.camera_readout();
    assert!(readout.starts_with("FREE"));
    assert!(readout.contains("Y+10°"));
    stage.follow();
    assert!(stage.camera_readout().starts_with("FOLLOW"));
}

#[test]
fn loading_lesson_can_be_cancelled_without_reappearing() {
    let mut stage = Scryglass::default();
    stage.queue_lesson_outcome(crate::ui::term::lookup::TestLookupOutcome::Success {
        title: "Poisson distribution",
        summary: "A discrete probability distribution.",
        source_url: "https://en.wikipedia.org/?curid=24268",
    });
    assert!(stage.begin_lesson("Poisson".to_string()));
    assert!(stage.lesson_loading());
    assert!(stage.back_overlay());
    assert!(stage.lesson().is_none());
    assert!(stage.controller.overlay().is_none());

    stage.poll_lesson(true);
    assert!(
        stage.lesson().is_none(),
        "a cancelled worker result must never reopen the world overlay"
    );
}

#[test]
fn rejected_operator_route_arrival_never_arms_a_timer() {
    let _env = crate::tests::env_lock();
    let mut stage = Scryglass::default();
    stage.navigate(StageRoute::Observatory);
    stage.sync_arrival(Some(Building::Keep));
    stage.sync_arrival(Some(Building::Smithy));

    assert_eq!(stage.controller.route(), StageRoute::Observatory);
    assert!(stage.controller.overlay().is_none());
    assert_eq!(stage.arrival(), None);
    assert!(stage.controller.back());
    assert_eq!(stage.controller.route(), StageRoute::Realm);
}

#[test]
fn journey_survives_the_in_transit_none_before_arrival() {
    // The 3D ride path; the overworld map hosts Realm travel by default.
    let _env = crate::tests::env_lock();
    let _ride = crate::tests::TestEnvGuard::set("ANGEL_WORLD_MAP", "3d");
    let mut stage = Scryglass::default();
    stage.sync_arrival(Some(Building::Keep));
    stage.begin_journey(ToolEventId("call-transit".into()), Building::Smithy, false);
    stage.sync_arrival(None);
    assert!(matches!(
        stage.controller.overlay(),
        Some(StageOverlay::Journey {
            destination: Building::Smithy,
            ..
        })
    ));
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldFirstPerson
    );
}

#[test]
fn map_dismisses_an_automatic_journey_to_the_realm() {
    let mut stage = Scryglass::default();
    stage.begin_journey(ToolEventId("call-map".into()), Building::Smithy, false);
    stage.toggle_world_route(Building::Smithy);
    assert_eq!(stage.controller.route(), StageRoute::Realm);
    assert!(stage.controller.overlay().is_none());
    assert_eq!(stage.world_mode(), WorldMode::Map);
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldMap
    );
}

#[test]
fn map_dismisses_journey_from_manual_explore_to_the_realm() {
    let mut stage = Scryglass::default();
    stage.navigate(StageRoute::Explore(Building::Smithy));
    stage.begin_journey(
        ToolEventId("call-map-from-explore".into()),
        Building::Gatehouse,
        false,
    );
    assert_eq!(stage.world_mode(), WorldMode::FirstPerson);

    stage.toggle_world_route(Building::Gatehouse);
    assert_eq!(stage.controller.route(), StageRoute::Realm);
    assert!(stage.controller.overlay().is_none());
    assert_eq!(stage.world_mode(), WorldMode::Map);
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldMap
    );
}

#[test]
fn lifecycle_ceremony_rejects_automatic_travel_but_allows_media() {
    let mut controller = StageController::default();
    assert!(controller.show_overlay(StageOverlay::Lifecycle {
        kind: CeremonyKind::LoopStart,
    }));
    assert!(!controller.show_overlay(StageOverlay::Journey {
        call_id: ToolEventId("call-during-ceremony".to_string()),
        destination: Building::Smithy,
    }));
    assert!(!controller.show_overlay(StageOverlay::Arrival {
        destination: Building::Smithy,
    }));
    assert_eq!(
        controller.resolved_scene(false, false, false),
        StageSurface::Lifecycle
    );

    assert!(controller.show_overlay(StageOverlay::Media { index: 4 }));
    assert_eq!(
        controller.resolved_scene(false, false, false),
        StageSurface::Still(4)
    );
}

#[test]
fn tool_journey_does_not_displace_user_opened_media() {
    let mut stage = Scryglass::default();
    stage.reveal_media(2, true);
    stage.begin_journey(
        ToolEventId("call-during-media".to_string()),
        Building::Smithy,
        false,
    );

    assert_eq!(stage.active_media(), Some(2));
    assert!(matches!(
        stage.controller.overlay(),
        Some(StageOverlay::Media { index: 2 })
    ));
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::Still(2)
    );

    stage.reveal_media(3, true);
    assert_eq!(stage.active_media(), Some(3));
    assert!(matches!(
        stage.controller.overlay(),
        Some(StageOverlay::Media { index: 3 })
    ));
}

#[test]
fn arrival_expires_at_exact_visible_boundary_without_redraw_reset() {
    let _env = crate::tests::env_lock();
    let mut stage = Scryglass::default();
    let started = Instant::now();
    stage.sync_arrival(Some(Building::Keep));
    stage.sync_arrival(Some(Building::Smithy));
    stage.set_stage_visibility(true, true);

    stage.tick_visible(started, false, false);
    stage.tick_visible(started + Duration::from_millis(1_249), false, false);
    assert_eq!(stage.arrival(), Some(Building::Smithy));

    // A normal redraw reports the same arrival and must not create a fresh
    // overlay or reset its visible clock.
    stage.sync_arrival(Some(Building::Smithy));
    stage.tick_visible(started + Duration::from_millis(1_250), false, false);
    assert_eq!(stage.arrival(), None);
}

#[test]
fn arrival_expiry_holds_the_destination_in_first_person_without_an_operator_pin() {
    let _env = crate::tests::env_lock();
    // The 3D ride path; on the overworld map the knight stays on the map.
    let _ride = crate::tests::TestEnvGuard::set("ANGEL_WORLD_MAP", "3d");
    let mut stage = Scryglass::default();
    let started = Instant::now();
    stage.sync_arrival(Some(Building::Keep));
    stage.sync_arrival(Some(Building::Smithy));
    stage.set_stage_visibility(true, true);

    stage.tick_visible(started, false, false);
    stage.tick_visible(started + ARRIVAL_REVEAL, false, false);

    assert_eq!(stage.arrival(), None);
    assert_eq!(
        stage.controller.route(),
        StageRoute::Explore(Building::Smithy)
    );
    assert!(stage.controller.overlay().is_none());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldFirstPerson
    );
}

#[test]
fn arrival_expiry_never_displaces_an_operator_pinned_destination() {
    let _env = crate::tests::env_lock();
    let mut stage = Scryglass::default();
    let started = Instant::now();
    stage.navigate(StageRoute::Explore(Building::Gatehouse));
    stage.sync_arrival(Some(Building::Keep));
    stage.sync_arrival(Some(Building::Smithy));
    stage.set_stage_visibility(true, true);

    stage.tick_visible(started, false, false);
    stage.tick_visible(started + ARRIVAL_REVEAL, false, false);

    assert_eq!(stage.arrival(), None);
    assert_eq!(
        stage.controller.route(),
        StageRoute::Explore(Building::Gatehouse)
    );
    assert!(stage.controller.overlay().is_none());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldFirstPerson
    );
}

#[test]
fn operator_navigation_cancels_arrival_overlay_and_timer_together() {
    let _env = crate::tests::env_lock();
    let mut stage = Scryglass::default();
    stage.sync_arrival(Some(Building::Keep));
    stage.sync_arrival(Some(Building::Smithy));
    assert_eq!(stage.arrival(), Some(Building::Smithy));
    assert!(matches!(
        stage.controller.overlay(),
        Some(StageOverlay::Arrival {
            destination: Building::Smithy
        })
    ));

    stage.navigate(StageRoute::Observatory);
    assert_eq!(stage.arrival(), None);
    assert!(stage.controller.overlay().is_none());
    stage.navigate(StageRoute::Realm);
    stage.set_stage_visibility(true, true);
    assert!(!stage.animating());
}

#[test]
fn hidden_time_does_not_consume_arrival_lifetime() {
    let _env = crate::tests::env_lock();
    let mut stage = Scryglass::default();
    let started = Instant::now();
    stage.sync_arrival(Some(Building::Keep));
    stage.sync_arrival(Some(Building::Gatehouse));
    stage.set_stage_visibility(true, true);
    stage.tick_visible(started, false, false);
    stage.tick_visible(started + Duration::from_millis(700), false, false);

    stage.set_stage_visibility(false, false);
    stage.tick_visible(started + Duration::from_secs(30), false, false);
    assert_eq!(stage.arrival(), Some(Building::Gatehouse));

    stage.set_stage_visibility(true, true);
    stage.tick_visible(started + Duration::from_secs(30), false, false);
    stage.tick_visible(
        started + Duration::from_secs(30) + Duration::from_millis(549),
        false,
        false,
    );
    assert_eq!(stage.arrival(), Some(Building::Gatehouse));
    stage.tick_visible(
        started + Duration::from_secs(30) + Duration::from_millis(550),
        false,
        false,
    );
    assert_eq!(stage.arrival(), None);
}

#[cfg(feature = "scryglass-video")]
#[test]
fn contain_fit_preserves_wide_and_tall_video_aspect() {
    assert_eq!(contain_video_cells(1920, 1080, 80, 20), (71, 20));
    assert_eq!(contain_video_cells(1080, 1920, 80, 20), (22, 20));
}

#[cfg(feature = "scryglass-video")]
#[test]
fn eof_is_a_parked_state_that_restart_can_resume() {
    let started = Instant::now();
    let mut playback = VideoPlayback::new(Some(Duration::from_secs(10)), started);
    playback.frame_published(Duration::from_secs(9));
    playback.mark_ended();

    assert!(playback.ended);
    assert!(playback.paused);
    assert!(!playback.is_playing());
    assert_eq!(playback.position, Duration::from_secs(10));

    let restarted = started + Duration::from_secs(30);
    playback.seek_succeeded(Duration::ZERO, true, restarted);
    assert!(!playback.ended);
    assert!(!playback.paused);
    assert!(playback.is_playing());
    assert_eq!(playback.position, Duration::ZERO);
    assert_eq!(
        playback.frame_deadline(Duration::from_millis(250)),
        restarted + Duration::from_millis(250)
    );
}

#[cfg(feature = "scryglass-video")]
#[test]
fn hidden_and_paused_time_are_removed_from_the_playback_anchor() {
    let started = Instant::now();
    let mut playback = VideoPlayback::new(Some(Duration::from_secs(20)), started);
    playback.frame_published(Duration::from_secs(2));

    playback.set_visible(false, started + Duration::from_secs(2));
    let shown_again = started + Duration::from_secs(62);
    playback.set_visible(true, shown_again);
    assert_eq!(
        playback.frame_deadline(Duration::from_millis(2_500)),
        shown_again + Duration::from_millis(500)
    );

    playback.frame_published(Duration::from_millis(2_500));
    playback.toggle(shown_again + Duration::from_millis(500));
    assert!(playback.paused);
    let resumed = started + Duration::from_secs(122);
    playback.toggle(resumed);
    assert!(!playback.paused);
    assert_eq!(
        playback.frame_deadline(Duration::from_secs(3)),
        resumed + Duration::from_millis(500)
    );
}

#[cfg(feature = "scryglass-video")]
#[test]
fn reduced_and_still_motion_sampling_keeps_source_time_pacing() {
    assert_eq!(video_sample_every(30.0, 12), 3);
    assert_eq!(video_sample_every(30.0, 6), 5);
    assert_eq!(video_sample_every(30.0, 1), 30);

    let started = Instant::now();
    let playback = VideoPlayback::new(None, started);
    assert_eq!(
        playback.frame_deadline(Duration::from_secs(1)),
        started + Duration::from_secs(1),
        "a 1 fps sampled frame must not be released after a short fixed sleep"
    );
}

#[cfg(feature = "scryglass-video")]
#[test]
fn seeks_clamp_to_duration_and_a_successful_seek_recovers_a_fault() {
    let started = Instant::now();
    let mut playback = VideoPlayback::new(Some(Duration::from_secs(10)), started);
    playback.frame_published(Duration::from_secs(8));
    assert_eq!(playback.seek_target(5), Duration::from_secs(10));
    assert_eq!(playback.seek_target(-20), Duration::ZERO);

    playback.mark_faulted();
    assert!(playback.faulted);
    playback.seek_succeeded(
        Duration::from_secs(4),
        false,
        started + Duration::from_secs(1),
    );
    assert!(!playback.faulted);
    assert!(!playback.ended);
    assert!(playback.paused, "an ordinary seek preserves pause state");
    assert_eq!(playback.position, Duration::from_secs(4));
}

#[cfg(feature = "scryglass-video")]
#[test]
fn restarting_cancels_the_end_of_reveal_countdown() {
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    let reveal = stage.reveal.as_mut().unwrap();
    reveal.state = RevealState::Showing;
    reveal.ending = true;
    reveal.visible_for = Duration::from_secs(1);

    stage.restart_video();

    let reveal = stage.reveal.as_ref().unwrap();
    assert!(!reveal.ending);
    assert_eq!(reveal.visible_for, Duration::ZERO);
    assert_eq!(reveal.last_visible_at, None);
}

#[cfg(feature = "scryglass-video")]
#[test]
fn explicit_video_recovery_unlatches_a_reveal_fault() {
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    stage.note_media_fault("seek failed");
    assert_eq!(stage.reveal.as_ref().unwrap().state, RevealState::Fault);

    stage.seek_video(-5);

    let reveal = stage.reveal.as_ref().unwrap();
    assert_eq!(reveal.state, RevealState::Loading);
    assert_eq!(reveal.error, None);
    assert_eq!(reveal.visible_for, Duration::ZERO);
}

#[cfg(feature = "scryglass-video")]
#[test]
fn show_work_video_rejects_local_network_playlist_before_opening_reference() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let path = std::env::temp_dir().join(format!(
        "angel-show-work-playlist-{}.mp4",
        std::process::id()
    ));
    std::fs::write(
        &path,
        format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nhttp://{}/secret.ts\n#EXT-X-ENDLIST\n",
            listener.local_addr().unwrap()
        ),
    )
    .unwrap();
    let result = dotmax::media::VideoPlayer::new_local(&path);
    assert!(
        result.is_err(),
        "local playlist must be rejected before network references are opened"
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    std::fs::remove_file(path).unwrap();
}

#[cfg(feature = "scryglass-video")]
#[test]
fn local_mp4_worker_decodes_and_restarts_after_eof_when_ffmpeg_is_available() {
    let _guard = crate::tests::env_lock();
    let path =
        std::env::temp_dir().join(format!("angel-scryglass-smoke-{}.mp4", std::process::id()));
    let generated = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x22cc88:s=16x16:d=0.6:r=4",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&path)
        .status();
    if !generated.is_ok_and(|status| status.success()) {
        return;
    }
    let media = Media::Video {
        label: "smoke reel".to_string(),
        path: path.display().to_string(),
    };
    // Exercise the same visibility lifecycle as every actual compositor
    // frame. Repeated layout resets must not restart the video clock.
    let displayed_frame = |stage: &mut Scryglass| {
        stage.begin_frame();
        stage.set_stage_visibility(true, true);
        let frame = stage.video_frame(&media, 24, 10, 12, true);
        stage.finish_frame();
        frame
    };
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    let mut decoded = false;
    for _ in 0..100 {
        match displayed_frame(&mut stage) {
            Ok((Some(frame), _, _, _, _)) => {
                decoded = frame.rgba.width() > 0 && frame.rgba.height() > 0;
                break;
            }
            Ok(_) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => panic!("video worker failed: {error}"),
        }
    }
    assert!(decoded, "worker never published decoded RGBA pixels");

    let mut reached_eof = false;
    for _ in 0..150 {
        match displayed_frame(&mut stage) {
            Ok((_, _, _, _, true)) => {
                reached_eof = true;
                break;
            }
            Ok(_) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => panic!("video worker failed before EOF: {error}"),
        }
    }
    assert!(reached_eof, "worker never published EOF state");

    stage.restart_video();
    let mut restarted = false;
    for _ in 0..150 {
        match displayed_frame(&mut stage) {
            Ok((Some(_), position, _, _, false)) if position > Duration::ZERO => {
                restarted = true;
                break;
            }
            Ok(_) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => panic!("video worker failed to restart: {error}"),
        }
    }
    let _ = std::fs::remove_file(path);
    assert!(restarted, "EOF worker did not decode again after restart");
}

#[cfg(feature = "scryglass-video")]
#[test]
fn native_video_pixels_seek_resize_hide_reopen_and_release_the_decoder() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-native-video-{}-{}",
        std::process::id(),
        NEXT_MEDIA_REQUEST.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("red-to-green.mp4");
    let generated = std::process::Command::new("ffmpeg").args([
            "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i",
            "color=c=red:s=32x24:r=8:d=1[r];color=c=lime:s=32x24:r=8:d=2[g];[r][g]concat=n=2:v=1:a=0",
            "-r", "8", "-pix_fmt", "yuv420p", "-y",
        ]).arg(&path).status();
    if !generated.is_ok_and(|status| status.success()) {
        eprintln!("SKIP native video fixture: ffmpeg CLI unavailable");
        return;
    }
    let media = Media::Video {
        label: "controlled red-to-green".into(),
        path: path.display().to_string(),
    };
    let mut stage = Scryglass::default();
    stage.reveal_media(0, true);
    let poll = |stage: &mut Scryglass, width, height| {
        stage.begin_frame();
        stage.set_stage_visibility(true, true);
        let result = stage.video_frame(&media, width, height, 8, false).unwrap();
        stage.finish_frame();
        result
    };
    let wait_frame = |stage: &mut Scryglass, width, height| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = poll(stage, width, height);
            if let Some(pixels) = result.0 {
                break (pixels, result.1, result.3);
            }
            assert!(Instant::now() < deadline, "native pixels did not arrive");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let (red, position, paused) = wait_frame(&mut stage, 40, 12);
    assert!(paused, "motion-off must publish one frame then pause");
    assert_eq!(position, Duration::ZERO);
    assert!(
        red.rgba
            .pixels()
            .all(|p| p[0] > 200 && p[1] < 30 && p[2] < 30)
    );
    assert_eq!(red.identity.request_id, stage.media_request_id());

    stage.seek_video(1);
    assert!(
        stage.video.snapshot().frame.is_none(),
        "seek invalidates old pixels synchronously"
    );
    let (green, position, paused) = wait_frame(&mut stage, 40, 12);
    assert!(paused, "paused seek must not start playback");
    assert!(position >= Duration::from_secs(1));
    assert_ne!(red.identity.generation, green.identity.generation);
    assert!(
        green
            .rgba
            .pixels()
            .all(|p| p[1] > 200 && p[0] < 30 && p[2] < 30)
    );

    let (resized, resized_position, _) = wait_frame(&mut stage, 80, 30);
    assert_eq!(
        resized.identity, green.identity,
        "resize must retain the decoder generation"
    );
    assert_eq!(
        resized_position, position,
        "resizing must not reset or advance a paused clock"
    );

    stage.toggle_video();
    let deadline = Instant::now() + Duration::from_secs(3);
    while poll(&mut stage, 80, 30).1 <= position {
        assert!(Instant::now() < deadline, "resized decoder did not resume");
        std::thread::sleep(Duration::from_millis(5));
    }
    stage.begin_frame();
    stage.set_stage_visibility(false, false);
    stage.finish_frame();
    // Barrier: wait for the same worker to consume the visibility command.
    std::thread::sleep(Duration::from_millis(30));
    let hidden = stage.video.snapshot();
    assert!(
        !hidden.paused,
        "visibility pauses scheduling without changing play intent"
    );
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(stage.video.snapshot().position, hidden.position);
    assert_eq!(
        stage.video.snapshot().frame.unwrap().sequence,
        hidden.frame.unwrap().sequence
    );

    stage.reveal_media(0, true);
    assert!(
        stage.video.active.is_none(),
        "reopen stops the previous decoder"
    );
    let (reopened, position, _) = wait_frame(&mut stage, 40, 12);
    assert_ne!(reopened.identity.request_id, green.identity.request_id);
    assert_ne!(reopened.identity.generation, green.identity.generation);
    assert_eq!(position, Duration::ZERO);
    let active = stage.video.active.as_ref().unwrap();
    let cancelled = Arc::clone(&active.cancelled);
    // Closing must bypass an accumulated control backlog, not wait for
    // hundreds of scaler resizes before reaching the queued Stop message.
    for index in 0..1_000 {
        active
            .tx
            .send(VideoCommand::Resize(40 + index % 2, 12))
            .unwrap();
    }
    stage.return_to_world();
    assert!(cancelled.load(Ordering::Acquire));
    assert!(stage.video.active.is_none());
    let deadline = Instant::now() + Duration::from_secs(2);
    while VIDEO_RENDER_IN_FLIGHT.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        !VIDEO_RENDER_IN_FLIGHT.load(Ordering::Acquire),
        "close must release the single decoder lease"
    );
    let bounded = contain_video_cells(16_384, 16_384, usize::MAX, usize::MAX);
    assert!(bounded.0 * bounded.1 * 8 <= 98_304);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[cfg(feature = "scryglass-video")]
#[test]
fn native_video_old_generation_cannot_publish_state_or_fault_after_seek() {
    let mailbox = Arc::new(Mutex::new(VideoSnapshot {
        generation: 2,
        position: Duration::from_secs(8),
        ..VideoSnapshot::default()
    }));
    let mut playback = VideoPlayback::new(Some(Duration::from_secs(10)), Instant::now());
    publish_video_state(&mailbox, &playback, true, 1);
    publish_video_error(&mailbox, &mut playback, "old decode failed", 1);
    let snapshot = mailbox.lock().unwrap();
    assert_eq!(snapshot.position, Duration::from_secs(8));
    assert_eq!(snapshot.error, None);
    drop(snapshot);
    publish_video_error(&mailbox, &mut playback, "current decode failed", 2);
    assert_eq!(
        mailbox.lock().unwrap().error.as_deref(),
        Some("current decode failed")
    );
}

/// A mid-journey world whose ride frames feed the stage paint during turns.
fn riding_world() -> crate::stage::world_viz::World {
    let mut world = crate::stage::world_viz::World::new(2024);
    world.note_tool_call("write_file", "forging a plate");
    for _ in 0..8 {
        world.tick();
    }
    world
}

/// (distinct fg colors, SGR fg runs, estimated flush bytes) for a full
/// repaint of `image`, mirroring what `paint_image` writes and how the
/// crossterm backend emits it: one ~19-byte truecolor SGR per fg change
/// along the row-major cell walk (fg state survives MoveTo), 3 UTF-8
/// bytes per braille glyph, ~8 bytes of MoveTo per row.
fn ink_stats(
    image: &ColoredBrailleImage,
    quant: impl Fn([u8; 3]) -> [u8; 3],
) -> (usize, usize, usize) {
    let mut distinct = std::collections::HashSet::new();
    let mut runs = 0usize;
    let mut last: Option<[u8; 3]> = None;
    for cell in &image.cells {
        let fg = quant(cell.fg);
        distinct.insert(fg);
        if last != Some(fg) {
            runs += 1;
            last = Some(fg);
        }
    }
    let bytes = runs * 19 + image.cells.len() * 3 + image.height * 8;
    (distinct.len(), runs, bytes)
}

/// (changed cells, SGR headers, estimated flush bytes) for the diff walk
/// between two consecutive painted frames — the steady-state cost while
/// the ride animates. MoveTo (~8B) per gap in the changed-cell sequence,
/// one truecolor header (~19B) per fg change along it.
fn diff_stats(
    a: &ColoredBrailleImage,
    b: &ColoredBrailleImage,
    quant: impl Fn([u8; 3]) -> [u8; 3],
) -> (usize, usize, usize) {
    let mut changed = 0usize;
    let mut headers = 0usize;
    let mut bytes = 0usize;
    let mut last_fg: Option<[u8; 3]> = None;
    let mut last_index: Option<usize> = None;
    for (index, (ca, cb)) in a.cells.iter().zip(&b.cells).enumerate() {
        let fg = quant(cb.fg);
        if ca.glyph == cb.glyph && quant(ca.fg) == fg {
            continue;
        }
        changed += 1;
        if last_index != Some(index.wrapping_sub(1)) {
            bytes += 8;
        }
        if last_fg != Some(fg) {
            headers += 1;
            last_fg = Some(fg);
        }
        bytes += 3;
        last_index = Some(index);
    }
    (changed, headers, bytes + headers * 19)
}

#[test]
fn world_ink_quantizer_is_gentle_and_idempotent() {
    let _guard = crate::tests::env_lock();
    // Default RGB444: max channel error 8/255.
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
    for channel in 0..=255u8 {
        let [q, _, _] = quantize_world_ink([channel; 3]);
        assert!(
            u8::abs_diff(q, channel) <= 8,
            "channel {channel} moved to {q}; RGB444 rounding must stay within 8",
        );
        assert_eq!(
            quantize_world_ink([q; 3])[0],
            q,
            "painted values must re-quantize to themselves",
        );
    }
    // Opt-back RGB555: max channel error 4/255.
    let _rgb555 = crate::tests::TestEnvGuard::set("ANGEL_WORLD_INK", "rgb555");
    for channel in 0..=255u8 {
        let [q, _, _] = quantize_world_ink([channel; 3]);
        assert!(
            u8::abs_diff(q, channel) <= 4,
            "channel {channel} moved to {q}; RGB555 rounding must stay within 4",
        );
    }
}

/// SGR-flood measurement: the terminal-parse cost of a genuinely new ride
/// frame is set by the ink's run structure, not the changed-cell count
/// alone. Guards the run structure the world paint relies on; prints the
/// tallies under --nocapture for perf work.
///
/// Z5 pinned this to the mesh ride, which is the frame it has always
/// measured and whose floor it states. The Dotmax world — the
/// launch default — is measured beside it in
/// [`dotmax_frame_ink_runs_are_measured_beside_the_ride`], which holds
/// the monotonicity claims and reports the run structure without
/// restating a floor the tile dither does not meet.
#[test]
fn ride_frame_ink_runs_bound_the_sgr_flood() {
    let _guard = crate::tests::env_lock();
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
    let _pin = crate::stage::world_viz::world3d::pin_world3d();
    let mut world = riding_world();
    let first = world
        .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
        .expect("ride frame");
    for _ in 0..3 {
        world.tick();
    }
    let second = world
        .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
        .expect("ride frame");
    assert_ne!(first, second, "ticked world must render a new frame");

    let inked = first
        .cells
        .iter()
        .filter(|cell| cell.glyph != '\u{2800}')
        .count();
    assert!(inked > 400, "ride frame should carry substantial ink");

    let raw = ink_stats(&first, |fg| fg);
    let q4 = ink_stats(&first, |fg| {
        // Force the RGB444 math independent of env so this bench is stable.
        fg.map(|channel| {
            let level = (u16::from(channel) * 15 + 127) / 255;
            ((level * 255 + 7) / 15) as u8
        })
    });
    let q5 = ink_stats(&first, |fg| {
        fg.map(|channel| {
            let level = (u16::from(channel) * 31 + 127) / 255;
            ((level * 255 + 15) / 31) as u8
        })
    });
    let raw_diff = diff_stats(&first, &second, |fg| fg);
    let q4_diff = diff_stats(&first, &second, |fg| {
        fg.map(|channel| {
            let level = (u16::from(channel) * 15 + 127) / 255;
            ((level * 255 + 7) / 15) as u8
        })
    });
    let q5_diff = diff_stats(&first, &second, |fg| {
        fg.map(|channel| {
            let level = (u16::from(channel) * 31 + 127) / 255;
            ((level * 255 + 15) / 31) as u8
        })
    });
    println!(
        "stage ink: {}x{} cells={} inked={}",
        first.width,
        first.height,
        first.cells.len(),
        inked,
    );
    for (label, (distinct, runs, bytes), (changed, headers, diff_bytes)) in [
        ("raw", raw, raw_diff),
        ("rgb555", q5, q5_diff),
        ("rgb444", q4, q4_diff),
    ] {
        println!(
            "  {label}: distinct={distinct} sgr-runs={runs} mean-run={:.2} \
                 full-repaint≈{bytes}B | next-frame changed={changed} headers={headers} ≈{diff_bytes}B",
            first.cells.len() as f32 / runs.max(1) as f32,
        );
    }
    assert!(
        q4.0 <= q5.0 && q4.1 <= q5.1 && q4_diff.2 <= q5_diff.2,
        "RGB444 must never add colors, runs, or flush bytes vs RGB555",
    );
    assert!(
        q5.0 <= raw.0 && q5.1 <= raw.1 && q5_diff.2 <= raw_diff.2,
        "quantizing must never add colors, runs, or flush bytes",
    );
    // Floor the default-on RGB444 paint must hold: quantized ink keeps
    // same-color runs long enough that a full repaint stays well under
    // one truecolor header per cell.
    assert!(
        q4.1 * 2 < first.cells.len(),
        "quantized fg runs ({}) approach one-per-cell; the SGR flood is back",
        q4.1,
    );
}

/// The same measurement for the view Z5 made the launch default.
///
/// The Dotmax world paints a true-3D scene through the
/// braille bridge, and the two-value rock / sparse-mark grounds alternate
/// colour from cell to cell by design — so its mean run is near 1 and it
/// does **not** meet the ride's one-header-per-two-cells floor. That is a
/// real cost of the flip, recorded here rather than hidden by relaxing the
/// ride's law: the monotonicity claims (quantizing never *adds* colours,
/// runs or flush bytes) still hold and are asserted, and the tallies print
/// under --nocapture so a later pass can attack the run structure.
#[test]
fn dotmax_frame_ink_runs_are_measured_beside_the_ride() {
    let _guard = crate::tests::env_lock();
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
    let _pin =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = riding_world();
    let first = world
        .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
        .expect("Dotmax frame");
    for _ in 0..3 {
        world.tick();
    }
    let second = world
        .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
        .expect("Dotmax frame");

    let quant4 = |fg: [u8; 3]| {
        fg.map(|channel| {
            let level = (u16::from(channel) * 15 + 127) / 255;
            ((level * 255 + 7) / 15) as u8
        })
    };
    let quant5 = |fg: [u8; 3]| {
        fg.map(|channel| {
            let level = (u16::from(channel) * 31 + 127) / 255;
            ((level * 255 + 15) / 31) as u8
        })
    };
    let raw = ink_stats(&first, |fg| fg);
    let q5 = ink_stats(&first, quant5);
    let q4 = ink_stats(&first, quant4);
    let raw_diff = diff_stats(&first, &second, |fg| fg);
    let q5_diff = diff_stats(&first, &second, quant5);
    let q4_diff = diff_stats(&first, &second, quant4);
    println!(
        "Dotmax ink: {}x{} cells={} raw-runs={} rgb555-runs={} rgb444-runs={} \
             mean-run={:.2}",
        first.width,
        first.height,
        first.cells.len(),
        raw.1,
        q5.1,
        q4.1,
        first.cells.len() as f32 / q4.1.max(1) as f32,
    );
    assert!(
        q4.0 <= q5.0 && q4.1 <= q5.1 && q4_diff.2 <= q5_diff.2,
        "RGB444 must never add colors, runs, or flush bytes vs RGB555",
    );
    assert!(
        q5.0 <= raw.0 && q5.1 <= raw.1 && q5_diff.2 <= raw_diff.2,
        "quantizing must never add colors, runs, or flush bytes",
    );
}

#[test]
fn arrival_expiry_on_the_overworld_map_keeps_the_realm() {
    let _env = crate::tests::env_lock();
    let _map = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_MAP");
    let mut stage = Scryglass::default();
    let started = Instant::now();
    stage.sync_arrival(Some(Building::Keep));
    stage.sync_arrival(Some(Building::Smithy));
    stage.set_stage_visibility(true, true);
    stage.tick_visible(started, false, false);
    stage.tick_visible(started + ARRIVAL_REVEAL, false, false);
    assert_eq!(stage.arrival(), None);
    assert_eq!(stage.controller.route(), StageRoute::Realm);
    assert!(stage.controller.overlay().is_none());
    assert_eq!(
        stage.controller.resolved_scene(false, false, false),
        StageSurface::WorldMap
    );
}

#[test]
fn a_session_opens_on_the_overworld_map() {
    let _env = crate::tests::env_lock();
    {
        let _map = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_MAP");
        let stage = Scryglass::for_session(Building::Smithy);
        assert_eq!(stage.controller.route(), StageRoute::Realm);
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldMap
        );
    }
    let _ride = crate::tests::TestEnvGuard::set("ANGEL_WORLD_MAP", "3d");
    let stage = Scryglass::for_session(Building::Smithy);
    assert_eq!(
        stage.controller.route(),
        StageRoute::Explore(Building::Smithy)
    );
}
