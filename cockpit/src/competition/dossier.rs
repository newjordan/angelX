use super::dossier_validation::{
    checkpoint_id, validate_anchor, validate_key, validate_read, validate_text,
};
use super::schema::ScheduledActionV1;
use super::schema_validation::{validate_id, validate_sha256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub(crate) const DOSSIER_SCHEMA_V1: &str = "angel.competition-dossier/v1";
pub(crate) type DossierError = String;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnchorKeyV1 {
    pub(crate) source_id: String,
    pub(crate) canonical_path: String,
    pub(crate) symbol_or_range: String,
    pub(crate) source_revision: String,
    pub(crate) content_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnchorProvenanceV1 {
    pub(crate) kind: String,
    pub(crate) reference: String,
    pub(crate) receipt_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnchorFreshnessV1 {
    Fresh,
    Stale {
        stale_at_revision: u64,
        reason: String,
        reacquire: ScheduledActionV1,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvidenceAnchorV1 {
    pub(crate) anchor_id: String,
    pub(crate) key: AnchorKeyV1,
    pub(crate) provenance: AnchorProvenanceV1,
    pub(crate) confidence_millis: u16,
    pub(crate) admitted_at_revision: u64,
    pub(crate) freshness: AnchorFreshnessV1,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelevantReadV1 {
    pub(crate) key: AnchorKeyV1,
    pub(crate) provenance: AnchorProvenanceV1,
    pub(crate) confidence_millis: u16,
    pub(crate) reacquire: ScheduledActionV1,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContinuationCheckpointV1 {
    pub(crate) checkpoint_id: String,
    pub(crate) dossier_revision: u64,
    pub(crate) turn_id: String,
    pub(crate) journal_head_sha256: String,
    pub(crate) reason: String,
    pub(crate) stale_anchor_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DossierV1 {
    pub(crate) schema: String,
    pub(crate) dossier_id: String,
    pub(crate) project_id: String,
    pub(crate) repository_id: String,
    pub(crate) source_revision: String,
    pub(crate) board_epoch: u64,
    pub(crate) dossier_revision: u64,
    pub(crate) journal_head_sha256: String,
    pub(crate) anchors: BTreeMap<String, EvidenceAnchorV1>,
    pub(crate) checkpoint: Option<ContinuationCheckpointV1>,
}

impl DossierV1 {
    pub(crate) fn new(
        dossier_id: String,
        project_id: String,
        repository_id: String,
        source_revision: String,
        board_epoch: u64,
        journal_head_sha256: String,
    ) -> Result<Self, DossierError> {
        let dossier = Self {
            schema: DOSSIER_SCHEMA_V1.into(),
            dossier_id,
            project_id,
            repository_id,
            source_revision,
            board_epoch,
            dossier_revision: 0,
            journal_head_sha256,
            anchors: BTreeMap::new(),
            checkpoint: None,
        };
        dossier.validate()?;
        Ok(dossier)
    }

    pub(crate) fn admit_relevant(&mut self, read: RelevantReadV1) -> Result<bool, DossierError> {
        validate_read(&read)?;
        let anchor_id = anchor_id(&read.key)?;
        if let Some(anchor) = self.anchors.get(&anchor_id) {
            if anchor.key != read.key {
                return Err("anchor digest collision".into());
            }
            return Ok(false);
        }
        let next_revision = self
            .dossier_revision
            .checked_add(1)
            .ok_or_else(|| "dossier revision exhausted".to_string())?;
        for anchor in self.anchors.values_mut() {
            if anchor.key.source_id == read.key.source_id
                && !matches!(anchor.freshness, AnchorFreshnessV1::Stale { .. })
            {
                anchor.freshness = AnchorFreshnessV1::Stale {
                    stale_at_revision: next_revision,
                    reason: "source revision, target, or digest changed".into(),
                    reacquire: read.reacquire.clone(),
                };
            }
        }
        let anchor = EvidenceAnchorV1 {
            anchor_id: anchor_id.clone(),
            key: read.key,
            provenance: read.provenance,
            confidence_millis: read.confidence_millis,
            admitted_at_revision: next_revision,
            freshness: AnchorFreshnessV1::Fresh,
        };
        self.anchors.insert(anchor_id.clone(), anchor);
        self.dossier_revision = next_revision;
        let continuation = self
            .checkpoint
            .as_ref()
            .map(|checkpoint| (checkpoint.turn_id.clone(), checkpoint.reason.clone()));
        if let Some((turn_id, reason)) = continuation {
            self.install_checkpoint(turn_id, reason)?;
        }
        Ok(true)
    }

    pub(crate) fn rollover_fresh_turn(
        &mut self,
        turn_id: String,
        journal_head_sha256: String,
        reason: String,
    ) -> Result<(), DossierError> {
        validate_id(&turn_id, "invalid turn id").map_err(str::to_string)?;
        validate_sha256(&journal_head_sha256).map_err(str::to_string)?;
        validate_text(&reason, "invalid rollover reason")?;
        let next_dossier = checked_next(self.dossier_revision, "dossier revision")?;
        self.dossier_revision = next_dossier;
        self.journal_head_sha256 = journal_head_sha256;
        self.install_checkpoint(turn_id, reason)?;
        Ok(())
    }

    fn install_checkpoint(&mut self, turn_id: String, reason: String) -> Result<(), DossierError> {
        let stale_anchor_ids = self.stale_anchor_ids();
        let checkpoint_id = checkpoint_id(
            &self.dossier_id,
            self.dossier_revision,
            &turn_id,
            &self.journal_head_sha256,
            &reason,
            &stale_anchor_ids,
        )?;
        self.checkpoint = Some(ContinuationCheckpointV1 {
            checkpoint_id,
            dossier_revision: self.dossier_revision,
            turn_id,
            journal_head_sha256: self.journal_head_sha256.clone(),
            reason,
            stale_anchor_ids,
        });
        Ok(())
    }

    fn stale_anchor_ids(&self) -> Vec<String> {
        self.anchors
            .values()
            .filter(|anchor| matches!(anchor.freshness, AnchorFreshnessV1::Stale { .. }))
            .map(|anchor| anchor.anchor_id.clone())
            .collect()
    }

    pub(crate) fn validate(&self) -> Result<(), DossierError> {
        if self.schema != DOSSIER_SCHEMA_V1 {
            return Err("unknown dossier schema".into());
        }
        validate_id(&self.dossier_id, "invalid dossier id").map_err(str::to_string)?;
        validate_id(&self.project_id, "invalid project id").map_err(str::to_string)?;
        validate_id(&self.repository_id, "invalid repository id").map_err(str::to_string)?;
        validate_id(&self.source_revision, "invalid source revision").map_err(str::to_string)?;
        validate_sha256(&self.journal_head_sha256).map_err(str::to_string)?;
        if self.board_epoch == 0 || self.anchors.len() as u64 > self.dossier_revision {
            return Err("invalid dossier revision order".into());
        }
        for (id, anchor) in &self.anchors {
            validate_anchor(anchor, self.dossier_revision)?;
            if id != &anchor.anchor_id || anchor_id(&anchor.key)? != *id {
                return Err("dossier anchor identity mismatch".into());
            }
        }
        if let Some(checkpoint) = &self.checkpoint {
            validate_text(&checkpoint.reason, "invalid checkpoint reason")?;
            let stale = self.stale_anchor_ids();
            if checkpoint.dossier_revision != self.dossier_revision
                || checkpoint.journal_head_sha256 != self.journal_head_sha256
                || validate_id(&checkpoint.turn_id, "invalid turn id").is_err()
                || checkpoint.stale_anchor_ids != stale
                || checkpoint.checkpoint_id
                    != checkpoint_id(
                        &self.dossier_id,
                        checkpoint.dossier_revision,
                        &checkpoint.turn_id,
                        &checkpoint.journal_head_sha256,
                        &checkpoint.reason,
                        &stale,
                    )?
            {
                return Err("invalid continuation checkpoint".into());
            }
        }
        Ok(())
    }
}

pub(crate) fn anchor_id(key: &AnchorKeyV1) -> Result<String, DossierError> {
    validate_key(key)?;
    serde_json::to_vec(&(DOSSIER_SCHEMA_V1, key))
        .map(|bytes| crate::cut::sha256_hex(&bytes))
        .map_err(|error| format!("encode anchor identity: {error}"))
}

fn checked_next(value: u64, label: &str) -> Result<u64, DossierError> {
    value
        .checked_add(1)
        .ok_or_else(|| format!("{label} exhausted"))
}
