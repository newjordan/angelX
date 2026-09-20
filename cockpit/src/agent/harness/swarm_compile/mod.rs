//! Outcome-coupled coding swarms: intent becomes a durable proof graph whose
//! accepted artifact is a parked branch, never an unverified merge.

mod alignment;
mod credit;
mod finalize;
mod policy;
mod prompts;
mod runner;
mod schema;
mod stages;
mod store;
mod tool;
mod verify;

pub(crate) use tool::{
    AlignmentCriterion, AlignmentIndependence, AlignmentProof, AlignmentVerdict,
    AuthorizedCampaignBase, CampaignAlignmentReceipt, CampaignAlignmentRequest, CampaignBase,
    CampaignCompileRequest, PreparedSwarmRun, SwarmCompilerEngine, SwarmCompilerTool,
    SwarmRunOutcome, SwarmRunReceipt,
};

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/swarm_compile__tests.rs"]
mod tests;
