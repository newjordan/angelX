use super::{FlywheelSettleFailureV1, settle_fires};
use crate::drive::competition::adapters::CompetitionAdapterV1;
use crate::drive::competition::adapters::fixture::MemoryRawBoardJournalV1;
use crate::drive::competition::adapters::flywheel::FlywheelAdapterV1;
use crate::drive::competition::board_reducer::BoardReducerV1;
use crate::drive::competition::rewards::{RewardLedgerV1, RewardSelectionV1};
use std::path::{Path, PathBuf};

fn fixture(track: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/cockpit/competition/fixtures/flywheel")
        .join(track)
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "angel-flywheel-results-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The x86 fixture holds one promoted, one rejected, one in-flight, and one
/// phantom fire: exactly two official bindings, the phantom surfaced as a
/// typed failure, in-flight ignored.
#[test]
fn fixture_fires_settle_into_two_bindings_with_phantom_surfaced() {
    let adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let owned = adapter.personal_submission_ids();
    let mut ledger = RewardLedgerV1::default();
    let settlement = settle_fires(
        &fixture("x86"),
        adapter.identity(),
        adapter.competition(),
        &owned,
        &mut ledger,
    );
    assert_eq!(settlement.rewards.len(), 2, "promoted + rejected bind");
    assert_eq!(ledger.bindings().len(), 2);
    let promoted = &settlement.rewards[0];
    assert_eq!(promoted.delta_pct, Some(0.0369));
    assert!(promoted.promoted);
    let rejected = &settlement.rewards[1];
    assert!(!rejected.promoted);
    assert_eq!(settlement.failures.len(), 1, "in-flight binds nothing");
    assert_eq!(
        settlement.failures[0],
        FlywheelSettleFailureV1::Phantom {
            submission_id: "c3df1e71-4a6f-4ad2-9f39-1117e03d5647".into()
        }
    );
    assert!(ledger.validate().is_ok());
}

#[test]
fn effective_reward_returns_the_official_delta_for_bound_candidates() {
    let adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let owned = adapter.personal_submission_ids();
    let mut ledger = RewardLedgerV1::default();
    let settlement = settle_fires(
        &fixture("x86"),
        adapter.identity(),
        adapter.competition(),
        &owned,
        &mut ledger,
    );
    let promoted_episode = format!("flywheel-episode-{}", settlement.rewards[0].candidate);
    let rejected_episode = format!("flywheel-episode-{}", settlement.rewards[1].candidate);
    for (episode, expected) in [
        (promoted_episode.as_str(), "541.05972012"),
        (rejected_episode.as_str(), "-27288.2126397"),
    ] {
        match ledger.effective_reward(episode) {
            Some(RewardSelectionV1::Official(binding)) => {
                assert_eq!(binding.oriented_delta.as_str(), expected);
                assert_eq!(binding.official_result_revision, 1);
            }
            other => panic!("official reward expected for {episode}, got {other:?}"),
        }
    }
    let in_flight = "flywheel-episode-599aeb4fdeadbeefdeadbeefdeadbeefdeadbeef";
    assert!(ledger.effective_reward(in_flight).is_none());
}

#[test]
fn apple_fixture_settles_the_same_way() {
    let adapter = FlywheelAdapterV1::open(&fixture("apple")).unwrap();
    let owned = adapter.personal_submission_ids();
    let mut ledger = RewardLedgerV1::default();
    let settlement = settle_fires(
        &fixture("apple"),
        adapter.identity(),
        adapter.competition(),
        &owned,
        &mut ledger,
    );
    assert_eq!(settlement.rewards.len(), 2);
    assert_eq!(ledger.bindings().len(), 2);
    assert!(matches!(
        &settlement.failures[..],
        [FlywheelSettleFailureV1::Phantom { submission_id }]
            if submission_id == "fdefddf5-5dfd-4c92-8056-c1860b240866"
    ));
}

#[test]
fn missing_fires_file_is_the_fresh_track_state_not_a_failure() {
    let adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let dir = scratch("missing-fires");
    std::fs::copy(fixture("x86").join("board.json"), dir.join("board.json")).unwrap();
    std::fs::copy(
        fixture("x86").join("api-rows.json"),
        dir.join("api-rows.json"),
    )
    .unwrap();
    let owned = adapter.personal_submission_ids();
    let mut ledger = RewardLedgerV1::default();
    let settlement = settle_fires(
        &dir,
        adapter.identity(),
        adapter.competition(),
        &owned,
        &mut ledger,
    );
    assert!(settlement.rewards.is_empty());
    assert!(
        settlement.failures.is_empty(),
        "a track that never fired has no fires.jsonl; that is not a failure: {:?}",
        settlement.failures
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn truncated_fires_line_is_a_typed_malformed_failure_not_a_panic() {
    let adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let dir = scratch("truncated-fires");
    for name in ["board.json", "api-rows.json"] {
        std::fs::copy(fixture("x86").join(name), dir.join(name)).unwrap();
    }
    let body = std::fs::read_to_string(fixture("x86").join("fires.jsonl")).unwrap();
    let cut = &body[..body.len() / 2];
    std::fs::write(dir.join("fires.jsonl"), cut).unwrap();
    let owned = adapter.personal_submission_ids();
    let mut ledger = RewardLedgerV1::default();
    let settlement = settle_fires(
        &dir,
        adapter.identity(),
        adapter.competition(),
        &owned,
        &mut ledger,
    );
    assert!(settlement.rewards.is_empty());
    assert_eq!(settlement.failures.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The full live path: the same fires also flow through the board reducer's
/// in-memory journal without disturbing reward binding (journal is board-only).
#[test]
fn settle_after_engage_keeps_board_and_rewards_independent() {
    let mut adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let mut reducer = BoardReducerV1::default();
    let mut journal = MemoryRawBoardJournalV1::default();
    reducer
        .engage("flywheel-results", &mut adapter, &mut journal, 1)
        .unwrap()
        .unwrap();
    let owned = adapter.personal_submission_ids();
    let mut ledger = RewardLedgerV1::default();
    let settlement = settle_fires(
        &fixture("x86"),
        adapter.identity(),
        adapter.competition(),
        &owned,
        &mut ledger,
    );
    assert_eq!(ledger.bindings().len(), 2);
    assert_eq!(settlement.rewards.len(), 2);
    assert!(journal.observations().len() == 1);
}
