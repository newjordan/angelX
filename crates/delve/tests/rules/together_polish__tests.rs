use super::*;
use super::polish_testing::*;
use crate::realm::Spoil;
use crate::render::overworld::Img;
use crate::render::overworld::arena::knights as figures;

fn shots_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from)
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_knight_sheet -- --ignored`
#[test]
#[ignore]
fn write_knight_sheet() {
    let Some(dir) = shots_dir() else {
        return;
    };
    let mut run = Run::new(4, 2, None);
    let kits = ["percival", "lynette", "ironbraid"];
    let (cell, cols) = (20, 9);
    let mut sheet = Img::black(cell * cols + 4, cell * kits.len() as i32 + 4);
    for (row, kit) in kits.iter().enumerate() {
        run.outfit(1, knights::knight(kit).unwrap());
        let base = run.players[&1].clone();
        let cases: [(u8, bool, f32, f32); 9] = [
            (0, false, 1.0, 0.0),
            (1, false, 1.0, 0.0),
            (2, false, 1.0, 0.0),
            (0, true, 1.0, 0.0),
            (0, true, 0.0, 1.0),
            (0, true, 0.0, -1.0),
            (1, true, 1.0, 0.0),
            (2, true, 0.0, 1.0),
            (2, true, 0.0, -1.0),
        ];
        for (col, (gear, shield, ax, ay)) in cases.into_iter().enumerate() {
            let mut hero = base.clone();
            hero.gear = gear;
            hero.shielding = shield;
            (hero.aim_x, hero.aim_y) = (ax, ay);
            let im = figures::figure(&hero, 1);
            sheet.stamp(&im, 2 + col as i32 * cell + 2, 2 + row as i32 * cell + 2);
        }
    }
    save(&sheet, &dir, "knight_sheet");
}

fn tick(run: &mut Run, input: Input) {
    run.step(&BTreeMap::from([(1, input)]));
}

fn press() -> Input {
    Input {
        take: true,
        ..Default::default()
    }
}

#[test]
fn walking_over_a_weapon_keeps_yours_and_the_take_key_swaps() {
    let mut run = quiet();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    assert_eq!(run.players[&1].arm.as_deref(), Some("bow"));
    lay(&mut run, "crossbow", x, y);
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(
        run.players[&1].arm.as_deref(),
        Some("bow"),
        "walking over is not taking"
    );
    assert!(run.room().items.iter().any(|i| i.card == "crossbow"));
    // P: the crossbow is in hand, the bow lies where it was.
    tick(&mut run, press());
    assert_eq!(run.players[&1].arm.as_deref(), Some("crossbow"));
    assert!(run.room().items.iter().any(|i| i.card == "bow"));
    assert!(!run.room().items.iter().any(|i| i.card == "crossbow"));
    // Held, it does not swap back and forth; let go and press, and it does.
    for _ in 0..5 {
        tick(&mut run, press());
    }
    assert_eq!(run.players[&1].arm.as_deref(), Some("crossbow"));
    tick(&mut run, Input::default());
    tick(&mut run, press());
    assert_eq!(run.players[&1].arm.as_deref(), Some("bow"), "swapped back");
}

#[test]
fn the_take_key_reaches_a_weapon_beside_you_and_not_one_far_off() {
    let mut run = quiet();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    lay(&mut run, "handgonne", x + 12.0, y);
    tick(&mut run, press());
    assert_eq!(run.players[&1].arm.as_deref(), Some("bow"), "too far");
    lay(&mut run, "handgonne", x + 1.8, y);
    tick(&mut run, Input::default());
    tick(&mut run, press());
    assert_eq!(run.players[&1].arm.as_deref(), Some("handgonne"));
}

#[test]
fn other_pickups_are_still_taken_by_walking() {
    let mut run = quiet();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    run.players.get_mut(&1).unwrap().bombs = 0;
    lay(&mut run, "bomb", x, y);
    run.step(&BTreeMap::new());
    assert!(run.players[&1].bombs > 0, "bombs are taken on touch");
    assert!(!run.room().items.iter().any(|i| i.card == "bomb"));
}

#[test]
fn the_dwarf_spins_her_axes_and_cuts_everything_round_her() {
    let mut run = quiet();
    run.outfit(1, knights::knight("ironbraid").unwrap());
    assert_eq!(run.players[&1].guard, cards::Guard::Spin);
    assert!(
        run.players[&1]
            .forged
            .as_ref()
            .is_some_and(|w| w.melee.is_some())
    );
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    for (dx, dy) in [(2.0, 0.0), (-2.0, 0.5), (0.0, -2.2), (0.5, 2.0)] {
        run.spawn_at_for_test(EnemyKind::Skeleton, x + dx, y + dy);
        run.enemies.last_mut().unwrap().hp = 500;
    }
    run.spawn_at_for_test(EnemyKind::Skeleton, x + 9.0, y);
    run.enemies.last_mut().unwrap().hp = 500;
    let held = Input {
        dash: true,
        ..Default::default()
    };
    tick(&mut run, held);
    assert!(run.players[&1].spin > 0, "the spin is under way");
    for _ in 0..spin::TICKS {
        tick(&mut run, held);
    }
    let cut = run.enemies.iter().filter(|e| e.hp < 500).count();
    assert_eq!(cut, 4, "all four round her are cut, the far one is not");
    assert_eq!(run.players[&1].spin, 0);
    assert!(
        run.players[&1].dash_cooldown > 0,
        "a breath before the next"
    );
}

#[test]
fn the_spin_cuts_down_shots_in_reach() {
    let mut run = quiet();
    run.outfit(1, knights::knight("ironbraid").unwrap());
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    run.projectiles.push(Projectile {
        x: x + 2.0,
        y,
        vx: 0.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Orb,
        damage: 12,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 90,
    });
    tick(
        &mut run,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    assert!(run.projectiles.iter().all(|p| !p.hostile || p.ttl == 0));
}

#[test]
fn health_takes_the_helmet_then_the_clothes_and_mending_puts_them_back() {
    for kit in ["percival", "ironbraid"] {
        let mut run = quiet();
        run.outfit(1, knights::knight(kit).unwrap());
        let max = run.players[&1].max_hp;
        let set = |run: &mut Run, hp: u32| {
            run.players.get_mut(&1).unwrap().hp = hp;
            run.step(&BTreeMap::new());
        };
        run.step(&BTreeMap::new());
        assert_eq!(run.players[&1].gear, 0);
        set(&mut run, max * 41 / 100);
        assert_eq!(
            run.players[&1].gear, 0,
            "{kit}: above two-fifths, the helmet stays"
        );
        set(&mut run, max * 40 / 100);
        assert_eq!(run.players[&1].gear, 1, "{kit}: bare-headed");
        assert!(run.players[&1].pop > 0, "{kit}: the helmet is in the air");
        set(&mut run, max * 16 / 100);
        assert_eq!(run.players[&1].gear, 1);
        set(&mut run, max * 15 / 100);
        assert_eq!(run.players[&1].gear, 2, "{kit}: in smallclothes");
        assert!(run.players[&1].pop > 0);
        set(&mut run, max * 30 / 100);
        assert_eq!(run.players[&1].gear, 1, "{kit}: the clothes are back on");
        set(&mut run, max);
        assert_eq!(run.players[&1].gear, 0, "{kit}: and the helmet");
    }
}

#[test]
fn a_hit_flashes_the_monster_and_the_flash_runs_down() {
    let mut run = quiet();
    run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 10.0);
    run.enemies.last_mut().unwrap().hp = 400;
    run.enemies.last_mut().unwrap().max_hp = 400;
    run.step(&BTreeMap::new());
    assert_eq!(run.enemies[0].hurt, 0, "untouched, no flash");
    run.enemies[0].hp -= 40;
    run.step(&BTreeMap::new());
    assert_eq!(run.enemies[0].hurt, desperate::HIT_FLASH);
    for _ in 0..desperate::HIT_FLASH {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.enemies[0].hurt, 0);
}

fn dragon(run: &mut Run, share: f32) {
    run.spawn_at_for_test(EnemyKind::Dragon, 24.0, 6.0);
    let dragon = run.enemies.last_mut().unwrap();
    dragon.max_hp = 1000;
    dragon.hp = (1000.0 * share) as u32;
}

#[test]
fn a_boss_flashes_faster_and_fires_sooner_as_it_dies() {
    let mut run = quiet();
    dragon(&mut run, 1.0);
    let whole = run.enemies[0].clone();
    assert_eq!(whole.flash_every(), None);
    assert_eq!(whole.beat(75), 75);
    let mut last = (u32::MAX, u32::MAX);
    for share in [0.34, 0.25, 0.15, 0.06, 0.01] {
        let mut boss = whole.clone();
        boss.hp = (1000.0 * share) as u32;
        let every = boss.flash_every().expect("desperate");
        let beat = boss.beat(75);
        assert!(every <= last.0 && beat <= last.1, "{share}: {every} {beat}");
        last = (every, beat);
    }
    assert!(last.0 <= 12, "near death it flickers: {}", last.0);
    assert!(last.1 < 75 * 7 / 10, "and fires well sooner: {}", last.1);
}

#[test]
fn a_dying_boss_warns_then_rings_the_room_with_one_lane_open() {
    let mut run = quiet();
    dragon(&mut run, 0.2);
    run.players.get_mut(&1).unwrap().invulnerable = 10_000;
    let mut warned = false;
    let mut ring = 0;
    for _ in 0..(desperate::RING_WARN + 4) {
        warned |= run.enemies[0].ring_warning().is_some();
        run.step(&BTreeMap::new());
        if run.enemies[0].dread == desperate::RING_WARN + 1 {
            ring = run
                .projectiles
                .iter()
                .filter(|p| p.hostile && p.kind == Shot::Ember && p.damage == 14)
                .count();
        }
    }
    assert!(warned, "the ring was shown before it flew");
    assert!(
        ring > 0 && ring < 18,
        "a ring with a lane left open: {ring} shots"
    );
}

#[test]
fn a_boss_near_death_adds_spiralling_arms_a_healthier_one_does_not() {
    for (share, arms) in [(0.3, false), (0.1, true)] {
        let mut run = quiet();
        dragon(&mut run, share);
        run.players.get_mut(&1).unwrap().invulnerable = 10_000;
        let mut orbs = 0;
        for _ in 0..desperate::ARMS_TO + 2 {
            run.step(&BTreeMap::new());
            orbs += run
                .projectiles
                .iter()
                .filter(|p| p.hostile && p.damage == 10 && p.kind == Shot::Orb)
                .count();
        }
        assert_eq!(orbs > 0, arms, "{share}");
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_polish_shots -- --ignored`
#[test]
#[ignore]
fn write_polish_shots() {
    let Some(dir) = shots_dir() else {
        return;
    };
    // Hits and health bars: a skeleton just struck (cream, then red), one
    // hurt and one whole, and a guardian's bar.
    let mut run = quiet();
    for (x, y) in [(14.0, 8.0), (20.0, 8.0), (26.0, 8.0), (32.0, 8.0)] {
        run.spawn_at_for_test(EnemyKind::Skeleton, x, y);
        let e = run.enemies.last_mut().unwrap();
        e.hp = 400;
        e.max_hp = 400;
        e.seen_hp = 400;
    }
    run.enemies[1].hp = 200;
    run.enemies[2].hp = 80;
    run.enemies[1].seen_hp = 200;
    run.enemies[2].seen_hp = 80;
    run.enemies[0].hp = 300;
    run.enemies[0].hurt = desperate::HIT_FLASH;
    run.enemies[3].hurt = 2;
    save(&frame(&run), &dir, "hit_bars_a");
    run.enemies[0].hurt = 3;
    run.enemies[3].hurt = 5;
    save(&frame(&run), &dir, "hit_bars_b");

    // A guardian at full health, then going desperate: flashing on and off.
    for (share, name) in [
        (1.0, "boss_full"),
        (0.30, "boss_30"),
        (0.12, "boss_12"),
        (0.03, "boss_03"),
    ] {
        let mut run = quiet();
        run.enemies.clear();
        run.spawn_boss(0);
        let boss = run.enemies.last_mut().unwrap();
        boss.hp = (boss.max_hp as f32 * share) as u32;
        boss.seen_hp = boss.hp;
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        // Walk the clock to a tick when it flashes, and one when it does not.
        let mut on = None;
        let mut off = None;
        for _ in 0..90 {
            run.step(&BTreeMap::new());
            let flashing = run.enemies[0].flashing(run.tick);
            if flashing && on.is_none() {
                on = Some(frame(&run));
            }
            if !flashing && off.is_none() {
                off = Some(frame(&run));
            }
            run.projectiles.clear();
            if on.is_some() && off.is_some() {
                break;
            }
        }
        if let Some(img) = off {
            save(&img, &dir, &format!("{name}_off"));
        }
        if let Some(img) = on {
            save(&img, &dir, &format!("{name}_on"));
        }
    }

    // The last stand's warnings: the ring closing, and the arms.
    let mut run = quiet();
    run.enemies.clear();
    run.spawn_boss(0);
    let boss = run.enemies.last_mut().unwrap();
    boss.hp = boss.max_hp / 10;
    boss.seen_hp = boss.hp;
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    let (mut ring, mut arms, mut burst) = (false, false, false);
    for _ in 0..400 {
        run.step(&BTreeMap::new());
        let e = &run.enemies[0];
        if !ring && e.ring_warning().is_some_and(|t| t > 0.8) {
            save(&frame(&run), &dir, "last_stand_ring");
            ring = true;
        }
        if !arms && e.arms_warning().is_some_and(|t| t > 0.7) {
            save(&frame(&run), &dir, "last_stand_arms");
            arms = true;
        }
        if !burst && ring && run.projectiles.iter().filter(|p| p.hostile).count() > 10 {
            for _ in 0..8 {
                run.step(&BTreeMap::new());
            }
            save(&frame(&run), &dir, "last_stand_burst");
            burst = true;
        }
        if ring && arms && burst {
            break;
        }
    }

    // A weapon beside the knight, with its prompt.
    let mut run = quiet();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    lay(&mut run, "knights-blade", x + 1.8, y - 0.5);
    run.step(&BTreeMap::new());
    save(&frame(&run), &dir, "take_prompt");

    // The dwarf's spin, frame by frame, with enemies round her.
    let mut run = quiet();
    run.outfit(1, knights::knight("ironbraid").unwrap());
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    for (dx, dy) in [
        (2.2, 0.0),
        (-2.2, 0.0),
        (0.0, -2.0),
        (1.5, 2.0),
        (-1.5, 2.0),
    ] {
        run.spawn_at_for_test(EnemyKind::Skeleton, x + dx, y + dy);
        let e = run.enemies.last_mut().unwrap();
        e.hp = 900;
        e.max_hp = 900;
        e.seen_hp = 900;
    }
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    save(&frame(&run), &dir, "dwarf_rest");
    let held = Input {
        dash: true,
        ..Default::default()
    };
    for n in 0..spin::TICKS {
        tick(&mut run, held);
        if [0, 2, 4, 7, 10].contains(&n) {
            save(&frame(&run), &dir, &format!("dwarf_spin_{n:02}"));
        }
    }

    // Each stage of dress in the room, with the helmet in the air and the
    // clothes after it: the knight, then the dwarf.
    for kit in ["percival", "ironbraid"] {
        let mut run = quiet();
        run.outfit(1, knights::knight(kit).unwrap());
        let max = run.players[&1].max_hp;
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        run.step(&BTreeMap::new());
        save(&frame(&run), &dir, &format!("gear_{kit}_0"));
        run.players.get_mut(&1).unwrap().hp = max * 38 / 100;
        for n in 0..gear::POP_TICKS {
            run.step(&BTreeMap::new());
            if [1, 5, 9, 14, 20].contains(&n) {
                save(&frame(&run), &dir, &format!("gear_{kit}_1_pop{n:02}"));
            }
        }
        save(&frame(&run), &dir, &format!("gear_{kit}_1"));
        run.players.get_mut(&1).unwrap().hp = max * 12 / 100;
        for n in 0..gear::POP_TICKS {
            run.step(&BTreeMap::new());
            if [2, 7, 12, 18].contains(&n) {
                save(&frame(&run), &dir, &format!("gear_{kit}_2_pop{n:02}"));
            }
        }
        save(&frame(&run), &dir, &format!("gear_{kit}_2"));
    }

    // A raised shield toward each way the knight faces.
    for (name, ax, ay) in [
        ("right", 1, 0),
        ("left", -1, 0),
        ("up", 0, -1),
        ("down", 0, 1),
    ] {
        let mut run = quiet();
        run.outfit(1, knights::knight("lynette").unwrap());
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        let held = Input {
            dash: true,
            aim_x: ax,
            aim_y: ay,
            ..Default::default()
        };
        for _ in 0..4 {
            tick(&mut run, held);
        }
        assert!(run.players[&1].shielding);
        save(&frame(&run), &dir, &format!("shield_{name}"));
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_goblin_shots -- --ignored`
#[test]
#[ignore]
fn write_goblin_shots() {
    use crate::render::overworld::arena::sprites;
    let Some(dir) = shots_dir() else {
        return;
    };
    // The goblin by itself: standing, then running, with the sack empty and
    // then ever fuller.
    let mut sheet = Img::black(5 * 22 + 4, 2 * 20 + 4);
    for (row, running) in [false, true].into_iter().enumerate() {
        for burden in 0..4u8 {
            let im = sprites::goblin(burden, running, 0);
            sheet.stamp(&im, 4 + burden as i32 * 22, 4 + row as i32 * 20);
        }
        let im = sprites::goblin(3, running, 3);
        sheet.stamp(&im, 4 + 4 * 22, 4 + row as i32 * 20);
    }
    save(&sheet, &dir, "goblin_sheet");

    // The room: a goblin after a potion on the floor, then robbing a knight,
    // running with a sack, and dying with it.
    let mut run = Run::new(33, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (14.0, 14.0);
    hero.carried.add(Spoil::Gold, 120);
    hero.carried.add(Spoil::Gem, 4);
    hero.hand = vec!["potion".into(), "holy-water".into()];
    run.spawn_at_for_test(EnemyKind::Goblin, 36.0, 14.0);
    lay(&mut run, "potion", 32.0, 14.0);
    for n in 0..10 {
        run.step(&BTreeMap::new());
        if n == 3 {
            save(&frame(&run), &dir, "goblin_a_scavenging");
        }
    }
    // It comes for the knight.
    let (mut stole, mut ran, mut dead) = (false, false, false);
    for _ in 0..600 {
        run.step(&BTreeMap::new());
        let Some(g) = run
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Goblin)
            .cloned()
        else {
            break;
        };
        if !stole && g.cry > 0 {
            save(&frame(&run), &dir, "goblin_b_thief_cry");
            stole = true;
        }
        if stole && !ran && g.cry == 0 && g.fleeing() {
            save(&frame(&run), &dir, "goblin_c_fleeing");
            ran = true;
        }
        if ran && !dead {
            // A few ticks on, then the kill.
            for _ in 0..40 {
                run.step(&BTreeMap::new());
            }
            save(&frame(&run), &dir, "goblin_d_before_kill");
            let e = run
                .enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::Goblin)
                .expect("still running");
            let (x, y) = (e.x, e.y);
            e.hp = 0;
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (x - 6.0, y);
            run.step(&BTreeMap::new());
            save(&frame(&run), &dir, "goblin_e_burst");
            dead = true;
            break;
        }
    }
    assert!(stole && ran && dead, "{stole} {ran} {dead}");
}
