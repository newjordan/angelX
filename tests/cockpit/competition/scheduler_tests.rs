use super::scheduler::{SchedulerIncidentV1, SchedulerStateV1};
use super::schema::LaneIdV1;

fn assignment_lanes(decision: &super::scheduler::SchedulerDecisionV1) -> Vec<LaneIdV1> {
    decision.assignments.iter().map(|item| item.lane).collect()
}

#[test]
fn dc_sched_001_single_seat_both_lanes_within_max_gap() {
    let mut scheduler = SchedulerStateV1::new(1, 4).unwrap();
    let mut last_service = [None, None];
    for now in 1..=128 {
        let decision = scheduler.schedule(now, SchedulerIncidentV1::None).unwrap();
        decision.validate().unwrap();
        assert_eq!(decision.assignments.len(), 1);
        for assignment in &decision.assignments {
            let index = usize::from(assignment.lane == LaneIdV1::DeepCut);
            if let Some(last) = last_service[index] {
                assert!(decision.tick - last <= scheduler.max_service_gap_ticks);
            }
            last_service[index] = Some(decision.tick);
        }
        for lane in &scheduler.lanes {
            assert!(lane.age_ticks < scheduler.max_service_gap_ticks);
        }
    }
    assert!(last_service.iter().all(Option::is_some));
    let restored: SchedulerStateV1 =
        serde_json::from_slice(&serde_json::to_vec(&scheduler).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, scheduler);
}

#[test]
fn fresh_state_initial_service_latency_is_bounded() {
    let mut scheduler = SchedulerStateV1::new(1, 2).unwrap();
    let mut first_service = [None, None];
    for now in 1..=scheduler.max_service_gap_ticks {
        let decision = scheduler.schedule(now, SchedulerIncidentV1::None).unwrap();
        for assignment in decision.assignments {
            let index = usize::from(assignment.lane == LaneIdV1::DeepCut);
            first_service[index].get_or_insert(decision.tick);
        }
    }
    for first in first_service {
        assert!(first.is_some_and(|tick| tick <= scheduler.max_service_gap_ticks));
    }
}

#[test]
fn dc_sched_002_multi_seat_reserves_frontier_and_deep_cut() {
    let mut scheduler = SchedulerStateV1::new(3, 4).unwrap();
    for now in 1..=32 {
        let decision = scheduler.schedule(now, SchedulerIncidentV1::None).unwrap();
        assert_eq!(
            assignment_lanes(&decision),
            vec![LaneIdV1::FrontierGuard, LaneIdV1::DeepCut]
        );
        assert_eq!(decision.services.observer.action, "observe-board");
        assert_eq!(decision.services.context.action, "service-context");
        assert_eq!(decision.services.submission_pump.action, "pump-submissions");
    }
    assert_eq!(
        scheduler
            .lane(LaneIdV1::FrontierGuard)
            .unwrap()
            .service_count,
        32
    );
    assert_eq!(scheduler.lane(LaneIdV1::DeepCut).unwrap().service_count, 32);
}

#[test]
fn dc_sched_003_board_move_bumps_frontier_without_erasing_deep_cut() {
    let mut scheduler = SchedulerStateV1::new(1, 4).unwrap();
    scheduler.schedule(1, SchedulerIncidentV1::None).unwrap();
    scheduler.schedule(2, SchedulerIncidentV1::None).unwrap();
    let deep_age = scheduler.lane(LaneIdV1::DeepCut).unwrap().age_ticks;
    assert!(deep_age > 0);

    assert!(scheduler.observe_board_move(7).unwrap());
    let decision = scheduler.schedule(3, SchedulerIncidentV1::None).unwrap();
    assert_eq!(assignment_lanes(&decision), vec![LaneIdV1::FrontierGuard]);
    assert_eq!(
        scheduler.lane(LaneIdV1::DeepCut).unwrap().age_ticks,
        deep_age + 1
    );
    assert!(!scheduler.frontier_priority_boost);

    let next = scheduler.schedule(4, SchedulerIncidentV1::None).unwrap();
    assert_eq!(assignment_lanes(&next), vec![LaneIdV1::DeepCut]);
    assert!(!scheduler.observe_board_move(7).unwrap());
}

#[test]
fn deep_cut_deadline_wins_same_quantum_frontier_board_bump() {
    let mut scheduler = SchedulerStateV1::new(1, 4).unwrap();
    assert_eq!(
        assignment_lanes(&scheduler.schedule(1, SchedulerIncidentV1::None).unwrap()),
        vec![LaneIdV1::DeepCut]
    );
    scheduler.schedule(2, SchedulerIncidentV1::None).unwrap();
    for (revision, now) in [(1, 3), (2, 4)] {
        scheduler.observe_board_move(revision).unwrap();
        assert_eq!(
            assignment_lanes(&scheduler.schedule(now, SchedulerIncidentV1::None).unwrap()),
            vec![LaneIdV1::FrontierGuard]
        );
    }
    assert_eq!(scheduler.lane(LaneIdV1::DeepCut).unwrap().age_ticks, 3);

    scheduler.observe_board_move(3).unwrap();
    let due = scheduler.schedule(5, SchedulerIncidentV1::None).unwrap();
    assert_eq!(assignment_lanes(&due), vec![LaneIdV1::DeepCut]);
    assert!(scheduler.frontier_priority_boost);
    let bumped = scheduler.schedule(6, SchedulerIncidentV1::None).unwrap();
    assert_eq!(assignment_lanes(&bumped), vec![LaneIdV1::FrontierGuard]);
    assert!(!scheduler.frontier_priority_boost);
}

#[test]
fn dc_sched_004_worker_replacement_preserves_lane_age() {
    let mut scheduler = SchedulerStateV1::new(1, 4).unwrap();
    while scheduler.lane(LaneIdV1::DeepCut).unwrap().age_ticks == 0 {
        let now = scheduler.tick + 1;
        scheduler.schedule(now, SchedulerIncidentV1::None).unwrap();
    }
    scheduler.checkpoint_lane(LaneIdV1::DeepCut, 42).unwrap();
    let before = scheduler.lane(LaneIdV1::DeepCut).unwrap().clone();
    let generation = scheduler.replace_worker(LaneIdV1::DeepCut).unwrap();
    let after = scheduler.lane(LaneIdV1::DeepCut).unwrap();
    assert_eq!(generation, before.worker_generation + 1);
    assert_eq!(after.age_ticks, before.age_ticks);
    assert_eq!(after.checkpoint_revision, before.checkpoint_revision);
    assert_eq!(after.service_count, before.service_count);
    scheduler = serde_json::from_slice(&serde_json::to_vec(&scheduler).unwrap()).unwrap();
    scheduler.validate().unwrap();

    let decision = scheduler
        .schedule(scheduler.tick + 1, SchedulerIncidentV1::WorkerReplacement)
        .unwrap();
    let deep = decision
        .assignments
        .iter()
        .find(|item| item.lane == LaneIdV1::DeepCut)
        .unwrap();
    assert_eq!(deep.worker_generation, generation);
    assert_eq!(deep.checkpoint_revision, 42);
}

#[test]
fn deterministic_fallback_and_throughput_fixture_toward_ac16() {
    let mut scheduler = SchedulerStateV1::new(1, 4).unwrap();
    scheduler.set_runnable(LaneIdV1::DeepCut, false).unwrap();
    let checkpoint = serde_json::to_vec(&scheduler).unwrap();
    let run = |mut state: SchedulerStateV1| {
        let mut trace = Vec::new();
        for now in 1..=64 {
            let incident = if now % 7 == 0 {
                SchedulerIncidentV1::AdapterFailure
            } else {
                SchedulerIncidentV1::None
            };
            trace.push(state.schedule(now, incident).unwrap());
        }
        (state, trace)
    };
    let (first_state, first_trace) =
        run(serde_json::from_slice::<SchedulerStateV1>(&checkpoint).unwrap());
    let (second_state, second_trace) =
        run(serde_json::from_slice::<SchedulerStateV1>(&checkpoint).unwrap());
    assert_eq!(first_trace, second_trace);
    assert_eq!(first_state, second_state);
    assert!(first_trace.iter().all(|decision| {
        assignment_lanes(decision) == vec![LaneIdV1::FrontierGuard]
            && decision.services.submission_pump.action == "pump-submissions"
    }));
    assert_eq!(
        first_state
            .lane(LaneIdV1::FrontierGuard)
            .unwrap()
            .service_count,
        64
    );
}

// --- live `lane-schedule` leg -------------------------------------------------
//
// The Bend leg owns the lane ranking; Rust keeps the state, the validation and
// the application of the choice. `select_lanes` stays as the native fallback and
// as the parity oracle for the table below.

fn lane_state(
    seats: u16,
    gap: u64,
    boost: bool,
    last: Option<LaneIdV1>,
    states: [(bool, u64); 2],
) -> SchedulerStateV1 {
    let mut state = SchedulerStateV1::new(seats, gap).unwrap();
    state.frontier_priority_boost = boost;
    state.last_single_lane = last;
    for (index, (runnable, age_ticks)) in states.into_iter().enumerate() {
        state.lanes[index].runnable = runnable;
        state.lanes[index].age_ticks = age_ticks;
    }
    state
}

fn live_lanes(
    leg: &angel_live::Program,
    state: &SchedulerStateV1,
) -> (Vec<usize>, crate::agent::harness::live_policy::LegSource) {
    let input = state.lane_input().expect("scheduler snapshot");
    let (selection, source) = crate::agent::harness::live_policy::decide_with(
        Some(leg.clone()),
        super::scheduler::LANE_SCHEDULE_LEG,
        &input,
        |selection: &super::scheduler::LaneSelectionV1| state.check_lanes(&selection.lanes),
        || super::scheduler::LaneSelectionV1 {
            lanes: state.select_lanes(),
        },
    );
    (selection.lanes, source)
}

/// The same decision, taken by the generation compiled into this binary: the
/// shipped leg answers without a module directory, exactly as a turn asks it.
fn shipped_lanes(
    state: &SchedulerStateV1,
) -> (Vec<usize>, crate::agent::harness::live_policy::LegSource) {
    let input = state.lane_input().expect("scheduler snapshot");
    let (selection, source) = crate::agent::harness::live_policy::decide_shipped(
        super::scheduler::LANE_SCHEDULE_LEG,
        &input,
        |selection: &super::scheduler::LaneSelectionV1| state.check_lanes(&selection.lanes),
    );
    (selection.lanes, source)
}

fn temporary_lane_leg(id: &str, render: &str) -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-lane-leg-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let module = root.join(id);
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.json"),
        format!(
            r#"{{"api_version":1,"id":"{id}","kind":"policy","entry":"index.js","probe":{{}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(module.join("index.js"), render).unwrap();
    let mut registry =
        angel_live::Registry::validated(root, crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    registry
        .snapshot()
        .policies
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("fixture leg {id} did not publish"))
}

#[test]
fn live_lane_schedule_leg_matches_the_native_selection() {
    let shapes = [(true, true), (true, false), (false, true), (false, false)];
    let mut checked = 0;
    let mut widths = std::collections::BTreeSet::new();
    let mut chosen = std::collections::BTreeSet::new();
    for seats in [1u16, 2, 3] {
        for gap in [2u64, 4, 8] {
            for boost in [false, true] {
                for last in [None, Some(LaneIdV1::FrontierGuard), Some(LaneIdV1::DeepCut)] {
                    for shape in shapes {
                        for ages in [
                            (0, 0),
                            (0, 1),
                            (1, 0),
                            (1, 1),
                            (gap - 1, 0),
                            (0, gap - 1),
                            (gap - 1, gap - 1),
                            (gap / 2, gap - 2),
                        ] {
                            let state = lane_state(
                                seats,
                                gap,
                                boost,
                                last,
                                [(shape.0, ages.0), (shape.1, ages.1)],
                            );
                            let native = state.select_lanes();
                            let (live, source) = shipped_lanes(&state);
                            assert_eq!(
                                source.as_str(),
                                "shipped",
                                "seats {seats} gap {gap} boost {boost} last {last:?} lanes {shape:?} ages {ages:?}: {source:?}"
                            );
                            assert_eq!(
                                live, native,
                                "seats {seats} gap {gap} boost {boost} last {last:?} lanes {shape:?} ages {ages:?}"
                            );
                            checked += 1;
                            widths.insert(native.len());
                            chosen.extend(native.iter().copied());
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 30, "only {checked} lane states were compared");
    assert_eq!(
        widths.into_iter().collect::<Vec<_>>(),
        vec![0, 1, 2],
        "the table must reach every seat width"
    );
    assert_eq!(
        chosen.into_iter().collect::<Vec<_>>(),
        vec![0, 1],
        "the table must seat both lanes"
    );
}

#[test]
fn the_lane_leg_reads_the_state_it_was_promised() {
    let state = lane_state(1, 4, true, Some(LaneIdV1::DeepCut), [(true, 3), (false, 1)]);
    assert_eq!(
        state.lane_input().unwrap(),
        serde_json::json!({
            "seats": 1,
            "max_service_gap_ticks": 4,
            "frontier_priority_boost": true,
            "last_single_lane": 1,
            "lanes": [
                {"index": 0, "runnable": true, "age_ticks": 3},
                {"index": 1, "runnable": false, "age_ticks": 1},
            ],
        })
    );
    let mut wide = lane_state(2, u32::MAX as u64, false, None, [(true, 0), (true, 0)]);
    assert!(wide.lane_input().is_some());
    wide.max_service_gap_ticks = u32::MAX as u64 + 1;
    assert!(
        wide.lane_input().is_none(),
        "a counter past the leg's U32 arithmetic keeps the native selection"
    );
}

#[test]
fn an_out_of_contract_live_schedule_keeps_the_native_selection() {
    let native = |state: &SchedulerStateV1| state.select_lanes();
    let wide = lane_state(2, 4, false, None, [(true, 1), (true, 0)]);
    assert_eq!(native(&wide), vec![0, 1]);
    for (id, render) in [
        (
            "lane-duplicate",
            "module.exports={apiVersion:1,render:()=>({lanes:[0,0]})}",
        ),
        (
            "lane-absent",
            "module.exports={apiVersion:1,render:()=>({lanes:[0,7]})}",
        ),
        (
            "lane-silent",
            "module.exports={apiVersion:1,render:()=>({})}",
        ),
    ] {
        let leg = temporary_lane_leg(id, render);
        let (lanes, source) = live_lanes(&leg, &wide);
        assert!(
            matches!(
                source,
                crate::agent::harness::live_policy::LegSource::LiveFallback { .. }
            ),
            "{id}: {source:?}"
        );
        assert_eq!(lanes, native(&wide), "{id}");
    }
    for (state, id, render) in [
        (
            lane_state(1, 4, false, None, [(true, 0), (true, 1)]),
            "lane-over-seated",
            "module.exports={apiVersion:1,render:()=>({lanes:[0,1]})}",
        ),
        (
            lane_state(2, 4, false, None, [(false, 0), (true, 0)]),
            "lane-unrunnable",
            "module.exports={apiVersion:1,render:()=>({lanes:[0]})}",
        ),
    ] {
        let leg = temporary_lane_leg(id, render);
        let (lanes, source) = live_lanes(&leg, &state);
        assert!(
            matches!(
                source,
                crate::agent::harness::live_policy::LegSource::LiveFallback { .. }
            ),
            "{id}: {source:?}"
        );
        assert_eq!(lanes, native(&state), "{id}");
    }
}

#[test]
fn live_lane_schedule_leg_decision_cost() {
    let state = lane_state(
        1,
        4,
        false,
        Some(LaneIdV1::FrontierGuard),
        [(true, 1), (true, 1)],
    );
    let native = state.select_lanes();
    let mut timings = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (lanes, source) = shipped_lanes(&state);
        timings.push(started.elapsed());
        assert_eq!(source.as_str(), "shipped");
        assert_eq!(lanes, native);
    }
    timings.sort();
    let mut native_timings = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        assert_eq!(state.select_lanes(), native);
        native_timings.push(started.elapsed());
    }
    native_timings.sort();
    eprintln!(
        "lane-schedule leg: median {:.2} us/decision, p95 {:.2} us over 200 calls (native oracle median {:.3} us)",
        timings[timings.len() / 2].as_secs_f64() * 1e6,
        timings[timings.len() * 95 / 100].as_secs_f64() * 1e6,
        native_timings[native_timings.len() / 2].as_secs_f64() * 1e6,
    );
}
