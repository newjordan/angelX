//! Ordered semantic policy-call capture at the harness-owned model boundary.
//!
//! The recorder is deliberately provider-independent: it accepts only the
//! `ChatMsg`/`ToolDef` request and `ClubReply` action already visible to the
//! harness. Private reasoning stream deltas, provider HTTP bodies, credentials,
//! and headers are outside its API and cannot enter a rollout accidentally.

mod audit_export;
mod recorder;
mod schema;
mod store;

#[allow(unused_imports)]
pub(crate) use audit_export::{
    AuditedRollout, audit_delegate_rollout_receipt, audit_workspace_rollout,
    audit_workspace_rollout_receipt, export_workspace_rollout_corpus_v2,
    export_workspace_rollout_v2,
};
pub(crate) use recorder::RolloutRecorder;
#[allow(unused_imports)]
pub(crate) use schema::{RewardOwner, RewardReceipt, TaskRolloutBindingV1};

#[cfg(test)]
pub(crate) use store::RolloutStore;

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/rollout__tests.rs"]
mod tests;
