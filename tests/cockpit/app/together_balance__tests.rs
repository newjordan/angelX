//! A bot plays each knight of the company through every fight room of the
//! first floor, for several seeds: how often it wins a room, what it costs
//! in health, how long it takes. Melee should be as winnable as a bow.
//!
//! `cargo test --release knight_balance -- --ignored --nocapture`
use super::*;

/// One knight's bot input: close in to `reach` (or hold at range), aim and
/// fire at the nearest monster, roll from shots about to land.
fn bot(run: &Run, reach: f32, ranged: bool) -> Input {
    bot_as(run, 1, reach, ranged)
}

/// Knight `id`'s bot input, as `bot`.
fn bot_as(run: &Run, id: u32, reach: f32, ranged: bool) -> Input {
    let hero = &run.players[&id];
    let Some(target) = run.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
        (a.x - hero.x)
            .hypot(a.y - hero.y)
            .total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
    }) else {
        return Input::default();
    };
    let (dx, dy) = (target.x - hero.x, target.y - hero.y);
    let d = dx.hypot(dy);
    let sign = |v: f32, dead: f32| {
        if v > dead {
            1
        } else if v < -dead {
            -1
        } else {
            0
        }
    };
    let toward = (sign(dx, 0.5), sign(dy, 0.5));
    let (move_x, move_y) = if ranged {
        if d < 8.0 {
            (-toward.0, -toward.1)
        } else if d > 13.0 {
            toward
        } else {
            // Strafe round the target.
            (-toward.1, toward.0)
        }
    } else if d > reach * 0.8 {
        toward
    } else {
        (0, 0)
    };
    // Step out of what the floor telegraphs, as a player would: a bomb's
    // shadow, a lit keg, a boar's marked line.
    let away = |x: f32, y: f32| (sign(hero.x - x, 0.1), sign(hero.y - y, 0.1));
    let bomb = run
        .lobs
        .iter()
        .find(|l| (l.x - hero.x).hypot(l.y - hero.y) < 3.4)
        .map(|l| away(l.x, l.y));
    let keg = run
        .kegs
        .iter()
        .find(|k| (k.x - hero.x).hypot(k.y - hero.y) < 4.0)
        .map(|k| away(k.x, k.y));
    let slam = run
        .slams
        .iter()
        .find(|s| (s.x - hero.x).hypot(s.y - hero.y) < pit::SLAM_REACH + 0.8)
        .map(|s| away(s.x, s.y));
    let rock = run
        .rocks
        .iter()
        .find(|r| r.fall > 0 && (r.x - hero.x).hypot(r.y - hero.y) < 3.0)
        .map(|r| away(r.x, r.y));
    let boar = run
        .enemies
        .iter()
        .filter(|e| matches!(e.kind, EnemyKind::Warboar | EnemyKind::Flesher) && e.stage == 1)
        .find_map(|e| {
            let (px, py) = (hero.x - e.x, hero.y - e.y);
            let along = px * e.dir.0 + py * e.dir.1;
            let off = px * -e.dir.1 + py * e.dir.0;
            (along > 0.0 && off.abs() < 2.6).then(|| {
                let s = if off >= 0.0 { 1.0 } else { -1.0 };
                (sign(-e.dir.1 * s, 0.1), sign(e.dir.0 * s, 0.1))
            })
        });
    let (move_x, move_y) = slam
        .or(bomb)
        .or(keg)
        .or(rock)
        .or(boar)
        .unwrap_or((move_x, move_y));
    // Walled that way? Slide along it, or go round, as a player would.
    let room = run.room();
    let open = |(mx, my): (i8, i8)| {
        let (x, y) = (hero.x + f32::from(mx) * 1.4, hero.y + f32::from(my) * 1.4);
        (mx, my) != (0, 0)
            && room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
    };
    let (move_x, move_y) = if (move_x, move_y) == (0, 0) || open((move_x, move_y)) {
        (move_x, move_y)
    } else {
        [
            (move_x, 0),
            (0, move_y),
            toward,
            (-move_y, move_x),
            (move_y, -move_x),
        ]
        .into_iter()
        .find(|&m| open(m))
        .unwrap_or((move_x, move_y))
    };
    let hooked_at = run
        .hooks
        .iter()
        .any(|h| !h.back && (h.x - hero.x).hypot(h.y - hero.y) < 3.5);
    let danger = hooked_at
        || run.projectiles.iter().any(|p| {
            p.hostile && {
                let (px, py) = (p.x - hero.x, p.y - hero.y);
                px.hypot(py) < 3.0 && (p.vx * px + p.vy * py) < 0.0
            }
        });
    Input {
        move_x,
        move_y,
        aim_x: sign(dx, d * 0.38),
        aim_y: sign(dy, d * 0.38),
        fire: true,
        dash: danger,
        ..Default::default()
    }
}

#[derive(Default, Debug)]
struct Tally {
    rooms: u32,
    won: u32,
    hp_lost: u32,
    ticks: u64,
}

fn play(knight: &knights::Knight, seeds: std::ops::Range<u64>) -> Tally {
    play_with(knight, seeds, None, false)
}

/// As `play`, with the Undercroft built to `home` and the bot casting its
/// ultimate whenever it is full (`ults`).
fn play_with(
    knight: &knights::Knight,
    seeds: std::ops::Range<u64>,
    home: Option<&home::Home>,
    ults: bool,
) -> Tally {
    let mut tally = Tally::default();
    for seed in seeds {
        let mut run = Run::new(seed, 1, None);
        if let Some(home) = home {
            run.rebuild_home(home.clone(), Default::default());
        }
        run.outfit(1, knight);
        if home.is_some() {
            run.kit_out_for_test();
        }
        let (reach, ranged) = {
            let hero = &run.players[&1];
            match hero.forged.as_ref().and_then(|w| w.melee) {
                Some(m) if hero.forged.as_ref().is_some_and(|w| w.bolt.is_none()) => {
                    (m.reach, false)
                }
                _ => (12.0, true),
            }
        };
        let fights: Vec<usize> = run
            .dungeon
            .rooms
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind == RoomKind::Fight)
            .map(|(i, _)| i)
            .collect();
        for room in fights {
            let hero = run.players.get_mut(&1).unwrap();
            hero.hp = hero.max_hp;
            run.phase = Phase::Exploring;
            run.enter_for_test(room);
            if run.phase != Phase::Fighting {
                continue;
            }
            tally.rooms += 1;
            let start = run.tick;
            while run.phase == Phase::Fighting && run.tick - start < u64::from(90 * HZ) {
                let mut input = bot(&run, reach, ranged);
                input.ult = ults
                    && run.players[&1].ult_charge >= ults::ULT_FULL
                    && run.tick.is_multiple_of(2);
                // A potion when low, as a player would.
                if home.is_some() && run.players[&1].hp < 35 && !run.players[&1].hand.is_empty() {
                    input.play = 1;
                }
                run.step(&BTreeMap::from([(1, input)]));
            }
            let hero = &run.players[&1];
            tally.hp_lost += hero.max_hp - hero.hp;
            tally.ticks += run.tick - start;
            if run.phase == Phase::Exploring {
                tally.won += 1;
            }
            if run.phase == Phase::Wiped {
                // Back on its feet for the next room.
                run.phase = Phase::Exploring;
                let hero = run.players.get_mut(&1).unwrap();
                hero.hp = hero.max_hp;
            }
        }
    }
    tally
}

#[test]
#[ignore]
fn knight_balance() {
    for knight in knights::COMPANY {
        let t = play(knight, 0..12);
        eprintln!(
            "BALANCE {:<12} rooms {:>3}  won {:>5.1}%  hp lost/room {:>5.1}  secs/room {:>5.1}",
            knight.id,
            t.rooms,
            100.0 * t.won as f32 / t.rooms.max(1) as f32,
            t.hp_lost as f32 / t.rooms.max(1) as f32,
            t.ticks as f32 / HZ as f32 / t.rooms.max(1) as f32,
        );
    }
}

/// The same bot with the Undercroft built a little (Edge II, Hearth II,
/// Rack I), then with its ultimate too: what the new progression is worth.
#[test]
#[ignore]
fn knight_balance_with_home() {
    let mut home = home::Home::default();
    home.levels.insert(home::Station::Forge, 2);
    home.levels.insert(home::Station::Hearth, 2);
    home.levels.insert(home::Station::Rack, 1);
    for knight in knights::COMPANY {
        let plain = play_with(knight, 0..12, None, false);
        let built = play_with(knight, 0..12, Some(&home), false);
        let both = play_with(knight, 0..12, Some(&home), true);
        let pct = |t: &Tally| 100.0 * t.won as f32 / t.rooms.max(1) as f32;
        eprintln!(
            "HOME {:<12} plain {:>5.1}%  built {:>5.1}%  built+ult {:>5.1}%",
            knight.id,
            pct(&plain),
            pct(&built),
            pct(&both),
        );
    }
}

/// How a knight's bot fights: its reach, and whether it holds at range.
fn style(run: &Run, id: u32) -> (f32, bool) {
    let hero = &run.players[&id];
    match hero.forged.as_ref().and_then(|w| w.melee) {
        Some(m) if hero.forged.as_ref().is_some_and(|w| w.bolt.is_none()) => {
            (m.reach, false)
        }
        _ => (12.0, true),
    }
}

/// Film plates: `ANGEL_ARENA_SHOTS=<dir> cargo test --release write_delve_film -- --ignored`.
/// The room's own pixels, one tick a frame: four bot knights, each holding
/// a wish, through the first floor's fights (`party/`); and one knight
/// before and after "triple wide fire my shots" (`wish/`, grants in
/// `wish/marks.txt`).
#[test]
#[ignore]
fn write_delve_film() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    for sub in ["party", "wish"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    let save = |run: &Run, name: String| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    let grant = |run: &mut Run, id: u32, wish: &str| {
        run.window = Some(Default::default());
        run.grant(id, &phrasebook::get(wish).unwrap()).unwrap();
    };

    let mut run = Run::new(7, 3, Some("Matt"));
    run.join_seat(3, "Bree");
    run.join_seat(4, "Ana");
    let party = [(1, "galahad"), (2, "percival"), (3, "lynette"), (4, "gareth")];
    for (id, who) in party {
        run.outfit(id, knights::knight(who).unwrap());
    }
    for (id, wish) in [
        (1, "morningstar"),
        (1, "morningstar"),
        (2, "volley"),
        (2, "volley"),
        (2, "volley"),
        (3, "chain"),
        (3, "chain"),
        (4, "powder"),
        (4, "powder"),
    ] {
        grant(&mut run, id, wish);
    }
    run.window = None;
    let fights: Vec<usize> = run
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RoomKind::Fight)
        .map(|(i, _)| i)
        .collect();
    let mut f = 0usize;
    'rooms: for room in fights {
        run.phase = Phase::Exploring;
        run.enter_for_test(room);
        if run.phase != Phase::Fighting {
            continue;
        }
        let mut after = 0;
        while after < 24 {
            if run.phase != Phase::Fighting {
                after += 1;
            }
            let inputs = party
                .iter()
                .filter(|(id, _)| run.players[id].hp > 0)
                .map(|&(id, _)| {
                    let (reach, ranged) = style(&run, id);
                    (id, bot_as(&run, id, reach, ranged))
                })
                .collect();
            run.step(&inputs);
            // Bruised, never felled: the film is the fight, not the wipe.
            for hero in run.players.values_mut() {
                hero.hp = hero.hp.max(hero.max_hp / 3);
            }
            save(&run, format!("party/{f:04}.ppm"));
            f += 1;
            if f >= 2400 {
                break 'rooms;
            }
        }
    }

    // One knight, before and after the wish: standing, loosing up at a
    // row of monsters that never fall.
    let mut run = Run::new(11, 4, None);
    run.outfit(1, knights::knight("percival").unwrap());
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    let row = [16.0, 20.0, 24.0, 28.0, 32.0];
    for (enemy, x) in run.enemies.iter_mut().zip(row.iter().cycle()) {
        enemy.hp = 99_999;
        (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (*x, 9.0, *x, 9.0);
    }
    let mut marks = String::new();
    for f in 0..(15 * HZ as usize) {
        if f == 4 * HZ as usize || f == 9 * HZ as usize || f == 11 * HZ as usize {
            grant(&mut run, 1, "volley");
            run.window = None;
            marks.push_str(&format!("{f} volley\n"));
        }
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y, hero.hp) = (24.0, 22.0, hero.max_hp);
        run.step(&BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                fire: true,
                ..Default::default()
            },
        )]));
        save(&run, format!("wish/{f:04}.ppm"));
    }
    std::fs::write(dir.join("wish/marks.txt"), marks).unwrap();
}

/// Film plates: `ANGEL_ARENA_SHOTS=<dir> cargo test write_fun_film -- --ignored`.
/// Scripted fun moments, the room's own pixels one tick a frame: a boss
/// felled by a sword swing (`boss/`), a dive roll through a bullet storm
/// (`dive/`), three morningstars shredding a pack (`star/`), a triple-wide
/// chained volley wiping a wave (`volley/`). Each folder's `marks.txt` notes
/// the beat to cut on.
#[test]
#[ignore]
fn write_fun_film() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let save = |run: &Run, sub: &str, f: usize| {
        let img = crate::stage::world_viz::overworld::arena::frame_for(run, 384, 224, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(sub).join(format!("{f:04}.ppm")), out).unwrap();
        // Where the action is, as a fraction of the room, for the film's camera.
        // On-screen pixels: eight to an arena unit, less the game's camera.
        let (ox, oy) = crate::stage::world_viz::overworld::arena::camera(run, Some(1));
        let px = |x: f32, y: f32| (x * 8.0 - ox as f32, y * 8.0 - oy as f32);
        let hero = run.players.get(&1).map_or((192.0, 112.0), |h| px(h.x, h.y));
        let boss = run
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Boss && e.hp > 0)
            .map(|e| {
                let (x, y) = px(e.x, e.y);
                format!("{x:.1},{y:.1}")
            })
            .unwrap_or_default();
        let line = format!("{f} {:.1},{:.1} {boss}\n", hero.0, hero.1);
        let path = dir.join(sub).join("pos.txt");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        text.push_str(&line);
        std::fs::write(path, text).unwrap();
    };
    let grant = |run: &mut Run, id: u32, wish: &str| {
        run.window = Some(Default::default());
        run.grant(id, &phrasebook::get(wish).unwrap()).unwrap();
        run.window = None;
    };
    let party = [(1, "galahad"), (2, "percival"), (3, "lynette"), (4, "gareth")];
    let heal = |run: &mut Run, floor: u32| {
        for hero in run.players.values_mut() {
            hero.hp = hero.hp.max(hero.max_hp * floor / 100);
        }
    };

    // The boss: the party fights the floor guardian; Galahad lands the blow.
    std::fs::create_dir_all(dir.join("boss")).unwrap();
    let mut run = Run::new(3, 9, Some("Matt"));
    run.join_seat(3, "Bree");
    run.join_seat(4, "Ana");
    for (id, who) in party {
        run.outfit(id, knights::knight(who).unwrap());
    }
    grant(&mut run, 2, "volley");
    grant(&mut run, 3, "chain");
    let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(stairs);
    let mut marks = String::new();
    let mut f = 0usize;
    let mut finisher = None;
    while f < 30 * 14 {
        let boss = run.enemies.iter().position(|e| e.kind == EnemyKind::Boss && e.hp > 0);
        let mut inputs: BTreeMap<u32, Input> = party
            .iter()
            .filter(|(id, _)| run.players[id].hp > 0)
            .map(|&(id, _)| {
                let (reach, ranged) = style(&run, id);
                (id, bot_as(&run, id, reach, ranged))
            })
            .collect();
        if let Some(b) = boss {
            let (bx, by) = (run.enemies[b].x, run.enemies[b].y);
            // After eight seconds the guardian is nearly spent; Galahad closes in.
            if f == 30 * 8 {
                let e = &mut run.enemies[b];
                e.hp = e.hp.min(60);
                marks.push_str(&format!("{f} closing\n"));
            }
            if f >= 30 * 8 {
                let hero = &run.players[&1];
                let (dx, dy) = (bx - hero.x, by - hero.y);
                let d = dx.hypot(dy);
                let sign = |v: f32| if v > 0.4 { 1 } else if v < -0.4 { -1 } else { 0 };
                inputs.insert(
                    1,
                    Input {
                        move_x: if d > 2.2 { sign(dx) } else { 0 },
                        move_y: if d > 2.2 { sign(dy) } else { 0 },
                        aim_x: sign(dx),
                        aim_y: sign(dy),
                        swing: d <= 2.6,
                        fire: d <= 2.6,
                        ..Default::default()
                    },
                );
                // Only the sword may finish it.
                for id in [2, 3, 4] {
                    if let Some(i) = inputs.get_mut(&id) {
                        i.fire = false;
                    }
                }
            }
        } else if finisher.is_none() {
            finisher = Some(f);
            marks.push_str(&format!("{f} felled\n"));
        }
        run.step(&inputs);
        heal(&mut run, 40);
        save(&run, "boss", f);
        f += 1;
        if finisher.is_some_and(|k| f > k + 60) {
            break;
        }
    }
    std::fs::write(dir.join("boss/marks.txt"), marks).unwrap();

    // The dive: rolling through a storm of embers and orbs.
    std::fs::create_dir_all(dir.join("dive")).unwrap();
    let mut run = Run::new(5, 10, None);
    run.outfit(1, knights::knight("percival").unwrap());
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    for (k, x, y) in [(EnemyKind::Imp, 10.0, 8.0), (EnemyKind::Imp, 38.0, 8.0), (EnemyKind::Wraith, 24.0, 6.0), (EnemyKind::Imp, 24.0, 20.0)] {
        run.spawn_at_for_test(k, x, y);
    }
    for e in &mut run.enemies {
        e.hp = 99_999;
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (8.0, 22.0);
    let mut marks = String::new();
    for f in 0..(30 * 9) {
        let phase = f % 45;
        let input = Input {
            move_x: 1,
            move_y: if (f / 45) % 2 == 0 { -1 } else { 1 },
            aim_y: -1,
            fire: true,
            dash: phase == 10,
            ..Default::default()
        };
        if phase == 10 {
            marks.push_str(&format!("{f} dive\n"));
        }
        run.step(&BTreeMap::from([(1, input)]));
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp;
        if hero.x > 40.0 {
            hero.x = 8.0;
        }
        save(&run, "dive", f);
    }
    std::fs::write(dir.join("dive/marks.txt"), marks).unwrap();

    // The morningstars: three of them shred a pack.
    std::fs::create_dir_all(dir.join("star")).unwrap();
    let mut run = Run::new(6, 11, None);
    run.outfit(1, knights::knight("galahad").unwrap());
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    for i in 0..10 {
        let (x, y) = (14.0 + (i % 5) as f32 * 4.5, 8.0 + (i / 5) as f32 * 4.0);
        run.spawn_at_for_test(if i % 2 == 0 { EnemyKind::Skeleton } else { EnemyKind::Bat }, x, y);
    }
    for _ in 0..3 {
        grant(&mut run, 1, "morningstar");
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 24.0);
    for f in 0..(30 * 8) {
        let target = run.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
            let h = &run.players[&1];
            (a.x - h.x).hypot(a.y - h.y).total_cmp(&(b.x - h.x).hypot(b.y - h.y))
        });
        let input = match target {
            Some(t) => {
                let h = &run.players[&1];
                let s = |v: f32| if v > 0.5 { 1 } else if v < -0.5 { -1 } else { 0 };
                Input { move_x: s(t.x - h.x), move_y: s(t.y - h.y), ..Default::default() }
            }
            None => Input::default(),
        };
        run.step(&BTreeMap::from([(1, input)]));
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp;
        save(&run, "star", f);
    }

    // The volley: triple-wide, chained, into a charging wave.
    std::fs::create_dir_all(dir.join("volley")).unwrap();
    let mut run = Run::new(7, 12, None);
    run.outfit(1, knights::knight("percival").unwrap());
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    for _ in 0..3 {
        grant(&mut run, 1, "volley");
    }
    grant(&mut run, 1, "chain");
    grant(&mut run, 1, "chain");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 24.0);
    for f in 0..(30 * 8) {
        if f % 40 == 0 {
            for i in 0..7 {
                run.spawn_at_for_test(EnemyKind::Skeleton, 9.0 + i as f32 * 5.0, 5.0);
            }
        }
        run.step(&BTreeMap::from([(1, Input { aim_y: -1, fire: true, ..Default::default() })]));
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y, hero.hp) = (24.0, 24.0, hero.max_hp);
        save(&run, "volley", f);
    }
}

/// Film plates: `ANGEL_ARENA_SHOTS=<dir> cargo test write_posse_film -- --ignored`.
/// The posse in the Delve: the whole company lined up (`lineup.ppm`), and
/// the Composer, Dispatcher, Money and Scryglass knights felling a floor
/// guardian (`posse/`, with `pos.txt` for the film's camera).
#[test]
#[ignore]
fn write_posse_film() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(dir.join("posse")).unwrap();
    let ppm = |img: &crate::stage::world_viz::overworld::Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        out
    };
    // The line-up: every knight of the company, in their colours.
    let mut sheet = crate::stage::world_viz::overworld::Img::new(16 * 11, 24);
    for (i, k) in knights::COMPANY.iter().enumerate() {
        sheet.stamp(&crate::stage::world_viz::overworld::arena::sprites::knight_in(k.colours), 4 + i as i32 * 17, 4);
    }
    std::fs::write(dir.join("lineup.ppm"), ppm(&sheet)).unwrap();

    let party = [(1, "composer"), (2, "dispatcher"), (3, "money"), (4, "scryglass")];
    let mut run = Run::new(3, 9, Some("Matt"));
    run.join_seat(3, "Bree");
    run.join_seat(4, "Ana");
    for (id, who) in party {
        run.outfit(id, knights::knight(who).unwrap());
    }
    let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    run.enter_for_test(stairs);
    let mut pos = String::new();
    for f in 0..(30 * 12) {
        let inputs = party
            .iter()
            .filter(|(id, _)| run.players[id].hp > 0)
            .map(|&(id, _)| {
                let (reach, ranged) = style(&run, id);
                (id, bot_as(&run, id, reach, ranged))
            })
            .collect();
        run.step(&inputs);
        for hero in run.players.values_mut() {
            hero.hp = hero.hp.max(hero.max_hp / 2);
        }
        let img = crate::stage::world_viz::overworld::arena::frame_for(&run, 384, 224, Some(1));
        std::fs::write(dir.join(format!("posse/{f:04}.ppm")), ppm(&img)).unwrap();
        let (ox, oy) = crate::stage::world_viz::overworld::arena::camera(&run, Some(1));
        let h = &run.players[&1];
        pos.push_str(&format!("{f} {:.1},{:.1}\n", h.x * 8.0 - ox as f32, h.y * 8.0 - oy as f32));
    }
    std::fs::write(dir.join("posse/pos.txt"), pos).unwrap();
}

/// The night's playtest film: `ANGEL_ARENA_SHOTS=<dir> cargo test
/// write_night_film -- --ignored`. One knight's evening, one frame a tick:
/// home in the Undercroft, Tobbin's forge lit, Fortune's wheel spun before
/// the audience, the Winding Stair, and a fight with an ultimate and an
/// achievement. Frames land in `<dir>/night/NNNNN.ppm`.
#[test]
#[ignore]
fn write_night_film() {
    use crate::drive::together_realm::{Spoil, Spoils};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("night");
    std::fs::create_dir_all(&dir).unwrap();
    let mut frame_no = 0u32;
    let mut save = |run: &Run| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 768, 448);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{frame_no:05}.ppm")), out).unwrap();
        frame_no += 1;
    };
    // Walk knight one toward a spot, a tick a frame.
    let walk = |run: &mut Run, (tx, ty): (f32, f32), fire: bool| -> Input {
        let hero = &run.players[&1];
        let step = |d: f32| if d > 0.4 { 1 } else if d < -0.4 { -1 } else { 0 };
        Input {
            move_x: step(tx - hero.x),
            move_y: step(ty - hero.y),
            fire,
            ..Default::default()
        }
    };
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 700);
    treasury.add(Spoil::Ore, 12);
    treasury.add(Spoil::Gem, 6);
    let mut run = Run::at_home(23, 1, None, home::Home::default(), treasury.clone());
    run.begin_in(Pack::Cavern);
    run.outfit(1, knights::knight("gareth").unwrap());
    // Home: a breath, then to the forge's plate and hold F.
    for _ in 0..20 {
        run.step(&BTreeMap::new());
        save(&run);
    }
    let forge = home::SPOTS[0].plate;
    let plate = ((forge.0 as f32 + 1.0) * 2.0, (forge.1 as f32 + 0.5) * 2.0);
    for _ in 0..70 {
        let input = walk(&mut run, plate, false);
        run.step(&BTreeMap::from([(1, input)]));
        save(&run);
    }
    for _ in 0..(home::BUY_HOLD + 2) {
        run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
        save(&run);
    }
    // The cockpit pays: Edge I, and the forge lights.
    let mut built = run.home.clone();
    built.buy(home::Station::Forge, &mut treasury).unwrap();
    run.rebuild_home(built, treasury);
    run.orders.clear();
    for _ in 0..30 {
        run.step(&BTreeMap::new());
        save(&run);
    }
    // Follow waypoints, a tick a frame, until `done` or the ticks run out.
    let path = |run: &mut Run,
                    points: &[(f32, f32)],
                    ticks: u32,
                    done: &dyn Fn(&Run) -> bool,
                    save: &mut dyn FnMut(&Run)| {
        let mut next = 0;
        for _ in 0..ticks {
            if done(run) || next >= points.len() {
                break;
            }
            let (tx, ty) = points[next];
            let hero = &run.players[&1];
            if (hero.x - tx).abs() < 0.6 && (hero.y - ty).abs() < 0.6 {
                next += 1;
                continue;
            }
            let input = walk(run, (tx, ty), false);
            run.step(&BTreeMap::from([(1, input)]));
            save(run);
        }
    };
    // Round the stairwell and east through the door to Fortune's hall.
    path(
        &mut run,
        &[(8.0, 20.0), (40.0, 20.0), (40.0, 14.0), (47.8, 14.0), (47.9, 14.0)],
        400,
        &|run| run.room().kind == RoomKind::Fortune,
        &mut save,
    );
    let lever = fortune::LEVER;
    let at_lever = ((lever.0 as f32 + 1.0) * 2.0, (lever.1 as f32 + 0.5) * 2.0);
    path(&mut run, &[(10.0, 15.0), at_lever], 200, &|_| false, &mut save);
    for _ in 0..(home::BUY_HOLD + 2) {
        run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
        save(&run);
    }
    for _ in 0..(fortune::SPIN_TICKS + 3 * HZ) {
        run.step(&BTreeMap::new());
        save(&run);
    }
    // Back west, round the stairwell, and down the Winding Stair.
    path(
        &mut run,
        &[(10.0, 15.0), (2.0, 14.0), (0.2, 14.0), (0.1, 14.0)],
        300,
        &|run| run.room().kind == RoomKind::Home,
        &mut save,
    );
    path(
        &mut run,
        &[(40.0, 14.0), (40.0, 20.0), (24.0, 20.0), (24.0, 17.0), (24.0, 13.0), (24.0, 12.5)],
        400,
        &|run| !run.at_home_now(),
        &mut save,
    );
    // Floor one: the bot fights; when its charge fills, Trebuchet.
    let fights: Vec<usize> = run
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RoomKind::Fight && !r.great())
        .map(|(i, _)| i)
        .collect();
    if let Some(&room) = fights.first() {
        run.enter_for_test(room);
        run.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL - 120;
        for _ in 0..(40 * HZ) {
            if run.phase != Phase::Fighting {
                break;
            }
            let mut input = bot(&run, 12.0, true);
            input.ult = run.players[&1].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
            let hero = run.players.get_mut(&1).unwrap();
            hero.hp = hero.hp.max(40);
            run.step(&BTreeMap::from([(1, input)]));
            save(&run);
        }
        run.announce("my_ult");
        for _ in 0..(4 * HZ) {
            run.step(&BTreeMap::new());
            save(&run);
        }
    }
    eprintln!("NIGHT FILM {frame_no} frames, mode {:?}", run.mode);
}

/// The night's second film, a tick a frame: the Undercroft's new corners
/// (Wren's board, the Trophy Hall), then down the Winding Stair to the
/// Fungal Deep, where the Flesher hooks and a fan box falls, and a cut to
/// the Archive's brood.
/// `ANGEL_ARENA_SHOTS=dir cargo test write_deep_film -- --ignored`
#[test]
#[ignore]
fn write_deep_film() {
    use crate::drive::together_shooter::bounties::Pinned;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("deep");
    std::fs::create_dir_all(&dir).unwrap();
    let mut frame_no = 0u32;
    let mut save = |run: &Run| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 768, 448);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{frame_no:05}.ppm")), out).unwrap();
        frame_no += 1;
    };
    let walk = |run: &Run, (tx, ty): (f32, f32)| -> Input {
        let hero = &run.players[&1];
        let step = |d: f32| {
            if d > 0.4 {
                1
            } else if d < -0.4 {
                -1
            } else {
                0
            }
        };
        Input {
            move_x: step(tx - hero.x),
            move_y: step(ty - hero.y),
            ..Default::default()
        }
    };
    let path = |run: &mut Run,
                points: &[(f32, f32)],
                ticks: u32,
                done: &dyn Fn(&Run) -> bool,
                save: &mut dyn FnMut(&Run)| {
        let mut next = 0;
        for _ in 0..ticks {
            if done(run) || next >= points.len() {
                break;
            }
            let (tx, ty) = points[next];
            let hero = &run.players[&1];
            if (hero.x - tx).abs() < 0.6 && (hero.y - ty).abs() < 0.6 {
                next += 1;
                continue;
            }
            let input = walk(run, (tx, ty));
            run.step(&BTreeMap::from([(1, input)]));
            save(run);
        }
    };
    let idle = |run: &mut Run, ticks: u32, save: &mut dyn FnMut(&Run)| {
        for _ in 0..ticks {
            run.step(&BTreeMap::new());
            save(run);
        }
    };
    // A realm some way along: a built home, three bounties, trophies.
    let mut home = home::Home {
        deepest: 5,
        landing: 5,
        ..Default::default()
    };
    for (station, level) in [
        (home::Station::Forge, 3),
        (home::Station::Hearth, 2),
        (home::Station::Rack, 2),
        (home::Station::Chapel, 1),
        (home::Station::Map, 4),
    ] {
        home.levels.insert(station, level);
    }
    home.bounties = vec![
        Pinned {
            id: "fleshers".into(),
            have: 1,
        },
        Pinned {
            id: "ratings".into(),
            have: 1240,
        },
        Pinned {
            id: "spiders".into(),
            have: 22,
        },
    ];
    for (id, n) in [
        ("waxen-warden", 11),
        ("cinderjaw", 6),
        ("the-index", 2),
        ("dragon", 4),
    ] {
        home.trophies.insert(id.into(), n);
    }
    let mut run = Run::at_home(31, 1, None, home, Default::default());
    run.begin_in(Pack::Crypt);
    run.outfit(1, knights::knight("percival").unwrap());
    idle(&mut run, 20, &mut save);
    // Wren's board, read.
    path(
        &mut run,
        &[(30.0, 23.5), (33.0, 21.6)],
        120,
        &|_| false,
        &mut save,
    );
    idle(&mut run, 75, &mut save);
    // Down the runner to the Trophy Hall.
    path(
        &mut run,
        &[(24.0, 24.0), (24.0, 27.9), (24.0, 28.5)],
        160,
        &|run| run.room().kind == RoomKind::Trophies,
        &mut save,
    );
    path(
        &mut run,
        &[(24.0, 7.0), (12.0, 9.0), (10.0, 9.5)],
        200,
        &|_| false,
        &mut save,
    );
    idle(&mut run, 45, &mut save);
    path(
        &mut run,
        &[(17.0, 9.0), (17.0, 15.5), (24.0, 15.5)],
        200,
        &|_| false,
        &mut save,
    );
    idle(&mut run, 45, &mut save);
    // Back up, and down the Winding Stair to the Fungal Deep.
    path(
        &mut run,
        &[
            (17.0, 15.5),
            (17.0, 8.0),
            (24.0, 4.0),
            (24.0, 1.0),
            (24.0, 0.2),
        ],
        300,
        &|run| run.room().kind == RoomKind::Home,
        &mut save,
    );
    path(
        &mut run,
        &[(24.0, 20.0), (24.0, 17.0), (24.0, 13.0)],
        300,
        &|run| !run.at_home_now(),
        &mut save,
    );
    for _ in 0..80 {
        if !run.at_home_now() {
            break;
        }
        run.step(&BTreeMap::new());
        save(&run);
    }
    // A fight room of the Fungal Deep, with the Flesher in it.
    let fights: Vec<usize> = run
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RoomKind::Fight && !r.great())
        .map(|(i, _)| i)
        .collect();
    let fight = |run: &mut Run, seconds: u32, save: &mut dyn FnMut(&Run)| {
        for t in 0..seconds * HZ {
            if run.phase != Phase::Fighting {
                break;
            }
            let (reach, ranged) = style(run, 1);
            let mut input = bot_as(run, 1, reach, ranged);
            input.ult = run.players[&1].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
            let hero = run.players.get_mut(&1).unwrap();
            hero.hp = hero.hp.max(50);
            if t == 3 * HZ {
                run.thrill_for_test(audience::MILESTONE);
            }
            run.step(&BTreeMap::from([(1, input)]));
            save(run);
        }
    };
    if let Some(&room) = fights.first() {
        run.enter_for_test(room);
        run.calm_for_test();
        run.enemies.clear();
        for kind in [
            EnemyKind::Flesher,
            EnemyKind::Bat,
            EnemyKind::Bat,
            EnemyKind::Imp,
        ] {
            run.spawn(kind);
        }
        run.phase = Phase::Fighting;
        fight(&mut run, 22, &mut save);
        idle(&mut run, 40, &mut save);
    }
    // A cut to the Drowned Archive, and the Silkmother.
    let mut run = Run::new(31, 1, None);
    run.outfit(1, knights::knight("percival").unwrap());
    for _ in 1..4 {
        run.descend_for_test();
    }
    let rooms: Vec<usize> = (0..run.dungeon.rooms.len())
        .filter(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight && !run.dungeon.rooms[i].great())
        .collect();
    if let Some(&room) = rooms.first() {
        run.enter_for_test(room);
        // The cut comes in mid-floor: no Sanctuary sign from the descent.
        run.hallowed = None;
        run.calm_for_test();
        run.enemies.clear();
        for kind in [
            EnemyKind::Silkmother,
            EnemyKind::Skeleton,
            EnemyKind::Wraith,
        ] {
            run.spawn(kind);
        }
        run.phase = Phase::Fighting;
        fight(&mut run, 14, &mut save);
    }
    eprintln!("DEEP FILM {frame_no} frames");
}

/// The night's third film, a tick a frame: Lady Tallow by the fire, a
/// lesson at Sir Ector's lectern, down the Winding Stair to Fortune's dare,
/// the Pit and its Lord (the slam, the fall, the Talisman, the cat fetching the
/// gold), and a cut to the Bone Choir.
/// `ANGEL_ARENA_SHOTS=dir cargo test write_third_film -- --ignored`
#[test]
#[ignore]
fn write_third_film() {
    use crate::drive::together_shooter::talents::{LECTERNS, LEVELS, Prowess, knight_key};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("third");
    std::fs::create_dir_all(&dir).unwrap();
    let mut frame_no = 0u32;
    let mut save = |run: &Run| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 768, 448);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{frame_no:05}.ppm")), out).unwrap();
        frame_no += 1;
    };
    let walk = |run: &Run, (tx, ty): (f32, f32), fire: bool| -> Input {
        let hero = &run.players[&1];
        let step = |d: f32| {
            if d > 0.4 {
                1
            } else if d < -0.4 {
                -1
            } else {
                0
            }
        };
        Input {
            move_x: step(tx - hero.x),
            move_y: step(ty - hero.y),
            fire,
            ..Default::default()
        }
    };
    let path = |run: &mut Run,
                points: &[(f32, f32)],
                ticks: u32,
                done: &dyn Fn(&Run) -> bool,
                save: &mut dyn FnMut(&Run)| {
        let mut next = 0;
        for _ in 0..ticks {
            if done(run) || next >= points.len() {
                break;
            }
            let (tx, ty) = points[next];
            let hero = &run.players[&1];
            if (hero.x - tx).abs() < 0.6 && (hero.y - ty).abs() < 0.6 {
                next += 1;
                continue;
            }
            let input = walk(run, (tx, ty), false);
            run.step(&BTreeMap::from([(1, input)]));
            save(run);
        }
    };
    let idle = |run: &mut Run, ticks: u32, save: &mut dyn FnMut(&Run)| {
        for _ in 0..ticks {
            run.step(&BTreeMap::new());
            save(run);
        }
    };
    // A realm some way along: a warm hearth (so Tallow comes), a map to the
    // second floor, a lesson waiting.
    let home_for = || {
        let mut home = home::Home {
            deepest: 3,
            landing: 2,
            ..Default::default()
        };
        for (station, level) in [
            (home::Station::Forge, 3),
            (home::Station::Hearth, 2),
            (home::Station::Rack, 2),
            (home::Station::Map, 1),
        ] {
            home.levels.insert(station, level);
        }
        home.knights.insert(
            "percival".into(),
            Prowess {
                xp: LEVELS[0] + 300,
                learned: Vec::new(),
            },
        );
        home
    };
    // A seed whose second floor has a pit.
    let seed = (0..200u64)
        .find(|&seed| {
            let mut run = Run::at_home(seed, 1, None, home_for(), Default::default());
            run.begin_in(Pack::Crypt);
            run.descend_for_test();
            run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Pit)
        })
        .expect("a second floor with a pit");
    let mut run = Run::at_home(seed, 1, None, home_for(), Default::default());
    run.begin_in(Pack::Crypt);
    run.outfit(1, knights::knight("percival").unwrap());
    idle(&mut run, 30, &mut save);
    // North to the Training Yard, round the stairwell, to Sir Ector.
    path(
        &mut run,
        &[(16.0, 23.5), (16.0, 3.0), (24.0, 2.0), (24.0, -2.0)],
        400,
        &|run| run.room().kind == RoomKind::Yard,
        &mut save,
    );
    let (c, r, w, _) = LECTERNS[0];
    let lectern = ((c as f32 + w as f32 / 2.0) * 2.0, (r as f32 + 0.5) * 2.0);
    path(
        &mut run,
        &[(10.0, 23.0), lectern],
        300,
        &|_| false,
        &mut save,
    );
    idle(&mut run, 20, &mut save);
    for _ in 0..(home::BUY_HOLD + 2) {
        run.step(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]));
        save(&run);
    }
    // The cockpit's part: Sir Ector teaches it, and the card is dealt.
    let mut learned = run.home.clone();
    let key = knight_key(&run.players[&1]);
    learned.learn(&key, 0).unwrap();
    let treasury = run.treasury.clone();
    run.rebuild_home(learned, treasury);
    run.orders.clear();
    idle(&mut run, 30, &mut save);
    // Back down through the Undercroft to the Winding Stair.
    path(
        &mut run,
        &[(24.0, 24.0), (24.0, 27.9), (24.0, 28.5)],
        300,
        &|run| run.room().kind == RoomKind::Home,
        &mut save,
    );
    path(
        &mut run,
        &[(16.0, 4.0), (16.0, 20.0), (24.0, 20.0), (24.0, 13.0)],
        400,
        &|run| !run.at_home_now(),
        &mut save,
    );
    for _ in 0..90 {
        if !run.at_home_now() {
            break;
        }
        run.step(&BTreeMap::new());
        save(&run);
    }
    // Fortune's dare drops in.
    idle(&mut run, 120, &mut save);
    // The Pit.
    let pit = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Pit)
        .expect("this floor's pit");
    run.enter_for_test(pit);
    for enemy in run
        .enemies
        .iter_mut()
        .filter(|e| e.kind == EnemyKind::PitTyrant)
    {
        // A film's fight: shorter than a real one.
        enemy.hp = enemy.max_hp / 3;
    }
    for _ in 0..(40 * HZ) {
        if run.phase != Phase::Fighting {
            break;
        }
        let (reach, ranged) = style(&run, 1);
        let mut input = bot_as(&run, 1, reach, ranged);
        input.ult = run.players[&1].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.hp.max(60);
        run.step(&BTreeMap::from([(1, input)]));
        save(&run);
    }
    // The Talisman, taken; Tallow fetches the gold.
    let talisman = run
        .room()
        .items
        .iter()
        .find(|i| i.card == "talisman")
        .map(|i| (i.x, i.y));
    if let Some(talisman) = talisman {
        path(
            &mut run,
            &[talisman],
            200,
            &|run| run.players[&1].talisman,
            &mut save,
        );
    }
    idle(&mut run, 150, &mut save);
    // A cut to the Crypt's stairs, where the Bone Choir sings.
    let choir = (0..64u64)
        .find_map(|raid| {
            let mut run = Run::new(9, raid, None);
            run.begin_in(Pack::Crypt);
            let stairs = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == RoomKind::Stairs)?;
            run.enter_for_test(stairs);
            let meets = run
                .enemies
                .iter()
                .find_map(|e| e.boss)
                .is_some_and(|i| run.bosses[usize::from(i)].id == "the-bone-choir");
            meets.then_some(run)
        })
        .expect("a delve that meets the Bone Choir");
    let mut run = choir;
    run.outfit(1, knights::knight("percival").unwrap());
    let (bx, by) = run
        .enemies
        .iter()
        .find(|e| e.boss.is_some())
        .map(|e| (e.x, e.y))
        .unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (bx, by + 10.0);
    for _ in 0..(10 * HZ) {
        let (reach, ranged) = style(&run, 1);
        let input = bot_as(&run, 1, reach, ranged);
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.hp.max(60);
        run.step(&BTreeMap::from([(1, input)]));
        save(&run);
    }
    eprintln!("THIRD FILM {frame_no} frames, seed {seed}");
}

/// The night's fourth film: the Undercroft with its rescued residents and
/// Tallow by the hearth; Fortune's wheel coming to rest on All Random; a
/// fight on floor one with a stranger's ultimate and a Double Damage rune
/// raced for; a Crypt room where the Hexer makes a frog of the knight; and a
/// cage opened in a treasure room.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_fourth_film -- --ignored`
#[test]
#[ignore]
fn write_fourth_film() {
    use crate::drive::together_shooter::fortune::{LEVER, Mode, SPIN_TICKS, Spin};
    use crate::drive::together_shooter::rescues::Captive;
    use crate::drive::together_shooter::runes::{Rune, RuneKind};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("fourth");
    std::fs::create_dir_all(&dir).unwrap();
    let mut frame_no = 0u32;
    let mut heard = 0usize;
    let mut place = (u64::MAX, u32::MAX, usize::MAX);
    let mut save = |run: &Run| {
        // Where the film's scenes and moments fall, for the cut list.
        let here = (run.seed, run.dungeon.depth, run.at);
        if here != place {
            eprintln!("FRAME {frame_no} room {:?}", run.room().kind);
            place = here;
        }
        heard = heard.min(run.cues.len());
        for cue in &run.cues[heard..] {
            if ["rune", "hexed", "ult:", "rescued", "wheel:", "all_random"]
                .iter()
                .any(|k| cue.starts_with(k))
            {
                eprintln!("FRAME {frame_no} {cue}");
            }
        }
        heard = run.cues.len();
        let img = crate::stage::world_viz::overworld::arena::frame(run, 768, 448);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{frame_no:05}.ppm")), out).unwrap();
        frame_no += 1;
    };
    let walk = |run: &Run, (tx, ty): (f32, f32)| -> Input {
        let hero = &run.players[&1];
        let step = |d: f32| {
            if d > 0.4 {
                1
            } else if d < -0.4 {
                -1
            } else {
                0
            }
        };
        Input {
            move_x: step(tx - hero.x),
            move_y: step(ty - hero.y),
            ..Default::default()
        }
    };
    let path = |run: &mut Run,
                points: &[(f32, f32)],
                ticks: u32,
                done: &dyn Fn(&Run) -> bool,
                save: &mut dyn FnMut(&Run)| {
        let mut next = 0;
        for _ in 0..ticks {
            if done(run) || next >= points.len() {
                break;
            }
            let (tx, ty) = points[next];
            let hero = &run.players[&1];
            if (hero.x - tx).abs() < 0.6 && (hero.y - ty).abs() < 0.6 {
                next += 1;
                continue;
            }
            let input = walk(run, (tx, ty));
            run.step(&BTreeMap::from([(1, input)]));
            save(run);
        }
    };
    let idle = |run: &mut Run, ticks: u32, save: &mut dyn FnMut(&Run)| {
        for _ in 0..ticks {
            run.step(&BTreeMap::new());
            save(run);
        }
    };
    let hold_f = |run: &mut Run, save: &mut dyn FnMut(&Run)| {
        for _ in 0..(home::BUY_HOLD + 2) {
            run.step(&BTreeMap::from([(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]));
            save(run);
        }
    };
    // A bot's fight, the knight kept on its feet for the film; `steer`
    // may take the keys over for a tick.
    let fight = |run: &mut Run,
                 ticks: u32,
                 steer: &dyn Fn(&Run, Input) -> Input,
                 save: &mut dyn FnMut(&Run)| {
        for _ in 0..ticks {
            if run.phase != Phase::Fighting {
                break;
            }
            let (reach, ranged) = style(run, 1);
            let mut input = bot_as(run, 1, reach, ranged);
            input.ult = run.players[&1].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
            let input = steer(run, input);
            let hero = run.players.get_mut(&1).unwrap();
            hero.hp = hero.hp.max(70);
            let known = run.feats.len();
            run.step(&BTreeMap::from([(1, input)]));
            // The cockpit's part: a new achievement gets its banner.
            let newly_unlocked = run.feats[known..].to_vec();
            for id in newly_unlocked {
                run.announce(id);
            }
            save(run);
        }
    };
    // A realm some way along: the hearth warm (Tallow comes), Fortune's
    // third rung bought, all four rescued and at home.
    let mut built = home::Home {
        deepest: 3,
        ..Default::default()
    };
    for (station, level) in [
        (home::Station::Forge, 2),
        (home::Station::Hearth, 2),
        (home::Station::Wheel, 3),
    ] {
        built.levels.insert(station, level);
    }
    for who in ["mabel", "anselm", "pip", "maud"] {
        built.residents.insert(who.into());
    }
    let mut run = Run::at_home(23, 1, None, built, Default::default());
    run.begin_in(Pack::Cavern);
    run.outfit(1, knights::knight("percival").unwrap());
    idle(&mut run, 40, &mut save);
    // Past Mabel at her pot, then east to Fortune's hall.
    let w = run.room().width();
    path(
        &mut run,
        &[(19.0, 25.0), (30.0, 24.0), (w - 3.0, 14.0), (w + 1.0, 14.0)],
        500,
        &|run| run.room().kind == RoomKind::Fortune,
        &mut save,
    );
    let (c, r, lw, lh) = LEVER;
    let lever = (
        (c as f32 + lw as f32 / 2.0) * TILE_UNITS,
        (r as f32 + lh as f32 / 2.0) * TILE_UNITS,
    );
    path(
        &mut run,
        &[(12.0, lever.1), lever],
        400,
        &|_| false,
        &mut save,
    );
    hold_f(&mut run, &mut save);
    // Fortune's wheel, for the film, comes to rest on All Random.
    if let Some(spin) = run.spin {
        let wedge = Mode::WHEEL
            .iter()
            .position(|&m| m == Mode::AllRandom)
            .unwrap() as u8;
        run.spin = Some(Spin { wedge, ..spin });
    }
    idle(&mut run, SPIN_TICKS + 3 * HZ, &mut save);
    // Down the stair: a stranger's ultimate, half charged, and a fight.
    run.descend_for_test();
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL - 120;
    let (rw, rh) = (run.room().width(), run.room().height());
    run.rune = Some(Rune {
        kind: RuneKind::DoubleDamage,
        x: rw * 0.5,
        y: rh * 0.5,
        at: run.tick + 2 * u64::from(HZ),
    });
    fight(
        &mut run,
        10 * HZ,
        &|run, input| match run.rune {
            // Race for the rune once it is up.
            Some(rune) if run.tick >= rune.at => Input {
                fire: input.fire,
                aim_x: input.aim_x,
                aim_y: input.aim_y,
                ..walk(run, (rune.x, rune.y))
            },
            _ => input,
        },
        &mut save,
    );
    idle(&mut run, HZ, &mut save);
    // A cut to a Crypt room open enough to see across, where the Hexer
    // waits.
    let mut run = Run::new(4, 1, None);
    run.begin_in(Pack::Crypt);
    run.outfit(1, knights::knight("percival").unwrap());
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.rune = None;
    run.hallowed = None;
    run.enemies.clear();
    // Open floor near each wanted spot: nobody starts inside a tomb.
    let open = |run: &Run, (x, y): (f32, f32)| -> (f32, f32) {
        let room = run.room();
        let (c0, r0) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
        let clear = |c: i32, r: i32| {
            [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .all(|&(dc, dr)| room.tile(c + dc, r + dr) == Tile::Floor)
        };
        (0..8)
            .flat_map(|ring| {
                (-ring..=ring).flat_map(move |dc| (-ring..=ring).map(move |dr| (dc, dr)))
            })
            .map(|(dc, dr)| (c0 + dc, r0 + dr))
            .find(|&(c, r)| clear(c, r))
            .map_or((x, y), |(c, r)| {
                ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS)
            })
    };
    let (hx, hy) = open(&run, (32.0, 12.0));
    run.spawn_at_for_test(EnemyKind::Hexer, hx, hy);
    if let Some(hexer) = run.enemies.last_mut() {
        // A film's witch: she lasts long enough to be seen at work.
        (hexer.hp, hexer.max_hp) = (420, 420);
    }
    let (sx, sy) = open(&run, (38.0, 21.0));
    run.spawn_at_for_test(EnemyKind::Skeleton, sx, sy);
    let (kx, ky) = open(&run, (14.0, 14.0));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (kx, ky);

    // For the film the knight doesn't see the green bolt coming: no roll,
    // and a step the wrong way as it closes.
    fight(
        &mut run,
        16 * HZ,
        &|run, input| {
            let hero = &run.players[&1];
            let closing = run
                .projectiles
                .iter()
                .find(|p| p.kind == Shot::Hex && (p.x - hero.x).hypot(p.y - hero.y) < 14.0);
            if let Some(bolt) = closing.filter(|_| hero.hexed == 0) {
                Input {
                    fire: input.fire,
                    aim_x: input.aim_x,
                    aim_y: input.aim_y,
                    ..walk(run, (bolt.x, bolt.y))
                }
            } else {
                Input {
                    dash: false,
                    ..input
                }
            }
        },
        &mut save,
    );
    idle(&mut run, 4 * HZ, &mut save);
    // A treasure room with a cage, and Brother Anselm in it.
    let mut run = Run::new(4, 1, None);
    run.begin_in(Pack::Crypt);
    run.outfit(1, knights::knight("percival").unwrap());
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Treasure)
        .unwrap();
    run.enter_for_test(room);
    let (x, y) = (run.room().width() / 2.0, run.room().height() / 2.0 - 6.0);
    run.captive = Some(Captive {
        who: "anselm".into(),
        room,
        x,
        y,
        freed: None,
    });
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x - 10.0, y + 9.0);
    idle(&mut run, 20, &mut save);
    path(&mut run, &[(x, y + 2.4)], 200, &|_| false, &mut save);
    hold_f(&mut run, &mut save);
    idle(&mut run, 3 * HZ, &mut save);
    eprintln!("FOURTH FILM {frame_no} frames");
}

/// The fifth film, session two's: through the Trophy Hall into Maud's
/// tavern, a stout poured at her tap, the rumour board read, a knight put
/// out of the Siege Perilous; the Lich's Rimeleap going round a pair in
/// the Drowned Archive; and on a Hollow Walls floor, a bomb, a wall coming
/// down, and the vault behind it.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_fifth_film -- --ignored`
#[test]
#[ignore]
fn write_fifth_film() {
    use crate::drive::together_shooter::fortune::Mode;
    use crate::drive::together_shooter::tavern::{BOARD, SIEGE, TAPS};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("fifth");
    std::fs::create_dir_all(&dir).unwrap();
    let mut frame_no = 0u32;
    let mut heard = 0usize;
    let mut place = (u64::MAX, u32::MAX, usize::MAX);
    let mut save = |run: &Run| {
        let here = (run.seed, run.dungeon.depth, run.at);
        if here != place {
            eprintln!("FRAME {frame_no} room {:?}", run.room().kind);
            place = here;
        }
        heard = heard.min(run.cues.len());
        for cue in &run.cues[heard..] {
            if [
                "dug",
                "bar",
                "round",
                "siege",
                "rimeleap",
                "frost_leap",
                "crack",
                "secret",
                "snibbet",
            ]
            .iter()
            .any(|k| cue.starts_with(k))
            {
                eprintln!("FRAME {frame_no} {cue}");
            }
        }
        heard = run.cues.len();
        let img = crate::stage::world_viz::overworld::arena::frame(run, 768, 448);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{frame_no:05}.ppm")), out).unwrap();
        frame_no += 1;
    };
    let walk = |run: &Run, id: u32, (tx, ty): (f32, f32)| -> Input {
        let hero = &run.players[&id];
        let step = |d: f32| {
            if d > 0.4 {
                1
            } else if d < -0.4 {
                -1
            } else {
                0
            }
        };
        Input {
            move_x: step(tx - hero.x),
            move_y: step(ty - hero.y),
            ..Default::default()
        }
    };
    let path = |run: &mut Run,
                points: &[(f32, f32)],
                ticks: u32,
                done: &dyn Fn(&Run) -> bool,
                save: &mut dyn FnMut(&Run)| {
        let mut next = 0;
        for _ in 0..ticks {
            if done(run) || next >= points.len() {
                break;
            }
            let (tx, ty) = points[next];
            let hero = &run.players[&1];
            if (hero.x - tx).abs() < 0.6 && (hero.y - ty).abs() < 0.6 {
                next += 1;
                continue;
            }
            let input = walk(run, 1, (tx, ty));
            run.step(&BTreeMap::from([(1, input)]));
            save(run);
        }
    };
    let idle = |run: &mut Run, ticks: u32, save: &mut dyn FnMut(&Run)| {
        for _ in 0..ticks {
            run.step(&BTreeMap::new());
            save(run);
        }
    };
    let centre = |(c, r, w, h): (i32, i32, i32, i32)| {
        (
            (c as f32 + w as f32 / 2.0) * TILE_UNITS,
            (r as f32 + h as f32 / 2.0) * TILE_UNITS,
        )
    };
    // A realm that has dug the west wing and brought Maud home.
    let mut built = home::Home {
        deepest: 4,
        ..Default::default()
    };
    for (station, level) in [
        (home::Station::Forge, 2),
        (home::Station::Hearth, 2),
        (home::Station::Wing, 1),
    ] {
        built.levels.insert(station, level);
    }
    for who in ["mabel", "anselm", "pip", "maud"] {
        built.residents.insert(who.into());
    }
    let mut run = Run::at_home(29, 1, None, built, Default::default());
    run.begin_in(Pack::Crypt);
    run.outfit(1, knights::knight("percival").unwrap());
    idle(&mut run, 20, &mut save);
    // South down the runner to the Trophy Hall, then west into the tavern.
    let hall = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Trophies)
        .unwrap();
    path(
        &mut run,
        &[(24.0, 26.0), (24.0, 28.6)],
        300,
        &|run| run.at == hall,
        &mut save,
    );
    // Round the dragon's plinth in the middle, then west along the
    // doorway's row.
    path(
        &mut run,
        &[(17.0, 4.5), (17.0, 13.0), (4.0, 13.0), (-0.6, 13.0)],
        600,
        &|run| run.room().kind == RoomKind::Tavern,
        &mut save,
    );
    // Up to the rumour board, and a moment to read it.
    let (bx, by) = centre(BOARD);
    path(&mut run, &[(bx, by + 3.2)], 300, &|_| false, &mut save);
    idle(&mut run, 3 * HZ, &mut save);
    // A stout at Maud's middle tap (the cockpit's part: she pours it).
    let tap = centre(TAPS[1]);
    path(
        &mut run,
        &[(tap.0, tap.1 + 1.0), tap],
        300,
        &|_| false,
        &mut save,
    );
    for _ in 0..(home::BUY_HOLD + 2) {
        run.step(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]));
        save(&run);
    }
    let mut poured = run.home.clone();
    poured.round = Some("stout".into());
    let treasury = run.treasury.clone();
    run.rebuild_home(poured, treasury);
    run.orders.clear();
    idle(&mut run, HZ, &mut save);
    // And then, against all advice, the Siege Perilous.
    let chair = centre(SIEGE);
    // Between the tables and under them, to the corner.
    path(
        &mut run,
        &[(20.0, 12.0), (20.0, 21.0), (chair.0 + 3.0, 21.0), chair],
        500,
        &|_| false,
        &mut save,
    );
    idle(&mut run, 3 * HZ, &mut save);
    // A cut to the Drowned Archive: the Lich, and a pair standing too close.
    let mut run = Run::new(6, 1, Some("Friend"));
    run.begin_in(Pack::Crypt);
    run.outfit(1, knights::knight("percival").unwrap());
    run.outfit(2, knights::knight("lynette").unwrap());
    for _ in 0..3 {
        run.descend_for_test();
    }
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.rune = None;
    run.hallowed = None;
    run.spawn_at_for_test(EnemyKind::Lich, 33.0, 13.0);
    if let Some(lich) = run.enemies.last_mut() {
        // A film's Lich: he lasts long enough to be seen at work.
        (lich.hp, lich.max_hp) = (900, 900);
    }
    for (id, (x, y)) in [(1, (14.0, 11.0)), (2, (15.0, 17.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (x, y, 0);
    }
    for _ in 0..9 * HZ {
        for hero in run.players.values_mut() {
            hero.hp = hero.hp.max(60);
        }
        run.step(&BTreeMap::new());
        save(&run);
    }
    // A cut to a Hollow Walls floor: a bomb, a wall, a vault.
    let mut run = Run::at_home(31, 1, None, home::Home::default(), Default::default());
    run.mode = Mode::HollowWalls;
    run.outfit(1, knights::knight("percival").unwrap());
    run.descend_for_test();
    let secret = run.dungeon.secret.expect("Hollow Walls: a vault");
    run.enter_for_test(secret.host);
    run.calm_for_test();
    run.rune = None;
    run.hallowed = None;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    let (_, cx, cy) = run.dungeon.crack().unwrap();
    let (dx, dy) = DIRS[secret.side];
    // The knight starts on the open floor before the crack (the doorway's
    // lane is clear, or the vault would not have been hung there).
    let before = (cx - dx as f32 * 6.0, cy - dy as f32 * 6.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = before;
    idle(&mut run, HZ, &mut save);
    run.step(&BTreeMap::from([(
        1,
        Input {
            bomb: true,
            ..Default::default()
        },
    )]));
    save(&run);
    idle(&mut run, 2 * HZ, &mut save);
    // Through the new doorway, into the vault.
    let door = (cx + dx as f32 * 2.0, cy + dy as f32 * 2.0);
    path(
        &mut run,
        &[door],
        300,
        &|run| run.room().kind == RoomKind::Secret,
        &mut save,
    );
    let (vw, vh) = (run.room().width(), run.room().height());
    path(
        &mut run,
        &[(vw / 2.0, vh / 2.0 + 3.0)],
        300,
        &|_| false,
        &mut save,
    );
    idle(&mut run, 3 * HZ, &mut save);
    eprintln!("FIFTH FILM {frame_no} frames");
}

/// The deep, by bot: every fight room, guardian and threshold from floor
/// one to six, one knight and then two, with the Undercroft built well
/// (Edge III, Hearth III, Rack II, Chapel I) and ultimates cast when full.
/// `cargo test deep_balance -- --ignored --nocapture`
#[test]
#[ignore]
fn deep_balance() {
    let mut built = home::Home::default();
    for (station, level) in [
        (home::Station::Forge, 3),
        (home::Station::Hearth, 3),
        (home::Station::Rack, 2),
        (home::Station::Chapel, 1),
    ] {
        built.levels.insert(station, level);
    }
    for party in [1u32, 2] {
        for depth in 1..=DEEPEST {
            // (rooms, won, hp lost, ticks, wiped) for fights, guardians, the
            // threshold.
            let mut tally = [(0u32, 0u32, 0u32, 0u64, 0u32); 3];
            let mut packs: Vec<&str> = Vec::new();
            // Fortune's audience over the floor, and its fan boxes.
            let (mut viewers, mut fans) = (0u32, 0u32);
            for seed in 0..8 {
                let mut run = Run::new(seed, 1, (party == 2).then_some("Friend"));
                run.rebuild_home(built.clone(), Default::default());
                run.outfit(1, knights::knight("percival").unwrap());
                if party == 2 {
                    run.outfit(2, knights::knight("lynette").unwrap());
                }
                for _ in 1..depth {
                    run.descend_for_test();
                }
                if !packs.contains(&run.dungeon.pack.name()) {
                    packs.push(run.dungeon.pack.name());
                }
                let rooms: Vec<(usize, usize)> = run
                    .dungeon
                    .rooms
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| match r.kind {
                        RoomKind::Fight => Some((i, 0)),
                        RoomKind::Stairs | RoomKind::Lair => Some((i, 1)),
                        RoomKind::Threshold => Some((i, 2)),
                        _ => None,
                    })
                    .collect();
                for (room, kind) in rooms {
                    for hero in run.players.values_mut() {
                        hero.hp = hero.max_hp;
                        hero.winds = 1;
                    }
                    run.phase = Phase::Exploring;
                    run.enter_for_test(room);
                    if run.phase != Phase::Fighting {
                        continue;
                    }
                    let start = run.tick;
                    while run.phase == Phase::Fighting && run.tick - start < u64::from(180 * HZ) {
                        let mut inputs = BTreeMap::new();
                        for &id in run.players.keys() {
                            let (reach, ranged) = style(&run, id);
                            let mut input = bot_as(&run, id, reach, ranged);
                            input.ult =
                                run.players[&id].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
                            if run.players[&id].hp < 35 && !run.players[&id].hand.is_empty() {
                                input.play = 1;
                            }
                            inputs.insert(id, input);
                        }
                        run.step(&inputs);
                    }
                    let lost: u32 = run.players.values().map(|h| h.max_hp - h.hp).sum();
                    let t = &mut tally[kind];
                    t.0 += 1;
                    t.1 += u32::from(run.phase == Phase::Exploring || run.phase == Phase::Won);
                    t.2 += lost;
                    t.3 += run.tick - start;
                    t.4 += u32::from(run.phase == Phase::Wiped);
                    if run.phase == Phase::Wiped {
                        run.phase = Phase::Exploring;
                    }
                }
                viewers += run.audience;
                fans += run.fans;
            }
            let line = |(n, won, lost, ticks, wiped): (u32, u32, u32, u64, u32)| {
                if n == 0 {
                    "      -       ".to_string()
                } else {
                    format!(
                        "{:>3}/{:<3} ({:>2} wiped) {:>5.0}hp {:>3.0}s",
                        won,
                        n,
                        wiped,
                        lost as f32 / n as f32,
                        ticks as f32 / HZ as f32 / n as f32
                    )
                }
            };
            eprintln!(
                "DEEP party {party} floor {depth} {:<24} fights {}  guardian {}  threshold {}  viewers {} ({:.1} boxes)",
                packs.join("/"),
                line(tally[0]),
                line(tally[1]),
                line(tally[2]),
                audience::viewers(viewers / 8),
                fans as f32 / 8.0,
            );
        }
    }
}

/// Which monster makes a deep floor hard: the bot plays floor
/// `PROBE_DEPTH`'s fight rooms (or, with `PROBE_THRESHOLD`, the threshold)
/// once as they are and once without each kind of its roster.
/// `PROBE_DEPTH=5 cargo test deep_probe -- --ignored --nocapture`
#[test]
#[ignore]
fn deep_probe() {
    let mut built = home::Home::default();
    for (station, level) in [
        (home::Station::Forge, 3),
        (home::Station::Hearth, 3),
        (home::Station::Rack, 2),
        (home::Station::Chapel, 1),
    ] {
        built.levels.insert(station, level);
    }
    let depth: u32 = std::env::var("PROBE_DEPTH")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(5);
    let kinds: Vec<Option<EnemyKind>> = std::iter::once(None)
        .chain(
            Pack::at(depth, Pack::Crypt)
                .roster_at(depth)
                .iter()
                .map(|k| Some(k.0)),
        )
        .collect();
    for without in kinds {
        let (mut n, mut won, mut wiped, mut lost) = (0, 0, 0, 0);
        let mut templates = BTreeMap::<String, (u32, u32)>::new();
        for seed in 0..8 {
            let mut run = Run::new(seed, 1, None);
            run.rebuild_home(built.clone(), Default::default());
            run.outfit(1, knights::knight("percival").unwrap());
            for _ in 1..depth {
                run.descend_for_test();
            }
            let want = if std::env::var("PROBE_THRESHOLD").is_ok() {
                RoomKind::Threshold
            } else {
                RoomKind::Fight
            };
            let rooms: Vec<usize> = (0..run.dungeon.rooms.len())
                .filter(|&i| run.dungeon.rooms[i].kind == want)
                .collect();
            for room in rooms {
                if let Some(k) = without {
                    for kind in &mut run.dungeon.rooms[room].roster {
                        if *kind == k {
                            *kind = EnemyKind::Bat;
                        }
                    }
                }
                let hero = run.players.get_mut(&1).unwrap();
                hero.hp = hero.max_hp;
                hero.winds = 1;
                run.phase = Phase::Exploring;
                run.enter_for_test(room);
                if run.phase != Phase::Fighting {
                    continue;
                }
                let shape: String = run.dungeon.rooms[room]
                    .roster
                    .iter()
                    .map(|k| format!("{k:?}")[..2].to_string())
                    .collect::<Vec<_>>()
                    .join("");
                let start = run.tick;
                while run.phase == Phase::Fighting && run.tick - start < u64::from(180 * HZ) {
                    let (reach, ranged) = style(&run, 1);
                    let mut input = bot_as(&run, 1, reach, ranged);
                    input.ult = run.players[&1].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
                    let mut inputs = BTreeMap::new();
                    inputs.insert(1, input);
                    run.step(&inputs);
                    if let Some(k) = without {
                        run.enemies.retain(|e| e.kind != k);
                    }
                }
                n += 1;
                let w = run.phase == Phase::Exploring;
                won += u32::from(w);
                wiped += u32::from(run.phase == Phase::Wiped);
                lost += run.players[&1].max_hp - run.players[&1].hp;
                if without.is_none() {
                    let e = templates.entry(shape).or_default();
                    e.0 += 1;
                    e.1 += u32::from(w);
                }
                if run.phase == Phase::Wiped {
                    run.phase = Phase::Exploring;
                }
            }
        }
        eprintln!(
            "PROBE depth {depth} without {:<12} won {won}/{n} wiped {wiped} timeout {} hp {} max {}",
            format!("{without:?}"),
            n - won - wiped,
            lost / n.max(1),
            Run::new(0, 1, None).players[&1].max_hp,
        );
        for (shape, (k, w)) in templates {
            eprintln!("   {shape:<20} {w}/{k}");
        }
    }
}

/// The Pit Tyrant, by bot: one knight and two, the Undercroft built well,
/// ultimates cast when full. How often the Pit is won, and how long it
/// takes. `cargo test pit_balance -- --ignored --nocapture`
#[test]
#[ignore]
fn pit_balance() {
    let mut built = home::Home::default();
    for (station, level) in [
        (home::Station::Forge, 3),
        (home::Station::Hearth, 3),
        (home::Station::Rack, 2),
        (home::Station::Chapel, 1),
    ] {
        built.levels.insert(station, level);
    }
    for party in [1u32, 2] {
        let (mut won, mut wiped, mut ticks) = (0, 0, 0u64);
        for seed in 0..16u64 {
            let mut run = Run::new(seed, 1, (party == 2).then_some("Friend"));
            run.rebuild_home(built.clone(), Default::default());
            run.outfit(1, knights::knight("percival").unwrap());
            if party == 2 {
                run.outfit(2, knights::knight("lynette").unwrap());
            }
            run.enter_for_test(0);
            run.calm_for_test();
            run.enemies.clear();
            // Where the beast and the knights stand varies by seed.
            let (bx, by) = (
                14.0 + (seed % 5) as f32 * 5.0,
                8.0 + (seed % 3) as f32 * 3.0,
            );
            run.spawn_at_for_test(EnemyKind::PitTyrant, bx, by);
            for (k, hero) in run.players.values_mut().enumerate() {
                (hero.x, hero.y) = (10.0 + ((seed * 7 + k as u64 * 13) % 28) as f32, 22.0);
            }
            run.phase = Phase::Fighting;
            let start = run.tick;
            while run.phase == Phase::Fighting && run.tick - start < u64::from(240 * HZ) {
                if std::env::var("PIT_TRACE").is_ok()
                    && seed == 0
                    && (run.tick - start).is_multiple_of(u64::from(HZ))
                {
                    let lord = run.enemies.iter().find(|e| e.kind == EnemyKind::PitTyrant);
                    eprintln!(
                        "t{} heroes {:?} lord {:?} slams {} shots {}",
                        (run.tick - start) / u64::from(HZ),
                        run.players
                            .values()
                            .map(|h| (h.x as i32, h.y as i32, h.hp))
                            .collect::<Vec<_>>(),
                        lord.map(|e| (e.x as i32, e.y as i32, e.hp, e.stage)),
                        run.slams.len(),
                        run.projectiles.iter().filter(|p| p.hostile).count(),
                    );
                }
                let mut inputs = BTreeMap::new();
                for &id in run.players.keys() {
                    let (reach, ranged) = style(&run, id);
                    let mut input = bot_as(&run, id, reach, ranged);
                    input.ult = run.players[&id].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
                    inputs.insert(id, input);
                }
                run.step(&inputs);
            }
            won += u32::from(run.phase != Phase::Fighting && run.phase != Phase::Wiped);
            wiped += u32::from(run.phase == Phase::Wiped);
            ticks += run.tick - start;
        }
        eprintln!(
            "PIT party {party}: won {won}/16, wiped {wiped}, {:.0}s a fight",
            ticks as f32 / 16.0 / HZ as f32
        );
    }
}

/// Each of a deep floor's guardians on its own, against the same eight
/// delves, one knight and two: which one is the harder fight.
/// `PROBE_DEPTH=4 cargo test guardian_probe -- --ignored --nocapture`
#[test]
#[ignore]
fn guardian_probe() {
    let mut built = home::Home::default();
    for (station, level) in [
        (home::Station::Forge, 3),
        (home::Station::Hearth, 3),
        (home::Station::Rack, 2),
        (home::Station::Chapel, 1),
    ] {
        built.levels.insert(station, level);
    }
    let depth: u32 = std::env::var("PROBE_DEPTH")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(4);
    let pack = Pack::at(depth, Pack::Crypt);
    let ids: Vec<String> = bosses::builtin()
        .into_iter()
        .filter(|b| b.only_in == pack)
        .map(|b| b.id)
        .collect();
    for id in &ids {
        for party in [1u32, 2] {
            let (mut won, mut wiped, mut lost, mut ticks) = (0u32, 0u32, 0u32, 0u64);
            for seed in 0..8 {
                let mut run = Run::new(seed, 1, (party == 2).then_some("Friend"));
                run.bosses.retain(|b| b.only_in != pack || &b.id == id);
                run.rebuild_home(built.clone(), Default::default());
                run.outfit(1, knights::knight("percival").unwrap());
                if party == 2 {
                    run.outfit(2, knights::knight("lynette").unwrap());
                }
                for _ in 1..depth {
                    run.descend_for_test();
                }
                let stairs = run
                    .dungeon
                    .rooms
                    .iter()
                    .position(|r| r.kind == RoomKind::Stairs)
                    .unwrap();
                for hero in run.players.values_mut() {
                    hero.hp = hero.max_hp;
                    hero.winds = 1;
                }
                run.phase = Phase::Exploring;
                run.enter_for_test(stairs);
                let start = run.tick;
                while run.phase == Phase::Fighting && run.tick - start < u64::from(180 * HZ) {
                    let mut inputs = BTreeMap::new();
                    for &hero in run.players.keys() {
                        let (reach, ranged) = style(&run, hero);
                        let mut input = bot_as(&run, hero, reach, ranged);
                        input.ult =
                            run.players[&hero].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
                        if run.players[&hero].hp < 35 && !run.players[&hero].hand.is_empty() {
                            input.play = 1;
                        }
                        inputs.insert(hero, input);
                    }
                    run.step(&inputs);
                }
                won += u32::from(run.phase == Phase::Exploring || run.phase == Phase::Won);
                wiped += u32::from(run.phase == Phase::Wiped);
                lost += run.players.values().map(|h| h.max_hp - h.hp).sum::<u32>();
                ticks += run.tick - start;
            }
            println!(
                "GUARDIAN {id:<20} party {party}  won {won}/8 ({wiped} wiped)  {:>3}hp {:>3}s",
                lost / 8,
                ticks / 8 / u64::from(HZ)
            );
        }
    }
}

/// What Beaumains is worth to one knight: each floor's fights and its
/// guardian, the same eight delves, with him and without.
/// `cargo test hireling_probe -- --ignored --nocapture`
#[test]
#[ignore]
fn hireling_probe() {
    let mut built = home::Home::default();
    for (station, level) in [
        (home::Station::Forge, 3),
        (home::Station::Hearth, 3),
        (home::Station::Rack, 2),
        (home::Station::Chapel, 1),
    ] {
        built.levels.insert(station, level);
    }
    for depth in 1..=4u32 {
        for hired in [false, true] {
            // (rooms, won, wiped) for fights, then the guardian.
            let mut tally = [(0u32, 0u32, 0u32); 2];
            let mut downs = 0u32;
            for seed in 0..8 {
                let mut run = Run::new(seed, 1, None);
                run.rebuild_home(built.clone(), Default::default());
                run.outfit(1, knights::knight("percival").unwrap());
                for _ in 1..depth {
                    run.descend_for_test();
                }
                if hired {
                    let at = (run.players[&1].x, run.players[&1].y);
                    run.hireling = Some(hireling::Hireling::new(depth, at));
                }
                let rooms: Vec<(usize, usize)> = run
                    .dungeon
                    .rooms
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| match r.kind {
                        RoomKind::Fight => Some((i, 0)),
                        RoomKind::Stairs | RoomKind::Lair => Some((i, 1)),
                        _ => None,
                    })
                    .collect();
                for (room, kind) in rooms {
                    let hero = run.players.get_mut(&1).unwrap();
                    hero.hp = hero.max_hp;
                    hero.winds = 1;
                    run.phase = Phase::Exploring;
                    run.enter_for_test(room);
                    if run.phase != Phase::Fighting {
                        continue;
                    }
                    let start = run.tick;
                    while run.phase == Phase::Fighting && run.tick - start < u64::from(180 * HZ) {
                        let (reach, ranged) = style(&run, 1);
                        let mut input = bot_as(&run, 1, reach, ranged);
                        input.ult =
                            run.players[&1].ult_charge >= ults::ULT_FULL && run.tick.is_multiple_of(2);
                        if run.players[&1].hp < 35 && !run.players[&1].hand.is_empty() {
                            input.play = 1;
                        }
                        let fell = run.hireling.as_ref().is_some_and(|h| h.down());
                        run.step(&BTreeMap::from([(1, input)]));
                        if !fell && run.hireling.as_ref().is_some_and(|h| h.down()) {
                            downs += 1;
                        }
                    }
                    let t = &mut tally[kind];
                    t.0 += 1;
                    t.1 += u32::from(run.phase == Phase::Exploring || run.phase == Phase::Won);
                    t.2 += u32::from(run.phase == Phase::Wiped);
                    if run.phase == Phase::Wiped {
                        run.phase = Phase::Exploring;
                    }
                }
            }
            println!(
                "HIRE floor {depth} {}  fights {:>2}/{:<2} ({:>2} wiped)  guardian {}/{} ({} wiped)  he fell {downs}x",
                if hired {
                    "with Beaumains   "
                } else {
                    "without Beaumains"
                },
                tally[0].1,
                tally[0].0,
                tally[0].2,
                tally[1].1,
                tally[1].0,
                tally[1].2,
            );
        }
    }
}
