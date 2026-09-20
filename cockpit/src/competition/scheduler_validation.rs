use super::scheduler::{
    IndependentServicesV1, LaneScheduleStateV1, SCHEDULER_SCHEMA_V1, SchedulerAssignmentV1,
    SchedulerDecisionV1, SchedulerIncidentV1, SchedulerStateV1,
};
use super::schema::{
    COMPETITION_SCHEMA_V1, DirectorHealthStateV1, DirectorHealthV1, LaneIdV1, ScheduledActionV1,
};
impl SchedulerStateV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEDULER_SCHEMA_V1 || self.seats == 0 || self.seats > 64 {
            return Err("invalid scheduler identity or seat count".into());
        }
        if self.max_service_gap_ticks < 2 {
            return Err("service gap must accommodate both core lanes".into());
        }
        if self.lanes[0].lane != LaneIdV1::FrontierGuard || self.lanes[1].lane != LaneIdV1::DeepCut
        {
            return Err("scheduler core lanes are not canonical".into());
        }
        for lane in &self.lanes {
            if lane.worker_generation == 0 || lane.age_ticks >= self.max_service_gap_ticks {
                return Err("invalid lane generation or starvation age".into());
            }
            if lane.last_served_tick.is_some_and(|tick| tick > self.tick) {
                return Err("lane service timestamp exceeds scheduler tick".into());
            }
        }
        if self.last_board_revision == Some(0)
            || self
                .last_single_lane
                .is_some_and(|lane| !is_core_lane(lane))
        {
            return Err("invalid scheduler revision or last lane".into());
        }
        Ok(())
    }
}

impl SchedulerDecisionV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        for (index, item) in self.assignments.iter().enumerate() {
            if !is_core_lane(item.lane)
                || item.worker_generation == 0
                || self.assignments[..index]
                    .iter()
                    .any(|prior| prior.lane == item.lane)
            {
                return Err("invalid or duplicate scheduler assignment".into());
            }
            item.scheduled.validate().map_err(str::to_string)?;
        }
        for action in [
            &self.services.observer,
            &self.services.context,
            &self.services.submission_pump,
        ] {
            action.validate().map_err(str::to_string)?;
        }
        self.health.validate().map_err(str::to_string)
    }
}

pub(super) fn assignment(lane: &LaneScheduleStateV1, now_ms: u64) -> SchedulerAssignmentV1 {
    let name = match lane.lane {
        LaneIdV1::FrontierGuard => "frontier-guard",
        LaneIdV1::DeepCut => "deep-cut",
        _ => "unsupported",
    };
    SchedulerAssignmentV1 {
        lane: lane.lane,
        worker_generation: lane.worker_generation,
        checkpoint_revision: lane.checkpoint_revision,
        scheduled: ScheduledActionV1 {
            action: format!("service-lane:{name}"),
            next_attempt_at_ms: now_ms,
        },
    }
}

pub(super) fn services(now_ms: u64) -> IndependentServicesV1 {
    let action = |name: &str| ScheduledActionV1 {
        action: name.into(),
        next_attempt_at_ms: now_ms,
    };
    IndependentServicesV1 {
        observer: action("observe-board"),
        context: action("service-context"),
        submission_pump: action("pump-submissions"),
    }
}

pub(super) fn health(
    incident: SchedulerIncidentV1,
    revision: Option<u64>,
    now_ms: u64,
) -> Result<DirectorHealthV1, String> {
    let (state, reason) = match incident {
        SchedulerIncidentV1::None => (DirectorHealthStateV1::Fresh, None),
        SchedulerIncidentV1::WorkerReplacement => (
            DirectorHealthStateV1::Degraded,
            Some("worker-replaced".into()),
        ),
        SchedulerIncidentV1::AdapterFailure => (
            DirectorHealthStateV1::Retrying,
            Some("adapter-failure".into()),
        ),
        SchedulerIncidentV1::ProviderFailure => (
            DirectorHealthStateV1::Retrying,
            Some("provider-failure".into()),
        ),
        SchedulerIncidentV1::ToolFailure => {
            (DirectorHealthStateV1::Retrying, Some("tool-failure".into()))
        }
        SchedulerIncidentV1::Retry => (DirectorHealthStateV1::Retrying, Some("retry-due".into())),
    };
    let next_at = now_ms
        .checked_add(1)
        .ok_or_else(|| "scheduler retry time exhausted".to_string())?;
    Ok(DirectorHealthV1 {
        schema: COMPETITION_SCHEMA_V1.into(),
        state,
        last_good_revision: revision,
        reason,
        next: ScheduledActionV1 {
            action: "scheduler-quantum".into(),
            next_attempt_at_ms: next_at,
        },
        updated_at_ms: now_ms,
    })
}

pub(super) fn is_core_lane(lane: LaneIdV1) -> bool {
    matches!(lane, LaneIdV1::FrontierGuard | LaneIdV1::DeepCut)
}
