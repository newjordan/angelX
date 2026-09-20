use super::scheduler::{SchedulerIncidentV1, SchedulerStateV1};
use super::schema::{DirectorHealthStateV1, LaneIdV1};

#[test]
fn director_million_tick_failure_soak_no_blocker_or_starvation_ac06() {
    const TICKS: u64 = 1_000_000;
    let mut scheduler = SchedulerStateV1::new(1, 4).unwrap();
    let mut random = 0x6a09_e667_f3bc_c909_u64;
    let mut board_revision = 0_u64;
    let mut checkpoint_revision = 0_u64;
    let mut incidents = [0_u64; 5];
    let mut replacements = 0_u64;

    for now in 1..=TICKS {
        random = random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        if random.is_multiple_of(997) {
            board_revision += 1;
            scheduler.observe_board_move(board_revision).unwrap();
        }
        let incident = if random.is_multiple_of(4_093) {
            checkpoint_revision += 1;
            scheduler
                .checkpoint_lane(LaneIdV1::DeepCut, checkpoint_revision)
                .unwrap();
            scheduler.replace_worker(LaneIdV1::DeepCut).unwrap();
            replacements += 1;
            SchedulerIncidentV1::WorkerReplacement
        } else {
            match random % 101 {
                0..=4 => SchedulerIncidentV1::AdapterFailure,
                5..=8 => SchedulerIncidentV1::ProviderFailure,
                9..=12 => SchedulerIncidentV1::ToolFailure,
                13..=16 => SchedulerIncidentV1::Retry,
                _ => SchedulerIncidentV1::None,
            }
        };
        let incident_index = match incident {
            SchedulerIncidentV1::AdapterFailure => Some(0),
            SchedulerIncidentV1::ProviderFailure => Some(1),
            SchedulerIncidentV1::ToolFailure => Some(2),
            SchedulerIncidentV1::Retry => Some(3),
            SchedulerIncidentV1::WorkerReplacement => Some(4),
            SchedulerIncidentV1::None => None,
        };
        if let Some(index) = incident_index {
            incidents[index] += 1;
        }

        let decision = scheduler.schedule(now, incident).unwrap();
        decision.validate().unwrap();
        assert!(!decision.assignments.is_empty());
        assert_eq!(decision.services.observer.action, "observe-board");
        assert_eq!(decision.services.context.action, "service-context");
        assert_eq!(decision.services.submission_pump.action, "pump-submissions");
        assert!(matches!(
            decision.health.state,
            DirectorHealthStateV1::Fresh
                | DirectorHealthStateV1::Degraded
                | DirectorHealthStateV1::Retrying
                | DirectorHealthStateV1::NeedsAttention
        ));
        for lane in &scheduler.lanes {
            assert!(lane.runnable);
            assert!(lane.age_ticks < scheduler.max_service_gap_ticks);
        }
    }

    assert_eq!(scheduler.tick, TICKS);
    assert!(incidents.iter().all(|count| *count > 0));
    assert!(replacements > 0);
    assert!(board_revision > 0);
    for lane in [LaneIdV1::FrontierGuard, LaneIdV1::DeepCut] {
        assert!(scheduler.lane(lane).unwrap().service_count >= TICKS / 4);
    }
}
