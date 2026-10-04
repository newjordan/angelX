//! Competition records every cartridge shares: a submission's official
//! status, the identity of the candidate behind it, and terminal evidence.
//! Field names follow the board receipts they were first read from, so
//! state saved before cartridges still loads.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmissionStatus {
    pub(crate) id: String,
    pub(crate) benchmark_id: String,
    pub(crate) status: String,
    pub(crate) promotion_status: Option<String>,
    pub(crate) promoted_source_ref: Option<String>,
    pub(crate) submission_commit_sha: Option<String>,
    pub(crate) official_score: Option<String>,
    #[serde(default)]
    pub(crate) rejection_reason: Option<String>,
    #[serde(default)]
    pub(crate) official_metrics: Value,
    #[serde(default)]
    pub(crate) improved: Option<bool>,
    #[serde(default)]
    pub(crate) frontier: Option<Frontier>,
    pub(crate) fetched_at_ms: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Frontier {
    pub(crate) score: Option<String>,
    pub(crate) source_ref: Option<String>,
    pub(crate) direction: Option<String>,
}

impl SubmissionStatus {
    pub(crate) fn promoted_improvement(&self) -> bool {
        self.status == "accepted"
            && self.improved == Some(true)
            && self.promotion_status.as_deref() == Some("promoted")
            && self.promoted_source_ref.is_some()
            && self.official_score.is_some()
    }
}

pub(crate) fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateIdentity {
    pub(crate) git_commit_id: Option<String>,
    pub(crate) workspace_evidence_sha256: Option<String>,
    pub(crate) source_archive_sha256: Option<String>,
    pub(crate) evaluated_binary_sha256: Option<String>,
    pub(crate) harness_build_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TerminalEvidence {
    #[serde(default)]
    pub(crate) dispatch_owner: Option<String>,
    #[serde(default)]
    pub(crate) official: Option<SubmissionStatus>,
    pub(crate) benchmark_id: String,
    pub(crate) submission_id: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) rejection_reason: Option<String>,
    pub(crate) promotion_status: Option<String>,
    pub(crate) promoted_source_ref: Option<String>,
    pub(crate) submission_commit_id: Option<String>,
    pub(crate) official_score: Option<String>,
    pub(crate) observed_at_ms: Option<u128>,
    pub(crate) candidate_at_dispatch: Option<CandidateIdentity>,
    pub(crate) candidate_commit_matches_receipt: Option<bool>,
    /// True only when terminal receipt and current frontier verification agree.
    /// The exact-ID status endpoint alone cannot establish this.
    pub(crate) frontier_win_verified: bool,
    pub(crate) source_id: String,
}
