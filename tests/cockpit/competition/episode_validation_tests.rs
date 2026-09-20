use super::*;

#[test]
fn profile_identity_is_embedded_golden_and_install_safe() {
    let profile = DeepCutProfileV1::embedded();
    let identity = profile.identity().unwrap();
    assert_eq!(identity.policy_sha256, DEEP_CUT_POLICY_SHA256_V1);
    assert_eq!(identity.profile_version, DEEP_CUT_PROFILE_VERSION_V1);
    let encoded = serde_json::to_vec(&profile).unwrap();
    assert_eq!(
        serde_json::from_slice::<DeepCutProfileV1>(&encoded).unwrap(),
        profile
    );
    let mut untrusted = identity;
    untrusted.profile_version = "caller/v9".into();
    assert!(untrusted.validate().is_err());
    let mut drift = profile;
    drift.capture_policy_version = "drift/v2".into();
    assert!(drift.validate().is_err());
}

#[test]
fn start_rejects_payload_version_key_and_journal_receipt_forgery() {
    let valid = start_receipt(false);
    valid.validate().unwrap();
    let mut wrong_payload = valid.clone();
    let mut intent = wrong_payload.start_journal_event.update.intent.clone();
    intent.payload_sha256 = digest('9');
    intent.action_key = canonical_action_key(&intent).unwrap();
    wrong_payload.start_journal_event = journal_started(intent);
    assert!(wrong_payload.validate().is_err());
    let mut wrong_version = valid.clone();
    let mut intent = wrong_version.start_journal_event.update.intent.clone();
    intent.intent_version = "caller-start/v9".into();
    intent.action_key = canonical_action_key(&intent).unwrap();
    wrong_version.start_journal_event = journal_started(intent);
    assert!(wrong_version.validate().is_err());
    let mut wrong_hash = valid.clone();
    wrong_hash.start_journal_event.event_sha256 = digest('0');
    assert!(wrong_hash.validate().is_err());
    let mut wrong_sequence = valid.clone();
    wrong_sequence.start_journal_event.seq += 1;
    assert!(wrong_sequence.validate().is_err());
    let mut wrong_key = valid;
    wrong_key.start_journal_event.update.intent.action_key = digest('8');
    assert!(wrong_key.validate().is_err());
}

#[test]
fn replay_ancestry_is_strict_and_same_base_new_epoch_is_valid() {
    let valid = start_receipt(true);
    valid.validate().unwrap();
    let mut origin_only = valid.clone();
    origin_only.replay = None;
    assert!(origin_only.validate().is_err());
    let mut replay_only = valid.clone();
    replay_only.origin_episode_id = None;
    assert!(replay_only.validate().is_err());
    let mut self_replay = valid.clone();
    self_replay.origin_episode_id = Some(self_replay.episode_id.clone());
    self_replay.replay.as_mut().unwrap().replay_of_episode_id = self_replay.episode_id.clone();
    assert!(self_replay.validate().is_err());
    let mut blank_candidate = valid.clone();
    blank_candidate
        .replay
        .as_mut()
        .unwrap()
        .replay_of_candidate_id = Some("".into());
    assert!(blank_candidate.validate().is_err());
    let mut non_older = valid.clone();
    non_older.replay.as_mut().unwrap().predecessor_board_epoch = non_older.board_epoch;
    assert!(non_older.validate().is_err());
    let mut stale_source_epoch = valid;
    stale_source_epoch.source.source_board_epoch = 1;
    assert!(stale_source_epoch.validate().is_err());
}

#[test]
fn every_terminal_outcome_keeps_its_negative_evidence_shape() {
    for outcome in [
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
    ] {
        super::super::episode::validate_terminal(&terminal(outcome, 7)).unwrap();
    }
}

#[test]
fn unsynced_started_action_never_opens_effect_gate() {
    let start = start_receipt(false);
    let event = EpisodeEventV1::started(start).unwrap();
    let mut state = EpisodeStateV1::default();
    let mut authority = MemoryAuthority::default();
    assert!(state.append_durable(&mut authority, &event).is_err());
    assert!(authority.events.is_empty());
    assert!(state.effect_permit().is_err());
}

#[test]
fn append_receipt_is_bound_to_exact_started_event_digest() {
    let start_a = start_receipt(false);
    let event_a = EpisodeEventV1::started(start_a.clone()).unwrap();
    let mut state = EpisodeStateV1::default();
    let mut authority = MemoryAuthority::with_start(&start_a);
    let receipt_a = state.append_durable(&mut authority, &event_a).unwrap();
    let mut start_b = start_receipt(false);
    start_b.model_id = "different-model".into();
    rebind_start(&mut start_b);
    start_b.validate().unwrap();
    assert!(receipt_a.start_action_completion(&start_b, 11).is_err());
}
