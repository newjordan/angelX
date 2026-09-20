use super::director_store::DirectorSnapshotV1;
use super::episode::{
    EpisodeEventKindV1, EpisodeEventV1, EpisodeEvidenceKindV1, EpisodeStartedV1, EpisodeTerminalV1,
    post_terminal_evidence, validate_event_body,
};
use super::full_portability_observations::{
    PortableObservationReceiptV1, validate_history_ids, validate_start_receipt,
};
use super::journal::{ActionJournalEventV1, ActionJournalStateV1};
use super::portability::PortableEntryV1;
use super::store_fs::{read_bounded, reject_symlink};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const MAX_EPISODE_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn collect_episode_entries(
    source_root: &Path,
    entries: &mut Vec<PortableEntryV1>,
) -> Result<(), String> {
    let root = source_root.join("episodes");
    if !root.exists() {
        return Ok(());
    }
    reject_symlink(&root)?;
    if !root.is_dir() {
        return Err("portable episode root is not a directory".into());
    }
    let mut paths = fs::read_dir(&root)
        .map_err(|error| format!("read portable episodes: {error}"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read portable episode entry: {error}"))?;
    paths.sort();
    for path in paths {
        reject_symlink(&path)?;
        if !path.is_file() {
            return Err("portable episode entry is not a regular file".into());
        }
        let body = read_bounded(&path, MAX_EPISODE_BYTES)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "invalid portable episode filename".to_string())?;
        if name == "episode-store.lock" {
            continue;
        }
        entries.push(PortableEntryV1 {
            path: format!("episodes/{name}"),
            content_sha256: crate::cut::sha256_hex(&body),
            body,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(())
}

pub(super) fn validate_episode_entries(
    snapshot: &DirectorSnapshotV1,
    entries: &BTreeMap<String, &PortableEntryV1>,
    observation_history: &[PortableObservationReceiptV1],
    action_journal: &[u8],
) -> Result<(), String> {
    let expected = snapshot
        .director
        .episodes
        .keys()
        .map(|id| {
            (
                format!(
                    "episodes/episode-{}.jsonl",
                    crate::cut::sha256_hex(id.as_bytes())
                ),
                id,
            )
        })
        .collect::<BTreeMap<_, _>>();
    if entries.keys().ne(expected.keys()) {
        return Err("portable episode journals do not match director references".into());
    }
    validate_history_ids(
        observation_history,
        &expected.values().map(|id| (*id).clone()).collect(),
    )?;
    if expected.is_empty() {
        return Ok(());
    }
    let actions = replay_actions(action_journal)?;
    for (path, id) in expected {
        let summary = replay_episode(id, &entries[&path].body)?;
        let reference = &snapshot.director.episodes[id];
        let candidate = snapshot
            .director
            .candidates
            .as_ref()
            .and_then(|repository| repository.catalog.get(&reference.candidate_id))
            .ok_or_else(|| "portable episode candidate is absent".to_string())?;
        let submission = snapshot
            .director
            .submissions
            .items
            .values()
            .find(|item| item.submission_id == reference.submission_id)
            .ok_or_else(|| "portable episode submission is absent".to_string())?;
        validate_start(snapshot, reference.board_epoch, &summary.start, candidate)?;
        validate_start_receipt(observation_history, &summary.start)?;
        if summary.candidates.len() != 1
            || summary.submissions.len() != 1
            || summary.candidates.get(&reference.candidate_id) != Some(&candidate.record_sha256)
            || summary.submissions.get(&reference.submission_id) != Some(&submission.request_sha256)
            || actions.get(&summary.start.start_journal_event.seq)
                != Some(&summary.start.start_journal_event)
            || summary.terminal.is_some() != reference.terminal
            || reference
                .latest_official_result_id
                .as_ref()
                .is_some_and(|value| !summary.official_results.contains(value))
            || reference
                .latest_reward_binding_sha256
                .as_ref()
                .is_some_and(|value| !summary.reward_bindings.contains(value))
        {
            return Err("portable episode journal disagrees with its director reference".into());
        }
        if let Some(terminal) = &summary.terminal
            && (terminal.action_journal_end_sequence < summary.start.start_journal_event.seq
                || actions
                    .get(&terminal.action_journal_end_sequence)
                    .map(|event| &event.event_sha256)
                    != Some(&terminal.action_journal_head_sha256)
                || terminal.action_journal_head_sha256 != reference.journal_head_sha256)
        {
            return Err("portable episode terminal action range is not durable".into());
        }
    }
    Ok(())
}

struct EpisodeSummaryV1 {
    start: EpisodeStartedV1,
    terminal: Option<EpisodeTerminalV1>,
    candidates: BTreeMap<String, String>,
    submissions: BTreeMap<String, String>,
    official_results: BTreeSet<String>,
    reward_bindings: BTreeSet<String>,
}

fn replay_episode(episode_id: &str, raw: &[u8]) -> Result<EpisodeSummaryV1, String> {
    if raw.is_empty() || !raw.ends_with(b"\n") {
        return Err("portable episode journal is empty or torn".into());
    }
    let mut sequence = 0_u64;
    let mut head = String::new();
    let mut start = None;
    let mut terminal = None;
    let mut candidates = BTreeMap::new();
    let mut submissions = BTreeMap::new();
    let mut official_results = BTreeSet::new();
    let mut reward_bindings = BTreeSet::new();
    for line in raw[..raw.len() - 1].split(|byte| *byte == b'\n') {
        let event: EpisodeEventV1 = serde_json::from_slice(line)
            .map_err(|error| format!("parse portable episode: {error}"))?;
        if event.episode_id != episode_id
            || event.sequence != sequence
            || event.previous_sha256 != head
            || event.event_sha256 != event.canonical_sha256()?
        {
            return Err("portable episode hash chain or identity mismatch".into());
        }
        validate_event_body(&event.event)?;
        match &event.event {
            EpisodeEventKindV1::Started(value) if sequence == 0 && start.is_none() => {
                value.validate()?;
                start = Some(value.as_ref().clone());
            }
            EpisodeEventKindV1::Started(_) => return Err("duplicate portable episode start".into()),
            EpisodeEventKindV1::Terminal(_) if terminal.is_some() => {
                return Err("duplicate portable episode terminal".into());
            }
            EpisodeEventKindV1::Terminal(value) if start.is_some() => {
                terminal = Some(value.clone())
            }
            EpisodeEventKindV1::Terminal(_) => {
                return Err("portable episode lacks its start".into());
            }
            EpisodeEventKindV1::EvidenceLinked(link)
                if terminal.is_some() && !post_terminal_evidence(link.kind) =>
            {
                return Err("invalid post-terminal portable episode evidence".into());
            }
            EpisodeEventKindV1::EvidenceLinked(_) if sequence == 0 => {
                return Err("portable episode lacks its start".into());
            }
            EpisodeEventKindV1::EvidenceLinked(link) => match link.kind {
                EpisodeEvidenceKindV1::Candidate
                    if candidates
                        .insert(link.identity.clone(), link.receipt_sha256.clone())
                        .is_some() =>
                {
                    return Err("duplicate portable candidate evidence".into());
                }
                EpisodeEvidenceKindV1::Submission
                    if submissions
                        .insert(link.identity.clone(), link.receipt_sha256.clone())
                        .is_some() =>
                {
                    return Err("duplicate portable submission evidence".into());
                }
                EpisodeEvidenceKindV1::OfficialResult => {
                    official_results.insert(link.identity.clone());
                }
                EpisodeEvidenceKindV1::RewardBinding => {
                    reward_bindings.insert(link.identity.clone());
                    reward_bindings.insert(link.receipt_sha256.clone());
                }
                _ => {}
            },
        }
        sequence = sequence
            .checked_add(1)
            .ok_or("portable episode sequence exhausted")?;
        head = event.event_sha256;
    }
    Ok(EpisodeSummaryV1 {
        start: start.ok_or_else(|| "portable episode lacks its start".to_string())?,
        terminal,
        candidates,
        submissions,
        official_results,
        reward_bindings,
    })
}

fn replay_actions(raw: &[u8]) -> Result<BTreeMap<u64, ActionJournalEventV1>, String> {
    if raw.is_empty() || !raw.ends_with(b"\n") {
        return Err("portable action journal is empty or torn".into());
    }
    let mut state = ActionJournalStateV1::default();
    let mut events = BTreeMap::new();
    for line in raw[..raw.len() - 1].split(|byte| *byte == b'\n') {
        let event: ActionJournalEventV1 = serde_json::from_slice(line)
            .map_err(|error| format!("parse portable action event: {error}"))?;
        state.apply(&event).map_err(|error| error.to_string())?;
        events.insert(event.seq, event);
    }
    Ok(events)
}

fn validate_start(
    snapshot: &DirectorSnapshotV1,
    reference_epoch: u64,
    start: &EpisodeStartedV1,
    candidate: &super::candidate::CandidateV1,
) -> Result<(), String> {
    let base = &candidate.board.source_accessible_base;
    let source = &base.source;
    if start.campaign_id != snapshot.director.campaign_id
        || start.competition != candidate.competition
        || start.objective != candidate.objective
        || start.episode_id != candidate.producing_episode_id
        || start.board_epoch != reference_epoch
        || start.board_epoch != candidate.board.board_epoch
        || start.board_decision_sha256 != candidate.board.board_decision_sha256
        || start.source.base_id != base.entry_id
        || start.source.source_board_epoch != base.source_board_epoch
        || start.source.commit_oid != source.commit_oid
        || start.source.tree_oid != source.tree_oid
        || start.source.workspace_sha256 != source.workspace_sha256
        || start.source.access_proof_sha256 != source.access_proof_sha256
    {
        return Err("portable episode start is cross-composed with candidate lineage".into());
    }
    if let Some(board) = snapshot
        .director
        .board
        .as_ref()
        .filter(|board| board.board_epoch == start.board_epoch)
        && start.board_observation_revision > board.observation_revision
    {
        return Err("portable episode start lacks board observation provenance".into());
    }
    Ok(())
}
