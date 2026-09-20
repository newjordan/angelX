use super::dossier::{
    AnchorFreshnessV1, AnchorKeyV1, AnchorProvenanceV1, DossierError, EvidenceAnchorV1,
    RelevantReadV1,
};
use super::schema_validation::{validate_id, validate_sha256};
use std::collections::BTreeMap;

const MAX_TARGET_BYTES: usize = 4 * 1024;

pub(super) fn validate_read(read: &RelevantReadV1) -> Result<(), DossierError> {
    validate_key(&read.key)?;
    validate_provenance(&read.provenance)?;
    read.reacquire.validate().map_err(str::to_string)?;
    if read.confidence_millis > 1_000 {
        return Err("confidence exceeds 1000".into());
    }
    Ok(())
}

pub(super) fn validate_anchor(
    anchor: &EvidenceAnchorV1,
    dossier_revision: u64,
) -> Result<(), DossierError> {
    validate_key(&anchor.key)?;
    validate_provenance(&anchor.provenance)?;
    if anchor.confidence_millis > 1_000
        || anchor.admitted_at_revision == 0
        || anchor.admitted_at_revision > dossier_revision
    {
        return Err("invalid anchor confidence or revision".into());
    }
    if let AnchorFreshnessV1::Stale {
        stale_at_revision,
        reason,
        reacquire,
    } = &anchor.freshness
    {
        if *stale_at_revision <= anchor.admitted_at_revision
            || *stale_at_revision > dossier_revision
        {
            return Err("invalid stale anchor revision".into());
        }
        validate_text(reason, "invalid stale reason")?;
        reacquire.validate().map_err(str::to_string)?;
    }
    Ok(())
}

pub(super) fn checkpoint_id(
    dossier_id: &str,
    revision: u64,
    turn_id: &str,
    journal_head: &str,
    reason: &str,
    stale: &[String],
) -> Result<String, DossierError> {
    serde_json::to_vec(&(dossier_id, revision, turn_id, journal_head, reason, stale))
        .map(|material| crate::knowledge::cut::sha256_hex(&material))
        .map_err(|error| format!("encode checkpoint: {error}"))
}

pub(super) fn validate_key(key: &AnchorKeyV1) -> Result<(), DossierError> {
    validate_id(&key.source_id, "invalid source id").map_err(str::to_string)?;
    validate_text(&key.canonical_path, "invalid canonical path")?;
    if key.canonical_path.starts_with('/')
        || key.canonical_path.contains('\\')
        || windows_absolute(&key.canonical_path)
        || key
            .canonical_path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("canonical path must be relative and normalized".into());
    }
    validate_text(&key.symbol_or_range, "invalid symbol or range")?;
    validate_id(&key.source_revision, "invalid source revision").map_err(str::to_string)?;
    validate_sha256(&key.content_sha256).map_err(str::to_string)
}

fn windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

pub(super) fn anchor_shards_are_canonical(
    anchors: &BTreeMap<String, (EvidenceAnchorV1, u64)>,
    anchors_per_shard: usize,
) -> bool {
    anchors
        .values()
        .enumerate()
        .all(|(position, (_, shard))| *shard == position as u64 / anchors_per_shard as u64)
}

pub(super) fn state_sha(dossier: &super::dossier::DossierV1) -> Result<String, DossierError> {
    serde_json::to_vec(dossier)
        .map(|body| crate::knowledge::cut::sha256_hex(&body))
        .map_err(|error| format!("encode dossier digest: {error}"))
}

pub(super) fn shard_count(anchor_count: usize) -> u64 {
    anchor_count.div_ceil(64) as u64
}

fn validate_provenance(provenance: &AnchorProvenanceV1) -> Result<(), DossierError> {
    validate_id(&provenance.kind, "invalid provenance kind").map_err(str::to_string)?;
    validate_text(&provenance.reference, "invalid provenance reference")?;
    validate_sha256(&provenance.receipt_sha256).map_err(str::to_string)
}

pub(super) fn validate_text(value: &str, error: &'static str) -> Result<(), DossierError> {
    if value.is_empty()
        || value.len() > MAX_TARGET_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn generation_revision(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if bytes.len() != 85
        || bytes[20] != b'-'
        || !bytes[..20].iter().all(u8::is_ascii_digit)
        || !bytes[21..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return None;
    }
    value[..20].parse().ok()
}
