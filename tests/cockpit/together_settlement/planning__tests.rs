use super::{planning::*, tests::Temp, *};
use crate::stage::world_viz::crawl::{dungeon::Dungeon, party};
use std::collections::{BTreeSet, VecDeque};

fn cue(door: &str, passage: ResearchPassage, outcome: Outcome) -> Input {
    Input {
        map_sha256: crate::knowledge::cut::sha256_hex(door.as_bytes()),
        task_id: "q.task".into(),
        door_id: door.into(),
        route: vec!["q.task".into(), "m.check".into(), door.into()],
        passage,
        outcome,
    }
}

/// Independently flood the collision map; this does not call either the
/// planner's search or Dungeon::path, which shares its shortest-path engine.
fn connected(site: &Site) {
    let dungeon = Dungeon::of(&site.floor);
    let start = party::spot(&dungeon, 0);
    let mut reached = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some((x, y)) = queue.pop_front() {
        for next in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if dungeon.walkable(next) && reached.insert(next) {
                queue.push_back(next);
            }
        }
    }
    let cells: BTreeSet<_> = site.floor.rooms.iter().map(|room| room.cell).collect();
    assert_eq!(
        cells.len(),
        site.floor.rooms.len(),
        "room coordinates collide"
    );
    for (index, room) in site.floor.rooms.iter().enumerate() {
        assert!(
            reached.contains(&party::spot(&dungeon, index)),
            "room {index} disconnected"
        );
        assert!(room.roster.is_empty());
        for direction in 0..4 {
            if room.doors[direction] {
                let neighbour = site
                    .floor
                    .neighbour(index, direction)
                    .expect("door has room");
                assert_eq!(
                    site.floor.neighbour(neighbour, (direction + 2) % 4),
                    Some(index)
                );
            }
        }
    }
}

#[test]
fn settlement_planning_changes_real_rock_and_connected_buildout() {
    let mut topology = BTreeSet::new();
    let mut first_cuts = BTreeSet::new();
    for example in 0..12 {
        let passage = [
            ResearchPassage::Door,
            ResearchPassage::Dark,
            ResearchPassage::Review,
        ][example % 3];
        let outcome = [Outcome::Unreviewed, Outcome::Failed, Outcome::Refereed][example % 3];
        let input = cue(&format!("q.door-{example}"), passage, outcome);
        let mut site = Site::new(77, "Miner");
        for iteration in 1..=30 {
            site.apply_with_plan("A", iteration, true, Some(&input))
                .unwrap();
            connected(&site);
            assert!(site.valid(&Site::new(77, "Miner")));
            if iteration == 1 {
                first_cuts.insert(serde_json::to_string(&site.floor.rooms[4]).unwrap());
                assert_eq!(
                    site.floor.rooms[4].cell,
                    (1, 2),
                    "safe west ingress retained"
                );
            }
        }
        assert_eq!(site.built, 6);
        assert_eq!(site.work, 30);
        assert_eq!(site.floor.rooms.len(), 10);
        let home = site.floor.rooms[0].cell;
        for room in &site.floor.rooms[4..] {
            assert!((room.cell.0 - home.0).abs() <= 6);
            assert!((room.cell.1 - home.1).abs() <= 6);
            assert_ne!(room.cell, (1, 3), "tavern coordinate remains reserved");
        }
        let resources = &site.resources;
        assert_eq!(
            resources.stone + resources.spent_stone,
            resources.mined_stone
        );
        assert_eq!(resources.ore + resources.spent_ore, resources.mined_ore);
        topology.insert(
            site.floor
                .rooms
                .iter()
                .map(|room| room.cell)
                .collect::<Vec<_>>(),
        );
    }
    assert!(
        topology.len() >= 4,
        "research doors must choose different real room cells"
    );
    assert!(
        first_cuts.len() >= 4,
        "research routes must change which rock is cut first"
    );
}

#[test]
fn settlement_planning_preserves_legacy_rooms_and_freezes_partial_jobs() {
    let mut site = Site::new(77, "Miner");
    for iteration in 1..=7 {
        site.apply("A", iteration, true).unwrap();
    }
    let first = serde_json::to_value(&site.floor.rooms[4]).unwrap();
    let partial = site.floor.rooms[5].cell;
    let input = cue("q.new-door", ResearchPassage::Dark, Outcome::UnderReview);
    for iteration in 8..=10 {
        site.apply_with_plan("A", iteration, false, Some(&input))
            .unwrap();
        assert_eq!(site.floor.rooms[5].cell, partial);
        assert!(site.loops["A"].last().unwrap().plan.is_none());
    }
    assert_eq!(serde_json::to_value(&site.floor.rooms[4]).unwrap(), first);
    site.apply_with_plan("A", 11, true, Some(&input)).unwrap();
    let recorded = site.loops["A"].last().unwrap().plan.clone().unwrap();
    let before = serde_json::to_value(&site.floor.rooms[..6]).unwrap();
    let other = cue("q.changed-map", ResearchPassage::Review, Outcome::Failed);
    for iteration in 12..=15 {
        site.apply_with_plan("A", iteration, false, Some(&other))
            .unwrap();
        assert_eq!(
            site.loops["A"].last().unwrap().plan.as_ref(),
            Some(&recorded)
        );
    }
    // The connected parent gains a doorway on the first new cut only; later
    // phases and map changes do not regenerate the existing rooms.
    assert_eq!(
        serde_json::to_value(&site.floor.rooms[..6]).unwrap(),
        before
    );
    site.apply("A", 16, false).unwrap();
    assert_eq!(
        site.loops["A"].last().unwrap().plan.as_ref().unwrap().input,
        input
    );
    assert!(site.valid(&Site::new(77, "Miner")));
    connected(&site);
}

#[test]
fn settlement_planning_replays_many_loops_and_leaves_complete_legacy_sites_intact() {
    let mut site = Site::new(77, "Miner");
    for n in 1..=5 {
        site.apply("legacy", n, false).unwrap();
    }
    for n in 6..=30 {
        let loop_id = format!("loop-{}", n % 7);
        let input = cue(
            &format!("q.history-{n}"),
            ResearchPassage::Review,
            if n % 2 == 0 {
                Outcome::Failed
            } else {
                Outcome::Refereed
            },
        );
        site.apply_with_plan(&loop_id, n, n % 2 == 0, Some(&input))
            .unwrap();
        if n == 8 {
            assert!(layout::dig_tavern(&mut site.floor));
            site.tavern = true;
        }
        let snapshot = super::tests::value(&site);
        assert!(
            !site
                .apply_with_plan(&loop_id, n, true, Some(&input))
                .unwrap()
        );
        assert_eq!(super::tests::value(&site), snapshot);
        let resumed: Site = serde_json::from_value(snapshot.clone()).unwrap();
        assert!(resumed.valid(&Site::new(77, "Miner")));
        assert_eq!(super::tests::value(&resumed), snapshot);
        site = resumed;
        connected(&site);
    }
    assert_eq!(site.built, 6);
    assert!(site.tavern);
    let mut legacy = Site::new(77, "Miner");
    for n in 1..=30 {
        legacy.apply("old", n, false).unwrap();
    }
    let floor = serde_json::to_value(&legacy.floor).unwrap();
    let input = cue(
        "q.later-research",
        ResearchPassage::Door,
        Outcome::UnderReview,
    );
    for n in 1..=40 {
        legacy
            .apply_with_plan("new", n, true, Some(&input))
            .unwrap();
    }
    assert_eq!(serde_json::to_value(&legacy.floor).unwrap(), floor);
    assert!(
        legacy
            .loops
            .values()
            .flatten()
            .all(|receipt| receipt.plan.is_none())
    );
    assert!(legacy.valid(&Site::new(77, "Miner")));
}

#[test]
fn settlement_planning_replays_saved_receipts_and_rejects_tampered_plans() {
    let dir = Temp::new();
    let rewards = dir.path().join("island.json");
    let input = cue("q.replay", ResearchPassage::Door, Outcome::Refereed);
    let mut store = Store::beside(Some(&rewards), 77, "Miner").unwrap();
    let facts = super::tests::facts("A", &[1, 2, 3, 4, 5, 6]);
    let inputs = (1..=6).map(|n| (n, input.clone())).collect();
    store.sync_with_plans(&facts, true, &inputs).unwrap();
    connected(&store.site);
    let original = super::tests::value(&store.site);
    let resumed = Store::beside(Some(&rewards), 77, "Miner").unwrap();
    assert_eq!(super::tests::value(&resumed.site), original);
    let path = store.path.as_ref().unwrap();
    for mutation in ["target", "rock_route", "digest", "input", "missing"] {
        let mut broken = original.clone();
        let plan = &mut broken["loops"]["A"][0]["plan"];
        match mutation {
            "target" => plan["target"] = serde_json::json!([2, 2]),
            "rock_route" => plan["rock_route"] = serde_json::json!([[0, 0]]),
            "digest" => plan["digest"] = serde_json::json!("0".repeat(64)),
            "input" => plan["input"]["route"] = serde_json::json!(["q.other"]),
            "missing" => broken["loops"]["A"][0]
                .as_object_mut()
                .unwrap()
                .remove("plan")
                .map(|_| ())
                .unwrap(),
            _ => unreachable!(),
        }
        std::fs::write(path, serde_json::to_vec(&broken).unwrap()).unwrap();
        assert!(
            Store::beside(Some(&rewards), 77, "Miner").is_err(),
            "accepted tamper {mutation}"
        );
    }
    std::fs::write(path, serde_json::to_vec(&original).unwrap()).unwrap();
    assert!(Store::beside(Some(&rewards), 77, "Miner").is_ok());
}

#[test]
fn settlement_planning_cannot_mint_work_or_rewrite_duplicate_provenance() {
    let input = cue("q.real", ResearchPassage::Door, Outcome::Unreviewed);
    let other = cue("q.duplicate", ResearchPassage::Review, Outcome::Failed);
    let mut store = Store::beside(None, 77, "Miner").unwrap();
    let before = super::tests::value(&store.site);
    let mut unconfirmed = super::tests::facts("A", &[1]);
    unconfirmed.workspace = Some(std::env::temp_dir().join("different-project"));
    assert!(
        !store
            .sync_with_plan(&unconfirmed, false, Some(&input))
            .unwrap()
    );
    assert_eq!(super::tests::value(&store.site), before);
    store
        .sync_with_plan(&super::tests::facts("A", &[1]), false, Some(&input))
        .unwrap();
    let before = super::tests::value(&store.site);
    assert!(
        !store
            .sync_with_plan(&super::tests::facts("A", &[1]), false, Some(&other))
            .unwrap()
    );
    assert_eq!(super::tests::value(&store.site), before);
    assert!(
        !store
            .site
            .apply_with_plan("A", 1, true, Some(&other))
            .unwrap()
    );
    assert_eq!(super::tests::value(&store.site), before);
    let mut invalid = input;
    invalid.map_sha256.clear();
    let mut untouched = Site::new(77, "Miner");
    let before = super::tests::value(&untouched);
    assert!(
        untouched
            .apply_with_plan("A", 1, true, Some(&invalid))
            .is_err()
    );
    assert_eq!(super::tests::value(&untouched), before);
}

#[test]
fn settlement_planning_catchup_does_not_stamp_current_map_on_historical_work() {
    let input = cue("q.current", ResearchPassage::Door, Outcome::UnderReview);
    let mut store = Store::beside(None, 77, "Miner").unwrap();
    store
        .sync_with_plan(
            &super::tests::facts("A", &[1, 2, 3, 4, 5]),
            false,
            Some(&input),
        )
        .unwrap();
    assert!(
        store.site.loops["A"]
            .iter()
            .all(|receipt| receipt.plan.is_none())
    );
    assert_eq!(store.site.floor.rooms[4].cell, (1, 2));
    store
        .sync_with_plan(
            &super::tests::facts("A", &[1, 2, 3, 4, 5, 6]),
            false,
            Some(&input),
        )
        .unwrap();
    assert!(store.site.loops["A"][5].plan.is_some());
    assert_eq!(store.site.work, 6);
    assert!(store.site.valid(&Site::new(77, "Miner")));
}

#[test]
fn settlement_planning_checked_negative_route_changes_future_jobs_and_preserves_paid_history() {
    let dir = Temp::new();
    std::fs::create_dir(dir.path().join("labyrinth")).unwrap();
    let task = "test closed research perspective";
    let mut door = serde_json::json!({"id":"q.open","kind":"question","status":"open","links":[],
        "review":{"state":"refereed","by":["prior-review"],"verdict":"ESTABLISHED"}});
    let write = |door: &serde_json::Value| {
        std::fs::write(
            dir.path().join("labyrinth/knowledge.json"),
            serde_json::to_vec(&serde_json::json!({"schema":1,"nodes":[door]})).unwrap(),
        )
        .unwrap();
    };
    write(&door);
    let original = input_for_door(dir.path(), task, "q.open").unwrap().unwrap();
    assert_eq!(original.outcome, Outcome::Refereed);
    let mut site = Site::new(77, "Miner");
    for iteration in 1..=5 {
        site.apply_with_plan("A", iteration, true, Some(&original))
            .unwrap();
    }
    let paid = super::tests::value(&site);
    let room = (site.floor.rooms[4].cell, site.floor.rooms[4].kind);
    let receipts = serde_json::to_value(&site.loops["A"]).unwrap();
    assert_eq!(site.floor.rooms[4].kind, RoomKind::Quarters);
    let digest = |file: &str| serde_json::json!({"path":format!("labyrinth/angel/campaigns/test/agents/{file}"),"sha256":"a".repeat(64)});
    door["failed_routes"] = serde_json::json!([{"claim_id":"claim-zero","verdict":"GAP",
        "route":"try the universal domain","lesson":"zero remains unchecked","next_test":"independently check zero",
        "author_report":digest("attack/report.md"),"referee_report":digest("referee/report.md"),
        "check":digest("referee/checks/referee-0.json")}]);
    write(&door);
    let failed = input_for_door(dir.path(), task, "q.open").unwrap().unwrap();
    assert_eq!(
        failed.passage,
        ResearchPassage::Door,
        "one failed perspective must not close the question"
    );
    assert_eq!(failed.outcome, Outcome::Failed);
    assert_ne!(failed.map_sha256, original.map_sha256);
    assert_eq!(
        super::tests::value(&site),
        paid,
        "reading a changed graph buys no work"
    );
    assert!(!site.apply_with_plan("A", 5, true, Some(&failed)).unwrap());
    assert_eq!(
        super::tests::value(&site),
        paid,
        "a replacement map cannot alter a paid receipt"
    );
    for iteration in 6..=9 {
        site.apply_with_plan("A", iteration, true, Some(&failed))
            .unwrap();
    }
    assert_eq!((site.floor.rooms[4].cell, site.floor.rooms[4].kind), room);
    assert_eq!(
        serde_json::to_value(&site.loops["A"][..5]).unwrap(),
        receipts
    );
    assert_eq!(site.floor.rooms[5].kind, RoomKind::Workshop);
    assert!(
        site.loops["A"][..5]
            .iter()
            .all(|receipt| receipt.plan.as_ref().unwrap().input == original)
    );
    assert!(
        site.loops["A"][5..]
            .iter()
            .all(|receipt| receipt.plan.as_ref().unwrap().input == failed)
    );
    assert_eq!(site.work, 9);
    connected(&site);
}

#[test]
fn settlement_planning_uses_real_frontier_routes_and_independent_review_states() {
    let dir = Temp::new();
    let task = "test the stone hypothesis";
    let task_id = format!(
        "q.angel-{}",
        &crate::knowledge::cut::sha256_hex(task.as_bytes())[..24]
    );
    std::fs::create_dir(dir.path().join("labyrinth")).unwrap();
    let write_map = |outcome: &str| {
        let review = match outcome {
            "refereed" => {
                serde_json::json!({"state":"refereed","by":["independent checker"],"verdict":"ESTABLISHED"})
            }
            _ => serde_json::json!({"state":"under-review"}),
        };
        let mut door = serde_json::json!({"id":"q.stone","kind":"question","status":"open","review":review,
            "links":[{"to":"m.test","rel":"uses"}]});
        if outcome == "failed" {
            door["angel_measurement"] = serde_json::json!({"passed":false});
        }
        if outcome == "board-failed" {
            door["angel_submission"] =
                serde_json::json!({"status":"failed","official_metrics":{"verified":false}});
        }
        let nodes = serde_json::json!([
            {"id":task_id,"kind":"question","status":"open","links":[{"to":"m.test","rel":"suggests"}]},
            {"id":"m.test","kind":"method","links":[]}, door,
            {"id":"dead.stone","kind":"deadend","lesson":"contradiction","status":"refuted","evidence":["counterexample"],"links":[]}
        ]);
        std::fs::write(
            dir.path().join("labyrinth/knowledge.json"),
            serde_json::to_vec(&serde_json::json!({"schema":1,"nodes":nodes})).unwrap(),
        )
        .unwrap();
    };
    write_map("under-review");
    let initial = input_for_door(dir.path(), task, "q.stone")
        .unwrap()
        .unwrap();
    assert_eq!(
        initial.route,
        vec![task_id.clone(), "m.test".into(), "q.stone".into()]
    );
    assert_eq!(initial.outcome, Outcome::UnderReview);
    assert!(input_for_door(dir.path(), task, "dead.stone").is_err());
    assert_eq!(input(dir.path(), task).unwrap().unwrap(), initial);
    write_map("failed");
    let failed = input_for_door(dir.path(), task, "q.stone")
        .unwrap()
        .unwrap();
    assert_eq!(failed.outcome, Outcome::Failed);
    assert_ne!(failed.map_sha256, initial.map_sha256);
    write_map("board-failed");
    assert_eq!(
        input_for_door(dir.path(), task, "q.stone")
            .unwrap()
            .unwrap()
            .outcome,
        Outcome::Failed
    );
    write_map("refereed");
    let refereed = input_for_door(dir.path(), task, "q.stone")
        .unwrap()
        .unwrap();
    assert_eq!(refereed.outcome, Outcome::Refereed);
    let mut failed_site = Site::new(77, "Miner");
    let mut reviewed_site = Site::new(77, "Miner");
    for n in 1..=4 {
        failed_site
            .apply_with_plan("A", n, true, Some(&failed))
            .unwrap();
        reviewed_site
            .apply_with_plan("A", n, true, Some(&refereed))
            .unwrap();
    }
    assert_eq!(failed_site.floor.rooms[4].kind, RoomKind::Workshop);
    assert_eq!(reviewed_site.floor.rooms[4].kind, RoomKind::Quarters);
    assert_eq!(failed_site.work, reviewed_site.work);
    assert_eq!(
        failed_site.resources, reviewed_site.resources,
        "review is not a resource multiplier"
    );
}
