use super::*;

/// One region rule per row: (kind, status, danger, measured, submissions,
/// verifying) → expected region. Every rule from the Z1 spec has a row.
#[test]
fn region_for_table() {
    let rows: Vec<(LoopKind, LoopStatus, u8, usize, usize, bool, Region)> = vec![
        // idle / no loop → CastleTown (also finished/parked loops).
        (
            LoopKind::Competition,
            LoopStatus::Idle,
            0,
            0,
            0,
            false,
            Region::CastleTown,
        ),
        (
            LoopKind::Research,
            LoopStatus::Paused,
            3,
            5,
            0,
            false,
            Region::CastleTown,
        ),
        (
            LoopKind::Coding,
            LoopStatus::Done,
            0,
            0,
            1,
            false,
            Region::CastleTown,
        ),
        // LoopStarted → region by kind.
        (
            LoopKind::Competition,
            LoopStatus::Running,
            0,
            0,
            0,
            false,
            Region::TheMines,
        ),
        (
            LoopKind::Coding,
            LoopStatus::Running,
            0,
            0,
            0,
            false,
            Region::TheMines,
        ),
        (
            LoopKind::Research,
            LoopStatus::Running,
            0,
            0,
            0,
            false,
            Region::DarkForest,
        ),
        (
            LoopKind::Unknown,
            LoopStatus::Running,
            0,
            0,
            0,
            false,
            Region::DarkForest,
        ),
        (
            LoopKind::Unknown,
            LoopStatus::Baselining,
            0,
            0,
            0,
            false,
            Region::DarkForest,
        ),
        // An unknown loop that measures candidates is on the measured path.
        (
            LoopKind::Unknown,
            LoopStatus::Running,
            0,
            1,
            0,
            false,
            Region::TheMines,
        ),
        // Stall level ≥ 2 → Swamp from any loop region; below stays home.
        (
            LoopKind::Competition,
            LoopStatus::Running,
            1,
            0,
            0,
            false,
            Region::TheMines,
        ),
        (
            LoopKind::Competition,
            LoopStatus::Running,
            2,
            0,
            0,
            false,
            Region::Swamp,
        ),
        (
            LoopKind::Research,
            LoopStatus::Running,
            3,
            2,
            0,
            false,
            Region::Swamp,
        ),
        // AwaitingApproval forces danger ≥ 2.
        (
            LoopKind::Coding,
            LoopStatus::AwaitingApproval,
            0,
            0,
            0,
            false,
            Region::Swamp,
        ),
        // Submitted or Verifying → DragonKeep.
        (
            LoopKind::Competition,
            LoopStatus::Running,
            0,
            3,
            1,
            false,
            Region::DragonKeep,
        ),
        (
            LoopKind::Research,
            LoopStatus::Verifying,
            0,
            0,
            0,
            false,
            Region::DragonKeep,
        ),
        (
            LoopKind::Coding,
            LoopStatus::Running,
            0,
            0,
            0,
            true,
            Region::DragonKeep,
        ),
        // The judge the loop is waiting on outranks the swamp.
        (
            LoopKind::Competition,
            LoopStatus::Verifying,
            3,
            0,
            0,
            false,
            Region::DragonKeep,
        ),
        // The danger ladder itself.
        (
            LoopKind::Competition,
            LoopStatus::Running,
            3,
            0,
            0,
            false,
            Region::Swamp,
        ),
    ];
    for (i, (kind, status, level, measured, submissions, verifying, expected)) in
        rows.into_iter().enumerate()
    {
        let got = region_for(
            kind,
            &status,
            Danger::new(level),
            measured,
            submissions,
            verifying,
        );
        assert_eq!(got, expected, "row {i}: {kind:?} {status:?} danger {level}");
    }
}

#[test]
fn danger_from_stall_follows_the_formula() {
    // stale * 3 / pivot.max(1), clamped 0..=3.
    assert_eq!(Danger::from_stall(0, 2).level(), 0);
    assert_eq!(Danger::from_stall(1, 3).level(), 1);
    assert_eq!(Danger::from_stall(2, 3).level(), 2);
    assert_eq!(Danger::from_stall(3, 3).level(), 3);
    assert_eq!(Danger::from_stall(9, 3).level(), 3, "clamped at 3");
    assert_eq!(Danger::from_stall(2, 0).level(), 3, "pivot 0 → max(1)");
    assert_eq!(Danger::from_stall(0, 0).level(), 0);
}

#[test]
fn loop_kind_for_table() {
    let empty = ToolMix::default();
    let rows: Vec<(&str, bool, LoopKind)> = vec![
        // podrace wins outright.
        ("anything at all", true, LoopKind::Competition),
        // Competition vocabulary.
        ("win the kernel benchmark", false, LoopKind::Competition),
        ("check the leaderboard", false, LoopKind::Competition),
        ("optimize the qr kernel", false, LoopKind::Competition),
        ("optimise and submit", false, LoopKind::Competition),
        ("make it faster", false, LoopKind::Competition),
        ("land a submission", false, LoopKind::Competition),
        // Research vocabulary.
        (
            "research the failing over pattern",
            false,
            LoopKind::Research,
        ),
        ("survey the literature", false, LoopKind::Research),
        ("read the docs", false, LoopKind::Research),
        ("explore the repo", false, LoopKind::Research),
        ("find out why it flakes", false, LoopKind::Research),
        ("why does it hang", false, LoopKind::Research),
        // Coding vocabulary.
        ("fix the login bug", false, LoopKind::Coding),
        ("implement the parser", false, LoopKind::Coding),
        ("refactor the module", false, LoopKind::Coding),
        ("build the dashboard", false, LoopKind::Coding),
        ("test the harness", false, LoopKind::Coding),
        ("write the docs", false, LoopKind::Coding),
        // Unknown otherwise.
        ("think about it", false, LoopKind::Unknown),
        ("", false, LoopKind::Unknown),
    ];
    for (i, (task, podrace, expected)) in rows.into_iter().enumerate() {
        let got = loop_kind_for(task, podrace, &empty);
        assert_eq!(got, expected, "row {i}: {task:?}");
    }
}

#[test]
fn tool_mix_promotes_unknown_but_never_demotes() {
    let mut mix = ToolMix::default();
    // 7 of the last 12 calls forge work → Competition by strict majority.
    for _ in 0..7 {
        mix.push(
            Building::Smithy,
            crate::stage::world_viz::RealmActivity::Forge,
        );
    }
    for _ in 0..5 {
        mix.push(
            Building::Scriptorium,
            crate::stage::world_viz::RealmActivity::Study,
        );
    }
    assert_eq!(
        loop_kind_for("something vague", false, &mix),
        LoopKind::Competition
    );

    // Research majority.
    let mut mix = ToolMix::default();
    for _ in 0..7 {
        mix.push(
            Building::Scriptorium,
            crate::stage::world_viz::RealmActivity::Study,
        );
    }
    for _ in 0..5 {
        mix.push(
            Building::Chapel,
            crate::stage::world_viz::RealmActivity::Memory,
        );
    }
    assert_eq!(
        loop_kind_for("something vague", false, &mix),
        LoopKind::Research
    );

    // A balanced window stays Unknown.
    let mut mix = ToolMix::default();
    for _ in 0..6 {
        mix.push(
            Building::Smithy,
            crate::stage::world_viz::RealmActivity::Forge,
        );
    }
    for _ in 0..6 {
        mix.push(
            Building::Scriptorium,
            crate::stage::world_viz::RealmActivity::Study,
        );
    }
    assert_eq!(
        loop_kind_for("something vague", false, &mix),
        LoopKind::Unknown
    );

    // The mix can never demote a named kind.
    let mut mix = ToolMix::default();
    for _ in 0..12 {
        mix.push(
            Building::Scriptorium,
            crate::stage::world_viz::RealmActivity::Study,
        );
    }
    assert_eq!(loop_kind_for("fix the bug", false, &mix), LoopKind::Coding);

    // The window really rolls: 12 research calls then 12 forge calls ends
    // up all-forge.
    let mut mix = ToolMix::default();
    for _ in 0..12 {
        mix.push(
            Building::Scriptorium,
            crate::stage::world_viz::RealmActivity::Study,
        );
    }
    for _ in 0..12 {
        mix.push(
            Building::Smithy,
            crate::stage::world_viz::RealmActivity::Forge,
        );
    }
    assert_eq!(loop_kind_for("vague", false, &mix), LoopKind::Competition);
}

/// The scripted competition loop from the spec: start → 3 iterations → stall
/// to level 2 → Swamp → unstall → Mines → Measured improved → Submitted →
/// DragonKeep → accepted → LoopFinished ok → Homecoming → 120 ticks →
/// CastleTown.
#[test]
fn scripted_competition_loop_walks_the_map() {
    let mut q = Quest::idle();
    assert_eq!(q.region(), Region::CastleTown);
    let mut tick = 0u64;

    let start = || AdventureEvent::LoopStarted {
        kind: LoopKind::Competition,
        task: "win the podrace".to_string(),
    };
    q.apply(start(), tick);
    assert_eq!(q.region(), Region::TheMines, "competition → the mines");
    assert_eq!(q.region().label(), "The Mines");

    for n in 1..=3 {
        tick += 1;
        q.apply(AdventureEvent::Iteration { n }, tick);
    }
    assert_eq!(q.iteration(), 3);

    // Stalling into the swamp.
    q.apply(AdventureEvent::Stall { level: 2 }, tick);
    assert_eq!(q.region(), Region::Swamp);
    assert_eq!(q.danger().level(), 2);

    // Unstalling returns to the kind's home region.
    q.apply(AdventureEvent::Stall { level: 0 }, tick);
    assert_eq!(q.region(), Region::TheMines);

    // A measured candidate that improved is a treasure + banner.
    tick += 1;
    q.apply(AdventureEvent::Measured { improved: true }, tick);
    assert_eq!(q.treasures(), 1);
    assert_eq!(q.banner(), Some("treasure!"));
    // A regression is only an empty chest — no danger, no lost loot.
    q.apply(AdventureEvent::Measured { improved: false }, tick);
    assert_eq!(q.treasures(), 1);
    assert_eq!(q.empty_chests(), 1);
    assert_eq!(q.danger().level(), 0);
    assert_eq!(q.banner(), Some("empty chest"));

    // The banner expires after 40 ticks.
    q.observe_tick(tick + BANNER_TICKS);
    assert_eq!(q.banner(), None);

    // Submitting walks into the Dragon Keep.
    tick += 1;
    q.apply(AdventureEvent::Submitted, tick);
    assert_eq!(q.region(), Region::DragonKeep);
    // An accepted verdict raises the banner and holds the keep.
    q.apply(AdventureEvent::SubmissionSettled { accepted: true }, tick);
    assert_eq!(q.region(), Region::DragonKeep);
    assert_eq!(q.banner(), Some("banner raised"));
    // A rejected verdict walks home with no penalty.
    q.apply(AdventureEvent::SubmissionSettled { accepted: false }, tick);
    assert_eq!(q.region(), Region::TheMines);

    // Re-submit and finish the run for the homecoming walk.
    q.apply(AdventureEvent::Submitted, tick);
    assert_eq!(q.region(), Region::DragonKeep);
    tick += 1;
    q.apply(AdventureEvent::LoopFinished { ok: true }, tick);
    assert_eq!(q.region(), Region::Homecoming);
    assert_eq!(q.banner(), Some("homecoming"));
    assert!(
        q.banner_text().unwrap().contains("1 treasure"),
        "{:?}",
        q.banner_text()
    );
    for t in tick + 1..tick + HOMECOMING_TICKS {
        q.observe_tick(t);
        assert_eq!(q.region(), Region::Homecoming, "still walking home at {t}");
    }
    q.observe_tick(tick + HOMECOMING_TICKS);
    assert_eq!(q.region(), Region::CastleTown);

    // A failed run retreats straight to town under the retreat banner.
    let mut q = Quest::idle();
    q.apply(start(), 0);
    q.apply(AdventureEvent::LoopFinished { ok: false }, 1);
    assert_eq!(q.region(), Region::CastleTown);
    assert_eq!(q.banner(), Some("retreat"));
}

#[test]
fn quest_transitions_are_deterministic() {
    let script = |mut q: Quest| {
        for (tick, ev) in [
            AdventureEvent::LoopStarted {
                kind: LoopKind::Research,
                task: "explore".to_string(),
            },
            AdventureEvent::Iteration { n: 1 },
            AdventureEvent::Party { size: 4 },
            AdventureEvent::Stall { level: 3 },
            AdventureEvent::Measured { improved: true },
            AdventureEvent::Pivot,
            AdventureEvent::Submitted,
            AdventureEvent::SubmissionSettled { accepted: false },
            AdventureEvent::LoopFinished { ok: true },
        ]
        .into_iter()
        .enumerate()
        {
            q.apply(ev, tick as u64 * 7);
        }
        q
    };
    let a = script(Quest::idle());
    let b = script(Quest::idle());
    assert_eq!(a, b, "two identical runs must be identical");
    assert_eq!(a.region(), Region::Homecoming);
    assert_eq!(a.party(), 4);
    assert_eq!(a.treasures(), 1);
}

#[test]
fn party_clamps_into_the_formation_band() {
    let mut q = Quest::idle();
    assert_eq!(q.party(), 1, "solo hero");
    q.apply(AdventureEvent::Party { size: 0 }, 0);
    assert_eq!(q.party(), 1, "0 clamps up to solo");
    q.apply(AdventureEvent::Party { size: 200 }, 0);
    assert_eq!(q.party(), 8, "clamped at 8");
    q.apply(AdventureEvent::Party { size: 5 }, 0);
    assert_eq!(q.party(), 5);
}

#[test]
fn submission_holds_the_keep_through_stalls_until_the_next_iteration() {
    let mut q = Quest::idle();
    q.apply(
        AdventureEvent::LoopStarted {
            kind: LoopKind::Coding,
            task: "build it".to_string(),
        },
        0,
    );
    assert_eq!(q.region(), Region::TheMines);
    q.apply(AdventureEvent::Submitted, 1);
    // A stall while the judge holds the work does not dislodge the keep —
    // and neither does the stall clearing (the mirror emits both edges).
    q.apply(AdventureEvent::Stall { level: 3 }, 2);
    assert_eq!(q.region(), Region::DragonKeep);
    q.apply(AdventureEvent::Stall { level: 0 }, 2);
    assert_eq!(q.region(), Region::DragonKeep);
    // The next iteration resumes the adventure grounds.
    q.apply(AdventureEvent::Iteration { n: 2 }, 3);
    assert_eq!(q.region(), Region::TheMines);
}

#[test]
fn pivot_clears_the_danger() {
    let mut q = Quest::idle();
    q.apply(
        AdventureEvent::LoopStarted {
            kind: LoopKind::Research,
            task: "explore".to_string(),
        },
        0,
    );
    q.apply(AdventureEvent::Stall { level: 2 }, 1);
    assert_eq!(q.region(), Region::Swamp);
    q.apply(AdventureEvent::Pivot, 2);
    assert_eq!(q.danger().level(), 0);
    assert_eq!(q.region(), Region::DarkForest);
}

/// The mirror emits the full ladder from a fake `LoopState` walk.
#[test]
fn loop_mirror_drains_the_event_ladder() {
    let mix = ToolMix::default();
    let mut mirror = LoopMirror::default();
    let party = 1u8;

    // Boot: nothing.
    let mut st = LoopState::default();
    assert!(mirror.drain(&st, &mix, party).is_empty());

    // A competition loop starts.
    st.status = LoopStatus::Running;
    st.task = "optimize the kernel benchmark".to_string();
    st.podrace = true;
    let events = mirror.drain(&st, &mix, party);
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
    let events = mirror.drain(&st, &mix, party);
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
    let events = mirror.drain(&st, &mix, party);
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(matches!(events[0], AdventureEvent::Iteration { n: 2 }));
    assert!(matches!(events[1], AdventureEvent::Stall { level: 0 }));
    assert!(matches!(events[2], AdventureEvent::MeasurementObserved));

    // Escalation is a pivot.
    st.tier = EscalationTier::Swarm;
    let events = mirror.drain(&st, &mix, party);
    assert!(matches!(events[0], AdventureEvent::Pivot), "{events:?}");

    // Submitting, then the acceptance gate takes the work.
    st.submissions = 1;
    let events = mirror.drain(&st, &mix, party);
    assert!(matches!(events[0], AdventureEvent::Submitted), "{events:?}");
    st.status = LoopStatus::Verifying;
    let events = mirror.drain(&st, &mix, party);
    assert!(matches!(events[0], AdventureEvent::Submitted), "{events:?}");

    // Accepted and done in one transition: settled before finished.
    st.status = LoopStatus::Done;
    let events = mirror.drain(&st, &mix, party);
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
    let events = mirror.drain(&st, &mix, 3);
    assert!(matches!(events[0], AdventureEvent::Party { size: 3 }));
}

#[test]
fn loop_mirror_verifier_block_forces_swamp_danger() {
    let mix = ToolMix::default();
    let mut mirror = LoopMirror::default();
    let mut st = LoopState {
        status: LoopStatus::Running,
        task: "vague".to_string(),
        pivot: 3,
        stale_count: 1,
        ..Default::default()
    };
    let events = mirror.drain(&st, &mix, 1);
    assert!(matches!(
        events.last(),
        Some(AdventureEvent::Stall { level: 1 })
    ));
    st.verifier_blocked = Some("benchmark missing --golden".to_string());
    let events = mirror.drain(&st, &mix, 1);
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
        status: LoopStatus::Verifying,
        task: "fix the flake".to_string(),
        ..Default::default()
    };
    mirror.drain(&st, &mix, 1);
    // The gate fails and the loop arms another iteration: the settled
    // verdict rides after the iteration event per the emission order.
    st.status = LoopStatus::Running;
    st.iteration = 1;
    let events = mirror.drain(&st, &mix, 1);
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
        status: LoopStatus::Running,
        podrace: true,
        stale_count: 999,
        pivot: 3,
        ..Default::default()
    };
    assert_eq!(
        LoopMirror::stall_level(&st),
        0,
        "missing observations do not prove inactivity"
    );
    st.log.push(serde_json::from_value(serde_json::json!({"iteration":1,"direction":"deep cut explored","new_findings":0,"tool_calls":3,"tool_errors":0,"workspace_changed":true,"stale_count":999,"ts_ms":0})).unwrap());
    assert_eq!(LoopMirror::stall_level(&st), 0);
    st.log.last_mut().unwrap().tool_errors = 3;
    assert_eq!(
        LoopMirror::stall_level(&st),
        2,
        "observed tool failures remain visible"
    );
    st.log.last_mut().unwrap().tool_errors = 0;
    st.verifier_blocked = Some("verifier unavailable".into());
    assert_eq!(
        LoopMirror::stall_level(&st),
        2,
        "concrete verifier blocker remains visible"
    );
}
#[test]
fn prose_improvement_never_awards_treasure_for_an_opaque_measurement() {
    let mut st = LoopState {
        status: LoopStatus::Running,
        podrace: true,
        ..Default::default()
    };
    let mut mirror = LoopMirror::default();
    let mix = ToolMix::default();
    let mut quest = Quest::idle();
    for event in mirror.drain(&st, &mix, 1) {
        quest.apply(event, 0);
    }
    st.measured_candidates = 1;
    st.log.push(serde_json::from_value(serde_json::json!({"iteration":1,"direction":"improved faster better","new_findings":0,"tool_calls":1,"verified_outcome_actions":1,"stale_count":999,"ts_ms":0})).unwrap());
    let events = mirror.drain(&st, &mix, 1);
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

#[test]
fn prospective_walk_clock_keeps_history_and_resets_epochs_deterministically() {
    fn script() -> Quest {
        let mut q = Quest::idle();
        q.apply(
            AdventureEvent::LoopStarted {
                kind: LoopKind::Competition,
                task: String::new(),
            },
            0,
        );
        q.observe_tick(21);
        q.apply(AdventureEvent::Stall { level: 1 }, 21);
        assert_eq!(q.walk_half_ticks(20), 40);
        assert_eq!(q.walk_half_ticks(21), 42);
        assert_eq!(q.walk_half_ticks(23), 44);
        q.apply(AdventureEvent::Stall { level: 0 }, 23);
        q.apply(AdventureEvent::Stall { level: 1 }, 23);
        q.apply(AdventureEvent::Pivot, 23);
        assert_eq!(q.walk_half_ticks(22), 43);
        assert_eq!(q.walk_half_ticks(25), 48);
        q.observe_tick(30);
        q.apply(AdventureEvent::Stall { level: 1 }, 24); // late fact: no retroactive motion
        assert_eq!(q.walk_half_ticks(30), 58);
        assert_eq!(q.walk_half_ticks(32), 60);
        q.apply(AdventureEvent::Iteration { n: 1 }, 32);
        assert_eq!(q.walk_half_ticks(32), 0);
        assert_eq!(q.walk_half_ticks(34), 2);
        q.apply(AdventureEvent::Stall { level: 2 }, 34); // actual region transition is a new epoch
        assert_eq!(q.region(), Region::Swamp);
        assert_eq!(q.walk_half_ticks(34), 0);
        q.apply(AdventureEvent::Pivot, 36);
        assert_eq!(q.walk_half_ticks(36), 0);
        q.apply(AdventureEvent::LoopFinished { ok: true }, 38);
        assert_eq!(q.walk_half_ticks(38), 0);
        q.observe_tick(38 + HOMECOMING_TICKS);
        assert_eq!(q.region(), Region::CastleTown);
        assert_eq!(q.walk_half_ticks(38 + HOMECOMING_TICKS), 0);
        q.apply(
            AdventureEvent::LoopStarted {
                kind: LoopKind::Competition,
                task: String::new(),
            },
            u64::MAX - 2,
        );
        assert_eq!(q.walk_half_ticks(u64::MAX), 4);
        q
    }
    assert_eq!(script(), script());
}

#[test]
fn same_tick_stall_pivot_preserves_mines_journey_and_room_history() {
    let mut q = Quest::idle();
    q.apply(
        AdventureEvent::LoopStarted {
            kind: LoopKind::Competition,
            task: String::new(),
        },
        0,
    );
    q.apply(AdventureEvent::Iteration { n: 3 }, 2);
    q.apply(AdventureEvent::Stall { level: 1 }, 90);
    q.observe_tick(100);
    let history: Vec<_> = (88..=100).map(|tick| q.walk_half_ticks(tick)).collect();
    let region_since = q.region_since();
    // LoopMirror::drain emits these in one batch, without a painted Swamp frame.
    q.apply(AdventureEvent::Stall { level: 2 }, 100);
    q.apply(AdventureEvent::Pivot, 100);
    assert_eq!(q.region(), Region::TheMines);
    assert_eq!(
        (88..=100)
            .map(|tick| q.walk_half_ticks(tick))
            .collect::<Vec<_>>(),
        history
    );
    assert_eq!(q.region_since(), region_since);
    assert_eq!(q.walk_half_ticks(101), history[12] + 2);
}

#[test]
fn walk_history_is_bounded_with_exact_twelve_tick_anchor_and_released_epochs() {
    let mut q = Quest::idle();
    q.apply(
        AdventureEvent::LoopStarted {
            kind: LoopKind::Competition,
            task: String::new(),
        },
        0,
    );
    let expected = |tick: u64| u128::from(tick) + u128::from(tick.div_ceil(2));
    for tick in 1u64..=100_000 {
        q.apply(
            AdventureEvent::Stall {
                level: (tick % 2) as u8,
            },
            tick,
        );
        for past in tick.saturating_sub(WALK_LOOKBACK)..=tick {
            assert_eq!(
                q.walk_half_ticks(past),
                expected(past),
                "tick={tick} past={past}"
            );
        }
        assert!(q.walk_clock.len() <= WALK_SEGMENTS);
        assert!(q.walk_clock.capacity() <= WALK_SEGMENTS);
    }
    // Older queries clamp to the anchor, not a fabricated old pace or zero.
    assert_eq!(q.walk_clock[0].tick, 100_000 - WALK_LOOKBACK);
    assert_eq!(q.walk_half_ticks(0), expected(100_000 - WALK_LOOKBACK));
    q.apply(AdventureEvent::Stall { level: 2 }, 100_000);
    assert!(q.region_rollback.as_ref().unwrap().clock.capacity() <= WALK_SEGMENTS);
    q.apply(AdventureEvent::Pivot, 100_000);
    assert_eq!(q.walk_half_ticks(100_000), expected(100_000));
    q.observe_tick(200_000);
    assert!(q.region_rollback.is_none());
    assert_eq!(
        q.walk_clock.len(),
        1,
        "quiet time compacts the tail as well"
    );
    q.apply(AdventureEvent::Iteration { n: 5 }, 200_000);
    assert_eq!(q.walk_clock.len(), 1);
    assert!(q.walk_clock.capacity() <= WALK_SEGMENTS);
    assert_eq!(
        q.walk_half_ticks(0),
        0,
        "older lookup before new epoch clamps to its zero anchor"
    );
    assert!(WALK_SEGMENTS * 2 * std::mem::size_of::<WalkPace>() <= 1024);
}

#[test]
fn same_tick_round_trip_does_not_override_explicit_iteration_or_cross_tick_entry() {
    let mut q = Quest::idle();
    q.apply(
        AdventureEvent::LoopStarted {
            kind: LoopKind::Competition,
            task: String::new(),
        },
        0,
    );
    q.apply(AdventureEvent::Stall { level: 2 }, 40);
    q.observe_tick(41); // now Swamp is a committed epoch
    q.apply(AdventureEvent::Pivot, 41);
    assert_eq!(q.walk_half_ticks(41), 0);
    q.apply(AdventureEvent::Stall { level: 2 }, 50);
    q.apply(AdventureEvent::Iteration { n: 3 }, 50);
    q.apply(AdventureEvent::Pivot, 50);
    assert_eq!(
        q.walk_half_ticks(50),
        0,
        "iteration must discard the old Mines epoch"
    );
    assert_eq!(q.walk_half_ticks(52), 4);
}
