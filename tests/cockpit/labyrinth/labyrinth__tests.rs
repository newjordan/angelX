use super::*;
use std::collections::VecDeque;

fn workspace() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "angel-labyrinth-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn proof(id: &str, tier: &str, review: &str) -> Value {
    json!({"id":id,"kind":"theorem","tier":tier,"title":id,"status":"established",
        "evidence":["independent-check.txt"],"review":{"state":review,"by":["independent-referee"],"verdict":"PROVED"},"links":[]})
}

#[test]
fn tier_alone_does_not_open_an_established_corridor() {
    for tier in ["T2", "T3"] {
        let mut node = proof("th.result", tier, "under-review");
        assert_eq!(passage(&node), Passage::Review);
        node["review"]["state"] = json!("refereed");
        assert_eq!(passage(&node), Passage::Charted);
        node["review"]["verdict"] = json!("GAP");
        assert_eq!(passage(&node), Passage::Review);
        node["review"]["verdict"] = json!("PROVED");
        node["evidence"] = json!([]);
        assert_eq!(passage(&node), Passage::Review);
    }
    let mut measured = proof("ev.run", "T4", "refereed");
    measured["kind"] = json!("evidence");
    assert_eq!(passage(&measured), Passage::Review);
}

#[test]
fn routes_do_not_cross_refutations_and_label_unknown_steps() {
    let mut a = proof("th.start", "T1", "refereed");
    a["links"] = json!([{"to":"x.wall","rel":"suggests"},{"to":"q.door","rel":"suggests"}]);
    let wall = json!({"id":"x.wall","kind":"deadend","status":"refuted","lesson":"counterexample", "evidence":["witness"],
        "links":[{"to":"th.goal","rel":"suggests"}]});
    let door = json!({"id":"q.door","kind":"question","status":"open",
        "links":[{"to":"th.goal","rel":"suggests"}]});
    let map =
        Map::parse(json!({"schema":1,"nodes":[a,wall,door,proof("th.goal","T1","refereed")]}))
            .unwrap();
    let path = map.route("th.start", "th.goal", true).unwrap();
    let ids: Vec<_> = path["path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["th.start", "q.door", "th.goal"]);
    assert_eq!(path["path"][1]["passage"], "door");
    assert!(map.route("th.start", "th.goal", false).unwrap()["path"].is_null());
    assert!(map.route("x.wall", "x.wall", true).unwrap()["path"].is_null());
}

#[test]
fn schema_rejects_duplicate_nodes_dangling_links_and_dependency_cycles() {
    let node = json!({"id":"q.a","kind":"question","links":[]});
    assert!(Map::parse(json!({"nodes":[node.clone(),node.clone()]})).is_err());
    let mut a = node;
    a["links"] = json!([{"to":"q.b","rel":"uses"}]);
    assert!(Map::parse(json!({"nodes":[a.clone()]})).is_err());
    let b = json!({"id":"q.b","kind":"question","links":[{"to":"q.a","rel":"uses"}]});
    assert!(
        Map::parse(json!({"nodes":[a,b]}))
            .err()
            .unwrap()
            .contains("cyclic")
    );
}

#[test]
fn shared_router_matches_an_independent_breadth_first_oracle() {
    for seed in 0..64u64 {
        let open =
            |i: usize| i == 0 || i == 63 || ((seed.wrapping_mul(37) + i as u64 * 19) % 7 != 0);
        let adjacent = |i: usize| {
            let (x, y) = (i % 8, i / 8);
            [
                (x.checked_sub(1), Some(y)),
                (Some(x + 1), Some(y)),
                (Some(x), y.checked_sub(1)),
                (Some(x), Some(y + 1)),
            ]
            .into_iter()
            .filter_map(|(x, y)| x.zip(y))
            .filter(|&(x, y)| x < 8 && y < 8)
            .map(|(x, y)| y * 8 + x)
            .filter(|&i| open(i))
            .collect::<Vec<_>>()
        };
        let mut distance = [usize::MAX; 64];
        distance[0] = 0;
        let mut queue = VecDeque::from([0]);
        while let Some(at) = queue.pop_front() {
            for next in adjacent(at) {
                if distance[next] == usize::MAX {
                    distance[next] = distance[at] + 1;
                    queue.push_back(next);
                }
            }
        }
        let route = routing::shortest_path(64, 0, 63, |i| {
            adjacent(i).into_iter().map(|i| (i, 1)).collect()
        });
        assert_eq!(
            route.as_ref().map(|path| path.len() - 1),
            (distance[63] != usize::MAX).then_some(distance[63])
        );
    }
    assert_eq!(
        routing::shortest_path(4, 0, 3, |i| match i {
            0 => vec![(1, 20), (2, 1)],
            2 => vec![(1, 1)],
            1 => vec![(3, 1)],
            _ => vec![],
        }),
        Some(vec![0, 2, 1, 3])
    );
    assert!(routing::shortest_path(262_145, 0, 1, |_| vec![]).is_none());
}

#[test]
fn every_tunnel_variant_is_connected_bounded_and_reproducible() {
    for variant in [
        routing::TunnelVariant::Gallery,
        routing::TunnelVariant::Warrens,
        routing::TunnelVariant::Karst,
    ] {
        for seed in 0..80 {
            for count in [1, 9, 14, 25, usize::MAX] {
                let cells = routing::tunnel_cells(variant, seed, 5, count);
                assert_eq!(cells.len(), count.min(25));
                assert_eq!(cells, routing::tunnel_cells(variant, seed, 5, count));
                let unique: BTreeSet<_> = cells.iter().copied().collect();
                assert_eq!(unique.len(), cells.len());
                for (i, &(x, y)) in cells.iter().enumerate() {
                    assert!((0..5).contains(&x) && (0..5).contains(&y));
                    if i > 0 {
                        assert!(
                            cells[..i]
                                .iter()
                                .any(|&(a, b)| (a - x).abs() + (b - y).abs() == 1)
                        );
                    }
                }
            }
        }
    }
    assert_ne!(
        routing::tunnel_cells(routing::TunnelVariant::Gallery, 9, 5, 14),
        routing::tunnel_cells(routing::TunnelVariant::Karst, 9, 5, 14)
    );
}

#[test]
fn observations_preserve_curated_knowledge_are_idempotent_and_workspace_scoped() {
    let root = workspace();
    let other = workspace();
    observe_iteration(&root, "deli", "task", &["reasoned premise".into()], &[]).unwrap();
    assert!(!root.join("labyrinth").exists());
    initialize(&root).unwrap();
    let curated = std::fs::read(root.join("labyrinth/knowledge.json")).unwrap();
    for _ in 0..2 {
        observe_iteration(
            &root,
            "deli",
            "task",
            &["reasoned premise".into()],
            &["maybe a shortcut".into()],
        )
        .unwrap();
    }
    let map = load(&root).unwrap().unwrap();
    assert_eq!(map.nodes.len(), 3);
    assert_eq!(
        std::fs::read_dir(root.join("labyrinth/angel/observations"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        std::fs::read(root.join("labyrinth/knowledge.json")).unwrap(),
        curated
    );
    assert!(
        map.nodes
            .iter()
            .any(|n| n["kind"] == "conjecture" && n["tier"] == "T5")
    );
    assert!(
        map.nodes
            .iter()
            .any(|n| n["kind"] == "hunch" && n["tier"] == "T6")
    );
    assert!(load(&other).unwrap().is_none());
    assert!(context(&root, "task").contains("reasoned premise"));
    assert!(context(&root, "different task").is_empty());
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(other).unwrap();
}

#[test]
fn automatic_observations_do_not_count_as_candidate_source_changes() {
    let root = workspace();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    initialize(&root).unwrap();
    let before = crate::agent::harness::workspace_fingerprint(&root).expect("usable Git workspace");
    observe_iteration(&root, "deli", "task", &["reasoned lead".into()], &[]).unwrap();
    let after = crate::agent::harness::workspace_fingerprint(&root).unwrap();
    assert_eq!(
        before, after,
        "automatic map writes are not candidate progress"
    );
    std::fs::write(root.join("candidate.rs"), "pub fn solution() {}\n").unwrap();
    assert_ne!(
        after,
        crate::agent::harness::workspace_fingerprint(&root).unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn transitive_unproved_prerequisites_keep_a_route_unestablished() {
    let mut result = proof("th.result", "T2", "refereed");
    result["links"] = json!([{"to":"m.conditional","rel":"uses"}]);
    let method =
        json!({"id":"m.conditional","kind":"method","links":[{"to":"h.assumption","rel":"uses"}]});
    let assumption =
        json!({"id":"h.assumption","kind":"hunch","tier":"T6","status":"live","links":[]});
    let map = Map::parse(json!({"nodes":[result,method,assumption]})).unwrap();
    assert!(map.route("th.result", "m.conditional", false).unwrap()["path"].is_null());
    assert_eq!(
        map.route("th.result", "m.conditional", true).unwrap()["path"][0]["passage"],
        "under-review"
    );
}

#[test]
fn concurrent_observations_merge_without_lost_doors() {
    let root = workspace();
    initialize(&root).unwrap();
    let workers: Vec<_> = (0..8)
        .map(|i| {
            let root = root.clone();
            std::thread::spawn(move || {
                observe_iteration(&root, "deli", "shared task", &[format!("lead {i}")], &[])
                    .unwrap()
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let map = load(&root).unwrap().unwrap();
    assert_eq!(map.nodes.len(), 9);
    let question = map.nodes.iter().find(|n| n["kind"] == "question").unwrap();
    assert_eq!(question["links"].as_array().unwrap().len(), 8);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn measured_failures_remain_t4_and_never_become_refutations() {
    let root = workspace();
    initialize(&root).unwrap();
    let observation = json!({"id":"run-1","task":"task","idea":"try a shortcut","passed":false,"paired_delta":-0.3,
        "source_sha256":"a".repeat(64),"receipt_sha256":"b".repeat(64),"evidence_sha256":"c".repeat(64)});
    observe_measurement(&root, &observation, &json!({"model":"fixture"})).unwrap();
    let map = load(&root).unwrap().unwrap();
    let node = map.nodes.iter().find(|n| n["kind"] == "evidence").unwrap();
    assert_eq!(node["tier"], "T4");
    assert_eq!(passage(node), Passage::Review);
    assert_eq!(node["angel_measurement"]["paired_delta"], -0.3);
    assert_eq!(node["angel_route"]["model"], "fixture");
    let mut invalid = observation;
    invalid["receipt_sha256"] = json!("invented");
    assert!(observe_measurement(&root, &invalid, &json!({})).is_err());
    let output = std::process::Command::new("python3")
        .arg(root.join("labyrinth/lab.py"))
        .arg("check")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new("python3")
        .arg(root.join("labyrinth/lab.py"))
        .arg("build")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let data: Value =
        serde_json::from_slice(&std::fs::read(root.join("labyrinth/dashboard/data.json")).unwrap())
            .unwrap();
    assert_eq!(data["nodes"].as_array().unwrap().len(), map.nodes.len());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_runner_requires_fresh_scores_and_unchanged_candidate_bytes() {
    use std::ffi::OsString;
    let root = workspace();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(root.join(".gitignore"), "score.json\n").unwrap();
    std::fs::write(root.join("candidate.txt"), "baseline\n").unwrap();
    let run = |idea: &str, script: &str| {
        cli::run(
            &root,
            &[
                "--task",
                "benchmark fixture",
                "--idea",
                idea,
                "--score-file",
                "score.json",
                "--",
                "python3",
                "-c",
                script,
            ]
            .map(OsString::from),
        )
        .unwrap()
    };
    let good = run(
        "fresh score",
        "from pathlib import Path; Path('score.json').write_text('{\"score\":42}')",
    );
    assert_eq!(good["passed"], true);
    assert_eq!(good["benchmark_receipt"]["report"]["score"], 42);
    let stale = run("stale score", "pass");
    assert_eq!(stale["passed"], false);
    assert_eq!(stale["benchmark_receipt"]["fresh_score"], false);
    let changed = run(
        "changed candidate",
        "from pathlib import Path; Path('score.json').write_text('{\"score\":7}'); Path('candidate.txt').write_text('changed')",
    );
    assert_eq!(changed["passed"], false);
    assert_eq!(changed["benchmark_receipt"]["fresh_score"], true);
    assert_eq!(changed["benchmark_receipt"]["source_stable"], false);
    let rejected = run(
        "child rejected candidate",
        "from pathlib import Path; Path('score.json').write_text('{\"score\":1}'); raise SystemExit(3)",
    );
    assert_eq!(rejected["passed"], false);
    assert_eq!(rejected["benchmark_receipt"]["exit_code"], 3);
    let map = load(&root).unwrap().unwrap();
    assert_eq!(
        map.plan(Some("benchmark fixture"), 6)["recorded_attempts"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    for node in map
        .nodes
        .iter()
        .filter(|node| node.get("angel_measurement").is_some())
    {
        assert_eq!(node["tier"], "T4");
        assert_eq!(passage(node), Passage::Review);
    }
    assert!(
        map.plan(Some("unrelated task"), 6)["recorded_attempts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn terminal_submission_receipts_preserve_verification_without_inventing_promotion() {
    use crate::agent::harness::{SubmissionReceipt, WatchNotify};
    let root = workspace();
    initialize(&root).unwrap();
    let official = crate::agent::harness::cartridges::records::SubmissionStatus {
        id: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".into(),
        benchmark_id: "a9214626-38ee-4912-97d7-10fc0d4b7861".into(),
        status: "validating".into(),
        promotion_status: Some("not_promoted".into()),
        promoted_source_ref: None,
        submission_commit_sha: Some("a".repeat(40)),
        official_score: Some("39181312".into()),
        rejection_reason: Some("did not improve".into()),
        official_metrics: json!({"verified":true,"signatureBytes":5312,"verificationCycles":7376}),
        improved: Some(false),
        frontier: None,
        fetched_at_ms: 1,
    };
    let mut notify = WatchNotify {
        id: official.id.clone(),
        status: official.status.clone(),
        score: official.official_score.clone(),
        rejection_reason: official.rejection_reason.clone(),
        source_note: None,
        receipt: Some(SubmissionReceipt {
            benchmark_id: official.benchmark_id.clone(),
            promotion_status: official.promotion_status.clone(),
            promoted_source_ref: None,
            submission_commit_id: official.submission_commit_sha.clone(),
            observed_at_ms: Some(1),
            source_id: "official-status-api".into(),
            official: Some(official),
        }),
    };
    observe_submission(&root, &notify).unwrap();
    assert!(
        load(&root).unwrap().unwrap().nodes.is_empty(),
        "pending is not terminal evidence"
    );
    notify.status = "rejected".into();
    notify
        .receipt
        .as_mut()
        .unwrap()
        .official
        .as_mut()
        .unwrap()
        .status = notify.status.clone();
    // Exercise the real watcher delivery hook, including repeated polls.
    crate::agent::harness::book::k_competition::watcher_turn(&root, &notify);
    notify
        .receipt
        .as_mut()
        .unwrap()
        .official
        .as_mut()
        .unwrap()
        .fetched_at_ms = 2;
    crate::agent::harness::book::k_competition::watcher_turn(&root, &notify);
    let map = load(&root).unwrap().unwrap();
    assert_eq!(map.nodes.len(), 2);
    assert_eq!(
        std::fs::read_dir(root.join("labyrinth/angel/observations"))
            .unwrap()
            .count(),
        1
    );
    let node = map
        .nodes
        .iter()
        .find(|node| node.get("angel_submission").is_some())
        .unwrap();
    assert_eq!(passage(node), Passage::Review);
    assert_eq!(node["angel_submission"]["status"], "rejected");
    assert_eq!(
        node["angel_submission"]["official_metrics"]["verified"],
        true
    );
    assert_eq!(node["angel_submission"]["improved"], false);
    assert!(node["angel_submission"]["promoted_source_ref"].is_null());
    for status in ["TIMEOUT", "timed_out", "timed-out", "canceled"] {
        notify
            .receipt
            .as_mut()
            .unwrap()
            .official
            .as_mut()
            .unwrap()
            .status = status.into();
        observe_submission(&root, &notify).unwrap();
    }
    assert_eq!(load(&root).unwrap().unwrap().nodes.len(), 6);
    notify.id = "different-submission".into();
    assert!(observe_submission(&root, &notify).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_library_projection_is_bounded_linked_and_cleared_on_workspace_switch() {
    use crate::drive::research_workspace::{Place, State};
    let root = workspace();
    let other = workspace();
    initialize(&root).unwrap();
    let mut proof = proof("th.claim", "T3", "refereed");
    proof["links"] = json!([{"to":"q.test","rel":"suggests"}]);
    let mut nodes = vec![proof, json!({"id":"q.test","kind":"question","links":[]})];
    nodes.extend(
        (0..160).map(|i| json!({"id":format!("zz.extra-{i}"),"kind":"question","links":[]})),
    );
    std::fs::write(
        root.join("labyrinth/knowledge.json"),
        json!({"schema":1,"nodes":nodes}).to_string(),
    )
    .unwrap();
    let mut projection = Projection::default();
    let rows = projection.refresh(&root);
    assert_eq!(rows.len(), 129);
    assert!(
        rows.iter()
            .all(|row| row.place == Place::Library && row.state == State::Recorded)
    );
    let claim = rows
        .iter()
        .find(|row| row.id == "labyrinth:th.claim")
        .unwrap();
    assert_eq!(
        claim.links,
        [("labyrinth:q.test".into(), "suggests".into())]
    );
    assert!(rows[0].summary.contains("162 nodes"));
    assert!(projection.refresh(&other).is_empty());
    assert_eq!(projection.refresh(&root).len(), 129);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(other).unwrap();
}

#[test]
fn fresh_context_bounds_referee_provenance_without_truncating_stored_knowledge() {
    let root = workspace();
    initialize(&root).unwrap();
    let mut node = proof("th.review", "T2", "refereed");
    node["review"]["by"] = json!(vec!["referee".repeat(2000); 80]);
    node["review"]["verdict"] = json!("GAP".repeat(5000));
    node["review"]["notes"] = json!("large authored provenance".repeat(3000));
    std::fs::write(
        root.join("labyrinth/knowledge.json"),
        json!({"schema":1,"nodes":[node.clone()]}).to_string(),
    )
    .unwrap();
    let cue = context(&root, "task");
    assert!(
        cue.len() < 4000,
        "fresh context must not expand to the full research file"
    );
    assert!(cue.contains("referee"));
    assert!(!cue.contains("large authored provenance"));
    assert_eq!(
        load(&root).unwrap().unwrap().nodes[0]["review"],
        node["review"]
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn map_reads_and_publications_refuse_symlink_redirection() {
    let root = workspace();
    let other = workspace();
    std::os::unix::fs::symlink(&other, root.join("labyrinth")).unwrap();
    assert!(initialize(&root).is_err());
    assert!(std::fs::read_dir(&other).unwrap().next().is_none());
    std::fs::remove_file(root.join("labyrinth")).unwrap();
    initialize(&root).unwrap();
    std::fs::create_dir_all(root.join("labyrinth/angel")).unwrap();
    std::os::unix::fs::symlink(&other, root.join("labyrinth/angel/observations")).unwrap();
    assert!(
        observe_iteration(&root, "loop", "task", &["file-backed evidence".into()], &[]).is_err()
    );
    assert!(std::fs::read_dir(&other).unwrap().next().is_none());
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(other).unwrap();
}
