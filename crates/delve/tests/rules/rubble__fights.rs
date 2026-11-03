//! QA: the six reviewed guardians fought to the end by four knights who
//! play as friends do, and what the fight left. Run by hand:
//! `cargo test -p angel-delve rubble::fights -- --ignored --nocapture`.
use super::*;
use crate::render::overworld::arena::testing;

const BOSSES: [&str; 6] = ["cinderjaw", "crypt-relic-liege", "grinwick", "the-big-ripper", "iron-tusk-chief", "hellforge-slag-sovereign"];

fn sign(v: f32, dead: f32) -> i8 {
    if v > dead { 1 } else if v < -dead { -1 } else { 0 }
}

/// A friend: keeps a few tiles off the guardian, goes round it (the first
/// knight holds its front), aims at it and fires, swings when close, rolls
/// now and then.
fn friend(run: &Run, id: u32) -> Input {
    let Some(h) = run.players.get(&id) else { return Input::default() };
    let Some(target) = run
        .enemies
        .iter()
        .filter(|e| e.hp > 0)
        .min_by(|a, b| {
            let boss = |e: &Enemy| if e.boss.is_some() { 0.0 } else { 6.0 };
            ((a.x - h.x).hypot(a.y - h.y) + boss(a)).total_cmp(&((b.x - h.x).hypot(b.y - h.y) + boss(b)))
        })
    else {
        return Input::default();
    };
    let (dx, dy) = (target.x - h.x, target.y - h.y);
    let d = dx.hypot(dy).max(0.01);
    let keep = if id == 1 { 4.0 } else { 6.0 };
    let (ux, uy) = (dx / d, dy / d);
    let round = if id % 2 == 0 { 1.0 } else { -1.0 };
    let (mut mx, mut my) = ((-uy) * round, ux * round);
    if d > keep + 1.0 {
        (mx, my) = (mx * 0.4 + ux, my * 0.4 + uy);
    } else if d < keep - 1.0 {
        (mx, my) = (mx * 0.4 - ux, my * 0.4 - uy);
    }
    if id == 1 {
        (mx, my) = (ux * (d - keep).signum(), uy * (d - keep).signum());
    }
    Input {
        move_x: sign(mx, 0.35),
        move_y: sign(my, 0.35),
        aim_x: sign(ux, 0.38),
        aim_y: sign(uy, 0.38),
        fire: true,
        swing: d < 3.0,
        dash: run.tick % 97 == u64::from(id) * 13,
        ..Input::default()
    }
}

#[derive(Default, Debug)]
struct Fight {
    killed_at: Option<u64>,
    min_share: f32,
    stages: Vec<(u64, String)>,
    blocks_before: usize,
    blocks_after: usize,
    revisions: u32,
    knight_in_stone: u32,
    boss_in_stone: u32,
    longest_unhurt: u64,
    barred_after: bool,
    /// Each blow struck: its kind and how far the nearest stone stood.
    strikes: Vec<(String, f32)>,
    in_stone_at: Vec<(u64, u8, i32, i32)>,
}

fn blocks(run: &Run) -> usize {
    let room = run.room();
    (0..room.rows as i32).flat_map(|r| (0..room.cols as i32).map(move |c| (c, r))).filter(|&(c, r)| room.tile(c, r) == Tile::Block).count()
}

fn stone(run: &Run, x: f32, y: f32) -> bool {
    matches!(run.room().tile((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32), Tile::Block | Tile::Wall)
}

fn fight(id: &str, seats: u32, ticks: u64) -> Fight {
    let mut run = testing::boss(id);
    for k in 2..=seats {
        run.join_seat(k, &format!("Knight {k}"));
    }
    let mut f = Fight { blocks_before: blocks(&run), min_share: 1.0, ..Fight::default() };
    let revision = run.dungeon.world_revision;
    let mut last_hp = u32::MAX;
    let mut last_hurt = 0u64;
    let mut stage = String::new();
    let mut was_stage = 0u8;
    for t in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        let inputs: BTreeMap<u32, Input> = run.players.keys().map(|&k| (k, friend(&run, k))).collect();
        run.step(&inputs);
        for hero in run.players.values() {
            f.knight_in_stone += u32::from(stone(&run, hero.x, hero.y));
        }
        let Some(boss) = run.enemies.iter().find(|e| e.boss.is_some() && e.hp > 0) else {
            f.killed_at = Some(t);
            break;
        };
        if std::env::var("QA_TRACE").is_ok_and(|v| v == id) && t > 700 && t < 800 {
            println!("t {t} boss ({:.2},{:.2}) tile ({},{}) stage {} timer {} tile_kind {:?} r {:.2}", boss.x, boss.y, (boss.x / TILE_UNITS) as i32, (boss.y / TILE_UNITS) as i32, boss.stage, boss.timer, run.room().tile((boss.x / TILE_UNITS) as i32, (boss.y / TILE_UNITS) as i32), boss.radius());
        }
        if stone(&run, boss.x, boss.y) {
            f.boss_in_stone += 1;
            if f.in_stone_at.len() < 6 {
                f.in_stone_at.push((t, boss.stage, (boss.x / TILE_UNITS) as i32, (boss.y / TILE_UNITS) as i32));
            }
        }
        if boss.stage == signatures::DAZED && was_stage != signatures::DAZED {
            let def = &run.bosses[usize::from(boss.boss.unwrap())];
            let kind = signatures::winding(boss, def).map_or("?".to_string(), |u| format!("{:?}", u.kind));
            let room = run.room();
            let near = (0..room.rows as i32)
                .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
                .filter(|&(c, r)| room.tile(c, r) == Tile::Block)
                .map(|(c, r)| ((c as f32 + 0.5) * TILE_UNITS - boss.x).hypot((r as f32 + 0.5) * TILE_UNITS - boss.y))
                .fold(f32::MAX, f32::min);
            f.strikes.push((kind, (near * 10.0).round() / 10.0));
        }
        was_stage = boss.stage;
        f.min_share = f.min_share.min(boss.hp as f32 / boss.max_hp as f32);
        let def = &run.bosses[usize::from(boss.boss.unwrap())];
        let now = format!("{:?}", def.stage(boss.hp, boss.max_hp));
        if now != stage {
            f.stages.push((t, now.clone()));
            stage = now;
        }
        if boss.hp < last_hp {
            f.longest_unhurt = f.longest_unhurt.max(t - last_hurt);
            last_hurt = t;
        }
        last_hp = boss.hp;
    }
    // Let the room settle after the guardian falls.
    for _ in 0..(3 * HZ) {
        run.step(&BTreeMap::new());
    }
    f.blocks_after = blocks(&run);
    f.revisions = run.dungeon.world_revision.wrapping_sub(revision);
    f.barred_after = run.barred();
    f
}

#[test]
#[ignore]
fn rubble_fights_qa() {
    for seats in [4, 6] {
        for id in BOSSES {
            let t = std::time::Instant::now();
            let f = fight(id, seats, 8 * 60 * u64::from(HZ));
            println!("{seats} knights vs {id}: {f:?} ({:.1}s)", t.elapsed().as_secs_f32());
        }
    }
}

fn write(run: &Run, name: &str) {
    let dir = std::env::var("RUBBLE_FIGHT_SHOTS").expect("set RUBBLE_FIGHT_SHOTS to the output directory");
    std::fs::create_dir_all(&dir).unwrap();
    let img = crate::render::overworld::arena::frame(run, 384, 224);
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(format!("{dir}/{name}.ppm"), out).unwrap();
}

/// Six knights against Cinderjaw, its frames at the ticks it stood in stone.
#[test]
#[ignore]
fn rubble_fights_shots() {
    let mut run = testing::boss("cinderjaw");
    for k in 2..=6 {
        run.join_seat(k, &format!("Knight {k}"));
    }
    for t in 0..800u64 {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        let inputs: BTreeMap<u32, Input> = run.players.keys().map(|&k| (k, friend(&run, k))).collect();
        run.step(&inputs);
        if [700, 760, 777, 790].contains(&t) {
            write(&run, &format!("cinderjaw-6-{t}"));
        }
    }
}

/// A charge at a lone boulder, in the middle of its row, and at two
/// boulders stacked across its way.
#[test]
#[ignore]
fn rubble_fights_charge_at_a_lone_boulder() {
    for (name, blocks) in [("lone", vec![(18usize, 7usize)]), ("stacked", vec![(18, 7), (18, 8)]), ("lone-y14", vec![(18, 7)])] {
        let mut run = Run::new(17, 1, Some("Friend"));
        run.dungeon = crawl_floor(2, Pack::Cavern, 17);
        let at = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
        run.enter_for_test(at);
        let room = &mut run.dungeon.rooms[at];
        for y in 1..room.rows - 1 {
            for x in 1..room.cols - 1 {
                room.set_for_test(x, y, Tile::Floor);
            }
        }
        for &(x, y) in &blocks {
            room.set_for_test(x, y, Tile::Block);
        }
        run.enemies.clear();
        run.calm_for_test();
        let tusk = bosses::checked("iron-tusk-chief").unwrap();
        let charge = tusk.windups.iter().position(|u| u.kind == bosses::Windup::Charge).unwrap();
        run.bosses.push(tusk);
        let index = run.bosses.len() - 1;
        let y = if name == "lone-y14" { 14.6 } else { 15.0 };
        run.spawn_boss_at_for_test(index, 28.0, y);
        let e = &mut run.enemies[0];
        (e.stage, e.timer, e.signature.windup, e.dir) = (signatures::DASH, 0, charge as u8, (1.0, 0.0));
        e.signature.rails = vec![signatures::Rail { a: (28.0, y), b: (46.0, y) }];
        let mut shatters = Vec::new();
        let mut path = Vec::new();
        for _ in 0..45 {
            let grid = Grid { room: &run.dungeon.rooms[run.at], barred: true };
            let e = &mut run.enemies[0];
            let mut deeds = foes::Deeds::default();
            signatures::wind(e, &run.bosses[index], &[(12.0, 20.0)], &grid, 17, &mut deeds);
            shatters.extend(deeds.shatters);
            path.push(((run.enemies[0].x / TILE_UNITS) as i32, (run.enemies[0].y / TILE_UNITS) as i32));
            if run.enemies[0].stage != signatures::DASH {
                break;
            }
        }
        path.dedup();
        println!("{name}: radius {:.2} shatters {} path {:?}", run.enemies[0].radius(), shatters.len(), path);
    }
}
