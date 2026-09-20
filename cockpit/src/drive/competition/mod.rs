//! Durable, always-running competition coordination.
//!
//! This subsystem is intentionally separate from [`crate::drive::campaign`], whose
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
#[path = "../../../../tests/cockpit/competition/candidate_store_tests.rs"]
mod candidate_store_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/candidate_tests.rs"]
mod candidate_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/candidate_validation_tests.rs"]
mod candidate_validation_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/cutpoint_tests.rs"]
mod cutpoint_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/deep_cut_policy_tests.rs"]
mod deep_cut_policy_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/director_refresh_tests.rs"]
mod director_refresh_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/director_restore_tests.rs"]
mod director_restore_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/director_results_tests.rs"]
mod director_results_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/director_store_tests.rs"]
mod director_store_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/director_tests.rs"]
mod director_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/dossier_store_tests.rs"]
mod dossier_store_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/dossier_tests.rs"]
mod dossier_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/episode_reducer_tests.rs"]
mod episode_reducer_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/full_portability_episode_fixture.rs"]
mod full_portability_episode_fixture;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/full_portability_episode_tests.rs"]
mod full_portability_episode_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/full_portability_tests.rs"]
mod full_portability_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/migration_tests.rs"]
mod migration_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/patterns_adversarial_tests.rs"]
mod patterns_adversarial_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/patterns_tests.rs"]
mod patterns_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/portability_review_tests.rs"]
mod portability_review_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/portability_tests.rs"]
mod portability_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/rewards_adversarial_tests.rs"]
mod rewards_adversarial_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/rewards_tests.rs"]
mod rewards_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/runtime_authorization_tests.rs"]
mod runtime_authorization_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/runtime_schema_tests.rs"]
mod runtime_schema_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/scheduler_soak_tests.rs"]
mod scheduler_soak_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/scheduler_tests.rs"]
mod scheduler_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/schema_tests.rs"]
mod schema_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/schema_validation_tests.rs"]
mod schema_validation_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/submission_cutpoint_tests.rs"]
mod submission_cutpoint_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/submission_origin_tests.rs"]
mod submission_origin_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/submission_store_tests.rs"]
mod submission_store_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/submission_tests.rs"]
mod submission_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/test_support__mod.rs"]
mod test_support;
#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/worker_policy_tests.rs"]
mod worker_policy_tests;
