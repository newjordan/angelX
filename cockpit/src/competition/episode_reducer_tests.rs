use super::episode::*;
use super::episode_reducer::*;
use super::journal::{
    ACTION_JOURNAL_SCHEMA_V1, ActionJournalEventV1, ActionJournalStateV1, ActionUpdateV1,
    PrepareActionV1, canonical_action_key,
};
use super::profile::*;
use super::schema::{
    ActionIntentV1, ActionKindV1, ActionPhaseV1, ComparatorKindV1, CompetitionKeyV1,
    ObjectiveComparatorV1,
};
#[path = "episode_store_tests.rs"]
mod store_tests;
#[path = "episode_validation_tests.rs"]
mod validation;
fn digest(byte: char) -> String {
    byte.to_string().repeat(64)
}
fn competition() -> CompetitionKeyV1 {
    CompetitionKeyV1 {
        platform_id: "fixture".into(),
        competition_id: "contest".into(),
        field_id: "kernels".into(),
        benchmark_id: "bench".into(),
        profile_id: "p1".into(),
        hardware_id: "gpu".into(),
    }
}
fn update(intent: ActionIntentV1, phase: ActionPhaseV1, at_ms: u64) -> ActionUpdateV1 {
    ActionUpdateV1 {
        intent,
        attempt: 0,
        phase,
        retryable: false,
        at_ms,
        reconcile_key: None,
        receipt_sha256: None,
        next: None,
    }
}
fn journal_started(intent: ActionIntentV1) -> ActionJournalEventV1 {
    let mut journal = ActionJournalStateV1::default();
    let planned = match journal
        .prepare(update(intent.clone(), ActionPhaseV1::Planned, 7))
        .unwrap()
    {
        PrepareActionV1::Append(event) => *event,
        _ => unreachable!(),
    };
    journal.apply(&planned).unwrap();
    match journal
        .prepare(update(intent, ActionPhaseV1::Started, 8))
        .unwrap()
    {
        PrepareActionV1::Append(event) => *event,
        _ => unreachable!(),
    }
}
fn start_receipt(replay: bool) -> EpisodeStartedV1 {
    let competition = competition();
    let placeholder_intent = ActionIntentV1 {
        action_key: digest('a'),
        campaign_id: "campaign".into(),
        competition: competition.clone(),
        kind: ActionKindV1::StartEpisode,
        subject_id: "episode-2".into(),
        payload_sha256: digest('b'),
        intent_version: DEEP_CUT_START_INTENT_VERSION_V1.into(),
    };
    let mut start = EpisodeStartedV1 {
        episode_id: "episode-2".into(),
        campaign_id: "campaign".into(),
        competition,
        objective: ObjectiveComparatorV1 {
            objective_id: "speed".into(),
            version: "v1".into(),
            kind: ComparatorKindV1::HigherIsBetter,
        },
        profile: DeepCutProfileV1::embedded().identity().unwrap(),
        board_epoch: 2,
        board_observation_revision: 3,
        board_decision_sha256: digest('c'),
        source: EpisodeSourceLineageV1 {
            base_id: "byte-identical-base".into(),
            source_board_epoch: 2,
            commit_oid: "same-commit".into(),
            tree_oid: "same-tree".into(),
            workspace_sha256: digest('d'),
            access_proof_sha256: digest('e'),
            observation_sha256: digest('f'),
        },
        hypothesis_id: "hypothesis".into(),
        origin_episode_id: replay.then(|| "episode-1".into()),
        replay: replay.then(|| ReplayAncestryV1 {
            replay_of_episode_id: "episode-1".into(),
            replay_of_candidate_id: Some("candidate-1".into()),
            preserved_hypothesis_id: "hypothesis".into(),
            predecessor_board_epoch: 1,
        }),
        worker_instance_id: "worker-7".into(),
        model_id: "model".into(),
        requested_route: "route".into(),
        reasoning_effort: Some("high".into()),
        tool_strategy_sha256: digest('1'),
        start_journal_event: ActionJournalEventV1 {
            schema: ACTION_JOURNAL_SCHEMA_V1.into(),
            seq: 0,
            previous_sha256: String::new(),
            update: update(placeholder_intent, ActionPhaseV1::Started, 8),
            event_sha256: digest('2'),
        },
        started_at_ms: 10,
    };
    let mut intent = start.start_journal_event.update.intent.clone();
    intent.payload_sha256 = start.canonical_start_payload_sha256().unwrap();
    intent.action_key = canonical_action_key(&intent).unwrap();
    start.start_journal_event = journal_started(intent);
    start
}

fn rebind_start(start: &mut EpisodeStartedV1) {
    let mut intent = start.start_journal_event.update.intent.clone();
    intent.payload_sha256 = start.canonical_start_payload_sha256().unwrap();
    intent.action_key = canonical_action_key(&intent).unwrap();
    start.start_journal_event = journal_started(intent);
}
#[derive(Default)]
struct MemoryAuthority {
    events: Vec<String>,
    fail: bool,
    durable_actions: Vec<ActionJournalEventV1>,
    durable_head: Option<(u64, String)>,
}
impl MemoryAuthority {
    fn with_start(start: &EpisodeStartedV1) -> Self {
        Self {
            durable_actions: vec![start.start_journal_event.clone()],
            ..Self::default()
        }
    }
}
impl EpisodeDurabilityAuthorityV1 for MemoryAuthority {
    fn verify_action_event_durable(&mut self, event: &ActionJournalEventV1) -> Result<(), String> {
        self.durable_actions
            .contains(event)
            .then_some(())
            .ok_or_else(|| "Started action event is not durable".into())
    }

    fn verify_action_head_durable(
        &mut self,
        sequence: u64,
        head_sha256: &str,
    ) -> Result<(), String> {
        (self.durable_head.as_ref() == Some(&(sequence, head_sha256.to_string())))
            .then_some(())
            .ok_or_else(|| "terminal action head is not durable".into())
    }

    fn append_and_sync(&mut self, event: &EpisodeEventV1) -> Result<(), String> {
        if self.fail {
            return Err("sync failed".into());
        }
        self.events.push(event.event_sha256.clone());
        Ok(())
    }
}
fn terminal(outcome: EpisodeTerminalOutcomeV1, end: u64) -> EpisodeTerminalV1 {
    EpisodeTerminalV1 {
        outcome,
        recovered_from: (outcome == EpisodeTerminalOutcomeV1::Recovered)
            .then_some(EpisodeTerminalOutcomeV1::Crash),
        detail_sha256: Some(digest('3')),
        elapsed_ms: 9,
        model_calls: 1,
        tool_calls: 2,
        input_tokens: 3,
        output_tokens: 4,
        monetary_microunits: 5,
        terminal_at_ms: 20,
        action_journal_end_sequence: end,
        action_journal_head_sha256: digest('4'),
    }
}
fn link(kind: EpisodeEvidenceKindV1, identity: &str) -> EpisodeEvidenceLinkV1 {
    EpisodeEvidenceLinkV1 {
        kind,
        identity: identity.into(),
        receipt_sha256: digest('5'),
    }
}
#[test]
fn dc_ep_001_durable_authority_precedes_effect_and_start_completion() {
    let start = start_receipt(false);
    assert_eq!(
        start.start_journal_event.update.phase,
        ActionPhaseV1::Started
    );
    let event = EpisodeEventV1::started(start.clone()).unwrap();
    let mut state = EpisodeStateV1::default();
    let mut authority = MemoryAuthority {
        fail: true,
        ..MemoryAuthority::with_start(&start)
    };
    assert!(state.append_durable(&mut authority, &event).is_err());
    assert!(state.effect_permit().is_err());
    authority.fail = false;
    let durable = state.append_durable(&mut authority, &event).unwrap();
    let completed = durable.start_action_completion(&start, 11).unwrap();
    assert_eq!(completed.phase, ActionPhaseV1::Completed);
    assert_eq!(
        completed.receipt_sha256.as_deref(),
        Some(event.event_sha256.as_str())
    );
    state.effect_permit().unwrap();
}
#[test]
fn terminal_closes_range_but_official_corrections_remain_append_only() {
    let start = start_receipt(true);
    let start_seq = start.start_journal_event.seq;
    let started = EpisodeEventV1::started(start.clone()).unwrap();
    let mut state = EpisodeStateV1::default();
    let mut authority = MemoryAuthority::with_start(&start);
    authority.durable_head = Some((start_seq + 4, digest('4')));
    state.append_durable(&mut authority, &started).unwrap();
    let effect = state.effect_permit().unwrap();
    let stale_effect = EpisodeEventV1::append(
        &effect,
        EpisodeEventKindV1::EvidenceLinked(link(EpisodeEvidenceKindV1::Candidate, "candidate")),
    )
    .unwrap();
    let backward = EpisodeEventV1::append(
        &effect,
        EpisodeEventKindV1::Terminal(terminal(
            EpisodeTerminalOutcomeV1::Regression,
            start_seq - 1,
        )),
    )
    .unwrap();
    assert!(state.append_durable(&mut authority, &backward).is_err());
    let mut forged_terminal = terminal(EpisodeTerminalOutcomeV1::Regression, start_seq + 4);
    forged_terminal.action_journal_head_sha256 = digest('9');
    let forged =
        EpisodeEventV1::append(&effect, EpisodeEventKindV1::Terminal(forged_terminal)).unwrap();
    assert!(state.append_durable(&mut authority, &forged).is_err());
    let closed = EpisodeEventV1::append(
        &effect,
        EpisodeEventKindV1::Terminal(terminal(
            EpisodeTerminalOutcomeV1::Regression,
            start_seq + 4,
        )),
    )
    .unwrap();
    state.append_durable(&mut authority, &closed).unwrap();
    assert!(state.effect_permit().is_err());
    let count = authority.events.len();
    assert!(state.append_durable(&mut authority, &stale_effect).is_err());
    assert_eq!(
        authority.events.len(),
        count,
        "rejected effects never reach durability"
    );
    assert!(
        EpisodeEventV1::link_evidence(
            &state.evidence_permit().unwrap(),
            link(EpisodeEvidenceKindV1::Candidate, "late")
        )
        .is_err()
    );
    for (kind, id) in [
        (EpisodeEvidenceKindV1::OfficialResult, "official-r1"),
        (EpisodeEvidenceKindV1::RewardBinding, "reward-r1"),
        (EpisodeEvidenceKindV1::RewardBinding, "reward-correction-r2"),
        (EpisodeEvidenceKindV1::PatternUpdate, "pattern-r2"),
    ] {
        let event =
            EpisodeEventV1::link_evidence(&state.evidence_permit().unwrap(), link(kind, id))
                .unwrap();
        state.append_durable(&mut authority, &event).unwrap();
    }
    assert!(state.append_durable(&mut authority, &closed).is_err());
}
