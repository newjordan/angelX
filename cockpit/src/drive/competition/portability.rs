use super::dossier_store::DossierStoreV1;
use super::dossier_validation::generation_revision;
use super::portability_io::{collect_entries, install_staging};
use super::recovery::recover_store;
use super::schema::COMPETITION_CONTRACT_V1;
use super::schema_validation::{validate_id, validate_sha256};
use super::store::CompetitionStore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub(crate) const PORTABLE_STATE_SCHEMA_V1: &str = "angel.competition-portable-state/v1";
const MAX_BUNDLE_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 16_384;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableEntryV1 {
    pub(crate) path: String,
    pub(crate) content_sha256: String,
    pub(crate) body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableStateBundleV1 {
    schema: String,
    contract: String,
    campaign_id: String,
    entries: Vec<PortableEntryV1>,
    bundle_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PortableImportReceiptV1 {
    pub(crate) campaign_id: String,
    pub(crate) bundle_sha256: String,
    pub(crate) journal_head_sha256: String,
    pub(crate) dossier_revision: u64,
}

pub(crate) fn export_portable_state(
    source_root: &Path,
    campaign_id: &str,
) -> Result<Vec<u8>, String> {
    validate_id(campaign_id, "invalid campaign id").map_err(str::to_string)?;
    let action_root = source_root.join("action");
    let dossier_root = source_root.join("dossier");
    validate_components(&action_root, &dossier_root, campaign_id)?;
    let generation = current_generation(&dossier_root)?;
    let entries = collect_entries(source_root, &generation)?;
    validate_entries(&entries)?;
    let mut bundle = PortableStateBundleV1 {
        schema: PORTABLE_STATE_SCHEMA_V1.into(),
        contract: COMPETITION_CONTRACT_V1.into(),
        campaign_id: campaign_id.into(),
        entries,
        bundle_sha256: String::new(),
    };
    bundle.bundle_sha256 = bundle_sha(&bundle)?;
    let body =
        serde_json::to_vec(&bundle).map_err(|error| format!("encode portable state: {error}"))?;
    if body.len() > MAX_BUNDLE_BYTES {
        return Err("portable state bundle exceeds bound".into());
    }
    Ok(body)
}

pub(crate) fn import_portable_state(
    raw: &[u8],
    target_root: &Path,
) -> Result<PortableImportReceiptV1, String> {
    if raw.len() > MAX_BUNDLE_BYTES {
        return Err("portable state bundle exceeds bound".into());
    }
    let bundle: PortableStateBundleV1 =
        serde_json::from_slice(raw).map_err(|error| format!("parse portable state: {error}"))?;
    validate_bundle(&bundle)?;
    let staging = install_staging(target_root, &bundle.entries)?;
    let result = (|| {
        let action_root = staging.join("action");
        let dossier_root = staging.join("dossier");
        let (journal_head_sha256, dossier_revision) =
            validate_components(&action_root, &dossier_root, &bundle.campaign_id)?;
        let action_store = CompetitionStore::new(action_root);
        action_store.update_leases(|leases| leases.fence_all_for_import())?;
        validate_components(
            &staging.join("action"),
            &staging.join("dossier"),
            &bundle.campaign_id,
        )?;
        Ok(PortableImportReceiptV1 {
            campaign_id: bundle.campaign_id.clone(),
            bundle_sha256: bundle.bundle_sha256.clone(),
            journal_head_sha256,
            dossier_revision,
        })
    })();
    match result {
        Ok(receipt) => {
            super::portability_io::publish_staging(&staging, target_root)?;
            Ok(receipt)
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(error)
        }
    }
}

fn validate_bundle(bundle: &PortableStateBundleV1) -> Result<(), String> {
    if bundle.schema != PORTABLE_STATE_SCHEMA_V1
        || bundle.contract != COMPETITION_CONTRACT_V1
        || validate_id(&bundle.campaign_id, "invalid campaign id").is_err()
        || validate_sha256(&bundle.bundle_sha256).is_err()
        || bundle.bundle_sha256 != bundle_sha(bundle)?
    {
        return Err("invalid portable state identity or schema".into());
    }
    validate_entries(&bundle.entries)
}

pub(super) fn validate_entries(entries: &[PortableEntryV1]) -> Result<(), String> {
    if entries.is_empty() || entries.len() > MAX_ENTRIES {
        return Err("invalid portable entry count".into());
    }
    let mut paths = BTreeSet::new();
    let mut generations = BTreeSet::new();
    let mut previous = None;
    for entry in entries {
        if entry.body.len() > MAX_ENTRY_BYTES
            || validate_sha256(&entry.content_sha256).is_err()
            || crate::knowledge::cut::sha256_hex(&entry.body) != entry.content_sha256
            || !portable_path(&entry.path)
            || !paths.insert(entry.path.as_str())
            || previous.is_some_and(|path| path >= entry.path.as_str())
        {
            return Err("invalid portable state entry".into());
        }
        previous = Some(entry.path.as_str());
        if let Some(generation) = dossier_generation(&entry.path) {
            generations.insert(generation);
        }
    }
    for required in [
        "action/actions.jsonl",
        "action/leases.snapshot.json",
        "dossier/current.json",
    ] {
        if !paths.contains(required) {
            return Err("portable state is missing a required component".into());
        }
    }
    if generations.len() != 1 {
        return Err("portable state must contain exactly one dossier generation".into());
    }
    Ok(())
}

fn portable_path(path: &str) -> bool {
    !path.starts_with('/')
        && !path.contains('\\')
        && !windows_absolute(path)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && (matches!(
            path,
            "action/actions.jsonl" | "action/leases.snapshot.json" | "dossier/current.json"
        ) || dossier_generation(path).is_some())
}

fn dossier_generation(path: &str) -> Option<&str> {
    let parts = path.split('/').collect::<Vec<_>>();
    let valid = match parts.as_slice() {
        ["dossier", "generations", generation, "manifest.json"] => {
            generation_revision(generation).is_some()
        }
        [
            "dossier",
            "generations",
            generation,
            "anchors",
            shard,
            anchor,
        ] => {
            generation_revision(generation).is_some()
                && shard.len() == 12
                && shard.starts_with("shard-")
                && shard[6..].bytes().all(|byte| byte.is_ascii_digit())
                && anchor.len() == 69
                && anchor.ends_with(".json")
                && validate_sha256(&anchor[..64]).is_ok()
        }
        _ => false,
    };
    if valid { Some(parts[2]) } else { None }
}

fn windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

pub(super) fn validate_components(
    action_root: &Path,
    dossier_root: &Path,
    campaign_id: &str,
) -> Result<(String, u64), String> {
    let report = recover_store(&CompetitionStore::new(action_root.into()))?;
    if report.torn_tail_discarded {
        return Err("portable action journal contains a torn tail".into());
    }
    if report.journal.campaign_id.as_deref() != Some(campaign_id) {
        return Err("portable journal campaign mismatch".into());
    }
    let leases = CompetitionStore::new(action_root.into()).recover_leases()?;
    if leases
        .snapshot()
        .iter()
        .any(|lease| lease.key.campaign_id != campaign_id)
    {
        return Err("portable lease campaign mismatch".into());
    }
    let dossier = DossierStoreV1::new(dossier_root.into())
        .load()?
        .ok_or_else(|| "portable dossier is missing".to_string())?;
    if dossier.journal_head_sha256 != report.journal.head_sha256
        || dossier
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.journal_head_sha256 != report.journal.head_sha256)
    {
        return Err("portable dossier and action journal heads do not match".into());
    }
    Ok((report.journal.head_sha256, dossier.dossier_revision))
}

pub(super) fn current_generation(dossier_root: &Path) -> Result<String, String> {
    let raw = super::store_fs::read_bounded(&dossier_root.join("current.json"), 4 * 1024)?;
    let value: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|error| format!("parse dossier pointer: {error}"))?;
    let generation = value
        .get("generation")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "portable dossier generation is missing".to_string())?;
    if generation_revision(generation).is_none() {
        return Err("invalid portable dossier generation".into());
    }
    Ok(generation.into())
}

fn bundle_sha(bundle: &PortableStateBundleV1) -> Result<String, String> {
    serde_json::to_vec(&(
        &bundle.schema,
        &bundle.contract,
        &bundle.campaign_id,
        &bundle.entries,
    ))
    .map(|body| crate::knowledge::cut::sha256_hex(&body))
    .map_err(|error| format!("encode portable digest: {error}"))
}
