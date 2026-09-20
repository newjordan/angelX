use super::schema::{CompetitionKeyV1, ObjectiveComparatorV1, ScoreV1};
use super::schema_validation::{validate_id, validate_sha256};
use super::submission::{SubmissionItemStateV1, SubmissionItemV1};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const OFFICIAL_RESULT_SCHEMA_V1: &str = "angel.competition-official-result/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialResultV1 {
    pub(crate) schema: String,
    pub(crate) result_id: String,
    pub(crate) submission_id: String,
    pub(crate) candidate_id: String,
    pub(crate) platform_submission_id: String,
    pub(crate) result_revision: u64,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) candidate_score: ScoreV1,
    pub(crate) receipt_sha256: String,
    pub(crate) corrects_result_id: Option<String>,
    pub(crate) record_sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialResultLedgerV1 {
    pub(crate) results: Vec<OfficialResultV1>,
    pub(crate) latest_by_submission: BTreeMap<String, String>,
}

impl OfficialResultV1 {
    pub(crate) fn new(
        item: &SubmissionItemV1,
        result_id: &str,
        result_revision: u64,
        candidate_score: ScoreV1,
        receipt_sha256: String,
        corrects_result_id: Option<String>,
    ) -> Result<Self, String> {
        let platform_submission_id = match &item.state {
            SubmissionItemStateV1::Acknowledged {
                platform_submission_id,
                ..
            } => platform_submission_id.clone(),
            _ => return Err("official result requires acknowledged submission".into()),
        };
        let mut result = Self {
            schema: OFFICIAL_RESULT_SCHEMA_V1.into(),
            result_id: result_id.into(),
            submission_id: item.submission_id.clone(),
            candidate_id: item.candidate_id.clone(),
            platform_submission_id,
            result_revision,
            competition: item.competition.clone(),
            objective: item.objective.clone(),
            candidate_score,
            receipt_sha256,
            corrects_result_id,
            record_sha256: String::new(),
        };
        result.record_sha256 = result.canonical_sha256()?;
        result.validate_record()?;
        Ok(result)
    }

    fn validate_record(&self) -> Result<(), String> {
        if self.schema != OFFICIAL_RESULT_SCHEMA_V1
            || self.result_revision == 0
            || self.record_sha256 != self.canonical_sha256()?
        {
            return Err("invalid official result schema, revision, or digest".into());
        }
        for (id, label) in [
            (&self.result_id, "invalid official result id"),
            (&self.submission_id, "invalid submission id"),
            (&self.candidate_id, "invalid official candidate id"),
            (
                &self.platform_submission_id,
                "invalid platform submission id",
            ),
        ] {
            validate_id(id, label).map_err(str::to_string)?;
        }
        if let Some(parent) = &self.corrects_result_id {
            validate_id(parent, "invalid corrected result id").map_err(str::to_string)?;
        }
        self.competition.validate().map_err(str::to_string)?;
        self.objective.validate().map_err(str::to_string)?;
        validate_sha256(&self.receipt_sha256).map_err(str::to_string)
    }

    fn canonical_sha256(&self) -> Result<String, String> {
        let mut canonical = self.clone();
        canonical.record_sha256.clear();
        serde_json::to_vec(&canonical)
            .map(|bytes| crate::cut::sha256_hex(&bytes))
            .map_err(|error| format!("encode official result: {error}"))
    }
}

impl OfficialResultLedgerV1 {
    pub(super) fn bind(
        &mut self,
        items: &BTreeMap<u64, SubmissionItemV1>,
        result: OfficialResultV1,
    ) -> Result<bool, String> {
        self.validate(items)?;
        result.validate_record()?;
        if let Some(existing) = self
            .results
            .iter()
            .find(|existing| existing.result_id == result.result_id)
        {
            return if existing == &result {
                Ok(false)
            } else {
                Err("official result id conflicts with durable evidence".into())
            };
        }
        if self
            .results
            .iter()
            .any(|existing| existing.receipt_sha256 == result.receipt_sha256)
        {
            return Err("official result receipt is already bound".into());
        }
        validate_binding(items, &result)?;
        validate_revision(self, &result)?;
        self.latest_by_submission
            .insert(result.submission_id.clone(), result.result_id.clone());
        self.results.push(result);
        self.validate(items)?;
        Ok(true)
    }

    pub(super) fn validate(&self, items: &BTreeMap<u64, SubmissionItemV1>) -> Result<(), String> {
        let mut rebuilt = BTreeMap::new();
        let mut ids = BTreeSet::new();
        let mut receipts = BTreeSet::new();
        for result in &self.results {
            result.validate_record()?;
            if !ids.insert(&result.result_id) || !receipts.insert(&result.receipt_sha256) {
                return Err("duplicate official result id".into());
            }
            validate_binding(items, result)?;
            let previous = rebuilt
                .get(&result.submission_id)
                .and_then(|id| self.results.iter().find(|prior| &prior.result_id == id));
            match previous {
                None if result.result_revision == 1 && result.corrects_result_id.is_none() => {}
                Some(prior)
                    if prior.result_revision.checked_add(1) == Some(result.result_revision)
                        && result.corrects_result_id.as_deref()
                            == Some(prior.result_id.as_str()) => {}
                _ => return Err("official result correction chain is not append-only".into()),
            }
            rebuilt.insert(result.submission_id.clone(), result.result_id.clone());
        }
        if rebuilt != self.latest_by_submission {
            return Err("official result latest revision index mismatch".into());
        }
        Ok(())
    }
}

fn validate_binding(
    items: &BTreeMap<u64, SubmissionItemV1>,
    result: &OfficialResultV1,
) -> Result<(), String> {
    let item = items
        .values()
        .find(|item| item.submission_id == result.submission_id)
        .ok_or_else(|| "official result submission is absent".to_string())?;
    let platform = match &item.state {
        SubmissionItemStateV1::Acknowledged {
            platform_submission_id,
            ..
        } => platform_submission_id,
        _ => return Err("official result submission is not acknowledged".into()),
    };
    if result.candidate_id != item.candidate_id
        || result.platform_submission_id != *platform
        || result.competition != item.competition
        || result.objective != item.objective
    {
        return Err("official result is not candidate-bound".into());
    }
    Ok(())
}

fn validate_revision(
    ledger: &OfficialResultLedgerV1,
    result: &OfficialResultV1,
) -> Result<(), String> {
    let previous = ledger
        .latest_by_submission
        .get(&result.submission_id)
        .and_then(|id| ledger.results.iter().find(|prior| &prior.result_id == id));
    match previous {
        None if result.result_revision == 1 && result.corrects_result_id.is_none() => Ok(()),
        Some(prior)
            if prior.result_revision.checked_add(1) == Some(result.result_revision)
                && result.corrects_result_id.as_deref() == Some(prior.result_id.as_str()) =>
        {
            Ok(())
        }
        _ => Err("official result correction revision is invalid".into()),
    }
}
