use super::leases::{
    LeaseBookV1, LeaseError, WORKER_LEASE_SCHEMA_V1, WorkLeaseKeyV1, WorkerLeaseV1,
};
use serde::{Deserialize, Serialize};

pub(super) const LEASE_SNAPSHOT_SCHEMA_V1: &str = "angel.competition-lease-snapshot/v1";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LeaseSnapshotV1 {
    schema: String,
    leases: Vec<WorkerLeaseV1>,
}

pub(super) fn encode(book: &LeaseBookV1) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(&LeaseSnapshotV1 {
        schema: LEASE_SNAPSHOT_SCHEMA_V1.to_string(),
        leases: book.snapshot(),
    })
    .map_err(|error| format!("encode lease snapshot: {error}"))
}

pub(super) fn decode(raw: &[u8]) -> Result<LeaseBookV1, String> {
    let snapshot: LeaseSnapshotV1 =
        serde_json::from_slice(raw).map_err(|error| format!("parse lease snapshot: {error}"))?;
    if snapshot.schema != LEASE_SNAPSHOT_SCHEMA_V1 {
        return Err("unknown lease snapshot schema".into());
    }
    LeaseBookV1::restore(snapshot.leases).map_err(|error| error.to_string())
}

pub(super) fn validate_grant(
    key: &WorkLeaseKeyV1,
    worker_instance_id: &str,
    now_ms: u64,
    deadline_at_ms: u64,
) -> Result<(), LeaseError> {
    if key.campaign_id.is_empty()
        || key.work_item_id.is_empty()
        || worker_instance_id.is_empty()
        || deadline_at_ms <= now_ms
    {
        return Err(LeaseError::InvalidIdentity);
    }
    Ok(())
}

pub(super) fn next_generation(previous: Option<&WorkerLeaseV1>) -> Result<u64, LeaseError> {
    previous.map_or(Ok(1), |lease| {
        lease
            .fencing_generation
            .checked_add(1)
            .ok_or(LeaseError::CounterExhausted)
    })
}

pub(super) fn build_lease(
    key: WorkLeaseKeyV1,
    generation: u64,
    worker_instance_id: String,
    now_ms: u64,
    deadline_at_ms: u64,
    checkpoint_revision: u64,
) -> WorkerLeaseV1 {
    WorkerLeaseV1 {
        schema: WORKER_LEASE_SCHEMA_V1.to_string(),
        lease_id: canonical_lease_id(&key, generation, &worker_instance_id, now_ms),
        key,
        fencing_generation: generation,
        worker_instance_id,
        granted_at_ms: now_ms,
        heartbeat_at_ms: now_ms,
        deadline_at_ms,
        checkpoint_revision,
        phase: super::leases::LeasePhaseV1::Granted,
    }
}

pub(super) fn validate_lease(lease: &WorkerLeaseV1) -> Result<(), LeaseError> {
    validate_grant(
        &lease.key,
        &lease.worker_instance_id,
        lease.granted_at_ms,
        lease.deadline_at_ms,
    )?;
    if lease.schema != WORKER_LEASE_SCHEMA_V1
        || lease.fencing_generation == 0
        || lease.heartbeat_at_ms < lease.granted_at_ms
        || lease.lease_id
            != canonical_lease_id(
                &lease.key,
                lease.fencing_generation,
                &lease.worker_instance_id,
                lease.granted_at_ms,
            )
    {
        return Err(LeaseError::InvalidIdentity);
    }
    Ok(())
}

pub(super) fn canonical_lease_id(
    key: &WorkLeaseKeyV1,
    generation: u64,
    worker_instance_id: &str,
    now_ms: u64,
) -> String {
    let material = serde_json::to_vec(&(key, generation, worker_instance_id, now_ms))
        .expect("worker lease identity is serializable");
    crate::cut::sha256_hex(&material)
}
