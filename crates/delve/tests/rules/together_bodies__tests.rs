use super::*;

/// Floor one of the Crypt, its first fight room emptied and calmed, with
/// knight one walking in `kind`'s body, aiming right.
fn raised(kind: EnemyKind) -> Run {
    let mut run = Run::new(4, 1, Some("Bree"));
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.rune = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (10.0, 14.0, 0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    hero.possessed = Some(kind);
    run
}

/// A monster `dx` to the right of knight one, held still.
fn foe(run: &mut Run, kind: EnemyKind, dx: f32) -> u32 {
    let (x, y) = (run.players[&1].x + dx, run.players[&1].y);
    run.spawn_at_for_test(kind, x, y);
    let enemy = run.enemies.last_mut().unwrap();
    (enemy.frozen, enemy.age) = (100_000, 100_000);
    enemy.id
}

fn tick(run: &mut Run, input: Input) {
    run.step(&BTreeMap::from([(1, input)]));
}

fn fire() -> Input {
    Input {
        fire: true,
        aim_x: 1,
        ..Default::default()
    }
}

fn cast() -> Input {
    Input {
        cast: 1,
        ..Default::default()
    }
}

fn hp(run: &Run, id: u32) -> u32 {
    run.enemies.iter().find(|e| e.id == id).map_or(0, |e| e.hp)
}

#[test]
fn a_raised_skeleton_looses_the_skeletons_bones_not_the_knights_arm() {
    let mut run = raised(EnemyKind::Skeleton);
    // One behind, so the room is still a fight.
    foe(&mut run, EnemyKind::Bat, -8.0);
    tick(&mut run, fire());
    let mine: Vec<&Projectile> = run.projectiles.iter().filter(|p| !p.hostile).collect();
    assert!(!mine.is_empty(), "the fire key looses something");
    for shot in &mine {
        assert_eq!(shot.kind, Shot::Bone, "a Crypt skeleton's darts");
        assert!(shot.traits.borrowed);
        assert_eq!(shot.traits.owner, 1);
        assert!(!shot.traits.arm, "no shot of the knight's own arm");
    }
    let (arms, every) = bodies::arms(EnemyKind::Skeleton, Pack::Crypt);
    assert!(matches!(arms, bodies::Arms::Volley { .. }));
    assert!(run.players[&1].fire_cooldown <= every && run.players[&1].fire_cooldown > 0);
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert_eq!(hud.weapon.name, "Bone Darts");
}

#[test]
fn the_bones_find_the_monster_down_the_aim() {
    let mut run = raised(EnemyKind::Skeleton);
    let bat = foe(&mut run, EnemyKind::Bat, 6.0);
    let full = hp(&run, bat);
    tick(&mut run, fire());
    for _ in 0..HZ {
        tick(&mut run, Input::default());
    }
    assert!(hp(&run, bat) < full, "a borrowed dart strikes a monster");
    assert_eq!(run.players[&1].hp, run.players[&1].max_hp, "and never its knight");
}

#[test]
fn a_raised_bat_bites_and_looses_nothing() {
    let mut run = raised(EnemyKind::Bat);
    let imp = foe(&mut run, EnemyKind::Imp, 1.6);
    let full = hp(&run, imp);
    tick(&mut run, fire());
    assert!(run.projectiles.iter().all(|p| p.hostile), "teeth, not shots");
    assert!(hp(&run, imp) < full, "the bite lands");
    assert_eq!(run.hud(1, &hud::Keys::FIRST).unwrap().weapon.name, "Bite");
}

#[test]
fn a_raised_leech_drinks_what_it_bites() {
    let mut run = raised(EnemyKind::Leech);
    foe(&mut run, EnemyKind::Imp, 1.4);
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = hero.max_hp / 2;
    let before = run.players[&1].hp;
    tick(&mut run, fire());
    assert!(run.players[&1].hp > before, "half of the bite mends it");
}

#[test]
fn the_sword_and_the_spell_cards_wait_in_a_borrowed_body() {
    let mut run = raised(EnemyKind::Bat);
    let imp = foe(&mut run, EnemyKind::Imp, 1.4);
    let full = hp(&run, imp);
    tick(
        &mut run,
        Input {
            swing: true,
            ..Default::default()
        },
    );
    assert_eq!(hp(&run, imp), full, "no sword in a bat");
    // A bat casts nothing, whatever cards the knight holds.
    tick(&mut run, cast());
    assert!(run.projectiles.is_empty());
    assert!(run.hud(1, &hud::Keys::FIRST).unwrap().spells.iter().all(Option::is_none));
}

#[test]
fn a_raised_lich_casts_rimeleap_and_it_leaps_monster_to_monster() {
    let mut run = raised(EnemyKind::Lich);
    foe(&mut run, EnemyKind::Bat, -8.0);
    tick(&mut run, cast());
    let frost: Vec<&Projectile> = run.projectiles.iter().filter(|p| !p.hostile).collect();
    assert!(!frost.is_empty(), "the spell key casts");
    for shot in &frost {
        assert_eq!(shot.kind, Shot::Frost);
        assert_eq!(shot.traits.chain, lich::BOUNCES);
        assert!(shot.traits.chill > 0);
    }
    assert_eq!(run.players[&1].body_rearm, bodies::SPELL_REARM);
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert_eq!(hud.spells[0].as_ref().map(|s| s.name.as_str()), Some("Rimeleap"));
    // Not again till it is ready.
    let cast_once = run.projectiles.len();
    tick(&mut run, Input::default());
    tick(&mut run, cast());
    assert!(run.projectiles.len() <= cast_once, "the spell rearms");
}

#[test]
fn a_raised_hexer_s_hex_dazes_what_it_strikes() {
    let mut run = raised(EnemyKind::Hexer);
    let imp = foe(&mut run, EnemyKind::Imp, 4.0);
    run.enemies.last_mut().unwrap().frozen = 0;
    tick(&mut run, cast());
    for _ in 0..HZ {
        tick(&mut run, Input::default());
        if hp(&run, imp) < run.enemies.iter().find(|e| e.id == imp).unwrap().max_hp {
            break;
        }
    }
    let imp = run.enemies.iter().find(|e| e.id == imp).unwrap();
    assert!(imp.hp < imp.max_hp, "the hex strikes");
    assert!(imp.frozen > feel::JOLT_HIT, "and dazes: {}", imp.frozen);
}

#[test]
fn a_borrowed_beam_burns_the_monsters_and_spares_the_knights() {
    let mut run = raised(EnemyKind::Wraith);
    run.join_seat(2, "Ana");
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y, friend.invulnerable) = (14.0, 14.0, 0);
    let imp = foe(&mut run, EnemyKind::Imp, 8.0);
    let full = hp(&run, imp);
    tick(&mut run, cast());
    assert!(run.projectiles.iter().any(|p| p.traits.borrowed && p.traits.path.beam.is_some()));
    for _ in 0..u32::from(bodies::BODY_WARN) + u32::from(patterns::BEAM_BURN) + 2 {
        tick(&mut run, Input::default());
    }
    assert!(hp(&run, imp) < full, "the stare burns the imp");
    assert_eq!(run.players[&2].hp, run.players[&2].max_hp, "and passes through a friend");
}

#[test]
fn a_borrowed_lob_comes_down_on_the_monsters() {
    let mut run = raised(EnemyKind::Hob);
    run.join_seat(2, "Ana");
    let imp = foe(&mut run, EnemyKind::Imp, 6.0);
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y, friend.invulnerable) = (16.0, 14.0, 0);
    let full = hp(&run, imp);
    tick(&mut run, fire());
    assert!(run.projectiles.iter().any(|p| p.traits.borrowed && p.traits.path.form == patterns::Form::Lob));
    for _ in 0..2 * HZ {
        tick(&mut run, Input::default());
    }
    assert!(hp(&run, imp) < full, "the bomb lands on the imp");
    assert_eq!(run.players[&2].hp, run.players[&2].max_hp, "not on the friend beside it");
}

#[test]
fn a_knight_s_own_page_draws_no_shot_for_a_borrowed_body() {
    let mut run = raised(EnemyKind::Skeleton);
    let mut shots = Vec::new();
    run.predict_fire(1, fire(), run.tick + 1, &mut shots);
    assert!(shots.is_empty(), "the server draws a borrowed volley");
}

#[test]
fn every_raisable_body_has_arms_that_loose_or_bite() {
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Archive, Pack::Hellforge, Pack::Fungal, Pack::Unknown] {
        use EnemyKind as K;
        for kind in [
            K::Bat, K::Skeleton, K::Wraith, K::Imp, K::Demon, K::Mimic, K::Sapper,
            K::Necromancer, K::Warboar, K::Slime, K::Hob, K::Shaman, K::Flesher,
            K::Silkmother, K::Spiderling, K::Hexer, K::Lich, K::Hollow, K::Biter,
            K::Shard, K::Mole, K::Pavise, K::Leech, K::Marksman, K::Mirrorling,
            K::Spitter, K::Ringer, K::Geode, K::Hound, K::Broodmarrow, K::BogHand,
            K::Drowned, K::Wisp,
        ] {
            let (arms, every) = bodies::arms(kind, pack);
            assert!((1..=bodies::BODY_SLOWEST.max(24)).contains(&every), "{kind:?} {every}");
            if let bodies::Arms::Volley { chain, damage } = arms {
                assert!(!chain.is_empty() && damage > 0, "{kind:?}");
            }
            assert!(!bodies::arms_name(kind, pack).is_empty());
        }
    }
}
