use super::*;

/// A calm fight room on floor one, knight one dressed as `knight` with a
/// full charge, facing east, monsters placed at `foes`.
fn arena(knight: &str, foes: &[(EnemyKind, f32, f32)]) -> Run {
    let mut run = Run::new(21, 1, None);
    run.outfit(1, knights::knight(knight).unwrap());
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(fight);
    run.calm_for_test();
    run.enemies.clear();
    for &(kind, x, y) in foes {
        run.spawn_at_for_test(kind, x, y);
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (12.0, 14.0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    hero.ult_charge = ULT_FULL;
    hero.invulnerable = 0;
    run.phase = Phase::Fighting;
    run
}

fn press(run: &mut Run) {
    run.step(&BTreeMap::from([(
        1,
        Input {
            ult: true,
            ..Input::default()
        },
    )]));
    run.step(&BTreeMap::new());
}

fn idle(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
    }
}

#[test]
fn every_knight_of_the_company_has_their_own_ultimate() {
    let ults: std::collections::BTreeSet<&str> = knights::COMPANY
        .iter()
        .map(|k| Ult::of(Some(k.id)).word())
        .collect();
    assert_eq!(ults.len(), knights::COMPANY.len());
}

#[test]
fn the_charge_fills_by_fighting_and_r_casts_only_a_full_one() {
    let mut run = arena("percival", &[(EnemyKind::Demon, 30.0, 14.0)]);
    run.players.get_mut(&1).unwrap().ult_charge = 0;
    press(&mut run);
    assert!(
        run.projectiles.iter().all(|p| p.kind != Shot::Sacred),
        "not charged"
    );
    // Shooting a demon fills it faster than standing about.
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            aim_x: 1,
            ..Input::default()
        },
    )]);
    for _ in 0..90 {
        run.players.get_mut(&1).unwrap().hp = 100;
        run.step(&fire);
    }
    let charge = run.players[&1].ult_charge;
    assert!(charge > 90 + 3 * 16, "{charge}: fighting plus damage dealt");
    run.players.get_mut(&1).unwrap().ult_charge = ULT_FULL;
    press(&mut run);
    assert!(run.projectiles.iter().any(|p| p.kind == Shot::Sacred));
    assert!(run.players[&1].ult_charge < 10, "spent, and filling again");
    assert!(run.cues.iter().any(|c| c == "ult:pilgrims_arrow"));
}

#[test]
fn a_pilgrims_arrow_pierces_and_stuns_what_it_hits() {
    let mut run = arena(
        "percival",
        &[
            (EnemyKind::Demon, 20.0, 14.0),
            (EnemyKind::Demon, 30.0, 14.0),
        ],
    );
    let before: Vec<u32> = run.enemies.iter().map(|e| e.hp).collect();
    press(&mut run);
    idle(&mut run, 24);
    assert_eq!(run.enemies.len(), 2);
    for (enemy, hp) in run.enemies.iter().zip(before) {
        assert!(enemy.hp < hp, "both hit");
        assert!(enemy.frozen > 0, "and stunned");
    }
}

#[test]
fn ladys_veil_makes_the_party_untouchable() {
    let mut run = arena("lynette", &[(EnemyKind::Imp, 30.0, 14.0)]);
    run.join_seat(2, "Friend");
    press(&mut run);
    for hero in run.players.values() {
        assert!(hero.angel > 0 && hero.invulnerable > 3 * HZ);
    }
}

#[test]
fn trebuchet_shells_land_where_gareth_aims() {
    let mut run = arena("gareth", &[(EnemyKind::Demon, 21.0, 14.0)]);
    let hp = run.enemies[0].hp;
    press(&mut run);
    assert_eq!(run.strikes.len(), 8);
    idle(&mut run, 90);
    assert!(run.strikes.is_empty());
    assert!(run.enemies.first().is_none_or(|e| e.hp < hp - 100));
}

#[test]
fn bladewind_blinks_galahad_from_monster_to_monster() {
    let mut run = arena(
        "galahad",
        &[
            (EnemyKind::Skeleton, 22.0, 10.0),
            (EnemyKind::Skeleton, 30.0, 18.0),
        ],
    );
    let start = (run.players[&1].x, run.players[&1].y);
    press(&mut run);
    idle(&mut run, 40);
    let hero = &run.players[&1];
    assert_ne!((hero.x, hero.y), start, "he went to them");
    assert_eq!(hero.slashes, 0);
    assert!(run.enemies.iter().all(|e| e.hp < e.max_hp) || run.enemies.len() < 2);
}

#[test]
fn grand_chord_pulls_the_room_into_a_stunned_knot() {
    let mut run = arena(
        "composer",
        &[(EnemyKind::Imp, 20.0, 8.0), (EnemyKind::Imp, 19.0, 20.0)],
    );
    press(&mut run);
    let point = (12.0 + 3.2, 14.0);
    for enemy in &run.enemies {
        assert!(enemy.frozen > 0);
        assert!(
            (enemy.x - point.0).hypot(enemy.y - point.1) < 3.0,
            "pulled in: {:?}",
            (enemy.x, enemy.y)
        );
    }
}

#[test]
fn the_dispatcher_sends_six_sealed_writs() {
    let mut run = arena("dispatcher", &[(EnemyKind::Wraith, 34.0, 6.0)]);
    let hp = run.enemies[0].hp;
    press(&mut run);
    assert_eq!(
        run.projectiles
            .iter()
            .filter(|p| p.kind == Shot::Missile)
            .count(),
        6
    );
    idle(&mut run, 60);
    assert!(run.enemies.first().is_none_or(|e| e.hp < hp));
}

#[test]
fn a_stillhour_stops_monsters_and_their_shots() {
    let mut run = arena("loop", &[(EnemyKind::Skeleton, 17.0, 14.0)]);
    press(&mut run);
    assert_eq!(run.spheres.len(), 1);
    let (x, y) = (run.enemies[0].x, run.enemies[0].y);
    run.projectiles.push(Projectile {
        x: 17.5,
        y: 15.0,
        vx: -9.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Bone,
        look: None,
        damage: 12,
        pierce: 0,
        last_hit: None,
        empowered: false,
        ttl: 90,
        traits: ShotTraits::default(),
    });
    idle(&mut run, 60);
    assert_eq!((run.enemies[0].x, run.enemies[0].y), (x, y), "held in time");
    assert!(
        run.projectiles.iter().any(|p| p.hostile && p.x == 17.5),
        "its shot hangs in the air"
    );
    idle(&mut run, 4 * HZ);
    assert!(run.spheres.is_empty(), "and time comes back");
}

#[test]
fn midas_touch_turns_the_nearest_monsters_to_gold() {
    let mut run = arena(
        "money",
        &[
            (EnemyKind::Skeleton, 16.0, 14.0),
            (EnemyKind::Imp, 18.0, 10.0),
            (EnemyKind::Bat, 22.0, 18.0),
            (EnemyKind::Wraith, 40.0, 4.0),
        ],
    );
    press(&mut run);
    assert_eq!(run.enemies.len(), 1, "the three nearest are gold");
    assert!(run.room().items.iter().filter(|i| i.card == "gold").count() >= 3);
}

#[test]
fn assassinate_aims_a_second_then_hits_the_toughest() {
    let mut run = arena(
        "competition",
        &[(EnemyKind::Bat, 16.0, 14.0), (EnemyKind::Demon, 36.0, 8.0)],
    );
    let demon = run.enemies[1].id;
    press(&mut run);
    assert_eq!(run.players[&1].aiming.map(|a| a.0), Some(demon));
    idle(&mut run, HZ / 2);
    assert!(
        run.projectiles.iter().all(|p| p.kind != Shot::Sacred),
        "still aiming"
    );
    let hp = run.enemies.iter().find(|e| e.id == demon).unwrap().hp;
    idle(&mut run, 2 * HZ);
    let after = run
        .enemies
        .iter()
        .find(|e| e.id == demon)
        .map_or(0, |e| e.hp);
    assert!(
        after < hp.saturating_sub(200),
        "{hp} → {after}: one shot, most of a demon"
    );
}

#[test]
fn phantasm_images_fire_with_their_knight_and_take_shots_for_them() {
    let mut run = arena("scryglass", &[(EnemyKind::Demon, 40.0, 14.0)]);
    press(&mut run);
    assert_eq!(run.phantoms.len(), 2);
    run.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            aim_x: 1,
            ..Input::default()
        },
    )]));
    let arrows = run
        .projectiles
        .iter()
        .filter(|p| !p.hostile && p.kind == Shot::Arrow)
        .count();
    assert_eq!(arrows, 3, "the knight's arrow and each image's");
    let (px, py) = (run.phantoms[0].x, run.phantoms[0].y);
    run.projectiles.push(Projectile {
        x: px + 1.0,
        y: py,
        vx: -20.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Bone,
        look: None,
        damage: 12,
        pierce: 0,
        last_hit: None,
        empowered: false,
        ttl: 90,
        traits: ShotTraits::default(),
    });
    run.step(&BTreeMap::new());
    assert_eq!(run.phantoms.len(), 1, "an image takes the shot and is gone");
}
