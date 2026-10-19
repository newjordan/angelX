use super::*;
use std::collections::BTreeMap;

/// A calm, empty fight with knight 1 standing at (24, 14), aiming `aim`,
/// a kite shield on the guard key, at full health with no grace.
fn guarded(aim: (f32, f32)) -> Run {
    let mut run = Run::new(1, 1, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.window = None;
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.guard = cards::Guard::Shield;
    let (ax, ay) = unit(aim.0, aim.1);
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (24.0, 14.0, ax, ay);
    (hero.hp, hero.invulnerable) = (hero.max_hp, 0);
    run
}

fn keys(raised: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { dash: raised, ..Default::default() })])
}

fn monster_shot(kind: Shot, at: (f32, f32), velocity: (f32, f32), damage: u32) -> Projectile {
    Projectile {
        x: at.0,
        y: at.1,
        vx: velocity.0,
        vy: velocity.1,
        hostile: true,
        look: None,
        kind,
        damage,
        pierce: if kind == Shot::Frost { 2 } else { 0 },
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 3 * HZ,
    }
}

/// What came of one shot: the blocks it left (counted the tick each was
/// new), the health lost, and the last place it was seen in flight.
struct Outcome {
    blocks: Vec<feel::Block>,
    lost: u32,
    last: (f32, f32),
}

/// Fly `shot` at knight 1 until it is gone, the shield raised or not,
/// keeping the knight where they stand.
fn fly(run: &mut Run, shot: Projectile, raised: bool) -> Outcome {
    let full = run.players[&1].hp;
    let mut last = (shot.x, shot.y);
    run.projectiles.push(shot);
    let mut blocks = Vec::new();
    for _ in 0..3 * HZ {
        if let Some(p) = run.projectiles.iter().find(|p| p.hostile) {
            last = (p.x, p.y);
        } else {
            break;
        }
        let at = (run.players[&1].x, run.players[&1].y);
        run.step(&keys(raised));
        blocks.extend(run.feel.blocks.iter().filter(|b| b.age() == 0).copied());
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (at.0, at.1, 0);
    }
    Outcome {
        blocks,
        lost: full - run.players[&1].hp,
        last,
    }
}

/// A shot coming in toward the knight's middle from `bearing` degrees off
/// the aim, `offset` across its line.
fn incoming(run: &Run, bearing: f32, offset: f32, speed: f32) -> ((f32, f32), (f32, f32)) {
    let hero = &run.players[&1];
    let base = hero.aim_y.atan2(hero.aim_x) + bearing.to_radians();
    let (ux, uy) = (base.cos(), base.sin());
    let (px, py) = (-uy, ux);
    let start = (hero.x + ux * 5.0 + px * offset, hero.y + uy * 5.0 + py * offset);
    (start, (-ux * speed, -uy * speed))
}

#[test]
fn a_shot_from_the_front_dies_at_the_shield_face() {
    for aim in [(1.0, 0.0), (0.0, 1.0), (-1.0, -1.0), (0.0, -1.0)] {
        for bearing in [0.0, 35.0, -35.0, 60.0, -60.0] {
            for speed in [8.0, 14.0, 22.0] {
                let mut run = guarded(aim);
                let (at, v) = incoming(&run, bearing, 0.0, speed);
                let out = fly(&mut run, monster_shot(Shot::Ember, at, v, 12), true);
                let hero = &run.players[&1];
                let c = (hero.x, hero.y);
                let case = format!("aim {aim:?} bearing {bearing} speed {speed}");
                assert_eq!(out.lost, 0, "{case}: no harm through the shield");
                assert_eq!(out.blocks.len(), 1, "{case}: one block");
                let block = out.blocks[0];
                let (bx, by) = (block.x - c.0, block.y - c.1);
                let d = bx.hypot(by);
                assert!((d - shield::SHIELD_REACH).abs() < 0.02, "{case}: stopped at the face ({d})");
                assert!(bx * hero.aim_x + by * hero.aim_y > 0.0, "{case}: in front");
                // In flight it was never inside the knight.
                let seen = (out.last.0 - c.0).hypot(out.last.1 - c.1);
                assert!(seen > HERO_RADIUS + 0.16, "{case}: last seen {seen} from the middle");
                assert!(
                    (out.last.0 - c.0) * hero.aim_x + (out.last.1 - c.1) * hero.aim_y > 0.0,
                    "{case}: last seen in front"
                );
            }
        }
    }
}

#[test]
fn a_shot_from_behind_or_past_the_arc_still_hits() {
    for bearing in [180.0, 150.0, -150.0, 110.0, -110.0] {
        let mut run = guarded((1.0, 0.0));
        let (at, v) = incoming(&run, bearing, 0.0, 10.0);
        let out = fly(&mut run, monster_shot(Shot::Ember, at, v, 12), true);
        assert!(out.lost > 0, "bearing {bearing}: struck");
        assert!(out.blocks.is_empty(), "bearing {bearing}: nothing blocked");
    }
}

#[test]
fn a_shield_down_blocks_nothing() {
    for bearing in [0.0, 30.0, -60.0] {
        let mut run = guarded((1.0, 0.0));
        let (at, v) = incoming(&run, bearing, 0.0, 10.0);
        let out = fly(&mut run, monster_shot(Shot::Ember, at, v, 12), false);
        assert!(out.lost > 0, "bearing {bearing}: struck with the shield down");
        assert!(out.blocks.is_empty());
    }
}

#[test]
fn every_kind_of_monster_shot_is_stopped_at_the_face() {
    let kinds = [
        Shot::Bone, Shot::Orb, Shot::Ember, Shot::Hex, Shot::Frost, Shot::Missile, Shot::Wisp,
        Shot::Ore, Shot::Flame, Shot::Page, Shot::Spore, Shot::Eye, Shot::Arrow, Shot::Bolt,
        Shot::Ball,
    ];
    for kind in kinds {
        let mut run = guarded((0.0, 1.0));
        let (at, v) = incoming(&run, 20.0, 0.0, 12.0);
        let out = fly(&mut run, monster_shot(kind, at, v, 14), true);
        let hero = &run.players[&1];
        assert_eq!(out.lost, 0, "{kind:?}");
        assert!(!hero.frog() && hero.chilled == 0, "{kind:?}: no hex, no chill");
        assert_eq!(out.blocks.len(), 1, "{kind:?}");
        assert_eq!(out.blocks[0].kind, kind);
        assert!(run.projectiles.iter().all(|p| !p.hostile), "{kind:?}: no Rimeleap off a shield");
    }
}

#[test]
fn a_raised_shield_never_lets_through_what_it_stopped_before() {
    // Before, a shot was turned when it reached the body coming within the
    // arc of the aim. Every such shot is still turned, wherever it meets
    // the shield, and wherever across the body it was going.
    let cos = shield::SHIELD_ARC.to_radians().cos();
    let mut cases = 0;
    for aim in [(1.0, 0.0), (1.0, 1.0), (0.0, -1.0)] {
        for step in -16..=16 {
            let bearing = step as f32 * 4.0;
            for offset in [-0.5, -0.25, 0.0, 0.25, 0.5] {
                let mut run = guarded(aim);
                let (at, v) = incoming(&run, bearing, offset, 11.0);
                let hero = &run.players[&1];
                let speed = v.0.hypot(v.1);
                let facing = -(v.0 * hero.aim_x + v.1 * hero.aim_y) / speed;
                let reaches = hit_segment(
                    at.0,
                    at.1,
                    at.0 + v.0,
                    at.1 + v.1,
                    hero.x,
                    hero.y,
                    HERO_RADIUS + 0.16,
                );
                if facing < cos || !reaches {
                    continue;
                }
                cases += 1;
                let out = fly(&mut run, monster_shot(Shot::Bone, at, v, 12), true);
                assert_eq!(out.lost, 0, "aim {aim:?} bearing {bearing} offset {offset}");
            }
        }
    }
    assert!(cases > 80, "{cases}");
}

#[test]
fn the_block_is_left_once_for_each_shot_stopped() {
    let mut run = guarded((-1.0, 0.0));
    let mut blocks = 0;
    let mut clangs = 0;
    for n in 0..5 {
        let (at, v) = incoming(&run, (n as f32 - 2.0) * 20.0, 0.0, 12.0);
        run.projectiles.push(monster_shot(Shot::Wisp, at, v, 12));
        for _ in 0..HZ {
            run.sounds.clear();
            run.step(&keys(true));
            blocks += run.feel.blocks.iter().filter(|b| b.age() == 0).count();
            clangs += run.sounds.iter().filter(|&&s| s == "shield_block").count();
            run.players.get_mut(&1).unwrap().x = 24.0;
        }
    }
    assert_eq!(blocks, 5);
    assert_eq!(clangs, 5);
    assert_eq!(run.players[&1].hp, run.players[&1].max_hp);
    // And then it fades.
    for _ in 0..feel::BLOCK_TICKS {
        run.step(&keys(true));
    }
    assert!(run.feel.blocks.is_empty());
}

#[test]
fn a_heavier_shot_is_a_heavier_block() {
    let light = monster_shot(Shot::Wisp, (0.0, 0.0), (1.0, 0.0), 10);
    let mid = monster_shot(Shot::Ore, (0.0, 0.0), (1.0, 0.0), 16);
    let heavy = monster_shot(Shot::Bone, (0.0, 0.0), (1.0, 0.0), 30);
    assert_eq!(
        [shield::weight(&light), shield::weight(&mid), shield::weight(&heavy)],
        [1, 2, 3]
    );
    let mut run = guarded((1.0, 0.0));
    let (at, v) = incoming(&run, 0.0, 0.0, 12.0);
    let out = fly(&mut run, monster_shot(Shot::Bone, at, v, 30), true);
    assert_eq!(out.blocks[0].weight, 3);
}

#[test]
fn a_beam_stops_at_a_raised_shield_and_spares_the_knight_behind() {
    let mut run = guarded((0.0, -1.0));
    run.join_seat(2, "Behind");
    let behind = run.players.get_mut(&2).unwrap();
    (behind.x, behind.y, behind.invulnerable) = (24.0, 16.5, 0);
    let grid = Grid { room: run.room(), barred: false };
    let aim = patterns::Aim {
        from: (24.0, 4.0),
        at: (24.0, 14.0),
        pace: (0.0, 0.0),
        grid: &grid,
        count: 0,
        salt: 0x5eed,
        step: 0,
        kind: Shot::Eye,
        damage: 20,
        guardian: None,
    };
    let mut shots = Vec::new();
    patterns::emit(patterns::Emitter::Laser, &aim, &mut shots);
    assert_eq!(shots.len(), 1);
    run.projectiles = shots;
    let mut blocks = 0;
    let mut reach = f32::MAX;
    for _ in 0..u32::from(patterns::BEAM_WARN) + u32::from(patterns::BEAM_BURN) + 2 {
        run.step(&BTreeMap::from([(1, Input { dash: true, ..Default::default() }), (2, Input::default())]));
        blocks += run.feel.blocks.iter().filter(|b| b.age() == 0).count();
        if let Some(beam) = run.projectiles.iter().find_map(|p| p.traits.path.beam)
            && beam.burning(run.projectiles[0].traits.path.age)
        {
            reach = reach.min(beam.reach);
        }
        for (id, y) in [(1, 14.0), (2, 16.5)] {
            let hero = run.players.get_mut(&id).unwrap();
            (hero.x, hero.y, hero.invulnerable) = (24.0, y, 0);
        }
    }
    assert_eq!(run.players[&1].hp, run.players[&1].max_hp, "the shield takes the beam");
    assert_eq!(run.players[&2].hp, run.players[&2].max_hp, "and nobody behind it is touched");
    assert_eq!(blocks, 1, "one block for the beam");
    assert!((reach - (10.0 - shield::SHIELD_REACH)).abs() < 0.3, "the beam ends at the face: {reach}");
}

#[test]
fn every_path_a_shot_takes_asks_the_shield_the_same() {
    // The shots' loop, the beams and the bash all go through `shield`; no
    // copy of the facing rule is left anywhere else.
    for (name, source) in [
        ("rules.rs", include_str!("../../src/rules.rs")),
        ("patterns.rs", include_str!("../../src/rules/patterns.rs")),
        ("critters.rs", include_str!("../../src/rules/critters.rs")),
        ("hireling.rs", include_str!("../../src/rules/hireling.rs")),
    ] {
        assert!(!source.contains("SHIELD_ARC"), "{name} keeps its own arc");
        assert!(!source.contains("hero.shielding &&"), "{name} keeps its own facing rule");
    }
    assert!(include_str!("../../src/rules.rs").contains("shield::meets("));
    assert!(include_str!("../../src/rules/patterns.rs").contains("shield::turns("));
    assert!(include_str!("../../src/rules/critters.rs").contains("shield::turns("));
    // And the two questions agree: what `turns` faces, `meets` stops.
    let mut hero = Hero::new("Knight", 24.0, 14.0);
    (hero.aim_x, hero.aim_y, hero.shielding) = (1.0, 0.0, true);
    for step in -20..=20 {
        let a = step as f32 * 0.15;
        let way = (-a.cos(), -a.sin());
        let from = (24.0 - way.0 * 3.0, 14.0 - way.1 * 3.0);
        let to = (24.0 + way.0 * 0.5, 14.0 + way.1 * 0.5);
        let met = shield::meets(&hero, (24.0, 14.0), from, to, HERO_RADIUS + 0.16).unwrap();
        assert_eq!(met.shield, shield::turns(&hero, way), "{a}");
    }
}
