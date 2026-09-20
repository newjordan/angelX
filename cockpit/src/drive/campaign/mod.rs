//! Native, project-bound proof-campaign state.
//!
//! Authority stays in deterministic Rust: the controller authors and persists
//! contracts, while explicit operator advancement hands one frozen round to
//! the shared swarm-compiler service. Technical success parks evidence for a
//! separate alignment gate; it never merges or mutates a live checkout.

mod controller;
mod lens;
mod schema;
mod store;

pub(crate) use controller::{CampaignCommand, CampaignController};
pub(crate) use lens::replace_lens_message;
#[allow(unused_imports)]
pub(crate) use schema::{
    AcceptanceCriterion, CampaignRecord, CampaignStatus, CriterionId, CriterionStatus,
    ExecutionRequest, NetworkPolicy, ProofRef, RoundContract, RoundReceipt, RoundState,
};

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/campaign__tests.rs"]
mod tests;
