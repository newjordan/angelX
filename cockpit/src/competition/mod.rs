//! Durable, always-running competition coordination.
//!
//! This subsystem is intentionally separate from [`crate::campaign`], whose
//! operator-gated proof states have different termination semantics.

pub(crate) mod adapters;
pub(crate) mod board;
pub(crate) mod board_reducer;
pub(crate) mod board_sync;
pub(crate) mod candidate;
mod candidate_lineage;
mod candidate_store;
pub(crate) mod deep_cut_policy;
pub(crate) mod director;
mod director_board;
mod director_board_validation;
mod director_results;
pub(crate) mod director_services;
pub(crate) mod director_store;
mod director_validation;
pub(crate) mod dossier;
mod dossier_store;
mod dossier_validation;
pub(crate) mod episode;
mod episode_reducer;
mod episode_store;
pub(crate) mod frontier;
mod full_portability;
mod full_portability_io;
mod full_portability_observations;
pub(crate) mod journal;
mod lease_store;
pub(crate) mod leases;
pub(crate) mod migration;
mod migration_qualification;
mod migration_state;
pub(crate) mod patterns;
mod patterns_reducer;
pub(crate) mod portability;
mod portability_io;
pub(crate) mod profile;
pub(crate) mod recovery;
pub(crate) mod rewards;
mod rewards_decimal;
mod rewards_validation;
mod runtime_authorization;
mod runtime_evidence;
mod runtime_identity;
pub(crate) mod runtime_schema;
pub(crate) mod scheduler;
mod scheduler_validation;
pub(crate) mod schema;
mod schema_validation;
pub(crate) mod store;
mod store_fs;
mod submission;
mod submission_pump;
mod submission_reconcile;
mod submission_results;
mod submission_store;
pub(crate) mod worker_policy;

#[cfg(test)]
mod candidate_store_tests;
#[cfg(test)]
mod candidate_tests;
#[cfg(test)]
mod candidate_validation_tests;
#[cfg(test)]
mod cutpoint_tests;
#[cfg(test)]
mod deep_cut_policy_tests;
#[cfg(test)]
mod director_refresh_tests;
#[cfg(test)]
mod director_restore_tests;
#[cfg(test)]
mod director_results_tests;
#[cfg(test)]
mod director_store_tests;
#[cfg(test)]
mod director_tests;
#[cfg(test)]
mod dossier_store_tests;
#[cfg(test)]
mod dossier_tests;
#[cfg(test)]
mod episode_reducer_tests;
#[cfg(test)]
mod full_portability_episode_fixture;
#[cfg(test)]
mod full_portability_episode_tests;
#[cfg(test)]
mod full_portability_tests;
#[cfg(test)]
mod migration_tests;
#[cfg(test)]
mod patterns_adversarial_tests;
#[cfg(test)]
mod patterns_tests;
#[cfg(test)]
mod portability_review_tests;
#[cfg(test)]
mod portability_tests;
#[cfg(test)]
mod rewards_adversarial_tests;
#[cfg(test)]
mod rewards_tests;
#[cfg(test)]
mod runtime_authorization_tests;
#[cfg(test)]
mod runtime_schema_tests;
#[cfg(test)]
mod scheduler_soak_tests;
#[cfg(test)]
mod scheduler_tests;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod schema_validation_tests;
#[cfg(test)]
mod submission_cutpoint_tests;
#[cfg(test)]
mod submission_origin_tests;
#[cfg(test)]
mod submission_store_tests;
#[cfg(test)]
mod submission_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod worker_policy_tests;
