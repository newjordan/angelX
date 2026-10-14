use super::*;
use crate::realm::Spoil;

/// Floor one's entrance (a plain room, no pillars) as a fight, knight one
/// at its middle, one monster of `kind` at `(x, y)` ready to act.
fn with(kind: EnemyKind, x: f32, y: f32) -> Run {
    let mut run = Run::new(33, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(kind, x, y);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.invulnerable = 0;
    run.phase = Phase::Fighting;
    run
}

fn tick(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

/// Step with knight one held at full health and in place.
fn hold(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp;
        run.step(&BTreeMap::new());
    }
}

#[test]
fn rooms_keep_to_one_of_each_dangerous_kind() {
    for seed in 0..40 {
        for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge] {
            let floor = layout::floor(2, pack, &mut Rng::new(seed));
            for room in &floor.rooms {
                for kind in room.roster.iter().filter(|k| k.elite()) {
                    assert_eq!(
                        room.roster.iter().filter(|k| *k == kind).count(),
                        1,
                        "{pack:?} {:?}",
                        room.roster
                    );
                }
            }
        }
    }
}

#[test]
fn a_sappers_keg_blows_and_a_shot_sets_it_off_early() {
    let mut run = with(EnemyKind::Sapper, 34.0, 14.0);
    hold(&mut run, 150);
    assert!(!run.kegs.is_empty(), "a keg is planted near the knight");
    let keg = run.kegs[0];
    assert!((keg.x - 24.0).hypot(keg.y - 14.0) < 10.0);
    // A knight's shot at a keg sets it off, and a monster beside it feels it.
    run.spawn_at_for_test(EnemyKind::Skeleton, keg.x + 1.0, keg.y);
    let skeleton = run.enemies.last().unwrap().id;
    let hp = run.enemies.last().unwrap().hp;
    run.projectiles.push(Projectile {
        x: keg.x - 0.5,
        y: keg.y,
        vx: 1.0,
        vy: 0.0,
        hostile: false,
        kind: Shot::Arrow,
        look: None,
        damage: 1,
        pierce: 0,
        last_hit: None,
        empowered: false,
        ttl: 30,
        traits: ShotTraits::default(),
    });
    run.players.get_mut(&1).unwrap().hp = 100;
    tick(&mut run, 3);
    assert!(
        run.kegs.iter().all(|k| (k.x, k.y) != (keg.x, keg.y)),
        "blown"
    );
    let after = run
        .enemies
        .iter()
        .find(|e| e.id == skeleton)
        .map_or(0, |e| e.hp);
    assert!(
        after + 30 <= hp,
        "the keg hurts monsters too: {hp} → {after}"
    );
}

#[test]
fn a_keg_left_alone_hurts_a_knight_beside_it() {
    let mut run = with(EnemyKind::Bat, 46.0, 2.0);
    run.enemies.clear();
    run.phase = Phase::Fighting;
    run.kegs.push(foes::Keg {
        x: 25.0,
        y: 14.0,
        fuse: 30,
    });
    let hp = run.players[&1].hp;
    tick(&mut run, 2);
    assert!(run.kegs.is_empty(), "an armed keg goes off under a knight");
    assert!(run.players[&1].hp < hp);
}

#[test]
fn a_necromancer_raises_the_dead() {
    let mut run = with(EnemyKind::Necromancer, 36.0, 10.0);
    let raised = |run: &Run| {
        run.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Skeleton)
            .count()
    };
    hold(&mut run, 160);
    assert!(raised(&run) >= 2, "two skeletons rise");
    assert!(run.cues.iter().any(|c| c == "necro_raise"));
}

#[test]
fn a_necromancer_has_bones_for_three_raisings() {
    let mut run = with(EnemyKind::Necromancer, 36.0, 10.0);
    hold(&mut run, 150 * 5);
    let raisings = run.cues.iter().filter(|c| *c == "necro_raise").count();
    assert_eq!(raisings, usize::from(foes::NECRO_RAISES));
}

#[test]
fn what_stands_in_for_a_second_dangerous_kind_is_never_dangerous() {
    for pack in [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Hellforge,
        Pack::Archive,
        Pack::Fungal,
        Pack::Unknown,
    ] {
        for depth in 1..=DEEPEST {
            assert!(!pack.plain_at(depth).elite(), "{pack:?} floor {depth}");
        }
    }
}

#[test]
fn a_warboar_marks_its_line_charges_and_runs_a_knight_down() {
    let mut run = with(EnemyKind::Warboar, 38.0, 14.0);
    let mut marked = false;
    let mut hit = false;
    for _ in 0..200 {
        let before = run.players[&1].hp;
        let at = (run.players[&1].x, run.players[&1].y);
        run.step(&BTreeMap::new());
        let boar = run.enemies.iter().find(|e| e.kind == EnemyKind::Warboar);
        marked |= boar.is_some_and(|b| b.stage == 1);
        let hero = run.players.get_mut(&1).unwrap();
        if hero.hp + 20 < before {
            hit = true;
            assert_ne!((hero.x, hero.y), at, "and thrown aside");
            break;
        }
        hero.invulnerable = 0;
    }
    assert!(marked, "it marks its line before it goes");
    assert!(hit, "the charge runs the knight down");
}

#[test]
fn a_warboar_that_hits_a_wall_stands_dazed() {
    let mut run = with(EnemyKind::Warboar, 24.0, 6.0);
    {
        let boar = run.enemies.last_mut().unwrap();
        boar.stage = 2;
        boar.dir = (0.0, -1.0);
    }
    tick(&mut run, 12);
    let boar = run.enemies.last().unwrap();
    assert_eq!(boar.stage, 3, "dazed against the north wall");
}

#[test]
fn slimes_split_twice() {
    let mut run = with(EnemyKind::Slime, 34.0, 14.0);
    for (generation, count) in [(1, 2), (2, 4)] {
        for enemy in &mut run.enemies {
            enemy.hp = 0;
        }
        tick(&mut run, 1);
        let slimes: Vec<&Enemy> = run
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Slime)
            .collect();
        assert_eq!(slimes.len(), count);
        assert!(slimes.iter().all(|s| s.stage == generation));
        assert!(slimes.iter().all(|s| s.radius() < 1.0));
    }
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    tick(&mut run, 1);
    assert!(
        run.enemies.iter().all(|e| e.kind != EnemyKind::Slime),
        "the smallest just pop"
    );
}

#[test]
fn a_loot_goblin_with_nothing_to_take_keeps_clear_and_slips_away() {
    let mut run = with(EnemyKind::Goblin, 30.0, 14.0);
    let start = run.enemies[0].x;
    hold(&mut run, thief::IDLE_TICKS / 4);
    let goblin = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Goblin)
        .unwrap();
    assert!(
        (goblin.x - 24.0).hypot(goblin.y - 14.0) > (start - 24.0).abs(),
        "it keeps clear of a knight with nothing to lose"
    );
    hold(&mut run, thief::IDLE_TICKS);
    assert!(
        run.enemies.iter().all(|e| e.kind != EnemyKind::Goblin),
        "gone"
    );
    assert!(run.cues.iter().any(|c| c == "goblin_escaped"));
    // Caught: gold all round, and now and then a prize.
    let mut run = with(EnemyKind::Goblin, 30.0, 14.0);
    run.enemies[0].hp = 0;
    tick(&mut run, 1);
    let gold = run.room().items.iter().filter(|i| i.card == "gold").count();
    assert!(gold >= 5, "{gold} gold");
}

/// A knight with something to lose, and a goblin at their elbow.
fn robbery() -> Run {
    let mut run = with(EnemyKind::Goblin, 26.0, 14.0);
    let hero = run.players.get_mut(&1).unwrap();
    hero.carried.add(Spoil::Gold, 100);
    hero.carried.add(Spoil::Gem, 3);
    hero.hand = vec!["potion".into()];
    run
}

fn goblin(run: &Run) -> Option<&Enemy> {
    run.enemies.iter().find(|e| e.kind == EnemyKind::Goblin)
}

#[test]
fn a_goblin_steals_on_touch_and_cries_thief() {
    let mut run = robbery();
    let had = run.players[&1].carried.clone();
    let hand = run.players[&1].hand.len();
    hold(&mut run, 20);
    let g = goblin(&run).expect("it is still here");
    assert!(g.burden() > 0, "something is in the sack");
    assert!(g.fleeing(), "and it runs");
    assert!(g.cry > 0, "THIEF! hangs over it");
    let hero = &run.players[&1];
    assert!(
        hero.carried != had || hero.hand.len() < hand,
        "the knight is lighter"
    );
    assert!(run.cues.iter().any(|c| c == "thief"));
    assert!(
        run.found
            .as_ref()
            .is_some_and(|(_, who, line)| *who == 1 && line.starts_with("THIEF!")),
        "{:?}",
        run.found
    );
    assert!(
        run.found_line().is_some_and(|l| l.contains("THIEF!")),
        "the notice every screen shows"
    );
}

#[test]
fn a_goblin_does_not_fight_back() {
    let mut run = with(EnemyKind::Goblin, 25.0, 14.0);
    let hp = run.players[&1].hp;
    tick(&mut run, 90);
    assert_eq!(run.players[&1].hp, hp, "no theft, no blow");
}

#[test]
fn a_goblin_runs_fast_and_gets_away_with_it_in_ten_seconds_or_so() {
    let mut run = robbery();
    hold(&mut run, 6);
    assert!(goblin(&run).is_some_and(Enemy::fleeing));
    let (x, y) = {
        let g = goblin(&run).unwrap();
        (g.x, g.y)
    };
    let kept = run.players[&1].carried.clone();
    hold(&mut run, 30);
    let g = goblin(&run).expect("still running");
    let went = (g.x - x).hypot(g.y - y);
    assert!(went > 6.0, "quick: {went} in a second");
    let mut ticks = 36;
    while goblin(&run).is_some() && ticks < 20 * HZ {
        hold(&mut run, 1);
        ticks += 1;
    }
    assert!(goblin(&run).is_none(), "through a door");
    assert!(
        (7 * HZ..=13 * HZ).contains(&ticks),
        "eight to twelve seconds: {ticks} ticks"
    );
    assert!(run.cues.iter().any(|c| c == "goblin_escaped"));
    assert!(
        run.players[&1].carried.get(Spoil::Gold) <= kept.get(Spoil::Gold),
        "what it took is gone"
    );
    assert!(
        !run.room().items.iter().any(|i| i.card == "potion"),
        "and nothing is left lying"
    );
}

#[test]
fn a_goblin_killed_bursts_with_everything_it_took_and_gold_on_top() {
    let mut run = robbery();
    run.players.get_mut(&1).unwrap().hand = vec!["potion".into(), "holy-water".into()];
    let total = run.players[&1].carried.clone();
    for _ in 0..3 {
        hold(&mut run, 4);
        // Let it take a second and third thing from the same knight.
        let g = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Goblin);
        if let Some(g) = g {
            let hero = &run.players[&1];
            (g.x, g.y) = (hero.x + 0.5, hero.y);
        }
    }
    let g = goblin(&run).expect("still here");
    assert!(g.burden() >= 1);
    let taken = g.stash.clone();
    let cards = g.loot.clone();
    let (x, y) = (g.x, g.y);
    run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Goblin).unwrap().hp = 0;
    run.players.get_mut(&1).unwrap().hp = 100;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x, y);
    tick(&mut run, 1);
    assert!(goblin(&run).is_none());
    assert!(!taken.is_empty() || !cards.is_empty());
    assert_eq!(run.players[&1].carried, total, "every pile it took is back");
    for card in &cards {
        assert!(
            run.room().items.iter().any(|i| &i.card == card),
            "{card} lies where it fell"
        );
    }
    let gold = run.room().items.iter().filter(|i| i.card == "gold").count();
    assert!(gold >= 5, "a pile of gold on top: {gold}");
    assert!(run.cues.iter().any(|c| c == "goblin_caught"));
}

#[test]
fn a_goblin_grabs_what_lies_on_the_floor() {
    let mut run = with(EnemyKind::Goblin, 36.0, 14.0);
    run.drop_item("potion".into(), 38.0, 14.0, None);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (10.0, 14.0);
    tick(&mut run, 60);
    assert!(
        !run.room().items.iter().any(|i| i.card == "potion"),
        "the potion is gone from the floor"
    );
    assert!(goblin(&run).is_some_and(|g| g.loot == ["potion"]));
}

#[test]
fn the_sack_fills_as_it_takes() {
    let mut run = robbery();
    assert_eq!(goblin(&run).unwrap().burden(), 0);
    let g = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Goblin).unwrap();
    g.loot.push("potion".into());
    assert_eq!(g.burden(), 1);
    g.stash.add(Spoil::Gold, 10);
    g.stash.add(Spoil::Gem, 1);
    assert_eq!(g.burden(), 2);
    g.loot.extend(["a".to_string(), "b".to_string()]);
    assert_eq!(g.burden(), 3);
}

#[test]
fn goblins_come_about_one_room_in_three_more_on_the_deep_floors_and_now_and_then_two() {
    let count = |depth: u32| {
        let (mut rooms, mut with_goblin, mut pairs) = (0u32, 0u32, 0u32);
        for seed in 0..40 {
            let mut run = Run::new(seed, 9, None);
            while run.dungeon.depth < depth {
                run.descend_for_test();
            }
            let fights: Vec<usize> = run
                .dungeon
                .rooms
                .iter()
                .enumerate()
                .filter(|(_, r)| r.kind == RoomKind::Fight && !r.cleared)
                .map(|(i, _)| i)
                .collect();
            for room in fights {
                run.enter_for_test(room);
                let n = run
                    .enemies
                    .iter()
                    .filter(|e| e.kind == EnemyKind::Goblin)
                    .count();
                rooms += 1;
                with_goblin += u32::from(n > 0);
                pairs += u32::from(n > 1);
            }
        }
        (rooms, with_goblin, pairs)
    };
    let (rooms, with, pairs) = count(1);
    let share = with * 100 / rooms.max(1);
    assert!((20..=45).contains(&share), "floor 1: {share}% of {rooms}");
    assert!(pairs > 0, "two at once now and then");
    let (rooms, deep, _) = count(5);
    assert!(deep * 100 / rooms.max(1) > share, "more on the deep floors");
}

#[test]
fn a_hob_lobs_a_bomb_where_the_knight_stood() {
    let mut run = with(EnemyKind::Hob, 36.0, 10.0);
    let mut thrown = None;
    for _ in 0..120 {
        run.players.get_mut(&1).unwrap().hp = 100;
        run.step(&BTreeMap::new());
        if let Some(lob) = run.lobs.first() {
            thrown = Some(*lob);
            break;
        }
    }
    let lob = thrown.expect("a bomb is thrown");
    assert!((lob.x - 24.0).hypot(lob.y - 14.0) < 0.1, "at the knight");
    let hp = run.players[&1].hp;
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    tick(&mut run, foes::LOB_FALL);
    assert!(run.players[&1].hp < hp, "standing still under it hurts");
}

#[test]
fn a_shaman_boxes_a_knight_in_with_wards_that_crumble() {
    let mut run = with(EnemyKind::Shaman, 36.0, 6.0);
    let wards = |run: &Run| {
        run.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Ward)
            .count()
    };
    hold(&mut run, 100);
    assert!(wards(&run) >= 3, "{} wards", wards(&run));
    for ward in run.enemies.iter().filter(|e| e.kind == EnemyKind::Ward) {
        assert!(
            (ward.x - 24.0).hypot(ward.y - 14.0) < 5.5,
            "round the knight"
        );
    }
    run.enemies.retain(|e| e.kind == EnemyKind::Ward);
    hold(&mut run, foes::WARD_STANDS + 2);
    assert_eq!(wards(&run), 0, "they crumble in time");
}

#[test]
fn the_mirror_carries_kegs_and_bombs() {
    let mut run = with(EnemyKind::Hob, 36.0, 10.0);
    run.kegs.push(foes::Keg {
        x: 10.0,
        y: 10.0,
        fuse: 50,
    });
    run.lobs.push(foes::Lob {
        from: (36.0, 10.0),
        x: 24.0,
        y: 14.0,
        fall: 20,
    });
    run.flings.push(foes::Fling {
        from: (36.0, 10.0),
        x: 24.0,
        y: 14.0,
        fall: 20,
    });
    let live = run.wire_copy();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.kegs.clear();
    friend.lobs.clear();
    friend.flings.clear();
    friend = live;
    assert_eq!((friend.kegs.len(), friend.lobs.len(), friend.flings.len()), (1, 1, 1));
}

#[test]
fn a_broodmarrow_tears_a_child_from_its_back_and_throws_it() {
    let mut run = with(EnemyKind::Broodmarrow, 34.0, 14.0);
    let full = run.enemies[0].hp;
    let mut ripped = false;
    for _ in 0..400 {
        run.players.get_mut(&1).unwrap().hp = run.players[&1].max_hp;
        run.step(&BTreeMap::new());
        if !run.flings.is_empty() {
            ripped = true;
            break;
        }
    }
    assert!(ripped, "it throws");
    assert!(run.cues.iter().any(|c| c == "brood_rip"));
    let (x, y) = (run.flings[0].x, run.flings[0].y);
    assert!((x - 24.0).hypot(y - 14.0) < 1.0, "at the knight");
    let marrow_hp = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Broodmarrow)
        .unwrap()
        .hp;
    assert!(marrow_hp < full, "the rip is a wound it chose");
    // It lands with its teeth out, and stands up as an imp.
    let hp = run.players[&1].hp;
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    tick(&mut run, foes::FLING_FALL);
    assert!(run.flings.is_empty(), "it landed");
    assert!(
        run.enemies.iter().any(|e| e.kind == EnemyKind::Imp),
        "an impling takes up the fight"
    );
    assert!(run.players[&1].hp < hp, "the landing bites");
}

#[test]
fn a_broodmarrows_brood_is_bounded_and_it_never_bleeds_itself_dead() {
    let mut run = with(EnemyKind::Broodmarrow, 34.0, 14.0);
    // One rip from the floor of its life: it pays once, and then the
    // brood is not free — no farming the poor for bounty.
    run.enemies[0].hp = foes::BROOD_RIP + 1;
    hold(&mut run, 26 * HZ);
    let imps = run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Imp)
        .count();
    assert!(
        imps <= foes::BROOD_BROOD,
        "at most {} children: {imps}",
        foes::BROOD_BROOD
    );
    let marrow = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Broodmarrow)
        .unwrap();
    assert!(
        marrow.hp >= 1 && marrow.hp <= foes::BROOD_RIP,
        "it paid what it had and keeps only its last measure"
    );
    assert!(run.enemies.len() <= 40, "the room keeps its bounds");
    assert!(run.flings.len() <= foes::BROOD_FLINGS);
}

#[test]
fn the_oathbound_share_one_wound_and_grieve_alone() {
    let mut run = with(EnemyKind::Oathbound, 34.0, 14.0);
    // Give it its twin: the pair stands shoulder to shoulder.
    let twin_id = run.spawn_at_for_test(EnemyKind::Oathbound, 35.0, 15.0);
    let ids: Vec<u32> = run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Oathbound)
        .map(|e| e.id)
        .collect();
    assert_eq!(ids.len(), 2);
    // Wound one: the oath bills both.
    run.enemies
        .iter_mut()
        .find(|e| e.id == ids[0])
        .unwrap()
        .hp -= 60;
    run.step(&BTreeMap::new());
    let (a, b) = (
        run.enemies.iter().find(|e| e.id == ids[0]).unwrap().hp,
        run.enemies.iter().find(|e| e.id == ids[1]).unwrap().hp,
    );
    assert_eq!(a, b, "what one takes, both feel ({a} vs {b})");
    assert!(a < 240);
    assert!(run.valid_snapshot());
    // Fell one with a blow meant for it: the survivor keeps exactly one
    // wound, the oath breaks, and the grief begins.
    run.enemies
        .iter_mut()
        .find(|e| e.id == ids[0])
        .unwrap()
        .hp = 0;
    run.step(&BTreeMap::new());
    let survivor = run.enemies.iter().find(|e| e.id == ids[1]).unwrap();
    assert_eq!(survivor.hp, 1, "the oath's last cruelty floors the survivor");
    assert_eq!(survivor.stage, 1, "the grief has begun");
    assert!(run.cues.iter().any(|c| c == "oath_broken"));
    let _ = twin_id;
}

#[test]
fn a_stroke_that_reaches_both_oathbound_ends_the_oath_entirely() {
    let mut run = with(EnemyKind::Oathbound, 34.0, 14.0);
    run.spawn_at_for_test(EnemyKind::Oathbound, 35.0, 15.0);
    // One stroke, both graves: no survivor, no grief, no cue.
    for e in run.enemies.iter_mut().filter(|e| e.kind == EnemyKind::Oathbound) {
        e.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert!(
        run.enemies.iter().all(|e| e.kind != EnemyKind::Oathbound),
        "both fell together"
    );
    assert!(
        !run.cues.iter().any(|c| c == "oath_broken"),
        "no oath broke: there is nobody left to grieve"
    );
}

#[test]
fn a_whirlgrave_well_pulls_knights_and_bends_shots() {
    let mut run = with(EnemyKind::Whirlgrave, 34.0, 14.0);
    // A well, opened at a known spot beside the knight.
    run.wells.push(foes::Well {
        x: 20.0,
        y: 14.0,
        left: foes::WELL_SPAN,
    });
    let start = run.players[&1].x;
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    let hero = &run.players[&1];
    assert!(
        hero.x < start - 0.3,
        "the knight slides toward the lean: {} -> {}",
        start,
        hero.x
    );
    assert!(run.valid_snapshot());
    // A straight arrow bends: fire one past the well's shoulder and it
    // lands off the line a no-well control run draws.
    let mut control = run.clone();
    control.wells.clear();
    let fire = BTreeMap::from([(
        1,
        Input {
            aim_x: 1,
            fire: true,
            ..Default::default()
        },
    )]);
    run.step(&fire);
    control.step(&fire);
    // Arrows fly fast: as far as the old slow ones went in twenty ticks.
    for _ in 0..8 {
        run.step(&BTreeMap::new());
        control.step(&BTreeMap::new());
    }
    let bent = run.projectiles.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>();
    let straight = control
        .projectiles
        .iter()
        .map(|p| (p.x, p.y))
        .collect::<Vec<_>>();
    assert!(!bent.is_empty() || !straight.is_empty(), "shots flew");
    if let (Some(b), Some(c)) = (bent.first(), straight.first()) {
        assert!(
            (b.0 - c.0).abs() + (b.1 - c.1).abs() > 0.5,
            "the well bent the shot ({b:?} vs {c:?})"
        );
    }
}

#[test]
fn a_whirlgraves_wells_close_with_their_stone_and_two_is_the_limit() {
    // Two stones, two leans: the room will humour exactly that many.
    let mut run = with(EnemyKind::Whirlgrave, 34.0, 14.0);
    run.spawn_at_for_test(EnemyKind::Whirlgrave, 30.0, 14.0);
    run.wells.push(foes::Well { x: 20.0, y: 14.0, left: foes::WELL_SPAN });
    run.wells.push(foes::Well { x: 26.0, y: 10.0, left: foes::WELL_SPAN });
    assert_eq!(run.wells.len(), 2);
    // A third is simply not poured, even when a stone tries.
    run.spawn_at_for_test(EnemyKind::Whirlgrave, 32.0, 12.0);
    for _ in 0..20 * HZ as usize {
        run.step(&BTreeMap::new());
        assert!(run.wells.len() <= foes::WELLS_MAX, "two wells, no more");
    }
    // Fell every stone: the lean ends with them.
    for e in run.enemies.iter_mut().filter(|e| e.kind == EnemyKind::Whirlgrave) {
        e.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert!(run.wells.is_empty(), "no stone, no lean");
}
