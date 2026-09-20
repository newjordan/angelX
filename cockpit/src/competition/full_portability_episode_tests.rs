use super::episode::{EpisodeEventKindV1, EpisodeEventV1, EpisodeEvidenceKindV1};
use super::episode_store::EpisodeStoreV1;
use super::full_portability::import_full_portable_state;
use super::full_portability_episode_fixture::{Temp, appended, digest, episode_bundle, update};
use super::journal::{ActionJournalStateV1, canonical_action_key};
use super::portability::PortableEntryV1;
use super::schema::ActionPhaseV1;
use super::store::CompetitionStore;

#[test]
fn full_state_nonempty_episode_replays_all_links_ac15() {
    let source = Temp::new("positive-source");
    let target = Temp::new("positive-target");
    let bundle = episode_bundle(&source);
    let encoded: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
    assert_eq!(
        encoded["observation_history"][0]["raw_sha256"],
        digest("raw-observation")
    );
    let receipt = import_full_portable_state(&bundle, target.path()).unwrap();
    assert!(receipt.snapshot.director.episodes["episode-i3"].terminal);
    assert_eq!(
        receipt
            .snapshot
            .director
            .board
            .as_ref()
            .unwrap()
            .latest_provenance
            .raw_sha256,
        digest("raw-observation-refresh")
    );
    let source_episode = EpisodeStoreV1::new(
        source.0.join("episodes"),
        CompetitionStore::new(source.0.join("action")),
    )
    .recover_episode("episode-i3")
    .unwrap();
    let target_episode = EpisodeStoreV1::new(
        target.0.join("episodes"),
        CompetitionStore::new(target.0.join("action")),
    )
    .recover_episode("episode-i3")
    .unwrap();
    assert_eq!(source_episode, target_episode);
}

#[derive(Clone, Copy)]
enum Mutation {
    Start,
    Candidate,
    Submission,
    Terminal,
}

#[test]
fn full_state_rejects_resealed_cross_composed_episode_links_ac15() {
    let source = Temp::new("negative-source");
    let bundle = episode_bundle(&source);
    for (index, mutation) in [
        Mutation::Start,
        Mutation::Candidate,
        Mutation::Submission,
        Mutation::Terminal,
    ]
    .into_iter()
    .enumerate()
    {
        let mut value: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
        mutate_episode(&mut value, mutation);
        let target = Temp::new(&format!("negative-target-{index}"));
        assert!(
            import_full_portable_state(&serde_json::to_vec(&value).unwrap(), target.path())
                .is_err()
        );
        assert!(!target.path().exists());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&bundle).unwrap();
    let observation = &mut value["observation_history"][0];
    let foreign = digest("foreign-historical-observation");
    observation["raw_sha256"] = foreign.clone().into();
    observation["source"]["observation_sha256"] = foreign.into();
    super::full_portability_observations::reseal_json_for_test(observation);
    super::full_portability_tests::reseal(&mut value);
    let target = Temp::new("negative-observation-history");
    assert!(
        import_full_portable_state(&serde_json::to_vec(&value).unwrap(), target.path()).is_err()
    );
    assert!(!target.path().exists());
}

fn mutate_episode(value: &mut serde_json::Value, mutation: Mutation) {
    let entry_value = value["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["path"].as_str().unwrap().starts_with("episodes/"))
        .unwrap();
    let mut entry: PortableEntryV1 = serde_json::from_value(entry_value.clone()).unwrap();
    let mut events = entry.body[..entry.body.len() - 1]
        .split(|byte| *byte == b'\n')
        .map(|line| serde_json::from_slice::<EpisodeEventV1>(line).unwrap())
        .collect::<Vec<_>>();
    match mutation {
        Mutation::Start => mutate_start(&mut events[0]),
        Mutation::Candidate => mutate_link(&mut events, EpisodeEvidenceKindV1::Candidate),
        Mutation::Submission => mutate_link(&mut events, EpisodeEvidenceKindV1::Submission),
        Mutation::Terminal => {
            let start = match &events[0].event {
                EpisodeEventKindV1::Started(start) => start,
                _ => unreachable!(),
            };
            let start_sequence = start.start_journal_event.seq;
            let start_head = start.start_journal_event.event_sha256.clone();
            let terminal = events
                .iter_mut()
                .find_map(|event| match &mut event.event {
                    EpisodeEventKindV1::Terminal(value) => Some(value),
                    _ => None,
                })
                .unwrap();
            terminal.action_journal_end_sequence = start_sequence;
            terminal.action_journal_head_sha256 = start_head;
        }
    }
    let mut head = String::new();
    for (sequence, event) in events.iter_mut().enumerate() {
        *event = EpisodeEventV1::new(
            event.episode_id.clone(),
            sequence as u64,
            head,
            event.event.clone(),
        )
        .unwrap();
        head = event.event_sha256.clone();
    }
    entry.body.clear();
    for event in events {
        entry.body.extend(serde_json::to_vec(&event).unwrap());
        entry.body.push(b'\n');
    }
    entry.content_sha256 = crate::cut::sha256_hex(&entry.body);
    *entry_value = serde_json::to_value(entry).unwrap();
    super::full_portability_tests::reseal(value);
}

fn mutate_link(events: &mut [EpisodeEventV1], kind: EpisodeEvidenceKindV1) {
    let link = events
        .iter_mut()
        .find_map(|event| match &mut event.event {
            EpisodeEventKindV1::EvidenceLinked(link) if link.kind == kind => Some(link),
            _ => None,
        })
        .unwrap();
    link.identity = format!("foreign-{:?}", kind).to_lowercase();
}

fn mutate_start(event: &mut EpisodeEventV1) {
    let start = match &mut event.event {
        EpisodeEventKindV1::Started(start) => start,
        _ => unreachable!(),
    };
    start.source.access_proof_sha256 = digest("foreign-access-proof");
    let mut intent = start.start_journal_event.update.intent.clone();
    intent.payload_sha256 = start.canonical_start_payload_sha256().unwrap();
    intent.action_key = canonical_action_key(&intent).unwrap();
    let mut isolated = ActionJournalStateV1::default();
    let planned = appended(
        isolated
            .prepare(update(intent.clone(), ActionPhaseV1::Planned, 10, None))
            .unwrap(),
    );
    isolated.apply(&planned).unwrap();
    start.start_journal_event = appended(
        isolated
            .prepare(update(intent, ActionPhaseV1::Started, 11, None))
            .unwrap(),
    );
}
