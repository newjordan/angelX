use super::*;
use crate::rules::{Input, Item, RoomKind};
use std::collections::BTreeMap;

fn on_palette(img: &Img) -> bool {
    img.pixels()
        .all(|c| c == BLACK || super::super::ink::palette_index(c).is_some())
}

/// A run standing in the first room of `kind`, `ticks` into its fight.
fn scene(seed: u64, kind: RoomKind, ticks: u32) -> Run {
    let mut run = Run::new(seed, 1, Some("Friend"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == kind && !r.great())
        .or_else(|| run.dungeon.rooms.iter().position(|r| r.kind == kind))
        .expect("room kind on floor one");
    run.enter_for_test(index);
    let aim = BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            ..Default::default()
        },
    )]);
    for _ in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&aim);
    }
    run
}

#[test]
fn a_pastime_board_with_twenty_mistakes_draws_without_overflow() {
    use crate::rules::home::Home;
    use crate::rules::{pastimes::Game, world};
    let mut run = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    run.come_to_the_gate();
    for game in [Game::Broadsheet, Game::Barrel, Game::Turnip, Game::Washtub] {
        let mut r = run.clone();
        r.arrive(
            world::room_of(&r.dungeon, game.room()).unwrap(),
            game.start(),
        );
        let friend = r.players.get_mut(&2).unwrap();
        (friend.x, friend.y) = (42.0, 23.0);
        r.step(&BTreeMap::new());
        r.pastimes.round.as_mut().expect("the course began").mistakes = 20;
        let mut board = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        super::pastimes::boards(&mut board, &r);
    }
}

#[test]
fn arena_frames_are_deterministic_and_on_the_realm_palette() {
    for (w, h) in [(53, 30), (135, 78), (200, 110)] {
        let run = scene(11, RoomKind::Fight, 70);
        let a = frame(&run, w, h);
        assert_eq!((a.w, a.h), (w, h));
        assert_eq!(a, frame(&run, w, h));
        assert!(on_palette(&a), "{w}x{h}");
    }
    assert_eq!(frame(&scene(11, RoomKind::Start, 0), 0, 0).w, 0);
}

#[test]
fn shots_and_figures_show_at_full_ink_over_the_dark_room() {
    let run = scene(11, RoomKind::Fight, 90);
    assert!(run.projectiles.iter().any(|p| p.hostile));
    let img = frame(&run, 135, 78);
    let f = Field::fit(135, 78);
    for shot in run.projectiles.iter().filter(|p| p.hostile) {
        let (x, y) = f.px(shot.x, shot.y);
        let c = img.get(x, y).unwrap();
        assert!(
            super::super::ink::is_signal(c) || c == super::super::ink::ink('H').unwrap(),
            "shot at {x},{y} drawn {c:?}"
        );
    }
    let hero = &run.players[&1];
    let (x, y) = f.px(hero.x, hero.y);
    assert_eq!(img.get(x, y), super::super::ink::ink('w'), "the hit point");
}

#[test]
fn barred_doors_open_when_the_room_is_cleared() {
    let mut run = scene(11, RoomKind::Fight, 5);
    let door = (0..4).find(|&d| run.room().doors[d]).unwrap();
    let f = Field::fit(135, 78);
    let (col, row) = [(11, 0), (23, 6), (11, 13), (0, 6)][door];
    let (x, y) = f.px(
        (col as f32 + 0.5) * TILE_UNITS,
        (row as f32 + 0.5) * TILE_UNITS,
    );
    let lit = |img: &Img| {
        (-2..=2)
            .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| img.get(x + dx, y + dy).is_some_and(|c| c != BLACK))
            .count()
    };
    let barred = lit(&frame(&run, 135, 78));
    run.calm_for_test();
    // Slimes split as they fall, and geodes: fell everything that is left.
    for _ in 0..4 {
        for enemy in &mut run.enemies {
            enemy.hp = 0;
        }
        run.step(&BTreeMap::new());
    }
    let open = lit(&frame(&run, 135, 78));
    assert!(barred > open + 4, "bars {barred} vs open {open}");
}

/// Offline renders for review: `ANGEL_ARENA_SHOTS=<dir> cargo test ... -- --ignored`.
#[test]
#[ignore]
fn write_arena_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    let opening = Run::new(17, 1, None);
    let native = frame(&opening, 480, 280);
    image::RgbaImage::from_raw(480, 280, native.rgba_bytes())
        .unwrap()
        .save(dir.join("opening-native.png"))
        .unwrap();
    for (name, pack) in [
        ("crypt", Pack::Crypt),
        ("mines", Pack::Cavern),
        ("keep", Pack::Hellforge),
    ] {
        let mut run = opening.clone();
        run.dungeon.pack = pack;
        let img = frame(&run, 480, 320);
        image::RgbaImage::from_raw(480, 320, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("room-{name}.png")))
            .unwrap();
    }
    let fight = scene(11, RoomKind::Fight, 100);
    let native = frame(&fight, 480, 280);
    image::RgbaImage::from_raw(480, 280, native.rgba_bytes())
        .unwrap()
        .save(dir.join("fight-native.png"))
        .unwrap();
    for seed in [11, 12] {
        let run = scene(seed, RoomKind::Fight, 100);
        let pack = format!("{:?}", run.dungeon.pack).to_lowercase();
        save(
            &format!("fight_{pack}_{seed}.ppm"),
            &frame(&run, NATIVE_W, NATIVE_H),
        );
        save(
            &format!("fight_{pack}_{seed}_term.ppm"),
            &frame(&run, 135, 78),
        );
        save(
            &format!("fight_{pack}_{seed}_small.ppm"),
            &frame(&run, 53, 30),
        );
        save(
            &format!("telegraph_{pack}_{seed}.ppm"),
            &frame(&scene(seed, RoomKind::Fight, 14), NATIVE_W, NATIVE_H),
        );
    }
    let mut start = scene(11, RoomKind::Start, 0);
    let at = start.at;
    let y = start.players[&1].y - 5.0;
    let ids: Vec<String> = start.book.cards.iter().map(|c| c.id.clone()).collect();
    start.dungeon.rooms[at].items = ids
        .into_iter()
        .enumerate()
        .map(|(i, card)| Item {
            card,
            x: 4.0 + (i % 9) as f32 * 4.8,
            y: y - 6.0 + (i / 9) as f32 * 5.0,
            held_off: None,
        })
        .collect();
    save("loot.ppm", &frame(&start, NATIVE_W, NATIVE_H));
    let stairs = scene(11, RoomKind::Stairs, 0);
    save("stairs_barred.ppm", &frame(&stairs, NATIVE_W, NATIVE_H));
    let mut cleared = stairs.clone();
    cleared.clear_for_test();
    save("stairs_open.ppm", &frame(&cleared, NATIVE_W, NATIVE_H));
    save(
        "treasure.ppm",
        &frame(&scene(11, RoomKind::Treasure, 0), NATIVE_W, NATIVE_H),
    );
    for (name, ticks) in [("lair", 120), ("lair_late", 400)] {
        let mut run = scene(11, RoomKind::Start, 0);
        run.lair_for_test();
        for _ in 0..ticks {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
        save(&format!("{name}.ppm"), &frame(&run, NATIVE_W, NATIVE_H));
    }
    // The hippodrome on race day, beside the joust's shots: the field in
    // house banners at the gate, spread down the straights, and coming
    // round the west end — the SNES-floor sheets at braille density.
    {
        use crate::realm::chivalry::{Horse, Stable, bloodline};
        use crate::rules::chariot::GATE_TICKS;
        use crate::rules::home::{BUY_HOLD, Home};
        use crate::rules::world;
        let mut stable = Stable::default();
        stable.herd.push(Horse {
            name: "Gallery".into(),
            house: "qwen".into(),
            blood: bloodline("qwen"),
            ..Horse::default()
        });
        let home = Home { stable, ..Home::default() };
        let mut run = Run::at_home(7, 1, None, home, Default::default());
        let hip = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
        run.arrive(hip, (20.0, 10.5));
        run.step(&BTreeMap::new());
        let interact = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
        for _ in 0..BUY_HOLD + 2 {
            run.step(&interact);
        }
        save("hippodrome_gate.ppm", &frame(&run, NATIVE_W, NATIVE_H));
        let drive = BTreeMap::from([(1, Input { dash: true, ..Input::default() })]);
        for _ in 0..GATE_TICKS + 260 {
            run.step(&drive);
        }
        save("hippodrome_race.ppm", &frame(&run, NATIVE_W, NATIVE_H));
        save("hippodrome_race_term.ppm", &frame(&run, 135, 78));
        // Every course under the following camera, field strung out a
        // third of a lap in where the bends are interesting.
        for (t, track) in crate::rules::chariot::TRACKS.iter().enumerate() {
            {
                let race = run.chariot.as_mut().unwrap();
                race.track = t;
                for (i, racer) in race.racers.iter_mut().enumerate() {
                    racer.dist = track.lap * 0.32 + i as f32 * 85.0;
                }
                race.traps.push((track.lap * 0.32 - 45.0, -0.3));
            }
            let name = track.name.to_lowercase().replace(' ', "_");
            save(&format!("hippodrome_{name}.ppm"), &frame(&run, NATIVE_W, NATIVE_H));
            run.chariot.as_mut().unwrap().traps.clear();
        }
        // A friend at the reins of a rival team, from each seat's own view.
        {
            let mut coop = run.clone();
            coop.join_seat(2, "Friend");
            let race = coop.chariot.as_mut().unwrap();
            race.track = 1; // The Serpent
            race.seat_friend(2, "Friend");
            let lap = race.course().lap;
            race.racers[0].dist = lap * 0.32;
            race.racers[1].dist = lap * 0.32 + 70.0;
            race.racers[2].dist = lap * 0.32 + 170.0;
            race.racers[3].dist = lap * 0.32 - 120.0;
            save("hippodrome_coop_host.ppm", &frame_for(&coop, NATIVE_W, NATIVE_H, Some(1)));
            let seat = coop.view_for_seat(2).unwrap();
            save("hippodrome_coop_friend.ppm", &frame_for(&seat, NATIVE_W, NATIVE_H, Some(2)));
        }
        for _ in 0..4000 {
            run.step(&drive);
            if run.chariot.as_ref().is_some_and(|r| r.phase == crate::rules::chariot::Phase::Done) {
                break;
            }
        }
        save("hippodrome_finish.ppm", &frame(&run, NATIVE_W, NATIVE_H));
    }
}

#[test]
fn dungeon_native_pixels_keep_opening_enemies_visible_and_lazy_frame_exact() {
    let run = Run::new(17, 1, None);
    assert_eq!(run.enemies.len(), 2);
    let img = frame(&run, 480, 280);
    let lazy = LazyFrame::new(run.clone(), 480, 280, None);
    assert_eq!(lazy.as_ref(), img.rgba_bytes());
    let mut empty = run.clone();
    empty.enemies.clear();
    let empty_img = frame(&empty, 480, 280);
    let field = Field::fit(480, 280);
    for enemy in &run.enemies {
        let (x, y) = field.px(enemy.x, enemy.y);
        let mut changed = 0;
        for dy in -12..=12 {
            for dx in -12..=12 {
                if img.get(x + dx, y + dy) != empty_img.get(x + dx, y + dy) {
                    changed += 1;
                }
            }
        }
        assert!(
            changed >= 15,
            "opening enemy must be readable before moving: {changed}"
        );
    }
}

#[test]
fn the_room_is_drawn_at_overworld_scale_and_enlarged_by_whole_pixels() {
    assert_eq!(
        (NATIVE_W, NATIVE_H),
        (384, 224),
        "16 px tiles, as in the overworld"
    );
    let run = scene(11, RoomKind::Fight, 40);
    let one = frame(&run, NATIVE_W, NATIVE_H);
    let two = frame(&run, NATIVE_W * 2 + 6, NATIVE_H * 2 + 4);
    for (x, y) in [(0, 0), (100, 50), (383, 223), (190, 112)] {
        assert_eq!(one.get(x, y), two.get(3 + 2 * x, 2 + 2 * y));
        assert_eq!(one.get(x, y), two.get(3 + 2 * x + 1, 2 + 2 * y + 1));
    }
}

#[test]
fn scenery_keeps_to_material_banks_on_black_paper() {
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge] {
        let mut run = scene(11, RoomKind::Fight, 0);
        run.dungeon.pack = pack;
        run.enemies.clear();
        run.players.clear();
        run.projectiles.clear();
        let img = scenery(&run);
        assert!(on_palette(&img), "{pack:?}");
        let paper = img.pixels().filter(|c| *c == BLACK).count();
        assert!(
            paper > img.pixels().count() / 2,
            "{pack:?}: drawn on black paper, never filled ({paper})"
        );
        // Only fire may burn in the signal bank: braziers and torches.
        let signal = img
            .pixels()
            .filter(|&c| super::super::ink::is_signal(c))
            .count();
        assert!(signal < 400, "{pack:?}: {signal} signal pixels of scenery");
    }
}

/// The bestiary reads beside the knight (`stature`): every monster at rest
/// stands at least `FOE_MIN` tall and shows on black paper, no frame or pose
/// of one shrinks to a dot, a slime's last split is not drawn under its art, every
/// boss's own art is taller than a knight, and a boss stands at least a
/// knight and a half tall, the floor's guardian two knights.
#[test]
fn every_foe_reads_beside_the_knight() {
    use super::stature::{CHIEF, FOE_MIN, GUARDIAN, KNIGHT};
    use crate::rules::EnemyKind as K;
    use crate::rules::bestiary::ENTRIES;
    // The box round what is drawn, and the share of it lighter than `g`.
    let seen = |im: &Img| {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
        for y in 0..im.h {
            for x in 0..im.w {
                if im.get(x, y).is_some() {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        (x1 - x0 + 1, y1 - y0 + 1)
    };
    let lit = |im: &Img| {
        let inked: Vec<_> = (0..im.h)
            .flat_map(|y| (0..im.w).filter_map(move |x| im.get(x, y)))
            .collect();
        let lit = inked
            .iter()
            .filter(|c| c.iter().max() >= Some(&0x50))
            .count();
        lit * 100 / inked.len().max(1)
    };
    assert_eq!(super::super::kit::knight().h, KNIGHT);
    for entry in &ENTRIES {
        let kind = entry.kind;
        if matches!(kind, K::Dragon | K::PitTyrant) {
            continue;
        }
        let rest = sprites::enemy(kind, 0);
        assert!(
            seen(&rest).1 >= FOE_MIN,
            "{kind:?} stands {:?} at rest",
            seen(&rest)
        );
        for tick in [0, 4, 8, 16, 24, 48] {
            let im = sprites::enemy(kind, tick);
            let (w, h) = seen(&im);
            assert!(
                w.max(h) >= FOE_MIN && w.min(h) >= FOE_MIN / 2,
                "{kind:?} at {tick} is {w}x{h}"
            );
            assert!(lit(&im) >= 33, "{kind:?} is {}% lit", lit(&im));
        }
        if kind.critter() {
            for stage in [0, 1, 2, 3, 4, 9] {
                for (t, tick) in [(0, 0), (12, 5), (40, 9)] {
                    let (w, h) = seen(&critters::figure(kind, stage, t, tick));
                    assert!(
                        w.max(h) >= FOE_MIN && w.min(h) >= FOE_MIN / 2,
                        "{kind:?} stage {stage} is {w}x{h}"
                    );
                }
            }
        }
    }
    let split = super::stature::monster(
        K::Slime,
        sprites::enemy(K::Slime, 0),
        0.55 / sprites::SLIME_REACH,
    );
    assert!(split.h >= FOE_MIN, "the last split is {} px", split.h);
    for pack in [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Hellforge,
        Pack::Archive,
        Pack::Fungal,
        Pack::Unknown,
    ] {
        for (kind, floor) in [(RoomKind::Fight, CHIEF), (RoomKind::Stairs, GUARDIAN)] {
            let mut run = Run::new(21, 1, None);
            run.begin_in(pack);
            let at = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == kind)
                .unwrap();
            run.enter_for_test(at);
            run.enemies.clear();
            for index in run.pack_bosses_for_test(pack) {
                let art = run.bosses[index].art.len() as i32;
                assert!(art > KNIGHT, "{}: art {art} rows", run.bosses[index].id);
                run.spawn_boss_at_for_test(index, 20.0, 12.0);
            }
            if kind == RoomKind::Fight {
                run.spawn_at_for_test(K::PitTyrant, 20.0, 12.0);
            }
            assert!(!run.enemies.is_empty());
            for e in &run.enemies {
                let h = foe_figure(&run, e, 0).h;
                assert!(
                    h >= floor,
                    "{pack:?} {kind:?} {:?} {:?}: {h} px",
                    e.kind,
                    e.boss
                );
            }
        }
    }
    let mut lair = scene(11, RoomKind::Start, 0);
    lair.lair_for_test();
    let dragon = lair.enemies.iter().find(|e| e.kind == K::Dragon).unwrap();
    assert!(foe_figure(&lair, dragon, 0).h >= GUARDIAN);
}

#[test]
fn the_heroes_are_the_mini_viz_knight() {
    let mut run = scene(11, RoomKind::Fight, 0);
    run.enemies.clear();
    run.projectiles.clear();
    // Toward the viewer, at rest with nothing in hand, the arena's knight is
    // the mini-viz's to the pixel; any other way they stand is the same
    // hero turned, stepping or armed.
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.aim_x, hero.aim_y) = (0.0, 1.0);
        hero.weapon = crate::rules::Weapon::Fists;
        hero.forged = None;
        hero.stride = 0.0;
        hero.shielding = false;
    }
    assert_eq!(
        knights::figure(&run.players[&1], 1).rgb_bytes(),
        super::super::kit::knight().rgb_bytes(),
        "the realm's knight at rest"
    );
    let img = native(&run);
    let hero = &run.players[&1];
    let (x, y) = at(hero.x, hero.y);
    let knight = super::super::kit::knight();
    let (ox, oy) = (x - knight.w / 2, y - knight.h + 1);
    let mut same = 0;
    for ky in 0..knight.h {
        for kx in 0..knight.w {
            if let Some(c) = knight.get(kx, ky)
                && img.get(ox + kx, oy + ky) == Some(c)
            {
                same += 1;
            }
        }
    }
    assert!(same > 100, "the realm's knight, pixel for pixel: {same}");
}

#[test]
fn a_great_hall_is_seen_through_a_camera_that_follows_its_knight() {
    let mut run = Run::new(0, 1, Some("Friend"));
    let stairs = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(stairs);
    assert!(run.room().great());
    let (w, h) = (run.room().width(), run.room().height());
    run.players.get_mut(&1).unwrap().x = 3.0;
    run.players.get_mut(&1).unwrap().y = 3.0;
    run.players.get_mut(&2).unwrap().x = w - 3.0;
    run.players.get_mut(&2).unwrap().y = h - 3.0;
    assert_eq!(
        camera(&run, Some(1)),
        (0, 0),
        "clamped to the hall's corner"
    );
    let far = camera(&run, Some(2));
    assert_eq!(
        far,
        (
            run.room().cols as i32 * 16 - NATIVE_W,
            run.room().rows as i32 * 16 - NATIVE_H
        )
    );
    let view = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    assert_eq!(
        (view.w, view.h),
        (NATIVE_W, NATIVE_H),
        "one screen, whatever the hall"
    );
    // The other knight is out of sight, so the view's edge points to them.
    let (_, over) = layers(&run, Some(1), true);
    let blue = super::super::ink::ink('1').unwrap();
    let edge = (0..NATIVE_W)
        .flat_map(|x| [(x, NATIVE_H - 5), (x, NATIVE_H - 4)])
        .chain((0..NATIVE_H).map(|y| (NATIVE_W - 5, y)));
    assert!(
        edge.into_iter().any(|(x, y)| over.get(x, y) == Some(blue)),
        "a mark toward the friend"
    );
}

#[test]
fn an_ordinary_room_never_moves_the_camera() {
    let run = scene(11, RoomKind::Fight, 0);
    assert!(!run.room().great());
    assert_eq!(camera(&run, Some(1)), (0, 0));
    assert_eq!(camera(&run, None), (0, 0));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_great_hall_shots -- --ignored`
#[test]
#[ignore]
fn write_great_hall_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    for seed in [0u64, 4] {
        let mut run = Run::new(seed, 1, Some("Friend"));
        let stairs = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(stairs);
        for _ in 0..60 {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
        let scenery = {
            let (pw, ph) = (run.room().cols as i32 * 16, run.room().rows as i32 * 16);
            let mut whole = scenery(&run);
            let mut figs = Img::new(pw, ph);
            figures(&mut figs, &run);
            whole.stamp(&figs, 0, 0);
            whole
        };
        save(&format!("great_whole_{seed}.ppm"), &scenery);
        save(
            &format!("great_view_p1_{seed}.ppm"),
            &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
        );
        let hero = run.players.get_mut(&2).unwrap();
        hero.x = 6.0;
        hero.y = 6.0;
        save(
            &format!("great_view_p2_{seed}.ppm"),
            &frame_for(&run, NATIVE_W, NATIVE_H, Some(2)),
        );
    }
}

#[test]
#[ignore]
fn write_home_shots() {
    use crate::realm::{Spoil, Spoils};
    use crate::rules::home::{Home, Station};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut treasury = Spoils::default();
    for (s, n) in [
        (Spoil::Gold, 420),
        (Spoil::Ore, 5),
        (Spoil::Wax, 9),
        (Spoil::Bone, 3),
    ] {
        treasury.add(s, n);
    }
    let bare = Run::at_home(17, 1, None, Home::default(), treasury.clone());
    save("home-bare.png", &native(&bare));
    let mut built = Home::default();
    for (s, l) in [
        (Station::Forge, 3),
        (Station::Hearth, 2),
        (Station::Rack, 4),
        (Station::Chapel, 1),
        (Station::Map, 2),
    ] {
        built.levels.insert(s, l);
    }
    built.deepest = 3;
    let mut run = Run::at_home(17, 1, Some("Friend"), built, treasury.clone());
    // Knight one reads the forge's ledger, half-way through a purchase.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (8.0, 7.0);
    hero.buying = 10;
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = (24.0, 17.0);
    for _ in 0..3 {
        run.tick += 1;
    }
    save("home-built.png", &native(&run));
    let mut hearth = run.clone();
    let hero = hearth.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (8.0, 21.0);
    hero.buying = 0;
    save("home-hearth-ledger.png", &native(&hearth));
}

#[test]
#[ignore]
fn write_fortune_shots() {
    use crate::realm::{Spoil, Spoils};
    use crate::rules::fortune::Mode;
    use crate::rules::home::{Home, Station};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 900);
    treasury.add(Spoil::Gem, 6);
    let mut home = Home::default();
    home.levels.insert(Station::Wheel, 1);
    let mut run = Run::at_home(17, 1, Some("Friend"), home, treasury);
    run.enter_for_test(1);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 17.0);
    save("fortune-hall.png", &native(&run));
    // Pull the lever.
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..crate::rules::home::BUY_HOLD + 1 {
        run.step(&fire);
    }
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    save("fortune-spinning.png", &native(&run));
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    save("fortune-landed.png", &native(&run));
    eprintln!("landed on {:?}", run.mode);
    // Lights Out and Giant's Feast on floor one.
    for (name, mode) in [
        ("lights-out", Mode::LightsOut),
        ("giants", Mode::GiantsFeast),
    ] {
        let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Spoils::default());
        run.mode = mode;
        run.descend_for_test();
        let fight = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight && !r.great())
            .unwrap();
        run.enter_for_test(fight);
        for _ in 0..80 {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
        save(&format!("mode-{name}.png"), &native(&run));
    }
}

#[test]
#[ignore]
fn write_ult_shots() {
    use crate::rules::{EnemyKind, knights};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    for (knight, wait) in [
        ("percival", 6),
        ("lynette", 4),
        ("gareth", 20),
        ("galahad", 9),
        ("composer", 2),
        ("dispatcher", 8),
        ("loop", 10),
        ("money", 2),
        ("competition", 12),
        ("scryglass", 6),
    ] {
        let mut run = Run::new(21, 1, Some("Friend"));
        run.outfit(1, knights::knight(knight).unwrap());
        let fight = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight && !r.great())
            .unwrap();
        run.enter_for_test(fight);
        run.calm_for_test();
        run.enemies.clear();
        for (kind, x, y) in [
            (EnemyKind::Skeleton, 22.0, 10.0),
            (EnemyKind::Imp, 26.0, 17.0),
            (EnemyKind::Demon, 34.0, 12.0),
            (EnemyKind::Wraith, 18.0, 20.0),
        ] {
            run.spawn_at_for_test(kind, x, y);
        }
        for (id, (x, y)) in [(1, (12.0, 14.0)), (2, (10.0, 18.0))] {
            let hero = run.players.get_mut(&id).unwrap();
            (hero.x, hero.y) = (x, y);
            (hero.aim_x, hero.aim_y) = (1.0, 0.0);
        }
        run.players.get_mut(&1).unwrap().ult_charge =
            crate::rules::ults::ULT_FULL;
        let ult = BTreeMap::from([(
            1,
            Input {
                ult: true,
                ..Default::default()
            },
        )]);
        run.step(&ult);
        for _ in 0..wait {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
        save(&format!("ult-{knight}.png"), &native(&run));
    }
}

#[test]
#[ignore]
fn write_foe_shots() {
    use crate::rules::EnemyKind as K;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    for (name, pack, foes, ticks) in [
        (
            "sapper",
            Pack::Cavern,
            vec![(K::Sapper, 34.0, 10.0), (K::Sapper, 30.0, 20.0)],
            125,
        ),
        ("necro", Pack::Crypt, vec![(K::Necromancer, 36.0, 8.0)], 100),
        ("boar", Pack::Cavern, vec![(K::Warboar, 40.0, 12.0)], 72),
        (
            "slime",
            Pack::Crypt,
            vec![(K::Slime, 34.0, 10.0), (K::Slime, 14.0, 18.0)],
            20,
        ),
        (
            "goblin",
            Pack::Hellforge,
            vec![(K::Goblin, 30.0, 10.0), (K::Imp, 36.0, 18.0)],
            40,
        ),
        (
            "hob",
            Pack::Cavern,
            vec![(K::Hob, 36.0, 8.0), (K::Hob, 38.0, 20.0)],
            52,
        ),
        ("shaman", Pack::Hellforge, vec![(K::Shaman, 38.0, 6.0)], 98),
    ] {
        let mut run = Run::new(33, 1, Some("Friend"));
        run.dungeon.pack = pack;
        run.enter_for_test(0);
        run.calm_for_test();
        run.enemies.clear();
        for (kind, x, y) in foes {
            run.spawn_at_for_test(kind, x, y);
        }
        for (id, (x, y)) in [(1, (20.0, 14.0)), (2, (16.0, 17.0))] {
            let hero = run.players.get_mut(&id).unwrap();
            (hero.x, hero.y) = (x, y);
        }
        run.phase = crate::rules::Phase::Fighting;
        if name == "slime" {
            // One already split.
            run.enemies[1].hp = 0;
        }
        for _ in 0..ticks {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
        save(&format!("foe-{name}.png"), &native(&run));
    }
}

#[test]
#[ignore]
fn write_deep_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let calm = |run: &mut Run, ticks: u32| {
        for _ in 0..ticks {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
    };
    let mut run = Run::new(9, 1, Some("Friend"));
    run.lair_for_test();
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    calm(&mut run, 6);
    save("deep-lair-light.png", &native(&run));
    for (name, kind) in [
        ("archive", RoomKind::Fight),
        ("fungal", RoomKind::Fight),
        ("unknown", RoomKind::Fight),
    ] {
        run.descend_for_test();
        let room = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == kind && !r.great())
            .or_else(|| run.dungeon.rooms.iter().position(|r| r.kind == kind))
            .unwrap();
        run.enter_for_test(room);
        calm(&mut run, 70);
        save(&format!("deep-{name}.png"), &native(&run));
        if name != "unknown" {
            let stairs = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == RoomKind::Stairs)
                .unwrap();
            run.enter_for_test(stairs);
            run.calm_for_test();
            let w = run.room().width();
            for (id, x) in [(1, w / 2.0 - 3.0), (2, w / 2.0 + 3.0)] {
                let hero = run.players.get_mut(&id).unwrap();
                (hero.x, hero.y) = (x, 16.0);
            }
            calm(&mut run, 50);
            save(&format!("deep-{name}-guardian.png"), &native(&run));
        }
    }
    let threshold = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Threshold)
        .unwrap();
    run.enter_for_test(threshold);
    for (id, x) in [(1, 44.0), (2, 52.0)] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x, 12.0);
    }
    calm(&mut run, 140);
    save("deep-threshold.png", &native(&run));
}

#[test]
#[ignore]
fn write_feat_shots() {
    use crate::rules::feats::Tier;
    use crate::rules::home::Home;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let home = Home {
        boxes: vec![Tier::Gold, Tier::Silver, Tier::Bronze, Tier::Legendary],
        ..Home::default()
    };
    let mut run = Run::at_home(5, 1, Some("Friend"), home, Default::default());
    run.enter_for_test(1);
    let (c, r, w, h) = crate::rules::fortune::COFFER_PLATE;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * 2.0;
    hero.y = (r as f32 + h as f32 / 2.0) * 2.0;
    run.step(&BTreeMap::new());
    save("coffer-reading.png", &native(&run));
    let mut spoils = crate::realm::Spoils::default();
    spoils.add(crate::realm::Spoil::Gold, 260);
    spoils.add(crate::realm::Spoil::Gem, 2);
    run.home.boxes.remove(0);
    run.unbox(Tier::Gold, &spoils);
    for _ in 0..18 {
        run.step(&BTreeMap::new());
    }
    save("coffer-unboxing.png", &native(&run));
    // A banner in a fight.
    let mut run = Run::new(5, 1, Some("Friend"));
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(fight);
    run.announce("splitting_headache");
    for _ in 0..30 {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
    }
    save("feat-banner.png", &native(&run));
}

#[test]
#[ignore]
fn write_yard_shots() {
    use crate::rules::home::Home;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    for (name, goblins) in [("yard-closed", 0), ("yard-grubbins", 1)] {
        let home = Home {
            goblins,
            ..Home::default()
        };
        let mut treasury = crate::realm::Spoils::default();
        treasury.add(crate::realm::Spoil::Gold, 200);
        let mut run = Run::at_home(8, 1, Some("Friend"), home, treasury);
        let yard = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Yard)
            .unwrap();
        run.enter_for_test(yard);
        let q = crate::rules::yard::QUINTAINS[1];
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (q.0 * 2.0, q.1 * 2.0 + 7.0);
        if goblins > 0 {
            let (c, r, w, _) = crate::rules::yard::STALL_PLATES[0];
            let friend = run.players.get_mut(&2).unwrap();
            (friend.x, friend.y) = ((c as f32 + w as f32 / 2.0) * 2.0, (r as f32 + 0.5) * 2.0);
        }
        let fire = BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                fire: true,
                ..Default::default()
            },
        )]);
        for _ in 0..50 {
            run.step(&fire);
        }
        save(&format!("{name}.png"), &native(&run));
    }
}

#[test]
#[ignore]
fn write_audience_shots() {
    use crate::rules::audience::{BOX_FALL, MILESTONE};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::new(33, 1, Some("Friend"));
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    for (id, (x, y)) in [(1, (20.0, 14.0)), (2, (16.0, 17.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x, y);
    }
    run.phase = crate::rules::Phase::Exploring;
    run.thrill_for_test(MILESTONE);
    for (name, ticks) in [
        ("high", 6),
        ("low", BOX_FALL / 2),
        ("landed", BOX_FALL / 2 - 4),
    ] {
        for _ in 0..ticks {
            run.step(&BTreeMap::new());
        }
        save(&format!("fan-box-{name}.png"), &native(&run));
    }
}

#[test]
#[ignore]
fn write_bounty_shots() {
    use crate::rules::bounties::Pinned;
    use crate::rules::home::Home;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut home = Home {
        deepest: 3,
        ..Default::default()
    };
    home.bounties = vec![
        Pinned {
            id: "bones".into(),
            have: 17,
        },
        Pinned {
            id: "ratings".into(),
            have: 640,
        },
        Pinned {
            id: "dragon".into(),
            have: 0,
        },
    ];
    let mut run = Run::at_home(17, 1, Some("Friend"), home, Default::default());
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = (12.0, 21.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (22.0, 14.0);
    save("bounty-board-idle.png", &native(&run));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (33.0, 21.5);
    run.tick += 1;
    save("bounty-board-reading.png", &native(&run));
}

#[test]
#[ignore]
fn write_hunter_shots() {
    use crate::rules::EnemyKind as K;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let fight = |pack: Pack, foes: &[(K, f32, f32)]| {
        let mut run = Run::new(33, 1, Some("Friend"));
        run.dungeon.pack = pack;
        run.enter_for_test(0);
        run.calm_for_test();
        run.enemies.clear();
        for &(kind, x, y) in foes {
            run.spawn_at_for_test(kind, x, y);
        }
        for (id, (x, y)) in [(1, (18.0, 14.0)), (2, (14.0, 20.0))] {
            let hero = run.players.get_mut(&id).unwrap();
            (hero.x, hero.y) = (x, y);
        }
        run.phase = crate::rules::Phase::Fighting;
        run
    };
    let hold = |run: &mut Run| {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
    };
    let mut run = fight(Pack::Unknown, &[(K::Flesher, 34.0, 12.0)]);
    let mut said = [false; 3];
    for _ in 0..8 * crate::rules::HZ {
        hold(&mut run);
        let stare = run
            .enemies
            .iter()
            .any(|e| e.kind == K::Flesher && e.stage == 1);
        if stare && !said[0] && run.tick % 2 == 0 {
            save("hunter-flesher-stare.png", &native(&run));
            said[0] = true;
        }
        if run.hooks.iter().any(|h| !h.back && h.out > 6.0) && !said[1] {
            save("hunter-flesher-hook.png", &native(&run));
            said[1] = true;
        }
        if run.hooks.iter().any(|h| h.caught.is_some() && h.back) && !said[2] {
            for _ in 0..4 {
                hold(&mut run);
            }
            save("hunter-flesher-reel.png", &native(&run));
            said[2] = true;
        }
    }
    let mut run = fight(Pack::Archive, &[(K::Silkmother, 36.0, 8.0)]);
    for _ in 0..13 * crate::rules::HZ {
        hold(&mut run);
    }
    save("hunter-brood.png", &native(&run));
}

#[test]
#[ignore]
fn write_trophy_shots() {
    use crate::rules::home::Home;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let hall_of = |home: Home| {
        let mut run = Run::at_home(17, 1, Some("Friend"), home, Default::default());
        let index = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Trophies)
            .unwrap();
        run.enter_for_test(index);
        run
    };
    let mut run = hall_of(Home::default());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (20.0, 6.5);
    save("trophy-hall-empty.png", &native(&run));
    let mut home = Home::default();
    for (id, n) in [
        ("waxen-warden", 12),
        ("cinderjaw", 4),
        ("the-index", 1),
        ("the-bone-choir", 2),
        ("the-foreman", 5),
        ("late-fee-leviathan", 1),
        ("mother-of-spores", 1),
        ("dragon", 3),
        ("grail", 1),
    ] {
        home.trophies.insert(id.into(), n);
    }
    let mut run = hall_of(home);
    for (id, (x, y)) in [(1, (10.0, 9.5)), (2, (30.0, 20.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x, y);
    }
    run.tick += 1;
    save("trophy-hall.png", &native(&run));
    let home_room = Run::at_home(17, 1, None, Home::default(), Default::default());
    save("undercroft-south-door.png", &native(&home_room));
}

#[test]
#[ignore]
fn write_lesson_shots() {
    use crate::rules::home::Home;
    use crate::rules::talents::{LECTERNS, LEVELS, Prowess};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut home = Home::default();
    home.knights.insert(
        "you".into(),
        Prowess {
            xp: LEVELS[1] + 200,
            learned: vec![1],
        },
    );
    let mut run = Run::at_home(17, 1, Some("Friend"), home, Default::default());
    let yard = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Yard)
        .unwrap();
    run.enter_for_test(yard);
    let (c, r, w, _) = LECTERNS[0];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((c as f32 + w as f32 / 2.0) * 2.0, (r as f32 + 0.5) * 2.0);
    hero.buying = 12;
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = (24.0, 20.0);
    run.tick += 1;
    save("lesson-ector.png", &native(&run));
}

#[test]
#[ignore]
fn write_dare_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::new(12, 1, Some("Friend"));
    run.descend_for_test();
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    save("dare-called.png", &native(&run));
}

#[test]
#[ignore]
fn write_pit_shots() {
    use crate::rules::EnemyKind as K;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    // A floor with a pit, its Pit Tyrant awake.
    let mut run = Run::new(3, 1, Some("Friend"));
    let mut pit = None;
    for _ in 0..4 {
        run.descend_for_test();
        pit = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Pit);
        if pit.is_some() {
            break;
        }
    }
    let pit = pit.expect("a pit within four floors");
    run.enter_for_test(pit);
    for (id, (x, y)) in [(1, (18.0, 20.0)), (2, (30.0, 21.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..(9 * crate::rules::HZ) {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
        if !run.slams.is_empty() && run.slams[0].left < 12 {
            break;
        }
    }
    save("pit-slam.png", &native(&run));
    for enemy in run.enemies.iter_mut().filter(|e| e.kind == K::PitTyrant) {
        enemy.hp = 0;
    }
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    save("pit-talisman.png", &native(&run));
}

#[test]
#[ignore]
fn write_guardian_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    for (id, pack) in [
        ("the-bone-choir", Pack::Crypt),
        ("the-foreman", Pack::Cavern),
    ] {
        for raid in 0..32 {
            let mut run = Run::new(5, raid, Some("Friend"));
            run.begin_in(pack);
            let stairs = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == RoomKind::Stairs)
                .unwrap();
            run.enter_for_test(stairs);
            let meets = run
                .enemies
                .iter()
                .find_map(|e| e.boss)
                .is_some_and(|i| run.bosses[usize::from(i)].id == id);
            if !meets {
                continue;
            }
            // Close enough that the camera has the guardian in view.
            let (bx, by) = run
                .enemies
                .iter()
                .find(|e| e.boss.is_some())
                .map(|e| (e.x, e.y))
                .unwrap();
            for (k, hero) in run.players.values_mut().enumerate() {
                (hero.x, hero.y) = (bx - 3.0 + k as f32 * 6.0, by + 9.0);
            }
            for _ in 0..70 {
                for hero in run.players.values_mut() {
                    hero.hp = hero.max_hp;
                }
                run.step(&BTreeMap::new());
            }
            save(&format!("guardian-{id}.png"), &native(&run));
            break;
        }
    }
}

#[test]
#[ignore]
fn write_tallow_shots() {
    use crate::rules::home::{Home, Station};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut home = Home::default();
    home.levels.insert(Station::Hearth, 2);
    let mut run = Run::at_home(13, 1, None, home, Default::default());
    save("tallow-home.png", &native(&run), 3);
    run.descend_for_test();
    run.calm_for_test();
    run.enemies.clear();
    let room = run.at;
    for (k, (x, y)) in [(30.0, 20.0), (33.0, 21.0), (12.0, 9.0)]
        .into_iter()
        .enumerate()
    {
        run.dungeon.rooms[room]
            .items
            .push(crate::rules::Item {
                card: if k == 2 { "potion" } else { "gold" }.into(),
                x,
                y,
                held_off: None,
            });
    }
    for _ in 0..12 {
        run.step(&BTreeMap::new());
    }
    save("tallow-fetching.png", &native(&run), 3);
}

#[test]
#[ignore]
fn write_rescue_shots() {
    use crate::rules::home::{Home, Station};
    use crate::rules::rescues::Captive;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::new(4, 1, Some("Friend"));
    run.begin_in(Pack::Crypt);
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
    for (id, (hx, hy)) in [(1, (x - 2.0, y + 3.0)), (2, (x + 3.0, y + 4.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (hx, hy);
    }
    run.tick += 1;
    save("rescue-caged.png", &native(&run));
    run.captive.as_mut().unwrap().freed = Some(run.tick);
    run.tick += 20;
    save("rescue-freed.png", &native(&run));
    let mut home = Home::default();
    home.levels.insert(Station::Hearth, 2);
    for who in ["mabel", "anselm", "pip", "maud"] {
        home.residents.insert(who.into());
    }
    let run = Run::at_home(4, 1, None, home, Default::default());
    save("rescue-residents.png", &native(&run));
}

/// Power runes: the eight orbs side by side, one on a fight room's floor
/// with a hasted knight, motes gathering before one wells up, and a knight
/// under Invisibility among the monsters.
#[test]
#[ignore]
fn write_rune_shots() {
    use crate::rules::runes::{Held, Rune, RuneKind};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut sheet = Img::black(8 * 15 + 3, 17);
    for (i, kind) in RuneKind::ALL.iter().enumerate() {
        let orb = runes::orb(*kind);
        sheet.stamp(&orb, 3 + i as i32 * 15, 3);
    }
    save("rune-orbs.png", &sheet, 10);
    let mut run = Run::new(4, 1, Some("Friend"));
    run.begin_in(Pack::Cavern);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    let (w, h) = (run.room().width(), run.room().height());
    for (id, (hx, hy)) in [(1, (w * 0.35, h * 0.6)), (2, (w * 0.3, h * 0.45))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (hx, hy);
    }
    run.players.get_mut(&2).unwrap().rune = Some(Held {
        kind: RuneKind::Haste,
        left: RuneKind::Haste.lasts(),
    });
    run.rune = Some(Rune {
        kind: RuneKind::DoubleDamage,
        x: w * 0.55,
        y: h * 0.5,
        at: run.tick + 10,
    });
    run.tick += 1;
    save("rune-gathering.png", &native(&run), 3);
    run.tick += 20;
    save("rune-floor.png", &native(&run), 3);
    run.rune = None;
    run.players.get_mut(&1).unwrap().rune = Some(Held {
        kind: RuneKind::Invisibility,
        left: RuneKind::Invisibility.lasts(),
    });
    run.tick += 7;
    save("rune-invisible.png", &native(&run), 3);
}

/// Fortune's wheel with its third rung bought: eleven wedges, the third rung's
/// three among them, and the wheel at rest on Turbo.
#[test]
#[ignore]
fn write_third_rung_wheel_shots() {
    use crate::realm::Spoils;
    use crate::rules::fortune::{Mode, SPIN_TICKS, Spin};
    use crate::rules::home::{Home, Station};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut home = Home::default();
    home.levels.insert(Station::Wheel, 3);
    let mut run = Run::at_home(17, 1, Some("Friend"), home, Spoils::default());
    run.enter_for_test(1);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 17.0);
    save("fortune-wheel-third-rung.png", &native(&run));
    // And with the fourth rung: fourteen wedges.
    let mut four = Home::default();
    four.levels.insert(Station::Wheel, 4);
    let mut wheel = Run::at_home(17, 1, None, four, Spoils::default());
    wheel.enter_for_test(1);
    let hero = wheel.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 17.0);
    save("fortune-wheel-iv.png", &native(&wheel));
    let turbo = Mode::WHEEL.iter().position(|&m| m == Mode::Turbo).unwrap() as u8;
    run.spin = Some(Spin {
        settled: false,
        started: run.tick,
        from: 0.0,
        wedge: turbo,
        turns: 3,
        by: 1,
    });
    for _ in 0..SPIN_TICKS + 20 {
        run.step(&BTreeMap::new());
    }
    save("fortune-landed-turbo.png", &native(&run));
    // Fortune's ledger, selling her third rung.
    let mut home = Home {
        deepest: 3,
        ..Home::default()
    };
    home.levels.insert(Station::Wheel, 2);
    let mut run = Run::at_home(17, 1, None, home, Spoils::default());
    run.enter_for_test(1);
    let (c, r, w, h) = crate::rules::fortune::FORTUNE_PLATE;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * 2.0;
    hero.y = (r as f32 + h as f32 / 2.0) * 2.0;
    run.step(&BTreeMap::new());
    save("fortune-ledger-third-rung.png", &native(&run));
}

/// The Hexer: in a Crypt room with her bolt on its way, and the knight it
/// found, a frog in a tiny helmet.
#[test]
#[ignore]
fn write_hexer_shots() {
    use crate::rules::EnemyKind;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::new(4, 1, Some("Friend"));
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.rune = None;
    run.spawn_at_for_test(EnemyKind::Hexer, 32.0, 12.0);
    for (id, (x, y)) in [(1, (14.0, 14.0)), (2, (12.0, 19.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (x, y, 0);
    }
    let mut bolt = false;
    for _ in 0..8 * crate::rules::HZ {
        for hero in run.players.values_mut() {
            (hero.hp, hero.invulnerable) = (hero.max_hp, 0);
        }
        run.step(&BTreeMap::new());
        let flying = run
            .projectiles
            .iter()
            .any(|p| p.kind == crate::rules::Shot::Hex && p.x < 27.0);
        if flying && !bolt {
            bolt = true;
            save("hexer-bolt.png", &native(&run));
        }
        if run.players.values().any(|h| h.frog()) {
            break;
        }
    }
    for _ in 0..8 {
        run.step(&BTreeMap::new());
    }
    save("hexer-frog.png", &native(&run));
}

/// Secret rooms: the cracked wall in its fight room, the wall blown by a
/// knight's bomb, and the vault behind it.
#[test]
#[ignore]
fn write_secret_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = (0..200u64)
        .map(|seed| {
            let mut run = Run::new(seed, 1, None);
            run.begin_in(Pack::Crypt);
            run
        })
        .find(|run| run.dungeon.secret.is_some())
        .unwrap();
    let secret = run.dungeon.secret.unwrap();
    run.enter_for_test(secret.host);
    run.calm_for_test();
    run.rune = None;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    let (_, cx, cy) = run.dungeon.crack().unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    let (w, h) = (
        run.dungeon.rooms[secret.host].width(),
        run.dungeon.rooms[secret.host].height(),
    );
    (hero.x, hero.y) = (
        (cx * 0.6 + w * 0.2).clamp(4.0, w - 4.0),
        (cy * 0.6 + h * 0.2).clamp(4.0, h - 4.0),
    );
    save("secret-crack.png", &native(&run));
    run.step(&BTreeMap::from([(
        1,
        Input {
            bomb: true,
            ..Default::default()
        },
    )]));
    for _ in 0..4 {
        run.step(&BTreeMap::new());
    }
    save("secret-blown.png", &native(&run));
    run.enter_for_test(secret.vault);
    run.step(&BTreeMap::new());
    save("secret-vault.png", &native(&run));
    // Snibbet's vault, and the hush money.
    let mut run = (0..400u64)
        .map(|seed| {
            let mut run = Run::new(seed, 1, None);
            run.begin_in(Pack::Crypt);
            run
        })
        .find(|run| run.dungeon.secret.is_some_and(|s| s.snibbet))
        .unwrap();
    let secret = run.dungeon.secret.unwrap();
    run.dungeon.open_secret();
    run.enter_for_test(secret.vault);
    let (sx, sy) = crate::rules::secrets::snibbet_at(run.room());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (sx, sy + 9.0);
    run.step(&BTreeMap::new());
    save("secret-snibbet.png", &native(&run));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (sx + 1.0, sy + 4.5);
    for _ in 0..6 {
        run.step(&BTreeMap::new());
    }
    save("secret-snibbet-paid.png", &native(&run));
}

/// Merlin by the Winding Stair, a knight walking up to him.
#[test]
#[ignore]
fn write_merlin_shot() {
    use crate::rules::home::{Home, Station};
    use crate::rules::merlin::MERLIN_AT;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut home = Home::default();
    home.levels.insert(Station::Hearth, 2);
    home.levels.insert(Station::Forge, 1);
    for who in ["mabel", "anselm"] {
        home.residents.insert(who.into());
    }
    let mut run = Run::at_home(4, 1, None, home, Default::default());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (MERLIN_AT.0 * 2.0 + 3.0, MERLIN_AT.1 * 2.0 + 2.5);
    run.step(&BTreeMap::new());
    let img = native(&run);
    let k = 3u32;
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
        .unwrap()
        .save(dir.join("home-merlin.png"))
        .unwrap();
}

/// The west wing: the Trophy Hall's rubble and Tobbin's plate (its ledger
/// up), the tavern with Maud at her bar, the rumour board read, and a knight
/// at the Siege Perilous; and the dry tavern before Maud is home.
#[test]
#[ignore]
fn write_tavern_shots() {
    use crate::rules::home::{Home, Station};
    use crate::rules::tavern::{BOARD, SIEGE, WING_PLATE};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let centre = |(c, r, w, h): (i32, i32, i32, i32)| {
        (
            (c as f32 + w as f32 / 2.0) * 2.0,
            (r as f32 + h as f32 / 2.0) * 2.0,
        )
    };
    let room = |run: &Run, kind: RoomKind| {
        run.dungeon
            .rooms
            .iter()
            .position(|r| r.kind == kind)
            .unwrap()
    };
    // Before: the rubble, the plate, its ledger.
    let home = Home {
        deepest: 1,
        ..Home::default()
    };
    let mut run = Run::at_home(5, 1, None, home, Default::default());
    run.enter_for_test(room(&run, RoomKind::Trophies));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = centre(WING_PLATE);
    run.step(&BTreeMap::new());
    save("wing-rubble.png", &native(&run));
    // After: the tavern, Maud at her bar.
    let mut home = Home {
        deepest: 2,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    home.residents.insert("maud".into());
    let mut run = Run::at_home(5, 1, Some("Friend"), home.clone(), Default::default());
    run.enter_for_test(room(&run, RoomKind::Tavern));
    let (bx, by) = centre(BOARD);
    let (sx, sy) = centre(SIEGE);
    for (id, (x, y)) in [(1, (bx, by + 3.5)), (2, (sx + 3.0, sy - 1.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x, y);
    }
    run.step(&BTreeMap::new());
    save("tavern.png", &native(&run));
    // A knight at the stout's tap, the hold half done.
    let tap = crate::rules::tavern::TAPS[1];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = centre(tap);
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..10 {
        run.step(&fire);
    }
    save("tavern-tap.png", &native(&run));
    // Sir Dinadan's stage: a knight at the Iron Hymn's plate, a song
    // already asked for, so the notes rise.
    let plate = crate::rules::tavern::SONG_PLATES[1];
    let mut asked = run.home.clone();
    asked.song = Some("haste".into());
    let treasury = run.treasury.clone();
    run.rebuild_home(asked, treasury);
    for (id, at) in [(1, centre(plate)), (2, (30.0, 22.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (at.0, at.1, 0);
    }
    run.step(&BTreeMap::new());
    save("tavern-stage.png", &native(&run));
    // Down the stair with the song: two knights close enough to hear it.
    run.descend_for_test();
    run.calm_for_test();
    for (id, at) in [(1, (20.0, 14.0)), (2, (24.0, 16.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (at.0, at.1, 0);
    }
    run.step(&BTreeMap::new());
    save("song-heard.png", &native(&run));
    // The dry tavern, Maud still in her cage.
    home.residents.clear();
    let mut run = Run::at_home(5, 1, None, home, Default::default());
    run.enter_for_test(room(&run, RoomKind::Tavern));
    run.step(&BTreeMap::new());
    save("tavern-dry.png", &native(&run));
}

/// The Lich in the Drowned Archive: Rimeleap leaping between two
/// knights, the first of them rimed with chill.
#[test]
#[ignore]
fn write_lich_shots() {
    use crate::rules::{EnemyKind, Tile};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::new(6, 1, Some("Friend"));
    run.begin_in(Pack::Crypt);
    for _ in 0..3 {
        run.descend_for_test();
    }
    assert_eq!(run.dungeon.pack, Pack::Archive);
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
    // Open floor near each wanted spot.
    let open = |run: &Run, (x, y): (f32, f32)| -> (f32, f32) {
        let room = run.room();
        let (c0, r0) = ((x / 2.0) as i32, (y / 2.0) as i32);
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
                ((c as f32 + 0.5) * 2.0, (r as f32 + 0.5) * 2.0)
            })
    };
    let (lx, ly) = open(&run, (34.0, 14.0));
    run.spawn_at_for_test(EnemyKind::Lich, lx, ly);
    if let Some(lich) = run.enemies.last_mut() {
        lich.age = 1000;
        lich.timer = 0;
    }
    for (id, spot) in [(1, (14.0, 11.0)), (2, (15.0, 18.0))] {
        let (x, y) = open(&run, spot);
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (x, y, 0);
    }
    let mut struck = 0;
    for _ in 0..10 * crate::rules::HZ {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
            hero.invulnerable = 0;
        }
        let chilled: u32 = run.players.values().map(|h| u32::from(h.chilled > 0)).sum();
        run.step(&BTreeMap::new());
        let now: u32 = run.players.values().map(|h| u32::from(h.chilled > 0)).sum();
        if now > chilled {
            struck += 1;
        }
        let leaping = run
            .projectiles
            .iter()
            .any(|p| p.kind == crate::rules::Shot::Frost);
        if leaping && struck >= 2 {
            save("lich-chain.png", &native(&run));
            return;
        }
    }
    save("lich-chain.png", &native(&run));
}

/// The item cards on a floor, and a knight under a Pendragon Sceptre.
#[test]
#[ignore]
fn write_item_shots() {
    use crate::rules::Item;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = scene(4, RoomKind::Start, 0);
    run.calm_for_test();
    for (k, card) in ["fae-dagger", "pendragon-sceptre", "censer"]
        .iter()
        .enumerate()
    {
        run.dungeon.rooms[run.at].items.push(Item {
            card: (*card).into(),
            x: 18.0 + k as f32 * 6.0,
            y: 10.0,
            held_off: None,
        });
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.immune) = (24.0, 18.0, 100);
    run.tick += 1;
    let img = native(&run);
    let k = 3u32;
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
        .unwrap()
        .save(dir.join("items.png"))
        .unwrap();
}

/// The Late-Fee Leviathan on the Drowned Archive's stairs: its Ravage
/// rippling, then bursting ring by ring, and a knight thrown.
#[test]
#[ignore]
fn write_ravage_shots() {
    use crate::rules::{Pack, tide};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    for raid in 0..32 {
        let mut run = Run::new(5, raid, Some("Friend"));
        run.begin_in(Pack::Crypt);
        for _ in 0..3 {
            run.descend_for_test();
        }
        let stairs = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(stairs);
        let Some(boss) = run.enemies.iter_mut().find(|e| {
            e.boss
                .is_some_and(|i| run.bosses[usize::from(i)].id == "late-fee-leviathan")
        }) else {
            continue;
        };
        // A beat before its Ravage.
        boss.age = 209;
        let (bx, by) = (boss.x, boss.y);
        run.calm_for_test();
        run.enemies.retain(|e| e.boss.is_some());
        // Fortune's dare board would hide the guardian.
        run.dare = None;
        for (k, hero) in run.players.values_mut().enumerate() {
            (hero.x, hero.y) = (bx - 4.0 + k as f32 * 9.0, by + 5.0 + k as f32 * 2.0);
        }
        let mut shots = [
            (20, "ravage-ripples.png"),
            (tide::WARN + 11, "ravage-bursting.png"),
        ]
        .into_iter()
        .peekable();
        let mut thrown = false;
        for _ in 0..4 * crate::rules::HZ {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
            // No hit's blink in a still.
            for hero in run.players.values_mut() {
                hero.invulnerable = 0;
            }
            let age = run.ravages.first().map_or(0, |r| r.age);
            if age == 1 {
                // The lane toward knight one, so the still shows one knight
                // in it and the other not.
                let hero = &run.players[&1];
                let ravage = &mut run.ravages[0];
                ravage.lane = (hero.y - ravage.y).atan2(hero.x - ravage.x);
            }
            if let Some(&(at, name)) = shots.peek()
                && age == at
            {
                save(name, &native(&run));
                shots.next();
            }
            if !thrown && run.players.values().any(|h| h.tossed == tide::TOSSED / 2) {
                save("ravage-thrown.png", &native(&run));
                thrown = true;
            }
        }
        return;
    }
    panic!("no delve met the Leviathan");
}

/// Beaumains: at his table in the tavern with a knight on his plate, then
/// hired and waiting by the Winding Stair, then fighting beside the party,
/// and felled.
#[test]
#[ignore]
fn write_hireling_shots() {
    use crate::rules::hireling::HIRE_PLATE;
    use crate::rules::home::{Home, Station};
    use crate::rules::{EnemyKind, HZ};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut home = Home {
        deepest: 2,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    home.residents.insert("maud".into());
    let mut run = Run::at_home(5, 1, Some("Friend"), home.clone(), Default::default());
    let tavern = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Tavern)
        .unwrap();
    run.enter_for_test(tavern);
    let (c, r, w, h) = HIRE_PLATE;
    let plate = (
        (c as f32 + w as f32 / 2.0) * 2.0,
        (r as f32 + h as f32 / 2.0) * 2.0,
    );
    for (id, at) in [(1, plate), (2, (24.0, 14.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (at.0, at.1, 0);
    }
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..12 {
        run.step(&fire);
    }
    save("beaumains-table.png", &native(&run));
    // Hired: he waits by the stair.
    home.hire = Some("beaumains".into());
    let mut run = Run::at_home(5, 1, None, home, Default::default());
    run.step(&BTreeMap::new());
    save("beaumains-stair.png", &native(&run));
    // Down the stair, in a fight beside the knight.
    run.descend_for_test();
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    // Fortune's dare board would hide the top of the room.
    run.dare = None;
    // The knight in the middle of the room, Beaumains at his side, and the
    // monsters coming on.
    let (w, h) = (run.room().width(), run.room().height());
    run.players.get_mut(&1).unwrap().x = w / 2.0 - 4.0;
    run.players.get_mut(&1).unwrap().y = h / 2.0;
    if let Some(hire) = run.hireling.as_mut() {
        (hire.x, hire.y) = (w / 2.0 - 6.0, h / 2.0 + 2.0);
    }
    let knives = |run: &Run| {
        run.projectiles
            .iter()
            .filter(|p| !p.hostile && p.kind == crate::rules::Shot::Blade)
            .count()
    };
    for _ in 0..6 * HZ {
        run.players.get_mut(&1).unwrap().hp = 100;
        run.step(&BTreeMap::new());
        if let Some(hire) = run.hireling.as_mut() {
            hire.hp = hire.max_hp;
        }
        if knives(&run) >= 2 {
            break;
        }
    }
    assert!(knives(&run) > 0, "his knives in the air");
    if let Some(hire) = run.hireling.as_mut() {
        hire.invulnerable = 0;
    }
    save("beaumains-fighting.png", &native(&run));
    // Felled, he lies where he fell.
    if let Some(hire) = run.hireling.as_mut() {
        hire.hp = 0;
    }
    if !run.enemies.iter().any(|e| e.kind != EnemyKind::Dummy) {
        run.spawn_at_for_test(EnemyKind::Bat, 44.0, 3.0);
    }
    run.step(&BTreeMap::new());
    save("beaumains-down.png", &native(&run));
}

/// The Hollow One and its Black Hole: the warning ring, then the hole open
/// with two knights in its pull.
#[test]
#[ignore]
fn write_hollow_shots() {
    use crate::rules::hollow::{HOLE_PULL, HOLE_WARN, Hole};
    use crate::rules::{EnemyKind, Pack};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::new(6, 1, Some("Friend"));
    run.begin_in(Pack::Crypt);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.dare = None;
    run.spawn_at_for_test(EnemyKind::Hollow, 34.0, 8.0);
    let hollow = run.enemies.last_mut().unwrap();
    (hollow.stage, hollow.timer) = (1, 0);
    let (id, hp) = (hollow.id, hollow.hp);
    for (k, (x, y)) in [(1, (18.0, 15.0)), (2, (24.0, 18.0))] {
        let hero = run.players.get_mut(&k).unwrap();
        (hero.x, hero.y, hero.invulnerable) = (x, y, 0);
    }
    run.holes.push(Hole {
        x: 21.0,
        y: 15.0,
        warn: HOLE_WARN,
        left: HOLE_PULL,
        owner: id,
        owner_hp: hp,
    });
    for _ in 0..HOLE_WARN / 2 {
        run.step(&BTreeMap::new());
    }
    save("hollow-warning.png", &native(&run));
    for _ in 0..HOLE_WARN / 2 + 30 {
        for hero in run.players.values_mut() {
            (hero.hp, hero.invulnerable) = (hero.max_hp, 0);
        }
        if let Some(h) = run.enemies.iter_mut().find(|e| e.id == id) {
            h.timer = 0;
        }
        run.step(&BTreeMap::new());
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    save("hollow-hole.png", &native(&run));
}

/// A sheet of the overworld kit's world pieces, for building world rooms
/// in the realm's own hand: `ANGEL_ARENA_SHOTS=<dir> ... write_world_kit_sheet`.
#[test]
#[ignore]
fn write_world_kit_sheet() {
    use super::super::kit::{self, Heraldry, House, Roof, Trade, Wall};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tiles = kit::Tiles::get();
    let pieces: Vec<Img> = vec![
        kit::lists_ground(),
        kit::tent(Heraldry::Red),
        kit::tent(Heraldry::Blue),
        kit::rider(Heraldry::Red),
        kit::rider(Heraldry::Blue),
        kit::mine_mouth(),
        kit::gatehouse(),
        kit::workshop(Trade::Smith, true),
        kit::cottage(1, true),
        kit::chapel(true),
        kit::market_stall(1),
        kit::well(),
        kit::lantern(true),
        kit::quintain(false, 0),
        kit::villager(0, true),
        kit::squire('B'),
        kit::keep(1),
        kit::house(&House {
            w: 48,
            h: 40,
            roof_h: 18,
            wall: Wall::Timber,
            roof: Roof::Thatch,
            door_glow: false,
            windows: 2,
            lit: true,
            chimney: true,
        }),
        tiles.tree(1).clone(),
        tiles.tree(2).clone(),
        kit::barrels(),
        kit::handcart(),
        kit::sacks(3),
        kit::wagon(true, 0, None),
    ];
    let (w, mut x, mut y, mut row_h) = (520, 4, 4, 0);
    let mut sheet = Img::black(w, 400);
    for p in &pieces {
        if x + p.w > w {
            x = 4;
            y += row_h + 6;
            row_h = 0;
        }
        sheet.stamp(p, x, y);
        x += p.w + 6;
        row_h = row_h.max(p.h);
    }
    let k = 3u32;
    image::RgbaImage::from_raw(sheet.w as u32 * k, sheet.h as u32 * k, sheet.rgba_scaled(k))
        .unwrap()
        .save(dir.join("world-kit.png"))
        .unwrap();
}

/// The world above the Delve, room by room, and the Undercroft's stair up:
/// `ANGEL_ARENA_SHOTS=<dir> ... write_world_shots`.
#[test]
#[ignore]
fn write_world_shots() {
    use crate::rules::home::Home;
    use crate::rules::world;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    run.come_to_the_gate();
    // Follow the corrected northern roads through the real travel step. The
    // setup starts just inside the origin's southern door; no arrival is
    // injected into the affected room. Capture the complete four-knight fan.
    for (origin, destination, label) in [
        (RoomKind::Caravan, RoomKind::Lists, "lists"),
        (RoomKind::Mummers, RoomKind::MineHead, "mine-head"),
    ] {
        let mut r = run.clone();
        r.join_seat(3, "Lady Sandwich");
        r.join_seat(4, "Goose Inspector");
        r.arrive(world::room_of(&r.dungeon, origin).unwrap(), (12.0, 12.0));
        for _ in 0..crate::rules::HZ {
            if r.room().kind != origin { break; }
            // Seat two starts within the two-tile southern doorway; the
            // outer seats wait inside the origin until the party crosses.
            r.step(&BTreeMap::from([(2, Input { move_y: 1, ..Default::default() })]));
        }
        assert_eq!(r.room().kind, destination);
        // Arrival grants a brief blinking grace. Let it expire through the
        // normal step before claiming the capture shows four visible bodies.
        // No movement or artificial sprite/lighting state is injected here.
        for _ in 0..crate::rules::HZ {
            r.step(&BTreeMap::new());
        }
        assert_eq!(r.room().kind, destination);
        for hero in r.players.values() {
            assert!(r.settlement_walkable_for_test(hero.x, hero.y));
            assert_eq!(hero.invulnerable, 0);
        }
        assert!(r.valid_snapshot());
        // Verify the scenery gap and the animated crowd independently of
        // knight/HUD pixels and of renderer lighting or PNG compression.
        if destination == RoomKind::Lists {
            let mut gap = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
            let mut lights = Vec::new();
            super::world::lists(&mut gap, &mut lights, r.tick as u32, true);
            joust::crowd(&mut gap, &r);
            for y in TILE..3 * TILE { for x in 9 * TILE..15 * TILE {
                assert!(gap.get(x, y).is_none(), "bench or spectator obscures the northern road at {x},{y}");
            } }
        } else {
            let mut mouth = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
            let mut lights = Vec::new();
            super::world::mine_head(&mut mouth, &r, &mut lights, r.tick as u32);
            for y in TILE..3 * TILE { for x in 9 * TILE..15 * TILE {
                assert_eq!(mouth.get(x, y), crate::render::overworld::ink::ink(super::world::road_mark(destination, x, y).unwrap_or('k')), "rock or timber obscures the northern road at {x},{y}");
            } }
        }
        save(&format!("world-road-{label}-four-arrivals.png"), &native(&r));
        save(&format!("world-road-{label}-scenery.png"), &super::world::scenery(&r));
    }
    for _ in 0..7 {
        run.tick += 1;
    }
    save("world-gate.png", &native(&run));
    // A knight at the gate's stair, its board up and its ring half full.
    let mut stair = run.clone();
    let (gx, gy) = world::entrance_centre(&world::ENTRANCES[0]);
    let hero = stair.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (gx, gy);
    stair.descending = 15;
    save("world-gate-stair.png", &native(&stair));
    for (kind, name) in [
        (RoomKind::Stables, "world-stables.png"),
        (RoomKind::Lists, "world-lists.png"),
        (RoomKind::MineHead, "world-mine-head.png"),
        (RoomKind::Forest, "world-forest.png"),
        (RoomKind::Village, "world-village.png"),
        (RoomKind::Fairground, "world-fairground.png"),
        (RoomKind::Riverbank, "world-riverbank.png"),
        (RoomKind::Windmill, "world-windmill.png"),
        (RoomKind::Ruins, "world-ruins.png"),
        (RoomKind::Meadow, "world-meadow.png"),
        (RoomKind::Orchard, "world-orchard.png"),
        (RoomKind::Marsh, "world-marsh.png"),
        (RoomKind::Downs, "world-downs.png"),
        (RoomKind::Cliffs, "world-cliffs.png"),
        (RoomKind::Causeway, "world-causeway.png"),
        (RoomKind::Priory, "world-priory.png"),
    ] {
        let mut r = run.clone();
        let i = world::room_of(&r.dungeon, kind).unwrap();
        r.arrive(i, (12.0, 9.5));
        save(name, &native(&r));
    }
    // The host's real step starts each course; capture its shared objectives
    // and instruction board, not a separately invented screenshot state.
    use crate::rules::{countryside, pastimes::Game};
    for (game, name) in [
        (Game::Broadsheet, "world-broadsheet-round.png"),
        (Game::Barrel, "world-barrel-round.png"),
        (Game::Turnip, "world-turnip-round.png"),
        (Game::Washtub, "world-washtub-round.png"),
        (Game::Cheese, "world-cheese-round.png"),
        (Game::Hobbyhorse, "world-hobbyhorse-round.png"),
    ] {
        let mut r = run.clone();
        r.arrive(
            world::room_of(&r.dungeon, game.room()).unwrap(),
            game.start(),
        );
        let friend = r.players.get_mut(&2).unwrap();
        (friend.x, friend.y) = (42.0, 23.0);
        r.step(&BTreeMap::new());
        assert_eq!(r.pastimes.round.as_ref().unwrap().game, game);
        assert!(r.valid_snapshot());
        save(name, &native(&r));
    }
    // Illustration fixtures for the four completed works. No live treasury is
    // charged and no operator game is started by the image exporter.
    for (plot, name) in countryside::PLOTS.iter().zip([
        "world-bakery-built.png",
        "world-beacon-built.png",
        "world-inn-built.png",
        "world-ferry-built.png",
        "world-lift-built.png",
        "world-post-built.png",
    ]) {
        let mut r = run.clone();
        r.home.countryside.built = 63;
        r.home.countryside.picnic = 3;
        r.home.countryside.picnic_paid = true;
        r.home.countryside.survey = 7;
        r.home.countryside.survey_paid = true;
        r.home.countryside.visited = 63;
        r.home.countryside.relics = 7;
        r.home.countryside.lamps = 7;
        r.home.countryside.lamps_paid = true;
        r.arrive(world::room_of(&r.dungeon, plot.room).unwrap(), plot.at);
        let mut instructions = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        super::countryside::boards(&mut instructions, &r);
        let before_tray = instructions.clone();
        tray::corner(&mut instructions, &r, None);
        for y in 0..instructions.h {
            for x in 0..instructions.w {
                if before_tray.get(x, y).is_some() {
                    assert_eq!(
                        instructions.get(x, y),
                        before_tray.get(x, y),
                        "the icon tray overwrites {} instructions at ({x}, {y})",
                        plot.id
                    );
                }
            }
        }
        save(name, &native(&r));
    }
    for (i, lamp) in countryside::LAMPS.iter().enumerate() {
        let mut r = run.clone();
        r.home.countryside.built = 2;
        r.home.countryside.relics = 2;
        r.arrive(world::room_of(&r.dungeon, lamp.room).unwrap(), lamp.at);
        let dark = scenery_cached_exact(&r);
        r.home.countryside.lamps = 1 << i;
        let lit = scenery_cached_exact(&r);
        assert_ne!(dark, lit, "lighting invalidates the scenery cache");
        assert!(r.valid_snapshot());
        save(&format!("world-lantern-{}.png", i + 1), &native(&r));
    }
    for (i, &(kind, stake)) in countryside::SURVEY.iter().enumerate() {
        let mut r = run.clone();
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 6.0));
        let friend = r.players.get_mut(&2).unwrap();
        (friend.x, friend.y) = (stake.0 * 2.0, stake.1 * 2.0);
        r.step(&BTreeMap::from([(
            2,
            Input {
                interact: true,
                ..Input::default()
            },
        )]));
        assert_eq!(r.marks.get(&format!("country:survey:{i}")), Some(&1));
        assert!(r.valid_snapshot());
        save(&format!("world-survey-{}.png", i + 1), &native(&r));
    }
    let mut r = run.clone();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Downs).unwrap(),
        Game::Cheese.start(),
    );
    let friend = r.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = (42.0, 23.0);
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            aim_x: 1,
            ..Input::default()
        },
    )]));
    for _ in 0..12 {
        r.step(&BTreeMap::new());
    }
    assert!(r.pastimes.round.as_ref().unwrap().cheese.is_some());
    assert!(r.valid_snapshot());
    save("world-cheese-flight.png", &native(&r));
    eastern_world_shots(&run, &save);
    northern_world_shots(&run, &save);
    coastal_world_shots(&run, &save);
    woodland_world_shots(&run, &save);
    wetland_world_shots(&run, &save);
    aerie_world_shots(&run, &save);
    mummers_world_shots(&run, &save);
    hippodrome_world_shots(&run, &save);
    windcountry_world_shots(&run, &save);
    for done in [false, true] {
        let mut r = run.clone();
        let plot = &countryside::PLOTS[4];
        r.home.countryside.survey = if done { 7 } else { 0 };
        r.home.countryside.survey_paid = done;
        r.arrive(world::room_of(&r.dungeon, plot.room).unwrap(), plot.at);
        let mut instructions = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        super::countryside::boards(&mut instructions, &r);
        let before = instructions.clone();
        tray::corner(&mut instructions, &r, None);
        for y in 0..instructions.h {
            for x in 0..instructions.w {
                if before.get(x, y).is_some() {
                    assert_eq!(
                        before.get(x, y),
                        instructions.get(x, y),
                        "lift plot text overlaps tray"
                    );
                }
            }
        }
        assert!(r.valid_snapshot());
        save(
            if done {
                "world-lift-ready.png"
            } else {
                "world-lift-locked.png"
            },
            &native(&r),
        );
    }
    // The Undercroft with its stair up to the gate, a knight before it.
    let mut cellar = run.clone();
    let e = &world::ENTRANCES[0];
    cellar.take_entrance(e);
    save("world-undercroft-stair-up.png", &native(&cellar));
}

/// The stables and a bout at the lists, played through real keys:
/// `ANGEL_ARENA_SHOTS=<dir> ... write_joust_shots`.
#[test]
#[ignore]
fn write_joust_shots() {
    use crate::realm::chivalry::{Mount, Stable};
    use crate::rules::home::{BUY_HOLD, Home};
    use crate::rules::joust::{Aim, Stage};
    use crate::rules::world;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let keys = |fire: bool, dash: bool, move_y: i8| {
        BTreeMap::from([(
            1,
            Input {
                fire,
                dash,
                move_y,
                ..Input::default()
            },
        )])
    };
    // The stables: Cinder saddled and tended in the yard, a knight reading
    // Mist's stall, a friend by the groom.
    let home = Home {
        stable: Stable {
            selected: Mount::Cinder,
            tended: [false, true, false],
            ..Stable::default()
        },
        ..Home::default()
    };
    let mut run = Run::at_home(17, 1, Some("Friend"), home, Default::default());
    let stables = world::room_of(&run.dungeon, RoomKind::Stables).unwrap();
    run.arrive(stables, (12.0, 9.0));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (18.0 * 2.0, 4.5 * 2.0);
    hero.buying = 12;
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = (8.5 * 2.0, 9.5 * 2.0);
    for _ in 0..9 {
        run.tick += 1;
    }
    save("joust-stables.png", &native(&run));
    // The lists at rest, a knight on the mount plate.
    let mut run = Run::at_home(17, 1, None, Home::default(), Default::default());
    let lists = world::room_of(&run.dungeon, RoomKind::Lists).unwrap();
    run.arrive(lists, (1.9, 8.5));
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().buying = 14;
    save("joust-lists-plate.png", &native(&run));
    run.players.get_mut(&1).unwrap().buying = 0;
    for _ in 0..BUY_HOLD + 1 {
        run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    }
    run.step(&keys(false, false, 0));
    save("joust-ready.png", &native(&run));
    // The first course: spur, aim at the open target, strike at the moment.
    run.step(&keys(true, false, 0));
    let m = run.joust.as_ref().unwrap().meeting();
    let guard = run.joust.as_ref().unwrap().guard;
    let aim = if guard == Aim::High {
        Aim::Low
    } else {
        Aim::High
    };
    let dy = if aim == Aim::High { -1 } else { 1 };
    let mut shots = vec![(m / 2, "joust-charge.png"), (m - 4, "joust-closing.png")];
    let window = Mount::Bramble
        .steed()
        .strike
        .max(Mount::Bramble.steed().brace);
    shots.push((m + 6, "joust-clash.png"));
    shots.push((m + window + 3, "joust-impact.png"));
    shots.push((m + window + 16, "joust-after.png"));
    for _ in 0..400 {
        let j = run.joust.as_ref().unwrap();
        if j.stage != Stage::Charge {
            break;
        }
        let next = j.t + 1;
        run.step(&keys(next == m, false, dy));
        let t = run.joust.as_ref().map_or(0, |j| j.t);
        if let Some((_, name)) = shots.iter().find(|(at, _)| *at == t) {
            save(name, &native(&run));
        }
    }
    // The second course, the same way: Sir Kay goes over.
    run.step(&keys(true, false, 0));
    let guard = run.joust.as_ref().unwrap().guard;
    let aim = if guard == Aim::High {
        Aim::Low
    } else {
        Aim::High
    };
    let dy = if aim == Aim::High { -1 } else { 1 };
    for _ in 0..400 {
        let Some(j) = run.joust.as_ref() else { break };
        if j.stage != Stage::Charge {
            break;
        }
        let next = j.t + 1;
        run.step(&keys(next == m, false, dy));
        let t = run.joust.as_ref().map_or(0, |j| j.t);
        if t == m + window + 9 {
            save("joust-unhorse.png", &native(&run));
        }
        if t == m + window + 30 {
            save("joust-on-the-sand.png", &native(&run));
        }
    }
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    save("joust-verdict.png", &native(&run));
}

/// King Brannoc's barony, stage by stage: his camp at the mine-head, his
/// mission, the gate-hall rising, his hall inside, and a forge-hall below
/// from ruin to burning: `ANGEL_ARENA_SHOTS=<dir> ... write_barony_shots`.
#[test]
#[ignore]
fn write_barony_shots() {
    use crate::realm::{Spoil, Spoils};
    use crate::rules::barony::{self, Court, Work};
    use crate::rules::home::Home;
    use crate::rules::{Pack, world};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut treasury = Spoils::default();
    for (s, n) in [
        (Spoil::Gold, 2400),
        (Spoil::Ore, 52),
        (Spoil::Bone, 12),
        (Spoil::Gem, 9),
        (Spoil::Ember, 5),
    ] {
        treasury.add(s, n);
    }
    let in_room = |home: Home, kind: RoomKind, at: (f32, f32), tick: u64| {
        let mut run = Run::at_home(17, 1, None, home, treasury.clone());
        let i = world::room_of(&run.dungeon, kind).unwrap();
        run.arrive(i, at);
        run.tick = tick;
        run
    };
    let mut b = barony::Barony::default();
    b.court = Court::Camped;
    let home = |b: &barony::Barony| Home {
        barony: b.clone(),
        deepest: 3,
        ..Home::default()
    };
    // His camp, the night he came up the shaft.
    let run = in_room(home(&b), RoomKind::MineHead, (12.0, 11.5), 40);
    save("barony-camp.png", &native(&run));
    // Met: his mission at the plate before him.
    b.court = barony::Court::Met;
    b.mission = Some(crate::rules::bounties::Pinned {
        id: "workings".into(),
        have: 5,
    });
    let run = in_room(home(&b), RoomKind::MineHead, (8.0, 10.5), 41);
    save("barony-mission.png", &native(&run));
    // The Upper Workings his: miners going down, porters coming up; a
    // knight reading the ruin's price.
    b.mission = None;
    b.done.push("workings".into());
    let mut run = in_room(home(&b), RoomKind::MineHead, (6.0, 7.5), 300);
    run.players.get_mut(&1).unwrap().buying = 9;
    save("barony-hall-plate.png", &native(&run));
    // The gate-hall going up: rubble, walls, roof, front, and done.
    for (p, tick) in [(10u32, 130u64), (40, 260), (65, 400), (90, 520)] {
        let mut bb = b.clone();
        let labour = barony::work("hall").unwrap().labour * p / 100;
        bb.works = vec![Work {
            id: "hall".into(),
            labour,
        }];
        let run = in_room(home(&bb), RoomKind::MineHead, (15.0, 11.0), tick);
        save(&format!("barony-hall-{p}.png"), &native(&run));
    }
    let mut sworn = b.clone();
    sworn.works = vec![Work {
        id: "hall".into(),
        labour: barony::work("hall").unwrap().labour,
    }];
    sworn.court = Court::Sworn;
    let run = in_room(home(&sworn), RoomKind::MineHead, (15.0, 11.0), 300);
    save("barony-hall-done.png", &native(&run));
    // Inside: the King on his seat; the war table; the ledger.
    let mut inside = sworn.clone();
    let mut paid = Spoils::default();
    paid.add(Spoil::Gold, 3000);
    paid.add(Spoil::Ore, 40);
    inside.ledger.paid_in = paid.clone();
    inside.ledger.lines.push(barony::Entry {
        what: "Jordan paid for The King's Hall".into(),
        spoils: paid,
        paid_in: true,
    });
    let mut back = Spoils::default();
    back.add(Spoil::Gold, 330);
    back.add(Spoil::Ore, 31);
    inside.ledger.paid_back = back.clone();
    inside.ledger.lines.push(barony::Entry {
        what: "Tribute: 3 floors cleared".into(),
        spoils: back,
        paid_in: false,
    });
    inside.works.push(Work {
        id: "ore-forge".into(),
        labour: 1800,
    });
    let run = in_room(home(&inside), RoomKind::KingsHall, (12.0, 5.0), 60);
    save("barony-throne.png", &native(&run));
    let run = in_room(home(&inside), RoomKind::KingsHall, (6.0, 10.6), 61);
    save("barony-war-table.png", &native(&run));
    let run = in_room(home(&inside), RoomKind::KingsHall, (3.5, 4.6), 62);
    save("barony-ledger.png", &native(&run));
    // A forge-hall below: in ruin, rising, rebuilt and cold, burning.
    let forge_shot = |bb: &barony::Barony, name: &str, at: (f32, f32), tick: u64| {
        let mut run = Run::at_home(17, 1, None, home(bb), treasury.clone());
        run.dungeon.pack = Pack::Cavern;
        run.descend_for_test();
        let i = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Forge)
            .unwrap();
        run.enter_for_test(i);
        // Fortune's dare board would cover the chimney: a shot of the room.
        run.dare = None;
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (at.0 * 2.0, at.1 * 2.0);
        run.tick = tick;
        save(name, &native(&run));
    };
    forge_shot(&b, "forge-ruin.png", (12.0, 8.5), 20);
    let mut building = b.clone();
    building.works = vec![
        sworn.works[0].clone(),
        Work {
            id: "ore-forge".into(),
            labour: barony::work("ore-forge").unwrap().labour / 2,
        },
    ];
    forge_shot(&building, "forge-building.png", (12.0, 11.0), 33);
    let mut cold = building.clone();
    cold.works[1].labour = barony::work("ore-forge").unwrap().labour;
    forge_shot(&cold, "forge-cold.png", (12.0, 8.5), 21);
    let mut lit = cold.clone();
    lit.lit.push("ore-forge".into());
    forge_shot(&lit, "forge-lit.png", (12.0, 11.5), 44);
    // The realm's share of the ore, out through the gate courtyard.
    let mut market = lit.clone();
    market.court = Court::Sworn;
    for tick in (0..3000u64).step_by(5) {
        let figs = crate::rules::folk::figures(
            &home(&market),
            RoomKind::Gate,
            Pack::Cavern,
            tick,
        );
        if figs.iter().any(|f| f.loaded && f.y > 12.0 && f.y < 14.0) {
            let run = in_room(home(&market), RoomKind::Gate, (9.0, 10.0), tick);
            save("barony-carter.png", &native(&run));
            break;
        }
    }
}

/// Frame time of the world's rooms at their busiest, against the
/// Undercroft's: `cargo test --release frame_time_of_the_world -- --ignored --nocapture`.
#[test]
#[ignore]
fn frame_time_of_the_world() {
    use crate::rules::barony::{self, Court, Work};
    use crate::rules::home::Home;
    use crate::rules::world;
    let mut b = barony::Barony::default();
    b.court = Court::Met;
    b.done.push("workings".into());
    b.lit.push("ore-forge".into());
    b.works.push(Work {
        id: "hall".into(),
        labour: barony::work("hall").unwrap().labour / 2,
    });
    let home = Home {
        barony: b,
        ..Home::default()
    };
    let time = |label: &str, mut run: Run| {
        let n = 600;
        let start = std::time::Instant::now();
        for _ in 0..n {
            run.tick += 1;
            std::hint::black_box(frame(&run, NATIVE_W, NATIVE_H));
        }
        let us = start.elapsed().as_micros() as f64 / n as f64;
        eprintln!("FRAME {label:<28} {us:>8.1} us");
    };
    let mut base = Run::at_home(17, 1, Some("Friend"), home.clone(), Default::default());
    base.arrive(0, (12.0, 10.0));
    time("undercroft (before)", base);
    for kind in [
        RoomKind::Gate,
        RoomKind::MineHead,
        RoomKind::KingsHall,
        RoomKind::Stables,
    ] {
        let mut run = Run::at_home(17, 1, Some("Friend"), home.clone(), Default::default());
        let i = world::room_of(&run.dungeon, kind).unwrap();
        run.arrive(i, (12.0, 10.0));
        time(&format!("{kind:?}"), run);
    }
    let mut lists = Run::at_home(17, 1, Some("Friend"), home.clone(), Default::default());
    let i = world::room_of(&lists.dungeon, RoomKind::Lists).unwrap();
    lists.arrive(i, (2.0, 8.5));
    lists.mount_up(1);
    lists.step(&BTreeMap::new());
    lists.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Input::default()
        },
    )]));
    for _ in 0..40 {
        lists.step(&BTreeMap::new());
    }
    time("Lists, a bout", lists);
    // And the simulation step at the mine-head with every folk route live.
    let mut run = Run::at_home(17, 1, None, home, Default::default());
    let i = world::room_of(&run.dungeon, RoomKind::MineHead).unwrap();
    run.arrive(i, (12.0, 10.0));
    let n = 3000;
    let start = std::time::Instant::now();
    for _ in 0..n {
        run.step(&BTreeMap::new());
    }
    eprintln!(
        "STEP {:<29} {:>8.1} us",
        "mine-head",
        start.elapsed().as_micros() as f64 / n as f64
    );
}

#[test]
fn merchant_art_and_price_boards_are_seeded_and_on_palette() {
    use crate::rules::{TILE_UNITS, home::Home, merchant::PLATES};
    let mut run = Run::at_home(23, 7, None, Home::default(), Default::default());
    let camp = run.merchant_camp().unwrap();
    run.enter_for_test(camp.room);
    let mut camp_art = Img::new(NATIVE_W, NATIVE_H);
    merchant::figures(&mut camp_art, &run);
    let mut board = Img::new(NATIVE_W, NATIVE_H);
    merchant::boards(&mut board, &run);
    assert!(
        board.pixels().all(|p| p == BLACK),
        "the price list waits for a knight at the cart"
    );
    let cart = crate::rules::merchant::CART_AT;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (cart.0 * TILE_UNITS, (cart.1 + 2.0) * TILE_UNITS);
    merchant::boards(&mut board, &run);
    assert!(on_palette(&camp_art));
    assert!(on_palette(&board));
    assert!(camp_art.pixels().any(|p| p != BLACK));
    assert!(board.pixels().any(|p| p != BLACK));
    let (c, r) = PLATES[0];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
    let a = native(&run);
    let b = native(&run.clone());
    assert_eq!(a.rgba_bytes(), b.rgba_bytes());
    assert!(on_palette(&a));
    let mut reading = Img::new(NATIVE_W, NATIVE_H);
    merchant::boards(&mut reading, &run);
    assert_ne!(
        board.rgba_bytes(),
        reading.rgba_bytes(),
        "approach shows price, purse and stand-to-buy instructions"
    );
    let cellar = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Home)
        .unwrap();
    run.enter_for_test(cellar);
    let mut absent = Img::new(NATIVE_W, NATIVE_H);
    merchant::figures(&mut absent, &run);
    merchant::boards(&mut absent, &run);
    assert!(
        absent.pixels().all(|p| p == BLACK),
        "estate furniture left alone"
    );
}

include!("overworld__east_shots.rs");
include!("overworld__north_shots.rs");

include!("overworld__coast_shots.rs");

include!("overworld__woodland_shots.rs");

include!("overworld__wetland_shots.rs");

include!("overworld__aerie_shots.rs");

include!("overworld__mummers_shots.rs");

include!("overworld__hippodrome_shots.rs");

include!("overworld__windcountry_shots.rs");

include!("overworld__lineup_shots.rs");

include!("overworld__art_pass_shots.rs");

/// The risen sheet for review: every pose in every direction, west flipped
/// from east. `ANGEL_ARENA_SHOTS=<dir> cargo test write_risen_sheet -- --ignored`.
#[test]
#[ignore]
fn write_risen_sheet() {
    use sprites::{risen, RisenDir, RisenPose};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let poses: [(RisenPose, u32, &str); 8] = [
        (RisenPose::Idle, 0, "IDLE A"),
        (RisenPose::Idle, 1, "IDLE B"),
        (RisenPose::Walk, 0, "WALK A"),
        (RisenPose::Walk, 1, "WALK B"),
        (RisenPose::WindUp, 0, "ATTACK"),
        (RisenPose::Shove, 0, "SHOVE"),
        (RisenPose::Hit, 0, "HIT"),
        (RisenPose::Rest, 0, "REST"),
    ];
    let cell_w = 26;
    let cell_h = 30;
    let mut sheet = Img::black(
        cell_w * 5 + 8,
        cell_h * poses.len() as i32 + 10,
    );
    sheet.text(6, 2, "S    N    E    W", '5');
    for (row, &(pose, frame, label)) in poses.iter().enumerate() {
        let y = 10 + row as i32 * cell_h;
        sheet.text(2, y + 6, label, '4');
        for (col, dir) in [
            RisenDir::South,
            RisenDir::North,
            RisenDir::East,
            RisenDir::East,
        ]
        .into_iter()
        .enumerate()
        {
            let mut im = risen(pose, dir, frame);
            if col == 3 {
                im = im.flip_h();
            }
            sheet.stamp(&im, 40 + col as i32 * cell_w, y);
        }
    }
    let png = image::RgbaImage::from_raw(
        (sheet.w * 4) as u32,
        (sheet.h * 4) as u32,
        sheet.rgba_scaled(4),
    )
    .unwrap();
    png.save(dir.join("risen-sheet.png")).unwrap();
}

/// Humphrey's animation reels for review, as numbered frames:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_risen_reels -- --ignored`,
/// then assemble with ffmpeg (see scripts/README or the release notes).
#[test]
#[ignore]
fn write_risen_reels() {
    use crate::rules::home::Home;
    use crate::rules::{pastimes::Game, world, EnemyKind, RoomKind, HZ};
    use sprites::{risen, RisenDir, RisenPose};
    use std::path::PathBuf;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let sheet_dir = dir.join("sheet-frames");
    let hunt_dir = dir.join("hunt-frames");
    std::fs::create_dir_all(&sheet_dir).unwrap();
    std::fs::create_dir_all(&hunt_dir).unwrap();

    // Reel one: the sprite set, cycling poses and directions on dark paper.
    let beats: [(RisenPose, u32, usize); 11] = [
        (RisenPose::Idle, 0, 4),
        (RisenPose::Idle, 1, 4),
        (RisenPose::Walk, 0, 4),
        (RisenPose::Walk, 1, 4),
        (RisenPose::Walk, 0, 4),
        (RisenPose::Walk, 1, 4),
        (RisenPose::WindUp, 0, 4),
        (RisenPose::Shove, 0, 4),
        (RisenPose::Hit, 0, 3),
        (RisenPose::Rest, 0, 3),
        (RisenPose::Rest, 1, 3),
    ];
    let mut n = 0u32;
    for (pose, frame, hold) in beats {
        for rep in 0..hold {
            let dirn = match (pose, rep % 4) {
                (_, 0) => RisenDir::South,
                (_, 1) => RisenDir::East,
                (_, 2) => RisenDir::North,
                _ => RisenDir::East,
            };
            let mut im = risen(pose, dirn, frame);
            if rep % 4 == 3 {
                im = im.flip_h();
            }
            let mut card = Img::black(96, 96);
            card.stamp(&im, (96 - im.w) / 2, (96 - im.h) / 2 - 4);
            let png = image::RgbaImage::from_raw(96 * 4, 96 * 4, card.rgba_scaled(4)).unwrap();
            png.save(sheet_dir.join(format!("f_{n:04}.png"))).unwrap();
            n += 1;
        }
    }

    // Reel two: the hunt, in the game's own frame. Humphrey claws in,
    // shambles over, shoves the knight flat, is felled, dragged under by
    // the hand, and claws straight back up — larger.
    let mut run = crate::rules::Run::at_home(
        17,
        1,
        Some("Friend"),
        Home::default(),
        Default::default(),
    );
    run.come_to_the_gate();
    let fair = world::room_of(&run.dungeon, RoomKind::Fairground).unwrap();
    run.arrive(fair, (12.0, 7.0));
    // A festival worth interrupting: three rounds, one win, and a party
    // stood about long enough for the compost to stir.
    run.home.countryside.games.plays[Game::Barrel.index() as usize] = 3;
    run.home.countryside.games.wins[Game::Barrel.index() as usize] = 1;
    let calm: BTreeMap<u32, Input> = BTreeMap::new();
    let mut waited = 0;
    while run.enemies.iter().all(|e| e.kind != EnemyKind::Risen) {
        run.step(&calm);
        waited += 1;
        assert!(waited < 20 * HZ as usize, "he never clawed in");
    }
    let id = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Risen)
        .unwrap()
        .id;
    let hero_id = *run.players.keys().next().unwrap();
    let mut felled_at = None;
    let mut f = 0u32;
    for t in 0..(13 * HZ as usize) {
        if t == 8 {
            // Close the commute for the reel; he starts a few strides out.
            let (hx, hy) = {
                let h = run.players.get(&hero_id).unwrap();
                (h.x, h.y)
            };
            let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
            (e.x, e.y) = (hx - 7.0, hy + 1.0);
        }
        if t == 8 * HZ as usize {
            if let Some(e) = run.enemies.iter_mut().find(|e| e.id == id && e.hp > 0) {
                e.hp = 0;
                felled_at = Some(t);
            }
        }
        run.step(&calm);
        if let Some(h) = run.players.get_mut(&hero_id) {
            // The knight stands their ground for the whole reel.
            h.hp = h.max_hp;
        }
        if t % 2 == 0 {
            let img = frame(&run, NATIVE_W, NATIVE_H);
            let png =
                image::RgbaImage::from_raw(NATIVE_W as u32 * 3, NATIVE_H as u32 * 3, img.rgba_scaled(3))
                    .unwrap();
            png.save(hunt_dir.join(format!("f_{f:04}.png"))).unwrap();
            f += 1;
        }
        // After the hand has taken him, he claws straight back (bigger).
        if felled_at.is_some_and(|at| t == at + 5 * HZ as usize) {
            run.home.countryside.humphrey_dig = 0;
        }
    }
}

/// Stills of the dimensional stages for review.
#[test]
#[ignore]
fn write_3d_stage_shots() {
    use crate::rules::home::Home;
    use crate::rules::{pastimes::Game, world, Input, RoomKind};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let png = image::RgbaImage::from_raw(
            (img.w * 3) as u32,
            (img.h * 3) as u32,
            img.rgba_scaled(3),
        )
        .unwrap();
        png.save(dir.join(name)).unwrap();
    };
    // The duck range: start a round, sweep the gun, let some BBs fly.
    let mut run = crate::rules::Run::at_home(
        17,
        1,
        Some("Friend"),
        Home::default(),
        Default::default(),
    );
    run.come_to_the_gate();
    let fair = world::room_of(&run.dungeon, RoomKind::Fairground).unwrap();
    run.arrive(fair, Game::DuckShoot.start());
    run.step(&BTreeMap::new());
    assert!(run.pastimes.round.is_some());
    save("duck-0.png", &native(&run));
    for t in 0..90 {
        let sweep = ((t / 6) % 3) as i8 - 1;
        let aim = BTreeMap::from([(
            1,
            Input {
                aim_x: sweep,
                aim_y: if t % 12 < 6 { -1 } else { 1 },
                fire: t % 5 == 0,
                ..Default::default()
            },
        )]);
        run.step(&aim);
        if t == 24 {
            save("duck-fired.png", &native(&run));
        }
    }
    save("duck-late.png", &native(&run));

    // The half-pipe: pump along the wall, then catch whatever air the
    // published windows allow.
    let mut run = crate::rules::Run::at_home(
        17,
        1,
        Some("Friend"),
        Home::default(),
        Default::default(),
    );
    run.come_to_the_gate();
    let mum = world::room_of(&run.dungeon, RoomKind::Mummers).unwrap();
    run.arrive(mum, Game::Halfpipe.start());
    run.step(&BTreeMap::new());
    assert!(run.pastimes.round.is_some());
    for t in 0..150 {
        let pump = if t / 30 % 2 == 0 { 1 } else { -1 };
        let aim = BTreeMap::from([(
            1,
            Input {
                aim_x: pump,
                fire: t == 84,
                ..Default::default()
            },
        )]);
        run.step(&aim);
        if t == 60 {
            save("pipe-roll.png", &native(&run));
        }
    }
    save("pipe-late.png", &native(&run));
}

/// Animation reels of the dimensional stages.
#[test]
#[ignore]
fn write_3d_reels() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::{DuckShoot, Game, LIP_CLOSE, LIP_OPEN};
    use crate::rules::{world, Input, RoomKind, HZ};
    use std::path::PathBuf;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let duck_dir = PathBuf::from(&dir).join("duck-frames");
    let pipe_dir = PathBuf::from(&dir).join("pipe-frames");
    std::fs::create_dir_all(&duck_dir).unwrap();
    std::fs::create_dir_all(&pipe_dir).unwrap();

    // The range: a gunner works the near rail, leading the ducks.
    let mut run = crate::rules::Run::at_home(
        17,
        1,
        Some("Friend"),
        Home::default(),
        Default::default(),
    );
    run.come_to_the_gate();
    let fair = world::room_of(&run.dungeon, RoomKind::Fairground).unwrap();
    run.arrive(fair, Game::DuckShoot.start());
    run.step(&BTreeMap::new());
    let mut f = 0;
    for t in 0..(9 * HZ as usize) {
        let range = run.pastimes.round.as_ref().unwrap().duckshoot.clone().unwrap();
        let duck = &range.ducks[0];
        let sx = DuckShoot::screen_x(duck.x + duck.vx, 0);
        let (dx, dy) = (192 - i32::from(range.ax), 163 - i32::from(range.ay));
        let axis = |d: i32| if d.abs() <= 4 { 0 } else { d.signum() as i8 };
        let fire = dx.abs() <= 4
            && dy.abs() <= 4
            && duck.down == 0
            && (sx - 192).abs() <= 6;
        run.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: axis(dx),
                aim_y: axis(dy),
                fire,
                ..Default::default()
            },
        )]));
        if run.pastimes.round.is_none() {
            break;
        }
        if t % 2 == 0 {
            let img = native(&run);
            let png = image::RgbaImage::from_raw(
                (img.w * 3) as u32,
                (img.h * 3) as u32,
                img.rgba_scaled(3),
            )
            .unwrap();
            png.save(duck_dir.join(format!("f_{f:04}.png"))).unwrap();
            f += 1;
        }
    }

    // The pipe: pump the walls, catch the window, spin what spins.
    let mut run = crate::rules::Run::at_home(
        17,
        1,
        Some("Friend"),
        Home::default(),
        Default::default(),
    );
    run.come_to_the_gate();
    let mum = world::room_of(&run.dungeon, RoomKind::Mummers).unwrap();
    run.arrive(mum, Game::Halfpipe.start());
    run.step(&BTreeMap::new());
    let mut f = 0;
    for t in 0..(10 * HZ as usize) {
        let pipe = run.pastimes.round.as_ref().unwrap().halfpipe.clone().unwrap();
        let dir = crate::rules::pastimes::Halfpipe::direction(
            run.pastimes.round.as_ref().unwrap().progress,
            run.pastimes.round.as_ref().unwrap().mistakes,
        );
        let pumping = pipe.flight == 0;
        let spin_air = pipe.flight > 0 && pipe.spin < 576;
        let aim_x = if pumping {
            dir
        } else if spin_air {
            dir
        } else {
            0
        };
        let fire = pumping
            && pipe.phase >= LIP_OPEN
            && pipe.phase < LIP_CLOSE
            && pipe.momentum >= 60;
        run.step(&BTreeMap::from([(
            1,
            Input {
                aim_x,
                fire,
                ..Default::default()
            },
        )]));
        if run.pastimes.round.is_none() {
            break;
        }
        if t % 2 == 0 {
            let img = native(&run);
            let png = image::RgbaImage::from_raw(
                (img.w * 3) as u32,
                (img.h * 3) as u32,
                img.rgba_scaled(3),
            )
            .unwrap();
            png.save(pipe_dir.join(format!("f_{f:04}.png"))).unwrap();
            f += 1;
        }
    }
}

/// Every signature pose through the same native frame sent to browser seats.
#[test]
#[ignore = "offline boss art review"]
fn write_new_boss_shots() {
    use crate::rules::{crawl_floor, signatures as fight, skills};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, run: &Run| {
        let im = native(run);
        assert!(on_palette(&im));
        image::RgbaImage::from_raw(im.w as u32 * 3, im.h as u32 * 3, im.rgba_scaled(3))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let setup = |id: &str, depth, pack| {
        let mut run = Run::new(17, 1, Some("Friend"));
        run.dungeon = crawl_floor(depth, pack, 17);
        let at = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(at);
        run.enemies.clear();
        run.projectiles.clear();
        run.calm_for_test();
        run.works.clouds.clear();
        run.works.perils.clear();
        let index = run.bosses.iter().position(|b| b.id == id).unwrap();
        run.spawn_boss_at_for_test(index, 24.0, 20.0);
        run.enemies[0].signature.active = true;
        for (id, h) in &mut run.players {
            h.x = if *id == 1 { 10.0 } else { 36.0 };
            h.y = 22.0;
            h.invulnerable = 10000;
        }
        run.phase = Phase::Fighting;
        run.tick = 90;
        run
    };
    let base = setup(fight::RIPPER, 5, Pack::Fungal);
    for (name, stage, timer) in [
        ("ripper-01-rest", 0, 0),
        ("ripper-02-swell-100", fight::INHALE, 1),
        ("ripper-03-swell-135", fight::INHALE, 24),
        ("ripper-04-swell-170", fight::INHALE, 48),
        ("ripper-05-swell-200", fight::TELL, 0),
        ("ripper-06-telegraph", fight::TELL, 14),
        ("ripper-09-dazed", fight::DAZED, 12),
    ] {
        let mut run = base.clone();
        let e = &mut run.enemies[0];
        e.stage = stage;
        e.timer = timer;
        e.dir = (0.92, -0.39);
        save(name, &run);
    }
    let mut rip = base.clone();
    // Keep this action's whole launch corridor in the review camera.
    let room = &mut rip.dungeon.rooms[rip.at];
    for y in 9..16 {
        for x in 5..20 {
            room.set_for_test(x, y, Tile::Floor);
        }
    }
    rip.enemies[0].stage = fight::DASH;
    rip.enemies[0].x = 34.0;
    rip.enemies[0].y = 25.0;
    rip.enemies[0].dir = (-0.98, -0.20);
    rip.enemies[0].signature.left = 1;
    rip.enemies[0].signature.cycle = 1;
    for _ in 0..14 {
        rip.step(&BTreeMap::new());
    }
    save("ripper-07-mid-rip", &rip);
    let mut room = base.clone();
    for row in 0..3 {
        for n in 0..10 {
            fight::add_wall(
                &mut room.works.clouds,
                skills::Cloud {
                    x: 6.0 + n as f32 * 1.6 + row as f32 * 5.0,
                    y: 9.5 + row as f32 * 5.0 + n as f32 * 0.3,
                    vx: 0.0,
                    vy: 0.0,
                    age: 50,
                    owner: 0,
                    power: 4,
                    tint: 3,
                },
            );
        }
    }
    room.enemies[0].stage = fight::DAZED;
    room.enemies[0].x = 32.0;
    room.enemies[0].y = 18.0;
    save("ripper-08-carved-room", &room);
    let engine = setup(fight::ENGINE, 2, Pack::Cavern);
    for (name, tick) in [
        ("engine-01-skitter-a", 90),
        ("engine-02-skitter-b", 95),
        ("engine-02-skitter-c", 96),
        ("engine-02-skitter-d", 100),
    ] {
        let mut r = engine.clone();
        r.tick = tick;
        save(name, &r);
    }
    let mut rails = engine.clone();
    rails.enemies[0].timer = 104;
    rails.step(&BTreeMap::new());
    for _ in 0..14 {
        rails.step(&BTreeMap::new());
    }
    save("engine-03-rails-lighting", &rails);
    for _ in 0..4 {
        rails.step(&BTreeMap::new());
    }
    save("engine-03-rails-pulse", &rails);
    for _ in 0..20 {
        rails.step(&BTreeMap::new());
    }
    assert_eq!(
        rails.enemies[0].stage,
        fight::DASH,
        "review the charge in motion"
    );
    save("engine-04-mid-charge", &rails);
    let mut coal = engine.clone();
    coal.enemies[0].age = 28;
    for _ in 0..60 {
        coal.step(&BTreeMap::new());
        if coal.projectiles.iter().any(|p| p.hostile) {
            break;
        }
    }
    coal.enemies[0].timer = 0;
    coal.slams.clear();
    coal.blasts.clear();
    for _ in 0..18 {
        coal.step(&BTreeMap::new());
    }
    assert!(
        coal.projectiles.iter().any(|p| p.hostile),
        "coal volley is visible"
    );
    save("engine-05-coal-shot", &coal);
    let mut babies = engine.clone();
    babies.enemies[0].hp = babies.enemies[0].max_hp * 45 / 100;
    babies.step(&BTreeMap::new());
    for _ in 0..35 {
        babies.step(&BTreeMap::new());
    }
    assert!(
        babies
            .enemies
            .iter()
            .any(|e| e.kind == EnemyKind::Spiderling && e.stage == fight::TENDER)
    );
    save("engine-06-coal-tenders", &babies);
    let mut steam = engine.clone();
    steam.enemies[0].timer = 80;
    save("engine-07-steam", &steam);
    let mut stomp = engine.clone();
    stomp.enemies[0].timer = 35;
    for _ in 0..16 {
        stomp.step(&BTreeMap::new());
    }
    save("engine-09-stomp-ring", &stomp);
    babies.enemies[0].timer = 104;
    babies.step(&BTreeMap::new());
    for _ in 0..160 {
        babies.step(&BTreeMap::new());
        if babies.enemies[0].stage == fight::JUMP && babies.enemies[0].timer == 12 {
            break;
        }
    }
    assert_eq!(babies.enemies[0].stage, fight::JUMP);
    save("engine-08-rail-jump", &babies);
}

#[test]
fn kingdom_entrance_prompts_stay_clear_as_cameras_move_past_every_sign() {
    use super::super::kingdom;
    use crate::rules::home::Home;
    let mut run = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    let geo = kingdom::geography(&run.kingdom.wishes);
    for site in &geo.sites {
        run.walk_kingdom(Some(site.stand));
        let a = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
        // Put another knight and an edge marker in the prompt's band,
        // while camera easing moves the building's own sign across it.
        let hero = run.players.get_mut(&1).unwrap();
        hero.camera = Some((hero.x - 6.0, hero.y + 8.0));
        let origin = camera(&run, Some(1));
        let friend = run.players.get_mut(&2).unwrap();
        (friend.x, friend.y) = (
            (origin.0 + NATIVE_W / 2) as f32 / UNIT,
            (origin.1 + NATIVE_H - 4) as f32 / UNIT,
        );
        run.tick += 11;
        let b = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
        for y in NATIVE_H - 24..NATIVE_H {
            for x in 0..NATIVE_W {
                assert_eq!(a.get(x, y), b.get(x, y), "{} at {x},{y}", site.name);
            }
        }
        run.players.get_mut(&2).unwrap().y += 40.0;
        let c = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
        for y in NATIVE_H - 24..NATIVE_H {
            for x in 0..NATIVE_W {
                assert_eq!(b.get(x, y), c.get(x, y), "{} edge at {x},{y}", site.name);
            }
        }
    }
}

/// ANGEL_ARENA_SHOTS=<dir> cargo test write_kingdom_shots -- --ignored --nocapture
#[test]
#[ignore]
fn write_kingdom_shots() {
    use super::super::kingdom;
    use crate::rules::{home::Home, world};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    run.walk_kingdom(Some(kingdom::keep_stand()));
    let geo = kingdom::geography(&run.kingdom.wishes);
    // A continuous road walk from the Keep to the College. Route through
    // actual collision, sampling camera frames as the knight advances.
    let from = kingdom::keep_stand();
    let to = geo
        .sites
        .iter()
        .find(|s| s.room == RoomKind::CollegeGrounds)
        .unwrap()
        .stand;
    let far = geo
        .sites
        .iter()
        .filter(|s| s.room == RoomKind::RealmHall)
        .max_by_key(|s| s.stand.1)
        .unwrap()
        .stand;
    let mut route = vec![from];
    for (from, to) in [(from, far), (far, to)] {
        let mut todo = std::collections::VecDeque::from([from]);
        let mut parents = std::collections::BTreeMap::from([(from, from)]);
        while let Some((x, y)) = todo.pop_front() {
            if (x, y) == to {
                break;
            }
            for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if geo.room.tile(p.0, p.1) == Tile::Floor && !parents.contains_key(&p) {
                    parents.insert(p, (x, y));
                    todo.push_back(p);
                }
            }
        }
        let mut segment = vec![to];
        let mut at = to;
        while at != from {
            at = parents[&at];
            segment.push(at);
        }
        segment.reverse();
        route.extend(segment.into_iter().skip(1));
    }
    let mut frames = 0;
    let mut ticks = 0;
    for target in route.iter().skip(1) {
        let point = ((target.0 as f32 + 0.5) * 2.0, (target.1 as f32 + 0.5) * 2.0);
        for _ in 0..20 {
            let h = &run.players[&1];
            let (dx, dy) = (point.0 - h.x, point.1 - h.y);
            if dx.hypot(dy) < 0.35 {
                break;
            }
            let input = Input {
                move_x: if dx.abs() > 0.3 { dx.signum() as i8 } else { 0 },
                move_y: if dy.abs() > 0.3 { dy.signum() as i8 } else { 0 },
                ..Default::default()
            };
            run.step(&BTreeMap::from([(1, input)]));
            ticks += 1;
            if ticks % 180 == 0 && frames < 12 {
                save(
                    &format!("sweep-{frames:02}.png"),
                    &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
                );
                frames += 1;
            }
        }
    }
    save(
        "sweep-college.png",
        &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
    );
    for (kind, name) in [
        (RoomKind::CollegeGrounds, "college"),
        (RoomKind::Ranch, "ranch"),
        (RoomKind::Gate, "gate"),
    ] {
        let (index, site) = geo
            .sites
            .iter()
            .enumerate()
            .find(|(_, s)| s.room == kind)
            .unwrap();
        run.walk_kingdom(Some(site.stand));
        run.follow_kingdom_cameras();
        save(
            &format!("{name}-door.png"),
            &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
        );
        run.step(&BTreeMap::new());
        run.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert_eq!(run.room().kind, kind);
        save(
            &format!("{name}-inside.png"),
            &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
        );
        if kind == RoomKind::CollegeGrounds {
            run.take_entrance(
                world::ENTRANCES
                    .iter()
                    .find(|e| e.id == "college-hall")
                    .unwrap(),
            );
            save(
                "college-hall.png",
                &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
            );
            run.take_entrance(
                world::ENTRANCES
                    .iter()
                    .find(|e| e.id == "hall-out")
                    .unwrap(),
            );
            run.take_entrance(
                world::ENTRANCES
                    .iter()
                    .find(|e| e.id == "college-out")
                    .unwrap(),
            );
        } else if kind == RoomKind::Ranch {
            run.take_entrance(
                world::ENTRANCES
                    .iter()
                    .find(|e| e.id == "ranch-out")
                    .unwrap(),
            );
        } else {
            run.leave_kingdom_place();
        }
        assert_eq!(
            run.room().kind,
            RoomKind::Kingdom,
            "return from site {index}"
        );
        save(
            &format!("{name}-back.png"),
            &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
        );
    }
    let artisans = geo.sites.iter().find(|s| s.name == "Artisan Quarter").unwrap();
    run.walk_kingdom(Some(artisans.stand));
    save(
        "artisan-door.png",
        &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
    );
    run.walk_kingdom(Some(kingdom::keep_stand()));
    let far = geo
        .sites
        .iter()
        .rev()
        .find(|s| s.name.len() > 0)
        .unwrap()
        .stand;
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y) = ((far.0 as f32 + 0.5) * 2.0, (far.1 as f32 + 0.7) * 2.0);
    friend.camera = Some((friend.x, friend.y - 1.0));
    save(
        "seat-host.png",
        &frame_for(&run, NATIVE_W, NATIVE_H, Some(1)),
    );
    save(
        "seat-friend.png",
        &frame_for(&run, NATIVE_W, NATIVE_H, Some(2)),
    );
    save("terminal.png", &frame_for(&run, 135, 78, Some(1)));
    let mut scene = super::super::Scene::resting();
    scene.ambient = 0.84;
    scene.wishes = run.kingdom.wishes.clone();
    save(
        "overview.png",
        &kingdom::landscape(&scene, super::super::View::realm()),
    );
}

#[test]
#[ignore]
fn kingdom_frame_times() {
    use crate::rules::home::Home;
    let mut kingdom = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    kingdom.walk_kingdom(None);
    let mut room = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    room.come_to_the_gate();
    for (w, h) in [(384, 224), (135, 78)] {
        frame_for(&kingdom, w, h, Some(1));
        frame_for(&room, w, h, Some(1));
        let mut samples = [Vec::new(), Vec::new()];
        for i in 0..500 {
            for (j, run) in [&mut room, &mut kingdom].into_iter().enumerate() {
                run.tick += 1;
                let start = std::time::Instant::now();
                std::hint::black_box(frame_for(run, w, h, Some(1)));
                samples[j].push(start.elapsed().as_secs_f64() * 1000.0);
            }
            if i == 0 {
                samples.iter_mut().for_each(|v| v.clear());
            }
        }
        for (name, mut values) in ["room", "kingdom"].into_iter().zip(samples) {
            values.sort_by(f64::total_cmp);
            println!(
                "{name} {w}x{h}: median {:.3} ms p95 {:.3} ms",
                values[values.len() / 2],
                values[values.len() * 95 / 100]
            );
        }
    }
    let start = std::time::Instant::now();
    for _ in 0..3000 {
        kingdom.step(&BTreeMap::new());
    }
    println!(
        "kingdom simulation {:.3} ms/tick",
        start.elapsed().as_secs_f64() / 3.0
    );
}

/// Naked-start review: real E pickups and the shared arena/tray renderer.
#[test]
#[ignore]
fn write_naked_start_shots() {
    use crate::rules::home::Home;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return; };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes()).unwrap()
            .save(dir.join(name)).unwrap();
    };
    let mut run = Run::at_home(17, 1, None, Home::default(), Default::default());
    run.naked_start = true;
    run.restore_gear(1);
    run.descend_for_test();
    save("06-fresh-floor-one-cache.png", &frame(&run, 768, 448));
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.dungeon.rooms[run.at].items.clear();
    run.dungeon.rooms[run.at].chest = None;
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (24.0, 20.0, 1.0, 0.0);
    let review = |run: &Run| {
        let mut canvas = Img::new(640, 320);
        canvas.rect(0, 0, 640, 320, 'K');
        let body = knights::figure(&run.players[&1], 1);
        let large = image::RgbaImage::from_raw(16, 16, body.rgba_bytes()).unwrap();
        let large = image::imageops::resize(&large, 128, 128, image::imageops::FilterType::Nearest);
        let arena = frame(run, 480, 280);
        canvas.stamp(&arena, 160, 20);
        let mut out = image::RgbaImage::from_raw(640, 320, canvas.rgba_bytes()).unwrap();
        image::imageops::overlay(&mut out, &large, 16, 80);
        out
    };
    review(&run).save(dir.join("00-naked.png")).unwrap();
    for (name, card) in [
        ("01-weapon-found.png", "bow"),
        ("02-body-armour.png", "body-armour"),
        ("03-iron-helm.png", "iron-helm"),
        ("04-shield.png", "kite-shield"),
    ] {
        let h = &run.players[&1];
        let (x, y) = (h.x, h.y);
        run.dungeon.rooms[run.at].items.push(Item { card: card.into(), x, y, held_off: None });
        run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
        run.step(&BTreeMap::new());
        review(&run).save(dir.join(name)).unwrap();
    }
    let mut naked = Run::at_home(17, 2, None, Home::default(), Default::default());
    naked.naked_start = true;
    naked.restore_gear(1);
    naked.descend_for_test();
    naked.step(&BTreeMap::from([(1, Input { fire: true, aim_x: 1, ..Default::default() })]));
    save("05-punch.png", &frame(&naked, 768, 448));
}

/// The floor's distance to a wall, as it was walked pixel by pixel: the
/// reckoning by tiles gives the same everywhere.
#[test]
fn a_floor_pixels_distance_to_the_wall_is_the_walk_it_was() {
    let walked = |room: &Room, x: i32, y: i32| -> i32 {
        let mut best = TILE;
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            for d in 1..TILE {
                if room.tile((x + dx * d).div_euclid(TILE), (y + dy * d).div_euclid(TILE)) == Tile::Wall {
                    best = best.min(d);
                    break;
                }
            }
        }
        best
    };
    let mut checked = 0;
    for seed in 0..4 {
        let run = Run::new(seed, 1, None);
        for room in &run.dungeon.rooms {
            let (pw, ph) = room_px(room);
            for y in 0..ph {
                for x in 0..pw {
                    if room.tile(x / TILE, y / TILE) == Tile::Floor {
                        assert_eq!(near_wall(room, x, y), walked(room, x, y), "{:?} at {x},{y}", room.kind);
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 30_000);
}

#[test]
fn an_away_knight_is_marked_and_a_holding_room_says_so() {
    let mut run = scene(5, RoomKind::Fight, 0);
    let plain = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let mut marked = plain.clone();
    presence_marks(&mut marked, &run, Some(1));
    assert_eq!(marked.words(), plain.words(), "everyone here, nothing held: nothing drawn");
    run.set_away(2, true);
    run.hold(crate::rules::Hold::Arriving);
    let mut img = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    presence_marks(&mut img, &run, Some(1));
    // The banner sits at the top in the middle; the tag over knight 2.
    let changed = |x0: i32, y0: i32, x1: i32, y1: i32| {
        (y0..y1).any(|y| (x0..x1).any(|x| img.get(x, y) != plain.get(x, y)))
    };
    assert!(changed(NATIVE_W / 2 - 20, 12, NATIVE_W / 2 + 20, 33), "the banner");
    let (x, y) = frame_px(&run, NATIVE_W, NATIVE_H, Some(1), run.players[&2].x, run.players[&2].y);
    assert!(changed(x - 10, y - 30, x + 10, y - 19), "the tag");
    assert!(on_palette(&img));
    // Two away side by side: the second tag sits above the first.
    run.set_away(1, true);
    let hero = run.players[&2].clone();
    let one = run.players.get_mut(&1).unwrap();
    (one.x, one.y) = (hero.x + 1.0, hero.y);
    let mut two = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    presence_marks(&mut two, &run, Some(1));
    let (x, y) = frame_px(&run, NATIVE_W, NATIVE_H, Some(1), hero.x, hero.y);
    let base = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let tagged = |y0: i32| (y0..y0 + 11).any(|y| (x - 10..x + 10).any(|x| two.get(x, y) != base.get(x, y)));
    assert!(tagged(y - 30) && tagged(y - 42), "stacked, not overprinted");
}
