use super::lease_store::{
    build_lease, canonical_lease_id, next_generation, validate_grant, validate_lease,
};
use super::schema::LaneIdV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub(crate) const HEARTBEAT_CADENCE_MS: u64 = 30_000;
pub(crate) const SUSPECT_AFTER_MS: u64 = 120_000;
pub(crate) const WORKER_LEASE_SCHEMA_V1: &str = "angel.competition-worker-lease/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LeasePhaseV1 {
    Granted,
    Active,
    Suspect,
    Nudged,
    Inspected,
    Fenced,
    Released,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkLeaseKeyV1 {
    pub(crate) campaign_id: String,
    pub(crate) lane_id: LaneIdV1,
    pub(crate) work_item_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerLeaseV1 {
    pub(crate) schema: String,
    pub(crate) key: WorkLeaseKeyV1,
    pub(crate) lease_id: String,
    pub(crate) fencing_generation: u64,
    pub(crate) worker_instance_id: String,
    pub(crate) granted_at_ms: u64,
    pub(crate) heartbeat_at_ms: u64,
    pub(crate) deadline_at_ms: u64,
    pub(crate) checkpoint_revision: u64,
    pub(crate) phase: LeasePhaseV1,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LeaseBookV1 {
    leases: BTreeMap<WorkLeaseKeyV1, WorkerLeaseV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LeaseError {
    InvalidIdentity,
    Missing,
    StaleGeneration,
    InvalidPhase,
    StaleCheckpoint,
    CounterExhausted,
}

impl fmt::Display for LeaseError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(match self {
            Self::InvalidIdentity => "invalid worker lease identity",
            Self::Missing => "worker lease is missing",
            Self::StaleGeneration => "worker lease generation is fenced",
            Self::InvalidPhase => "worker lease phase transition is invalid",
            Self::StaleCheckpoint => "worker checkpoint revision moved backward",
            Self::CounterExhausted => "worker lease generation is exhausted",
        })
    }
}

impl LeaseBookV1 {
    pub(crate) fn grant(
        &mut self,
        key: WorkLeaseKeyV1,
        worker_instance_id: String,
        now_ms: u64,
        deadline_at_ms: u64,
        checkpoint_revision: u64,
    ) -> Result<WorkerLeaseV1, LeaseError> {
        validate_grant(&key, &worker_instance_id, now_ms, deadline_at_ms)?;
        let previous = self.leases.get(&key);
        if previous.is_some_and(|lease| {
            !matches!(lease.phase, LeasePhaseV1::Fenced | LeasePhaseV1::Released)
        }) {
            return Err(LeaseError::InvalidPhase);
        }
        if previous.is_some_and(|lease| checkpoint_revision < lease.checkpoint_revision) {
            return Err(LeaseError::StaleCheckpoint);
        }
        let generation = next_generation(previous)?;
        let lease = build_lease(
            key.clone(),
            generation,
            worker_instance_id,
            now_ms,
            deadline_at_ms,
            checkpoint_revision,
        );
        self.leases.insert(key, lease.clone());
        Ok(lease)
    }

    pub(crate) fn current(&self, key: &WorkLeaseKeyV1) -> Option<&WorkerLeaseV1> {
        self.leases.get(key)
    }

    pub(crate) fn heartbeat(
        &mut self,
        key: &WorkLeaseKeyV1,
        generation: u64,
        now_ms: u64,
        checkpoint_revision: u64,
    ) -> Result<(), LeaseError> {
        let lease = self.current_mut(key, generation)?;
        if matches!(lease.phase, LeasePhaseV1::Fenced | LeasePhaseV1::Released) {
            return Err(LeaseError::InvalidPhase);
        }
        if checkpoint_revision < lease.checkpoint_revision {
            return Err(LeaseError::StaleCheckpoint);
        }
        lease.heartbeat_at_ms = lease.heartbeat_at_ms.max(now_ms);
        lease.checkpoint_revision = checkpoint_revision;
        lease.phase = LeasePhaseV1::Active;
        Ok(())
    }

    pub(crate) fn poll(
        &mut self,
        key: &WorkLeaseKeyV1,
        generation: u64,
        now_ms: u64,
    ) -> Result<LeasePhaseV1, LeaseError> {
        let lease = self.current_mut(key, generation)?;
        if matches!(lease.phase, LeasePhaseV1::Granted | LeasePhaseV1::Active)
            && (now_ms >= lease.deadline_at_ms
                || now_ms.saturating_sub(lease.heartbeat_at_ms) >= SUSPECT_AFTER_MS)
        {
            lease.phase = LeasePhaseV1::Suspect;
        }
        Ok(lease.phase)
    }

    pub(crate) fn nudge(
        &mut self,
        key: &WorkLeaseKeyV1,
        generation: u64,
    ) -> Result<(), LeaseError> {
        transition(
            self.current_mut(key, generation)?,
            LeasePhaseV1::Suspect,
            LeasePhaseV1::Nudged,
        )
    }

    pub(crate) fn inspect(
        &mut self,
        key: &WorkLeaseKeyV1,
        generation: u64,
    ) -> Result<(), LeaseError> {
        transition(
            self.current_mut(key, generation)?,
            LeasePhaseV1::Nudged,
            LeasePhaseV1::Inspected,
        )
    }

    pub(crate) fn replace(
        &mut self,
        key: &WorkLeaseKeyV1,
        generation: u64,
        worker_instance_id: String,
        now_ms: u64,
        deadline_at_ms: u64,
    ) -> Result<WorkerLeaseV1, LeaseError> {
        validate_grant(key, &worker_instance_id, now_ms, deadline_at_ms)?;
        let current = self.leases.get(key).ok_or(LeaseError::Missing)?;
        if current.fencing_generation != generation {
            return Err(LeaseError::StaleGeneration);
        }
        if current.phase != LeasePhaseV1::Inspected {
            return Err(LeaseError::InvalidPhase);
        }
        let next_generation = current
            .fencing_generation
            .checked_add(1)
            .ok_or(LeaseError::CounterExhausted)?;
        let replacement = build_lease(
            key.clone(),
            next_generation,
            worker_instance_id,
            now_ms,
            deadline_at_ms,
            current.checkpoint_revision,
        );
        self.leases.insert(key.clone(), replacement.clone());
        Ok(replacement)
    }

    pub(crate) fn accept_landing(
        &self,
        key: &WorkLeaseKeyV1,
        generation: u64,
    ) -> Result<(), LeaseError> {
        let lease = self.leases.get(key).ok_or(LeaseError::Missing)?;
        if lease.fencing_generation != generation {
            return Err(LeaseError::StaleGeneration);
        }
        if matches!(lease.phase, LeasePhaseV1::Fenced | LeasePhaseV1::Released) {
            return Err(LeaseError::InvalidPhase);
        }
        Ok(())
    }

    pub(crate) fn release(
        &mut self,
        key: &WorkLeaseKeyV1,
        generation: u64,
        checkpoint_revision: u64,
    ) -> Result<(), LeaseError> {
        let lease = self.current_mut(key, generation)?;
        if checkpoint_revision < lease.checkpoint_revision {
            return Err(LeaseError::StaleCheckpoint);
        }
        lease.checkpoint_revision = checkpoint_revision;
        lease.phase = LeasePhaseV1::Released;
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Vec<WorkerLeaseV1> {
        self.leases.values().cloned().collect()
    }

    pub(crate) fn restore(leases: Vec<WorkerLeaseV1>) -> Result<Self, LeaseError> {
        let mut restored = Self::default();
        for lease in leases {
            validate_lease(&lease)?;
            if restored.leases.insert(lease.key.clone(), lease).is_some() {
                return Err(LeaseError::InvalidIdentity);
            }
        }
        Ok(restored)
    }

    pub(crate) fn fence_all_for_import(&mut self) -> Result<(), LeaseError> {
        let mut fenced = self.clone();
        for lease in fenced.leases.values_mut() {
            lease.fencing_generation = lease
                .fencing_generation
                .checked_add(1)
                .ok_or(LeaseError::CounterExhausted)?;
            lease.lease_id = canonical_lease_id(
                &lease.key,
                lease.fencing_generation,
                &lease.worker_instance_id,
                lease.granted_at_ms,
            );
            lease.phase = LeasePhaseV1::Fenced;
        }
        *self = fenced;
        Ok(())
    }

    fn current_mut(
        &mut self,
        key: &WorkLeaseKeyV1,
        generation: u64,
    ) -> Result<&mut WorkerLeaseV1, LeaseError> {
        let lease = self.leases.get_mut(key).ok_or(LeaseError::Missing)?;
        if lease.fencing_generation != generation {
            return Err(LeaseError::StaleGeneration);
        }
        Ok(lease)
    }
}

fn transition(
    lease: &mut WorkerLeaseV1,
    expected: LeasePhaseV1,
    next: LeasePhaseV1,
) -> Result<(), LeaseError> {
    if lease.phase != expected {
        return Err(LeaseError::InvalidPhase);
    }
    lease.phase = next;
    Ok(())
}
