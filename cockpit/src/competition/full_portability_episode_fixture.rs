use super::candidate::{CandidateCatalogV1, originate_candidate};
use super::director::CompetitionDirectorStateV1;
use super::director_services::{DirectorEpisodeRefV1, DirectorWorkerRefV1};
use super::director_store::{DirectorSnapshotV1, DirectorStoreV1};
use super::dossier::DossierV1;
use super::dossier_store::DossierStoreV1;
use super::episode::{
    DEEP_CUT_START_INTENT_VERSION_V1, EpisodeEventKindV1, EpisodeEventV1, EpisodeEvidenceKindV1,
    EpisodeEvidenceLinkV1, EpisodeStartedV1, EpisodeTerminalOutcomeV1, EpisodeTerminalV1,
};
use super::episode_reducer::EpisodeStateV1;
use super::episode_store::EpisodeStoreV1;
use super::full_portability::export_full_portable_state;
use super::journal::{
    ACTION_JOURNAL_SCHEMA_V1, ActionJournalEventV1, ActionUpdateV1, PrepareActionV1,
    canonical_action_key,
};
use super::leases::WorkLeaseKeyV1;
use super::profile::{DeepCutProfileV1, EpisodeSourceLineageV1};
use super::scheduler::SchedulerStateV1;
use super::schema::{ActionIntentV1, ActionKindV1, ActionPhaseV1, LaneIdV1};
use super::store::CompetitionStore;
use std::path::{Path, PathBuf};

pub(super) struct Temp(pub(super) PathBuf);

impl Temp {
    pub(super) fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!("angel-full-episode-{tag}-{}", std::process::id())))
    }
    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn digest(value: &str) -> String {
    crate::cut::sha256_hex(value.as_bytes())
}

pub(super) fn episode_bundle(root: &Temp) -> Vec<u8> {
    let board = super::migration_tests::board();
    let mut director = CompetitionDirectorStateV1::engage("campaign-i3", 0).unwrap();
    director
        .apply_board_outcome(
            &super::director_tests::outcome(
                board.clone(),
                super::board::BoardReduceEffectV1::Initialized,
            ),
            1,
        )
        .unwrap();
    let candidate = originate_candidate(
        &CandidateCatalogV1::new(),
        "candidate-i3",
        "hypothesis-i3",
        "episode-i3",
        &board,
        super::candidate_tests::artifact("i3"),
        super::candidate_tests::evidence("i3"),
    )
    .unwrap();
    director
        .candidates
        .as_mut()
        .unwrap()
        .insert(candidate.clone())
        .unwrap();
    let submission_id = director
        .submissions
        .enqueue(
            director.candidates.as_ref().unwrap(),
            "campaign-i3",
            &candidate.candidate_id,
        )
        .unwrap();
    let submission = director
        .submissions
        .items
        .values()
        .find(|item| item.submission_id == submission_id)
        .unwrap()
        .clone();

    let action_store = CompetitionStore::new(root.0.join("action"));
    let key = WorkLeaseKeyV1 {
        campaign_id: "campaign-i3".into(),
        lane_id: LaneIdV1::DeepCut,
        work_item_id: "episode-work".into(),
    };
    let lease = action_store
        .update_leases(|book| book.grant(key.clone(), "deep-cut-i3".into(), 10, 500, 7))
        .unwrap();
    let mut start = start_receipt(&board);
    let mut intent = start.start_journal_event.update.intent.clone();
    intent.payload_sha256 = start.canonical_start_payload_sha256().unwrap();
    intent.action_key = canonical_action_key(&intent).unwrap();
    let planned = update(intent.clone(), ActionPhaseV1::Planned, 10, None);
    action_store
        .append_action_fenced(&key, lease.fencing_generation, planned)
        .unwrap();
    let started = update(intent, ActionPhaseV1::Started, 11, None);
    let journal = action_store.recover().unwrap().state;
    start.start_journal_event = appended(journal.prepare(started.clone()).unwrap());
    action_store
        .append_action_fenced(&key, lease.fencing_generation, started)
        .unwrap();

    let mut episode_store = EpisodeStoreV1::new(root.0.join("episodes"), action_store.clone());
    let mut episode = EpisodeStateV1::default();
    let start_event = EpisodeEventV1::started(start.clone()).unwrap();
    let receipt = episode
        .append_durable(&mut episode_store, &start_event)
        .unwrap();
    action_store
        .append_action_fenced(
            &key,
            lease.fencing_generation,
            receipt.start_action_completion(&start, 12).unwrap(),
        )
        .unwrap();
    for link in [
        EpisodeEvidenceLinkV1 {
            kind: EpisodeEvidenceKindV1::Candidate,
            identity: candidate.candidate_id.clone(),
            receipt_sha256: candidate.record_sha256.clone(),
        },
        EpisodeEvidenceLinkV1 {
            kind: EpisodeEvidenceKindV1::Submission,
            identity: submission_id.clone(),
            receipt_sha256: submission.request_sha256.clone(),
        },
    ] {
        let event =
            EpisodeEventV1::link_evidence(&episode.evidence_permit().unwrap(), link).unwrap();
        episode.append_durable(&mut episode_store, &event).unwrap();
    }
    let journal = action_store.recover().unwrap().state;
    let terminal = EpisodeTerminalV1 {
        outcome: EpisodeTerminalOutcomeV1::Success,
        recovered_from: None,
        detail_sha256: None,
        elapsed_ms: 2,
        model_calls: 1,
        tool_calls: 1,
        input_tokens: 10,
        output_tokens: 10,
        monetary_microunits: 1,
        terminal_at_ms: 13,
        action_journal_end_sequence: journal.next_seq.checked_sub(1).unwrap(),
        action_journal_head_sha256: journal.head_sha256.clone(),
    };
    let event = EpisodeEventV1::append(
        &episode.effect_permit().unwrap(),
        EpisodeEventKindV1::Terminal(terminal),
    )
    .unwrap();
    episode.append_durable(&mut episode_store, &event).unwrap();

    director
        .register_worker(
            DirectorWorkerRefV1 {
                work_item_id: key.work_item_id,
                lane: key.lane_id,
                lease_id: lease.lease_id.clone(),
                fencing_generation: lease.fencing_generation,
                checkpoint_revision: lease.checkpoint_revision,
            },
            13,
        )
        .unwrap();
    director
        .register_episode(
            DirectorEpisodeRefV1 {
                episode_id: "episode-i3".into(),
                candidate_id: candidate.candidate_id,
                submission_id,
                board_epoch: board.board_epoch,
                journal_head_sha256: journal.head_sha256.clone(),
                terminal: true,
                latest_official_result_id: None,
                latest_reward_binding_sha256: None,
            },
            13,
        )
        .unwrap();
    let refreshed_revision =
        super::full_portability_observations::apply_unchanged_refresh(&mut director, &board);
    let dossier = DossierV1::new(
        "episode-dossier".into(),
        "episode-project".into(),
        "episode-repository".into(),
        "episode-revision".into(),
        board.board_epoch,
        journal.head_sha256.clone(),
    )
    .unwrap();
    DossierStoreV1::new(root.0.join("dossier"))
        .sync(&dossier)
        .unwrap();
    let mut scheduler = SchedulerStateV1::new(2, 8).unwrap();
    scheduler.observe_board_move(refreshed_revision).unwrap();
    scheduler
        .checkpoint_lane(LaneIdV1::DeepCut, lease.checkpoint_revision)
        .unwrap();
    let snapshot = DirectorSnapshotV1::new(
        director,
        scheduler,
        journal.head_sha256,
        dossier.dossier_revision,
    )
    .unwrap();
    DirectorStoreV1::new(root.0.join("director"))
        .persist(&snapshot)
        .unwrap();
    export_full_portable_state(root.path(), "campaign-i3").unwrap()
}

fn start_receipt(board: &super::board::CanonicalBoardV1) -> EpisodeStartedV1 {
    let base = board.frontier.source_accessible_base.as_ref().unwrap();
    let intent = ActionIntentV1 {
        action_key: digest("placeholder-key"),
        campaign_id: "campaign-i3".into(),
        competition: board.competition.clone(),
        kind: ActionKindV1::StartEpisode,
        subject_id: "episode-i3".into(),
        payload_sha256: digest("placeholder-payload"),
        intent_version: DEEP_CUT_START_INTENT_VERSION_V1.into(),
    };
    EpisodeStartedV1 {
        episode_id: "episode-i3".into(),
        campaign_id: "campaign-i3".into(),
        competition: board.competition.clone(),
        objective: board.comparator.clone(),
        profile: DeepCutProfileV1::embedded().identity().unwrap(),
        board_epoch: board.board_epoch,
        board_observation_revision: board.observation_revision,
        board_decision_sha256: board.decision_sha256.clone(),
        source: EpisodeSourceLineageV1 {
            base_id: base.entry_id.clone(),
            source_board_epoch: base.source_board_epoch,
            commit_oid: base.source.commit_oid.clone(),
            tree_oid: base.source.tree_oid.clone(),
            workspace_sha256: base.source.workspace_sha256.clone(),
            access_proof_sha256: base.source.access_proof_sha256.clone(),
            observation_sha256: board.latest_provenance.raw_sha256.clone(),
        },
        hypothesis_id: "hypothesis-i3".into(),
        origin_episode_id: None,
        replay: None,
        worker_instance_id: "deep-cut-i3".into(),
        model_id: "model-i3".into(),
        requested_route: "route-i3".into(),
        reasoning_effort: Some("high".into()),
        tool_strategy_sha256: digest("tools-i3"),
        start_journal_event: ActionJournalEventV1 {
            schema: ACTION_JOURNAL_SCHEMA_V1.into(),
            seq: 0,
            previous_sha256: String::new(),
            update: update(intent, ActionPhaseV1::Started, 11, None),
            event_sha256: digest("placeholder-event"),
        },
        started_at_ms: 11,
    }
}

pub(super) fn update(
    intent: ActionIntentV1,
    phase: ActionPhaseV1,
    at_ms: u64,
    receipt_sha256: Option<String>,
) -> ActionUpdateV1 {
    ActionUpdateV1 {
        intent,
        attempt: 0,
        phase,
        retryable: false,
        at_ms,
        reconcile_key: None,
        receipt_sha256,
        next: None,
    }
}

pub(super) fn appended(prepared: PrepareActionV1) -> ActionJournalEventV1 {
    match prepared {
        PrepareActionV1::Append(event) => *event,
        PrepareActionV1::Replay { .. } => panic!("expected append"),
    }
}
