use super::episode::*;
use super::episode_reducer::*;
use super::journal::{
    canonical_action_key, ActionJournalEventV1, ActionJournalStateV1, ActionUpdateV1,
    PrepareActionV1, ACTION_JOURNAL_SCHEMA_V1,
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
    assert!(EpisodeEventV1::link_evidence(
        &state.evidence_permit().unwrap(),
        link(EpisodeEvidenceKindV1::Candidate, "late")
    )
    .is_err());
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

/// The three reduced states and the receipts that produced them, so a table of
/// candidate events can be run against each arm.
struct LegFixture {
    start: EpisodeStartedV1,
    started: EpisodeEventV1,
    opened: DurableEpisodeAppendReceiptV1,
    empty: EpisodeStateV1,
    open: EpisodeStateV1,
    closed: EpisodeStateV1,
    closed_receipt: DurableEpisodeAppendReceiptV1,
    terminal_event: EpisodeEventV1,
}

fn leg_fixture() -> LegFixture {
    let start = start_receipt(false);
    let started = EpisodeEventV1::started(start.clone()).unwrap();
    let mut authority = MemoryAuthority::with_start(&start);
    let empty = EpisodeStateV1::default();
    let mut open = EpisodeStateV1::default();
    let opened = open.append_durable(&mut authority, &started).unwrap();
    let start_seq = start.start_journal_event.seq;
    authority.durable_head = Some((start_seq + 4, digest('4')));
    let terminal_event = EpisodeEventV1::append(
        &open.effect_permit().unwrap(),
        EpisodeEventKindV1::Terminal(terminal(
            EpisodeTerminalOutcomeV1::Regression,
            start_seq + 4,
        )),
    )
    .unwrap();
    let mut closed = open.clone();
    let closed_receipt = closed
        .append_durable(&mut authority, &terminal_event)
        .unwrap();
    LegFixture {
        start,
        started,
        opened,
        empty,
        open,
        closed,
        closed_receipt,
        terminal_event,
    }
}

/// Recompute the canonical digest, so a mutated event still has a matching
/// chain and reaches the state arm under test.
fn resealed(mut event: EpisodeEventV1) -> EpisodeEventV1 {
    event.event_sha256 = event.canonical_sha256().unwrap();
    event
}

/// A throwaway policy module, so a deliberately out-of-contract leg can be
/// pinned without touching the shipped one.
fn temporary_policy_leg(id: &str, render: &str) -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-episode-leg-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let module = root.join(id);
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.json"),
        format!(
            r#"{{"api_version":1,"id":"{id}","kind":"policy","entry":"index.js","probe":{{}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(module.join("index.js"), render).unwrap();
    let mut registry =
        angel_live::Registry::validated(root, crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    registry
        .snapshot()
        .policies
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("fixture leg {id} did not publish"))
}

fn verify<T: PartialEq + std::fmt::Debug>(
    label: &str,
    native: T,
    live: (T, crate::agent::harness::live_policy::LegSource),
) {
    use crate::agent::harness::live_policy::LegSource;
    assert_eq!(
        live.0, native,
        "the shipped leg disagreed with the native arms: {label}"
    );
    assert!(
        matches!(live.1, LegSource::Live { .. }),
        "{label} did not come from the leg: {:?}",
        live.1
    );
}

/// Permits are opaque, so acceptance and the message are what parity can mean.
fn verify_permit<T>(
    label: &str,
    native: &Result<T, String>,
    live: (
        Result<T, String>,
        crate::agent::harness::live_policy::LegSource,
    ),
) {
    use crate::agent::harness::live_policy::LegSource;
    match (&native, &live.0) {
        (Ok(_), Ok(_)) => {}
        (Err(native), Err(shipped)) => assert_eq!(
            native, shipped,
            "the shipped leg refused with another message: {label}"
        ),
        _ => panic!("the shipped leg disagreed about acceptance: {label}"),
    }
    assert!(
        matches!(live.1, LegSource::Live { .. }),
        "{label} did not come from the leg: {:?}",
        live.1
    );
}

/// Every branch of the reducer, once through the shipped leg and once through
/// the untouched Rust arms: acceptance, next phase, stored sequence and message.
#[test]
fn the_shipped_episode_reducer_leg_matches_the_native_state_machine() {
    let leg = crate::agent::harness::live_policy::shipped_leg("episode-reducer");
    let fixture = leg_fixture();
    let start_seq = fixture.start.start_journal_event.seq;

    let crossed_start = resealed(
        EpisodeEventV1::new(
            "episode-9".into(),
            fixture.started.sequence,
            fixture.started.previous_sha256.clone(),
            fixture.started.event.clone(),
        )
        .unwrap(),
    );
    let duplicate_start = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.started.sequence + 1,
            fixture.started.event_sha256.clone(),
            EpisodeEventKindV1::Started(Box::new(fixture.start.clone())),
        )
        .unwrap(),
    );
    let mut wrong_schema = fixture.started.clone();
    wrong_schema.schema = DEEP_CUT_EPISODE_SCHEMA_V1.to_string() + "-stale";
    let mut wrong_sequence = fixture.started.clone();
    wrong_sequence.sequence = 5;
    let mut wrong_previous = fixture.started.clone();
    wrong_previous.previous_sha256 = digest('9');
    let mut forged_digest = fixture.started.clone();
    forged_digest.event_sha256 = digest('8');
    let other_episode = resealed(
        EpisodeEventV1::new(
            "episode-9".into(),
            fixture.started.sequence + 1,
            fixture.started.event_sha256.clone(),
            EpisodeEventKindV1::EvidenceLinked(link(
                EpisodeEvidenceKindV1::OfficialResult,
                "official",
            )),
        )
        .unwrap(),
    );

    let evidence = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.started.sequence + 1,
            fixture.started.event_sha256.clone(),
            EpisodeEventKindV1::EvidenceLinked(link(
                EpisodeEvidenceKindV1::OfficialResult,
                "official-r1",
            )),
        )
        .unwrap(),
    );
    let mut forged_identity = evidence.clone();
    if let EpisodeEventKindV1::EvidenceLinked(link) = &mut forged_identity.event {
        link.identity = "not an id".into();
    }
    let forged_identity = resealed(forged_identity);
    let mut forged_receipt = evidence.clone();
    if let EpisodeEventKindV1::EvidenceLinked(link) = &mut forged_receipt.event {
        link.receipt_sha256 = digest('7').to_uppercase();
    }
    let forged_receipt = resealed(forged_receipt);
    let terminal_ok = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.started.sequence + 1,
            fixture.started.event_sha256.clone(),
            EpisodeEventKindV1::Terminal(terminal(EpisodeTerminalOutcomeV1::Success, start_seq)),
        )
        .unwrap(),
    );
    let terminal_backward = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.started.sequence + 1,
            fixture.started.event_sha256.clone(),
            EpisodeEventKindV1::Terminal(terminal(
                EpisodeTerminalOutcomeV1::Success,
                start_seq.saturating_sub(1),
            )),
        )
        .unwrap(),
    );
    let mut invalid_terminal = terminal(EpisodeTerminalOutcomeV1::Recovered, start_seq + 1);
    invalid_terminal.recovered_from = None;
    let terminal_invalid = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.started.sequence + 1,
            fixture.started.event_sha256.clone(),
            EpisodeEventKindV1::Terminal(invalid_terminal),
        )
        .unwrap(),
    );
    let mut bad_head = terminal(EpisodeTerminalOutcomeV1::Success, start_seq + 1);
    bad_head.action_journal_head_sha256 = "short".into();
    let terminal_bad_head = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.started.sequence + 1,
            fixture.started.event_sha256.clone(),
            EpisodeEventKindV1::Terminal(bad_head),
        )
        .unwrap(),
    );

    let official_after = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.terminal_event.sequence + 1,
            fixture.terminal_event.event_sha256.clone(),
            EpisodeEventKindV1::EvidenceLinked(link(
                EpisodeEvidenceKindV1::OfficialResult,
                "official-r2",
            )),
        )
        .unwrap(),
    );
    let candidate_after = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.terminal_event.sequence + 1,
            fixture.terminal_event.event_sha256.clone(),
            EpisodeEventKindV1::EvidenceLinked(link(
                EpisodeEvidenceKindV1::Candidate,
                "candidate-r2",
            )),
        )
        .unwrap(),
    );
    let stale_after = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.terminal_event.sequence,
            fixture.terminal_event.event_sha256.clone(),
            EpisodeEventKindV1::EvidenceLinked(link(
                EpisodeEvidenceKindV1::OfficialResult,
                "official-r2",
            )),
        )
        .unwrap(),
    );
    let start_after = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.terminal_event.sequence + 1,
            fixture.terminal_event.event_sha256.clone(),
            EpisodeEventKindV1::Started(Box::new(fixture.start.clone())),
        )
        .unwrap(),
    );

    let table: [(&str, &EpisodeStateV1, &EpisodeEventV1); 21] = [
        ("empty/start", &fixture.empty, &fixture.started),
        ("empty/evidence", &fixture.empty, &evidence),
        ("empty/terminal", &fixture.empty, &terminal_ok),
        ("empty/crossed-start", &fixture.empty, &crossed_start),
        ("empty/wrong-schema", &fixture.empty, &wrong_schema),
        ("empty/wrong-sequence", &fixture.empty, &wrong_sequence),
        ("empty/wrong-previous", &fixture.empty, &wrong_previous),
        ("empty/forged-digest", &fixture.empty, &forged_digest),
        ("open/second-start", &fixture.open, &duplicate_start),
        ("open/evidence", &fixture.open, &evidence),
        ("open/forged-identity", &fixture.open, &forged_identity),
        ("open/forged-receipt", &fixture.open, &forged_receipt),
        ("open/terminal", &fixture.open, &terminal_ok),
        ("open/terminal-backward", &fixture.open, &terminal_backward),
        ("open/terminal-invalid", &fixture.open, &terminal_invalid),
        ("open/terminal-bad-head", &fixture.open, &terminal_bad_head),
        ("open/other-episode", &fixture.open, &other_episode),
        ("closed/official", &fixture.closed, &official_after),
        ("closed/candidate", &fixture.closed, &candidate_after),
        ("closed/stale", &fixture.closed, &stale_after),
        ("closed/second-start", &fixture.closed, &start_after),
    ];
    for (label, state, event) in table {
        verify(
            &format!("validate/{label}"),
            state.validate_next_native(event),
            state.validate_next_with(Some(leg.clone()), event),
        );
        verify(
            &format!("durable/{label}"),
            state.append_native(event),
            state.append_with(Some(leg.clone()), event),
        );
    }

    let closed_terminal = resealed(
        EpisodeEventV1::new(
            fixture.start.episode_id.clone(),
            fixture.terminal_event.sequence + 1,
            fixture.terminal_event.event_sha256.clone(),
            fixture.terminal_event.event.clone(),
        )
        .unwrap(),
    );
    verify(
        "validate/closed/second-terminal",
        fixture.closed.validate_next_native(&closed_terminal),
        fixture
            .closed
            .validate_next_with(Some(leg.clone()), &closed_terminal),
    );

    let exhausted = EpisodeStateV1::default().at_sequence_for_test(u64::MAX);
    let exhausted_event = EpisodeEventV1::new(
        fixture.start.episode_id.clone(),
        u64::MAX,
        String::new(),
        EpisodeEventKindV1::Started(Box::new(fixture.start.clone())),
    )
    .unwrap();
    verify(
        "durable/last-sequence",
        exhausted.append_native(&exhausted_event),
        exhausted.append_with(Some(leg.clone()), &exhausted_event),
    );

    for (label, state) in [
        ("empty", &fixture.empty),
        ("open", &fixture.open),
        ("closed", &fixture.closed),
    ] {
        let native = state.effect_permit_native();
        let live = state.effect_permit_with(Some(leg.clone()));
        verify_permit(&format!("effect/{label}"), &native, live);

        let native = state.evidence_permit_native();
        let live = state.evidence_permit_with(Some(leg.clone()));
        verify_permit(&format!("evidence/{label}"), &native, live);
    }

    // The live evidence permit carries the same terminal flag: a candidate link
    // is refused through it exactly when the native permit refuses one.
    let live_closed_permit = fixture
        .closed
        .evidence_permit_with(Some(leg.clone()))
        .0
        .unwrap();
    verify(
        "link/candidate-through-the-live-permit",
        EpisodeEventV1::link_evidence_native(
            &fixture.closed.evidence_permit().unwrap(),
            link(EpisodeEvidenceKindV1::Candidate, "candidate-r2"),
        ),
        EpisodeEventV1::link_evidence_with(
            Some(leg.clone()),
            &live_closed_permit,
            link(EpisodeEvidenceKindV1::Candidate, "candidate-r2"),
        ),
    );

    let effect = fixture.open.effect_permit().unwrap();
    let evidence_body =
        EpisodeEventKindV1::EvidenceLinked(link(EpisodeEvidenceKindV1::OfficialResult, "official"));
    let mut forged_body = evidence_body.clone();
    if let EpisodeEventKindV1::EvidenceLinked(link) = &mut forged_body {
        link.identity = "not an id".into();
    }
    for (label, body) in [
        ("append/evidence", evidence_body),
        ("append/forged-identity", forged_body),
        (
            "append/start",
            EpisodeEventKindV1::Started(Box::new(fixture.start.clone())),
        ),
        (
            "append/terminal",
            EpisodeEventKindV1::Terminal(terminal(EpisodeTerminalOutcomeV1::Success, start_seq)),
        ),
        (
            "append/invalid-terminal",
            EpisodeEventKindV1::Terminal({
                let mut bad = terminal(EpisodeTerminalOutcomeV1::Recovered, start_seq);
                bad.recovered_from = None;
                bad
            }),
        ),
    ] {
        verify(
            label,
            EpisodeEventV1::append_native(&effect, body.clone()),
            EpisodeEventV1::append_with(Some(leg.clone()), &effect, body),
        );
    }

    let open_permit = fixture.open.evidence_permit().unwrap();
    let closed_permit = fixture.closed.evidence_permit().unwrap();
    for (label, permit, body) in [
        (
            "link/official-after-close",
            &closed_permit,
            link(EpisodeEvidenceKindV1::OfficialResult, "official-r2"),
        ),
        (
            "link/candidate-after-close",
            &closed_permit,
            link(EpisodeEvidenceKindV1::Candidate, "candidate-r2"),
        ),
        (
            "link/candidate-while-open",
            &open_permit,
            link(EpisodeEvidenceKindV1::Candidate, "candidate-r1"),
        ),
        (
            "link/forged-identity",
            &open_permit,
            EpisodeEvidenceLinkV1 {
                kind: EpisodeEvidenceKindV1::OfficialResult,
                identity: "not an id".into(),
                receipt_sha256: digest('5'),
            },
        ),
    ] {
        verify(
            label,
            EpisodeEventV1::link_evidence_native(permit, body.clone()),
            EpisodeEventV1::link_evidence_with(Some(leg.clone()), permit, body),
        );
    }

    let mut crossed_start_event = fixture.start.clone();
    crossed_start_event.episode_id = "episode-9".into();
    let mut invalid_start = fixture.start.clone();
    invalid_start.start_journal_event.update.phase = ActionPhaseV1::Planned;
    let mut later_start = fixture.start.clone();
    later_start.started_at_ms = 11;
    for (label, receipt, start, completed_at) in [
        ("complete/at-the-start", &fixture.opened, &fixture.start, 10),
        (
            "complete/after-the-start",
            &fixture.opened,
            &fixture.start,
            11,
        ),
        (
            "complete/before-the-start",
            &fixture.opened,
            &fixture.start,
            9,
        ),
        (
            "complete/another-episode",
            &fixture.opened,
            &crossed_start_event,
            11,
        ),
        (
            "complete/another-digest",
            &fixture.opened,
            &later_start,
            999,
        ),
        (
            "complete/not-the-start-sequence",
            &fixture.closed_receipt,
            &fixture.start,
            999,
        ),
        (
            "complete/invalid-start",
            &fixture.opened,
            &invalid_start,
            11,
        ),
    ] {
        verify(
            label,
            receipt.start_action_completion_native(start, completed_at),
            receipt.start_action_completion_with(Some(leg.clone()), start, completed_at),
        );
    }
}

/// An out-of-contract append and an out-of-contract phase both fall back to the
/// native arms, so a bad hot edit never moves the episode's state machine.
#[test]
fn an_out_of_contract_episode_reducer_leg_keeps_the_native_decision() {
    use crate::agent::harness::live_policy::LegSource;
    let fixture = leg_fixture();
    let evidence = EpisodeEventV1::append(
        &fixture.open.effect_permit().unwrap(),
        EpisodeEventKindV1::EvidenceLinked(link(EpisodeEvidenceKindV1::OfficialResult, "official")),
    )
    .unwrap();

    let wrong_sequence = temporary_policy_leg(
        "episode-reducer-sequence",
        "module.exports={apiVersion:1,render:()=>({accept:true,fault:null,phase:'none',next_hi:0,next_lo:99,post_terminal:false})}",
    );
    let (live, source) = fixture.open.append_with(Some(wrong_sequence), &evidence);
    assert_eq!(live, fixture.open.append_native(&evidence));
    assert_eq!(live, Ok(2));
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} kept the leg"
    );

    let wrong_phase = temporary_policy_leg(
        "episode-reducer-phase",
        "module.exports={apiVersion:1,render:()=>({accept:true,fault:null,phase:'closed',next_hi:0,next_lo:0,post_terminal:false})}",
    );
    let (live, source) = fixture
        .empty
        .validate_next_with(Some(wrong_phase), &fixture.started);
    assert_eq!(live, fixture.empty.validate_next_native(&fixture.started));
    assert_eq!(live, Ok(()));
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} kept the leg"
    );

    let accepted_after_close = temporary_policy_leg(
        "episode-reducer-effect",
        "module.exports={apiVersion:1,render:()=>({accept:true,fault:null,phase:'none',next_hi:0,next_lo:0,post_terminal:false})}",
    );
    let (live, source) = fixture
        .closed
        .effect_permit_with(Some(accepted_after_close));
    assert!(live.is_err());
    assert_eq!(live.err(), fixture.closed.effect_permit_native().err());
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{source:?} kept the leg"
    );
}

/// 200 decisions through the pinned generation, for the receipt's timing note.
#[test]
fn the_episode_reducer_leg_decides_in_microseconds() {
    use crate::agent::harness::live_policy::shipped_leg;
    let leg = shipped_leg("episode-reducer");
    let fixture = leg_fixture();
    let evidence = EpisodeEventV1::append(
        &fixture.open.effect_permit().unwrap(),
        EpisodeEventKindV1::EvidenceLinked(link(EpisodeEvidenceKindV1::OfficialResult, "official")),
    )
    .unwrap();
    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (result, source) = fixture.open.append_with(Some(leg.clone()), &evidence);
        samples.push(started.elapsed());
        assert_eq!(source.as_str(), "live");
        assert_eq!(result, Ok(2));
    }
    let mut native = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let result = fixture.open.append_native(&evidence);
        native.push(started.elapsed());
        assert_eq!(result, Ok(2));
    }
    let mut permits = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (permit, source) = fixture.open.effect_permit_with(Some(leg.clone()));
        permits.push(started.elapsed());
        assert!(permit.is_ok());
        assert_eq!(source.as_str(), "live");
    }
    samples.sort();
    native.sort();
    permits.sort();
    eprintln!(
        "episode-reducer leg: {:.1} µs median, {:.1} µs p95 over 200 decisions (native arms: {:.1} µs median; effect permit: {:.1} µs median)",
        samples[100].as_secs_f64() * 1e6,
        samples[190].as_secs_f64() * 1e6,
        native[100].as_secs_f64() * 1e6,
        permits[100].as_secs_f64() * 1e6
    );
}

/// With no live generation offered, the shipped module still answers and the
/// receipt says where the answer came from.
#[test]
fn a_turn_with_no_live_generation_uses_the_shipped_episode_reducer() {
    use crate::agent::harness::live_policy::LegSource;
    let fixture = leg_fixture();
    let (permit, source) = fixture.open.effect_permit_with(None);
    permit.expect("the shipped reducer grants the open episode's effect permit");
    assert!(
        matches!(source, LegSource::Shipped { .. }),
        "{source:?} did not name the shipped leg"
    );
    assert!(fixture.open.effect_permit_native().is_ok());
}
