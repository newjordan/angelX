use super::*;

fn workspace(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "angel-brief-{tag}-{}-{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const SPEC: &str = r#"{
  "schemaVersion": 2,
  "name": "qsb-grind-benchmark",
  "tracks": [{
    "name": "pinning",
    "description": "Maximize verified candidate throughput on a single GPU.",
    "direction": "+",
    "editablePaths": ["candidates/pinning"],
    "setupCommand": ["bash", "-lc", "./setup.sh pinning"],
    "benchmarkCommand": ["bash", "-lc", "./benchmark.sh pinning"],
    "scorePath": "score-pinning.json",
    "minScoreImprovementBips": 100
  }]
}"#;

#[test]
fn benchmark_json_becomes_a_card_with_its_local_score() {
    let root = workspace("card");
    write(&root, "challenge/benchmark.json", SPEC);
    write(
        &root,
        "challenge/score-pinning.json",
        "{\n  \"score\": 1.5e9\n}\n",
    );
    let walk = walk(&root);
    let card = benchmark(&root, &walk, now_secs());
    assert!(
        card.contains("challenge/benchmark.json (qsb-grind-benchmark):"),
        "{card}"
    );
    assert!(
        card.contains("- track pinning (higher is better): Maximize"),
        "{card}"
    );
    assert!(card.contains("editable: candidates/pinning"), "{card}");
    assert!(
        card.contains("run: ./benchmark.sh pinning (in challenge/)"),
        "{card}"
    );
    assert!(
        card.contains("setup: ./setup.sh pinning (in challenge/)"),
        "{card}"
    );
    assert!(card.contains("100 bips (1.00%)"), "{card}");
    assert!(
        card.contains(
            "score file score-pinning.json in challenge/: just now, { \"score\": 1.5e9 }"
        ),
        "{card}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn clones_of_one_benchmark_share_a_card() {
    let root = workspace("clones");
    for clone in ["challenge", "pin-a", "pin-b"] {
        write(&root, &format!("{clone}/benchmark.json"), SPEC);
    }
    write(&root, "pin-b/score-pinning.json", "{\"score\": 29.0}");
    let walk = walk(&root);
    let card = benchmark(&root, &walk, now_secs());
    assert_eq!(card.matches("(qsb-grind-benchmark):").count(), 1, "{card}");
    assert!(
        card.contains("the same spec is in 2 more copies: pin-a/, pin-b/"),
        "{card}"
    );
    assert!(
        card.contains("score file score-pinning.json in pin-b/: just now, {\"score\": 29.0}"),
        "{card}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_map_names_nested_repos_and_folds_sibling_runs() {
    let root = workspace("map");
    std::fs::create_dir_all(root.join("challenge/.git")).unwrap();
    write(&root, "challenge/benchmark.sh", "#!/bin/sh\n");
    for n in 1..=5 {
        std::fs::create_dir_all(root.join(format!("bench-iteration{n}"))).unwrap();
    }
    write(&root, "HANDOFF.md", "# handoff\n");
    std::fs::create_dir_all(root.join("target/debug")).unwrap();
    let walk = walk(&root);
    assert!(
        !walk
            .entries
            .iter()
            .any(|e| e.rel.starts_with("target/debug")),
        "build output is not entered"
    );
    let map = workspace_map(&root, &walk);
    assert!(map.contains("(not a git repository)"), "{map}");
    assert!(
        map.contains("1 repositories inside the workspace (their changes do not show in the top-level git diff):"),
        "{map}"
    );
    assert!(map.contains("  challenge/"), "{map}");
    assert!(map.contains("bench-* (5 directories)"), "{map}");
    assert!(map.contains("HANDOFF.md (10 B)"), "{map}");
    assert!(
        map.contains("build and run files: benchmark.sh (in challenge/)"),
        "{map}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn notes_list_newest_first_with_ages() {
    let root = workspace("notes");
    write(&root, "old/DEAD-ENDS.md", "x");
    write(&root, "NEXT.md", "y");
    let old = std::fs::File::options()
        .write(true)
        .open(root.join("old/DEAD-ENDS.md"))
        .unwrap();
    old.set_modified(SystemTime::now() - std::time::Duration::from_secs(3 * 3600))
        .unwrap();
    let walk = walk(&root);
    let notes = notes(&walk, now_secs());
    let lines: Vec<&str> = notes.lines().collect();
    assert_eq!(lines[0], "- NEXT.md (1 B, just now)", "{notes}");
    assert_eq!(lines[1], "- old/DEAD-ENDS.md (1 B, 3 h ago)", "{notes}");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn versions_read_from_the_usual_version_lines() {
    assert_eq!(
        version_token("rustc 1.95.0 (abc 2026-08-01)").as_deref(),
        Some("1.95.0")
    );
    assert_eq!(version_token("v24.1.0").as_deref(), Some("24.1.0"));
    assert_eq!(
        version_token("go version go1.24.1 linux/amd64").as_deref(),
        Some("1.24.1")
    );
    assert_eq!(
        version_token("gcc (GCC) 15.2.1 20250813").as_deref(),
        Some("15.2.1")
    );
    assert_eq!(version_token("no version here"), None);
}

#[test]
fn ages_read_in_plain_words() {
    let now = 1_000_000;
    assert_eq!(age(now, now - 5), "just now");
    assert_eq!(age(now, now - 12 * 60), "12 min ago");
    assert_eq!(age(now, now - 3 * 3600), "3 h ago");
    assert_eq!(age(now, now - 90_000), "1 day ago");
    assert_eq!(age(now, now - 3 * 86_400), "3 days ago");
}
