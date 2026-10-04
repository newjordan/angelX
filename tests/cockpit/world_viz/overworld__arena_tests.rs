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
    let (_, over) = layers(&run, Some(1));
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
        let (scenery, over) = {
            let (pw, ph) = (run.room().cols as i32 * 16, run.room().rows as i32 * 16);
            let mut whole = scenery(&run);
            let mut figs = Img::new(pw, ph);
            figures(&mut figs, &run);
            whole.stamp(&figs, 0, 0);
            (whole, ())
        };
        let _ = over;
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
