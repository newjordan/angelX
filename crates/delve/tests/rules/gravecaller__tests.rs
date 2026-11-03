use super::*;
use crate::rules::hireling::testing::{fight_with, foe_at};

fn caller() -> Run {
    let mut run = fight_with(false, &[]);
    assert!(run.take_up(1, GRAVECALLER));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (12.0, 12.0, 1.0, 0.0);
    hero.stone = false;
    hero.idle = false;
    run
}

fn post(run: &mut Run, kind: EnemyKind, at: (f32, f32), hp: u32) -> u32 {
    let id = foe_at(run, kind, at, u32::MAX / 2);
    let enemy = run.enemies.last_mut().unwrap();
    enemy.age = TELEGRAPH;
    enemy.hp = hp;
    enemy.max_hp = hp;
    run.phase = Phase::Fighting;
    id
}

fn step(run: &mut Run, input: Input, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::from([(1, input)]));
    }
}

fn gauge(run: &mut Run, percent: u8) {
    run.minions.graves.entry(1).or_default().corpses = percent;
}

fn golems(run: &Run) -> Vec<AllyKind> {
    run.minions.raised.iter().filter(|m| m.kind.golem()).map(|m| m.kind).collect()
}

#[test]
fn aura_pulses_without_input_and_uses_the_crowd_damage_rule() {
    let mut run = caller();
    let ids: Vec<u32> = [13.0, 13.5, 14.0, 14.5, 15.0].into_iter()
        .map(|x| post(&mut run, EnemyKind::Skeleton, (x, 12.0), 1000)).collect();
    let far = post(&mut run, EnemyKind::Skeleton, (25.0, 12.0), 1000);
    let damage = run.players[&1].scaled(AURA_DAMAGE);
    step(&mut run, Input::default(), AURA_EVERY - 1);
    assert!(run.enemies.iter().all(|e| e.hp == 1000));
    step(&mut run, Input::default(), 1);
    for (rank, id) in ids.iter().enumerate() {
        let enemy = run.enemies.iter().find(|e| e.id == *id).unwrap();
        assert_eq!(1000 - enemy.hp, aoe::part(damage, rank));
        assert_eq!(enemy.wits.foe, 1);
    }
    assert_eq!(run.enemies.iter().find(|e| e.id == far).unwrap().hp, 1000);
    let pulse = run.minions.graves[&1].pulse.as_ref().unwrap();
    assert_eq!((pulse.x, pulse.y), (12.0, 12.0));
    step(&mut run, Input::default(), AURA_EVERY);
    assert_eq!(run.enemies.iter().find(|e| e.id == ids[0]).unwrap().hp, 1000 - damage * 2);
}

#[test]
fn aura_reaches_great_foes_like_soul_rip_but_spares_dummies_and_telegraphs() {
    let mut run = caller();
    for kind in [EnemyKind::Boss, EnemyKind::Dragon, EnemyKind::PitTyrant, EnemyKind::Dummy] {
        post(&mut run, kind, (14.0, 12.0), 1000);
    }
    post(&mut run, EnemyKind::Bat, (13.0, 12.0), 1000);
    run.enemies.last_mut().unwrap().age = 0;
    run.grave_pulse(1);
    for enemy in &run.enemies {
        if matches!(enemy.kind, EnemyKind::Dummy | EnemyKind::Bat) {
            assert_eq!(enemy.hp, 1000);
        } else {
            assert!(enemy.hp < 1000);
        }
    }
    run.players.get_mut(&1).unwrap().stone = true;
    run.minions.graves.get_mut(&1).unwrap().aura = 1;
    run.tick_gravecallers(&BTreeMap::new());
    assert_eq!(run.minions.graves[&1].aura, 1);
}

#[test]
fn settled_bodies_fill_only_nearby_or_credited_callers_once_and_cap_at_full() {
    let mut run = caller();
    post(&mut run, EnemyKind::Skeleton, (28.0, 12.0), 0);
    post(&mut run, EnemyKind::Skeleton, (35.0, 12.0), 0);
    post(&mut run, EnemyKind::Skeleton, (36.0, 12.0), 0);
    run.enemies.last_mut().unwrap().wits.foe = 1;
    post(&mut run, EnemyKind::Dummy, (12.0, 12.0), 0);
    run.settle_kills();
    assert_eq!(run.minions.graves[&1].corpses, 16);
    run.settle_kills();
    assert_eq!(run.minions.graves[&1].corpses, 16);
    for _ in 0..20 {
        post(&mut run, EnemyKind::Skeleton, (15.0, 12.0), 0);
    }
    run.settle_kills();
    assert_eq!(run.minions.graves[&1].corpses, 100);
}

#[test]
fn a_remote_minion_kill_credits_its_caller_and_gathers_the_body() {
    let mut run = caller();
    post(&mut run, EnemyKind::Skeleton, (35.0, 12.0), 1);
    run.raise(1, AllyKind::Ghoul, 0);
    let ghoul = &mut run.minions.raised[0];
    (ghoul.x, ghoul.y, ghoul.rising, ghoul.cooldown) = (34.5, 12.0, 0, 0);
    run.tick_minions();
    assert_eq!(run.enemies[0].hp, 0);
    assert_eq!(run.enemies[0].wits.foe, 1);
    run.settle_kills();
    assert_eq!(run.minions.graves[&1].corpses, CORPSE_GAIN);
}

#[test]
fn every_golem_boundary_replaces_only_the_golem_and_spends_the_bar() {
    let mut run = caller();
    run.raise(1, AllyKind::Ghoul, 0);
    run.raise(1, AllyKind::Warden, 0);
    run.raise(1, AllyKind::Totem, 0);
    for (percent, kind) in [(1, AllyKind::FleshGolem), (33, AllyKind::FleshGolem),
        (34, AllyKind::BoneGolem), (66, AllyKind::BoneGolem),
        (67, AllyKind::GraveGolem), (99, AllyKind::GraveGolem), (100, AllyKind::Colossus)] {
        gauge(&mut run, percent);
        run.release_golem(1);
        assert_eq!(golems(&run), [kind]);
        assert_eq!(run.minions.graves[&1].corpses, 0);
        assert_eq!(run.minions.raised.iter().filter(|m| m.kind.kept()).count(), KEPT);
        assert!(run.minions.raised.iter().all(|m| m.summoner == Some(1)));
    }
    run.raise(1, AllyKind::Ghoul, 0);
    assert_eq!(golems(&run), [AllyKind::Colossus]);
    assert_eq!(run.minions.raised.iter().filter(|m| m.kind.kept()).count(), KEPT);
    let tiers = [AllyKind::FleshGolem, AllyKind::BoneGolem, AllyKind::GraveGolem, AllyKind::Colossus];
    for pair in tiers.windows(2) {
        assert!(pair[1].health() > pair[0].health());
        assert!(pair[1].blow().unwrap().0 > pair[0].blow().unwrap().0);
    }
    assert!(run.valid_snapshot());
}

#[test]
fn holding_g_releases_once_and_tapping_g_still_commands_the_dead() {
    let mut run = caller();
    post(&mut run, EnemyKind::Skeleton, (40.0, 12.0), 1000);
    run.raise(1, AllyKind::Ghoul, 0);
    gauge(&mut run, 66);
    let hold = Input { vigil: true, ..Default::default() };
    step(&mut run, hold, 1);
    assert_eq!(run.minions.order(1), Order::Heel);
    assert!(golems(&run).is_empty());
    step(&mut run, hold, GOLEM_HOLD - 1);
    assert_eq!(golems(&run), [AllyKind::BoneGolem]);
    gauge(&mut run, 100);
    step(&mut run, hold, GOLEM_HOLD + 2);
    assert_eq!(golems(&run), [AllyKind::BoneGolem]);
    assert_eq!(run.minions.graves[&1].corpses, 100);
    step(&mut run, Input::default(), 1);
    step(&mut run, hold, GOLEM_HOLD);
    assert_eq!(golems(&run), [AllyKind::Colossus]);
    assert_eq!(run.minions.order(1), Order::Hunt);
}

#[test]
fn golem_obeys_heel_and_follows_its_caller_through_rooms() {
    let mut run = caller();
    gauge(&mut run, 100);
    run.release_golem(1);
    run.minions.orders.insert(1, Order::Heel);
    post(&mut run, EnemyKind::Skeleton, (35.0, 12.0), 1000);
    run.minions.raised[0].rising = 0;
    for _ in 0..90 { run.tick_minions(); }
    let golem = &run.minions.raised[0];
    assert!((golem.x - 12.0).hypot(golem.y - 12.0) < HEEL_GUARD);
    assert_eq!(run.enemies[0].hp, 1000);
    run.players.get_mut(&1).unwrap().x = 25.0;
    run.minions_follow();
    assert_eq!(golems(&run), [AllyKind::Colossus]);
    assert!((run.minions.raised[0].x - 25.0).abs() < 3.0);
}

#[test]
fn guard_keeps_the_roll_and_raises_five_in_a_wedge_with_a_cooldown() {
    let mut run = caller();
    post(&mut run, EnemyKind::Skeleton, (40.0, 12.0), 1000);
    step(&mut run, Input { dash: true, aim_x: 1, move_x: 1, ..Default::default() }, 1);
    assert!(run.players[&1].dash_cooldown > 0, "the dodge still works");
    assert_eq!(run.minions.raised.len(), WALL_COUNT);
    let wall = &run.minions.raised;
    assert!(wall.iter().all(|m| m.kind == AllyKind::Skeleton));
    assert!(wall[0].x > wall[1].x && wall[1].x > wall[3].x);
    assert!((wall[1].x - wall[2].x).abs() < 0.01);
    assert!((wall[3].x - wall[4].x).abs() < 0.01);
    assert!(wall[1].y < wall[0].y && wall[2].y > wall[0].y);
    assert!(wall[3].y < wall[1].y && wall[4].y > wall[2].y);
    assert_eq!(run.minions.graves[&1].wall, WALL_TICKS);
    step(&mut run, Input::default(), WALL_TICKS);
    assert!(!run.minions.raised.iter().any(|m| m.kind == AllyKind::Skeleton));
    step(&mut run, Input { dash: true, ..Default::default() }, 1);
    assert!(!run.minions.raised.iter().any(|m| m.kind == AllyKind::Skeleton));
    step(&mut run, Input::default(), WALL_REARM);
    step(&mut run, Input { dash: true, ..Default::default() }, 1);
    assert_eq!(run.minions.raised.len(), WALL_COUNT);
}

#[test]
fn wall_walks_and_turns_with_aim_and_stays_on_open_floor() {
    let mut run = caller();
    run.skeleton_wall(1);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (16.0, 16.0, 0.0, 1.0);
    for _ in 0..30 { run.walk_skeletons(); }
    let wall = &run.minions.raised;
    assert!(wall[0].y > wall[1].y && wall[1].y > wall[3].y);
    assert!((wall[0].x - 16.0).abs() < 0.01);
    assert!(wall[1].x > 16.0 && wall[2].x < 16.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (1.5, 1.5, -1.0, 0.0);
    for _ in 0..40 { run.walk_skeletons(); }
    let grid = Grid { room: &run.dungeon.rooms[run.at], barred: true };
    assert!(run.minions.raised.iter().all(|m| grid.clear(m.x, m.y, HERO_RADIUS, Mover::Hero)));
}

fn shot(at: (f32, f32), velocity: (f32, f32)) -> Projectile {
    Projectile { x: at.0, y: at.1, vx: velocity.0, vy: velocity.1, hostile: true,
        look: None, kind: Shot::Orb, damage: 20, pierce: 0, last_hit: None,
        empowered: false, traits: Default::default(), ttl: HZ }
}

#[test]
fn skeletons_intercept_fast_shots_before_the_caller_and_take_each_hit() {
    let mut run = caller();
    post(&mut run, EnemyKind::Skeleton, (40.0, 12.0), 1000);
    run.skeleton_wall(1);
    let hp = run.players[&1].hp;
    for _ in 0..3 {
        run.projectiles.push(shot((19.0, 12.0), (-240.0, 0.0)));
    }
    step(&mut run, Input::default(), 1);
    assert_eq!(run.players[&1].hp, hp);
    assert_eq!(run.minions.raised[0].hp, 70);
    assert!(!run.projectiles.iter().any(|p| p.hostile && p.ttl > 0));
    // A shot from behind reaches the caller before any of the wall.
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    run.projectiles.push(shot((9.0, 12.0), (240.0, 0.0)));
    step(&mut run, Input::default(), 1);
    assert!(run.players[&1].hp < hp);
}

#[test]
fn wall_blocks_enemy_bodies_takes_blows_and_crumbling_guards_leave_gaps() {
    let mut run = caller();
    run.skeleton_wall(1);
    let (x, y) = (run.minions.raised[0].x, run.minions.raised[0].y);
    post(&mut run, EnemyKind::Skeleton, (x + 0.2, y), 1000);
    run.enemies[0].frozen = 0;
    run.skeletons_block();
    assert!(run.minions.raised[0].hp < 100);
    assert!((run.enemies[0].x - x).hypot(run.enemies[0].y - y) >= WALL_RADIUS);
    run.minions.raised[0].hp = 0;
    run.walk_skeletons();
    assert_eq!(run.minions.raised.len(), WALL_COUNT - 1);
    assert!(run.minions.raised.iter().all(|m| m.power != 0));
}

#[test]
fn gauge_wall_and_golem_survive_save_and_wire_and_old_saves_start_empty() {
    let mut run = caller();
    gauge(&mut run, 100);
    run.release_golem(1);
    run.skeleton_wall(1);
    gauge(&mut run, 56);
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert_eq!(hud.corpses.unwrap().state, "56%");
    assert_eq!(hud.skeleton_wall.unwrap().state, "10s up");
    let saved = serde_json::to_string(&run).unwrap();
    let back: Run = serde_json::from_str(&saved).unwrap();
    assert_eq!(back.minions, run.minions);
    let guest = crate::wire::read(&crate::wire::publish(&run).unwrap()).unwrap();
    assert_eq!(guest.minions, run.minions);
    assert!(guest.valid_snapshot());
    let old: Pack = serde_json::from_str("{}").unwrap();
    assert!(old.graves.is_empty());
    let old: Grave = serde_json::from_str("{}").unwrap();
    assert_eq!(old.corpses, 0);
    assert_eq!(old.aura, AURA_EVERY);
    run.minions_follow();
    assert_eq!(golems(&run), [AllyKind::Colossus]);
    assert!(!run.minions.raised.iter().any(|m| m.kind == AllyKind::Skeleton));
    assert_eq!(run.minions.graves[&1].corpses, 56);
}

#[test]
fn the_new_skills_belong_only_to_gravecallers() {
    let mut run = caller();
    assert!(run.take_up(1, "percival"));
    step(&mut run, Input { dash: true, vigil: true, ..Default::default() }, GOLEM_HOLD + 1);
    assert!(run.minions.graves.is_empty());
    assert!(run.minions.raised.is_empty());
    assert!(run.hud(1, &hud::Keys::FIRST).unwrap().corpses.is_none());
}

#[test]
fn skeleton_wall_stops_a_beam_before_it_burns_the_caller() {
    let mut run = caller();
    post(&mut run, EnemyKind::Skeleton, (40.0, 12.0), 1000);
    run.skeleton_wall(1);
    let hp = run.players[&1].hp;
    let mut beam = shot((22.0, 12.0), (0.0, 0.0));
    beam.traits.path.beam = Some(patterns::Beam { angle: std::f32::consts::PI,
        reach: 30.0, warn: 0, burn: 10, ..Default::default() });
    run.projectiles.push(beam);
    step(&mut run, Input::default(), 1);
    assert_eq!(run.players[&1].hp, hp);
    assert!(run.minions.raised[0].hp < 100);
    assert!(run.projectiles[0].traits.path.beam.unwrap().reach < 8.0);
}

#[test]
fn two_callers_keep_their_own_gauges_golems_and_walls() {
    let mut run = caller();
    run.join_seat(2, "Morrow");
    assert!(run.take_up(2, GRAVECALLER));
    let second = run.players.get_mut(&2).unwrap();
    (second.x, second.y, second.stone, second.idle) = (35.0, 12.0, false, false);
    post(&mut run, EnemyKind::Skeleton, (14.0, 12.0), 0);
    run.settle_kills();
    assert_eq!(run.minions.graves[&1].corpses, 8);
    assert!(run.minions.graves.get(&2).is_none_or(|g| g.corpses == 0));
    gauge(&mut run, 100);
    run.minions.graves.entry(2).or_default().corpses = 34;
    run.release_golem(1);
    run.release_golem(2);
    run.skeleton_wall(1);
    run.skeleton_wall(2);
    gauge(&mut run, 67);
    run.release_golem(1);
    for (id, kind) in [(1, AllyKind::GraveGolem), (2, AllyKind::BoneGolem)] {
        let mine: Vec<_> = run.minions.raised.iter().filter(|m| m.summoner == Some(id)).collect();
        assert_eq!(mine.iter().filter(|m| m.kind == AllyKind::Skeleton).count(), 5);
        assert_eq!(mine.iter().filter(|m| m.kind.golem()).map(|m| m.kind).collect::<Vec<_>>(), [kind]);
    }
    assert!(run.valid_snapshot());
}

#[test]
fn golem_tiers_land_their_own_blows_but_keep_the_minions_great_foe_rule() {
    for (percent, weight) in [(1, 20), (34, 34), (67, 52), (100, 90)] {
        let mut run = caller();
        post(&mut run, EnemyKind::Skeleton, (14.0, 12.0), 1000);
        gauge(&mut run, percent);
        run.release_golem(1);
        let minion = &mut run.minions.raised[0];
        (minion.x, minion.y, minion.rising) = (13.5, 12.0, 0);
        run.tick_minions();
        assert_eq!(run.enemies[0].hp, 1000 - weight);
        assert_eq!(run.enemies[0].wits.foe, 1);
        run.enemies[0].kind = EnemyKind::Dragon;
        run.minions.raised[0].cooldown = 0;
        run.tick_minions();
        assert_eq!(run.enemies[0].hp, 1000 - weight);
    }
}

#[test]
fn a_calm_vigil_can_release_bodies_but_an_empty_bar_raises_nothing() {
    let mut run = caller();
    run.phase = Phase::Exploring;
    run.dungeon.rooms[run.at].cleared = true;
    gauge(&mut run, 33);
    step(&mut run, Input { vigil: true, ..Default::default() }, GOLEM_HOLD);
    assert!(run.players[&1].vigil);
    assert_eq!(golems(&run), [AllyKind::FleshGolem]);
    let mut run = caller();
    run.release_golem(1);
    assert!(golems(&run).is_empty());
}
