use super::scheduler_validation::{assignment, health, services};
use super::schema::{DirectorHealthV1, LaneIdV1, ScheduledActionV1};
use serde::{Deserialize, Serialize};

pub(crate) const SCHEDULER_SCHEMA_V1: &str = "angel.competition-scheduler/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SchedulerIncidentV1 {
    None,
    AdapterFailure,
    ProviderFailure,
    ToolFailure,
    Retry,
    WorkerReplacement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LaneScheduleStateV1 {
    pub(crate) lane: LaneIdV1,
    pub(crate) runnable: bool,
    pub(crate) age_ticks: u64,
    pub(crate) checkpoint_revision: u64,
    pub(crate) worker_generation: u64,
    pub(crate) service_count: u64,
    pub(crate) last_served_tick: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchedulerStateV1 {
    pub(crate) schema: String,
    pub(crate) tick: u64,
    pub(crate) seats: u16,
    pub(crate) max_service_gap_ticks: u64,
    pub(crate) lanes: [LaneScheduleStateV1; 2],
    pub(crate) frontier_priority_boost: bool,
    pub(crate) last_board_revision: Option<u64>,
    pub(crate) last_single_lane: Option<LaneIdV1>,
    pub(crate) last_now_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchedulerAssignmentV1 {
    pub(crate) lane: LaneIdV1,
    pub(crate) worker_generation: u64,
    pub(crate) checkpoint_revision: u64,
    pub(crate) scheduled: ScheduledActionV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndependentServicesV1 {
    pub(crate) observer: ScheduledActionV1,
    pub(crate) context: ScheduledActionV1,
    pub(crate) submission_pump: ScheduledActionV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchedulerDecisionV1 {
    pub(crate) tick: u64,
    pub(crate) assignments: Vec<SchedulerAssignmentV1>,
    pub(crate) services: IndependentServicesV1,
    pub(crate) health: DirectorHealthV1,
}

impl SchedulerStateV1 {
    pub(crate) fn new(seats: u16, max_service_gap_ticks: u64) -> Result<Self, String> {
        let lane = |lane| LaneScheduleStateV1 {
            lane,
            runnable: true,
            age_ticks: 0,
            checkpoint_revision: 0,
            worker_generation: 1,
            service_count: 0,
            last_served_tick: None,
        };
        let state = Self {
            schema: SCHEDULER_SCHEMA_V1.into(),
            tick: 0,
            seats,
            max_service_gap_ticks,
            lanes: [lane(LaneIdV1::FrontierGuard), lane(LaneIdV1::DeepCut)],
            frontier_priority_boost: false,
            last_board_revision: None,
            last_single_lane: None,
            last_now_ms: 0,
        };
        state.validate()?;
        Ok(state)
    }

    pub(crate) fn lane(&self, lane: LaneIdV1) -> Result<&LaneScheduleStateV1, String> {
        self.lanes
            .iter()
            .find(|state| state.lane == lane)
            .ok_or_else(|| "lane is not scheduled by the core scheduler".into())
    }

    pub(crate) fn set_runnable(&mut self, lane: LaneIdV1, runnable: bool) -> Result<(), String> {
        self.lane_mut(lane)?.runnable = runnable;
        self.validate()
    }

    pub(crate) fn checkpoint_lane(&mut self, lane: LaneIdV1, revision: u64) -> Result<(), String> {
        let state = self.lane_mut(lane)?;
        if revision < state.checkpoint_revision {
            return Err("lane checkpoint cannot roll back".into());
        }
        state.checkpoint_revision = revision;
        self.validate()
    }

    pub(crate) fn replace_worker(&mut self, lane: LaneIdV1) -> Result<u64, String> {
        let state = self.lane_mut(lane)?;
        state.worker_generation = state
            .worker_generation
            .checked_add(1)
            .ok_or_else(|| "worker generation exhausted".to_string())?;
        let generation = state.worker_generation;
        self.validate()?;
        Ok(generation)
    }

    pub(crate) fn observe_board_move(&mut self, revision: u64) -> Result<bool, String> {
        if revision == 0 || self.last_board_revision.is_some_and(|seen| revision < seen) {
            return Err("board revision cannot be zero or roll back".into());
        }
        if self.last_board_revision == Some(revision) {
            return Ok(false);
        }
        self.last_board_revision = Some(revision);
        self.frontier_priority_boost = true;
        self.validate()?;
        Ok(true)
    }

    pub(crate) fn schedule(
        &mut self,
        now_ms: u64,
        incident: SchedulerIncidentV1,
    ) -> Result<SchedulerDecisionV1, String> {
        let mut next = self.clone();
        let decision = next.schedule_next(now_ms, incident)?;
        *self = next;
        Ok(decision)
    }

    fn schedule_next(
        &mut self,
        now_ms: u64,
        incident: SchedulerIncidentV1,
    ) -> Result<SchedulerDecisionV1, String> {
        self.validate()?;
        if now_ms < self.last_now_ms {
            return Err("scheduler time cannot roll back".into());
        }
        let next_tick = self
            .tick
            .checked_add(1)
            .ok_or_else(|| "scheduler tick exhausted".to_string())?;
        let selected = self.select_lanes();
        for index in 0..self.lanes.len() {
            if !self.lanes[index].runnable {
                continue;
            }
            if selected.contains(&index) {
                let lane = &mut self.lanes[index];
                lane.age_ticks = 0;
                lane.service_count = lane
                    .service_count
                    .checked_add(1)
                    .ok_or_else(|| "lane service counter exhausted".to_string())?;
                lane.last_served_tick = Some(next_tick);
            } else {
                self.lanes[index].age_ticks = self.lanes[index]
                    .age_ticks
                    .checked_add(1)
                    .ok_or_else(|| "lane age exhausted".to_string())?;
            }
        }
        if self.seats == 1 {
            self.last_single_lane = selected.first().map(|index| self.lanes[*index].lane);
        }
        if selected.contains(&0) {
            self.frontier_priority_boost = false;
        }
        self.tick = next_tick;
        self.last_now_ms = now_ms;
        self.validate()?;

        let assignments = selected
            .into_iter()
            .map(|index| assignment(&self.lanes[index], now_ms))
            .collect::<Vec<_>>();
        let decision = SchedulerDecisionV1 {
            tick: self.tick,
            assignments,
            services: services(now_ms),
            health: health(incident, self.last_board_revision, now_ms)?,
        };
        decision.validate()?;
        Ok(decision)
    }

    fn select_lanes(&self) -> Vec<usize> {
        let runnable = (0..self.lanes.len())
            .filter(|index| self.lanes[*index].runnable)
            .collect::<Vec<_>>();
        if self.seats >= 2 || runnable.len() < 2 {
            return runnable;
        }
        let due = runnable
            .iter()
            .copied()
            .filter(|index| self.lanes[*index].age_ticks + 1 >= self.max_service_gap_ticks);
        if let Some(index) = choose_oldest(due, &self.lanes, self.last_single_lane) {
            return vec![index];
        }
        if self.frontier_priority_boost && self.lanes[0].runnable {
            return vec![0];
        }
        vec![choose_oldest(runnable.into_iter(), &self.lanes, self.last_single_lane).unwrap()]
    }

    fn lane_mut(&mut self, lane: LaneIdV1) -> Result<&mut LaneScheduleStateV1, String> {
        self.lanes
            .iter_mut()
            .find(|state| state.lane == lane)
            .ok_or_else(|| "lane is not scheduled by the core scheduler".into())
    }
}

fn choose_oldest(
    indices: impl Iterator<Item = usize>,
    lanes: &[LaneScheduleStateV1; 2],
    last: Option<LaneIdV1>,
) -> Option<usize> {
    indices.max_by_key(|index| (lanes[*index].age_ticks, Some(lanes[*index].lane) != last))
}
