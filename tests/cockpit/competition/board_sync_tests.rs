use super::run;
use crate::drive::competition::adapters::AdapterFailureClassV1;
use std::path::{Path, PathBuf};

fn fixture(track: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/cockpit/competition/fixtures/flywheel")
        .join(track)
}

#[test]
fn board_sync_report_matches_the_fixture_board_and_binds_official_results() {
    let report: serde_json::Value =
        serde_json::from_str(&run(&fixture("x86")).unwrap()).expect("one JSON object");
    assert_eq!(report["schema"], "angel.board-sync/v1");
    assert_eq!(report["identity"]["competition"]["platform_id"], "yukon");
    assert_eq!(
        report["identity"]["competition"]["competition_id"],
        "eigenlabs/flock-challenge-multi/x86"
    );
    assert_eq!(report["identity"]["comparator"]["kind"], "higher_is_better");
    let canonical = &report["canonical_board"];
    assert_eq!(canonical["score"], "1615101.60914411");
    assert_eq!(
        canonical["commit"],
        "da9435c5f95c627c5d67343ea21596c6e8b14746"
    );
    assert_eq!(canonical["solver"], "jacklightChen");
    assert_eq!(canonical["epoch"], 1);
    assert_eq!(
        report["personal"]["in_flight"]
            .as_array()
            .expect("in_flight array")
            .len(),
        0
    );
    let best = &report["personal"]["best"];
    assert_eq!(best["solver"], "newjordan");
    assert_eq!(
        best["submission_id"],
        "70a82356-0548-47e0-bd97-e7a5e5a4bc45"
    );
    assert_eq!(best["status"], "rejected");
    let rewards = report["rewards"].as_array().expect("rewards array");
    assert_eq!(rewards.len(), 2, "promoted + rejected bind");
    assert_eq!(
        rewards[0]["candidate"],
        "ae8ef1597e21864170e2a8b36d5c09b28ee1e46c"
    );
    assert_eq!(rewards[0]["delta_pct"], 0.0369);
    assert_eq!(rewards[0]["promoted"], true);
    assert_eq!(rewards[1]["promoted"], false);
    assert_eq!(rewards[1]["delta_pct"], -1.6896);
    let failures = report["failures"].as_array().expect("failures array");
    assert_eq!(failures.len(), 1, "phantom surfaced, in-flight ignored");
    assert_eq!(failures[0]["kind"], "phantom");
    assert_eq!(
        failures[0]["submission_id"],
        "c3df1e71-4a6f-4ad2-9f39-1117e03d5647"
    );
}

#[test]
fn apple_fixture_reports_its_own_board_and_results() {
    let report: serde_json::Value =
        serde_json::from_str(&run(&fixture("apple")).unwrap()).expect("one JSON object");
    assert_eq!(
        report["identity"]["competition"]["competition_id"],
        "eigenlabs/flock-challenge"
    );
    assert_eq!(report["identity"]["competition"]["hardware_id"], "apple");
    let canonical = &report["canonical_board"];
    assert_eq!(canonical["score"], "1851759.05313334");
    assert_eq!(canonical["solver"], "pepedesigner");
    assert_eq!(
        report["rewards"][0]["candidate"],
        "1e51febbafed271fe6c1f8e7465abff3126abdd6"
    );
    assert_eq!(report["rewards"][0]["promoted"], true);
    assert_eq!(report["failures"][0]["kind"], "phantom");
    assert_eq!(
        report["failures"][0]["submission_id"],
        "fdefddf5-5dfd-4c92-8056-c1860b240866"
    );
}

#[test]
fn unreadable_state_dir_is_an_error_not_a_report() {
    let empty =
        std::env::temp_dir().join(format!("angel-board-sync-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&empty);
    let failure = run(&empty).unwrap_err();
    assert_eq!(failure.class, AdapterFailureClassV1::Transport);
    // Truncated JSON in a state file is also typed, never a panic.
    let dir =
        std::env::temp_dir().join(format!("angel-board-sync-truncated-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture("x86").join("board.json"), dir.join("board.json")).unwrap();
    let body = std::fs::read(fixture("x86").join("api-rows.json")).unwrap();
    std::fs::write(dir.join("api-rows.json"), &body[..400]).unwrap();
    let failure = run(&dir).unwrap_err();
    assert_eq!(failure.class, AdapterFailureClassV1::Malformed);
    let _ = std::fs::remove_dir_all(&dir);
}
