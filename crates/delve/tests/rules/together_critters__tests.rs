use super::*;
use std::collections::BTreeMap;

fn tick(run: &mut Run, input: Input) {
    run.step(&BTreeMap::from([(1, input)]));
}

fn fight(run: &mut Run) {
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
}

/// A calm, cleared room, the knight in the middle, nothing else.
fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.hp = 100_000;
    hero.max_hp = 100_000;
    run
}

/// A critter put in the quiet room, past its spawn telegraph.
fn put(run: &mut Run, kind: EnemyKind, x: f32, y: f32) -> usize {
    run.spawn_at_for_test(kind, x, y);
    let enemy = run.enemies.last_mut().unwrap();
    enemy.age = TELEGRAPH + 5;
    enemy.hp = enemy.max_hp;
    enemy.seen_hp = enemy.max_hp;
    run.enemies.len() - 1
}

fn frame(run: &Run) -> crate::render::overworld::Img {
    crate::render::overworld::arena::frame(run, 384, 224)
}

fn save(img: &crate::render::overworld::Img, name: &str) {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

const KINDS: [EnemyKind; 11] = [
    EnemyKind::Biter,
    EnemyKind::Shard,
    EnemyKind::Mole,
    EnemyKind::Pavise,
    EnemyKind::Leech,
    EnemyKind::Marksman,
    EnemyKind::Mirrorling,
    EnemyKind::Spitter,
    EnemyKind::Ringer,
    EnemyKind::Geode,
    EnemyKind::Hound,
];

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_critter_shots -- --ignored`
#[test]
#[ignore]
fn write_critter_shots() {
    for (name, stage, timer) in [
        ("idle", 0u8, 0u32),
        ("tell", 1, 12),
        ("act", 2, 6),
        ("after", 3, 10),
    ] {
        let mut run = quiet();
        run.players.get_mut(&1).unwrap().x = 4.0;
        run.players.get_mut(&1).unwrap().y = 24.0;
        for (i, kind) in KINDS.iter().enumerate() {
            let (x, y) = (7.0 + (i % 6) as f32 * 7.2, 6.0 + (i / 6) as f32 * 8.0);
            let k = put(&mut run, *kind, x, y);
            let e = &mut run.enemies[k];
            e.stage = stage;
            e.timer = timer;
            e.dir = (1.0, 0.0);
            if *kind == EnemyKind::Spitter {
                e.dir = (x - 4.0, y + 4.0);
            }
        }
        save(&frame(&run), &format!("critters_{name}"));
    }
}

/// Every critter at every stage on black paper, two animation frames each.
#[test]
#[ignore]
fn write_critter_sheet() {
    use crate::render::overworld::Img;
    use crate::render::overworld::arena::critter_figure;
    let stages = [(0u8, 0u32), (1, 12), (2, 6), (3, 10), (4, 10)];
    let (cw, ch) = (22, 22);
    let mut sheet = Img::black(cw * stages.len() as i32 * 2 + 4, ch * KINDS.len() as i32 + 4);
    for (row, kind) in KINDS.iter().enumerate() {
        for (col, (stage, timer)) in stages.iter().enumerate() {
            for frame in 0..2 {
                let mut stage = *stage;
                if *kind == EnemyKind::Mirrorling && stage == 1 {
                    stage = 1 | 8;
                }
                let im = critter_figure(*kind, stage, *timer, frame * 4);
                let x = 2 + (col as i32 * 2 + frame as i32) * cw + (cw - im.w) / 2;
                let y = 2 + row as i32 * ch + (ch - im.h);
                sheet.stamp(&im, x, y);
            }
        }
    }
    save(&sheet, "critter_sheet");
}

/// Step the run `n` ticks with no input, noting the first enemy's stage and
/// place each tick.
fn watch(run: &mut Run, n: u32) -> Vec<(u8, (f32, f32))> {
    let mut seen = Vec::new();
    for _ in 0..n {
        run.step(&BTreeMap::new());
        let e = &run.enemies[0];
        seen.push((e.stage, (e.x, e.y)));
    }
    seen
}

/// How many ticks the first enemy spent in `stage`, over `seen`.
fn spent(seen: &[(u8, (f32, f32))], stage: u8) -> u32 {
    seen.iter().filter(|s| s.0 == stage).count() as u32
}

fn first(seen: &[(u8, (f32, f32))], stage: u8) -> Option<usize> {
    seen.iter().position(|s| s.0 == stage)
}

/// A knight standing still, a critter `dx` off on its left, with open floor
/// between them: the first long clear row of the room.
fn duel(kind: EnemyKind, dx: f32) -> Run {
    let mut run = quiet();
    let room = run.room();
    let need = (dx / TILE_UNITS) as i32 + 4;
    let lane = (2..room.rows as i32 - 2)
        .flat_map(|row| (1..room.cols as i32 - need).map(move |col| (col, row)))
        .find(|&(col, row)| (0..need).all(|k| room.tile(col + k, row) == Tile::Floor))
        .expect("a clear lane");
    let y = (lane.1 as f32 + 0.5) * TILE_UNITS;
    let hx = (lane.0 + need - 1) as f32 * TILE_UNITS;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (hx, y);
    put(&mut run, kind, hx - dx, y);
    run
}

fn shot(x: f32, y: f32, vx: f32, vy: f32, hostile: bool) -> Projectile {
    Projectile {
        x,
        y,
        vx,
        vy,
        hostile,
        kind: Shot::Arrow,
        look: None,
        damage: 20,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 4 * HZ,
    }
}

#[test]
fn a_biter_shivers_before_it_pounces_and_the_pounce_is_a_line() {
    let mut run = duel(EnemyKind::Biter, 6.0);
    let seen = watch(&mut run, 260);
    let crouch = first(&seen, 1).expect("it crouches");
    let pounce = first(&seen, 2).expect("then pounces");
    assert!(crouch < pounce);
    assert!(spent(&seen[..pounce], 1) >= BITER_CROUCH, "the whole shiver first");
    let (a, b) = (seen[pounce].1, seen[pounce + 6].1);
    assert!((a.0 - b.0).hypot(a.1 - b.1) > 1.0, "it flew");
}

#[test]
fn a_room_that_starts_with_a_biter_has_a_swarm_of_four() {
    let mut run = quiet();
    run.spawn_at_for_test(EnemyKind::Biter, 10.0, 8.0);
    run.swarm_up();
    assert_eq!(
        run.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Biter)
            .count(),
        4
    );
}

#[test]
fn a_mole_is_untouchable_underground_and_bursts_after_its_crack() {
    let mut run = duel(EnemyKind::Mole, 9.0);
    // Buried: a knight's arrow flies on, and the touch does nothing.
    run.enemies[0].stage = 1;
    let (x, y) = (run.enemies[0].x, run.enemies[0].y);
    run.projectiles.push(shot(x - 0.1, y, 4.0, 0.0, false));
    let hp = run.enemies[0].hp;
    run.step(&BTreeMap::new());
    assert!(run.enemies[0].buried());
    assert_eq!(run.enemies[0].hp, hp, "the arrow passed over");
    // Let it dig, crack and burst under the standing knight.
    let before = run.players[&1].hp;
    let seen = watch(&mut run, 200);
    let tell = first(&seen, 2).expect("the crack opens");
    assert!(
        spent(&seen, 2) >= MOLE_TELL,
        "the crack is shown for its whole turn"
    );
    assert!(seen[tell + MOLE_TELL as usize + 1].0 == 0, "then it is up");
    assert!(run.players[&1].hp < before, "and the burst hurt");
}

#[test]
fn a_mole_burst_misses_a_knight_who_moved_away() {
    let mut run = duel(EnemyKind::Mole, 9.0);
    run.enemies[0].stage = 2;
    run.enemies[0].timer = 1;
    let before = run.players[&1].hp;
    run.players.get_mut(&1).unwrap().x = 40.0;
    run.players.get_mut(&1).unwrap().y = 24.0;
    for _ in 0..MOLE_TELL + 4 {
        run.step(&BTreeMap::new());
    }
    // Health is held to the knight's cap, never cut by the burst.
    assert_eq!(run.players[&1].hp, before.min(run.players[&1].hp_cap()));
}

#[test]
fn a_pavise_turns_shots_from_the_front_and_not_from_behind() {
    let mut run = duel(EnemyKind::Pavise, 8.0);
    // It faces the knight on its right.
    run.step(&BTreeMap::new());
    let (x, y, hp) = (run.enemies[0].x, run.enemies[0].y, run.enemies[0].hp);
    assert!(run.enemies[0].dir.0 > 0.5);
    run.projectiles.push(shot(x + 1.0, y, -6.0, 0.0, false));
    run.step(&BTreeMap::new());
    assert_eq!(run.enemies[0].hp, hp, "turned by the shield");
    assert!(run.sounds.contains(&"shield_block"));
    run.projectiles.clear();
    let (x, y) = (run.enemies[0].x, run.enemies[0].y);
    run.projectiles.push(shot(x - 1.0, y, 6.0, 0.0, false));
    run.step(&BTreeMap::new());
    assert!(run.enemies[0].hp < hp, "from behind it hurts");
}

#[test]
fn a_pavise_winds_up_its_bash_and_drops_its_shield_after() {
    let mut run = duel(EnemyKind::Pavise, 3.0);
    let before = run.players[&1].hp;
    let seen = watch(&mut run, 140);
    let wind = first(&seen, 1).expect("it winds up");
    let bash = first(&seen, 2).expect("then bashes");
    assert!(spent(&seen[..bash], 1) >= PAVISE_WINDUP);
    assert!(run.players[&1].hp < before, "the bash landed");
    assert!(wind < bash);
    assert!(first(&seen, 3).is_some(), "and lowers it");
    let mut lowered = run.enemies[0].clone();
    lowered.stage = 3;
    lowered.dir = (1.0, 0.0);
    assert!(!blocks(&lowered, -5.0, 0.0), "a lowered shield turns nothing");
    lowered.stage = 0;
    assert!(blocks(&lowered, -5.0, 0.0));
}

#[test]
fn a_leech_latches_drains_and_lets_go_of_a_roll() {
    let mut run = duel(EnemyKind::Leech, 5.0);
    run.players.get_mut(&1).unwrap().max_hp = 1000;
    run.players.get_mut(&1).unwrap().hp = 900;
    let seen = watch(&mut run, 200);
    assert!(spent(&seen, 1) >= LEECH_REAR, "it rears first");
    assert!(first(&seen, 3).is_some(), "then latches");
    assert!(run.players[&1].hp < 900, "and drains");
    assert!(run.enemies[0].hp > 0);
    for _ in 0..120 {
        if run.enemies[0].latched() {
            break;
        }
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies[0].latched());
    tick(
        &mut run,
        Input {
            dash: true,
            move_x: 1,
            ..Default::default()
        },
    );
    assert_eq!(run.enemies[0].stage, 4, "shaken off");
}

#[test]
fn a_marksman_draws_its_line_holds_it_and_looses_one_bolt() {
    let mut run = duel(EnemyKind::Marksman, 12.0);
    let mut bolts = 0;
    let mut locked: Option<(f32, f32)> = None;
    let mut tell_ticks = 0;
    for _ in 0..400 {
        run.step(&BTreeMap::new());
        let e = &run.enemies[0];
        if e.stage == 1 {
            tell_ticks += 1;
            if e.timer >= AIM_LOCK {
                // Held still: the knight sidesteps and the line does not.
                match locked {
                    None => locked = Some(e.dir),
                    Some(d) => assert_eq!(d, e.dir),
                }
                run.players.get_mut(&1).unwrap().y = 10.0;
            }
        }
        bolts += run
            .projectiles
            .iter()
            .filter(|p| p.hostile && p.kind == Shot::Bolt)
            .count();
        if bolts > 0 {
            break;
        }
    }
    assert!(bolts > 0, "it fires");
    assert!(tell_ticks >= AIM_TICKS, "after the whole aim: {tell_ticks}");
}

#[test]
fn a_mirrorling_answers_with_a_copy_of_the_last_shot_after_a_flash() {
    let mut run = duel(EnemyKind::Mirrorling, 8.0);
    // The knight looses an arrow.
    tick(
        &mut run,
        Input {
            fire: true,
            aim_x: -1,
            ..Default::default()
        },
    );
    assert_ne!(run.enemies[0].stage & 7, 0, "it saw it");
    let mut flashed = 0;
    let mut copy = false;
    for _ in 0..300 {
        run.step(&BTreeMap::new());
        let e = &run.enemies[0];
        if e.stage & 8 != 0 {
            flashed += 1;
        }
        if run
            .projectiles
            .iter()
            .any(|p| p.hostile && p.kind == Shot::Arrow)
        {
            copy = true;
            break;
        }
    }
    assert!(copy, "it handed the arrow back");
    assert!(flashed >= MIRROR_FLASH, "after its flash: {flashed}");
    assert_eq!(run.enemies[0].stage, 0, "and the copy is spent");
}

#[test]
fn a_spitter_swells_spits_and_leaves_a_puddle_that_stings() {
    let mut run = duel(EnemyKind::Spitter, 8.0);
    let mut swelled = 0;
    let mut landed = false;
    for _ in 0..400 {
        run.step(&BTreeMap::new());
        if run.enemies[0].stage == 1 {
            swelled += 1;
        }
        if !run.critters.puddles.is_empty() {
            landed = true;
            break;
        }
    }
    assert!(landed, "a puddle");
    assert!(swelled >= SPIT_SWELL, "after the swell: {swelled}");
    // Stand in it: it stings; it dries up.
    let (px, py) = (run.critters.puddles[0].x, run.critters.puddles[0].y);
    run.enemies.clear();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (px, py);
    hero.invulnerable = 0;
    let before = hero.hp;
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].hp < before);
    for _ in 0..PUDDLE_STAYS + 5 {
        run.step(&BTreeMap::new());
    }
    assert!(run.critters.puddles.is_empty(), "it dries");
}

#[test]
fn a_ringer_quickens_the_monsters_near_it_for_a_few_seconds() {
    let mut run = duel(EnemyKind::Ringer, 10.0);
    put(&mut run, EnemyKind::Dummy, 12.0, 12.0);
    // A dummy is not quickened; use a skeleton far from the knight's reach.
    run.enemies.pop();
    put(&mut run, EnemyKind::Skeleton, 12.0, 12.0);
    let mut rung = false;
    let mut tell = 0;
    for _ in 0..400 {
        run.step(&BTreeMap::new());
        if run.enemies[0].stage == 1 {
            tell += 1;
        }
        if run.enemies[1].rung > 0 {
            rung = true;
            break;
        }
    }
    assert!(rung, "the bell was rung");
    assert!(tell >= BELL_RAISE, "after it was raised: {tell}");
    assert_eq!(run.enemies[0].rung, 0, "not the ringer itself");
    assert!(run.enemies[1].rung > RUNG_TICKS - 3);
    run.enemies.remove(0);
    for _ in 0..RUNG_TICKS + 3 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.enemies[0].rung, 0, "and it wears off");
}

#[test]
fn a_quickened_monster_covers_more_ground() {
    let walk = |rung: u32| {
        let mut run = quiet();
        put(&mut run, EnemyKind::Skeleton, 6.0, 14.0);
        run.enemies[0].rung = rung;
        let mut path = 0.0;
        for _ in 0..30 {
            let (x0, y0) = (run.enemies[0].x, run.enemies[0].y);
            run.step(&BTreeMap::new());
            path += (run.enemies[0].x - x0).hypot(run.enemies[0].y - y0);
        }
        path
    };
    assert!(walk(100) > walk(0) * 1.2);
}

#[test]
fn a_geode_glows_then_bursts_a_ring_and_splits_in_two_when_it_falls() {
    let mut run = duel(EnemyKind::Geode, 8.0);
    let mut glow = 0;
    let mut orbs = 0;
    for _ in 0..400 {
        run.step(&BTreeMap::new());
        if run.enemies[0].stage == 1 {
            glow += 1;
        }
        orbs = run.projectiles.iter().filter(|p| p.hostile).count();
        if orbs >= 8 {
            break;
        }
    }
    assert!(orbs >= 8, "a ring of shards");
    assert!(glow >= GEODE_GLOW, "after the glow: {glow}");
    run.projectiles.clear();
    run.enemies[0].hp = 0;
    run.step(&BTreeMap::new());
    let shards = run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Shard)
        .count();
    assert_eq!(shards, 2, "it falls apart in two");
    assert!(!run.enemies.iter().any(|e| e.kind == EnemyKind::Geode));
}

#[test]
fn a_hound_crouches_marks_its_line_and_rushes_down_it_dropping_embers() {
    let mut run = duel(EnemyKind::Hound, 9.0);
    // Straight down the lane: it has just decided to go.
    run.enemies[0].stage = 1;
    run.enemies[0].dir = (1.0, 0.0);
    let before = run.players[&1].hp;
    let mut embers = 0;
    let mut crouch = 0;
    let mut rushed = false;
    for _ in 0..500 {
        run.step(&BTreeMap::new());
        let e = &run.enemies[0];
        if e.stage == 1 {
            crouch += 1;
        }
        if e.stage == 2 {
            rushed = true;
            assert!(crouch + 1 >= HOUND_CROUCH, "the whole crouch first: {crouch}");
        }
        embers = embers.max(
            run.projectiles
                .iter()
                .filter(|p| p.hostile && p.kind == Shot::Ember)
                .count(),
        );
        if rushed && e.stage == 3 {
            break;
        }
    }
    assert!(rushed);
    assert!(embers >= 2, "embers on the floor behind it: {embers}");
    assert!(run.players[&1].hp < before, "the rush ran the knight down");
}

#[test]
fn the_third_company_lives_in_the_packs_and_every_fight_has_one() {
    use crate::rules::layout;
    let expect = [
        (
            Pack::Crypt,
            vec![
                EnemyKind::Pavise,
                EnemyKind::Marksman,
                EnemyKind::Mirrorling,
                EnemyKind::Ringer,
                EnemyKind::Biter,
            ],
        ),
        (
            Pack::Cavern,
            vec![
                EnemyKind::Mole,
                EnemyKind::Geode,
                EnemyKind::Leech,
                EnemyKind::Spitter,
                EnemyKind::Biter,
            ],
        ),
        (
            Pack::Hellforge,
            vec![
                EnemyKind::Hound,
                EnemyKind::Pavise,
                EnemyKind::Spitter,
                EnemyKind::Marksman,
            ],
        ),
        (
            Pack::Archive,
            vec![
                EnemyKind::Mirrorling,
                EnemyKind::Ringer,
                EnemyKind::Marksman,
                EnemyKind::Leech,
            ],
        ),
        (
            Pack::Fungal,
            vec![
                EnemyKind::Leech,
                EnemyKind::Spitter,
                EnemyKind::Mole,
                EnemyKind::Geode,
            ],
        ),
        (
            Pack::Unknown,
            vec![
                EnemyKind::Mirrorling,
                EnemyKind::Marksman,
                EnemyKind::Hound,
                EnemyKind::Pavise,
            ],
        ),
    ];
    let depth_of = |pack: Pack| match pack {
        Pack::Crypt | Pack::Cavern => 2,
        Pack::Hellforge => 3,
        Pack::Archive => 4,
        Pack::Fungal => 5,
        Pack::Unknown => 6,
    };
    for (pack, kinds) in expect {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let floor = layout::floor(depth_of(pack), pack, &mut Rng::new(seed));
            for room in floor.rooms.iter().filter(|r| r.kind == RoomKind::Fight) {
                assert!(
                    room.roster.iter().any(|k| k.critter()),
                    "{pack:?} seed {seed}: a fight with none of the third company"
                );
                seen.extend(room.roster.iter().copied().map(|k| format!("{k:?}")));
            }
        }
        for kind in kinds {
            assert!(
                seen.contains(&format!("{kind:?}")),
                "{pack:?} never meets {kind:?}"
            );
        }
    }
}

#[test]
fn a_leech_shaken_off_by_a_wall_never_lands_inside_it() {
    let mut run = duel(EnemyKind::Leech, 5.0);
    run.players.get_mut(&1).unwrap().max_hp = 1000;
    run.players.get_mut(&1).unwrap().hp = 900;
    for _ in 0..400 {
        if run.enemies[0].latched() {
            break;
        }
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies[0].latched());
    // Hard by the west wall, aiming east: the throw goes west, at the wall.
    let row = (run.players[&1].y / TILE_UNITS) as i32;
    let x = TILE_UNITS * 1.0 + 0.6;
    run.players.get_mut(&1).unwrap().x = x;
    run.players.get_mut(&1).unwrap().aim_x = 1.0;
    run.enemies[0].x = x;
    tick(
        &mut run,
        Input {
            dash: true,
            move_x: 1,
            ..Default::default()
        },
    );
    assert_eq!(run.enemies[0].stage, 4, "shaken off");
    let col = (run.enemies[0].x / TILE_UNITS).floor() as i32;
    assert_ne!(run.room().tile(col, row), Tile::Wall, "thrown into the wall");
}
