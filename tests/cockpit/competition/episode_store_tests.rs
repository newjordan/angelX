use super::*;
use crate::drive::competition::episode_store::EpisodeStoreV1;
use crate::drive::competition::leases::WorkLeaseKeyV1;
use crate::drive::competition::schema::LaneIdV1;
use crate::drive::competition::store::CompetitionStore;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-episode-store-{tag}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn durable_harness(
    tag: &str,
    replay: bool,
) -> (
    TestDir,
    EpisodeStartedV1,
    CompetitionStore,
    EpisodeStoreV1,
    WorkLeaseKeyV1,
    u64,
) {
    let dir = TestDir::new(tag);
    let start = start_receipt(replay);
    let action_store = CompetitionStore::new(dir.path().join("actions"));
    let key = WorkLeaseKeyV1 {
        campaign_id: start.campaign_id.clone(),
        lane_id: LaneIdV1::DeepCut,
        work_item_id: start.episode_id.clone(),
    };
    let lease = action_store
        .update_leases(|leases| leases.grant(key.clone(), "worker".into(), 0, 500_000, 0))
        .unwrap();
    action_store
        .append_action_fenced(
            &key,
            lease.fencing_generation,
            update(
                start.start_journal_event.update.intent.clone(),
                ActionPhaseV1::Planned,
                7,
            ),
        )
        .unwrap();
    let durable_started = action_store
        .append_action_fenced(
            &key,
            lease.fencing_generation,
            start.start_journal_event.update.clone(),
        )
        .unwrap();
    assert_eq!(
        durable_started.event_sha256,
        start.start_journal_event.event_sha256
    );
    let episode_store = EpisodeStoreV1::new(dir.path().join("episodes"), action_store.clone());
    (
        dir,
        start,
        action_store,
        episode_store,
        key,
        lease.fencing_generation,
    )
}

fn persist_start(
    start: &EpisodeStartedV1,
    action_store: &CompetitionStore,
    episode_store: &mut EpisodeStoreV1,
    key: &WorkLeaseKeyV1,
    generation: u64,
) -> EpisodeStateV1 {
    let mut state = EpisodeStateV1::default();
    let started = EpisodeEventV1::started(start.clone()).unwrap();
    let receipt = state.append_durable(episode_store, &started).unwrap();
    let completed = receipt.start_action_completion(start, 11).unwrap();
    action_store
        .append_action_fenced(key, generation, completed)
        .unwrap();
    state
}

fn close_terminal(
    state: &mut EpisodeStateV1,
    store: &mut EpisodeStoreV1,
    action_store: &CompetitionStore,
    outcome: EpisodeTerminalOutcomeV1,
) {
    let action = action_store.recover().unwrap().state;
    let mut receipt = terminal(outcome, action.next_seq - 1);
    receipt.action_journal_head_sha256 = action.head_sha256;
    let event = EpisodeEventV1::append(
        &state.effect_permit().unwrap(),
        EpisodeEventKindV1::Terminal(receipt),
    )
    .unwrap();
    state.append_durable(store, &event).unwrap();
}

/// DC-EP-002-all-terminal-outcomes-finalize-or-recover
#[test]
fn dc_ep_002_all_terminal_outcomes_finalize_or_recover() {
    let outcomes = [
        EpisodeTerminalOutcomeV1::Success,
        EpisodeTerminalOutcomeV1::Regression,
        EpisodeTerminalOutcomeV1::Rejection,
        EpisodeTerminalOutcomeV1::CorrectnessFailure,
        EpisodeTerminalOutcomeV1::Timeout,
        EpisodeTerminalOutcomeV1::Crash,
        EpisodeTerminalOutcomeV1::Abandoned,
        EpisodeTerminalOutcomeV1::Stale,
        EpisodeTerminalOutcomeV1::Replayed,
        EpisodeTerminalOutcomeV1::Recovered,
    ];
    for (index, outcome) in outcomes.into_iter().enumerate() {
        let (dir, start, actions, mut episodes, key, generation) = durable_harness(
            &format!("terminal-{index}"),
            outcome == EpisodeTerminalOutcomeV1::Replayed,
        );
        let mut state = persist_start(&start, &actions, &mut episodes, &key, generation);
        close_terminal(&mut state, &mut episodes, &actions, outcome);
        assert!(state.effect_permit().is_err());
        assert_eq!(episodes.recover_episode(&start.episode_id).unwrap(), state);
        drop(dir);
    }
}

/// DC-EP-003-graph-rollout-action-candidate-lineage-complete
#[test]
fn dc_ep_003_graph_rollout_action_candidate_lineage_complete() {
    let (dir, start, actions, mut episodes, key, generation) = durable_harness("lineage", false);
    let mut state = persist_start(&start, &actions, &mut episodes, &key, generation);
    let links = [
        (EpisodeEvidenceKindV1::GraphEpisode, "graph-episode-7"),
        (EpisodeEvidenceKindV1::GraphTrace, "graph-trace-7"),
        (EpisodeEvidenceKindV1::HarnessRollout, "rollout-7"),
        (EpisodeEvidenceKindV1::Action, "action-key-7"),
        (EpisodeEvidenceKindV1::Candidate, "candidate-7"),
        (EpisodeEvidenceKindV1::Verifier, "verifier-7"),
        (EpisodeEvidenceKindV1::LocalMeasurement, "measurement-7"),
    ];
    for (kind, identity) in links {
        let event =
            EpisodeEventV1::link_evidence(&state.evidence_permit().unwrap(), link(kind, identity))
                .unwrap();
        state.append_durable(&mut episodes, &event).unwrap();
    }
    close_terminal(
        &mut state,
        &mut episodes,
        &actions,
        EpisodeTerminalOutcomeV1::Success,
    );
    for (kind, identity) in [
        (EpisodeEvidenceKindV1::OfficialResult, "official-7"),
        (EpisodeEvidenceKindV1::RewardBinding, "reward-7"),
        (EpisodeEvidenceKindV1::PatternUpdate, "pattern-7"),
    ] {
        let event =
            EpisodeEventV1::link_evidence(&state.evidence_permit().unwrap(), link(kind, identity))
                .unwrap();
        state.append_durable(&mut episodes, &event).unwrap();
    }
    let raw = std::fs::read(episodes.journal_path(&start.episode_id)).unwrap();
    for (_, identity) in links {
        assert!(
            raw.windows(identity.len())
                .any(|window| window == identity.as_bytes())
        );
    }
    assert_eq!(episodes.recover_episode(&start.episode_id).unwrap(), state);
    drop(dir);
}

/// DC-EP-004-crash-replay-equals-uninterrupted-reduction
#[test]
fn dc_ep_004_crash_replay_equals_uninterrupted_reduction() {
    let (dir, start, actions, mut episodes, key, generation) = durable_harness("replay", false);
    let mut state = persist_start(&start, &actions, &mut episodes, &key, generation);
    for (kind, identity) in [
        (EpisodeEvidenceKindV1::GraphTrace, "trace-replay"),
        (EpisodeEvidenceKindV1::Action, "action-replay"),
        (EpisodeEvidenceKindV1::Candidate, "candidate-replay"),
    ] {
        let event =
            EpisodeEventV1::link_evidence(&state.evidence_permit().unwrap(), link(kind, identity))
                .unwrap();
        state.append_durable(&mut episodes, &event).unwrap();
        let restarted = EpisodeStoreV1::new(dir.path().join("episodes"), actions.clone());
        assert_eq!(restarted.recover_episode(&start.episode_id).unwrap(), state);
    }
    let path = episodes.journal_path(&start.episode_id);
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(br#"{"schema":"torn""#).unwrap();
    file.sync_data().unwrap();
    assert!(episodes.recover_episode(&start.episode_id).is_err());
}

#[test]
fn production_authority_and_replay_fail_closed_on_untrusted_bytes() {
    let dir = TestDir::new("unsynced-production");
    let start = start_receipt(false);
    let actions = CompetitionStore::new(dir.path().join("actions"));
    let mut episodes = EpisodeStoreV1::new(dir.path().join("episodes"), actions);
    let mut state = EpisodeStateV1::default();
    let event = EpisodeEventV1::started(start).unwrap();
    assert!(state.append_durable(&mut episodes, &event).is_err());
    assert!(state.effect_permit().is_err());

    let (dir, start, actions, mut episodes, key, generation) = durable_harness("malformed", false);
    persist_start(&start, &actions, &mut episodes, &key, generation);
    let path = episodes.journal_path(&start.episode_id);
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(b"{malformed}\n").unwrap();
    file.sync_data().unwrap();
    assert!(episodes.recover_episode(&start.episode_id).is_err());
    drop(dir);

    let (_dir, start, actions, mut episodes, key, generation) = durable_harness("nonappend", false);
    persist_start(&start, &actions, &mut episodes, &key, generation);
    let path = episodes.journal_path(&start.episode_id);
    let raw = std::fs::read(&path).unwrap();
    let first = raw.iter().position(|byte| *byte == b'\n').unwrap() + 1;
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(&raw[..first]).unwrap();
    file.sync_data().unwrap();
    assert!(episodes.recover_episode(&start.episode_id).is_err());
}
