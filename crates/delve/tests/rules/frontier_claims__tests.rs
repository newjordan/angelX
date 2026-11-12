use super::*;

#[test]
fn frontier_claims_full_ledger_notice_is_once_per_seam_and_room_entry() {
    let (mut run, d) = claim_bosses::tests::fixture(claim_bosses::Guardian::Ironroot);
    run.home.industry.claims.clear();
    for i in 0..MAX_CLAIMS {
        assert!(run.home.industry.discover(
            catalog(),
            format!("old-{i}"),
            Claim {
                resource: "coal".into(),
                name: "Old seam".into(),
                source: "Crypt".into(),
                secured: false,
                rate: 2,
            }
        ));
        assert!(run.home.industry.secure(&format!("old-{i}")));
    }
    run.notices.clear();
    for _ in 0..100 {
        assert!(!run.discover_frontier_claim(&d));
    }
    assert_eq!(run.notices.len(), 1);
    assert!(run.notices[0].contains("ledger full") && run.notices[0].contains("Claims"));
    let room = run.at;
    run.at = 0;
    run.reset_claim_capacity_notice();
    run.at = room;
    assert!(!run.discover_frontier_claim(&d));
    assert_eq!(run.notices.len(), 2);
    // A checkpoint discards only transient notice suppression, never claims.
    let mut run: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    let n = run.notices.len();
    assert!(!run.discover_frontier_claim(&d));
    assert_eq!(run.notices.len(), n + 1);
    run.home
        .industry
        .apply(catalog(), Command::ReleaseClaim("old-0".into()))
        .unwrap();
    assert!(run.discover_frontier_claim(&d));
    assert_eq!(run.home.industry.claims.len(), MAX_CLAIMS);
    assert!(!run.home.industry.claims[&d.id].secured);
}

#[test]
fn frontier_claims_live_and_parked_encounters_cannot_be_released() {
    let (mut run, d) = claim_bosses::tests::start(claim_bosses::Guardian::Ironroot);
    let before = serde_json::to_string(&run).unwrap();
    assert!(
        run.release_frontier_claim(&d.id)
            .unwrap_err()
            .contains("encounter")
    );
    assert_eq!(serde_json::to_string(&run).unwrap(), before);
    run.parked.append(&mut run.enemies);
    assert!(
        run.release_frontier_claim(&d.id)
            .unwrap_err()
            .contains("encounter")
    );
    assert!(run.home.industry.claims.contains_key(&d.id));
    let (mut run, d) = ore_threats::tests::fixture(ore_threats::OreKind::Coal);
    assert!(
        run.release_frontier_claim(&d.id)
            .unwrap_err()
            .contains("encounter")
    );
    run.parked.append(&mut run.enemies);
    assert!(
        run.release_frontier_claim(&d.id)
            .unwrap_err()
            .contains("encounter")
    );
}

#[test]
fn frontier_claims_retirement_keeps_death_ledgers_and_never_repeats_loot() {
    let (mut run, d) = claim_bosses::tests::start(claim_bosses::Guardian::Ironroot);
    run.enemies[0].hp = 0;
    run.settle_kills();
    assert!(run.home.industry.secure(&d.id));
    let depth = run.dungeon.depth;
    assert!(
        run.release_frontier_claim(&d.id)
            .unwrap_err()
            .contains("kingdom")
    );
    run.dungeon.depth = 0;
    let stock = run.home.industry.stock.clone();
    let items = run.room().items.len();
    let marks = run.marks.clone();
    run.release_frontier_claim(&d.id).unwrap();
    assert!(!run.home.industry.claims.contains_key(&d.id));
    assert!(run.claim_guardian_defeated(&d.id));
    assert!(run.ore_sentries_defeated(&d.id));
    assert!(run.home.industry.has_secured_resource("iron-ore"));
    assert!(run.home.industry.valid(catalog()) && run.kingdom.valid());
    assert_eq!(run.marks, marks, "retirement emits no new reward marks");
    let mut run: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    // Marks are a transient delivery queue, deliberately omitted by a Run
    // checkpoint. Rediscovery must not mint fresh events after restoration.
    let marks = run.marks.clone();
    run.dungeon.depth = depth;
    assert!(run.discover_frontier_claim(&d));
    run.tick_ore_sentries(&d);
    run.tick_claim_guardian(
        &d,
        &BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]),
    );
    assert!(run.enemies.is_empty());
    assert!(run.home.industry.secure(&d.id));
    assert_eq!(run.home.industry.stock, stock);
    assert_eq!(run.room().items.len(), items);
    assert_eq!(run.marks, marks);
    run.home.industry.survey_goal = Some((2, 0));
    run.home
        .industry
        .apply(catalog(), Command::ReleaseClaim(d.id))
        .unwrap();
    assert!(run.frontier_objective().unwrap().status.claim_won());
}
