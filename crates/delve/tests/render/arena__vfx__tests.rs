use super::super::*;
use super::{Budget, PARTICLES, blocks, struck};
use knights::Struck;
use crate::rules::{EnemyKind, Input, Projectile, RoomKind, Shot, Weapon, cards};
use std::collections::BTreeMap;

/// The first fight on floor one, a party of four in it.
fn party_fight(seed: u64) -> Run {
    let mut run = Run::new(seed, 1, Some("Friend"));
    run.join_seat(3, "Wren");
    run.join_seat(4, "Bram");
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .expect("a fight on floor one");
    run.enter_for_test(index);
    run.window = None;
    run
}

/// The way from a knight to the nearest monster, as keys.
fn toward(run: &Run, id: u32) -> (i8, i8) {
    let hero = &run.players[&id];
    run.enemies
        .iter()
        .filter(|e| e.hp > 0)
        .min_by(|a, b| {
            (a.x - hero.x).hypot(a.y - hero.y).total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
        })
        .map_or((0, -1), |e| {
            let (dx, dy) = (e.x - hero.x, e.y - hero.y);
            let key = |v: f32, o: f32| if v.abs() > o.abs() * 0.4 { v.signum() as i8 } else { 0 };
            (key(dx, dy), key(dy, dx))
        })
}

/// A busy fight: four knights (a handgonne, a crossbow, a bow behind a
/// raised shield, a bow that rolls), a dozen monsters kept coming, every
/// knight kept standing. `ticks` into it.
fn busy_fight(ticks: u32) -> Run {
    let mut run = party_fight(11);
    for (id, weapon) in [(1, Weapon::Handgonne), (2, Weapon::Crossbow), (3, Weapon::Bow), (4, Weapon::Bow)] {
        let hero = run.players.get_mut(&id).unwrap();
        hero.weapon = weapon;
        hero.guard = if id == 3 { cards::Guard::Shield } else { cards::Guard::Roll };
    }
    // A party late in a delve: quick hands, two shots a loose.
    for hero in run.players.values_mut() {
        (hero.bonus.rate, hero.bonus.shots) = (80, 1);
    }
    for _ in 0..ticks {
        busy_step(&mut run);
    }
    run
}

/// One tick of the busy fight.
fn busy_step(run: &mut Run) {
    let live = run.enemies.iter().filter(|e| e.hp > 0).count();
    if live < 18 {
        let n = run.tick as f32;
        let (x, y) = (10.0 + (n * 7.3) % 28.0, 7.0 + (n * 3.1) % 7.0);
        let kind = [EnemyKind::Skeleton, EnemyKind::Imp, EnemyKind::Wraith, EnemyKind::Bat][run.tick as usize % 4];
        run.spawn_at_for_test(kind, x, y);
    }
    let mut inputs = BTreeMap::new();
    for id in 1..=4 {
        let (ax, ay) = toward(run, id);
        let tick = run.tick;
        inputs.insert(
            id,
            Input {
                aim_x: ax,
                aim_y: ay,
                fire: id != 3 || tick % 40 > 30,
                dash: (id == 3 && tick % 40 <= 30) || (id == 4 && tick % 37 == 0),
                move_x: if id == 4 { [1, 0, -1, 0][(tick / 20 % 4) as usize] } else { 0 },
                move_y: if id == 4 { [0, 1, 0, -1][(tick / 20 % 4) as usize] } else { 0 },
                ..Default::default()
            },
        );
    }
    run.step(&inputs);
    // The party holds the middle of the room, the monsters round them.
    for (&id, hero) in run.players.iter_mut() {
        hero.hp = hero.max_hp;
        if id != 4 {
            (hero.x, hero.y) = (17.0 + id as f32 * 3.0, 18.0);
        }
    }
}

/// Frame time of a busy four-knight fight, for each seat, as a frame
/// (`frame_for`) and as a scene in parts (`scene_for`):
/// `cargo test --release -p angel-delve vfx_frame_time -- --ignored --nocapture`.
#[test]
#[ignore]
fn vfx_frame_time() {
    let mut run = busy_fight(240);
    let mut frames = Vec::new();
    let mut scenes = Vec::new();
    let mut fx = 0usize;
    for i in 0..400 {
        busy_step(&mut run);
        fx += run.feel.impacts.len() + run.feel.deaths.len() + run.feel.puffs.len() + run.feel.blocks.len() + run.blasts.len();
        let focus = Some(1 + i % 4);
        let start = std::time::Instant::now();
        std::hint::black_box(frame_for(&run, NATIVE_W, NATIVE_H, focus));
        frames.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = std::time::Instant::now();
        std::hint::black_box(scene_for(&run, NATIVE_W, NATIVE_H, focus));
        scenes.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let report = |name: &str, mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        println!(
            "{name}: mean {mean:.3} ms, median {:.3} ms, p95 {:.3} ms",
            v[v.len() / 2],
            v[v.len() * 95 / 100]
        );
    };
    println!(
        "busy fight: {} monsters, {} shots, {:.1} effects a frame",
        run.enemies.len(),
        run.projectiles.len(),
        fx as f64 / 400.0
    );
    report("frame_for", frames);
    report("scene_for", scenes);
}

/// `img` cut to `w` × `h` round `(cx, cy)`, black where it runs out.
fn crop(img: &Img, (cx, cy): (i32, i32), w: i32, h: i32) -> Img {
    let mut out = Img::black(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = img.get(cx - w / 2 + x, cy - h / 2 + y) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// Frames side by side, a grey rule between.
fn strip(frames: &[Img]) -> Img {
    let w: i32 = frames.iter().map(|f| f.w + 1).sum::<i32>() - 1;
    let h = frames.iter().map(|f| f.h).max().unwrap_or(1);
    let mut out = Img::black(w, h);
    let mut x = 0;
    for f in frames {
        out.stamp(f, x, 0);
        if x + f.w < w {
            for y in 0..h {
                out.put(x + f.w, y, 'g');
            }
        }
        x += f.w + 1;
    }
    out
}

fn save(dir: &std::path::Path, name: &str, img: &Img, k: u32) {
    let rgba = img.rgba_scaled(k);
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
        .unwrap()
        .save(dir.join(format!("{name}.png")))
        .unwrap();
}

/// The seat's whole screen, as a browser draws it, and where `(ux, uy)` is.
fn seat_view(run: &Run, focus: u32, (ux, uy): (f32, f32)) -> (Img, (i32, i32)) {
    let (w, h) = (NATIVE_W, NATIVE_H + BAND_H + HUD_H);
    let img = seat_scene(run, w, h, Some(focus)).compose();
    (img, seat_px(run, w, h, Some(focus), ux, uy))
}

/// A calm fight with knight 1 at (24, 14) aiming `aim`.
fn quiet(aim: (f32, f32), weapon: Weapon, guard: cards::Guard) -> Run {
    let mut run = Run::new(11, 1, None);
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(index);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.step(&BTreeMap::new());
    run.window = None;
    let hero = run.players.get_mut(&1).unwrap();
    hero.weapon = weapon;
    hero.guard = guard;
    let len = aim.0.hypot(aim.1);
    (hero.x, hero.y, hero.aim_x, hero.aim_y, hero.invulnerable) = (24.0, 14.0, aim.0 / len, aim.1 / len, 0);
    run
}

fn monster_shot(kind: Shot, at: (f32, f32), v: (f32, f32), damage: u32) -> Projectile {
    serde_json::from_value(serde_json::json!({
        "x": at.0, "y": at.1, "vx": v.0, "vy": v.1, "hostile": true,
        "kind": kind, "damage": damage, "pierce": 0, "ttl": 90
    }))
    .unwrap()
}

fn hold(id: u32, input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, input)])
}

/// A shot blocked on a raised shield, frame by frame from a tick before
/// it lands: the knight aiming `aim`, the shot of `kind` coming from
/// `bearing` degrees off the aim.
fn block_strip(aim: (f32, f32), kind: Shot, damage: u32, bearing: f32) -> Vec<Img> {
    let mut run = quiet(aim, Weapon::Bow, cards::Guard::Shield);
    let hero = &run.players[&1];
    let a = hero.aim_y.atan2(hero.aim_x) + bearing.to_radians();
    let from = (24.0 + a.cos() * 4.0, 14.0 + a.sin() * 4.0);
    run.projectiles.push(monster_shot(kind, from, (-a.cos() * 12.0, -a.sin() * 12.0), damage));
    let guard = hold(1, Input { dash: true, ..Default::default() });
    // Up to the tick before it lands.
    for _ in 0..30 {
        let mut ahead = run.clone();
        ahead.step(&guard);
        if !ahead.feel.blocks.is_empty() {
            break;
        }
        run = ahead;
    }
    let mut frames = Vec::new();
    for _ in 0..12 {
        let (img, at) = seat_view(&run, 1, (24.0, 14.0));
        frames.push(crop(&img, (at.0, at.1 - 6), 48, 40));
        run.step(&guard);
    }
    frames
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_vfx_shots -- --ignored`
/// (`ANGEL_VFX_TAG` names the set: before, after): the shield's block, and
/// each of the fight's effects, as strips of consecutive frames at a
/// browser's scale and whole seats mid-fight.
#[test]
#[ignore]
fn write_vfx_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let tag = std::env::var("ANGEL_VFX_TAG").unwrap_or_else(|_| "after".into());
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let k = 4;
    // The shield.
    save(&dir, &format!("{tag}-block-ember-side"), &strip(&block_strip((1.0, 0.0), Shot::Ember, 12, 0.0)), k);
    save(&dir, &format!("{tag}-block-ore-heavy-side"), &strip(&block_strip((-1.0, 0.0), Shot::Ore, 30, 20.0)), k);
    save(&dir, &format!("{tag}-block-wisp-down"), &strip(&block_strip((0.0, 1.0), Shot::Wisp, 12, -25.0)), k);
    save(&dir, &format!("{tag}-block-frost-up"), &strip(&block_strip((0.0, -1.0), Shot::Frost, 16, 30.0)), k);
    save(&dir, &format!("{tag}-block-hex-diag"), &strip(&block_strip((1.0, 1.0), Shot::Hex, 12, 0.0)), k);
    // Hits on a monster, and its death.
    for (name, weapon) in [("crossbow", Weapon::Crossbow), ("handgonne", Weapon::Handgonne), ("bow", Weapon::Bow)] {
        let mut run = quiet((1.0, 0.0), weapon, cards::Guard::Roll);
        run.spawn_at_for_test(EnemyKind::Skeleton, 31.0, 14.0);
        let fire = hold(1, Input { aim_x: 1, fire: true, ..Default::default() });
        let mut frames = Vec::new();
        for t in 0..36 {
            if let Some(e) = run.enemies.first_mut() {
                (e.x, e.y, e.frozen) = (31.0, 14.0, 99);
                if t == 20 {
                    e.hp = 1;
                }
            }
            run.step(&fire);
            if t % 2 == 0 && t < 18 {
                let (img, at) = seat_view(&run, 1, (27.5, 14.0));
                frames.push(crop(&img, (at.0, at.1 - 6), 80, 44));
            } else if t >= 20 {
                let (img, at) = seat_view(&run, 1, (31.0, 14.0));
                frames.push(crop(&img, (at.0, at.1 - 6), 56, 44));
            }
        }
        save(&dir, &format!("{tag}-fire-{name}"), &strip(&frames[..9]), 3);
        save(&dir, &format!("{tag}-fire-{name}-kill"), &strip(&frames[9..]), 3);
    }
    // A handgonne ball's burst and a bomb.
    {
        let mut run = quiet((0.0, -1.0), Weapon::Bow, cards::Guard::Roll);
        let mut frames = Vec::new();
        run.blasts.push((24.0, 8.0, crate::rules::BLAST_TICKS - 1));
        for _ in 0..crate::rules::BLAST_TICKS {
            let (img, at) = seat_view(&run, 1, (24.0, 8.0));
            frames.push(crop(&img, at, 56, 44));
            run.step(&BTreeMap::new());
        }
        save(&dir, &format!("{tag}-blast"), &strip(&frames), k);
        let mut run = quiet((0.0, -1.0), Weapon::Bow, cards::Guard::Roll);
        run.players.get_mut(&1).unwrap().bombs = 3;
        for y in [8.0, 10.0, 18.0] {
            run.projectiles.push(monster_shot(Shot::Wisp, (20.0, y), (0.0, 0.1), 12));
            run.projectiles.push(monster_shot(Shot::Ore, (29.0, y), (0.0, 0.1), 12));
        }
        run.step(&hold(1, Input { bomb: true, ..Default::default() }));
        let mut frames = Vec::new();
        for _ in 0..12 {
            let (img, at) = seat_view(&run, 1, (24.0, 14.0));
            frames.push(crop(&img, (at.0, at.1 - 6), 120, 88));
            run.step(&BTreeMap::new());
        }
        save(&dir, &format!("{tag}-bomb"), &strip(&frames[..6]), 2);
        save(&dir, &format!("{tag}-bomb-late"), &strip(&frames[6..]), 2);
    }
    // A roll, with its dust.
    {
        let mut run = quiet((1.0, 0.0), Weapon::Bow, cards::Guard::Roll);
        run.players.get_mut(&1).unwrap().x = 18.0;
        let mut frames = Vec::new();
        for t in 0..16 {
            run.step(&hold(1, Input { move_x: 1, dash: t == 0, ..Default::default() }));
            let (img, at) = seat_view(&run, 1, (22.0, 14.0));
            frames.push(crop(&img, (at.0, at.1 - 6), 96, 36));
        }
        save(&dir, &format!("{tag}-roll"), &strip(&frames[..8]), 3);
        save(&dir, &format!("{tag}-roll-late"), &strip(&frames[8..]), 3);
    }
    // A card picked up off the floor.
    {
        let mut run = quiet((1.0, 0.0), Weapon::Bow, cards::Guard::Roll);
        run.players.get_mut(&1).unwrap().x = 20.0;
        let room = run.at;
        run.dungeon.rooms[room].items.push(crate::rules::Item {
            card: "potion".into(),
            x: 24.0,
            y: 14.0,
            held_off: None, took: Vec::new(),
        });
        let mut frames = Vec::new();
        for t in 0..24 {
            run.step(&hold(1, Input { move_x: i8::from(t < 8), ..Default::default() }));
            if t % 2 == 0 {
                let (img, at) = seat_view(&run, 1, (23.0, 14.0));
                frames.push(crop(&img, (at.0, at.1 - 8), 56, 40));
            }
        }
        save(&dir, &format!("{tag}-pickup"), &strip(&frames), 3);
    }
    // Whole seats mid-fight.
    let run = busy_fight(200);
    for focus in [1, 3] {
        let (img, _) = seat_view(&run, focus, (24.0, 14.0));
        save(&dir, &format!("{tag}-busy-seat{focus}"), &img, 2);
    }
}

/// A shot from the front, blocked: the run on the tick the block lands.
fn blocked(aim: (f32, f32), kind: Shot, damage: u32) -> Run {
    let mut run = quiet(aim, Weapon::Bow, cards::Guard::Shield);
    let hero = &run.players[&1];
    let (ax, ay) = (hero.aim_x, hero.aim_y);
    run.projectiles.push(monster_shot(kind, (24.0 + ax * 4.0, 14.0 + ay * 4.0), (-ax * 12.0, -ay * 12.0), damage));
    let guard = hold(1, Input { dash: true, ..Default::default() });
    for _ in 0..30 {
        run.step(&guard);
        if !run.feel.blocks.is_empty() {
            return run;
        }
    }
    panic!("never blocked");
}

#[test]
fn a_block_is_drawn_for_every_seat_and_composes_from_its_parts() {
    let mut run = blocked((1.0, 0.0), Shot::Ember, 12);
    run.join_seat(2, "Friend");
    run.join_seat(3, "Wren");
    let mut quiet = run.clone();
    quiet.feel.blocks.clear();
    let (w, h) = (NATIVE_W, NATIVE_H + BAND_H + HUD_H);
    for focus in [Some(1), Some(2), Some(3), None] {
        let with = seat_frame(&run, w, h, focus);
        assert!(seat_scene(&run, w, h, focus).compose().words() == with.words(), "{focus:?}: parts compose to the frame");
        let without = seat_frame(&quiet, w, h, focus);
        let (x, y) = seat_px(&run, w, h, focus, run.feel.blocks[0].x, run.feel.blocks[0].y);
        let changed = (y - 14..y + 10)
            .flat_map(|py| (x - 14..x + 14).map(move |px| (px, py)))
            .filter(|&(px, py)| with.get(px, py) != without.get(px, py))
            .count();
        assert!(changed >= 20, "{focus:?}: the block shows ({changed} pixels)");
        assert!(with.pixels().all(|c| c == super::super::super::ink::BLACK || super::super::super::ink::palette_index(c).is_some()));
    }
}

#[test]
fn a_raised_shield_is_knocked_back_and_flashes_when_struck() {
    let run = blocked((1.0, 0.0), Shot::Ore, 30);
    let knock = struck(&run, 1);
    assert_eq!((knock.dx, knock.dy, knock.flash), (-2, 0, 2), "back along the shot, two pixels for a heavy one");
    let hero = &run.players[&1];
    let calm = knights::figure_with(hero, 1, false);
    let hit = knights::figure_struck(hero, 1, false, knock);
    assert!(calm.words() != hit.words());
    // Facing left, the picture is turned over: the shield still goes back.
    let run = blocked((-1.0, 0.0), Shot::Wisp, 12);
    let knock = struck(&run, 1);
    assert_eq!((knock.dx, knock.dy), (-1, 0));
    // A knight nobody struck is drawn as ever.
    assert_eq!(struck(&run, 2), Struck::default());
}

#[test]
fn no_burst_hides_a_monster_shot() {
    let mut run = blocked((1.0, 0.0), Shot::Bone, 30);
    // A second shot right through the burst, still in flight.
    let b = run.feel.blocks[0];
    run.projectiles.push(monster_shot(Shot::Eye, (b.x + 0.4, b.y - 0.6), (0.0, 0.5), 12));
    let mut bare = run.clone();
    bare.feel.blocks.clear();
    let mut alone = run.clone();
    alone.feel.blocks.clear();
    alone.projectiles.clear();
    let with = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let shot = frame_for(&bare, NATIVE_W, NATIVE_H, Some(1));
    let empty = frame_for(&alone, NATIVE_W, NATIVE_H, Some(1));
    let mut checked = 0;
    for y in 0..NATIVE_H {
        for x in 0..NATIVE_W {
            // A pixel of the shot's own sprite (its inks are all signal; its
            // glow is not): the burst leaves it be.
            if shot.get(x, y) != empty.get(x, y) && shot.get(x, y).is_some_and(super::super::super::ink::is_signal) {
                assert_eq!(with.get(x, y), shot.get(x, y), "{x},{y}");
                checked += 1;
            }
        }
    }
    assert!(checked > 10, "{checked}");
}

#[test]
fn a_room_full_of_blocks_keeps_to_its_motes() {
    let mut run = blocked((1.0, 0.0), Shot::Ore, 30);
    let b = run.feel.blocks[0];
    run.feel.blocks = (0..16)
        .map(|i| crate::rules::feel::Block { x: 6.0 + i as f32 * 2.5, ttl: crate::rules::feel::BLOCK_TICKS - 1, ..b })
        .collect();
    let mut cv = Img::new(NATIVE_W, NATIVE_H);
    let mut budget = Budget::new();
    blocks(&mut cv, &run, &mut budget, false);
    assert_eq!(budget.spent(), PARTICLES, "a full room spends the frame's motes and no more");
    // A lone block is far under it.
    run.feel.blocks.truncate(1);
    let mut budget = Budget::new();
    blocks(&mut cv, &run, &mut budget, false);
    assert!(budget.spent() <= PARTICLES / 5, "{}", budget.spent());
}

#[test]
fn a_roll_kicks_up_dust_leaves_it_along_the_way_and_lands_in_it() {
    use crate::rules::feel::{KICK, LAND, TRAIL};
    let mut run = quiet((1.0, 0.0), Weapon::Bow, cards::Guard::Roll);
    let mut seen = Vec::new();
    for t in 0..16 {
        let before = run.feel.puffs.len();
        run.step(&hold(1, Input { move_x: 1, dash: t == 0, ..Default::default() }));
        let fresh = run.feel.puffs.len().saturating_sub(before);
        seen.extend(run.feel.puffs.iter().rev().take(fresh).filter(|p| p.dust > 0).map(|p| p.dust));
    }
    assert_eq!(seen.first(), Some(&KICK), "{seen:?}");
    assert_eq!(seen.iter().filter(|&&d| d == KICK).count(), 1);
    assert_eq!(seen.iter().filter(|&&d| d == LAND).count(), 1, "{seen:?}");
    assert!(seen.contains(&TRAIL));
    // A knight walking leaves none.
    let mut run = quiet((1.0, 0.0), Weapon::Bow, cards::Guard::Roll);
    for _ in 0..20 {
        run.step(&hold(1, Input { move_x: 1, ..Default::default() }));
    }
    assert!(run.feel.puffs.iter().all(|p| p.dust == 0));
}

#[test]
fn a_hit_knows_its_way_a_death_its_monster_and_a_bomb_pops_what_it_clears() {
    let mut run = quiet((1.0, 0.0), Weapon::Crossbow, cards::Guard::Roll);
    run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 14.0);
    run.enemies[0].hp = 1;
    let fire = hold(1, Input { aim_x: 1, fire: true, ..Default::default() });
    let mut hit = None;
    let mut death = None;
    for _ in 0..20 {
        run.enemies.iter_mut().for_each(|e| e.frozen = 99);
        run.step(&fire);
        hit = hit.or_else(|| run.feel.impacts.first().copied());
        death = death.or_else(|| run.feel.deaths.first().copied());
    }
    let hit = hit.expect("a hit");
    assert!(hit.dx > 0.9 && hit.dy.abs() < 0.1, "going east: {hit:?}");
    assert_eq!(death.expect("a death").kind, Some(EnemyKind::Skeleton));
    // The bomb.
    let mut run = quiet((1.0, 0.0), Weapon::Bow, cards::Guard::Roll);
    run.players.get_mut(&1).unwrap().bombs = 2;
    for x in [21.0, 27.0] {
        run.projectiles.push(monster_shot(Shot::Wisp, (x, 12.0), (0.0, 0.1), 12));
    }
    run.step(&hold(1, Input { bomb: true, ..Default::default() }));
    assert!(run.projectiles.iter().all(|p| !p.hostile), "cleared");
    let pops: Vec<_> = run.feel.impacts.iter().filter(|i| i.dx == 0.0 && i.dy == 0.0).collect();
    assert_eq!(pops.len(), 2, "each popped where it hung");
}

#[test]
fn a_find_taken_glints_by_its_rarity() {
    let mut run = quiet((1.0, 0.0), Weapon::Bow, cards::Guard::Roll);
    let room = run.at;
    run.dungeon.rooms[room].items.push(crate::rules::Item { card: "potion".into(), x: 24.5, y: 14.0, held_off: None, took: Vec::new() });
    run.step(&BTreeMap::new());
    assert_eq!(run.feel.glints.len(), 1);
    assert_eq!(run.feel.glints[0].tier, 0);
    let (img, _) = seat_view(&run, 1, (24.0, 14.0));
    let mut bare = run.clone();
    bare.feel.glints.clear();
    let (plain, _) = seat_view(&bare, 1, (24.0, 14.0));
    assert!(img.words() != plain.words(), "it shows");
}

/// The busy fight with only what is wanted of it: its shots or not, its
/// effects or not. Coins and a rail's tracer are left out of all of them:
/// they are drawn over the shots, as before.
fn variant(run: &Run, shots: bool, effects: bool) -> Run {
    let mut run = run.clone();
    run.feel.coins.clear();
    run.feel.tracers.clear();
    if !shots {
        run.projectiles.clear();
    }
    if !effects {
        run.feel.impacts.clear();
        run.feel.deaths.clear();
        run.feel.puffs.clear();
        run.feel.blocks.clear();
        run.feel.glints.clear();
        run.blasts.clear();
        for hero in run.players.values_mut() {
            hero.bomb_cooldown = 0;
        }
    }
    run
}

#[test]
fn in_a_busy_fight_no_effect_hides_a_shot_and_every_seat_composes() {
    let mut run = busy_fight(150);
    let mut checked = 0;
    let mut effects = 0;
    for i in 0..40 {
        busy_step(&mut run);
        if i % 5 != 0 {
            continue;
        }
        effects += run.feel.impacts.len() + run.feel.puffs.len() + run.feel.deaths.len() + run.feel.blocks.len();
        let focus = Some(1 + (i / 5) % 4);
        let full = frame_for(&variant(&run, true, true), NATIVE_W, NATIVE_H, focus);
        let shots = frame_for(&variant(&run, true, false), NATIVE_W, NATIVE_H, focus);
        let empty = frame_for(&variant(&run, false, false), NATIVE_W, NATIVE_H, focus);
        for y in 0..NATIVE_H {
            for x in 0..NATIVE_W {
                let c = shots.get(x, y);
                if c != empty.get(x, y) && c.is_some_and(super::super::super::ink::is_signal) {
                    assert_eq!(full.get(x, y), c, "a shot's pixel at {x},{y}, tick {}", run.tick);
                    checked += 1;
                }
            }
        }
        let (w, h) = (NATIVE_W, NATIVE_H + BAND_H + HUD_H);
        let whole = seat_frame(&run, w, h, focus);
        assert!(seat_scene(&run, w, h, focus).compose().words() == whole.words(), "{focus:?} composes");
        assert!(whole.pixels().all(|c| c == super::super::super::ink::BLACK || super::super::super::ink::palette_index(c).is_some()));
    }
    assert!(checked > 500, "{checked}");
    assert!(effects > 20, "a busy fight: {effects}");
}
