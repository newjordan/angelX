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
