use super::episode::{EpisodeEventKindV1, EpisodeEventV1, EpisodeStartedV1};
use super::portability::PortableEntryV1;
use super::profile::EpisodeSourceLineageV1;
use super::schema_validation::{validate_id, validate_sha256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const PORTABLE_OBSERVATION_SCHEMA_V1: &str = "angel.competition-portable-observation/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableObservationReceiptV1 {
    schema: String,
    episode_id: String,
    campaign_id: String,
    board_epoch: u64,
    observation_revision: u64,
    raw_sha256: String,
    source: EpisodeSourceLineageV1,
    start_action_event_sha256: String,
    receipt_sha256: String,
}

pub(super) fn build_observation_history(
    entries: &[PortableEntryV1],
) -> Result<Vec<PortableObservationReceiptV1>, String> {
    let mut receipts = entries
        .iter()
        .filter(|entry| entry.path.starts_with("episodes/"))
        .map(|entry| receipt_from_start(&read_start(&entry.body)?))
        .collect::<Result<Vec<_>, _>>()?;
    receipts.sort_by(|left, right| left.episode_id.cmp(&right.episode_id));
    validate_unique(&receipts)?;
    Ok(receipts)
}

pub(super) fn validate_history_ids(
    receipts: &[PortableObservationReceiptV1],
    episode_ids: &BTreeSet<String>,
) -> Result<(), String> {
    validate_unique(receipts)?;
    if receipts
        .iter()
        .map(|receipt| receipt.episode_id.clone())
        .collect::<BTreeSet<_>>()
        != *episode_ids
    {
        return Err("portable observation history does not match episodes".into());
    }
    Ok(())
}

pub(super) fn validate_start_receipt(
    receipts: &[PortableObservationReceiptV1],
    start: &EpisodeStartedV1,
) -> Result<(), String> {
    let expected = receipt_from_start(start)?;
    if receipts
        .iter()
        .find(|receipt| receipt.episode_id == start.episode_id)
        != Some(&expected)
    {
        return Err("episode start lacks exact historical observation evidence".into());
    }
    Ok(())
}

fn receipt_from_start(start: &EpisodeStartedV1) -> Result<PortableObservationReceiptV1, String> {
    start.validate()?;
    let mut receipt = PortableObservationReceiptV1 {
        schema: PORTABLE_OBSERVATION_SCHEMA_V1.into(),
        episode_id: start.episode_id.clone(),
        campaign_id: start.campaign_id.clone(),
        board_epoch: start.board_epoch,
        observation_revision: start.board_observation_revision,
        raw_sha256: start.source.observation_sha256.clone(),
        source: start.source.clone(),
        start_action_event_sha256: start.start_journal_event.event_sha256.clone(),
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = receipt.canonical_sha256()?;
    Ok(receipt)
}

fn validate_unique(receipts: &[PortableObservationReceiptV1]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for receipt in receipts {
        if receipt.schema != PORTABLE_OBSERVATION_SCHEMA_V1
            || !ids.insert(&receipt.episode_id)
            || validate_id(&receipt.episode_id, "invalid observation episode").is_err()
            || validate_id(&receipt.campaign_id, "invalid observation campaign").is_err()
            || receipt.board_epoch == 0
            || receipt.observation_revision == 0
            || receipt.raw_sha256 != receipt.source.observation_sha256
            || validate_sha256(&receipt.raw_sha256).is_err()
            || validate_sha256(&receipt.start_action_event_sha256).is_err()
            || receipt.receipt_sha256 != receipt.canonical_sha256()?
        {
            return Err("invalid portable observation receipt".into());
        }
    }
    Ok(())
}

impl PortableObservationReceiptV1 {
    fn canonical_sha256(&self) -> Result<String, String> {
        let mut value = self.clone();
        value.receipt_sha256.clear();
        serde_json::to_vec(&value)
            .map(|body| crate::cut::sha256_hex(&body))
            .map_err(|error| format!("encode portable observation receipt: {error}"))
    }
}

fn read_start(raw: &[u8]) -> Result<EpisodeStartedV1, String> {
    let line = raw
        .split(|byte| *byte == b'\n')
        .next()
        .filter(|line| !line.is_empty())
        .ok_or_else(|| "portable episode lacks start evidence".to_string())?;
    let event: EpisodeEventV1 = serde_json::from_slice(line)
        .map_err(|error| format!("parse portable episode start: {error}"))?;
    match event.event {
        EpisodeEventKindV1::Started(start) if event.sequence == 0 => Ok(*start),
        _ => Err("portable episode first event is not Started".into()),
    }
}

#[cfg(test)]
pub(super) fn reseal_json_for_test(value: &mut serde_json::Value) {
    let mut receipt: PortableObservationReceiptV1 = serde_json::from_value(value.clone()).unwrap();
    receipt.receipt_sha256 = receipt.canonical_sha256().unwrap();
    *value = serde_json::to_value(receipt).unwrap();
}

#[cfg(test)]
pub(super) fn apply_unchanged_refresh(
    director: &mut super::director::CompetitionDirectorStateV1,
    board: &super::board::CanonicalBoardV1,
) -> u64 {
    let mut refreshed = board.clone();
    refreshed.observation_revision += 1;
    refreshed.latest_observation_id = "observation-refresh".into();
    refreshed.latest_provenance.observed_at_ms = 14;
    refreshed.latest_provenance.raw_sha256 = crate::cut::sha256_hex(b"raw-observation-refresh");
    refreshed
        .frontier
        .source_accessible_base
        .as_mut()
        .unwrap()
        .selected_from_observation_id = refreshed.latest_observation_id.clone();
    refreshed.decision_sha256 = refreshed
        .frontier
        .decision_sha256(&refreshed.comparator)
        .unwrap();
    director
        .apply_board_outcome(
            &super::director_tests::outcome(
                refreshed.clone(),
                super::board::BoardReduceEffectV1::DecisionRefreshed,
            ),
            14,
        )
        .unwrap();
    refreshed.observation_revision
}
