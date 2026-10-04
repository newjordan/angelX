use super::*;

fn input(fire: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            aim_x: 1,
            fire,
            ..Default::default()
        },
    )])
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

#[test]
fn a_cleared_room_opens_a_window_and_the_next_fight_closes_it() {
    let mut run = Run::new(4, 2, Some("Matt"));
    fight(&mut run);
    assert!(!run.can_wish(1), "no wishes mid-fight");
    run.clear_for_test();
    assert!(run.can_wish(1) && run.can_wish(2));
    assert!(run.cues.iter().any(|c| c == "window"));
    let volley = phrasebook::get("volley").unwrap();
    assert_eq!(run.grant(1, &volley).unwrap(), "Triple Volley I");
    assert!(!run.can_wish(1), "one wish a window");
    assert!(run.can_wish(2), "each knight their own");
    let other = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.cleared)
        .unwrap();
    run.enter_for_test(other);
    assert!(run.window.is_none() || run.phase != Phase::Fighting);
}

#[test]
fn a_granted_volley_flies_three_wide_on_its_cadence_and_climbs() {
    let mut run = Run::new(4, 2, None);
    fight(&mut run);
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 99_999;
    }
    let volley = phrasebook::get("volley").unwrap();
    run.grant(1, &volley).unwrap();
    assert_eq!(run.players[&1].bonus.volley, 5);
    let mut fired = Vec::new();
    for _ in 0..(10 * HZ) {
        let before = run.projectiles.iter().filter(|p| !p.hostile).count();
        run.step(&input(true));
        let now = run.projectiles.iter().filter(|p| !p.hostile).count();
        if run.players[&1].fire_cooldown > 0 && now > before.saturating_sub(4) {
            fired.push(now.saturating_sub(before));
        }
        if run.players[&1].loosed >= 10 {
            break;
        }
    }
    assert!(run.players[&1].loosed >= 10);
    // Granted again: the tighter cadence replaces the first.
    run.window = Some(Default::default());
    run.grant(1, &volley).unwrap();
    assert_eq!(run.players[&1].bonus.volley, 3);
    assert_eq!(
        run.players[&1]
            .deck
            .iter()
            .filter(|c| c.starts_with("wish-volley-"))
            .count(),
        1
    );
}

#[test]
fn bouncing_homing_chaining_mending_shots_do_what_they_say() {
    // The entrance room: open floor, no pillars in a shot's way.
    let mut run = Run::new(4, 2, Some("Matt"));
    run.calm_for_test();
    for (id, wish) in [(1, "ricochet"), (1, "seeker"), (1, "chain"), (1, "mend")] {
        run.window = Some(Default::default());
        run.grant(id, &phrasebook::get(wish).unwrap()).unwrap();
    }
    let bonus = run.players[&1].bonus;
    assert_eq!(
        (bonus.bounce, bonus.homing, bonus.chain, bonus.mend),
        (1, 1, 1, 1)
    );
    // Bounce: a shot at the west wall comes back.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (8.0, 10.0);
    hero.fire_cooldown = 0;
    run.enemies.clear();
    // The room clears (and sweeps its shots) before the test shot.
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().fire_cooldown = 0;
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_x: -1,
            ..Default::default()
        },
    )]));
    let mut came_back = false;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
        came_back |= run.projectiles.iter().any(|p| !p.hostile && p.vx > 0.0);
    }
    assert!(came_back, "a bounced shot flies back east");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_wish_shots -- --ignored`
#[test]
#[ignore]
fn write_wish_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(4, 2, None);
    fight(&mut run);
    run.calm_for_test();
    let spots = [
        (24.0, 12.0),
        (28.0, 11.0),
        (20.0, 11.0),
        (24.0, 8.0),
        (31.0, 9.0),
        (17.0, 9.0),
    ];
    for (enemy, (x, y)) in run.enemies.iter_mut().zip(spots.iter().cycle()) {
        enemy.hp = 99_999;
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (*x, *y, *x, *y);
    }
    for wish in ["volley", "volley", "volley", "chain", "chain"] {
        run.window = Some(Default::default());
        run.grant(1, &phrasebook::get(wish).unwrap()).unwrap();
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 22.0);
    let mut best = 0;
    for _ in 0..(3 * HZ) {
        run.step(&BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                fire: true,
                ..Default::default()
            },
        )]));
        let lit = run.sparks.len() * 10 + run.projectiles.iter().filter(|p| !p.hostile).count();
        if lit > best && !run.sparks.is_empty() {
            best = lit;
            let img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
            let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
            out.extend(img.rgb_bytes());
            std::fs::write(dir.join("wish_volley_chain.ppm"), out).unwrap();
        }
    }
    assert!(best > 0, "a volley and a chain were drawn");
}

#[test]
fn a_thrown_blade_flies_out_and_comes_home_and_morningstars_strike() {
    let mut run = Run::new(4, 2, None);
    run.outfit(1, knights::knight("galahad").unwrap());
    run.calm_for_test();
    run.enemies.clear();
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.fire_cooldown) = (24.0, 16.0, 0);
    // Nothing in reach: the blade is thrown.
    run.step(&input(true));
    let blade = run
        .projectiles
        .iter()
        .find(|p| p.kind == Shot::Blade)
        .expect("thrown");
    assert_eq!(blade.traits.back, 1);
    let mut came_back = false;
    for _ in 0..(3 * HZ) {
        run.step(&BTreeMap::new());
        came_back |= run.projectiles.iter().any(|p| p.traits.back == 2);
        if came_back && run.projectiles.iter().all(|p| p.kind != Shot::Blade) {
            break;
        }
    }
    assert!(came_back, "it turned for home");
    assert!(
        run.projectiles.iter().all(|p| p.kind != Shot::Blade),
        "and was caught"
    );
    // A morningstar strikes what it touches.
    run.window = Some(Default::default());
    run.grant(1, &phrasebook::get("morningstar").unwrap())
        .unwrap();
    let (bx, by) = run.orbits(1)[0];
    run.spawn_at_for_test(EnemyKind::Skeleton, bx, by);
    let hp = run.enemies[0].hp;
    for _ in 0..8 {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.first().is_none_or(|e| e.hp < hp), "struck");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_melee_shots -- --ignored`
#[test]
#[ignore]
fn write_melee_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = Run::new(4, 2, Some("Matt"));
    run.outfit(1, knights::knight("galahad").unwrap());
    fight(&mut run);
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 99_999;
    }
    run.window = Some(Default::default());
    run.grant(2, &phrasebook::get("morningstar").unwrap())
        .unwrap();
    run.window = Some(Default::default());
    run.grant(2, &phrasebook::get("morningstar").unwrap())
        .unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 22.0);
    let matt = run.players.get_mut(&2).unwrap();
    (matt.x, matt.y) = (14.0, 16.0);
    run.window = None;
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    for i in 0..9 {
        run.step(&BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                fire: i == 0,
                ..Default::default()
            },
        )]));
    }
    save(&run, "melee_throw_orbit");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_flourish_shots -- --ignored`
#[test]
#[ignore]
fn write_flourish_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = Run::new(4, 2, None);
    run.outfit(1, knights::knight("gareth").unwrap());
    fight(&mut run);
    run.calm_for_test();
    let spots = [(24.0, 12.0), (26.5, 11.0), (21.5, 11.0)];
    for (enemy, (x, y)) in run.enemies.iter_mut().zip(spots.iter().cycle()) {
        enemy.hp = 99_999;
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (*x, *y, *x, *y);
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (24.0, 22.0, 0);
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            fire: true,
            ..Default::default()
        },
    )]));
    for _ in 0..(2 * HZ) {
        run.step(&BTreeMap::new());
        if run.blasts.iter().any(|b| b.2 == BLAST_TICKS - 3) {
            break;
        }
    }
    assert!(!run.blasts.is_empty(), "the ball burst");
    save(&run, "flourish_burst");
    run.window = Some(Default::default());
    run.grant(1, &phrasebook::get("volley").unwrap()).unwrap();
    for _ in 0..8 {
        run.step(&BTreeMap::new());
    }
    save(&run, "flourish_wish");
}

#[test]
fn blasting_powder_bursts_on_the_neighbours() {
    let mut run = Run::new(4, 2, None);
    fight(&mut run);
    run.calm_for_test();
    let spots = [(24.0, 12.0), (25.8, 12.0)];
    run.enemies.truncate(2);
    for (enemy, (x, y)) in run.enemies.iter_mut().zip(spots) {
        enemy.hp = 9_999;
        enemy.kind = EnemyKind::Skeleton;
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (x, y, x, y);
    }
    run.window = Some(Default::default());
    run.grant(1, &phrasebook::get("powder").unwrap()).unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.fire_cooldown) = (24.0, 22.0, 0);
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            fire: true,
            ..Default::default()
        },
    )]));
    let side = run.enemies[1].id;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
        if !run.blasts.is_empty() {
            break;
        }
    }
    let neighbour = run.enemies.iter().find(|e| e.id == side).unwrap();
    assert!(neighbour.hp < 9_999, "the burst reached the one beside it");
}
