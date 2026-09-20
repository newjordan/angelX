use super::director_services::DirectorWorkerRefV1;
use super::director_store::{DirectorSnapshotV1, DirectorStoreV1};
use super::full_portability_io::{collect_episode_entries, validate_episode_entries};
use super::full_portability_observations::{
    PortableObservationReceiptV1, build_observation_history,
};
use super::leases::LeaseBookV1;
use super::patterns::PatternEvidenceClassV1;
use super::portability::{
    PortableEntryV1, current_generation, validate_components, validate_entries,
};
use super::portability_io::{collect_entries, install_staging, publish_staging};
use super::schema::COMPETITION_CONTRACT_V1;
use super::schema_validation::{validate_id, validate_sha256};
use super::store::CompetitionStore;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
pub(crate) const FULL_PORTABLE_STATE_SCHEMA_V1: &str = "angel.competition-full-portable-state/v1";
const MAX_BUNDLE_BYTES: usize = 256 * 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullPortableStateV1 {
    schema: String,
    contract: String,
    campaign_id: String,
    snapshot: DirectorSnapshotV1,
    observation_history: Vec<PortableObservationReceiptV1>,
    entries: Vec<PortableEntryV1>,
    bundle_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FullPortableReceiptV1 {
    pub(crate) campaign_id: String,
    pub(crate) bundle_sha256: String,
    pub(crate) snapshot: DirectorSnapshotV1,
}
pub(crate) fn export_full_portable_state(
    source_root: &Path,
    campaign_id: &str,
) -> Result<Vec<u8>, String> {
    validate_id(campaign_id, "invalid campaign id").map_err(str::to_string)?;
    let snapshot = DirectorStoreV1::new(source_root.join("director"))
        .recover()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "full portable director snapshot is missing".to_string())?;
    let generation = current_generation(&source_root.join("dossier"))?;
    let mut entries = collect_entries(source_root, &generation)?;
    collect_episode_entries(source_root, &mut entries)?;
    let observation_history = build_observation_history(&entries)?;
    let mut bundle = FullPortableStateV1 {
        schema: FULL_PORTABLE_STATE_SCHEMA_V1.into(),
        contract: COMPETITION_CONTRACT_V1.into(),
        campaign_id: campaign_id.into(),
        snapshot,
        observation_history,
        entries,
        bundle_sha256: String::new(),
    };
    validate_bundle(&bundle, false)?;
    validate_root(source_root, &bundle.snapshot)?;
    bundle.bundle_sha256 = bundle_sha(&bundle)?;
    let raw = serde_json::to_vec(&bundle)
        .map_err(|error| format!("encode full portable state: {error}"))?;
    if raw.len() > MAX_BUNDLE_BYTES {
        return Err("full portable state exceeds bound".into());
    }
    Ok(raw)
}
pub(crate) fn import_full_portable_state(
    raw: &[u8],
    target_root: &Path,
) -> Result<FullPortableReceiptV1, String> {
    if raw.len() > MAX_BUNDLE_BYTES {
        return Err("full portable state exceeds bound".into());
    }
    let bundle: FullPortableStateV1 = serde_json::from_slice(raw)
        .map_err(|error| format!("parse full portable state: {error}"))?;
    validate_bundle(&bundle, true)?;
    let staging = install_staging(target_root, &bundle.entries)?;
    let result = (|| {
        validate_root(&staging, &bundle.snapshot)?;
        let action_store = CompetitionStore::new(staging.join("action"));
        action_store.update_leases(|leases| leases.fence_all_for_import())?;
        let leases = action_store.recover_leases()?;
        let mut director = bundle.snapshot.director.clone();
        director.workers = worker_refs(&leases)?;
        director.revision = director
            .revision
            .checked_add(1)
            .ok_or_else(|| "director revision exhausted during import".to_string())?;
        let mut scheduler = bundle.snapshot.scheduler.clone();
        align_scheduler(&mut scheduler, &leases)?;
        let snapshot = DirectorSnapshotV1::new_with_migration(
            director,
            scheduler,
            bundle.snapshot.action_journal_head_sha256.clone(),
            bundle.snapshot.dossier_revision,
            bundle.snapshot.migration.receipt().cloned(),
        )?;
        DirectorStoreV1::new(staging.join("director"))
            .persist(&snapshot)
            .map_err(|error| error.to_string())?;
        validate_root(&staging, &snapshot)?;
        Ok(snapshot)
    })();
    match result {
        Ok(snapshot) => {
            publish_staging(&staging, target_root)?;
            Ok(FullPortableReceiptV1 {
                campaign_id: bundle.campaign_id,
                bundle_sha256: bundle.bundle_sha256,
                snapshot,
            })
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(error)
        }
    }
}
fn validate_bundle(bundle: &FullPortableStateV1, sealed: bool) -> Result<(), String> {
    if bundle.schema != FULL_PORTABLE_STATE_SCHEMA_V1
        || bundle.contract != COMPETITION_CONTRACT_V1
        || validate_id(&bundle.campaign_id, "invalid campaign id").is_err()
        || bundle.snapshot.director.campaign_id != bundle.campaign_id
        || (sealed
            && (validate_sha256(&bundle.bundle_sha256).is_err()
                || bundle.bundle_sha256 != bundle_sha(bundle)?))
    {
        return Err("invalid full portable identity, contract, or digest".into());
    }
    bundle.snapshot.validate()?;
    let mut base = Vec::new();
    let mut episode_paths = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut previous = None;
    for entry in &bundle.entries {
        if entry.body.len() > MAX_ENTRY_BYTES
            || crate::cut::sha256_hex(&entry.body) != entry.content_sha256
            || !seen.insert(entry.path.as_str())
            || previous.is_some_and(|path| path >= entry.path.as_str())
        {
            return Err("invalid full portable entry".into());
        }
        previous = Some(entry.path.as_str());
        if entry.path.starts_with("episodes/") {
            episode_paths.insert(entry.path.clone(), entry);
        } else {
            base.push(entry.clone());
        }
    }
    validate_entries(&base)?;
    let actions = base
        .iter()
        .find(|entry| entry.path == "action/actions.jsonl")
        .ok_or_else(|| "portable action journal is missing".to_string())?;
    validate_episode_entries(
        &bundle.snapshot,
        &episode_paths,
        &bundle.observation_history,
        &actions.body,
    )
}

fn validate_root(root: &Path, snapshot: &DirectorSnapshotV1) -> Result<(), String> {
    snapshot.validate()?;
    let (head, dossier_revision) = validate_components(
        &root.join("action"),
        &root.join("dossier"),
        &snapshot.director.campaign_id,
    )?;
    if head != snapshot.action_journal_head_sha256 || dossier_revision != snapshot.dossier_revision
    {
        return Err("full portable snapshot is cross-composed with action or dossier state".into());
    }
    let leases = CompetitionStore::new(root.join("action")).recover_leases()?;
    if worker_refs(&leases)? != snapshot.director.workers {
        return Err("full portable director worker references do not match leases".into());
    }
    let mut core_lanes = BTreeSet::new();
    for worker in snapshot.director.workers.values().filter(|worker| {
        matches!(
            worker.lane,
            super::schema::LaneIdV1::FrontierGuard | super::schema::LaneIdV1::DeepCut
        )
    }) {
        let lane = snapshot.scheduler.lane(worker.lane)?;
        if !core_lanes.insert(worker.lane)
            || lane.worker_generation != worker.fencing_generation
            || lane.checkpoint_revision != worker.checkpoint_revision
        {
            return Err("full portable scheduler is not fenced with its worker lease".into());
        }
    }
    validate_domain_links(snapshot)
}

fn validate_domain_links(snapshot: &DirectorSnapshotV1) -> Result<(), String> {
    let director = &snapshot.director;
    for episode in director.episodes.values() {
        let board_epoch = director.board.as_ref().map(|board| board.board_epoch);
        let official_present = episode.latest_official_result_id.as_ref().is_none_or(|id| {
            director
                .submissions
                .official
                .results
                .iter()
                .any(|result| &result.result_id == id)
        });
        let reward_present = episode
            .latest_reward_binding_sha256
            .as_ref()
            .is_none_or(|id| {
                director
                    .rewards
                    .bindings()
                    .iter()
                    .any(|binding| &binding.binding_id == id)
            });
        if board_epoch.is_none_or(|epoch| episode.board_epoch > epoch)
            || !official_present
            || !reward_present
        {
            return Err("director episode lineage is not present in full state".into());
        }
    }
    let rewards = director.rewards.bindings();
    if rewards.iter().any(|binding| {
        !director.episodes.contains_key(&binding.episode_id)
            || !director.submissions.items.values().any(|item| {
                item.submission_id == binding.submission_id
                    && item.candidate_id == binding.candidate_id
            })
    }) {
        return Err("reward binding is cross-composed with director state".into());
    }
    for evidence in director.patterns.evidence_records() {
        if let PatternEvidenceClassV1::ConfirmedOfficial {
            reward_binding_id, ..
        } = &evidence.class
            && !rewards
                .iter()
                .any(|binding| &binding.binding_id == reward_binding_id)
        {
            return Err("pattern evidence lacks its reward binding".into());
        }
    }
    Ok(())
}

fn worker_refs(book: &LeaseBookV1) -> Result<BTreeMap<String, DirectorWorkerRefV1>, String> {
    let mut refs = BTreeMap::new();
    for lease in book.snapshot() {
        let reference = DirectorWorkerRefV1 {
            work_item_id: lease.key.work_item_id,
            lane: lease.key.lane_id,
            lease_id: lease.lease_id.clone(),
            fencing_generation: lease.fencing_generation,
            checkpoint_revision: lease.checkpoint_revision,
        };
        if refs.insert(lease.lease_id, reference).is_some() {
            return Err("duplicate portable worker lease identity".into());
        }
    }
    Ok(refs)
}

fn align_scheduler(
    scheduler: &mut super::scheduler::SchedulerStateV1,
    leases: &LeaseBookV1,
) -> Result<(), String> {
    for lane in &mut scheduler.lanes {
        if let Some(lease) = leases
            .snapshot()
            .into_iter()
            .find(|lease| lease.key.lane_id == lane.lane)
        {
            lane.worker_generation = lease.fencing_generation;
            lane.checkpoint_revision = lease.checkpoint_revision;
        }
    }
    scheduler.validate()
}

fn bundle_sha(bundle: &FullPortableStateV1) -> Result<String, String> {
    serde_json::to_vec(&(
        &bundle.schema,
        &bundle.contract,
        &bundle.campaign_id,
        &bundle.snapshot,
        &bundle.observation_history,
        &bundle.entries,
    ))
    .map(|body| crate::cut::sha256_hex(&body))
    .map_err(|error| format!("encode full portable digest: {error}"))
}
