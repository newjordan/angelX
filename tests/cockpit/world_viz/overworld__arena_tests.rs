use super::*;
use crate::drive::together_shooter::{Input, Item, RoomKind};
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
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
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

#[test]
fn the_heroes_are_the_mini_viz_knight() {
    let mut run = scene(11, RoomKind::Fight, 0);
    run.enemies.clear();
    run.projectiles.clear();
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
    use crate::drive::together_realm::{Spoil, Spoils};
    use crate::drive::together_shooter::home::{Home, Station};
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
    use crate::drive::together_realm::{Spoil, Spoils};
    use crate::drive::together_shooter::fortune::Mode;
    use crate::drive::together_shooter::home::{Home, Station};
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
    let fire = BTreeMap::from([(1, Input { fire: true, ..Default::default() })]);
    for _ in 0..crate::drive::together_shooter::home::BUY_HOLD + 1 {
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
    for (name, mode) in [("lights-out", Mode::LightsOut), ("giants", Mode::GiantsFeast)] {
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
    use crate::drive::together_shooter::{EnemyKind, knights};
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
        run.players.get_mut(&1).unwrap().ult_charge = crate::drive::together_shooter::ults::ULT_FULL;
        let ult = BTreeMap::from([(1, Input { ult: true, ..Default::default() })]);
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
    use crate::drive::together_shooter::EnemyKind as K;
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
        ("sapper", Pack::Cavern, vec![(K::Sapper, 34.0, 10.0), (K::Sapper, 30.0, 20.0)], 125),
        ("necro", Pack::Crypt, vec![(K::Necromancer, 36.0, 8.0)], 100),
        ("boar", Pack::Cavern, vec![(K::Warboar, 40.0, 12.0)], 72),
        ("slime", Pack::Crypt, vec![(K::Slime, 34.0, 10.0), (K::Slime, 14.0, 18.0)], 20),
        ("goblin", Pack::Hellforge, vec![(K::Goblin, 30.0, 10.0), (K::Imp, 36.0, 18.0)], 40),
        ("hob", Pack::Cavern, vec![(K::Hob, 36.0, 8.0), (K::Hob, 38.0, 20.0)], 52),
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
        run.phase = crate::drive::together_shooter::Phase::Fighting;
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
    for (name, kind) in [("archive", RoomKind::Fight), ("fungal", RoomKind::Fight), ("unknown", RoomKind::Fight)] {
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
    use crate::drive::together_shooter::feats::Tier;
    use crate::drive::together_shooter::home::Home;
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
    let (c, r, w, h) = crate::drive::together_shooter::fortune::COFFER_PLATE;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * 2.0;
    hero.y = (r as f32 + h as f32 / 2.0) * 2.0;
    run.step(&BTreeMap::new());
    save("coffer-reading.png", &native(&run));
    let mut spoils = crate::drive::together_realm::Spoils::default();
    spoils.add(crate::drive::together_realm::Spoil::Gold, 260);
    spoils.add(crate::drive::together_realm::Spoil::Gem, 2);
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
    use crate::drive::together_shooter::home::Home;
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
        let mut treasury = crate::drive::together_realm::Spoils::default();
        treasury.add(crate::drive::together_realm::Spoil::Gold, 200);
        let mut run = Run::at_home(8, 1, Some("Friend"), home, treasury);
        let yard = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Yard)
            .unwrap();
        run.enter_for_test(yard);
        let q = crate::drive::together_shooter::yard::QUINTAINS[1];
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (q.0 * 2.0, q.1 * 2.0 + 7.0);
        if goblins > 0 {
            let (c, r, w, _) = crate::drive::together_shooter::yard::STALL_PLATES[0];
            let friend = run.players.get_mut(&2).unwrap();
            (friend.x, friend.y) = ((c as f32 + w as f32 / 2.0) * 2.0, (r as f32 + 0.5) * 2.0);
        }
        let fire = BTreeMap::from([(1, Input { aim_y: -1, fire: true, ..Default::default() })]);
        for _ in 0..50 {
            run.step(&fire);
        }
        save(&format!("{name}.png"), &native(&run));
    }
}

#[test]
#[ignore]
fn write_audience_shots() {
    use crate::drive::together_shooter::audience::{BOX_FALL, MILESTONE};
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
    run.phase = crate::drive::together_shooter::Phase::Exploring;
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
    use crate::drive::together_shooter::bounties::Pinned;
    use crate::drive::together_shooter::home::Home;
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
    use crate::drive::together_shooter::EnemyKind as K;
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
        run.phase = crate::drive::together_shooter::Phase::Fighting;
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
    for _ in 0..8 * crate::drive::together_shooter::HZ {
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
    for _ in 0..13 * crate::drive::together_shooter::HZ {
        hold(&mut run);
    }
    save("hunter-brood.png", &native(&run));
}

#[test]
#[ignore]
fn write_trophy_shots() {
    use crate::drive::together_shooter::home::Home;
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
    use crate::drive::together_shooter::home::Home;
    use crate::drive::together_shooter::talents::{LECTERNS, LEVELS, Prowess};
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
    use crate::drive::together_shooter::EnemyKind as K;
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
    for _ in 0..(9 * crate::drive::together_shooter::HZ) {
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
    use crate::drive::together_shooter::home::{Home, Station};
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
            .push(crate::drive::together_shooter::Item {
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
    use crate::drive::together_shooter::home::{Home, Station};
    use crate::drive::together_shooter::rescues::Captive;
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
    use crate::drive::together_shooter::runes::{Held, Rune, RuneKind};
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
    use crate::drive::together_realm::Spoils;
    use crate::drive::together_shooter::fortune::{Mode, SPIN_TICKS, Spin};
    use crate::drive::together_shooter::home::{Home, Station};
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
    let (c, r, w, h) = crate::drive::together_shooter::fortune::FORTUNE_PLATE;
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
    use crate::drive::together_shooter::EnemyKind;
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
    for _ in 0..8 * crate::drive::together_shooter::HZ {
        for hero in run.players.values_mut() {
            (hero.hp, hero.invulnerable) = (hero.max_hp, 0);
        }
        run.step(&BTreeMap::new());
        let flying = run
            .projectiles
            .iter()
            .any(|p| p.kind == crate::drive::together_shooter::Shot::Hex && p.x < 27.0);
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
    let (sx, sy) = crate::drive::together_shooter::secrets::snibbet_at(run.room());
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
    use crate::drive::together_shooter::home::{Home, Station};
    use crate::drive::together_shooter::merlin::MERLIN_AT;
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
    use crate::drive::together_shooter::home::{Home, Station};
    use crate::drive::together_shooter::tavern::{BOARD, SIEGE, WING_PLATE};
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
    let tap = crate::drive::together_shooter::tavern::TAPS[1];
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
    let plate = crate::drive::together_shooter::tavern::SONG_PLATES[1];
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
    use crate::drive::together_shooter::{EnemyKind, Tile};
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
    for _ in 0..10 * crate::drive::together_shooter::HZ {
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
            .any(|p| p.kind == crate::drive::together_shooter::Shot::Frost);
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
    use crate::drive::together_shooter::Item;
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
    use crate::drive::together_shooter::{Pack, tide};
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
        for _ in 0..4 * crate::drive::together_shooter::HZ {
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
    use crate::drive::together_shooter::hireling::HIRE_PLATE;
    use crate::drive::together_shooter::home::{Home, Station};
    use crate::drive::together_shooter::{EnemyKind, HZ};
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
            .filter(|p| !p.hostile && p.kind == crate::drive::together_shooter::Shot::Blade)
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
    use crate::drive::together_shooter::hollow::{HOLE_PULL, HOLE_WARN, Hole};
    use crate::drive::together_shooter::{EnemyKind, Pack};
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
    use crate::drive::together_shooter::home::Home;
    use crate::drive::together_shooter::world;
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
    ] {
        let mut r = run.clone();
        let i = world::room_of(&r.dungeon, kind).unwrap();
        r.arrive(i, (12.0, 9.5));
        save(name, &native(&r));
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
    use crate::drive::chivalry::{Mount, Stable};
    use crate::drive::together_shooter::home::{BUY_HOLD, Home};
    use crate::drive::together_shooter::joust::{Aim, Stage};
    use crate::drive::together_shooter::world;
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
        run.step(&keys(true, false, 0));
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
    use crate::drive::together_realm::{Spoil, Spoils};
    use crate::drive::together_shooter::barony::{self, Court, Work};
    use crate::drive::together_shooter::home::Home;
    use crate::drive::together_shooter::{Pack, world};
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
    b.mission = Some(crate::drive::together_shooter::bounties::Pinned {
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
        let figs = crate::drive::together_shooter::folk::figures(
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
    use crate::drive::together_shooter::barony::{self, Court, Work};
    use crate::drive::together_shooter::home::Home;
    use crate::drive::together_shooter::world;
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
