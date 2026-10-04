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
    let danger = run.projectiles.iter().any(|p| {
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
    let mut tally = Tally::default();
    for seed in seeds {
        let mut run = Run::new(seed, 1, None);
        run.outfit(1, knight);
        let (reach, ranged) = {
            let hero = &run.players[&1];
            match hero.forged.as_ref().and_then(|w| w.melee) {
                Some(m) if hero.forged.as_ref().is_some_and(|w| w.bolt.is_none()) => {
                    (m.reach as f32, false)
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
                let input = bot(&run, reach, ranged);
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

/// How a knight's bot fights: its reach, and whether it holds at range.
fn style(run: &Run, id: u32) -> (f32, bool) {
    let hero = &run.players[&id];
    match hero.forged.as_ref().and_then(|w| w.melee) {
        Some(m) if hero.forged.as_ref().is_some_and(|w| w.bolt.is_none()) => {
            (m.reach as f32, false)
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
