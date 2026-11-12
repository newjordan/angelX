use super::*;

#[test]
fn ore_threats_crowded_fresh_seams_wait_for_the_whole_roster_before_guardians() {
    let (mut run, d) = claim_bosses::tests::fixture(claim_bosses::Guardian::Ironroot);
    for _ in 0..boss_ecology::ENTITY_CAP - 1 {
        run.spawn_at_for_test(EnemyKind::Skeleton, 1.0, 1.0);
    }
    let hands = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    run.tick_ore_sentries(&d);
    assert!(!run.kingdom.ore_sentries.contains_key(&d.id));
    assert!(!run.ore_sentries_defeated(&d.id));
    run.tick_claim_guardian(&d, &hands);
    assert!(!run.kingdom.claim_bosses.contains_key(&d.id));
    assert_eq!(run.enemies.len(), boss_ecology::ENTITY_CAP - 1);
    run.enemies.clear();
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), 2);
    run.tick_claim_guardian(&d, &hands);
    assert!(!run.kingdom.claim_bosses.contains_key(&d.id));
    for e in &mut run.enemies { e.hp = 0; }
    run.settle_kills();
    run.tick_claim_guardian(&d, &hands);
    assert!(run.kingdom.claim_bosses.contains_key(&d.id));
    assert_eq!(run.enemies.len(), 1);
}

#[test]
fn ore_threats_legacy_challenged_and_completed_guardians_do_not_gain_new_sentries() {
    let (mut run, d) = claim_bosses::tests::start(claim_bosses::Guardian::Ironroot);
    run.kingdom.ore_sentries.remove(&d.id);
    let mut run: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(run.ore_sentries_defeated(&d.id));
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), 1);
    assert!(run.admitted_claim_guardian(&run.enemies[0]));
    run.enemies[0].hp = 0;
    run.settle_kills();
    assert!(run.claim_guardian_defeated(&d.id));
    run.tick_ore_sentries(&d);
    assert!(run.enemies.is_empty());
    assert!(run.home.industry.secure(&d.id));
    assert!(run.home.industry.claims[&d.id].secured);
}

pub(crate) fn fixture(kind: OreKind) -> (Run, industry::Deposit) {
    let guardian = match kind {
        OreKind::Iron => claim_bosses::Guardian::Ironroot,
        OreKind::Crystal => claim_bosses::Guardian::Amethyst,
        _ => claim_bosses::Guardian::Cindermaw,
    };
    let (mut run, mut d) = claim_bosses::tests::fixture(guardian);
    if kind == OreKind::Ember {
        let mut claim = run.home.industry.claims.remove(&d.id).unwrap();
        run.dungeon.pack = Pack::Hellforge;
        d = run.industry_deposits().into_iter().next().unwrap();
        claim.resource = d.resource.into();
        run.home
            .industry
            .discover(crate::kingdom::catalog(), d.id.clone(), claim);
    }
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), 2);
    (run, d)
}
#[test]
fn ore_threats_have_distinct_real_mechanics_and_visible_forewarnings() {
    let mut kinds = Vec::new();
    for kind in OreKind::ALL {
        let (run, _) = fixture(kind);
        assert!(
            run.enemies
                .iter()
                .all(|e| e.kind == kind.kind() && run.admitted_ore_sentry(e))
        );
        assert!(
            run.notices
                .iter()
                .any(|s| s.contains(kind.name()) && s.contains(kind.warning()))
        );
        assert_eq!(OreKind::for_resource(kind.resource()), Some(kind));
        kinds.push(kind.kind());
    }
    for i in 0..kinds.len() {
        assert!(!kinds[..i].contains(&kinds[i]));
    }
}
#[test]
fn ore_threats_guard_the_claim_and_revisits_keep_damage_and_finite_rewards() {
    let (mut run, d) = fixture(OreKind::Iron);
    run.enemies[0].hp -= 17;
    let hp = run.enemies[0].hp;
    let first = run.enemies[0].id;
    let hands = BTreeMap::from([(
        1,
        Input {
            interact: true,
            take: true,
            ..Default::default()
        },
    )]);
    run.tick_claim_guardian(&d, &hands);
    assert!(run.kingdom.claim_bosses.is_empty());
    let mut run: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), 2);
    assert_eq!(run.enemies.iter().find(|e| e.id == first).unwrap().hp, hp);
    let mut repeated = run.enemies[0].clone();
    repeated.hp = 0;
    run.enemies[0].hp = 0;
    run.enemies.push(repeated.clone());
    run.settle_kills();
    assert!(!run.ore_sentries_defeated(&d.id));
    let marks = run.marks.clone();
    let items = run.room().items.len();
    run.enemies.push(repeated);
    run.settle_kills();
    assert_eq!(run.marks, marks);
    assert_eq!(run.room().items.len(), items);
    run.enemies[0].hp = 0;
    run.settle_kills();
    assert!(run.ore_sentries_defeated(&d.id));
    run.tick_ore_sentries(&d);
    assert!(run.enemies.is_empty());
    run.tick_claim_guardian(&d, &hands);
    assert_eq!(run.enemies.len(), 1);
    assert_eq!(run.home.industry.amount("iron-ore"), 0);
}
#[test]
fn ore_threats_missing_bodies_are_not_defeated_and_spawns_obey_caps() {
    let (mut run, d) = fixture(OreKind::Coal);
    let old = run.enemies[0].id;
    run.enemies.remove(0);
    run.settle_kills();
    assert!(!run.ore_sentries_defeated(&d.id));
    run.tick_ore_sentries(&d);
    assert_eq!(run.enemies.len(), 2);
    assert!(!run.enemies.iter().any(|e| e.id == old));
    run.enemies.clear();
    run.next_id = u32::MAX;
    let ledger = serde_json::to_string(&run.kingdom.ore_sentries).unwrap();
    run.tick_ore_sentries(&d);
    assert!(run.enemies.is_empty());
    assert_eq!(
        serde_json::to_string(&run.kingdom.ore_sentries).unwrap(),
        ledger
    );
}
#[test]
fn ore_threats_are_admitted_beside_authentic_floor_camps() {
    let mut run = boss_gate_testing::on_floor(1, 511, fortune::Mode::LongWayDown);
    let room = (1..run.dungeon.rooms.len())
        .find(|&i| i % 3 == 1 && run.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    run.enter(room, None);
    let native_count = run.enemies.len();
    let d = run.industry_deposits().into_iter().next().unwrap();
    let h = run.players.get_mut(&1).unwrap();
    h.x = d.x;
    h.y = d.y;
    run.tick_industry(&BTreeMap::new(), run.tick);
    assert_eq!(run.enemies.len(), native_count + 2);
    assert_eq!(run.kingdom.ore_sentries[&d.id].sentries.len(), 2);
    assert!(
        run.valid_snapshot(),
        "ore sentries coexist with the live native camp"
    );
    // This helper advances real steps and therefore settles the ore sentries
    // too. Inspect admission before it clears the entire company.
    boss_gate_testing::finish_room(&mut run);
    assert!(run.valid_snapshot());
    assert!(run.ore_sentries_defeated(&d.id));
}
#[test]
fn ore_threats_warn_before_their_attacks_hurt_a_careless_knight() {
    for kind in OreKind::ALL {
        let (mut run, _) = fixture(kind);
        let start = run.players[&1].hp;
        let mut warned = false;
        for tick in 0..HZ * 25 {
            // A copied arrow is fired away from the stalkers; then this
            // careless player stands on the return line instead of dodging.
            let input = Input {
                fire: kind == OreKind::Crystal && tick < 45,
                aim_y: 1,
                ..Default::default()
            };
            run.step(&BTreeMap::from([(1, input)]));
            assert_eq!(run.kingdom.ore_sentries.len(), 1);
            assert_eq!(run.enemies.len(), 2);
            warned |= match kind {
                OreKind::Coal => !run.lobs.is_empty(),
                OreKind::Crystal => run.enemies.iter().any(|e| e.stage & 8 != 0),
                _ => run.enemies.iter().any(|e| e.stage == 1),
            };
            if warned && run.players[&1].hp < start {
                break;
            }
        }
        assert!(warned, "{kind:?} must show its actual native tell");
        assert!(
            run.players[&1].hp < start,
            "{kind:?} must inflict real damage if its warning is ignored"
        );
        eprintln!(
            "{kind:?}: careless knight lost {} HP",
            start - run.players[&1].hp
        );
    }
}
