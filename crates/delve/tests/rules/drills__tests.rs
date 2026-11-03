use super::*;
use crate::rules::knights::{COMPANY, knight};

fn wearing(kit: &str) -> Run {
    let mut run = Run::new(9, 1, None);
    run.outfit(1, knight(kit).unwrap());
    run
}

fn hero(run: &mut Run) -> &mut Hero {
    run.players.get_mut(&1).unwrap()
}

/// One pull of the knight's arm: the path both host and client come through.
fn loose(run: &mut Run) -> Vec<Projectile> {
    let book = run.book.clone();
    let mut shots = Vec::new();
    let hero = hero(run);
    hero.aim_x = 1.0;
    hero.aim_y = 0.0;
    hero.loose(1, &book, false, 0, 0, &mut shots);
    shots
}

fn rest(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        hero(run).hero_kit.tick();
    }
}

#[test]
fn each_family_the_audit_found_has_one_drilled_kit_on_its_own_arm() {
    let mut drilled = Vec::new();
    for kit in COMPANY {
        if let Some((drill, arm)) = of_kit(kit.id) {
            assert_eq!(arm, kit.arm, "{}'s drill is on its own arm", kit.id);
            assert!(kit.spells.is_empty(), "{} had nothing else to tell it apart", kit.id);
            drilled.push((kit.arm, drill));
        }
    }
    drilled.sort_by_key(|(arm, _)| *arm);
    assert_eq!(
        drilled,
        [
            ("bow", Drill::Loop),
            ("crossbow", Drill::Measured),
            ("handgonne", Drill::Purse),
            ("knights-blade", Drill::Clean),
        ]
    );
}

#[test]
fn a_kit_with_no_drill_shoots_as_it_always_did() {
    for (kit, arm) in [("percival", Weapon::Bow), ("dispatcher", Weapon::Crossbow), ("gareth", Weapon::Handgonne)] {
        let mut run = wearing(kit);
        hero(&mut run).carried.add(Spoil::Gold, 500);
        let plain = hero(&mut run).scaled(arm.arms().damage);
        for shot in loose(&mut run) {
            assert_eq!(shot.damage, plain, "{kit}");
            assert!(!shot.empowered, "{kit}");
        }
        assert!(hero(&mut run).hero_kit.is_empty(), "{kit}");
    }
}

#[test]
fn the_drill_stays_with_the_kits_own_arm() {
    let mut run = wearing("competition");
    let bow = run.book.get("bow").cloned().unwrap();
    hero(&mut run).equip(&bow);
    assert_eq!(of(&run.players[&1]), None);
    let shot = loose(&mut run);
    assert!(!shot[0].empowered, "a borrowed bow has no measure");
    let crossbow = run.book.get("crossbow").cloned().unwrap();
    hero(&mut run).equip(&crossbow);
    assert_eq!(of(&run.players[&1]), Some(Drill::Measured));
    hero(&mut run).shape = Some(identity::Shape::Square);
    assert_eq!(of(&run.players[&1]), None, "a wished shape leaves it behind");
}

#[test]
fn arrows_without_a_rest_build_the_beat_and_a_rest_loses_it() {
    let mut run = wearing("loop");
    let first = {
        loose(&mut run);
        hero(&mut run).fire_cooldown
    };
    for _ in 0..BEAT_MOST + 3 {
        rest(&mut run, 1);
        loose(&mut run);
    }
    assert_eq!(hero(&mut run).hero_kit.beat, BEAT_MOST, "the beat has a ceiling");
    let quick = hero(&mut run).fire_cooldown;
    assert!(quick < first, "the full beat rearms faster: {quick} against {first}");
    assert_eq!(hero(&mut run).hero_kit.attack_rate(), BEAT_MOST * BEAT_RATE);
    rest(&mut run, BEAT_LIFE - 1);
    assert_eq!(hero(&mut run).hero_kit.beat, BEAT_MOST, "not lost yet");
    rest(&mut run, 1);
    assert_eq!(hero(&mut run).hero_kit.beat, 0, "a rest loses all of it");
}

#[test]
fn a_bolt_after_a_full_measure_strikes_harder_and_a_hurried_one_does_not() {
    let mut run = wearing("competition");
    let plain = hero(&mut run).scaled(Weapon::Crossbow.arms().damage);
    let opener = loose(&mut run);
    assert_eq!(opener[0].damage, plain * MEASURED / 100, "a knight comes in rested");
    assert!(opener[0].empowered);
    rest(&mut run, MEASURE - 1);
    let hurried = loose(&mut run);
    assert_eq!(hurried[0].damage, plain, "one tick short is a plain bolt");
    assert!(!hurried[0].empowered);
    rest(&mut run, MEASURE - 1);
    assert!(!loose(&mut run)[0].empowered, "a hurried bolt starts the measure again");
    rest(&mut run, MEASURE);
    assert_eq!(loose(&mut run)[0].damage, plain * MEASURED / 100);
}

#[test]
fn the_measure_is_worth_waiting_for_but_only_just() {
    let arms = Weapon::Crossbow.arms();
    assert!(MEASURE > arms.cooldown, "holding the trigger never measures");
    let spam = arms.damage as f32 / arms.cooldown as f32;
    let measured = (arms.damage * MEASURED / 100) as f32 / MEASURE as f32;
    assert!(measured > spam && measured < spam * 1.5, "{measured} against {spam}");
}

#[test]
fn the_gold_carried_weighs_the_money_knights_shot_up_to_a_ceiling() {
    let mut run = wearing("money");
    let plain = hero(&mut run).scaled(Weapon::Handgonne.arms().damage);
    assert_eq!(loose(&mut run)[0].damage, plain, "an empty purse is a plain gonne");
    hero(&mut run).carried.add(Spoil::Gold, 100);
    let balls = loose(&mut run);
    assert_eq!(balls.len(), Weapon::Handgonne.arms().shots as usize);
    assert!(balls.iter().all(|b| b.damage == plain * 120 / 100), "every ball");
    hero(&mut run).carried.add(Spoil::Gold, 10_000);
    assert_eq!(loose(&mut run)[0].damage, plain * (100 + PURSE_MOST) / 100);
    assert_eq!(hero(&mut run).carried.get(Spoil::Gold), 10_100, "the purse is weighed, not spent");
}

#[test]
fn the_third_stroke_of_a_chain_is_the_clean_one_and_a_pause_breaks_it() {
    let mut run = wearing("composer");
    let hero_now = hero(&mut run);
    assert_eq!((stroke(hero_now), stroke(hero_now), stroke(hero_now)), (false, false, true));
    assert!(!stroke(hero_now), "the count starts again");
    rest(&mut run, STROKE_LIFE);
    let hero_now = hero(&mut run);
    assert_eq!(hero_now.hero_kit.strokes, 0, "a pause breaks the chain");
    assert_eq!((stroke(hero_now), stroke(hero_now), stroke(hero_now)), (false, false, true));
}

#[test]
fn a_blade_with_no_drill_never_strokes_clean() {
    let mut run = wearing("galahad");
    let hero = hero(&mut run);
    assert!((0..12).all(|_| !stroke(hero)));
    assert!(hero.hero_kit.is_empty());
}

#[test]
fn the_clean_stroke_cuts_behind_the_composer_half_again_as_hard() {
    let mut run = Run::new(21, 1, None);
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.outfit(1, knight("composer").unwrap());
    let hero = hero(&mut run);
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (12.0, 14.0, 1.0, 0.0);
    hero.invulnerable = 1000;
    // One before the blade, one behind where a plain stroke never reaches.
    run.spawn_at_for_test(EnemyKind::Demon, 14.0, 14.0);
    run.spawn_at_for_test(EnemyKind::Demon, 9.5, 14.0);
    let held = 100_000;
    for e in &mut run.enemies {
        (e.hp, e.max_hp, e.seen_hp, e.sleep_until) = (held, held, held, 100_000);
    }
    run.phase = Phase::Fighting;
    let fire = BTreeMap::from([(1, Input { fire: true, ..Default::default() })]);
    let (mut front, mut dealt) = (held, Vec::new());
    for _ in 0..120 {
        run.step_sim_for_test(&fire);
        if run.enemies[0].hp < front {
            dealt.push(front - run.enemies[0].hp);
            front = run.enemies[0].hp;
        }
        if dealt.len() == 3 {
            break;
        }
    }
    assert_eq!(dealt.len(), 3, "three strokes: {dealt:?}");
    assert!(run.enemies[1].hp < held, "the third reached behind him");
    assert_eq!(dealt[0], dealt[1], "the plain strokes match");
    assert_eq!(dealt[2], dealt[0] * CLEAN_HIT / 100, "{dealt:?}");
}

#[test]
fn the_drill_clocks_are_quiet_on_the_wire_and_on_old_saves() {
    let stance: heroes::Stance = serde_json::from_str("{}").unwrap();
    assert!(stance.is_empty());
    let mut stance = heroes::Stance::default();
    stance.tick();
    assert!(stance.is_empty(), "an idle knight sends no stance");
}
