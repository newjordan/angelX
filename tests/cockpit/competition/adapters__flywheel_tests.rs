use super::FlywheelAdapterV1;
use crate::competition::adapters::fixture::MemoryRawBoardJournalV1;
use crate::competition::adapters::{AdapterFailureClassV1, CompetitionAdapterV1};
use crate::competition::board_reducer::BoardReducerV1;
use crate::competition::schema::{ComparatorKindV1, ScoreV1};
use std::io::Write;
use std::path::{Path, PathBuf};

fn fixture(track: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/competition/test_support/fixtures/flywheel")
        .join(track)
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "angel-flywheel-adapter-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_truncated(source: &Path, destination: &Path, bytes: usize) {
    let body = std::fs::read(source).unwrap();
    let mut file = std::fs::File::create(destination).unwrap();
    file.write_all(&body[..bytes.min(body.len())]).unwrap();
}

fn top_promoted_row(track: &str) -> serde_json::Value {
    let capture: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(fixture(track).join("api-rows.json")).unwrap())
            .unwrap();
    let mut promoted: Vec<&serde_json::Value> = capture["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["promotionStatus"] == "promoted" && !row["officialScore"].is_null())
        .collect();
    promoted.sort_by_key(|row| {
        row["promotionFinishedAt"]
            .as_str()
            .unwrap_or("")
            .to_string()
    });
    promoted.pop().unwrap().clone()
}

fn me_scored_rows(track: &str, me: &str) -> usize {
    let capture: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(fixture(track).join("api-rows.json")).unwrap())
            .unwrap();
    capture["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            row["solverUsername"] == me
                && !row["officialScore"].is_null()
                && row["officialScore"].is_number()
        })
        .count()
}

#[test]
fn opens_x86_fixture_with_expected_identity() {
    let mut adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let identity = adapter.identify_competition().unwrap();
    assert_eq!(identity.competition.platform_id, "yukon");
    assert_eq!(
        identity.competition.competition_id,
        "eigenlabs/flock-challenge-multi/x86"
    );
    assert_eq!(
        identity.competition.benchmark_id,
        "8abde9cf-9256-4453-99e2-9de8632a7943"
    );
    assert_eq!(identity.competition.hardware_id, "x86");
    assert_eq!(identity.comparator.kind, ComparatorKindV1::HigherIsBetter);
    assert_eq!(adapter.threshold_bips(), 100);
    assert_eq!(
        adapter.me(),
        Some("newjordan"),
        "me is derived from the my_best submission when board.json lacks it"
    );
    assert_eq!(
        adapter.identity().adapter_id,
        crate::competition::adapters::flywheel::FLYWHEEL_ADAPTER_ID
    );
}

#[test]
fn frontier_observation_is_the_latest_promoted_row() {
    let expected = top_promoted_row("x86");
    let mut adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let mut reducer = BoardReducerV1::default();
    let mut journal = MemoryRawBoardJournalV1::default();
    let outcome = reducer
        .engage("flywheel-sync", &mut adapter, &mut journal, 1)
        .unwrap()
        .unwrap();
    let canonical = outcome.canonical.expect("canonical board");
    let frontier = canonical.frontier.global_target.expect("global target");
    assert_eq!(
        frontier.score,
        ScoreV1::new(expected["officialScore"].to_string().replace('"', "")).unwrap()
    );
    assert_eq!(frontier.submission_id.as_deref(), expected["id"].as_str());
    assert_eq!(
        frontier.participant_id,
        expected["solverUsername"].as_str().unwrap()
    );
    let frontier_row = adapter.frontier_row().unwrap();
    assert_eq!(
        frontier_row.commit(),
        expected["promotedSourceRef"].as_str()
    );
    assert_eq!(canonical.board_epoch, 1);
}

#[test]
fn personal_observations_cover_the_me_rows() {
    let mut adapter = FlywheelAdapterV1::open(&fixture("x86")).unwrap();
    let personal = adapter.fetch_personal_submissions(None).unwrap();
    assert_eq!(
        personal.entries.len(),
        me_scored_rows("x86", "newjordan"),
        "one entry per scored me row"
    );
    assert!(personal.entries.iter().all(|entry| entry.personal));
    let mut rival = FlywheelAdapterV1::open_as(&fixture("x86"), "jacklightChen").unwrap();
    let rival_personal = rival.fetch_personal_submissions(None).unwrap();
    assert_eq!(
        rival_personal.entries.len(),
        me_scored_rows("x86", "jacklightChen")
    );
}

#[test]
fn missing_state_files_are_typed_failures() {
    let empty = scratch("missing");
    let failure = FlywheelAdapterV1::open(&empty).unwrap_err();
    assert_eq!(failure.class, AdapterFailureClassV1::Transport);
    let board_only = scratch("board-only");
    std::fs::copy(
        fixture("x86").join("board.json"),
        board_only.join("board.json"),
    )
    .unwrap();
    let failure = FlywheelAdapterV1::open(&board_only).unwrap_err();
    assert_eq!(failure.class, AdapterFailureClassV1::Transport);
    let _ = std::fs::remove_dir_all(&empty);
    let _ = std::fs::remove_dir_all(&board_only);
}

#[test]
fn truncated_json_is_a_malformed_failure_not_a_panic() {
    let dir = scratch("truncated");
    std::fs::copy(fixture("x86").join("board.json"), dir.join("board.json")).unwrap();
    write_truncated(
        &fixture("x86").join("api-rows.json"),
        &dir.join("api-rows.json"),
        400,
    );
    let failure = FlywheelAdapterV1::open(&dir).unwrap_err();
    assert_eq!(failure.class, AdapterFailureClassV1::Malformed);
    write_truncated(
        &fixture("x86").join("board.json"),
        &dir.join("board.json"),
        60,
    );
    let failure = FlywheelAdapterV1::open(&dir).unwrap_err();
    assert_eq!(failure.class, AdapterFailureClassV1::Malformed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adapter_is_shadow_read_only() {
    let dir = scratch("shadow");
    for name in [
        "board.json",
        "api-rows.json",
        "fires.jsonl",
        "mechanisms.jsonl",
    ] {
        std::fs::copy(fixture("x86").join(name), dir.join(name)).unwrap();
    }
    let before: Vec<u8> = std::fs::read(dir.join("api-rows.json")).unwrap();
    let mut adapter = FlywheelAdapterV1::open(&dir).unwrap();
    adapter.identify_competition().unwrap();
    adapter.fetch_board(None).unwrap();
    adapter.fetch_personal_submissions(None).unwrap();
    let after: Vec<u8> = std::fs::read(dir.join("api-rows.json")).unwrap();
    assert_eq!(before, after, "no adapter method may write");
    assert!(
        adapter
            .observe_source_access(None)
            .is_err_and(|failure| failure.class == AdapterFailureClassV1::Unsupported),
        "files cannot provide source access"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
