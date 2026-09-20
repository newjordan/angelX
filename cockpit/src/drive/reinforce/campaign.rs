//! Framework-owned technical campaign persistence and terminal release commit.

use super::promotion::ReceiptStoreContext;
use super::{BatchMetrics, TechnicalReinforceReport};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const RELEASE_SCHEMA: &str = "angel.rlvr.technical-release/v1";
const CANDIDATE_SCHEMA: &str = "angel.rlvr.frozen-training-proposal/v1";
const MAX_RELEASE_BYTES: u64 = 1024 * 1024;

/// Single framework-owned persistence authority for a technical release
/// campaign. Production callers choose an identity, never arbitrary artifact
/// or ledger paths; selection and audit are consequently forced into one
/// durable uniqueness domain.
#[derive(Clone, Debug)]
pub struct TechnicalCampaignAuthority {
    campaign_id: String,
    artifact_root: PathBuf,
    ledger_root: PathBuf,
}

#[derive(Deserialize, Serialize)]
struct ReleaseCommit {
    schema: String,
    campaign_id: String,
    request_sha256: String,
    receipt_ledger_head_sha256: String,
    promotion_manifest_sha256: String,
    audit_manifest_sha256: String,
    report: TechnicalReinforceReport,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(super) struct FrozenTrainingProposal {
    schema: String,
    campaign_id: String,
    request_sha256: String,
    incumbent_prompt: String,
    incumbent_prompt_sha256: String,
    incumbent_version: u64,
    candidate_prompt: String,
    candidate_prompt_sha256: String,
    metrics: BatchMetrics,
    best_reward_bits: u32,
    record_sha256: String,
}

impl FrozenTrainingProposal {
    pub(super) fn incumbent_prompt(&self) -> &str {
        &self.incumbent_prompt
    }

    pub(super) fn incumbent_version(&self) -> u64 {
        self.incumbent_version
    }

    pub(super) fn candidate_prompt(&self) -> &str {
        &self.candidate_prompt
    }

    pub(super) fn metrics(&self) -> BatchMetrics {
        self.metrics.clone()
    }

    pub(super) fn best_reward(&self) -> f32 {
        f32::from_bits(self.best_reward_bits)
    }
}

/// Root of the durable technical-campaign authorities.
/// `ANGEL_REINFORCE_AUTHORITY_DIR` overrides it so tests and sandboxed runners
/// never write the operator's real store.
pub(crate) fn authority_root() -> PathBuf {
    match std::env::var("ANGEL_REINFORCE_AUTHORITY_DIR") {
        Ok(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => crate::platform::workspace_store::angel_subdir("reinforce-authority"),
    }
}

impl TechnicalCampaignAuthority {
    pub fn new(campaign_id: impl Into<String>) -> Result<Self, String> {
        let campaign_id = campaign_id.into();
        Self::validate_id(&campaign_id)?;
        let directory = crate::knowledge::cut::sha256_hex(campaign_id.as_bytes());
        Self::at_root(campaign_id, authority_root().join(directory))
    }

    #[cfg(test)]
    pub(crate) fn new_in(campaign_id: impl Into<String>, root: PathBuf) -> Result<Self, String> {
        let campaign_id = campaign_id.into();
        Self::validate_id(&campaign_id)?;
        Self::at_root(campaign_id, root)
    }

    fn at_root(campaign_id: String, root: PathBuf) -> Result<Self, String> {
        Ok(Self {
            campaign_id,
            artifact_root: root.join("artifacts"),
            ledger_root: root.join("ledger"),
        })
    }

    fn validate_id(campaign_id: &str) -> Result<(), String> {
        if campaign_id.is_empty()
            || campaign_id.len() > 256
            || campaign_id.contains(['\n', '\r', '\0'])
        {
            return Err("technical campaign id is invalid".into());
        }
        Ok(())
    }

    pub(super) fn receipt_store(&self) -> ReceiptStoreContext<'_> {
        ReceiptStoreContext {
            artifact_root: &self.artifact_root,
            ledger_root: &self.ledger_root,
            run_id: &self.campaign_id,
            reproduction_id: "authoritative-campaign",
        }
    }

    /// Receipt store for a measured exploration pass: the same authority-owned
    /// artifact and ledger roots the release path uses, so an exploration
    /// verdict is run through the identical durable receipt machinery.
    pub(crate) fn exploration_receipt_store(&self) -> ReceiptStoreContext<'_> {
        self.receipt_store()
    }

    pub(super) fn acquire_execution(
        &self,
    ) -> Result<super::consumption::CampaignExecutionLock, String> {
        super::consumption::acquire_campaign_execution_lock(&self.ledger_root)
    }

    pub(super) fn bind_request(&self, request_sha256: &str) -> Result<(), String> {
        let record = format!(
            "angel.rlvr.technical-campaign/v1\n{}:{}\n{}:{}\n",
            self.campaign_id.len(),
            self.campaign_id,
            request_sha256.len(),
            request_sha256
        );
        super::consumption::bind_campaign_authority(&self.ledger_root, record.as_bytes())
    }

    pub(super) fn try_load_candidate(
        &self,
        request_sha256: &str,
        incumbent_prompt: &str,
        incumbent_version: u64,
    ) -> Result<Option<FrozenTrainingProposal>, String> {
        super::consumption::load_campaign_candidate(&self.ledger_root)?
            .map(|bytes| {
                Self::decode_candidate(
                    &bytes,
                    &self.campaign_id,
                    request_sha256,
                    incumbent_prompt,
                    incumbent_version,
                )
            })
            .transpose()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn install_or_adopt_candidate(
        &self,
        request_sha256: &str,
        incumbent_prompt: &str,
        incumbent_version: u64,
        candidate_prompt: &str,
        metrics: BatchMetrics,
        best_reward: f32,
    ) -> Result<FrozenTrainingProposal, String> {
        let mut record = FrozenTrainingProposal {
            schema: CANDIDATE_SCHEMA.to_string(),
            campaign_id: self.campaign_id.clone(),
            request_sha256: request_sha256.to_string(),
            incumbent_prompt: incumbent_prompt.to_string(),
            incumbent_prompt_sha256: crate::knowledge::cut::sha256_hex(incumbent_prompt.as_bytes()),
            incumbent_version,
            candidate_prompt: candidate_prompt.to_string(),
            candidate_prompt_sha256: crate::knowledge::cut::sha256_hex(candidate_prompt.as_bytes()),
            metrics,
            best_reward_bits: best_reward.to_bits(),
            record_sha256: String::new(),
        };
        record.record_sha256 = Self::candidate_record_sha256(&record)?;
        let mut proposed = serde_json::to_vec(&record)
            .map_err(|error| format!("could not encode frozen training proposal: {error}"))?;
        proposed.push(b'\n');
        let adopted =
            super::consumption::install_or_adopt_campaign_candidate(&self.ledger_root, &proposed)?;
        Self::decode_candidate(
            &adopted,
            &self.campaign_id,
            request_sha256,
            incumbent_prompt,
            incumbent_version,
        )
    }

    fn decode_candidate(
        bytes: &[u8],
        campaign_id: &str,
        request_sha256: &str,
        incumbent_prompt: &str,
        incumbent_version: u64,
    ) -> Result<FrozenTrainingProposal, String> {
        let record: FrozenTrainingProposal = serde_json::from_slice(bytes)
            .map_err(|error| format!("could not decode frozen training proposal: {error}"))?;
        if record.schema != CANDIDATE_SCHEMA
            || record.campaign_id != campaign_id
            || record.request_sha256 != request_sha256
            || record.incumbent_prompt != incumbent_prompt
            || record.incumbent_version != incumbent_version
            || record.incumbent_prompt_sha256
                != crate::knowledge::cut::sha256_hex(record.incumbent_prompt.as_bytes())
            || record.candidate_prompt_sha256
                != crate::knowledge::cut::sha256_hex(record.candidate_prompt.as_bytes())
            || record.candidate_prompt.trim().is_empty()
            || record.candidate_prompt.trim() == record.incumbent_prompt.trim()
            || record.record_sha256 != Self::candidate_record_sha256(&record)?
        {
            return Err("frozen training proposal authority context mismatch".into());
        }
        Ok(record)
    }

    fn candidate_record_sha256(record: &FrozenTrainingProposal) -> Result<String, String> {
        let mut unsigned = record.clone();
        unsigned.record_sha256.clear();
        serde_json::to_vec(&unsigned)
            .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
            .map_err(|error| format!("could not hash frozen training proposal: {error}"))
    }

    pub(super) fn commit_release(
        &self,
        request_sha256: &str,
        receipt_ledger_head_sha256: &str,
        report: &TechnicalReinforceReport,
    ) -> Result<(), String> {
        let release = report
            .release_candidate()
            .ok_or_else(|| "terminal release report has no release candidate".to_string())?;
        let record = ReleaseCommit {
            schema: RELEASE_SCHEMA.to_string(),
            campaign_id: self.campaign_id.clone(),
            request_sha256: request_sha256.to_string(),
            receipt_ledger_head_sha256: receipt_ledger_head_sha256.to_string(),
            promotion_manifest_sha256: release.promotion_manifest_sha256.clone(),
            audit_manifest_sha256: release.audit_manifest_sha256.clone(),
            report: report.clone(),
        };
        let mut bytes = serde_json::to_vec(&record)
            .map_err(|error| format!("could not encode technical release commit: {error}"))?;
        bytes.push(b'\n');
        if bytes.len() as u64 > MAX_RELEASE_BYTES {
            return Err("technical release commit exceeds its bounded size".into());
        }
        super::consumption::commit_terminal_release(
            &self.ledger_root,
            receipt_ledger_head_sha256,
            &bytes,
        )
        .map(|_| ())
    }

    pub(super) fn try_load_release(
        &self,
        request_sha256: &str,
        promotion_manifest_sha256: &str,
        audit_manifest_sha256: &str,
    ) -> Result<Option<(TechnicalReinforceReport, String)>, String> {
        let Some(snapshot) = super::consumption::load_terminal_release(&self.ledger_root)? else {
            return Ok(None);
        };
        let record: ReleaseCommit = serde_json::from_slice(&snapshot.bytes)
            .map_err(|error| format!("could not decode terminal release: {error}"))?;
        if record.schema != RELEASE_SCHEMA
            || record.campaign_id != self.campaign_id
            || record.request_sha256 != request_sha256
            || record.promotion_manifest_sha256 != promotion_manifest_sha256
            || record.audit_manifest_sha256 != audit_manifest_sha256
            || record.receipt_ledger_head_sha256 != snapshot.receipt_ledger_head_sha256
        {
            return Err(
                "immutable campaign record drift: terminal release authority context mismatch"
                    .into(),
            );
        }
        let release = record
            .report
            .release_candidate()
            .ok_or_else(|| "terminal release report has no release candidate".to_string())?;
        if release.promotion_manifest_sha256() != promotion_manifest_sha256
            || release.audit_manifest_sha256() != audit_manifest_sha256
        {
            return Err("terminal release report manifest mismatch".into());
        }
        Ok(Some((record.report, snapshot.receipt_ledger_head_sha256)))
    }

    /// Return a caller-retainable digest of the complete, validated terminal
    /// release record. Keeping this value outside the campaign authority lets
    /// a deployment controller detect whole-authority rollback or replacement
    /// before accepting a previously published release.
    pub fn release_anchor_sha256(&self) -> Result<String, String> {
        let snapshot = self.validated_terminal_snapshot()?;
        Ok(crate::knowledge::cut::sha256_hex(&snapshot.bytes))
    }

    /// Verify the complete terminal release against an anchor retained by the
    /// caller outside this authority directory.
    pub fn verify_release_anchor(&self, expected_sha256: &str) -> Result<(), String> {
        if !Self::valid_sha256(expected_sha256) {
            return Err("external release anchor must be a lowercase SHA-256 digest".into());
        }
        let observed_sha256 = self.release_anchor_sha256()?;
        if observed_sha256 != expected_sha256 {
            return Err(format!(
                "external release anchor mismatch: expected {expected_sha256}, observed {observed_sha256}"
            ));
        }
        Ok(())
    }

    fn validated_terminal_snapshot(
        &self,
    ) -> Result<super::consumption::TerminalReleaseSnapshot, String> {
        let snapshot = super::consumption::load_terminal_release(&self.ledger_root)?
            .ok_or_else(|| "external release anchor requires a terminal release".to_string())?;
        let record: ReleaseCommit = serde_json::from_slice(&snapshot.bytes)
            .map_err(|error| format!("could not decode terminal release: {error}"))?;
        if record.schema != RELEASE_SCHEMA
            || record.campaign_id != self.campaign_id
            || !Self::valid_sha256(&record.request_sha256)
            || !Self::valid_sha256(&record.promotion_manifest_sha256)
            || !Self::valid_sha256(&record.audit_manifest_sha256)
            || record.receipt_ledger_head_sha256 != snapshot.receipt_ledger_head_sha256
        {
            return Err(
                "immutable campaign record drift: terminal release authority context mismatch"
                    .into(),
            );
        }
        let final_audit = record
            .report
            .final_audit()
            .ok_or_else(|| "terminal release has no final audit report".to_string())?;
        let expected_release = super::derive_release_candidate(
            record.report.reinforcement(),
            final_audit,
            &record.campaign_id,
            &snapshot.receipt_ledger_head_sha256,
            &record.promotion_manifest_sha256,
            &record.audit_manifest_sha256,
        )?;
        if record.report.release_candidate() != Some(&expected_release) {
            return Err("terminal release candidate digest mismatch".into());
        }
        Ok(snapshot)
    }

    fn valid_sha256(value: &str) -> bool {
        value.len() == 64
            && value
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    }

    #[cfg(test)]
    pub(crate) fn ledger_root(&self) -> PathBuf {
        self.ledger_root.clone()
    }

    #[cfg(test)]
    pub(crate) fn release_path(&self) -> PathBuf {
        self.ledger_root.join("terminal-release.json")
    }
}
